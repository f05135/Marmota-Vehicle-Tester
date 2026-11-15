use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::{OnceLock, RwLock};
use tracing::{debug, info};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VehicleBranch {
    Army,
    Aviation,
    Naval,
}

impl VehicleBranch {
    pub fn as_str(&self) -> &'static str {
        match self {
            VehicleBranch::Army => "army",
            VehicleBranch::Aviation => "aviation",
            VehicleBranch::Naval => "ships",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VehicleClassification {
    pub id: String,
    pub branch: VehicleBranch,
    pub block_type: &'static str,
    pub pkg_local_subpath: &'static str,
    pub include_base_path: String,
    pub exp_class: String,
    pub is_ground: bool,
    pub is_air: bool,
    pub is_heli: bool,
    pub is_ship: bool,
}

const FLAG_UT_TANK: u8 = 1 << 0;
const FLAG_UT_SPAA: u8 = 1 << 1;
const FLAG_UT_SHIP: u8 = 1 << 2;

#[derive(Debug, Clone, Default)]
struct CompactUnitMeta {
    unit_move_type: Option<Box<str>>,
    unit_class: Option<Box<str>>,
    ut_type: Option<Box<str>>,
    ut_hangar: Option<Box<str>>,
    flags: u8,
}

#[derive(Debug, Deserialize)]
struct WpcostEntryMin {
    #[serde(default, rename = "unitMoveType")]
    unit_move_type: Option<String>,
    #[serde(default, rename = "unitClass")]
    unit_class: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum WpcostTopVal {
    Entry(WpcostEntryMin),
    Other(serde::de::IgnoredAny),
}

#[derive(Debug, Default, Deserialize)]
struct UnittagsTagsMin {
    #[serde(default)]
    tank: bool,
    #[serde(default)]
    type_spaa: bool,
    #[serde(default)]
    ship: bool,
}

#[derive(Debug, Deserialize)]
struct UnittagsEntryMin {
    #[serde(default, rename = "type")]
    unit_type: Option<String>,
    #[serde(default)]
    hangar_place: Option<String>,
    #[serde(default)]
    tags: Option<UnittagsTagsMin>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum UnittagsTopVal {
    Entry(UnittagsEntryMin),
    Other(serde::de::IgnoredAny),
}

static CLASSIFIER_INSTANCE: OnceLock<VehicleClassifier> = OnceLock::new();

pub struct VehicleClassifier {
    unit_meta: RwLock<HashMap<String, CompactUnitMeta>>,
    classification_cache: RwLock<HashMap<String, VehicleClassification>>,
}

const GROUND_ID_TOKENS: &[&str] = &[
    "spaa",
    "flak",
    "gepard",
    "pantsir",
    "pantsyr",
    "tunguska",
    "bradley",
    "abrams",
    "leopard",
    "t_72",
    "t_80",
    "t_90",
    "t_64",
    "t_62",
    "t_55",
    "t_54",
    "t_34",
    "t_44",
    "panzer",
    "pzkpfw",
    "sdkfz",
    "tiger",
    "panther",
    "maus",
    "stug",
    "jagd",
    "marder",
    "sherman",
    "stuart",
    "pershing",
    "patton",
    "chaffee",
    "walker_bulldog",
    "hellcat",
    "adats",
    "fk_2000",
    "antelope",
    "type_93",
    "chaparral",
    "lav_ad",
    "tor_m",
    "strela",
    "zsu_",
    "shilka",
    "osa_",
    "buk_",
    "roland",
    "ito_90",
    "crotale",
    "otomatic",
    "challenger",
    "chieftain",
    "centurion",
    "cromwell",
    "churchill",
    "merkava",
    "leclerc",
    "ariete",
    "strv_",
    "cv90",
    "type_10",
    "type_90",
    "type_74",
    "ztz",
    "zbd",
    "bmp_",
    "btr_",
    "m1a1",
    "m1a2",
    "m2a2",
    "m2a4",
    "m3a1",
    "m3a3",
    "m4a1",
    "m4a2",
    "m4a3",
    "m18_",
    "m24_",
    "m26_",
    "m41_",
    "m46_",
    "m47_",
    "m48_",
    "m60",
    "m109",
    "m247",
    "us_m2",
    "us_m3",
    "us_m4",
    "us_m5",
    "us_m6",
    "us_m8",
    "us_m10",
    "us_m15",
    "us_m16",
    "us_m19",
    "us_m22",
    "us_m36",
    "us_m42",
    "us_m50",
    "us_m55",
    "us_m56",
    "us_m113",
    "us_t14",
    "us_t20",
    "us_t25",
    "us_t26",
    "us_t29",
    "us_t30",
    "us_t32",
    "us_t34",
    "us_t95",
    "us_hstvl",
    "us_rdf_lt",
    "us_xm",
    "germ_pz",
    "ussr_t_",
    "ussr_kv_",
    "ussr_is_",
    "ussr_su_",
    "ussr_isu_",
    "ussr_bt_",
    "uk_cruiser",
    "uk_a_",
    "jp_type_",
    "fr_amx",
    "fr_arl",
    "fr_b1",
    "fr_somua",
    "it_m13",
    "it_m14",
    "it_m15",
    "it_semovente",
    "cn_t_",
    "cn_pt_",
    "sw_strv",
    "sw_ikv",
    "sw_pbv",
    "il_magach",
    "il_merkava",
    "il_sho_t",
];

const NAVAL_ID_TOKENS: &[&str] = &[
    "_destroyer_",
    "_cruiser_",
    "_battleship_",
    "_battlecruiser_",
    "_frigate_",
    "_corvette_",
    "_gunboat_",
    "_torpedo_boat_",
    "_subchaser_",
    "_patrol_",
    "_minesweeper_",
    "_pt_boat",
    "_elco_",
    "_higgins_",
    "_vosper_",
    "_fairmile_",
    "_s_boot",
    "_r_boot",
    "_pr_",
    "destroyer_",
    "cruiser_",
    "battleship_",
    "battlecruiser_",
    "us_pt_",
    "germ_s_",
    "ussr_g5",
    "ussr_pr_",
    "uk_mtb",
    "uk_mgb",
    "jp_t14",
    "it_ms_",
];

const HELI_ID_PREFIXES: &[&str] = &[
    "ah_", "uh_", "mi_", "ka_", "mh_", "ch_", "bo_", "tiger_", "oh_", "ec_", "sa_", "h_34", "yah_",
    "z_10", "z_19", "z_9", "a129", "rooivalk", "lynx",
];

impl VehicleClassifier {
    pub fn get_instance() -> &'static Self {
        CLASSIFIER_INSTANCE.get_or_init(|| {
            let assets_dir = crate::config::get_assets_dir();
            let dm_dir = assets_dir.join("datamine");
            Self::new_for_datamine_dir(&dm_dir)
        })
    }

    pub fn new_for_datamine_dir(dm_dir: &Path) -> Self {
        let classifier = Self {
            unit_meta: RwLock::new(HashMap::new()),
            classification_cache: RwLock::new(HashMap::new()),
        };
        classifier.load_indexes_from_dir(dm_dir);
        classifier
    }

    /// Reloads datamine indexes and clears cached classifications.
    /// Must be called after onboarding downloads or datamine updates.
    pub fn reload_indexes(&self) {
        if let Ok(mut c) = self.classification_cache.write() {
            c.clear();
        }
        if let Ok(mut meta) = self.unit_meta.write() {
            meta.clear();
        }
        self.load_indexes();
    }

    pub fn load_indexes(&self) {
        let assets_dir = crate::config::get_assets_dir();
        let dm_dir = assets_dir.join("datamine");
        self.load_indexes_from_dir(&dm_dir);
    }

    pub fn load_indexes_from_dir(&self, dm_dir: &Path) {
        let mut combined: HashMap<String, CompactUnitMeta> = HashMap::with_capacity(4096);

        let wpcost_path = dm_dir.join("wpcost.blkx");
        if wpcost_path.exists() {
            if let Ok(f) = File::open(&wpcost_path) {
                let reader = BufReader::with_capacity(64 * 1024, f);
                if let Ok(map) = serde_json::from_reader::<_, HashMap<String, WpcostTopVal>>(reader)
                {
                    let mut wp_count = 0usize;
                    for (k, v) in map {
                        if let WpcostTopVal::Entry(e) = v {
                            if e.unit_move_type.is_some() || e.unit_class.is_some() {
                                let entry = combined.entry(k.to_lowercase()).or_default();
                                entry.unit_move_type = e.unit_move_type.map(|s| s.into_boxed_str());
                                entry.unit_class = e.unit_class.map(|s| s.into_boxed_str());
                                wp_count += 1;
                            }
                        }
                    }
                    info!(
                        "[CLASSIFIER] Loaded {} compact entries from wpcost.blkx",
                        wp_count
                    );
                }
            }
        }

        let unittags_path = dm_dir.join("unittags.blkx");
        if unittags_path.exists() {
            if let Ok(f) = File::open(&unittags_path) {
                let reader = BufReader::with_capacity(64 * 1024, f);
                if let Ok(map) =
                    serde_json::from_reader::<_, HashMap<String, UnittagsTopVal>>(reader)
                {
                    let mut ut_count = 0usize;
                    for (k, v) in map {
                        if let UnittagsTopVal::Entry(e) = v {
                            let entry = combined.entry(k.to_lowercase()).or_default();
                            if e.unit_type.is_some() {
                                entry.ut_type = e.unit_type.map(|s| s.into_boxed_str());
                            }
                            if e.hangar_place.is_some() {
                                entry.ut_hangar = e.hangar_place.map(|s| s.into_boxed_str());
                            }
                            if let Some(tags) = e.tags {
                                if tags.tank {
                                    entry.flags |= FLAG_UT_TANK;
                                }
                                if tags.type_spaa {
                                    entry.flags |= FLAG_UT_SPAA;
                                }
                                if tags.ship {
                                    entry.flags |= FLAG_UT_SHIP;
                                }
                            }
                            ut_count += 1;
                        }
                    }
                    info!(
                        "[CLASSIFIER] Loaded {} compact entries from unittags.blkx",
                        ut_count
                    );
                }
            }
        }

        if let Ok(mut meta_lock) = self.unit_meta.write() {
            *meta_lock = combined;
        }
    }

    pub fn classify(&self, vehicle_id: &str) -> VehicleClassification {
        let safe_id = Path::new(vehicle_id)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(vehicle_id)
            .trim_start_matches("custom_");

        let safe_low = safe_id.to_lowercase();

        if let Ok(cache) = self.classification_cache.read() {
            if let Some(c) = cache.get(&safe_low) {
                return c.clone();
            }
        }

        let meta_opt = self
            .unit_meta
            .read()
            .ok()
            .and_then(|g| g.get(&safe_low).cloned());
        let has_wp = meta_opt
            .as_ref()
            .map(|m| m.unit_move_type.is_some() || m.unit_class.is_some())
            .unwrap_or(false);

        let unit_move_type = meta_opt
            .as_ref()
            .and_then(|m| m.unit_move_type.as_deref())
            .unwrap_or("");

        let unit_class_str = meta_opt
            .as_ref()
            .and_then(|m| m.unit_class.as_deref())
            .unwrap_or("");

        let ut_type = meta_opt
            .as_ref()
            .and_then(|m| m.ut_type.as_deref())
            .unwrap_or("");

        let ut_hangar = meta_opt
            .as_ref()
            .and_then(|m| m.ut_hangar.as_deref())
            .unwrap_or("");

        let flags = meta_opt.as_ref().map(|m| m.flags).unwrap_or(0);
        let is_ut_tank = (flags & FLAG_UT_TANK) != 0;
        let is_ut_spaa = (flags & FLAG_UT_SPAA) != 0;
        let is_ut_ship = (flags & FLAG_UT_SHIP) != 0;

        // Naval check first when not explicitly marked tank
        let is_ship_by_name = NAVAL_ID_TOKENS.iter().any(|tok| safe_low.contains(tok))
            && !safe_low.contains("uk_cruiser_"); // British "Cruiser Tank" edge case (A13/Cromwell)
        let is_ship = unit_move_type == "ship"
            || unit_move_type == "slow_ship"
            || unit_move_type == "fast_ship"
            || unit_class_str.starts_with("exp_naval")
            || unit_class_str == "exp_destroyer"
            || unit_class_str == "exp_cruiser"
            || unit_class_str == "exp_torpedo_boat"
            || unit_class_str == "exp_gun_boat"
            || unit_class_str == "exp_submarine_chaser"
            || ut_type == "ship"
            || ut_hangar == "ship"
            || is_ut_ship
            || (!has_wp && is_ship_by_name);

        let is_name_ground = !is_ship && GROUND_ID_TOKENS.iter().any(|tok| safe_low.contains(tok));

        let is_ground = !is_ship
            && (unit_move_type == "tank"
                || unit_move_type == "heavy_tank"
                || unit_move_type == "wheeled_vehicle"
                || unit_class_str.starts_with("exp_tank")
                || unit_class_str == "exp_SPAA"
                || unit_class_str == "exp_heavy_tank"
                || unit_class_str == "exp_tank_destroyer"
                || ut_type == "tank"
                || ut_hangar == "tank"
                || is_ut_tank
                || is_ut_spaa
                || (!has_wp && is_name_ground));

        let is_heli = !is_ship
            && !is_ground
            && (unit_move_type == "helicopter"
                || unit_class_str == "exp_helicopter"
                || ut_type == "helicopter"
                || ut_hangar == "helicopter"
                || HELI_ID_PREFIXES
                    .iter()
                    .any(|p| safe_low.starts_with(p) || safe_low.contains(p)));

        let (branch, block_type, pkg_local_subpath, include_base_path) = if is_ship {
            (
                VehicleBranch::Naval,
                "ships",
                "units/ships",
                format!("#/develop/gameBase/gameData/units/ships/{}.blk", safe_id),
            )
        } else if is_ground {
            (
                VehicleBranch::Army,
                "tankModels",
                "units/tankModels",
                format!(
                    "#/develop/gameBase/gameData/units/tankModels/{}.blk",
                    safe_id
                ),
            )
        } else {
            (
                VehicleBranch::Aviation,
                "armada",
                "flightModels",
                format!("#/develop/gameBase/gameData/flightModels/{}.blk", safe_id),
            )
        };

        let exp_class = if !unit_class_str.is_empty() {
            unit_class_str.to_string()
        } else if is_ship {
            if safe_low.contains("battleship") || safe_low.contains("cruiser") {
                "exp_cruiser".to_string()
            } else if safe_low.contains("destroyer") {
                "exp_destroyer".to_string()
            } else if safe_low.contains("gun_boat") || safe_low.contains("gunboat") {
                "exp_gun_boat".to_string()
            } else {
                "exp_torpedo_boat".to_string()
            }
        } else if is_ground {
            if is_ut_spaa
                || safe_low.contains("spaa")
                || safe_low.contains("flak")
                || safe_low.contains("gepard")
                || safe_low.contains("pantsir")
                || safe_low.contains("pantsyr")
                || safe_low.contains("adats")
                || safe_low.contains("tunguska")
                || safe_low.contains("fk_2000")
                || safe_low.contains("antelope")
            {
                "exp_SPAA".to_string()
            } else if safe_low.contains("destroyer")
                || safe_low.contains("jagd")
                || safe_low.contains("object_268")
            {
                "exp_tank_destroyer".to_string()
            } else if safe_low.contains("heavy")
                || safe_low.contains("tiger_ii")
                || safe_low.contains("is_")
                || safe_low.contains("t_10")
                || safe_low.contains("kv_")
            {
                "exp_heavy_tank".to_string()
            } else {
                "exp_tank".to_string()
            }
        } else if is_heli {
            "exp_helicopter".to_string()
        } else if safe_low.contains("bomber")
            || safe_low.starts_with("b_")
            || safe_low.starts_with("tu_")
            || safe_low.starts_with("ju_")
            || safe_low.starts_with("pe_")
        {
            "exp_bomber".to_string()
        } else if safe_low.contains("su_25")
            || safe_low.contains("a_10")
            || safe_low.contains("a_7")
            || safe_low.contains("amx")
            || safe_low.contains("assault")
        {
            "exp_assault".to_string()
        } else {
            "exp_fighter".to_string()
        };

        let is_air = branch == VehicleBranch::Aviation;

        let classification = VehicleClassification {
            id: safe_id.to_string(),
            branch,
            block_type,
            pkg_local_subpath,
            include_base_path,
            exp_class,
            is_ground,
            is_air,
            is_heli,
            is_ship,
        };

        debug!(
            "[CLASSIFIER] '{}' -> branch={:?}, block={}, subpath={}",
            safe_id,
            classification.branch,
            classification.block_type,
            classification.pkg_local_subpath
        );

        if let Ok(mut cache) = self.classification_cache.write() {
            cache.insert(safe_low, classification.clone());
        }

        classification
    }
}

pub fn classify_vehicle(vehicle_id: &str) -> VehicleClassification {
    VehicleClassifier::get_instance().classify(vehicle_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spaa_classification() {
        let classifier = VehicleClassifier::get_instance();
        let adats = classifier.classify("us_adats_bradley");
        assert_eq!(adats.branch, VehicleBranch::Army);
        assert_eq!(adats.block_type, "tankModels");
        assert_eq!(adats.pkg_local_subpath, "units/tankModels");
        assert_eq!(adats.exp_class, "exp_SPAA");
        assert!(adats.is_ground);
        assert!(!adats.is_air);

        let fk2000 = classifier.classify("cn_fk_2000");
        assert_eq!(fk2000.branch, VehicleBranch::Army);
        assert!(fk2000.is_ground);

        let gepard = classifier.classify("germ_flakpz_1a2_gepard");
        assert_eq!(gepard.branch, VehicleBranch::Army);
        assert_eq!(gepard.exp_class, "exp_SPAA");

        let pantsir = classifier.classify("ussr_pantsyr_s1");
        assert_eq!(pantsir.branch, VehicleBranch::Army);
        assert_eq!(pantsir.exp_class, "exp_SPAA");

        let custom_spaa = classifier.classify("custom_my_test_spaa");
        assert_eq!(custom_spaa.branch, VehicleBranch::Army);
        assert!(custom_spaa.is_ground);
    }

    #[test]
    fn test_tank_and_wwii_classification() {
        let classifier = VehicleClassifier::get_instance();

        let m2a4 = classifier.classify("us_m2a4");
        assert_eq!(m2a4.branch, VehicleBranch::Army);
        assert_eq!(m2a4.block_type, "tankModels");
        assert!(m2a4.is_ground);

        let m1a2 = classifier.classify("us_m1a2_sep3_abrams");
        assert_eq!(m1a2.branch, VehicleBranch::Army);
        assert!(m1a2.is_ground);

        let leo = classifier.classify("germ_leopard_2a7v");
        assert_eq!(leo.branch, VehicleBranch::Army);
        assert!(leo.is_ground);
    }

    #[test]
    fn test_aircraft_and_naval_classification() {
        let classifier = VehicleClassifier::get_instance();

        let f18 = classifier.classify("fa_18e_block_2");
        assert_eq!(f18.branch, VehicleBranch::Aviation);
        assert_eq!(f18.block_type, "armada");
        assert!(f18.is_air);

        let jas39 = classifier.classify("saab_jas39e");
        assert_eq!(jas39.branch, VehicleBranch::Aviation);
        assert!(jas39.is_air);

        let apache = classifier.classify("ah_64e");
        assert_eq!(apache.branch, VehicleBranch::Aviation);
        assert_eq!(apache.exp_class, "exp_helicopter");
        assert!(apache.is_heli);

        let fletcher = classifier.classify("us_destroyer_fletcher");
        assert_eq!(fletcher.branch, VehicleBranch::Naval);
        assert_eq!(fletcher.block_type, "ships");
        assert_eq!(fletcher.pkg_local_subpath, "units/ships");
        assert!(fletcher.is_ship);
    }
}
