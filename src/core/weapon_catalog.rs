//! Data-driven weapon classification, localization (`units_weaponry.csv`),
//! and real War Thunder ballistic / missile telemetry parser.

use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::BufReader;
use std::path::Path;
use std::sync::{OnceLock, RwLock};
use tracing::info;

use crate::core::database::CatalogDatabase;

#[derive(Clone, Debug)]
pub struct WeaponPresetOption {
    pub icon_type: Option<String>,
    pub blk: Option<String>,
    pub trigger: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct ModBulletMeta {
    pub additive_bullet_mod: Option<String>,
    pub is_reserve: bool,
}

#[derive(Debug, Deserialize)]
struct ModEffectsMin {
    #[serde(default, rename = "additiveBulletMod")]
    additive_bullet_mod: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ModEntryMin {
    #[serde(default, rename = "isReserve")]
    is_reserve: bool,
    #[serde(default)]
    effects: Option<ModEffectsMin>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ModTopVal {
    Entry(ModEntryMin),
    Other(serde::de::IgnoredAny),
}

#[derive(Debug, Clone)]
pub struct WeaponRule {
    pub patterns: &'static [&'static str],
    pub short_name: &'static str,
    pub code: &'static str,
    pub kind: &'static str,
    pub color: &'static str,
    pub default_icon: &'static str,
    pub is_a2a: bool,
    pub is_a2g: bool,
}

/// Ordered from most specific pattern to generic family fallback.
const WEAPON_RULES: &[WeaponRule] = &[
    // Soviet / Russian A2G & A2A
    WeaponRule {
        patterns: &["kh_38ml", "kh38ml"],
        short_name: "Kh-38ML",
        code: "AGM",
        kind: "agm",
        color: "#b36b00",
        default_icon: "agm_missile.png",
        is_a2a: false,
        is_a2g: true,
    },
    WeaponRule {
        patterns: &["kh_38mt", "kh38mt"],
        short_name: "Kh-38MT",
        code: "AGM",
        kind: "agm",
        color: "#b36b00",
        default_icon: "agm_missile.png",
        is_a2a: false,
        is_a2g: true,
    },
    WeaponRule {
        patterns: &["kh_31pd", "kh31pd"],
        short_name: "Kh-31PD",
        code: "ARM",
        kind: "agm",
        color: "#b36b00",
        default_icon: "arm_kh31_type_c.png",
        is_a2a: false,
        is_a2g: true,
    },
    WeaponRule {
        patterns: &["kh_31ad", "kh31ad"],
        short_name: "Kh-31AD",
        code: "AShM",
        kind: "agm",
        color: "#b36b00",
        default_icon: "missile_air_to_earth_kh31.png",
        is_a2a: false,
        is_a2g: true,
    },
    WeaponRule {
        patterns: &["kh_31p", "kh31p"],
        short_name: "Kh-31P",
        code: "ARM",
        kind: "agm",
        color: "#b36b00",
        default_icon: "arm_kh31_type_c.png",
        is_a2a: false,
        is_a2g: true,
    },
    WeaponRule {
        patterns: &["kh_31a", "kh31a"],
        short_name: "Kh-31A",
        code: "AShM",
        kind: "agm",
        color: "#b36b00",
        default_icon: "missile_air_to_earth_kh31.png",
        is_a2a: false,
        is_a2g: true,
    },
    WeaponRule {
        patterns: &["kh_31", "kh31"],
        short_name: "Kh-31",
        code: "AGM",
        kind: "agm",
        color: "#b36b00",
        default_icon: "missile_air_to_earth_kh31.png",
        is_a2a: false,
        is_a2g: true,
    },
    WeaponRule {
        patterns: &["kh_29te", "kh29te"],
        short_name: "Kh-29TE",
        code: "AGM",
        kind: "agm",
        color: "#b36b00",
        default_icon: "guided_bomb_red.png",
        is_a2a: false,
        is_a2g: true,
    },
    WeaponRule {
        patterns: &["kh_29td", "kh29td"],
        short_name: "Kh-29TD",
        code: "AGM",
        kind: "agm",
        color: "#b36b00",
        default_icon: "guided_bomb_red.png",
        is_a2a: false,
        is_a2g: true,
    },
    WeaponRule {
        patterns: &["kh_29t", "kh29t"],
        short_name: "Kh-29T",
        code: "AGM",
        kind: "agm",
        color: "#b36b00",
        default_icon: "guided_bomb_red.png",
        is_a2a: false,
        is_a2g: true,
    },
    WeaponRule {
        patterns: &["kh_29l", "kh29l"],
        short_name: "Kh-29L",
        code: "AGM",
        kind: "agm",
        color: "#b36b00",
        default_icon: "guided_bomb_red.png",
        is_a2a: false,
        is_a2g: true,
    },
    WeaponRule {
        patterns: &["kh_25ml", "kh25ml"],
        short_name: "Kh-25ML",
        code: "AGM",
        kind: "agm",
        color: "#b36b00",
        default_icon: "agm_missile.png",
        is_a2a: false,
        is_a2g: true,
    },
    WeaponRule {
        patterns: &["kh_25", "kh25"],
        short_name: "Kh-25",
        code: "AGM",
        kind: "agm",
        color: "#b36b00",
        default_icon: "agm_missile.png",
        is_a2a: false,
        is_a2g: true,
    },
    WeaponRule {
        patterns: &["kh_23", "kh23"],
        short_name: "Kh-23M",
        code: "AGM",
        kind: "agm",
        color: "#b36b00",
        default_icon: "agm_missile.png",
        is_a2a: false,
        is_a2g: true,
    },
    WeaponRule {
        patterns: &["r_77_1", "r77_1"],
        short_name: "R-77-1",
        code: "ARH",
        kind: "aam_arh",
        color: "#1d6fa5",
        default_icon: "air_to_air_midrange_missile.png",
        is_a2a: true,
        is_a2g: false,
    },
    WeaponRule {
        patterns: &["r_77", "r77"],
        short_name: "R-77",
        code: "ARH",
        kind: "aam_arh",
        color: "#1d6fa5",
        default_icon: "air_to_air_midrange_missile.png",
        is_a2a: true,
        is_a2g: false,
    },
    WeaponRule {
        patterns: &["r_73", "r73"],
        short_name: "R-73",
        code: "IR",
        kind: "aam_ir",
        color: "#d9442c",
        default_icon: "air_to_air_missile.png",
        is_a2a: true,
        is_a2g: false,
    },
    WeaponRule {
        patterns: &["r_27er", "r27er"],
        short_name: "R-27ER",
        code: "SARH",
        kind: "aam_sarh",
        color: "#2b7a78",
        default_icon: "air_to_air_midrange_missile.png",
        is_a2a: true,
        is_a2g: false,
    },
    WeaponRule {
        patterns: &["r_27et", "r27et"],
        short_name: "R-27ET",
        code: "IR-LR",
        kind: "aam_ir",
        color: "#d9442c",
        default_icon: "air_to_air_midrange_missile.png",
        is_a2a: true,
        is_a2g: false,
    },
    WeaponRule {
        patterns: &["r_27r", "r27r"],
        short_name: "R-27R",
        code: "SARH",
        kind: "aam_sarh",
        color: "#2b7a78",
        default_icon: "air_to_air_midrange_missile.png",
        is_a2a: true,
        is_a2g: false,
    },
    WeaponRule {
        patterns: &["r_27t", "r27t"],
        short_name: "R-27T",
        code: "IR-LR",
        kind: "aam_ir",
        color: "#d9442c",
        default_icon: "air_to_air_midrange_missile.png",
        is_a2a: true,
        is_a2g: false,
    },
    WeaponRule {
        patterns: &["r_60m", "r60m"],
        short_name: "R-60M",
        code: "IR",
        kind: "aam_ir",
        color: "#d9442c",
        default_icon: "air_to_air_missile.png",
        is_a2a: true,
        is_a2g: false,
    },
    WeaponRule {
        patterns: &["r_60", "r60"],
        short_name: "R-60",
        code: "IR",
        kind: "aam_ir",
        color: "#d9442c",
        default_icon: "air_to_air_missile.png",
        is_a2a: true,
        is_a2g: false,
    },
    WeaponRule {
        patterns: &["r_24r", "r24r"],
        short_name: "R-24R",
        code: "SARH",
        kind: "aam_sarh",
        color: "#2b7a78",
        default_icon: "air_to_air_midrange_missile.png",
        is_a2a: true,
        is_a2g: false,
    },
    WeaponRule {
        patterns: &["r_24t", "r24t"],
        short_name: "R-24T",
        code: "IR",
        kind: "aam_ir",
        color: "#d9442c",
        default_icon: "air_to_air_midrange_missile.png",
        is_a2a: true,
        is_a2g: false,
    },
    WeaponRule {
        patterns: &["r_13m", "r13m"],
        short_name: "R-13M1",
        code: "IR",
        kind: "aam_ir",
        color: "#d9442c",
        default_icon: "air_to_air_missile.png",
        is_a2a: true,
        is_a2g: false,
    },
    WeaponRule {
        patterns: &["r_3s", "r3s"],
        short_name: "R-3S",
        code: "IR",
        kind: "aam_ir",
        color: "#d9442c",
        default_icon: "air_to_air_missile.png",
        is_a2a: true,
        is_a2g: false,
    },
    // US / NATO A2A
    WeaponRule {
        patterns: &["aim_120c", "aim120c"],
        short_name: "AIM-120C",
        code: "ARH",
        kind: "aam_arh",
        color: "#1d6fa5",
        default_icon: "air_to_air_midrange_missile.png",
        is_a2a: true,
        is_a2g: false,
    },
    WeaponRule {
        patterns: &["aim_120d", "aim120d"],
        short_name: "AIM-120D",
        code: "ARH",
        kind: "aam_arh",
        color: "#1d6fa5",
        default_icon: "air_to_air_midrange_missile.png",
        is_a2a: true,
        is_a2g: false,
    },
    WeaponRule {
        patterns: &["aim_120", "aim120"],
        short_name: "AIM-120A/B",
        code: "ARH",
        kind: "aam_arh",
        color: "#1d6fa5",
        default_icon: "air_to_air_midrange_missile.png",
        is_a2a: true,
        is_a2g: false,
    },
    WeaponRule {
        patterns: &["aim_54", "aim54"],
        short_name: "AIM-54 Phoenix",
        code: "ARH",
        kind: "aam_arh",
        color: "#1d6fa5",
        default_icon: "air_to_air_midrange_missile.png",
        is_a2a: true,
        is_a2g: false,
    },
    WeaponRule {
        patterns: &["aim_7p", "aim7p"],
        short_name: "AIM-7P Sparrow",
        code: "SARH",
        kind: "aam_sarh",
        color: "#2b7a78",
        default_icon: "air_to_air_midrange_missile.png",
        is_a2a: true,
        is_a2g: false,
    },
    WeaponRule {
        patterns: &["aim_7m", "aim7m"],
        short_name: "AIM-7M Sparrow",
        code: "SARH",
        kind: "aam_sarh",
        color: "#2b7a78",
        default_icon: "air_to_air_midrange_missile.png",
        is_a2a: true,
        is_a2g: false,
    },
    WeaponRule {
        patterns: &["aim_7f", "aim7f"],
        short_name: "AIM-7F Sparrow",
        code: "SARH",
        kind: "aam_sarh",
        color: "#2b7a78",
        default_icon: "air_to_air_midrange_missile.png",
        is_a2a: true,
        is_a2g: false,
    },
    WeaponRule {
        patterns: &["aim_7e", "aim7e"],
        short_name: "AIM-7E-2 Sparrow",
        code: "SARH",
        kind: "aam_sarh",
        color: "#2b7a78",
        default_icon: "air_to_air_midrange_missile.png",
        is_a2a: true,
        is_a2g: false,
    },
    WeaponRule {
        patterns: &["aim_7", "aim7", "sparrow"],
        short_name: "AIM-7 Sparrow",
        code: "SARH",
        kind: "aam_sarh",
        color: "#2b7a78",
        default_icon: "air_to_air_midrange_missile.png",
        is_a2a: true,
        is_a2g: false,
    },
    WeaponRule {
        patterns: &["aim_92", "aim92", "atas", "stinger"],
        short_name: "AIM-92 Stinger",
        code: "STINGER",
        kind: "aam_ir",
        color: "#d9442c",
        default_icon: "missile_air_to_air_group_heli.png",
        is_a2a: true,
        is_a2g: false,
    },
    WeaponRule {
        patterns: &["aim_9x", "aim9x"],
        short_name: "AIM-9X",
        code: "IR",
        kind: "aam_ir",
        color: "#d9442c",
        default_icon: "air_to_air_missile.png",
        is_a2a: true,
        is_a2g: false,
    },
    WeaponRule {
        patterns: &["aim_9m", "aim9m"],
        short_name: "AIM-9M",
        code: "IR",
        kind: "aam_ir",
        color: "#d9442c",
        default_icon: "air_to_air_missile.png",
        is_a2a: true,
        is_a2g: false,
    },
    WeaponRule {
        patterns: &["aim_9l", "aim9l"],
        short_name: "AIM-9L",
        code: "IR",
        kind: "aam_ir",
        color: "#d9442c",
        default_icon: "air_to_air_missile.png",
        is_a2a: true,
        is_a2g: false,
    },
    WeaponRule {
        patterns: &["aim_9j", "aim9j", "aim_9p", "aim9p"],
        short_name: "AIM-9J/P",
        code: "IR",
        kind: "aam_ir",
        color: "#d9442c",
        default_icon: "air_to_air_missile.png",
        is_a2a: true,
        is_a2g: false,
    },
    WeaponRule {
        patterns: &["aim_9e", "aim9e", "aim_9b", "aim9b"],
        short_name: "AIM-9B/E",
        code: "IR",
        kind: "aam_ir",
        color: "#d9442c",
        default_icon: "air_to_air_missile.png",
        is_a2a: true,
        is_a2g: false,
    },
    WeaponRule {
        patterns: &["aim_9", "aim9", "sidewinder"],
        short_name: "AIM-9",
        code: "IR",
        kind: "aam_ir",
        color: "#d9442c",
        default_icon: "air_to_air_missile.png",
        is_a2a: true,
        is_a2g: false,
    },
    WeaponRule {
        patterns: &["meteor"],
        short_name: "MBDA Meteor",
        code: "ARH",
        kind: "aam_arh",
        color: "#1d6fa5",
        default_icon: "air_to_air_midrange_missile.png",
        is_a2a: true,
        is_a2g: false,
    },
    WeaponRule {
        patterns: &["mica_em", "micaem"],
        short_name: "MICA-EM",
        code: "ARH",
        kind: "aam_arh",
        color: "#1d6fa5",
        default_icon: "air_to_air_midrange_missile.png",
        is_a2a: true,
        is_a2g: false,
    },
    WeaponRule {
        patterns: &["mica_ir", "micair"],
        short_name: "MICA-IR",
        code: "IR",
        kind: "aam_ir",
        color: "#d9442c",
        default_icon: "air_to_air_missile.png",
        is_a2a: true,
        is_a2g: false,
    },
    WeaponRule {
        patterns: &["magic"],
        short_name: "Magic 2",
        code: "IR",
        kind: "aam_ir",
        color: "#d9442c",
        default_icon: "air_to_air_missile.png",
        is_a2a: true,
        is_a2g: false,
    },
    WeaponRule {
        patterns: &["super_530", "super530"],
        short_name: "Super 530D",
        code: "SARH",
        kind: "aam_sarh",
        color: "#2b7a78",
        default_icon: "air_to_air_midrange_missile.png",
        is_a2a: true,
        is_a2g: false,
    },
    WeaponRule {
        patterns: &["pl_12", "pl12"],
        short_name: "PL-12",
        code: "ARH",
        kind: "aam_arh",
        color: "#1d6fa5",
        default_icon: "air_to_air_midrange_missile.png",
        is_a2a: true,
        is_a2g: false,
    },
    WeaponRule {
        patterns: &["pl_8", "pl8"],
        short_name: "PL-8",
        code: "IR",
        kind: "aam_ir",
        color: "#d9442c",
        default_icon: "air_to_air_missile.png",
        is_a2a: true,
        is_a2g: false,
    },
    WeaponRule {
        patterns: &["pl_5", "pl5"],
        short_name: "PL-5",
        code: "IR",
        kind: "aam_ir",
        color: "#d9442c",
        default_icon: "air_to_air_missile.png",
        is_a2a: true,
        is_a2g: false,
    },
    WeaponRule {
        patterns: &["derby"],
        short_name: "Derby",
        code: "ARH",
        kind: "aam_arh",
        color: "#1d6fa5",
        default_icon: "air_to_air_midrange_missile.png",
        is_a2a: true,
        is_a2g: false,
    },
    WeaponRule {
        patterns: &["python"],
        short_name: "Python",
        code: "IR",
        kind: "aam_ir",
        color: "#d9442c",
        default_icon: "air_to_air_missile.png",
        is_a2a: true,
        is_a2g: false,
    },
    WeaponRule {
        patterns: &["aam_4", "aam4"],
        short_name: "AAM-4",
        code: "ARH",
        kind: "aam_arh",
        color: "#1d6fa5",
        default_icon: "air_to_air_midrange_missile.png",
        is_a2a: true,
        is_a2g: false,
    },
    WeaponRule {
        patterns: &["aam_3", "aam3"],
        short_name: "AAM-3",
        code: "IR",
        kind: "aam_ir",
        color: "#d9442c",
        default_icon: "air_to_air_missile.png",
        is_a2a: true,
        is_a2g: false,
    },
    WeaponRule {
        patterns: &["atam", "igla", "9m39"],
        short_name: "9M39 Igla",
        code: "IGLA",
        kind: "aam_ir",
        color: "#d9442c",
        default_icon: "air_to_air_missile.png",
        is_a2a: true,
        is_a2g: false,
    },
    WeaponRule {
        patterns: &["mistral"],
        short_name: "Mistral",
        code: "AAM",
        kind: "aam_ir",
        color: "#d9442c",
        default_icon: "air_to_air_missile.png",
        is_a2a: true,
        is_a2g: false,
    },
    // Helicopter & Western ATGMs / AGMs
    WeaponRule {
        patterns: &["114l"],
        short_name: "AGM-114L Longbow",
        code: "AGM",
        kind: "agm",
        color: "#b36b00",
        default_icon: "atgm_type_helfire_x2.png",
        is_a2a: false,
        is_a2g: true,
    },
    WeaponRule {
        patterns: &["114k"],
        short_name: "AGM-114K Hellfire",
        code: "AGM",
        kind: "agm",
        color: "#b36b00",
        default_icon: "atgm_type_helfire_x2.png",
        is_a2a: false,
        is_a2g: true,
    },
    WeaponRule {
        patterns: &["114b"],
        short_name: "AGM-114B Hellfire",
        code: "AGM",
        kind: "agm",
        color: "#b36b00",
        default_icon: "atgm_type_helfire_x2.png",
        is_a2a: false,
        is_a2g: true,
    },
    WeaponRule {
        patterns: &["114", "hellfire", "helfire"],
        short_name: "AGM-114 Hellfire",
        code: "AGM",
        kind: "agm",
        color: "#b36b00",
        default_icon: "atgm_type_helfire_x2.png",
        is_a2a: false,
        is_a2g: true,
    },
    WeaponRule {
        patterns: &["agm_179", "jagm"],
        short_name: "AGM-179 JAGM",
        code: "JAGM",
        kind: "agm",
        color: "#b36b00",
        default_icon: "atgm_type_helfire_x2.png",
        is_a2a: false,
        is_a2g: true,
    },
    WeaponRule {
        patterns: &["spike_nlos"],
        short_name: "Spike NLOS",
        code: "SPIKE",
        kind: "agm",
        color: "#b36b00",
        default_icon: "atgm_type_helfire_x2.png",
        is_a2a: false,
        is_a2g: true,
    },
    WeaponRule {
        patterns: &["spike"],
        short_name: "Spike ER",
        code: "SPIKE",
        kind: "agm",
        color: "#b36b00",
        default_icon: "atgm_type_helfire_x2.png",
        is_a2a: false,
        is_a2g: true,
    },
    WeaponRule {
        patterns: &["vihr", "vikhr", "9k127"],
        short_name: "9K127 Vikhr",
        code: "VIKHR",
        kind: "agm",
        color: "#b36b00",
        default_icon: "agm_missile.png",
        is_a2a: false,
        is_a2g: true,
    },
    WeaponRule {
        patterns: &["ataka", "9m120"],
        short_name: "9M120 Ataka",
        code: "ATGM",
        kind: "agm",
        color: "#b36b00",
        default_icon: "agm_missile.png",
        is_a2a: false,
        is_a2g: true,
    },
    WeaponRule {
        patterns: &["shturm", "9m114"],
        short_name: "9M114 Shturm",
        code: "ATGM",
        kind: "agm",
        color: "#b36b00",
        default_icon: "agm_missile.png",
        is_a2a: false,
        is_a2g: true,
    },
    WeaponRule {
        patterns: &["pars", "trigat"],
        short_name: "PARS 3 LR",
        code: "ATGM",
        kind: "agm",
        color: "#b36b00",
        default_icon: "agm_missile.png",
        is_a2a: false,
        is_a2g: true,
    },
    WeaponRule {
        patterns: &["tow"],
        short_name: "BGM-71 TOW",
        code: "TOW",
        kind: "agm",
        color: "#b36b00",
        default_icon: "agm_missile.png",
        is_a2a: false,
        is_a2g: true,
    },
    WeaponRule {
        patterns: &["agm_65", "agm65", "maverick"],
        short_name: "AGM-65 Maverick",
        code: "AGM",
        kind: "agm",
        color: "#b36b00",
        default_icon: "agm_missile.png",
        is_a2a: false,
        is_a2g: true,
    },
    WeaponRule {
        patterns: &["agm_88", "agm88"],
        short_name: "AGM-88 HARM",
        code: "ARM",
        kind: "agm",
        color: "#b36b00",
        default_icon: "agm_missile.png",
        is_a2a: false,
        is_a2g: true,
    },
    WeaponRule {
        patterns: &["agm_84", "agm84"],
        short_name: "AGM-84 Harpoon",
        code: "AShM",
        kind: "agm",
        color: "#b36b00",
        default_icon: "agm_missile.png",
        is_a2a: false,
        is_a2g: true,
    },
    // Guided & Dumb Bombs
    WeaponRule {
        patterns: &["gbu_31", "gbu31", "gbu_38", "gbu38", "jdam"],
        short_name: "JDAM",
        code: "GBU",
        kind: "gbu",
        color: "#2d6a4f",
        default_icon: "guided_bomb_middle_jdam.png",
        is_a2a: false,
        is_a2g: true,
    },
    WeaponRule {
        patterns: &[
            "gbu_10", "gbu_12", "gbu_16", "gbu_24", "gbu", "paveway", "kab_500", "kab_1500", "kab",
        ],
        short_name: "Guided Bomb",
        code: "GBU",
        kind: "gbu",
        color: "#2d6a4f",
        default_icon: "guided_bomb_middle_jdam.png",
        is_a2a: false,
        is_a2g: true,
    },
    WeaponRule {
        patterns: &["fab_1500", "fab_3000", "mk_84", "mk84", "2000lb", "1000kg"],
        short_name: "Heavy Bomb",
        code: "BOMB",
        kind: "bomb",
        color: "#4a5568",
        default_icon: "bombs_large.png",
        is_a2a: false,
        is_a2g: true,
    },
    WeaponRule {
        patterns: &["fab_500", "mk_83", "mk83", "1000lb", "500kg"],
        short_name: "Medium Bomb",
        code: "BOMB",
        kind: "bomb",
        color: "#4a5568",
        default_icon: "bombs_middle.png",
        is_a2a: false,
        is_a2g: true,
    },
    WeaponRule {
        patterns: &["mk_82", "mk82"],
        short_name: "MK82 500 lb",
        code: "BOMB",
        kind: "bomb",
        color: "#4a5568",
        default_icon: "bombs_middle.png",
        is_a2a: false,
        is_a2g: true,
    },
    WeaponRule {
        patterns: &[
            "fab_250", "ofab_100", "mk_81", "mk81", "500lb", "250lb", "bomb", "fab", "ofab",
        ],
        short_name: "Bomb",
        code: "BOMB",
        kind: "bomb",
        color: "#4a5568",
        default_icon: "bombs_small.png",
        is_a2a: false,
        is_a2g: true,
    },
    // Rockets
    WeaponRule {
        patterns: &["apkws", "hydra", "m247", "m151", "m282"],
        short_name: "Hydra-70 / APKWS",
        code: "RCKT",
        kind: "rocket",
        color: "#5c6d7e",
        default_icon: "nar_ap_large.png",
        is_a2a: false,
        is_a2g: true,
    },
    WeaponRule {
        patterns: &[
            "s_25", "s25", "s_24", "s24", "s_13", "s13", "s_8", "s8", "b8m", "nar", "rocket",
            "sneb",
        ],
        short_name: "Rocket Pod",
        code: "RCKT",
        kind: "rocket",
        color: "#5c6d7e",
        default_icon: "nar_ap_large.png",
        is_a2a: false,
        is_a2g: true,
    },
    // Utility Pods & Guns
    WeaponRule {
        patterns: &["tank", "fuel", "ptb"],
        short_name: "Drop Tank",
        code: "FUEL",
        kind: "fuel",
        color: "#475569",
        default_icon: "fuel_tank_middle.png",
        is_a2a: false,
        is_a2g: false,
    },
    WeaponRule {
        patterns: &[
            "pod",
            "targeting",
            "sniper",
            "litening",
            "lantirn",
            "atlis",
            "damocles",
        ],
        short_name: "Targeting Pod",
        code: "POD",
        kind: "pod",
        color: "#6b46c1",
        default_icon: "aiming.png",
        is_a2a: false,
        is_a2g: false,
    },
    WeaponRule {
        patterns: &["m134", "minigun", "gau_", "cannon", "gun", "m61", "gsh_"],
        short_name: "Gun Pod",
        code: "GUN",
        kind: "gun",
        color: "#334155",
        default_icon: "gunpods.png",
        is_a2a: true,
        is_a2g: true,
    },
];

static WEAPONRY_LOC_MAP: OnceLock<RwLock<HashMap<String, HashMap<String, String>>>> =
    OnceLock::new();
static MOD_BULLET_MAP: OnceLock<RwLock<HashMap<String, ModBulletMeta>>> = OnceLock::new();

fn weaponry_map() -> &'static RwLock<HashMap<String, HashMap<String, String>>> {
    WEAPONRY_LOC_MAP.get_or_init(|| RwLock::new(HashMap::new()))
}

fn mod_bullet_map() -> &'static RwLock<HashMap<String, ModBulletMeta>> {
    MOD_BULLET_MAP.get_or_init(|| RwLock::new(HashMap::new()))
}

pub fn load_modifications_index(mods_path: &Path) -> usize {
    if !mods_path.exists() {
        return 0;
    }
    let Ok(file) = File::open(mods_path) else {
        return 0;
    };
    let reader = BufReader::with_capacity(64 * 1024, file);
    let Ok(raw_map) = serde_json::from_reader::<_, HashMap<String, serde_json::Value>>(reader)
    else {
        return 0;
    };

    let mut out: HashMap<String, ModBulletMeta> = HashMap::with_capacity(1500);
    let mut ingest = |k: &str, v: &serde_json::Value| {
        if let Ok(ModTopVal::Entry(m)) = serde_json::from_value::<ModTopVal>(v.clone()) {
            let abm = m.effects.and_then(|e| e.additive_bullet_mod);
            if abm.is_some() || m.is_reserve {
                out.insert(
                    k.trim().to_lowercase(),
                    ModBulletMeta {
                        additive_bullet_mod: abm,
                        is_reserve: m.is_reserve,
                    },
                );
            }
        }
    };

    if let Some(mods_obj) = raw_map.get("modifications").and_then(|m| m.as_object()) {
        for (k, v) in mods_obj {
            ingest(k, v);
        }
    } else {
        for (k, v) in &raw_map {
            ingest(k, v);
        }
    }

    let count = out.len();
    if let Ok(mut guard) = mod_bullet_map().write() {
        *guard = out;
    }
    info!("[MODS.BLKX] Indexed {} bullet/reserve modifications", count);
    count
}

pub fn resolve_additive_bullet_mod(mod_key: &str) -> Option<ModBulletMeta> {
    let key_low = mod_key.trim().to_lowercase();
    mod_bullet_map().read().ok()?.get(&key_low).cloned()
}

pub fn modifications_index_ready() -> bool {
    mod_bullet_map()
        .read()
        .map(|g| !g.is_empty())
        .unwrap_or(false)
}

#[derive(Debug, Clone, Copy)]
pub struct CsvLangColumns {
    pub en: usize,
    pub fr: usize,
    pub de: usize,
    pub es: usize,
    pub ru: usize,
    pub zh: usize,
    pub ja: usize,
    pub pt: usize,
}

impl Default for CsvLangColumns {
    fn default() -> Self {
        Self {
            en: 1,
            fr: 2,
            de: 4,
            es: 5,
            ru: 6,
            zh: 10,
            ja: 11,
            pt: 12,
        }
    }
}

impl CsvLangColumns {
    /// Detects language column indices from a War Thunder Datamine CSV header row,
    /// falling back to the standard column layout when a header label is absent.
    pub fn from_header(header_cols: &[String]) -> Self {
        let mut cols = Self::default();
        let mut found_zh = false;
        for (idx, raw) in header_cols.iter().enumerate() {
            let norm = raw
                .trim()
                .trim_matches(|c| c == '<' || c == '>' || c == '"')
                .to_lowercase();
            if norm == "english" {
                cols.en = idx;
            } else if norm == "french" {
                cols.fr = idx;
            } else if norm == "german" {
                cols.de = idx;
            } else if norm == "spanish" {
                cols.es = idx;
            } else if norm == "russian" {
                cols.ru = idx;
            } else if norm == "chinese" {
                cols.zh = idx;
                found_zh = true;
            } else if !found_zh && norm == "hchinese" {
                cols.zh = idx;
            } else if norm == "japanese" {
                cols.ja = idx;
            } else if norm == "portuguese" {
                cols.pt = idx;
            }
        }
        cols
    }

    pub fn col_or<'a>(&self, row: &'a [String], idx: usize, fallback: &'a str) -> &'a str {
        row.get(idx)
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .unwrap_or(fallback)
    }
}

pub type WeaponryCsvParsed = (
    HashMap<String, HashMap<String, String>>,
    Vec<(String, serde_json::Value)>,
);

pub fn parse_weaponry_csv(csv_path: &Path) -> WeaponryCsvParsed {
    let mut in_mem: HashMap<String, HashMap<String, String>> = HashMap::new();
    let mut db_entries: Vec<(String, serde_json::Value)> = Vec::new();
    if !csv_path.exists() {
        return (in_mem, db_entries);
    }
    let Ok(content) = fs::read_to_string(csv_path) else {
        return (in_mem, db_entries);
    };

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
        let raw_key = cols[0].trim().to_lowercase();
        if raw_key.is_empty() {
            continue;
        }
        let en = cols.get(lang_cols.en).map(|s| s.trim()).unwrap_or("");
        if en.is_empty() {
            continue;
        }
        let pt = lang_cols.col_or(&cols, lang_cols.pt, en);
        let es = lang_cols.col_or(&cols, lang_cols.es, en);
        let ru = lang_cols.col_or(&cols, lang_cols.ru, en);
        let fr = lang_cols.col_or(&cols, lang_cols.fr, en);
        let zh = lang_cols.col_or(&cols, lang_cols.zh, en);
        let ja = lang_cols.col_or(&cols, lang_cols.ja, en);

        let mut langs = HashMap::new();
        langs.insert("en-US".to_string(), en.to_string());
        langs.insert("pt-BR".to_string(), pt.to_string());
        langs.insert("es".to_string(), es.to_string());
        langs.insert("ru".to_string(), ru.to_string());
        langs.insert("fr".to_string(), fr.to_string());
        langs.insert("zh".to_string(), zh.to_string());
        langs.insert("ja".to_string(), ja.to_string());

        let short_key = raw_key
            .strip_prefix("weapons/")
            .unwrap_or(&raw_key)
            .to_string();

        if !short_key.is_empty() {
            in_mem.insert(short_key, langs);
        }
    }

    for (short_key, langs) in &in_mem {
        db_entries.push((
            short_key.clone(),
            json!({
                "en-US": langs.get("en-US").cloned().unwrap_or_default(),
                "pt-BR": langs.get("pt-BR").cloned().unwrap_or_default(),
                "es": langs.get("es").cloned().unwrap_or_default(),
                "ru": langs.get("ru").cloned().unwrap_or_default(),
                "fr": langs.get("fr").cloned().unwrap_or_default(),
                "zh": langs.get("zh").cloned().unwrap_or_default(),
                "ja": langs.get("ja").cloned().unwrap_or_default(),
            }),
        ));
    }

    (in_mem, db_entries)
}

pub fn load_weaponry_csv_with_db(csv_path: &Path, db: Option<&CatalogDatabase>) -> usize {
    let (in_mem, db_entries) = parse_weaponry_csv(csv_path);
    if in_mem.is_empty() {
        return 0;
    }
    let count = in_mem.len();
    if let Ok(mut guard) = weaponry_map().write() {
        *guard = in_mem;
    }
    if let Some(target_db) = db {
        if !db_entries.is_empty() {
            if let Err(e) = target_db.replace_weaponry_loc(&db_entries) {
                tracing::warn!(
                    "[WEAPONS.CSV] Failed persisting {} entries to SQLite: {}",
                    db_entries.len(),
                    e
                );
            }
        }
    }
    info!("[WEAPONS.CSV] Indexed {} localized weapon entries", count);
    count
}

pub fn lookup_localized_weapon_name(key: &str, lang: &str) -> Option<String> {
    let clean = key
        .trim()
        .to_lowercase()
        .strip_prefix("weapons/")
        .unwrap_or(&key.trim().to_lowercase())
        .to_string();

    if let Ok(guard) = weaponry_map().read() {
        if let Some(langs) = guard.get(&clean) {
            if let Some(v) = langs.get(lang).or_else(|| langs.get("en-US")) {
                return Some(v.clone());
            }
        }
    }
    CatalogDatabase::get_instance().lookup_weaponry_name(&clean, lang)
}

/// Parses a semicolon-separated CSV line respecting double quotes `""`.
pub fn parse_semicolon_csv_row(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '"' {
            if in_quotes && i + 1 < chars.len() && chars[i + 1] == '"' {
                current.push('"');
                i += 2;
                continue;
            }
            in_quotes = !in_quotes;
        } else if c == ';' && !in_quotes {
            fields.push(current.clone());
            current.clear();
        } else {
            current.push(c);
        }
        i += 1;
    }
    fields.push(current);
    fields
}

pub fn classify_aircraft_weapon(
    preset_id: &str,
    blk: &str,
    opt_info: Option<&WeaponPresetOption>,
    unit_exp_class: &str,
    weapon_telemetry: Option<&serde_json::Value>,
) -> (serde_json::Value, bool, bool) {
    let icon_type_str = opt_info.and_then(|o| o.icon_type.as_deref()).unwrap_or("");
    let opt_blk = opt_info.and_then(|o| o.blk.as_deref()).unwrap_or("");
    let trigger = opt_info.and_then(|o| o.trigger.as_deref()).unwrap_or("");
    let combined = format!(
        "{} {} {} {} {}",
        preset_id, blk, icon_type_str, opt_blk, trigger
    )
    .to_lowercase();

    let count = extract_multiplier_count(&combined);

    let matched = WEAPON_RULES
        .iter()
        .find(|r| r.patterns.iter().any(|p| combined.contains(p)));

    let (mut short_name, code, kind, color, default_icon, is_a2a, is_a2g) =
        if let Some(rule) = matched {
            let mut a2a = rule.is_a2a;
            let mut a2g = rule.is_a2g;
            if rule.kind == "gun" {
                if unit_exp_class == "exp_helicopter" || unit_exp_class == "exp_assault" {
                    a2a = false;
                    a2g = true;
                } else {
                    a2a = true;
                    a2g = false;
                }
            }
            (
                rule.short_name.to_string(),
                rule.code,
                rule.kind,
                rule.color,
                rule.default_icon,
                a2a,
                a2g,
            )
        } else if unit_exp_class == "exp_helicopter" || unit_exp_class == "exp_assault" {
            (
                "Combat Load".to_string(),
                "WPN",
                "agm",
                "#b36b00",
                "atgm_type_helfire_x2.png",
                false,
                true,
            )
        } else {
            (
                "Air Weapon".to_string(),
                "AAM",
                "aam_ir",
                "#3b82f6",
                "air_to_air_missile.png",
                true,
                false,
            )
        };

    // Enrich with authentic localized name from units_weaponry.csv when available
    let blk_stem = Path::new(if !opt_blk.is_empty() { opt_blk } else { blk })
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    let mut full_name = short_name.clone();
    if !blk_stem.is_empty() {
        if let Some(loc_short) =
            lookup_localized_weapon_name(&format!("{}/short", blk_stem), "en-US")
        {
            short_name = loc_short;
        }
        if let Some(loc_full) = lookup_localized_weapon_name(blk_stem, "en-US") {
            full_name = loc_full;
        }
    }

    let chosen_icon = if !icon_type_str.is_empty() {
        format!("{}.png", icon_type_str)
    } else {
        default_icon.to_string()
    };

    let range_str = weapon_telemetry
        .and_then(|t| t.get("rangeText"))
        .and_then(|s| s.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            if is_a2g {
                "Ground Target".to_string()
            } else {
                "Air Combat".to_string()
            }
        });

    let overload_str = weapon_telemetry
        .and_then(|t| t.get("overloadText"))
        .and_then(|s| s.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| code.to_string());

    let mass_str = weapon_telemetry
        .and_then(|t| t.get("massText"))
        .and_then(|s| s.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "—".to_string());

    let opt = json!({
        "presetId": preset_id,
        "name": short_name,
        "shortName": short_name,
        "fullName": full_name,
        "count": count,
        "iconFile": chosen_icon,
        "code": code,
        "kind": kind,
        "color": color,
        "mass": mass_str,
        "overload": overload_str,
        "range": range_str,
        "telemetry": weapon_telemetry.cloned().unwrap_or(serde_json::Value::Null),
    });

    (opt, is_a2a, is_a2g)
}

fn extract_multiplier_count(combined: &str) -> i32 {
    for n in [38, 28, 24, 19, 16, 14, 12, 10, 8, 7, 6, 5, 4, 3, 2] {
        let pat_u = format!("_x{}", n);
        let pat_x = format!("x{}", n);
        let pat_nx_u = format!("_{}x_", n);
        let pat_nx_end = format!("_{}x", n);
        if combined.contains(&pat_u)
            || combined.contains(&pat_x)
            || combined.contains(&pat_nx_u)
            || combined.contains(&pat_nx_end)
        {
            return n;
        }
    }
    1
}

fn parse_single_shell(
    shell_id: &str,
    bullet_raw: &serde_json::Value,
    is_default: bool,
) -> Option<serde_json::Value> {
    let b = if let Some(arr) = bullet_raw.as_array() {
        arr.first()?
    } else {
        bullet_raw
    };
    if !b.is_object() {
        return None;
    }

    let rocket = b.get("rocket").filter(|r| r.is_object());

    let bullet_type = b
        .get("bulletType")
        .and_then(|v| v.as_str())
        .unwrap_or("shell");
    let caliber_m = rocket
        .and_then(|r| r.get("caliber"))
        .or_else(|| b.get("caliber"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let dmg_caliber_m = b.get("damageCaliber").and_then(|v| v.as_f64());
    let mass_kg = rocket
        .and_then(|r| r.get("mass"))
        .or_else(|| b.get("mass"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let dmg_mass_kg = b.get("damageMass").and_then(|v| v.as_f64());
    let speed_ms = b
        .get("speed")
        .or_else(|| rocket.and_then(|r| r.get("endSpeed").or_else(|| r.get("maxSpeed"))))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let exp_mass = rocket
        .and_then(|r| r.get("explosiveMass"))
        .or_else(|| b.get("explosiveMass"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let exp_type = rocket
        .and_then(|r| r.get("explosiveType"))
        .or_else(|| b.get("explosiveType"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let (pen0, pen100, pen500, pen1000, pen1500, pen2000) = extract_penetration_curve(
        b,
        rocket,
        bullet_type,
        caliber_m,
        dmg_caliber_m,
        mass_kg,
        dmg_mass_kg,
        speed_ms,
        exp_mass,
    );

    let bullet_name = b
        .get("bulletName")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(shell_id);
    let human_name = lookup_localized_weapon_name(bullet_name, "en-US")
        .or_else(|| lookup_localized_weapon_name(shell_id, "en-US"))
        .unwrap_or_else(|| bullet_name.replace('_', " "));

    let code = classify_shell_code(shell_id, bullet_type);

    Some(json!({
        "id": shell_id,
        "bulletName": bullet_name,
        "isDefault": is_default,
        "name": human_name,
        "code": code,
        "bulletType": bullet_type,
        "caliberMm": (caliber_m * 1000.0 * 10.0).round() / 10.0,
        "massKg": (mass_kg * 100.0).round() / 100.0,
        "velocityMs": speed_ms.round(),
        "explosiveKg": (exp_mass * 1000.0).round() / 1000.0,
        "explosiveType": exp_type,
        "pen0": pen0.round(),
        "pen100": pen100.round(),
        "pen500": pen500.round(),
        "pen1000": pen1000.round(),
        "pen1500": pen1500.round(),
        "pen2000": pen2000.round(),
        "pen60deg0": (pen0 * slope_retention_at_60(code)).round(),
        "pen60deg1000": (pen1000 * slope_retention_at_60(code)).round(),
    }))
}

/// Parses ballistic shells from a ground/naval cannon `.blkx` JSON structure,
/// including BOTH the root stock `"bullet"` and named modification sub-objects.
pub fn extract_shells_from_cannon_blkx(cannon_json: &serde_json::Value) -> Vec<serde_json::Value> {
    let mut shells = Vec::new();
    let Some(obj) = cannon_json.as_object() else {
        return shells;
    };

    let not_use_default_gui = obj
        .get("notUseDefaultBulletInGui")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if let Some(root_bullet) = obj.get("bullet") {
        let b_first = root_bullet
            .as_array()
            .and_then(|a| a.first())
            .unwrap_or(root_bullet);
        let default_id = b_first
            .get("bulletName")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or("default");
        if let Some(mut sh) = parse_single_shell(default_id, root_bullet, true) {
            if not_use_default_gui {
                if let Some(m) = sh.as_object_mut() {
                    m.insert("notUseDefaultInGui".to_string(), json!(true));
                }
            }
            shells.push(sh);
        }
    }

    for (mod_key, mod_val) in obj {
        let Some(mod_obj) = mod_val.as_object() else {
            continue;
        };
        let Some(bullet_raw) = mod_obj.get("bullet") else {
            continue;
        };
        if let Some(sh) = parse_single_shell(mod_key, bullet_raw, false) {
            shells.push(sh);
        }
    }

    shells
}

fn slope_retention_at_60(code: &str) -> f64 {
    match code {
        "APFSDS" => 0.58,
        "APDS" => 0.45,
        "APCR" => 0.34,
        "HEAT-FS" | "ATGM" => 0.50,
        "HEAT" => 0.48,
        "HESH" => 0.55,
        "APCBC" => 0.42,
        "APHE" | "APBC" | "AP" | "SAP" => 0.38,
        _ => 0.45,
    }
}

#[allow(clippy::too_many_arguments)]
fn extract_penetration_curve(
    bullet: &serde_json::Value,
    rocket: Option<&serde_json::Value>,
    bullet_type: &str,
    caliber_m: f64,
    dmg_caliber_m: Option<f64>,
    mass_kg: f64,
    dmg_mass_kg: Option<f64>,
    speed_ms: f64,
    exp_mass: f64,
) -> (f64, f64, f64, f64, f64, f64) {
    let ap = rocket
        .and_then(|r| r.get("armorpower"))
        .or_else(|| bullet.get("armorpower"));
    let get_ap = |key: &str| -> Option<f64> {
        ap.and_then(|a| a.get(key))
            .and_then(|arr| arr.as_array())
            .and_then(|arr| arr.first())
            .and_then(|v| v.as_f64())
    };
    let ap0 = get_ap("ArmorPower0m").unwrap_or(0.0);

    // 1. Chemical shaped-charge penetration (HEAT / HEAT-FS / ATGM / HE-OR cumulative jet)
    let cumul_pen = rocket
        .and_then(|r| r.get("cumulativeDamage"))
        .or_else(|| bullet.get("cumulativeDamage"))
        .and_then(|c| c.get("armorPower"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    let bt_low = bullet_type.to_lowercase();
    if cumul_pen > 10.0 && (bt_low.contains("heat") || bt_low.contains("atgm") || ap0 <= 15.0) {
        return (
            cumul_pen, cumul_pen, cumul_pen, cumul_pen, cumul_pen, cumul_pen,
        );
    }

    // 2. Explicit ArmorPower table
    if let Some(p0) = get_ap("ArmorPower0m") {
        let p100 = get_ap("ArmorPower100m").unwrap_or(p0 * 0.99);
        let p500 = get_ap("ArmorPower500m").unwrap_or(p0 * 0.94);
        let p1000 = get_ap("ArmorPower1000m").unwrap_or(p0 * 0.88);
        let p1500 = get_ap("ArmorPower1500m").unwrap_or(p0 * 0.82);
        let p2000 = get_ap("ArmorPower2000m").unwrap_or(p0 * 0.76);
        if p0 > 8.0 {
            return (p0, p100, p500, p1000, p1500, p2000);
        }
    }

    // 3. Lanz-Odermatt APFSDS kinetic equation fallback
    let kin = rocket
        .and_then(|r| r.get("damage"))
        .or_else(|| bullet.get("damage"))
        .and_then(|d| d.get("kinetic"));

    if let Some(lo_len) = kin
        .and_then(|k| k.get("lanzOdermattWorkingLength"))
        .and_then(|v| v.as_f64())
    {
        let density = kin
            .and_then(|k| k.get("lanzOdermattDensity"))
            .and_then(|v| v.as_f64())
            .unwrap_or(17500.0);
        let v_km = (speed_ms / 1000.0).max(0.5);
        let p0 = (lo_len * 0.92 * (density / 18500.0).sqrt() * (v_km / 1.65)).max(50.0);
        return (
            p0,
            p0 * 0.995,
            p0 * 0.975,
            p0 * 0.952,
            p0 * 0.928,
            p0 * 0.905,
        );
    }

    // 4. De Marre kinetic formula using Gaijin's demarrePenetrationK + damageCaliber / damageMass
    let eff_cal_m = dmg_caliber_m.filter(|&c| c > 0.0).unwrap_or(caliber_m);
    let eff_mass_kg = dmg_mass_kg.filter(|&m| m > 0.0).unwrap_or(mass_kg);
    if eff_cal_m > 0.0 && eff_mass_kg > 0.0 && speed_ms > 0.0 {
        let demarre_k = kin
            .and_then(|k| k.get("demarrePenetrationK"))
            .and_then(|v| v.as_f64());
        let speed_pow = kin
            .and_then(|k| k.get("demarreSpeedPow"))
            .and_then(|v| v.as_f64())
            .unwrap_or(1.43);
        let mass_pow = kin
            .and_then(|k| k.get("demarreMassPow"))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.71);
        let cal_pow = kin
            .and_then(|k| k.get("demarreCaliberPow"))
            .and_then(|v| v.as_f64())
            .unwrap_or(1.07);

        let cal_mm = eff_cal_m * 1000.0;
        let k_armor: f64 = if demarre_k.is_some() || exp_mass > 0.05 {
            1900.0
        } else {
            2150.0
        };
        let dk = demarre_k.unwrap_or(1.0);
        let p0 = ((eff_mass_kg.powf(mass_pow) * speed_ms.powf(speed_pow))
            / (k_armor.powf(speed_pow) * (cal_mm / 100.0).powf(cal_pow)))
            * 100.0
            * dk;
        let p0_clamped = p0.clamp(1.0, 950.0);
        if dk <= 0.2 {
            // High-explosive / thin-walled shell fragmentation penetration stays nearly flat
            return (
                p0_clamped, p0_clamped, p0_clamped, p0_clamped, p0_clamped, p0_clamped,
            );
        }
        return (
            p0_clamped,
            p0_clamped * 0.96,
            p0_clamped * 0.86,
            p0_clamped * 0.75,
            p0_clamped * 0.65,
            p0_clamped * 0.56,
        );
    }

    (0.0, 0.0, 0.0, 0.0, 0.0, 0.0)
}

pub fn classify_shell_code(mod_id: &str, bullet_type: &str) -> &'static str {
    let bt = bullet_type.trim().to_lowercase();
    if !bt.is_empty() && bt != "shell" {
        if bt.contains("sam") || bt.contains("aam") {
            return "SAM";
        } else if bt.contains("atgm") {
            return "ATGM";
        } else if bt.contains("apfsds") || bt.contains("apds_fs") {
            return "APFSDS";
        } else if bt.contains("apds") {
            return "APDS";
        } else if bt.contains("apcr") {
            return "APCR";
        } else if bt.contains("heat_fs")
            || bt.contains("heatfs")
            || bt.contains("heat-fs")
            || bt.contains("heat_mp")
        {
            return "HEAT-FS";
        } else if bt.contains("heat") {
            return "HEAT";
        } else if bt.contains("hesh") {
            return "HESH";
        } else if bt.contains("he_vt")
            || bt.contains("_vt")
            || bt.contains("dist_fuse")
            || bt.contains("radio_fuse")
        {
            return "HE-VT";
        } else if bt.contains("sap") {
            return "SAP";
        } else if bt.contains("aphe") {
            return "APHE";
        } else if bt.contains("apcbc") {
            return "APCBC";
        } else if bt.contains("apbc") || bt.starts_with("ap_") || bt == "ap" {
            return "AP";
        } else if bt.contains("smoke") || bt.contains("smk") {
            return "SMK";
        } else if bt.contains("he") || bt.contains("frag") || bt.contains("shrapnel") {
            return "HE";
        }
    }

    let mid = mod_id.trim().to_lowercase();
    if mid.contains("atgm")
        || mid.contains("tow")
        || mid.contains("mim146")
        || mid.contains("9m1")
        || mid.contains("kornet")
        || mid.contains("konkurs")
        || mid.contains("ataka")
        || mid.contains("shturm")
        || mid.contains("bastion")
        || mid.contains("refleks")
        || mid.contains("krizantema")
    {
        "ATGM"
    } else if mid.contains("aam")
        || mid.contains("sam")
        || mid.contains("mim72")
        || mid.contains("mim_72")
        || mid.contains("9m31")
        || mid.contains("9m37")
        || mid.contains("9m39")
        || mid.contains("9m311")
        || mid.contains("9m33")
        || mid.contains("stinger")
        || mid.contains("igla")
        || mid.contains("mistral")
    {
        "SAM"
    } else if mid.contains("apfsds") || mid.contains("apds_fs") {
        "APFSDS"
    } else if mid.contains("apds") {
        "APDS"
    } else if mid.contains("apcr") {
        "APCR"
    } else if mid.contains("heat_fs")
        || mid.contains("heatfs")
        || mid.contains("heat-fs")
        || mid.contains("heat_mp")
    {
        "HEAT-FS"
    } else if mid.contains("heat") {
        "HEAT"
    } else if mid.contains("hesh") {
        "HESH"
    } else if mid.contains("he_vt")
        || mid.contains("_vt")
        || mid.contains("dist_fuse")
        || mid.contains("radio_fuse")
        || mid.contains("aavt")
    {
        "HE-VT"
    } else if mid.contains("sap") {
        "SAP"
    } else if mid.contains("aphe") {
        "APHE"
    } else if mid.contains("apcbc") {
        "APCBC"
    } else if mid.contains("apbc")
        || mid.contains("_ap_")
        || mid.ends_with("_ap")
        || mid.starts_with("ap_")
        || mid == "ap"
    {
        "AP"
    } else if mid.contains("smk") || mid.contains("smoke") {
        "SMK"
    } else if mid.contains("he") || mid.contains("frag") {
        "HE"
    } else {
        "SHELL"
    }
}

/// Extracts real missile/rocket/bomb performance telemetry from a weapon `.blkx`.
pub fn extract_air_weapon_telemetry(weapon_json: &serde_json::Value) -> serde_json::Value {
    let root = weapon_json
        .get("rocket")
        .or_else(|| weapon_json.get("bomb"))
        .unwrap_or(weapon_json);
    let mass = root.get("mass").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let max_speed = root
        .get("endSpeed")
        .or_else(|| root.get("maxSpeed"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let overload = root
        .get("loadFactorMax")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let range_m = root
        .get("rangeMax")
        .or_else(|| root.get("maxDistance"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let exp_mass = root
        .get("explosiveMass")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let guidance = root
        .get("guidanceType")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let range_text = if range_m >= 1000.0 {
        format!("{:.1} km", range_m / 1000.0)
    } else if range_m > 0.0 {
        format!("{:.0} m", range_m)
    } else {
        "—".to_string()
    };

    let overload_text = if overload > 0.0 {
        format!("{:.0}G • {:.0} m/s", overload, max_speed)
    } else if max_speed > 0.0 {
        format!("{:.0} m/s", max_speed)
    } else if exp_mass > 0.0 {
        format!("{:.1} kg TNT", exp_mass)
    } else {
        "—".to_string()
    };

    let mass_text = if mass > 0.0 {
        format!("{:.1} kg", mass)
    } else {
        "—".to_string()
    };

    json!({
        "massKg": (mass * 10.0).round() / 10.0,
        "maxSpeedMs": max_speed.round(),
        "maxOverloadG": overload.round(),
        "rangeM": range_m.round(),
        "explosiveMassKg": (exp_mass * 100.0).round() / 100.0,
        "guidanceType": guidance,
        "rangeText": range_text,
        "overloadText": overload_text,
        "massText": mass_text,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cannon_shell_extraction_armorpower_and_lanz_odermatt() {
        let cannon = json!({
            "bullet": {
                "bulletName": "75mm_m72",
                "bulletType": "ap_tank",
                "caliber": 0.075,
                "mass": 6.3,
                "speed": 619.0,
                "damage": {
                    "kinetic": {
                        "demarrePenetrationK": 0.9,
                        "demarreSpeedPow": 1.43,
                        "demarreMassPow": 0.71,
                        "demarreCaliberPow": 1.07
                    }
                }
            },
            "75mm_usa_HE": {
                "bullet": {
                    "bulletName": "75mm_m48",
                    "bulletType": "he_frag_tank",
                    "caliber": 0.075,
                    "mass": 6.3,
                    "speed": 463.0,
                    "explosiveMass": 0.67,
                    "damage": {
                        "kinetic": {
                            "demarrePenetrationK": 0.15,
                            "demarreSpeedPow": 1.43,
                            "demarreMassPow": 0.71,
                            "demarreCaliberPow": 1.07
                        }
                    }
                }
            },
            "120mm_m830_root": {
                "bullet": {
                    "bulletName": "120mm_m830",
                    "bulletType": "heat_fs_tank",
                    "caliber": 0.12,
                    "mass": 13.1,
                    "speed": 1140.0,
                    "armorpower": { "ArmorPower0m": [5.0, 10.0] },
                    "cumulativeDamage": { "armorPower": 480.0 }
                }
            },
            "M829A2": {
                "bullet": {
                    "bulletType": "apfsds_tank",
                    "caliber": 0.12,
                    "mass": 4.9,
                    "speed": 1680.0,
                    "armorpower": {
                        "ArmorPower0m": [629.0, 0.0],
                        "ArmorPower100m": [625.0, 0.0],
                        "ArmorPower500m": [610.0, 0.0],
                        "ArmorPower1000m": [592.0, 0.0],
                        "ArmorPower1500m": [573.0, 0.0],
                        "ArmorPower2000m": [555.0, 0.0]
                    }
                }
            },
            "25mm_M242_APDS": {
                "bullet": {
                    "bulletName": "25mm_m791",
                    "bulletType": "apds_autocannon",
                    "caliber": 0.025,
                    "damageCaliber": 0.0135,
                    "mass": 0.134,
                    "damageMass": 0.134,
                    "speed": 1345.0,
                    "damage": {
                        "kinetic": {
                            "demarrePenetrationK": 0.65,
                            "demarreSpeedPow": 1.43,
                            "demarreMassPow": 0.71,
                            "demarreCaliberPow": 1.07
                        }
                    }
                }
            }
        });
        let shells = extract_shells_from_cannon_blkx(&cannon);

        let stock_m72 = shells
            .iter()
            .find(|s| s.get("id").and_then(|x| x.as_str()) == Some("75mm_m72"))
            .expect("stock root bullet must be extracted");
        assert_eq!(stock_m72["isDefault"].as_bool(), Some(true));
        assert_eq!(stock_m72["code"].as_str(), Some("AP"));
        let m72_pen = stock_m72["pen0"].as_f64().unwrap_or(0.0);
        assert!(
            (89.0..=93.0).contains(&m72_pen),
            "Expected ~91mm for M72 AP, got {}",
            m72_pen
        );

        let m48 = shells
            .iter()
            .find(|s| s["id"] == "75mm_usa_HE")
            .expect("m48 he");
        assert!(
            m48["pen0"].as_f64().unwrap_or(999.0) <= 15.0,
            "HE with demarrePenetrationK=0.15 must not get ~100mm AP pen"
        );

        let m830 = shells
            .iter()
            .find(|s| s["id"] == "120mm_m830_root")
            .expect("m830 heat-fs");
        assert_eq!(m830["pen0"].as_f64(), Some(480.0));
        assert_eq!(m830["pen2000"].as_f64(), Some(480.0));

        let m829 = shells
            .iter()
            .find(|s| s["id"] == "M829A2")
            .expect("m829a2 shell");
        assert_eq!(m829["pen0"].as_f64(), Some(629.0));
        assert_eq!(m829["pen2000"].as_f64(), Some(555.0));
        assert_eq!(m829["velocityMs"].as_f64(), Some(1680.0));

        let m791 = shells
            .iter()
            .find(|s| s["id"] == "25mm_M242_APDS")
            .expect("m791 apds");
        let m791_pen = m791["pen0"].as_f64().unwrap_or(0.0);
        assert!(
            (78.0..=84.0).contains(&m791_pen),
            "Expected ~81mm for 25mm M791 APDS, got {}",
            m791_pen
        );
    }

    #[test]
    fn test_shell_code_classification_accuracy() {
        assert_eq!(classify_shell_code("9m114", "atgm_tank"), "ATGM");
        assert_eq!(classify_shell_code("9m120_ataka", ""), "ATGM");
        assert_eq!(classify_shell_code("152mm_mim146", "atgm_tank"), "ATGM");
        assert_eq!(classify_shell_code("127mm_MIM72C_aam", "sam_tank"), "SAM");
        assert_eq!(classify_shell_code("9m39_igla", ""), "SAM");
        assert_eq!(classify_shell_code("39mm_ap", ""), "AP");
        assert_eq!(classify_shell_code("75mm_usa_APCR", "apcr_tank"), "APCR");
        assert_eq!(classify_shell_code("75mm_hl_gr_38b", "heat_tank"), "HEAT");
        assert_eq!(
            classify_shell_code("120mm_DM_HEAT_FS", "heat_fs_tank"),
            "HEAT-FS"
        );
        assert_eq!(
            classify_shell_code("127mm_usa_mk32_common_sap", "sap_tank"),
            "SAP"
        );
        assert_eq!(
            classify_shell_code("127mm_usa_mk31_aavt_radio_fuse", "he_frag_radio_fuse"),
            "HE-VT"
        );
    }

    #[test]
    fn test_missile_telemetry_and_weaponry_csv_isolated() {
        let aim9m = json!({
            "rocket": {
                "mass": 85.3,
                "endSpeed": 850.0,
                "loadFactorMax": 30.0,
                "rangeMax": 18000.0,
                "explosiveMass": 4.08,
                "guidanceType": "infrared"
            }
        });
        let tel = extract_air_weapon_telemetry(&aim9m);
        assert_eq!(tel["maxOverloadG"].as_f64(), Some(30.0));
        assert_eq!(tel["rangeText"].as_str(), Some("18.0 km"));

        let (classified, is_a2a, _) =
            classify_aircraft_weapon("us_aim_120c_5_x2", "", None, "exp_fighter", Some(&tel));
        assert!(is_a2a);
        assert_eq!(classified["code"].as_str(), Some("ARH"));

        let stinger_opt = WeaponPresetOption {
            icon_type: Some("missile_air_to_air_group_heli".to_string()),
            blk: Some("gamedata/weapons/containers/atas_2x_stinger.blk".to_string()),
            trigger: Some("aam".to_string()),
        };
        let (stinger, s_a2a, _) =
            classify_aircraft_weapon("aim92", "", Some(&stinger_opt), "exp_helicopter", None);
        assert!(s_a2a);
        assert_eq!(stinger["code"].as_str(), Some("STINGER"));
        assert_eq!(stinger["count"].as_i64(), Some(2));

        let mk82_opt = WeaponPresetOption {
            icon_type: Some("bombs_middle_group_x2".to_string()),
            blk: Some("gamedata/weapons/containers/us_bru_33aa_mk82_x2.blk".to_string()),
            trigger: Some("bombs".to_string()),
        };
        let (mk82, _, m_a2g) =
            classify_aircraft_weapon("us_mk82_x2_slot3", "", Some(&mk82_opt), "exp_fighter", None);
        assert!(m_a2g);
        assert_eq!(mk82["shortName"].as_str(), Some("MK82 500 lb"));
        assert_eq!(mk82["count"].as_i64(), Some(2));

        let egbu_opt = WeaponPresetOption {
            icon_type: Some("guided_bomb_heavy".to_string()),
            blk: Some("gamedata/weapons/bombGuns/us_gbu_24b.blk".to_string()),
            trigger: Some("bombs".to_string()),
        };
        let (egbu, _, egbu_a2g) =
            classify_aircraft_weapon("us_egbu_24b", "", Some(&egbu_opt), "exp_fighter", None);
        assert!(egbu_a2g);
        assert_eq!(egbu["code"].as_str(), Some("GBU"));
        assert_eq!(egbu["iconFile"].as_str(), Some("guided_bomb_heavy.png"));
        assert_eq!(
            stinger["iconFile"].as_str(),
            Some("missile_air_to_air_group_heli.png")
        );

        let dir = std::env::temp_dir().join(format!("wt_weaponry_iso_{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let csv_path = dir.join("units_weaponry.csv");
        let csv_data = "\"<ID|readonly|noverify>\";\"<English>\";\"<French>\";\"<Italian>\";\"<German>\";\"<Spanish>\";\"<Russian>\";\"<Polish>\";\"<Czech>\";\"<Turkish>\";\"<Chinese>\";\"<Japanese>\";\"<Portuguese>\"\n\"weapons/120mm_m829a2\";\"M829A2 APFSDS\";\"M829A2 FR\";\"M829A2\";\"M829A2\";\"M829A2 ES\";\"M829A2 RU\";\"x\";\"x\";\"x\";\"M829A2 ZH\";\"M829A2 JA\";\"M829A2 Perf.\"\n\"120mm_m829a2\";\"M829A2 Dup\";\"M829A2 FR\";\"M829A2\";\"M829A2\";\"M829A2 ES\";\"M829A2 RU\";\"x\";\"x\";\"x\";\"M829A2 ZH\";\"M829A2 JA\";\"M829A2 Atualizado\"";
        fs::write(&csv_path, csv_data).unwrap();
        let (in_mem, db_entries) = parse_weaponry_csv(&csv_path);
        assert_eq!(in_mem.len(), 1);
        assert_eq!(db_entries.len(), 1);
        let db = CatalogDatabase::open(&dir.join("test_wp.db")).unwrap();
        db.replace_weaponry_loc(&db_entries).unwrap();
        assert_eq!(
            db.lookup_weaponry_name("120mm_m829a2", "pt-BR").as_deref(),
            Some("M829A2 Atualizado")
        );
        assert_eq!(
            db.lookup_weaponry_name("120mm_m829a2", "ru").as_deref(),
            Some("M829A2 RU")
        );
        assert_eq!(
            db.lookup_weaponry_name("120mm_m829a2", "fr").as_deref(),
            Some("M829A2 FR")
        );
        assert_eq!(
            db.lookup_weaponry_name("120mm_m829a2", "zh").as_deref(),
            Some("M829A2 ZH")
        );
        assert_eq!(
            db.lookup_weaponry_name("120mm_m829a2", "ja").as_deref(),
            Some("M829A2 JA")
        );
        drop(db);
        let _ = fs::remove_dir_all(&dir);
    }
}
