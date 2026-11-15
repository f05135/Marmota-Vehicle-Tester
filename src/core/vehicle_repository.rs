use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::BufReader;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

use crate::core::database::CatalogDatabase;
use crate::core::vehicle_classifier::VehicleBranch;
use crate::core::vehicle_parser::{
    build_aircraft_detail, build_ship_detail, build_tank_detail, clean_vehicle_name,
    is_radar_sensor_entry,
};
use crate::core::weapon_catalog::{
    load_modifications_index, load_weaponry_csv_with_db, modifications_index_ready,
    parse_semicolon_csv_row, resolve_additive_bullet_mod, CsvLangColumns,
};

pub const TECH_TREE_SCHEMA_VERSION: &str = "7";

#[derive(Debug, Default)]
struct PresentFlag;

impl<'de> serde::Deserialize<'de> for PresentFlag {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        serde::de::IgnoredAny::deserialize(deserializer)?;
        Ok(PresentFlag)
    }
}

#[derive(Debug, Default)]
struct ModKeyList(Vec<String>);

impl<'de> serde::Deserialize<'de> for ModKeyList {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = ModKeyList;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("map of modification keys")
            }
            fn visit_map<M: serde::de::MapAccess<'de>>(
                self,
                mut map: M,
            ) -> Result<ModKeyList, M::Error> {
                let mut keys = Vec::new();
                while let Some(k) = map.next_key::<String>()? {
                    let _: serde::de::IgnoredAny = map.next_value()?;
                    keys.push(k);
                }
                Ok(ModKeyList(keys))
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> Result<ModKeyList, A::Error> {
                while seq.next_element::<serde::de::IgnoredAny>()?.is_some() {}
                Ok(ModKeyList(Vec::new()))
            }
            fn visit_unit<E: serde::de::Error>(self) -> Result<ModKeyList, E> {
                Ok(ModKeyList(Vec::new()))
            }
            fn visit_none<E: serde::de::Error>(self) -> Result<ModKeyList, E> {
                Ok(ModKeyList(Vec::new()))
            }
            fn visit_some<D2: serde::Deserializer<'de>>(
                self,
                d: D2,
            ) -> Result<ModKeyList, D2::Error> {
                serde::Deserialize::deserialize(d)
            }
        }
        deserializer.deserialize_any(V)
    }
}

#[derive(Debug, Deserialize)]
struct WpcostRawEntry {
    #[serde(default)]
    rank: Option<i64>,
    #[serde(default, rename = "economicRankHistorical")]
    economic_rank_historical: Option<i64>,
    #[serde(default, rename = "economicRankArcade")]
    economic_rank_arcade: Option<i64>,
    #[serde(default, rename = "economicRankSimulation")]
    economic_rank_simulation: Option<i64>,
    #[serde(default, rename = "economicRank")]
    economic_rank: Option<i64>,
    #[serde(default, rename = "unitMoveType")]
    unit_move_type: Option<String>,
    #[serde(default, rename = "unitClass")]
    unit_class: Option<String>,
    #[serde(default)]
    gift: Option<PresentFlag>,
    #[serde(default, rename = "costGold")]
    cost_gold: Option<f64>,
    #[serde(default, rename = "clanAir")]
    clan_air: Option<PresentFlag>,
    #[serde(default, rename = "isClanVehicle")]
    is_clan_vehicle: Option<bool>,
    #[serde(default, rename = "researchType")]
    research_type: Option<String>,
    #[serde(default, rename = "maxSpeed")]
    max_speed: Option<f64>,
    #[serde(default)]
    modifications: Option<ModKeyList>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum WpcostRawTopVal {
    Entry(Box<WpcostRawEntry>),
    Other(serde::de::IgnoredAny),
}

#[derive(Debug, Clone)]
struct CompactWpcostMeta {
    rank: Option<i64>,
    econ_rank: Option<i64>,
    unit_move_type: Option<String>,
    unit_class: Option<String>,
    has_gift: bool,
    cost_gold: i64,
    clan_air: bool,
    is_clan_vehicle: bool,
    research_type: Option<String>,
    max_speed: Option<f64>,
    mod_keys: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct UnittagsShopMin {
    #[serde(default, rename = "maxSpeed")]
    max_speed: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct UnittagsEntryRepoMin {
    #[serde(default, rename = "Shop")]
    shop: Option<UnittagsShopMin>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum UnittagsRepoTopVal {
    Entry(UnittagsEntryRepoMin),
    Other(serde::de::IgnoredAny),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TechTreePayload {
    pub version: u32,
    pub total: usize,
    #[serde(rename = "aviationCount")]
    pub aviation_count: usize,
    #[serde(rename = "armyCount")]
    pub army_count: usize,
    #[serde(rename = "shipsCount")]
    pub ships_count: usize,
    pub nations: Vec<String>,
    pub vehicles: Vec<serde_json::Value>,
}

pub struct VehicleRepository {
    pub config: std::sync::RwLock<crate::config::AppConfig>,
    pub cached_tree: std::sync::RwLock<Option<TechTreePayload>>,
    pub cached_map: std::sync::RwLock<HashMap<String, serde_json::Value>>,
    rebuild_lock: std::sync::Mutex<()>,
    #[cfg(test)]
    test_assets_dir: Option<PathBuf>,
    #[cfg(test)]
    test_db: Option<std::sync::Arc<CatalogDatabase>>,
}

impl VehicleRepository {
    pub fn new(config: crate::config::AppConfig) -> Self {
        let cache_dir = crate::config::get_assets_dir();
        let _ = fs::create_dir_all(&cache_dir);

        Self {
            config: std::sync::RwLock::new(config),
            cached_tree: std::sync::RwLock::new(None),
            cached_map: std::sync::RwLock::new(HashMap::new()),
            rebuild_lock: std::sync::Mutex::new(()),
            #[cfg(test)]
            test_assets_dir: None,
            #[cfg(test)]
            test_db: None,
        }
    }

    #[cfg(test)]
    pub fn new_isolated(
        config: crate::config::AppConfig,
        assets_dir: PathBuf,
        db: std::sync::Arc<CatalogDatabase>,
    ) -> Self {
        let _ = fs::create_dir_all(&assets_dir);
        Self {
            config: std::sync::RwLock::new(config),
            cached_tree: std::sync::RwLock::new(None),
            cached_map: std::sync::RwLock::new(HashMap::new()),
            rebuild_lock: std::sync::Mutex::new(()),
            test_assets_dir: Some(assets_dir),
            test_db: Some(db),
        }
    }

    fn assets_dir(&self) -> PathBuf {
        #[cfg(test)]
        if let Some(ref p) = self.test_assets_dir {
            return p.clone();
        }
        crate::config::get_assets_dir()
    }

    fn db(&self) -> &CatalogDatabase {
        #[cfg(test)]
        if let Some(ref d) = self.test_db {
            return d.as_ref();
        }
        CatalogDatabase::get_instance()
    }

    pub fn get_config(&self) -> crate::config::AppConfig {
        self.config.read().unwrap().clone()
    }

    pub fn update_config(&self, new_config: crate::config::AppConfig) {
        if let Ok(mut g) = self.config.write() {
            *g = new_config;
        }
        let cache_dir = self.assets_dir();
        let _ = fs::create_dir_all(&cache_dir);
    }

    pub fn get_datamine_dir(&self) -> PathBuf {
        self.assets_dir().join("datamine")
    }

    pub fn current_datamine_version(&self) -> String {
        let ver_file = self.get_datamine_dir().join("version.json");
        if ver_file.exists() {
            if let Ok(txt) = fs::read_to_string(&ver_file) {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&txt) {
                    if let Some(s) = v.get("version").and_then(|x| x.as_str()) {
                        let s = s.trim();
                        if !s.is_empty() {
                            return s.to_string();
                        }
                    }
                }
                let trimmed = txt.trim();
                if !trimmed.is_empty() && trimmed.len() < 64 && !trimmed.starts_with('{') {
                    return trimmed.to_string();
                }
            }
        }
        "local".to_string()
    }

    fn resolve_datamine_source(&self, dm_file: &str, cfg_fallback: &str) -> PathBuf {
        let dm = self.get_datamine_dir().join(dm_file);
        if dm.exists() {
            dm
        } else {
            PathBuf::from(cfg_fallback)
        }
    }

    fn compute_datamine_fingerprint(&self) -> String {
        let cfg = self.get_config();
        let dm_dir = self.get_datamine_dir();
        let paths = [
            self.resolve_datamine_source("shop.blkx", &cfg.shop_blkx_path),
            self.resolve_datamine_source("wpcost.blkx", &cfg.wpcost_blkx_path),
            dm_dir.join("unittags.blkx"),
            dm_dir.join("modifications.blkx"),
            self.resolve_datamine_source("units.csv", &cfg.units_csv_path),
            self.resolve_datamine_source("units_weaponry.csv", &cfg.weaponry_csv_path),
        ];

        let mut parts = Vec::with_capacity(paths.len());
        for p in paths {
            if let Ok(meta) = fs::metadata(&p) {
                let mtime = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                parts.push(format!("{}:{}", meta.len(), mtime));
            } else {
                parts.push("0:0".to_string());
            }
        }
        parts.join("|")
    }

    pub fn get_full_tree(&self, force_refresh: bool) -> TechTreePayload {
        if !force_refresh {
            if let Ok(guard) = self.cached_tree.read() {
                if let Some(tree) = guard.as_ref() {
                    if !tree.vehicles.is_empty() {
                        return tree.clone();
                    }
                }
            }
        }

        let _build_guard = self.rebuild_lock.lock().ok();
        if !force_refresh {
            if let Ok(guard) = self.cached_tree.read() {
                if let Some(tree) = guard.as_ref() {
                    if !tree.vehicles.is_empty() {
                        return tree.clone();
                    }
                }
            }

            // Fast-path: load from SQLite WAL database only when schema, datamine version, and file fingerprint all match
            let db = self.db();
            let db_ver = db.get_meta("datamine_version").unwrap_or_default();
            let schema_ver = db.get_meta("schema_version").unwrap_or_default();
            let db_fp = db.get_meta("datamine_fingerprint").unwrap_or_default();
            let cur_ver = self.current_datamine_version();
            let cur_fp = self.compute_datamine_fingerprint();
            if schema_ver == TECH_TREE_SCHEMA_VERSION
                && db_ver == cur_ver
                && db_fp == cur_fp
                && !cur_fp.is_empty()
            {
                let vehicles = db.load_all_vehicles();
                if vehicles.len() > 200 {
                    let dm_dir = self.get_datamine_dir();
                    let mods_path = dm_dir.join("modifications.blkx");
                    if mods_path.exists() {
                        load_modifications_index(&mods_path);
                    }
                    let cfg = self.get_config();
                    let wcsv =
                        self.resolve_datamine_source("units_weaponry.csv", &cfg.weaponry_csv_path);
                    if wcsv.exists() {
                        load_weaponry_csv_with_db(&wcsv, Some(db));
                    }
                    let payload = self.build_payload_from_vehicles(vehicles);
                    self.populate_in_mem_cache(&payload);
                    info!(
                        "[DB.SQLITE] Loaded {} vehicles from WAL cache",
                        payload.total
                    );
                    return payload;
                }
            }
        }

        info!("Rebuilding unified Tech Tree (Aviation + Army + Naval) into SQLite WAL...");
        let db = self.db();
        let dm_dir = self.get_datamine_dir();
        let mods_path = dm_dir.join("modifications.blkx");
        if mods_path.exists() {
            load_modifications_index(&mods_path);
        }
        let cfg = self.get_config();
        let wcsv = self.resolve_datamine_source("units_weaponry.csv", &cfg.weaponry_csv_path);
        if wcsv.exists() {
            load_weaponry_csv_with_db(&wcsv, Some(db));
        }

        let tree = self.build_tree();
        if !tree.vehicles.is_empty() {
            let cur_ver = self.current_datamine_version();
            let cur_fp = self.compute_datamine_fingerprint();
            if let Err(e) = db.replace_vehicles(&tree.vehicles, &cur_ver) {
                warn!("[DB.SQLITE] Failed persisting vehicles: {}", e);
            } else {
                db.set_meta("schema_version", TECH_TREE_SCHEMA_VERSION);
                db.set_meta("datamine_fingerprint", &cur_fp);
                db.invalidate_vehicle_details_cache();
            }
        }

        self.populate_in_mem_cache(&tree);
        tree
    }

    fn populate_in_mem_cache(&self, tree: &TechTreePayload) {
        let mut map = HashMap::new();
        for v in &tree.vehicles {
            if let Some(id) = v.get("id").and_then(|i| i.as_str()) {
                map.insert(id.to_string(), v.clone());
                map.insert(id.to_lowercase(), v.clone());
            }
        }
        if let Ok(mut g_tree) = self.cached_tree.write() {
            *g_tree = Some(tree.clone());
        }
        if let Ok(mut g_map) = self.cached_map.write() {
            *g_map = map;
        }
    }

    fn build_payload_from_vehicles(&self, vehicles: Vec<serde_json::Value>) -> TechTreePayload {
        let aviation_count = vehicles
            .iter()
            .filter(|v| v.get("branch").and_then(|b| b.as_str()) == Some("aviation"))
            .count();
        let army_count = vehicles
            .iter()
            .filter(|v| v.get("branch").and_then(|b| b.as_str()) == Some("army"))
            .count();
        let ships_count = vehicles
            .iter()
            .filter(|v| v.get("branch").and_then(|b| b.as_str()) == Some("ships"))
            .count();

        TechTreePayload {
            version: 6,
            total: vehicles.len(),
            aviation_count,
            army_count,
            ships_count,
            nations: default_nations(),
            vehicles,
        }
    }

    fn load_wpcost_compact(&self, path: &Path) -> HashMap<String, CompactWpcostMeta> {
        let mut out = HashMap::new();
        let Ok(file) = File::open(path) else {
            return out;
        };
        let reader = BufReader::with_capacity(128 * 1024, file);
        let Ok(raw_map) = serde_json::from_reader::<_, HashMap<String, WpcostRawTopVal>>(reader)
        else {
            return out;
        };

        out.reserve(raw_map.len());
        for (k, v) in raw_map {
            if let WpcostRawTopVal::Entry(e) = v {
                let econ_rank = e
                    .economic_rank_historical
                    .or(e.economic_rank_arcade)
                    .or(e.economic_rank_simulation)
                    .or(e.economic_rank);
                out.insert(
                    k.to_lowercase(),
                    CompactWpcostMeta {
                        rank: e.rank,
                        econ_rank,
                        unit_move_type: e.unit_move_type,
                        unit_class: e.unit_class,
                        has_gift: e.gift.is_some(),
                        cost_gold: e.cost_gold.map(|c| c.round() as i64).unwrap_or(0),
                        clan_air: e.clan_air.is_some(),
                        is_clan_vehicle: e.is_clan_vehicle.unwrap_or(false),
                        research_type: e.research_type,
                        max_speed: e.max_speed,
                        mod_keys: e.modifications.map(|m| m.0).unwrap_or_default(),
                    },
                );
            }
        }
        out
    }

    fn load_unittags_speeds(&self, path: &Path) -> HashMap<String, f64> {
        let mut out = HashMap::new();
        let Ok(file) = File::open(path) else {
            return out;
        };
        let reader = BufReader::with_capacity(64 * 1024, file);
        let Ok(raw_map) = serde_json::from_reader::<_, HashMap<String, UnittagsRepoTopVal>>(reader)
        else {
            return out;
        };

        for (k, v) in raw_map {
            if let UnittagsRepoTopVal::Entry(e) = v {
                if let Some(spd) = e.shop.and_then(|s| s.max_speed) {
                    if spd > 0.0 {
                        out.insert(k.to_lowercase(), spd);
                    }
                }
            }
        }
        out
    }

    fn build_tree(&self) -> TechTreePayload {
        let cfg = self.get_config();
        let dm_dir = self.get_datamine_dir();

        let shop_path = self.resolve_datamine_source("shop.blkx", &cfg.shop_blkx_path);
        let wpcost_path = self.resolve_datamine_source("wpcost.blkx", &cfg.wpcost_blkx_path);
        let unittags_path = dm_dir.join("unittags.blkx");

        let shop_data = self.read_json_file(&shop_path);
        let wpcost_ci = self.load_wpcost_compact(&wpcost_path);
        let unittags_speeds = self.load_unittags_speeds(&unittags_path);
        let units_loc = self.load_units_localization();

        let mut vehicles = Vec::new();
        let mut seen_ids = HashSet::new();

        let nation_shop_keys = [
            ("USA", "country_usa"),
            ("GERMANY", "country_germany"),
            ("USSR", "country_ussr"),
            ("UK", "country_britain"),
            ("JAPAN", "country_japan"),
            ("CHINA", "country_china"),
            ("ITALY", "country_italy"),
            ("FRANCE", "country_france"),
            ("SWEDEN", "country_sweden"),
            ("ISRAEL", "country_israel"),
        ];

        for (nat_name, shop_key) in &nation_shop_keys {
            if let Some(c_block) = shop_data.get(shop_key) {
                // 1. Aviation & Helicopters
                for branch_key in &["aviation", "helicopters"] {
                    if let Some(branch_obj) = c_block.get(branch_key).and_then(|b| b.as_object()) {
                        self.walk_branch_columns(
                            branch_obj,
                            nat_name,
                            "aviation",
                            branch_key,
                            &wpcost_ci,
                            &unittags_speeds,
                            &units_loc,
                            &mut vehicles,
                            &mut seen_ids,
                        );
                    }
                }
                // 2. Army
                if let Some(army_obj) = c_block.get("army").and_then(|a| a.as_object()) {
                    self.walk_branch_columns(
                        army_obj,
                        nat_name,
                        "army",
                        "army",
                        &wpcost_ci,
                        &unittags_speeds,
                        &units_loc,
                        &mut vehicles,
                        &mut seen_ids,
                    );
                }
                // 3. Naval (Bluewater `ships` + Coastal `boats`)
                for branch_key in &["ships", "boats"] {
                    if let Some(ship_obj) = c_block.get(branch_key).and_then(|s| s.as_object()) {
                        self.walk_branch_columns(
                            ship_obj,
                            nat_name,
                            "ships",
                            branch_key,
                            &wpcost_ci,
                            &unittags_speeds,
                            &units_loc,
                            &mut vehicles,
                            &mut seen_ids,
                        );
                    }
                }
            }
        }

        self.build_payload_from_vehicles(vehicles)
    }

    /// Correctly walks `range: [ col0, col1, col2, ... ]` preserving exact column index `col_idx`!
    #[allow(clippy::too_many_arguments)]
    fn walk_branch_columns(
        &self,
        branch_obj: &serde_json::Map<String, serde_json::Value>,
        nation: &str,
        branch: &str,
        sub_branch: &str,
        wpcost_ci: &HashMap<String, CompactWpcostMeta>,
        unittags_speeds: &HashMap<String, f64>,
        units_loc: &HashMap<String, HashMap<String, String>>,
        vehicles: &mut Vec<serde_json::Value>,
        seen_ids: &mut HashSet<String>,
    ) {
        let mut fallback_col = 0usize;
        for (k, val) in branch_obj {
            if k == "range" {
                if let Some(arr) = val.as_array() {
                    for (arr_col_idx, col_node) in arr.iter().enumerate() {
                        self.walk_and_register(
                            col_node,
                            "range_col",
                            nation,
                            branch,
                            sub_branch,
                            arr_col_idx,
                            None,
                            wpcost_ci,
                            unittags_speeds,
                            units_loc,
                            vehicles,
                            seen_ids,
                        );
                    }
                } else if val.is_object() {
                    self.walk_and_register(
                        val,
                        k,
                        nation,
                        branch,
                        sub_branch,
                        fallback_col,
                        None,
                        wpcost_ci,
                        unittags_speeds,
                        units_loc,
                        vehicles,
                        seen_ids,
                    );
                    fallback_col += 1;
                }
            } else if k.starts_with("range")
                || k.starts_with("line")
                || k.ends_with("_group")
                || val.is_object()
            {
                self.walk_and_register(
                    val,
                    k,
                    nation,
                    branch,
                    sub_branch,
                    fallback_col,
                    None,
                    wpcost_ci,
                    unittags_speeds,
                    units_loc,
                    vehicles,
                    seen_ids,
                );
                fallback_col += 1;
            } else if let Some(arr) = val.as_array() {
                for (arr_col_idx, col_node) in arr.iter().enumerate() {
                    self.walk_and_register(
                        col_node,
                        k,
                        nation,
                        branch,
                        sub_branch,
                        arr_col_idx,
                        None,
                        wpcost_ci,
                        unittags_speeds,
                        units_loc,
                        vehicles,
                        seen_ids,
                    );
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn walk_and_register(
        &self,
        node: &serde_json::Value,
        node_name: &str,
        nation: &str,
        branch: &str,
        sub_branch: &str,
        col_idx: usize,
        folder_group: Option<&str>,
        wpcost_ci: &HashMap<String, CompactWpcostMeta>,
        unittags_speeds: &HashMap<String, f64>,
        units_loc: &HashMap<String, HashMap<String, String>>,
        vehicles: &mut Vec<serde_json::Value>,
        seen_ids: &mut HashSet<String>,
    ) {
        if let Some(obj) = node.as_object() {
            if obj.contains_key("rank") || obj.contains_key("unit_class") {
                self.register_unit(
                    node_name,
                    node,
                    nation,
                    branch,
                    sub_branch,
                    col_idx,
                    folder_group,
                    wpcost_ci,
                    unittags_speeds,
                    units_loc,
                    vehicles,
                    seen_ids,
                    None,
                );
                return;
            }
            let mut last_id = None;
            for (k, v) in obj {
                if matches!(
                    k.as_str(),
                    "rank"
                        | "image"
                        | "reqAir"
                        | "gift"
                        | "event"
                        | "hidden"
                        | "showOnlyWhenBought"
                ) {
                    continue;
                }
                if k.ends_with("_group") {
                    let group_label = units_loc
                        .get(&k.to_lowercase())
                        .and_then(|m| m.get("en-US"))
                        .cloned()
                        .unwrap_or_else(|| k.trim_end_matches("_group").replace('_', " "));
                    self.walk_and_register(
                        v,
                        k,
                        nation,
                        branch,
                        sub_branch,
                        col_idx,
                        Some(&group_label),
                        wpcost_ci,
                        unittags_speeds,
                        units_loc,
                        vehicles,
                        seen_ids,
                    );
                    continue;
                }
                if v.is_object() {
                    let reg_id = self.register_unit(
                        k,
                        v,
                        nation,
                        branch,
                        sub_branch,
                        col_idx,
                        folder_group,
                        wpcost_ci,
                        unittags_speeds,
                        units_loc,
                        vehicles,
                        seen_ids,
                        last_id.as_deref(),
                    );
                    if reg_id.is_some() {
                        last_id = reg_id;
                    }
                }
            }
        } else if let Some(arr) = node.as_array() {
            for item in arr {
                self.walk_and_register(
                    item,
                    node_name,
                    nation,
                    branch,
                    sub_branch,
                    col_idx,
                    folder_group,
                    wpcost_ci,
                    unittags_speeds,
                    units_loc,
                    vehicles,
                    seen_ids,
                );
            }
        }
    }

    fn inspect_local_radar_hint(
        &self,
        dm_dir: &Path,
        branch: &str,
        vid_low: &str,
    ) -> Option<String> {
        let sub = match branch {
            "ships" => "ships",
            "army" => "tankmodels",
            _ => "flightmodels",
        };
        let path = dm_dir.join(sub).join(format!("{}.blkx", vid_low));
        if !path.exists() {
            return None;
        }
        let Ok(txt) = fs::read_to_string(&path) else {
            return None;
        };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&txt) else {
            return None;
        };
        let has_sensor_radar = v
            .get("sensors")
            .and_then(|s| s.get("sensor"))
            .map(|sensors| {
                let arr = if let Some(a) = sensors.as_array() {
                    a.as_slice()
                } else {
                    std::slice::from_ref(sensors)
                };
                arr.iter().any(|item| is_radar_sensor_entry(dm_dir, item))
            })
            .unwrap_or(false);
        let r_deploy = v
            .get("radarDeployTime")
            .and_then(|r| r.as_f64())
            .unwrap_or(0.0);
        if has_sensor_radar || r_deploy > 0.0 {
            if branch == "army" {
                Some("radar_360".to_string())
            } else {
                Some("Radar".to_string())
            }
        } else {
            None
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn register_unit(
        &self,
        unit_id: &str,
        shop_entry: &serde_json::Value,
        nation: &str,
        branch: &str,
        sub_branch: &str,
        col_idx: usize,
        folder_group: Option<&str>,
        wpcost_ci: &HashMap<String, CompactWpcostMeta>,
        unittags_speeds: &HashMap<String, f64>,
        units_loc: &HashMap<String, HashMap<String, String>>,
        vehicles: &mut Vec<serde_json::Value>,
        seen_ids: &mut HashSet<String>,
        fallback_prev: Option<&str>,
    ) -> Option<String> {
        let vid = unit_id.trim();
        let vid_low = vid.to_lowercase();
        if vid.is_empty() || seen_ids.contains(&vid_low) {
            return None;
        }

        let excluded_suffixes = [
            "_football",
            "_tutorial",
            "_race",
            "_test",
            "_space",
            "_killstreak",
            "_snowball",
        ];
        if excluded_suffixes.iter().any(|s| vid_low.ends_with(s))
            || vid_low.starts_with("nt_")
            || vid_low == "uav_quadcopter"
            || vid_low.starts_with("ucav_recon_micro")
        {
            return None;
        }

        let wc = wpcost_ci.get(&vid_low);
        let rank_val = shop_entry
            .get("rank")
            .and_then(|r| r.as_i64())
            .or_else(|| wc.and_then(|w| w.rank))
            .unwrap_or(1);
        let rank_int = rank_val.clamp(1, 9);

        let econ_rank = wc.and_then(|w| w.econ_rank).or_else(|| {
            shop_entry
                .get("economicRankHistorical")
                .or_else(|| shop_entry.get("economicRankArcade"))
                .or_else(|| shop_entry.get("economicRankSimulation"))
                .or_else(|| shop_entry.get("economicRank"))
                .and_then(|e| e.as_i64())
        });

        let br_str = match econ_rank {
            Some(r) => format!("{:.1}", (r as f64 / 3.0) + 1.0),
            None => "1.0".to_string(),
        };

        let unit_move_type = wc.and_then(|w| w.unit_move_type.as_deref()).unwrap_or("");
        let unit_class = shop_entry
            .get("unit_class")
            .and_then(|c| c.as_str())
            .or_else(|| wc.and_then(|w| w.unit_class.as_deref()))
            .unwrap_or("");
        let uc_low = unit_class.to_lowercase();

        let cat = if branch == "ships" {
            if sub_branch == "boats"
                || uc_low == "exp_torpedo_boat"
                || uc_low == "exp_gun_boat"
                || uc_low == "exp_submarine_chaser"
                || uc_low == "exp_naval_ferry_barge"
            {
                "COASTAL"
            } else {
                "BLUEWATER"
            }
        } else if branch == "aviation" {
            if sub_branch == "helicopters"
                || unit_move_type == "helicopter"
                || uc_low == "exp_helicopter"
                || vid_low.contains("helicopter")
                || vid_low.starts_with("ah_")
                || vid_low.starts_with("uh_")
                || vid_low.starts_with("mi_")
                || vid_low.starts_with("ka_")
                || vid_low.starts_with("mh_")
                || vid_low.starts_with("ch_")
                || vid_low.starts_with("bo_")
                || vid_low.contains("lynx")
                || vid_low.starts_with("tiger_")
                || vid_low.contains("a129")
            {
                "HELICOPTERS"
            } else if uc_low == "exp_bomber"
                || vid_low.starts_with("b_")
                || vid_low.starts_with("tu_")
                || vid_low.starts_with("ju_")
            {
                "BOMBERS"
            } else if uc_low == "exp_assault"
                || vid_low.contains("su_25")
                || vid_low.contains("a_10")
                || vid_low.contains("a_7")
            {
                "ATTACK"
            } else {
                "FIGHTERS"
            }
        } else if uc_low == "exp_spaa"
            || vid_low.contains("_spaa")
            || vid_low.contains("flak")
            || vid_low.contains("gepard")
            || vid_low.contains("tunguska")
            || vid_low.contains("pantsir")
            || vid_low.contains("pantsyr")
        {
            "SPAA"
        } else if uc_low == "exp_tank_destroyer"
            || vid_low.contains("destroyer")
            || vid_low.contains("jagd")
            || vid_low.contains("object_268")
        {
            "TD"
        } else if uc_low == "exp_heavy_tank"
            || vid_low.contains("heavy")
            || vid_low.contains("tiger_ii")
            || vid_low.contains("is_")
            || vid_low.contains("kv_")
        {
            "HEAVY"
        } else {
            "TANKS"
        };

        let normalized_sub_branch = if branch == "ships" {
            if cat == "COASTAL" {
                "coastal"
            } else {
                "bluewater"
            }
        } else if branch == "aviation" {
            if cat == "HELICOPTERS" {
                "helicopters"
            } else {
                "aviation"
            }
        } else {
            "army"
        };

        let is_squadron = shop_entry
            .get("isClanVehicle")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
            || shop_entry.get("reqFeature").and_then(|v| v.as_str()) == Some("ClanVehicles")
            || shop_entry.get("clanAir").is_some()
            || wc
                .map(|w| {
                    w.is_clan_vehicle
                        || w.clan_air
                        || w.research_type.as_deref() == Some("clanVehicle")
                })
                .unwrap_or(false);

        let is_gift = shop_entry.get("gift").is_some()
            || shop_entry.get("marketplaceItemdefId").is_some()
            || shop_entry
                .get("costGold")
                .and_then(|v| v.as_i64())
                .unwrap_or(0)
                > 0
            || wc.map(|w| w.has_gift || w.cost_gold > 0).unwrap_or(false);

        let (tier_type, effective_col) = if is_squadron {
            ("squadron", 5)
        } else if is_gift || col_idx >= 5 {
            ("premium", 5)
        } else {
            ("regular", col_idx.min(4))
        };

        let req_air = shop_entry.get("reqAir").and_then(|r| r.as_str());
        let prev_id = if let Some(r) = req_air {
            if seen_ids.contains(&r.to_lowercase()) {
                Some(r)
            } else {
                fallback_prev
            }
        } else {
            fallback_prev
        };

        let loc_names = units_loc.get(&vid_low).cloned().unwrap_or_default();
        let raw_name = loc_names
            .get("en-US")
            .cloned()
            .unwrap_or_else(|| vid.to_string());
        let display_name = clean_vehicle_name(&raw_name);

        let effective_unit_class = if !unit_class.is_empty() {
            unit_class
        } else if branch == "ships" {
            if cat == "COASTAL" {
                "exp_torpedo_boat"
            } else {
                "exp_destroyer"
            }
        } else if branch == "aviation" {
            match cat {
                "HELICOPTERS" => "exp_helicopter",
                "BOMBERS" => "exp_bomber",
                "ATTACK" => "exp_assault",
                _ => "exp_fighter",
            }
        } else {
            match cat {
                "SPAA" => "exp_SPAA",
                "TD" => "exp_tank_destroyer",
                "HEAVY" => "exp_heavy_tank",
                _ => "exp_tank",
            }
        };

        let empty_mods: &[String] = &[];
        let mod_keys: &[String] = wc.map(|w| w.mod_keys.as_slice()).unwrap_or(empty_mods);
        let mods_ready = modifications_index_ready();

        let ammo_mods: Vec<String> = if branch == "army" || branch == "ships" {
            mod_keys
                .iter()
                .filter(|k| {
                    if mods_ready {
                        resolve_additive_bullet_mod(k).is_some()
                    } else {
                        let kl = k.to_lowercase();
                        !kl.contains("tracks")
                            && !kl.contains("brakes")
                            && !kl.contains("engine")
                            && !kl.contains("transmission")
                            && !kl.contains("suspension")
                            && !kl.contains("filter")
                            && !kl.contains("aiming")
                            && !kl.contains("extinguisher")
                            && !kl.contains("tool_kit")
                            && !kl.contains("optics")
                            && !kl.contains("medical")
                            && !kl.contains("airstrike")
                            && !kl.contains("art_support")
                            && !kl.contains("night_vision")
                            && !kl.contains("premexpmul")
                            && !kl.contains("new_wheels")
                            && !kl.contains("smoke_screen")
                            && !kl.contains("laser")
                            && !kl.contains("tank_new_gun")
                            && !kl.contains("abrams_protection")
                            && !kl.contains("mine_protection")
                            && !kl.ends_with("_ammo_pack")
                    }
                })
                .cloned()
                .collect()
        } else {
            Vec::new()
        };

        let has_nv = mod_keys.iter().any(|k| {
            let kl = k.to_lowercase();
            kl.contains("night_vision") || kl.contains("thermal")
        });
        let has_lrf = mod_keys
            .iter()
            .any(|k| k.to_lowercase().contains("laser_rangefinder"));
        let has_chaff = mod_keys.iter().any(|k| {
            let kl = k.to_lowercase();
            kl.contains("flare") || kl.contains("chaff")
        });
        let has_mod_radar = mod_keys.iter().any(|k| k.to_lowercase().contains("radar"));

        let raw_speed_ms = unittags_speeds
            .get(&vid_low)
            .copied()
            .or_else(|| wc.and_then(|w| w.max_speed));
        let max_speed_kmh = raw_speed_ms.map(|ms| (ms * 3.6).round());

        let dm_dir = self.get_datamine_dir();
        let local_radar = self.inspect_local_radar_hint(&dm_dir, branch, &vid_low);

        let mut telemetry = serde_json::Map::new();
        telemetry.insert("subBranch".to_string(), json!(normalized_sub_branch));
        if branch == "army" {
            if has_nv {
                telemetry.insert("thermal".to_string(), json!("nvd"));
            }
            if has_lrf {
                telemetry.insert("lrf".to_string(), json!(true));
            }
            if let Some(r) = local_radar.or_else(|| has_mod_radar.then(|| "radar_360".to_string()))
            {
                telemetry.insert("radar".to_string(), json!(r));
            }
            if let Some(first_ammo) = ammo_mods.first() {
                telemetry.insert("gun".to_string(), json!(first_ammo.replace('_', " ")));
            }
        } else if branch == "aviation" {
            if has_chaff {
                telemetry.insert("cmTag".to_string(), json!("Flares/Chaff"));
            }
            if has_nv {
                telemetry.insert("thermal".to_string(), json!("nvd"));
            }
            if let Some(r) = local_radar.or_else(|| has_mod_radar.then(|| "Radar".to_string())) {
                telemetry.insert("radar".to_string(), json!(r));
            }
        } else if branch == "ships" {
            if let Some(r) = local_radar.or_else(|| has_mod_radar.then(|| "radar_360".to_string()))
            {
                telemetry.insert("radar".to_string(), json!(r));
            }
        }
        if let Some(spd) = max_speed_kmh {
            if spd > 0.0 {
                telemetry.insert("maxSpeedKmh".to_string(), json!(spd));
            }
        }

        let mut card = json!({
            "id": vid,
            "name": display_name,
            "names": loc_names,
            "rank": rank_int,
            "br": br_str,
            "economicRank": econ_rank,
            "nation": nation,
            "category": cat,
            "cat": cat,
            "branch": branch,
            "subBranch": normalized_sub_branch,
            "tierType": tier_type,
            "col": effective_col,
            "prev": prev_id,
            "folderGroup": folder_group,
            "unitMoveType": unit_move_type,
            "unitClass": effective_unit_class,
            "ammoMods": ammo_mods,
            "telemetry": telemetry,
        });

        if let Some(tel_map) = card.get("telemetry").and_then(|t| t.as_object()).cloned() {
            if let Some(obj) = card.as_object_mut() {
                for (tk, tv) in tel_map {
                    obj.entry(tk).or_insert(tv);
                }
            }
        }

        seen_ids.insert(vid_low);
        vehicles.push(card);
        Some(vid.to_string())
    }

    fn read_json_file(&self, path: &Path) -> serde_json::Value {
        if path.exists() {
            if let Ok(c) = fs::read_to_string(path) {
                if let Ok(v) = serde_json::from_str(&c) {
                    return v;
                }
            }
        }
        serde_json::Value::Object(serde_json::Map::new())
    }

    fn load_units_localization(&self) -> HashMap<String, HashMap<String, String>> {
        let mut map: HashMap<String, HashMap<String, String>> = HashMap::new();
        let cfg = self.get_config();
        let dm_dir = self.get_datamine_dir();
        let path = if dm_dir.join("units.csv").exists() {
            dm_dir.join("units.csv")
        } else {
            PathBuf::from(&cfg.units_csv_path)
        };
        if path.exists() {
            if let Ok(content) = fs::read_to_string(path) {
                let mut lang_cols = CsvLangColumns::default();
                for (idx, line) in content.lines().enumerate() {
                    if line.trim().is_empty() {
                        continue;
                    }
                    let cols = parse_semicolon_csv_row(line);
                    if idx == 0 {
                        lang_cols = CsvLangColumns::from_header(&cols);
                        continue;
                    }
                    if cols.len() < 2 {
                        continue;
                    }
                    let clean_k = cols[0].trim().to_lowercase();
                    let en = clean_vehicle_name(
                        cols.get(lang_cols.en).map(|s| s.as_str()).unwrap_or(""),
                    );
                    if clean_k.is_empty() || en.is_empty() {
                        continue;
                    }
                    let pt = clean_vehicle_name(lang_cols.col_or(&cols, lang_cols.pt, &en));
                    let es = clean_vehicle_name(lang_cols.col_or(&cols, lang_cols.es, &en));
                    let ru = clean_vehicle_name(lang_cols.col_or(&cols, lang_cols.ru, &en));
                    let de = clean_vehicle_name(lang_cols.col_or(&cols, lang_cols.de, &en));
                    let fr = clean_vehicle_name(lang_cols.col_or(&cols, lang_cols.fr, &en));
                    let zh = clean_vehicle_name(lang_cols.col_or(&cols, lang_cols.zh, &en));
                    let ja = clean_vehicle_name(lang_cols.col_or(&cols, lang_cols.ja, &en));

                    let mut langs = HashMap::new();
                    langs.insert("en-US".to_string(), en);
                    langs.insert("pt-BR".to_string(), pt);
                    langs.insert("es".to_string(), es);
                    langs.insert("ru".to_string(), ru);
                    langs.insert("de".to_string(), de);
                    langs.insert("fr".to_string(), fr);
                    langs.insert("zh".to_string(), zh);
                    langs.insert("ja".to_string(), ja);

                    if let Some(stripped) = clean_k.strip_suffix("_shop") {
                        map.entry(stripped.to_string())
                            .or_insert_with(|| langs.clone());
                    } else if let Some(stripped) = clean_k.strip_suffix("_0") {
                        map.entry(stripped.to_string())
                            .or_insert_with(|| langs.clone());
                    }
                    map.insert(clean_k, langs);
                }
            }
        }
        map
    }

    pub fn get_vehicle_summary(&self, vehicle_id: &str) -> Option<serde_json::Value> {
        let vid_low = vehicle_id.to_lowercase();
        if let Ok(guard) = self.cached_map.read() {
            if let Some(v) = guard.get(&vid_low) {
                return Some(v.clone());
            }
        }
        let _ = self.get_full_tree(false);
        if let Ok(guard) = self.cached_map.read() {
            if let Some(v) = guard.get(&vid_low) {
                return Some(v.clone());
            }
        }
        None
    }

    pub fn get_vehicle_detail(&self, vehicle_id: &str) -> Option<serde_json::Value> {
        let cur_ver = self.current_datamine_version();
        let db = self.db();
        let card = self.get_vehicle_summary(vehicle_id);
        let dm_dir = self.get_datamine_dir();
        let classifier = crate::core::vehicle_classifier::VehicleClassifier::get_instance();
        let classification = classifier.classify(vehicle_id);

        let is_ship = classification.branch == VehicleBranch::Naval
            || card
                .as_ref()
                .and_then(|c| c.get("branch"))
                .map(|b| b == "ships")
                .unwrap_or(false);
        let is_army = !is_ship
            && (classification.is_ground
                || card
                    .as_ref()
                    .and_then(|c| c.get("branch"))
                    .map(|b| b == "army")
                    .unwrap_or(false));

        let cfg = self.get_config();

        // Return cached SQLite detail only if schema matches and the model blkx file wasn't newly downloaded after caching
        if let Some(cached) = db.get_cached_vehicle_detail(vehicle_id, &cur_ver) {
            let schema_ok = cached.get("schemaVersion").and_then(|s| s.as_str())
                == Some(TECH_TREE_SCHEMA_VERSION);
            if schema_ok {
                let model_exists_now = if is_ship {
                    dm_dir
                        .join("ships")
                        .join(format!("{}.blkx", vehicle_id.to_lowercase()))
                        .exists()
                } else if is_army {
                    dm_dir
                        .join("tankmodels")
                        .join(format!("{}.blkx", vehicle_id.to_lowercase()))
                        .exists()
                } else {
                    dm_dir
                        .join("flightmodels")
                        .join(format!("{}.blkx", vehicle_id.to_lowercase()))
                        .exists()
                };
                let has_presets = cached
                    .get("presets")
                    .and_then(|p| p.as_array())
                    .map(|a| a.len())
                    .unwrap_or(0)
                    > 1;
                if !model_exists_now || has_presets || is_army || is_ship {
                    return Some(cached);
                }
            }
        }

        let mut detail = if is_ship {
            build_ship_detail(vehicle_id, card.as_ref(), &cfg, &dm_dir)
        } else if is_army {
            build_tank_detail(vehicle_id, card.as_ref(), &cfg, &dm_dir)
        } else {
            build_aircraft_detail(vehicle_id, card.as_ref(), &cfg, &dm_dir)
        };

        if let Some(obj) = detail.as_object_mut() {
            obj.insert("schemaVersion".to_string(), json!(TECH_TREE_SCHEMA_VERSION));
        }

        db.save_cached_vehicle_detail(vehicle_id, &detail, &cur_ver);
        Some(detail)
    }

    pub fn invalidate_vehicle_detail(&self, vehicle_id: &str) {
        self.db().invalidate_vehicle_detail(vehicle_id);
    }
}

fn default_nations() -> Vec<String> {
    vec![
        "USA".to_string(),
        "GERMANY".to_string(),
        "USSR".to_string(),
        "UK".to_string(),
        "JAPAN".to_string(),
        "CHINA".to_string(),
        "ITALY".to_string(),
        "FRANCE".to_string(),
        "SWEDEN".to_string(),
        "ISRAEL".to_string(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn copy_if_exists(src: &Path, dst: &Path) {
        if src.exists() {
            if let Some(parent) = dst.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let _ = fs::copy(src, dst);
        }
    }

    fn setup_fixture_repo(
        tag: &str,
        mut custom_version: Option<&str>,
    ) -> (VehicleRepository, PathBuf) {
        let base = std::env::temp_dir().join(format!(
            "wt_repo_test_{}_{}_{:?}",
            tag,
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&base);
        let assets = base.join("assets");
        let dm = assets.join("datamine");
        let _ = fs::create_dir_all(&dm);

        let ws_assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");
        let ws_dm = ws_assets.join("datamine");

        if ws_dm.join("shop.blkx").exists() {
            for f in [
                "shop.blkx",
                "wpcost.blkx",
                "unittags.blkx",
                "modifications.blkx",
                "units.csv",
                "units_weaponry.csv",
            ] {
                copy_if_exists(&ws_dm.join(f), &dm.join(f));
            }
            for dir_sub in [
                "tankmodels/us_m4a1_1942_sherman.blkx",
                "tankmodels/us_mim_72_chaparral.blkx",
                "tankmodels/us_adats_bradley.blkx",
                "tankmodels/us_m1a2_sep3_abrams.blkx",
                "tankmodels/us_m2a4.blkx",
                "flightmodels/fa_18e_block_2.blkx",
                "flightmodels/a_10a_early.blkx",
                "flightmodels/a_4e_early.blkx",
                "flightmodels/a2d.blkx",
                "flightmodels/ah_64e.blkx",
                "tankmodels/germ_iris_slm_fcs.blkx",
                "ships/us_destroyer_fletcher.blkx",
            ] {
                copy_if_exists(&ws_dm.join(dir_sub), &dm.join(dir_sub));
            }
            // Copy ground/naval/aircraft weapons referenced if present
            for wdir in [
                "weapons/groundmodels_weapons",
                "weapons/navalmodels_weapons",
                "weapons",
            ] {
                let src_w = ws_dm.join(wdir);
                if src_w.is_dir() {
                    let dst_w = dm.join(wdir);
                    let _ = fs::create_dir_all(&dst_w);
                    if let Ok(entries) = fs::read_dir(&src_w) {
                        for ent in entries.flatten() {
                            if ent.path().is_file() {
                                let _ = fs::copy(ent.path(), dst_w.join(ent.file_name()));
                            }
                        }
                    }
                }
            }
        } else {
            // Minimal self-contained fixture if workspace lacks datamine files
            let _ = fs::write(
                dm.join("shop.blkx"),
                r#"{
                    "country_usa": {
                        "aviation": {
                            "range": [
                                { "a_10a_early": { "rank": 6, "gift": "msi_notebook" } },
                                { "a_4e_early": { "rank": 6, "isClanVehicle": true, "reqFeature": "ClanVehicles" } },
                                { "nt_b_52h": { "rank": 7, "showOnlyWhenBought": true } }
                            ]
                        },
                        "helicopters": {
                            "range": [{ "ah_64e": { "rank": 7 } }]
                        },
                        "army": {
                            "range": [{
                                "us_m4a1_1942_sherman": { "rank": 2 },
                                "us_mim_72_chaparral": { "rank": 6 },
                                "us_adats_bradley": { "rank": 8 },
                                "us_m1a2_sep3_abrams": { "rank": 8 },
                                "uav_quadcopter": { "rank": 7 }
                            }]
                        },
                        "ships": {
                            "range": [{ "us_destroyer_fletcher": { "rank": 2 } }]
                        }
                    },
                    "country_israel": {
                        "aviation": {
                            "range": [{ "spitfire_ix_cw_iaf": { "rank": 3 } }]
                        }
                    }
                }"#,
            );
            let _ = fs::write(
                dm.join("wpcost.blkx"),
                r#"{
                    "a_10a_early": { "rank": 6, "economicRankHistorical": 28, "unitClass": "exp_assault", "gift": "msi", "costGold": 9090, "modifications": { "countermeasures_launcher_chaff": {} } },
                    "a_4e_early": { "rank": 6, "economicRankHistorical": 23, "unitClass": "exp_assault", "researchType": "clanVehicle" },
                    "ah_64e": { "rank": 7, "economicRankHistorical": 32, "unitClass": "exp_helicopter", "unitMoveType": "helicopter", "modifications": { "heli_night_vision_system": {} } },
                    "us_m4a1_1942_sherman": { "rank": 2, "economicRankHistorical": 8, "unitClass": "exp_tank" },
                    "us_mim_72_chaparral": { "rank": 6, "economicRankHistorical": 25, "unitClass": "exp_SPAA" },
                    "us_adats_bradley": { "rank": 8, "economicRankHistorical": 32, "unitClass": "exp_SPAA", "modifications": { "modern_tank_laser_rangefinder": {}, "night_vision_system": {} } },
                    "us_m1a2_sep3_abrams": { "rank": 8, "economicRankHistorical": 35, "unitClass": "exp_tank" },
                    "us_destroyer_fletcher": { "rank": 2, "economicRankHistorical": 11, "unitClass": "exp_destroyer" },
                    "spitfire_ix_cw_iaf": { "rank": 3, "economicRankHistorical": 14, "unitClass": "exp_fighter" }
                }"#,
            );
            let _ = fs::write(
                dm.join("unittags.blkx"),
                r#"{
                    "ah_64e": { "Shop": { "maxSpeed": 82.0 } },
                    "us_destroyer_fletcher": { "Shop": { "maxSpeed": 18.0 } },
                    "a_10a_early": { "Shop": { "maxSpeed": 187.74 } }
                }"#,
            );
            let _ = fs::write(dm.join("modifications.blkx"), r#"{"modifications":{}}"#);
            let _ = fs::write(
                dm.join("units.csv"),
                "ID;English\nspitfire_ix_cw_iaf_shop;Weizman's \u{f059}Spitfire LF Mk.IXe\na_4e_early_shop;A-4E Early\n",
            );
            let _ = fs::write(dm.join("units_weaponry.csv"), "ID;English\n");
        }

        if let Some(ver) = custom_version.take() {
            let _ = fs::write(
                dm.join("version.json"),
                format!(r#"{{"version":"{}"}}"#, ver),
            );
        } else if !dm.join("version.json").exists() {
            let _ = fs::write(dm.join("version.json"), r#"{"version":"local"}"#);
        }

        let mut cfg = crate::config::detect_paths(None);
        cfg.shop_blkx_path = dm.join("shop.blkx").to_string_lossy().to_string();
        cfg.wpcost_blkx_path = dm.join("wpcost.blkx").to_string_lossy().to_string();
        cfg.units_csv_path = dm.join("units.csv").to_string_lossy().to_string();
        cfg.weaponry_csv_path = dm.join("units_weaponry.csv").to_string_lossy().to_string();

        let db = Arc::new(
            CatalogDatabase::open(&assets.join("marmota_catalog.db")).expect("isolated db"),
        );
        (VehicleRepository::new_isolated(cfg, assets, db), base)
    }

    #[test]
    fn test_clean_vehicle_name_and_tree_filters_isolated() {
        assert_eq!(clean_vehicle_name("o P-36C"), "P-36C");
        assert_eq!(clean_vehicle_name("◄F-4F KWS LV"), "F-4F KWS LV");
        assert_eq!(clean_vehicle_name("◊MiG-29 (9-12A)"), "MiG-29 (9-12A)");
        assert_eq!(clean_vehicle_name("␡Tornado IDS MFG"), "Tornado IDS MFG");
        assert_eq!(
            clean_vehicle_name("Weizman's \u{f059}Spitfire LF Mk.IXe"),
            "Weizman's Spitfire LF Mk.IXe"
        );

        let (repo, tmp) = setup_fixture_repo("tree_accuracy", Some("local"));
        let tree = repo.get_full_tree(true);
        assert!(!tree.vehicles.is_empty());

        let find = |id: &str| {
            tree.vehicles
                .iter()
                .find(|v| v.get("id").and_then(|x| x.as_str()) == Some(id))
        };

        assert!(
            find("nt_b_52h").is_none(),
            "nt_b_52h must be excluded from tree"
        );
        assert!(
            find("uav_quadcopter").is_none(),
            "uav_quadcopter must be excluded from tree"
        );
        assert!(
            find("h_34_killstreak").is_none(),
            "killstreak units must be excluded"
        );

        let a4e = find("a_4e_early").expect("a_4e_early should exist");
        assert_eq!(
            a4e.get("tierType").and_then(|x| x.as_str()),
            Some("squadron")
        );
        assert_eq!(a4e.get("col").and_then(|x| x.as_i64()), Some(5));

        let a10 = find("a_10a_early").expect("a_10a_early should exist");
        assert_eq!(
            a10.get("tierType").and_then(|x| x.as_str()),
            Some("premium")
        );
        assert!(
            a10.get("radar").is_none(),
            "A-10A Early must not have fake radar in tree: {:?}",
            a10.get("radar")
        );

        if let Some(a2d) = find("a2d") {
            if tmp.join("assets/datamine/flightmodels/a2d.blkx").exists() {
                assert_eq!(
                    a2d.get("radar").and_then(|x| x.as_str()),
                    Some("Radar"),
                    "A2D has AN/APS-19 radar"
                );
            }
        }
        if let Some(chap) = find("us_mim_72_chaparral") {
            assert!(
                chap.get("radar").is_none(),
                "Chaparral has IRST optical tracker, not radar: {:?}",
                chap.get("radar")
            );
        }

        let fletcher = find("us_destroyer_fletcher").expect("Fletcher should exist");
        assert_eq!(
            fletcher.get("subBranch").and_then(|x| x.as_str()),
            Some("bluewater")
        );
        assert_eq!(
            fletcher.get("maxSpeedKmh").and_then(|x| x.as_f64()),
            Some(65.0)
        );

        let heli = find("ah_64e").expect("ah_64e should exist");
        assert_eq!(
            heli.get("subBranch").and_then(|x| x.as_str()),
            Some("helicopters")
        );
        assert_eq!(heli.get("thermal").and_then(|x| x.as_str()), Some("nvd"));

        if let Some(spit) = find("spitfire_ix_cw_iaf") {
            let name = spit.get("name").and_then(|x| x.as_str()).unwrap_or("");
            assert!(
                !name.chars().any(|c| ('\u{E000}'..='\u{F8FF}').contains(&c)),
                "PUA glyph remained in {}",
                name
            );
        }

        // Verify local cache invalidation triggers when shop.blkx changes even with version == "local"
        let dm_shop = tmp.join("assets/datamine/shop.blkx");
        if let Ok(mut content) = fs::read_to_string(&dm_shop) {
            content.push(' ');
            let _ = fs::write(&dm_shop, content);
        }
        let fp_after = repo.compute_datamine_fingerprint();
        let db_fp = repo
            .db()
            .get_meta("datamine_fingerprint")
            .unwrap_or_default();
        assert_ne!(
            fp_after, db_fp,
            "Changed datamine file must change fingerprint even for version=local"
        );

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_vehicle_details_accuracy_isolated() {
        let (repo, tmp) = setup_fixture_repo("detail_accuracy", Some("test_v5"));
        let _ = repo.get_full_tree(true);

        if tmp
            .join("assets/datamine/flightmodels/fa_18e_block_2.blkx")
            .exists()
        {
            let d = repo
                .get_vehicle_detail("fa_18e_block_2")
                .expect("fa_18e detail");
            let presets = d
                .get("presets")
                .and_then(|p| p.as_array())
                .expect("presets array");
            assert!(!presets.is_empty());
        }

        if tmp.join("assets/datamine/tankmodels/us_m2a4.blkx").exists() {
            let m2a4 = repo.get_vehicle_detail("us_m2a4").expect("M2A4 detail");
            assert_eq!(m2a4.get("branch").and_then(|b| b.as_str()), Some("army"));
            assert_eq!(
                m2a4.get("radar_code").and_then(|r| r.as_str()),
                Some("none")
            );
            assert_eq!(
                m2a4.get("thermal_code").and_then(|r| r.as_str()),
                Some("day")
            );
            assert_eq!(
                m2a4.get("smoke_code").and_then(|r| r.as_str()),
                Some("none")
            );
        }

        if tmp
            .join("assets/datamine/flightmodels/a_4e_early.blkx")
            .exists()
        {
            let a4e_det = repo.get_vehicle_detail("a_4e_early").expect("A-4E detail");
            let gun_txt = a4e_det.get("gun").and_then(|g| g.as_str()).unwrap_or("");
            assert!(
                gun_txt.contains("Mk12") || gun_txt.contains("Colt"),
                "A-4E gun must not be JATO booster: {}",
                gun_txt
            );
            assert_eq!(
                a4e_det.get("gunAmmoMax").and_then(|x| x.as_i64()),
                Some(200)
            );
            assert_eq!(
                a4e_det.get("radar").and_then(|x| x.as_str()),
                Some("No Radar")
            );
        }

        if tmp
            .join("assets/datamine/tankmodels/us_m4a1_1942_sherman.blkx")
            .exists()
        {
            let sherman = repo
                .get_vehicle_detail("us_m4a1_1942_sherman")
                .expect("Sherman detail");
            let belts = sherman
                .get("belts")
                .and_then(|b| b.as_array())
                .expect("belts");
            assert!(
                !belts.iter().any(|b| b
                    .get("id")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .contains("T45")),
                "Sherman M4A1 (1942) must not inject T45: {:?}",
                belts
            );
            let spd = sherman
                .get("maxSpeedKmh")
                .and_then(|x| x.as_f64())
                .unwrap_or(0.0);
            assert!(
                (37.0..=41.0).contains(&spd),
                "Sherman gear top speed must be ~39 km/h, got {}",
                spd
            );
            assert_eq!(sherman.get("reload").and_then(|x| x.as_str()), Some("5.0s"));
        }

        if tmp
            .join("assets/datamine/tankmodels/us_mim_72_chaparral.blkx")
            .exists()
        {
            let chap = repo
                .get_vehicle_detail("us_mim_72_chaparral")
                .expect("Chaparral detail");
            let belts = chap.get("belts").and_then(|b| b.as_array()).expect("belts");
            assert!(
                !belts.iter().any(|b| b
                    .get("id")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_lowercase()
                    .contains("mim72g")),
                "MIM-72 Chaparral must not include MIM-72G: {:?}",
                belts
            );
            assert_eq!(
                chap.get("radar_code").and_then(|r| r.as_str()),
                Some("none")
            );
            assert_eq!(chap.get("reload").and_then(|x| x.as_str()), Some("15.0s"));
        }

        if tmp
            .join("assets/datamine/tankmodels/us_adats_bradley.blkx")
            .exists()
        {
            let adats = repo
                .get_vehicle_detail("us_adats_bradley")
                .expect("ADATS detail");
            let belts = adats
                .get("belts")
                .and_then(|b| b.as_array())
                .expect("belts");
            assert!(
                !belts.iter().any(|b| b
                    .get("id")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_lowercase()
                    .contains("m919")),
                "ADATS must not include M919: {:?}",
                belts
            );
            assert!(
                belts.iter().any(|b| b
                    .get("id")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_lowercase()
                    .contains("mim146")),
                "ADATS must include MIM-146 missile: {:?}",
                belts
            );
            assert_eq!(
                adats.get("radar_code").and_then(|r| r.as_str()),
                Some("radar360")
            );
        }

        if tmp
            .join("assets/datamine/tankmodels/us_m1a2_sep3_abrams.blkx")
            .exists()
        {
            let sep3 = repo
                .get_vehicle_detail("us_m1a2_sep3_abrams")
                .expect("SEPv3 detail");
            let belts = sep3.get("belts").and_then(|b| b.as_array()).expect("belts");
            let m829a2_count = belts
                .iter()
                .filter(|b| {
                    let id_low = b
                        .get("id")
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                        .to_lowercase();
                    let name_low = b
                        .get("name")
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                        .to_lowercase();
                    let bname_low = b
                        .get("ballistics")
                        .and_then(|bl| bl.get("bulletName"))
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                        .to_lowercase();
                    id_low.contains("m829a2")
                        || name_low.contains("m829a2")
                        || bname_low.contains("m829a2")
                })
                .count();
            assert_eq!(
                m829a2_count, 1,
                "M829A2 must appear exactly once on SEPv3: {:?}",
                belts
            );
            assert!(
                belts
                    .iter()
                    .any(|b| b.get("name").and_then(|x| x.as_str()).unwrap_or("")
                        == "M830 (Default)"),
                "SEPv3 stock round must be M830 (Default): {:?}",
                belts
            );
            assert_eq!(
                sep3.get("thermal_code").and_then(|r| r.as_str()),
                Some("gen2")
            );
            assert_eq!(sep3.get("massTons").and_then(|r| r.as_f64()), Some(66.8));
            assert_eq!(sep3.get("maxSpeedKmh").and_then(|r| r.as_f64()), Some(68.0));
        }

        if tmp
            .join("assets/datamine/tankmodels/germ_iris_slm_fcs.blkx")
            .exists()
        {
            let iris = repo
                .get_vehicle_detail("germ_iris_slm_fcs")
                .expect("IRIS FCS detail");
            assert_eq!(iris.get("is_fcs").and_then(|x| x.as_bool()), Some(true));
            assert_eq!(iris.get("ammoCount").and_then(|x| x.as_i64()), Some(0));
            assert!(iris
                .get("belts")
                .and_then(|b| b.as_array())
                .map(|a| a.is_empty())
                .unwrap_or(false));
        }

        if tmp
            .join("assets/datamine/flightmodels/ah_64e.blkx")
            .exists()
        {
            let ah64e = repo.get_vehicle_detail("ah_64e").expect("AH-64E detail");
            assert_ne!(
                ah64e.get("thermal_code").and_then(|r| r.as_str()),
                Some("day")
            );
            let presets = ah64e
                .get("presets")
                .and_then(|p| p.as_array())
                .expect("presets");
            let stinger = presets
                .iter()
                .find(|p| p.get("id").and_then(|x| x.as_str()) == Some("ah_64e_stinger"))
                .expect("stinger preset");
            assert!(stinger
                .get("name")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .contains("Stinger"));
        }

        if tmp
            .join("assets/datamine/ships/us_destroyer_fletcher.blkx")
            .exists()
        {
            let fletcher = repo
                .get_vehicle_detail("us_destroyer_fletcher")
                .expect("Fletcher detail");
            assert_eq!(
                fletcher.get("maxSpeedKmh").and_then(|x| x.as_f64()),
                Some(64.8)
            );
            assert_eq!(
                fletcher.get("crewCount").and_then(|x| x.as_i64()),
                Some(273)
            );
            let belts = fletcher
                .get("belts")
                .and_then(|b| b.as_array())
                .expect("ship belts");
            assert!(!belts.iter().any(|b| b
                .get("id")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .contains("40mm_bofors")));
            let presets = fletcher
                .get("presets")
                .and_then(|p| p.as_array())
                .expect("ship presets");
            assert!(presets
                .iter()
                .any(|p| p.get("id").and_then(|x| x.as_str()) == Some("")));
        }

        let _ = fs::remove_dir_all(&tmp);
    }
}
