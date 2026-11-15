//! Focused parsers for Ground (`tankModels`), Aviation (`flightModels`),
//! and Naval (`ships`) Datamine `.blkx` models.

use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::core::weapon_catalog::{
    classify_aircraft_weapon, classify_shell_code, extract_air_weapon_telemetry,
    extract_shells_from_cannon_blkx, load_modifications_index, lookup_localized_weapon_name,
    modifications_index_ready, resolve_additive_bullet_mod, WeaponPresetOption,
};

pub fn clean_vehicle_name(raw: &str) -> String {
    let mut cleaned = String::with_capacity(raw.len());
    for ch in raw.chars() {
        if ('\u{E000}'..='\u{F8FF}').contains(&ch) {
            continue;
        }
        if ch == '\u{00A0}' {
            cleaned.push(' ');
        } else {
            cleaned.push(ch);
        }
    }

    let trimmed = cleaned.trim();
    let mut s = trimmed;
    while let Some(ch) = s.chars().next() {
        if (ch == 'o' && s.starts_with("o ")) || (ch == 'O' && s.starts_with("O ")) {
            s = s[2..].trim_start();
        } else if matches!(
            ch,
            '○' | '◊'
                | '◄'
                | '►'
                | '␡'
                | '␢'
                | '▃'
                | '▂'
                | '▄'
                | '▅'
                | '*'
                | '×'
                | '•'
                | ' '
        ) {
            s = s[ch.len_utf8()..].trim_start();
        } else {
            break;
        }
    }
    let normalized = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        trimmed.split_whitespace().collect::<Vec<_>>().join(" ")
    } else {
        normalized
    }
}

pub fn prettify_preset_name(id: &str, vehicle_id: &str) -> String {
    let clean = id.trim_start_matches(vehicle_id).trim_start_matches('_');
    if clean.is_empty() {
        "Standard Loadout".to_string()
    } else {
        clean.replace('_', " ")
    }
}

fn ensure_mods_index_loaded(dm_dir: &Path) {
    if !modifications_index_ready() {
        let mods_path = dm_dir.join("modifications.blkx");
        if mods_path.exists() {
            load_modifications_index(&mods_path);
        }
    }
}

fn is_non_ammo_mod_fallback(mod_key: &str) -> bool {
    let kl = mod_key.to_lowercase();
    kl.contains("tracks")
        || kl.contains("brakes")
        || kl.contains("engine")
        || kl.contains("transmission")
        || kl.contains("suspension")
        || kl.contains("filter")
        || kl.contains("aiming")
        || kl.contains("extinguisher")
        || kl.contains("tool_kit")
        || kl.contains("optics")
        || kl.contains("medical")
        || kl.contains("airstrike")
        || kl.contains("art_support")
        || kl.contains("night_vision")
        || kl.contains("premexpmul")
        || kl.contains("new_wheels")
        || kl.contains("smoke_screen")
        || kl.contains("laser")
        || kl.contains("new_gun")
        || kl.contains("camouflage")
        || kl.contains("protection")
        || kl.contains("ship_hull")
        || kl.contains("ship_rudder")
        || kl.contains("ship_screw")
        || kl.contains("ship_pumps")
        || kl.contains("ship_venting")
        || kl.contains("ship_ammo_wetting")
        || kl.contains("ship_rangefinder")
        || kl.contains("ship_torpedoes")
        || kl.contains("ship_bomb_mortar")
        || kl.contains("caliber_turrets")
        || kl.contains("torpedoes_movement_mode")
        || kl.ends_with("_ammo_pack")
}

fn detect_thermal_from_mods(mods: &serde_json::Value) -> (&'static str, &'static str) {
    let nvd = mods
        .get("heli_night_vision_system")
        .or_else(|| mods.get("night_vision_system"))
        .or_else(|| mods.get("night_vision_system_upgrade_nv_to_thermal"))
        .or_else(|| mods.get("night_vision_system_panther_2"));

    let thermal_obj = nvd
        .and_then(|n| n.get("effects"))
        .and_then(|e| e.get("nightVision"))
        .and_then(|nv| {
            nv.get("sightThermal")
                .or_else(|| nv.get("gunnerThermal"))
                .or_else(|| nv.get("commanderViewThermal"))
                .or_else(|| nv.get("fpvShellThermal"))
        });

    if let Some(th) = thermal_obj {
        let res_x = th
            .get("resolution")
            .and_then(|r| r.as_array())
            .and_then(|a| a.first())
            .and_then(|v| v.as_i64())
            .unwrap_or(500);
        if res_x >= 1000 {
            ("gen3", "Gen 3 Thermal / CITV")
        } else if res_x >= 700 {
            ("gen2", "Gen 2 Thermal")
        } else {
            ("gen1", "Gen 1 Thermal")
        }
    } else if nvd.is_some() {
        ("nvd", "Night Vision (NVD)")
    } else {
        ("day", "Day Sight Only")
    }
}

pub fn is_rwr_sensor_stem(stem_low: &str) -> bool {
    (stem_low.contains("rwr")
        || stem_low.contains("spo")
        || stem_low.contains("alr")
        || stem_low.contains("apr")
        || stem_low.contains("elt_")
        || stem_low.contains("ralm")
        || stem_low.contains("kj_")
        || stem_low.contains("ari_"))
        && !stem_low.contains("radar")
        && !stem_low.contains("apg")
        && !stem_low.contains("aps")
}

pub fn is_radar_sensor_entry(dm_dir: &Path, sensor_val: &serde_json::Value) -> bool {
    let blk = sensor_val.get("blk").and_then(|b| b.as_str()).unwrap_or("");
    let blk_low = blk.to_lowercase().replace('\\', "/");
    let stem_low = Path::new(&blk_low)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    if !stem_low.is_empty() {
        let local_sensor = dm_dir.join("sensors").join(format!("{}.blkx", stem_low));
        if local_sensor.exists() {
            if let Ok(txt) = fs::read_to_string(&local_sensor) {
                if let Ok(sj) = serde_json::from_str::<serde_json::Value>(&txt) {
                    if let Some(st) = sj.get("type").and_then(|t| t.as_str()) {
                        return st.eq_ignore_ascii_case("radar");
                    }
                }
            }
        }
    }

    if stem_low.is_empty() {
        return false;
    }

    let non_radar_tokens = [
        "ir_tracker",
        "flir",
        "laser",
        "tgp",
        "targeting",
        "chaparral_track",
        "rwr",
        "spo",
        "alr",
        "apr",
        "avr",
        "aar",
        "lws",
        "maw",
        "elt_",
        "ralm",
        "kj_",
        "esm",
        "harm",
        "ari_",
    ];
    if non_radar_tokens.iter().any(|t| stem_low.contains(t)) {
        return false;
    }

    if sensor_val.get("radarParkingAzimuth").is_some() {
        return true;
    }

    let has_antenna_dm = sensor_val
        .get("dmPart")
        .map(|dp| {
            if let Some(s) = dp.as_str() {
                s.to_lowercase().contains("antenna") || s.to_lowercase().contains("radar")
            } else if let Some(arr) = dp.as_array() {
                arr.iter().any(|x| {
                    x.as_str()
                        .map(|s| {
                            s.to_lowercase().contains("antenna")
                                || s.to_lowercase().contains("radar")
                        })
                        .unwrap_or(false)
                })
            } else {
                false
            }
        })
        .unwrap_or(false);
    if has_antenna_dm {
        return true;
    }

    let radar_tokens = [
        "radar", "apg", "aps", "apq", "awg", "an_asg", "n0", "rbe", "rdi", "rdy", "cyr", "ps_",
        "zhuk", "sapfir", "sapphire", "rp_", "trml", "giraffe", "mpq", "el_m", "elm_", "grifo",
        "captor", "klj", "bars", "irbis", "zaslon", "kopyo", "search",
    ];
    radar_tokens.iter().any(|t| stem_low.contains(t))
}

fn compute_tank_gear_speed_kmh(tank: &serde_json::Value) -> Option<f64> {
    let vp = tank.get("VehiclePhys")?;
    let max_rpm = vp
        .get("engine")
        .and_then(|e| e.get("maxRPM"))
        .and_then(|v| v.as_f64())?;
    let mech = vp.get("mechanics")?;
    let radius = mech.get("driveGearRadius").and_then(|v| v.as_f64())?;
    let main_r = mech
        .get("mainGearRatio")
        .and_then(|v| v.as_f64())
        .unwrap_or(1.0);
    let side_r = mech
        .get("sideGearRatio")
        .and_then(|v| v.as_f64())
        .unwrap_or(1.0);
    let ratios = mech
        .get("gearRatios")
        .and_then(|g| g.get("ratio"))
        .and_then(|r| r.as_array())?;
    let min_pos_gear = ratios
        .iter()
        .filter_map(|v| v.as_f64())
        .filter(|&r| r > 0.05)
        .fold(f64::INFINITY, f64::min);

    if max_rpm > 0.0 && radius > 0.0 && main_r > 0.0 && side_r > 0.0 && min_pos_gear.is_finite() {
        let total_ratio = min_pos_gear * main_r * side_r;
        let v_kmh = (max_rpm / 60.0 / total_ratio) * (2.0 * std::f64::consts::PI * radius) * 3.6;
        if v_kmh > 3.0 && v_kmh < 150.0 {
            return Some((v_kmh * 10.0).round() / 10.0);
        }
    }
    None
}

fn format_reload_from_cannon_json(cj: &serde_json::Value) -> Option<String> {
    let sf = cj
        .get("shotFreq")
        .and_then(|v| v.as_f64())
        .filter(|&x| x > 0.0);
    let rt = cj
        .get("reloadTime")
        .and_then(|v| v.as_f64())
        .filter(|&x| x > 0.0);

    match (sf, rt) {
        (Some(s), Some(r)) if s <= 1.5 && r > (1.0 / s) * 2.0 => Some(format!("{:.1}s", r)),
        (Some(s), _) if s >= 4.0 => Some(format!("{:.1}s ({:.0} rpm)", 1.0 / s, s * 60.0)),
        (Some(s), _) => Some(format!("{:.1}s", 1.0 / s)),
        (None, Some(r)) => Some(format!("{:.1}s", r)),
        (None, None) => None,
    }
}

fn build_vehicle_belts_from_cannons(
    allowed_mods: &[String],
    cannon_files: &[PathBuf],
) -> Vec<serde_json::Value> {
    let mut primary_stock: Option<serde_json::Value> = None;
    let mut extra_stock: Vec<serde_json::Value> = Vec::new();
    let mut mod_shells_by_id: HashMap<String, serde_json::Value> = HashMap::new();
    let mut loaded_any_cannon = false;

    for (c_idx, cannon_file) in cannon_files.iter().enumerate() {
        if !cannon_file.exists() {
            continue;
        }
        if let Ok(c) = fs::read_to_string(cannon_file) {
            if let Ok(cj) = serde_json::from_str::<serde_json::Value>(&c) {
                loaded_any_cannon = true;
                for sh in extract_shells_from_cannon_blkx(&cj) {
                    let is_def = sh
                        .get("isDefault")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    if is_def {
                        if c_idx == 0 && primary_stock.is_none() {
                            primary_stock = Some(sh);
                        } else if c_idx > 0 {
                            extra_stock.push(sh);
                        }
                    } else if let Some(id) = sh.get("id").and_then(|i| i.as_str()) {
                        mod_shells_by_id.entry(id.to_lowercase()).or_insert(sh);
                    }
                }
            }
        }
    }

    let mut belts = Vec::new();
    let mut reserve_bullet_names: HashSet<String> = HashSet::new();
    let mut seen_belt_ids: HashSet<String> = HashSet::new();
    let has_mods_index = modifications_index_ready();

    for raw_mod_id in allowed_mods {
        let mod_low = raw_mod_id.to_lowercase();
        if mod_low.ends_with("_ammo_pack") {
            continue;
        }
        let meta_opt = resolve_additive_bullet_mod(raw_mod_id);
        let (bullet_mod_key, is_reserve) = if has_mods_index {
            match meta_opt {
                Some(meta) => match meta.additive_bullet_mod {
                    Some(abm) => (abm, meta.is_reserve),
                    None => continue,
                },
                None => {
                    if mod_shells_by_id.contains_key(&mod_low)
                        && !is_non_ammo_mod_fallback(&mod_low)
                    {
                        (raw_mod_id.clone(), false)
                    } else {
                        continue;
                    }
                }
            }
        } else if !is_non_ammo_mod_fallback(&mod_low) {
            (raw_mod_id.clone(), false)
        } else {
            continue;
        };

        if !seen_belt_ids.insert(bullet_mod_key.to_lowercase()) {
            continue;
        }

        let ballistics = mod_shells_by_id
            .get(&bullet_mod_key.to_lowercase())
            .cloned();
        if loaded_any_cannon && ballistics.is_none() {
            // Do not mix secondary turret modifications (e.g. 40mm Bofors) into main battery belts
            continue;
        }
        if is_reserve {
            if let Some(bn) = ballistics
                .as_ref()
                .and_then(|b| b.get("bulletName"))
                .and_then(|v| v.as_str())
            {
                reserve_bullet_names.insert(bn.to_lowercase());
            }
        }

        let code = ballistics
            .as_ref()
            .and_then(|b| b.get("code"))
            .and_then(|c| c.as_str())
            .unwrap_or_else(|| classify_shell_code(&bullet_mod_key, ""));
        let human_name = ballistics
            .as_ref()
            .and_then(|b| b.get("name"))
            .and_then(|n| n.as_str())
            .map(|s| s.to_string())
            .or_else(|| lookup_localized_weapon_name(&bullet_mod_key, "en-US"))
            .unwrap_or_else(|| bullet_mod_key.replace('_', " "));

        belts.push(json!({
            "id": bullet_mod_key,
            "name": human_name.trim(),
            "code": code,
            "type": "shell",
            "ballistics": ballistics.unwrap_or(serde_json::Value::Null),
        }));
    }

    if let Some(stock) = primary_stock {
        let hide_default_gui = stock
            .get("notUseDefaultInGui")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
            && !belts.is_empty();
        let stock_bn = stock
            .get("bulletName")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_lowercase();
        if !hide_default_gui && (stock_bn.is_empty() || !reserve_bullet_names.contains(&stock_bn)) {
            let name = stock
                .get("name")
                .and_then(|s| s.as_str())
                .unwrap_or("Standard");
            let code = stock
                .get("code")
                .and_then(|s| s.as_str())
                .unwrap_or("SHELL");
            belts.insert(
                0,
                json!({
                    "id": "",
                    "name": format!("{} (Default)", name),
                    "code": code,
                    "type": "shell",
                    "ballistics": stock,
                }),
            );
        }
    }

    for extra in extra_stock {
        let ex_id = extra.get("id").and_then(|s| s.as_str()).unwrap_or("");
        let ex_bn = extra
            .get("bulletName")
            .and_then(|s| s.as_str())
            .unwrap_or(ex_id);
        let dup = belts.iter().any(|b| {
            b.get("ballistics")
                .and_then(|x| x.get("bulletName"))
                .and_then(|v| v.as_str())
                .map(|s| s.eq_ignore_ascii_case(ex_bn))
                .unwrap_or(false)
        });
        if !dup && !ex_id.is_empty() {
            let name = extra.get("name").and_then(|s| s.as_str()).unwrap_or(ex_id);
            let code = extra
                .get("code")
                .and_then(|s| s.as_str())
                .unwrap_or("SHELL");
            belts.push(json!({
                "id": ex_id,
                "name": name,
                "code": code,
                "type": "shell",
                "ballistics": extra,
            }));
        }
    }

    belts
}

fn belts_to_ammo_slots(belts: &[serde_json::Value]) -> Vec<serde_json::Value> {
    belts
        .iter()
        .take(4)
        .enumerate()
        .map(|(idx, b)| {
            let code = b.get("code").and_then(|c| c.as_str()).unwrap_or("SHELL");
            let name = b.get("name").and_then(|n| n.as_str()).unwrap_or("Shell");
            let bal = b.get("ballistics");
            let pen0 = bal.and_then(|x| x.get("pen0")).and_then(|v| v.as_f64()).unwrap_or(0.0);
            let vel = bal.and_then(|x| x.get("velocityMs")).and_then(|v| v.as_f64()).unwrap_or(0.0);
            let mass = bal.and_then(|x| x.get("massKg")).and_then(|v| v.as_f64()).unwrap_or(0.0);
            let icon = match code {
                "SAM" | "ATGM" => "rocket_tank.png",
                "APFSDS" | "APDS" | "APCR" | "AP" | "APHE" | "APCBC" | "SAP" => "apdsfs_tank.png",
                "HEAT-FS" | "HEAT" => "heat_fs_tank.png",
                "SMK" => "smoke_tank.png",
                _ => "he_frag_tank.png",
            };
            json!({
                "slot": idx + 1,
                "id": b.get("id").and_then(|v| v.as_str()).unwrap_or(""),
                "code": code,
                "name": name,
                "type": code.to_lowercase(),
                "range": if pen0 > 0.0 { format!("{} mm @ 0m", pen0.round()) } else { "Direct Fire".to_string() },
                "overload": if vel > 0.0 { format!("{} m/s", vel.round()) } else { code.to_string() },
                "mass": if mass > 0.0 { format!("{:.1} kg", mass) } else { "—".to_string() },
                "icon": icon,
                "ballistics": bal.cloned().unwrap_or(serde_json::Value::Null),
            })
        })
        .collect()
}

pub fn build_tank_detail(
    vehicle_id: &str,
    card: Option<&serde_json::Value>,
    cfg: &crate::config::AppConfig,
    dm_dir: &Path,
) -> serde_json::Value {
    ensure_mods_index_loaded(dm_dir);
    let raw_name = card
        .and_then(|c| c.get("name"))
        .and_then(|n| n.as_str())
        .unwrap_or(vehicle_id);
    let name = clean_vehicle_name(raw_name);
    let nation = card
        .and_then(|c| c.get("nation"))
        .and_then(|n| n.as_str())
        .unwrap_or("USA");
    let rank = card
        .and_then(|c| c.get("rank"))
        .and_then(|r| r.as_i64())
        .unwrap_or(1);
    let br = card
        .and_then(|c| c.get("br"))
        .and_then(|b| b.as_str())
        .unwrap_or("1.0");
    let cat = card
        .and_then(|c| c.get("cat"))
        .and_then(|ct| ct.as_str())
        .unwrap_or("TANKS");
    let exp_class = card
        .and_then(|c| c.get("unitClass"))
        .and_then(|u| u.as_str())
        .unwrap_or("exp_tank");

    let tm_file = resolve_model_path(dm_dir, &cfg.tankmodels_dir, "tankmodels", vehicle_id);

    let mut gun_name = "Main Cannon".to_string();
    let mut gun_blk_rel = String::new();
    let mut extra_gun_blk_rels: Vec<String> = Vec::new();
    let mut reload_sec = "—".to_string();
    let mut speed_yaw = "—".to_string();
    let mut ammo_count = 40i64;
    let mut thermal_code = "day".to_string();
    let mut thermal_label = "Day Sight Only".to_string();
    let mut radar_code = "none".to_string();
    let mut radar_label = "No Radar".to_string();
    let mut smoke_count = 0i64;
    let mut has_ess = false;
    let mut has_lrf = false;
    let mut has_stabilizer = false;
    let mut max_fwd_speed = 0.0f64;
    let mut mass_tons = 0.0f64;
    let mut engine_hp = 0.0f64;
    let mut crew_count = 0usize;

    let mut detected_exp_class = None;
    let mut tank_presets = Vec::new();
    let mut model_mod_keys: Option<Vec<String>> = None;

    if tm_file.exists() {
        if let Ok(c) = fs::read_to_string(&tm_file) {
            if let Ok(tank) = serde_json::from_str::<serde_json::Value>(&c) {
                if let Some(ec) = tank.get("expClass").and_then(|e| e.as_str()) {
                    detected_exp_class = Some(ec.to_string());
                }
                max_fwd_speed = compute_tank_gear_speed_kmh(&tank)
                    .or_else(|| {
                        card.and_then(|c| c.get("maxSpeedKmh"))
                            .and_then(|v| v.as_f64())
                    })
                    .or_else(|| tank.get("maxFwdSpeed").and_then(|v| v.as_f64()))
                    .unwrap_or(0.0);
                let mass_kg = tank
                    .get("VehiclePhys")
                    .and_then(|vp| vp.get("Mass"))
                    .and_then(|m| m.get("TakeOff").or_else(|| m.get("Empty")))
                    .and_then(|v| v.as_f64())
                    .filter(|&x| x > 0.0)
                    .or_else(|| tank.get("mass").and_then(|v| v.as_f64()))
                    .unwrap_or(0.0);
                if mass_kg > 0.0 {
                    mass_tons = (mass_kg / 100.0).round() / 10.0;
                }
                engine_hp = tank
                    .get("VehiclePhys")
                    .and_then(|vp| vp.get("engine"))
                    .and_then(|e| e.get("horsePowers"))
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);

                if let Some(crew_obj) = tank.get("tank_crew").and_then(|c| c.as_object()) {
                    crew_count = crew_obj.keys().filter(|k| *k != "changeTimeMult").count();
                }

                // Parse presets
                if let Some(wp) = tank.get("weapon_presets").and_then(|w| w.get("preset")) {
                    let p_list = if wp.is_array() {
                        wp.as_array().cloned().unwrap_or_default()
                    } else {
                        vec![wp.clone()]
                    };
                    for (p_idx, preset_obj) in p_list.iter().enumerate() {
                        if let Some(pname) = preset_obj.get("name").and_then(|n| n.as_str()) {
                            let pblk = preset_obj.get("blk").and_then(|b| b.as_str()).unwrap_or("");
                            let eff_id = if pname == "tank_weapon_preset_default"
                                || pname == format!("{}_default", vehicle_id)
                            {
                                ""
                            } else {
                                pname
                            };
                            tank_presets.push(json!({
                                "id": eff_id,
                                "name": pname.replace('_', " "),
                                "name_key": if p_idx == 0 { "config.standardCombatLoadout" } else { "" },
                                "group": "To hit ground targets",
                                "is_default": p_idx == 0,
                                "blk": pblk,
                                "description": pname,
                                "description_key": "ammo.roundsDesc",
                                "cells": {}
                            }));
                        }
                    }
                }

                // Parse primary cannon, secondary ATGM/SAM launchers, turret speed, stabilizer, smoke
                if let Some(cw) = tank.get("commonWeapons").and_then(|w| w.get("Weapon")) {
                    let weapons = if cw.is_array() {
                        cw.as_array().cloned().unwrap_or_default()
                    } else {
                        vec![cw.clone()]
                    };
                    let primary_opt = weapons
                        .iter()
                        .find(|w| {
                            let is_dummy =
                                w.get("dummy").and_then(|d| d.as_bool()).unwrap_or(false)
                                    || w.get("blk")
                                        .and_then(|b| b.as_str())
                                        .map(|s| s.contains("dummy"))
                                        .unwrap_or(false);
                            if is_dummy {
                                return false;
                            }
                            let tr = w.get("trigger").and_then(|t| t.as_str()).unwrap_or("");
                            let blk = w
                                .get("blk")
                                .and_then(|b| b.as_str())
                                .unwrap_or("")
                                .to_lowercase();
                            tr.starts_with("gunner0")
                                || tr == "main_gun"
                                || blk.contains("cannon")
                                || blk.contains("rocket")
                                || blk.contains("missile")
                                || blk.contains("aam")
                        })
                        .or_else(|| {
                            weapons.iter().find(|w| {
                                !w.get("dummy").and_then(|d| d.as_bool()).unwrap_or(false)
                            })
                        });

                    if let Some(primary) = primary_opt {
                        if let Some(blk) = primary.get("blk").and_then(|b| b.as_str()) {
                            gun_blk_rel = blk.to_string();
                            let stem = Path::new(blk)
                                .file_stem()
                                .and_then(|s| s.to_str())
                                .unwrap_or("Cannon");
                            gun_name =
                                lookup_localized_weapon_name(stem, "en-US").unwrap_or_else(|| {
                                    stem.replace("_user_cannon", "")
                                        .replace("_cannon", "")
                                        .replace('_', " ")
                                });
                        }
                        if let Some(sy) = primary.get("speedYaw").and_then(|s| s.as_f64()) {
                            speed_yaw = format!("{:.1}°/s", sy);
                        }
                        if let Some(sf) = primary.get("shotFreq").and_then(|s| s.as_f64()) {
                            if sf > 0.0 {
                                reload_sec = format!("{:.1}s", 1.0 / sf);
                            }
                        }
                        if let Some(b) = primary.get("bullets").and_then(|b| b.as_i64()) {
                            ammo_count = b;
                        }
                        has_stabilizer = primary.get("gunStabilizer").is_some();
                    }

                    // Collect distinct secondary non-MG/non-smoke ground weapons (e.g. ADATS 152mm MIM-146, BMP ATGM)
                    for w in &weapons {
                        let is_dummy = w.get("dummy").and_then(|d| d.as_bool()).unwrap_or(false);
                        if is_dummy {
                            continue;
                        }
                        let blk = w.get("blk").and_then(|b| b.as_str()).unwrap_or("");
                        let blk_low = blk.to_lowercase();
                        let tg = w
                            .get("triggerGroup")
                            .and_then(|t| t.as_str())
                            .unwrap_or("")
                            .to_lowercase();
                        if blk.is_empty()
                            || blk == gun_blk_rel
                            || blk_low.contains("machinegun")
                            || blk_low.contains("smoke")
                            || tg == "coaxial"
                            || tg == "machinegun"
                            || tg.contains("smoke")
                        {
                            continue;
                        }
                        if blk_low.contains("groundmodels_weapons")
                            && !extra_gun_blk_rels.contains(&blk.to_string())
                        {
                            extra_gun_blk_rels.push(blk.to_string());
                        }
                    }

                    if let Some(sw) = weapons.iter().find(|w| {
                        let tg = w
                            .get("triggerGroup")
                            .and_then(|t| t.as_str())
                            .unwrap_or("")
                            .to_lowercase();
                        let blk = w
                            .get("blk")
                            .and_then(|b| b.as_str())
                            .unwrap_or("")
                            .to_lowercase();
                        tg.contains("smoke") || blk.contains("smoke")
                    }) {
                        if let Some(b) = sw.get("bullets").and_then(|b| b.as_i64()) {
                            smoke_count = b;
                        }
                    }
                }

                // Modifications: thermals (Gen 1/2/3 via resolution), ESS, Laser Rangefinder, and exact vehicle ammo mods
                if let Some(mods) = tank.get("modifications") {
                    let (t_code, t_label) = detect_thermal_from_mods(mods);
                    thermal_code = t_code.to_string();
                    thermal_label = t_label.to_string();
                    if let Some(mods_obj) = mods.as_object() {
                        model_mod_keys = Some(mods_obj.keys().cloned().collect());
                        has_ess = mods_obj
                            .keys()
                            .any(|k| k.to_lowercase().contains("smoke_screen_system"));
                        has_lrf = mods_obj
                            .keys()
                            .any(|k| k.to_lowercase().contains("laser_rangefinder"));
                    }
                }

                // Sensors / Radar (filter out IRST / optical / laser designators like Chaparral's us_chaparral_track)
                let has_genuine_radar = tank
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
                let r_deploy = tank
                    .get("radarDeployTime")
                    .and_then(|r| r.as_f64())
                    .unwrap_or(0.0);
                if has_genuine_radar || r_deploy > 0.0 {
                    radar_code = "radar360".to_string();
                    radar_label = "360° Search & Track Radar".to_string();
                }
            }
        }
    }

    let vid_low = vehicle_id.to_lowercase();
    let is_fcs =
        vid_low.ends_with("_fcs") || vid_low.contains("_radar") || vid_low.contains("_director");
    let launcher_counterpart = if is_fcs {
        Some(format!(
            "{}_launcher",
            vid_low.strip_suffix("_fcs").unwrap_or(&vid_low)
        ))
    } else {
        None
    };

    let mut launcher_ammo_found = false;
    if let Some(ref l_id) = launcher_counterpart {
        let l_file = resolve_model_path(dm_dir, &cfg.tankmodels_dir, "tankmodels", l_id);
        if l_file.exists() {
            if let Ok(c) = fs::read_to_string(&l_file) {
                if let Ok(l_tank) = serde_json::from_str::<serde_json::Value>(&c) {
                    if let Some(cw) = l_tank.get("commonWeapons").and_then(|w| w.get("Weapon")) {
                        let weapons = if cw.is_array() {
                            cw.as_array().cloned().unwrap_or_default()
                        } else {
                            vec![cw.clone()]
                        };
                        let total_bullets: i64 = weapons
                            .iter()
                            .filter_map(|w| w.get("bullets").and_then(|b| b.as_i64()))
                            .sum();
                        if total_bullets > 0 {
                            ammo_count = total_bullets;
                            launcher_ammo_found = true;
                        }
                    }
                }
            }
        }
    }
    if is_fcs && !launcher_ammo_found {
        ammo_count = 0;
    }

    let mut cannon_files: Vec<PathBuf> = Vec::new();
    for rel in std::iter::once(&gun_blk_rel).chain(extra_gun_blk_rels.iter()) {
        if !rel.is_empty() {
            let stem = Path::new(rel)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_lowercase();
            if !stem.is_empty() {
                cannon_files.push(
                    dm_dir
                        .join("weapons/groundmodels_weapons")
                        .join(format!("{}.blkx", stem)),
                );
            }
        }
    }

    if reload_sec == "—" && !is_fcs {
        if let Some(primary_cannon) = cannon_files.first() {
            if primary_cannon.exists() {
                if let Ok(cc) = fs::read_to_string(primary_cannon) {
                    if let Ok(cj) = serde_json::from_str::<serde_json::Value>(&cc) {
                        if let Some(formatted) = format_reload_from_cannon_json(&cj) {
                            reload_sec = formatted;
                        }
                    }
                }
            }
        }
    }

    let allowed_mods: Vec<String> = model_mod_keys.unwrap_or_else(|| {
        card.and_then(|c| c.get("ammoMods"))
            .and_then(|a| a.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default()
    });

    let mut belts = if is_fcs {
        Vec::new()
    } else {
        build_vehicle_belts_from_cannons(&allowed_mods, &cannon_files)
    };
    if belts.is_empty() && !is_fcs {
        belts.push(json!({ "id": "", "name": "Standard Combat Loadout", "code": "STD" }));
    }

    let ammo_slots = if is_fcs {
        vec![json!({
            "slot": 1, "code": "SAM", "name": "Surface-to-Air Missile", "name_key": "ammo.sam_bvr",
            "type": "sam", "range": "Active / SARH Radar", "overload": "SAM", "mass": "—", "icon": "rocket_tank.png"
        })]
    } else {
        belts_to_ammo_slots(&belts)
    };

    if tank_presets.is_empty() {
        tank_presets.push(json!({
            "id": "",
            "name": "Standard Combat Loadout",
            "name_key": "config.standardCombatLoadout",
            "group": "To hit ground targets",
            "is_default": true,
            "blk": "",
            "description": "Standard Combat Loadout",
            "description_key": "ammo.roundsDesc",
            "cells": {}
        }));
    } else {
        for p in &mut tank_presets {
            if let Some(pid) = p.get("id").and_then(|i| i.as_str()) {
                if pid == "tank_weapon_preset_default" {
                    if let Some(obj) = p.as_object_mut() {
                        obj.insert("id".to_string(), json!(""));
                    }
                }
            }
        }
    }

    let final_exp_class = detected_exp_class.unwrap_or_else(|| exp_class.to_string());
    let (smoke_label, eff_smoke_code) = if smoke_count > 0 && has_ess {
        (
            format!("{}x Launchers + ESS", smoke_count),
            "smoke_launchers",
        )
    } else if smoke_count > 0 {
        (format!("{}x Launchers", smoke_count), "smoke_launchers")
    } else if has_ess {
        ("Engine Smoke Screen (ESS)".to_string(), "ess")
    } else {
        ("No Smoke Screen".to_string(), "none")
    };

    let power_to_weight = if mass_tons > 0.0 && engine_hp > 0.0 {
        ((engine_hp / mass_tons) * 10.0).round() / 10.0
    } else {
        0.0
    };

    json!({
        "id": vehicle_id,
        "name": name,
        "nation": nation,
        "branch": "army",
        "rank": rank,
        "br": br,
        "cat": cat,
        "expClass": final_exp_class,
        "is_fcs": is_fcs,
        "gun": gun_name,
        "gunBlk": gun_blk_rel,
        "gun_code": "cannon",
        "reload": reload_sec,
        "speedYaw": speed_yaw,
        "ammoCount": ammo_count,
        "thermal": thermal_label,
        "thermal_code": thermal_code,
        "radar": radar_label,
        "radar_code": radar_code,
        "smoke": smoke_label,
        "smoke_code": eff_smoke_code,
        "smoke_count": smoke_count,
        "has_ess": has_ess,
        "has_lrf": has_lrf,
        "has_stabilizer": has_stabilizer,
        "maxSpeedKmh": max_fwd_speed.round(),
        "massTons": mass_tons,
        "engineHp": engine_hp.round(),
        "powerToWeight": power_to_weight,
        "crewCount": crew_count,
        "customizable": false,
        "pylons": if is_fcs { 0 } else { ammo_slots.len() },
        "presets": tank_presets,
        "ammo_slots": ammo_slots,
        "belts": belts,
        "launcher_counterpart": launcher_counterpart
    })
}

pub fn build_aircraft_detail(
    vehicle_id: &str,
    card: Option<&serde_json::Value>,
    cfg: &crate::config::AppConfig,
    dm_dir: &Path,
) -> serde_json::Value {
    let raw_name = card
        .and_then(|c| c.get("name"))
        .and_then(|n| n.as_str())
        .unwrap_or(vehicle_id);
    let name = clean_vehicle_name(raw_name);
    let nation = card
        .and_then(|c| c.get("nation"))
        .and_then(|n| n.as_str())
        .unwrap_or("USA");
    let rank = card
        .and_then(|c| c.get("rank"))
        .and_then(|r| r.as_i64())
        .unwrap_or(1);
    let br = card
        .and_then(|c| c.get("br"))
        .and_then(|b| b.as_str())
        .unwrap_or("1.0");
    let cat = card
        .and_then(|c| c.get("cat"))
        .and_then(|ct| ct.as_str())
        .unwrap_or("FIGHTERS");
    let exp_class = card
        .and_then(|c| c.get("unitClass"))
        .and_then(|u| u.as_str())
        .unwrap_or("exp_fighter");

    let fm_file = resolve_model_path(dm_dir, &cfg.flightmodels_dir, "flightmodels", vehicle_id);
    let mut radar: Option<String> = None;
    let mut rwr: Option<String> = None;
    let mut hmd = false;
    let mut ccip = false;
    let mut thermal_code = "day".to_string();
    let mut thermal_label = "Day Sight Only".to_string();
    let mut gun = "Internal Gun".to_string();
    let mut gun_ammo_max = 0i64;
    let mut has_guns = false;
    let mut cm_count = 0i64;
    let mut cm_label = "None".to_string();
    let mut presets = Vec::new();
    let mut pylons_out: Vec<serde_json::Value> = Vec::new();
    let mut slot_to_pylon: HashMap<i64, usize> = HashMap::new();
    let max_speed_kmh = card
        .and_then(|c| c.get("maxSpeedKmh"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    if fm_file.exists() {
        if let Ok(c) = fs::read_to_string(&fm_file) {
            if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(&c) {
                if let Some(mods) = json_val.get("modifications") {
                    let (t_code, t_label) = detect_thermal_from_mods(mods);
                    thermal_code = t_code.to_string();
                    thermal_label = t_label.to_string();
                }

                let mut sensor_radar: Option<String> = None;
                if let Some(sn) = json_val.get("sensors").and_then(|s| s.get("sensor")) {
                    let sn_list = if sn.is_array() {
                        sn.as_array().cloned().unwrap_or_default()
                    } else {
                        vec![sn.clone()]
                    };
                    for s in sn_list {
                        if let Some(blk) = s.get("blk").and_then(|b| b.as_str()) {
                            let stem = Path::new(blk)
                                .file_stem()
                                .and_then(|x| x.to_str())
                                .unwrap_or("");
                            let stem_low = stem.to_lowercase();
                            if is_radar_sensor_entry(dm_dir, &s) && sensor_radar.is_none() {
                                sensor_radar = Some(stem.to_uppercase().replace('_', "-"));
                            }
                            if is_rwr_sensor_stem(&stem_low) && rwr.is_none() {
                                rwr = Some(stem.to_uppercase().replace('_', "-"));
                            }
                        }
                    }
                }
                if let Some(r) = json_val.get("radarName").and_then(|r| r.as_str()) {
                    radar = Some(r.to_string());
                } else if let Some(r) = json_val
                    .get("radar")
                    .and_then(|r| r.get("name"))
                    .and_then(|n| n.as_str())
                {
                    radar = Some(r.to_string());
                } else {
                    radar = sensor_radar;
                }

                if rwr.is_none() {
                    if let Some(r) = json_val.get("rwrIndicator").and_then(|r| r.as_str()) {
                        rwr = Some(r.to_string());
                    }
                }

                if let Some(obj) = json_val.as_object() {
                    hmd = obj.keys().any(|k| {
                        let kl = k.to_lowercase();
                        (kl.starts_with("hmd")
                            || kl == "mfdcamtadsapache"
                            || kl == "hashelmetdesignator")
                            && obj.get(k).and_then(|v| v.as_bool()).unwrap_or(true)
                    });
                }

                ccip = json_val
                    .get("haveCCIPForRocket")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
                    || json_val
                        .get("haveCCIPForGun")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false)
                    || json_val
                        .get("haveCCIPForBomb")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false)
                    || json_val
                        .get("haveCCIPForBombs")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);

                let mut primary_gun_set = false;
                let mut record_internal_gun = |tr: &str, blk: &str, bullets: i64| {
                    let tr_low = tr.to_lowercase();
                    let blk_low = blk.to_lowercase().replace('\\', "/");
                    if tr_low == "countermeasures"
                        || tr_low == "boosters"
                        || blk_low.contains("countermeasure")
                        || blk_low.contains("payloadguns")
                        || blk_low.contains("booster")
                        || blk_low.contains("droptank")
                        || blk_low.contains("dummy")
                    {
                        if tr_low == "countermeasures" || blk_low.contains("countermeasure") {
                            cm_count += bullets;
                        }
                        return;
                    }
                    let is_primary_trigger = tr_low == "cannon"
                        || tr_low == "machine gun"
                        || tr_low == "machinegun"
                        || tr_low == "additionalgun"
                        || tr_low == "gunner0"
                        || tr_low.contains("cannon");
                    let is_turret_trigger = tr_low.starts_with("gunner");
                    let stem = Path::new(blk)
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("");
                    let stem_low = stem.to_lowercase();
                    let looks_like_gun_file = stem_low.starts_with("cannon")
                        || stem_low.starts_with("gun")
                        || stem_low.contains("_cannon")
                        || stem_low.contains("_gun");

                    if is_primary_trigger || is_turret_trigger || looks_like_gun_file {
                        has_guns = true;
                        gun_ammo_max += bullets;
                        if !stem.is_empty() && (is_primary_trigger || !primary_gun_set) {
                            gun =
                                lookup_localized_weapon_name(stem, "en-US").unwrap_or_else(|| {
                                    stem.replace("cannon", "")
                                        .replace('_', " ")
                                        .trim()
                                        .to_string()
                                });
                            if is_primary_trigger {
                                primary_gun_set = true;
                            }
                        }
                    }
                };

                // Parse commonWeapons for older planes without WeaponSlots[0]
                if let Some(cw) = json_val.get("commonWeapons").and_then(|w| w.get("Weapon")) {
                    let cw_list = if cw.is_array() {
                        cw.as_array().cloned().unwrap_or_default()
                    } else {
                        vec![cw.clone()]
                    };
                    for w in &cw_list {
                        if w.get("dummy").and_then(|d| d.as_bool()).unwrap_or(false) {
                            continue;
                        }
                        let tr = w.get("trigger").and_then(|t| t.as_str()).unwrap_or("");
                        let blk = w.get("blk").and_then(|b| b.as_str()).unwrap_or("");
                        let bullets = w.get("bullets").and_then(|b| b.as_i64()).unwrap_or(0);
                        record_internal_gun(tr, blk, bullets);
                    }
                }

                let mut slot_options: HashMap<(i64, String), WeaponPresetOption> = HashMap::new();
                let mut named_options: HashMap<String, WeaponPresetOption> = HashMap::new();

                // Parse WeaponSlots (slot 0 = internal guns & countermeasures; >=1 = external pylons)
                if let Some(ws) = json_val
                    .get("WeaponSlots")
                    .and_then(|w| w.get("WeaponSlot"))
                {
                    let slots = if ws.is_array() {
                        ws.as_array().cloned().unwrap_or_default()
                    } else {
                        vec![ws.clone()]
                    };

                    for s in &slots {
                        let s_idx = s.get("index").and_then(|i| i.as_i64()).unwrap_or(0);
                        if s_idx == 0 {
                            if let Some(wp) = s.get("WeaponPreset") {
                                let p_list = if wp.is_array() {
                                    wp.as_array().cloned().unwrap_or_default()
                                } else {
                                    vec![wp.clone()]
                                };
                                for p in &p_list {
                                    if let Some(w_raw) = p.get("Weapon") {
                                        let w_arr = if w_raw.is_array() {
                                            w_raw.as_array().cloned().unwrap_or_default()
                                        } else {
                                            vec![w_raw.clone()]
                                        };
                                        for w in &w_arr {
                                            if w.get("dummy")
                                                .and_then(|d| d.as_bool())
                                                .unwrap_or(false)
                                            {
                                                continue;
                                            }
                                            let tr = w
                                                .get("trigger")
                                                .and_then(|t| t.as_str())
                                                .unwrap_or("");
                                            let blk =
                                                w.get("blk").and_then(|b| b.as_str()).unwrap_or("");
                                            let bullets = w
                                                .get("bullets")
                                                .and_then(|b| b.as_i64())
                                                .unwrap_or(0);
                                            record_internal_gun(tr, blk, bullets);
                                        }
                                    }
                                }
                            }
                            continue;
                        }

                        if let Some(wp) = s.get("WeaponPreset") {
                            let p_list = if wp.is_array() {
                                wp.as_array().cloned().unwrap_or_default()
                            } else {
                                vec![wp.clone()]
                            };
                            for p in p_list {
                                let p_name = p
                                    .get("name")
                                    .and_then(|n| n.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                if p_name.is_empty() {
                                    continue;
                                }
                                let icon_type = p
                                    .get("iconType")
                                    .and_then(|i| i.as_str())
                                    .map(|s| s.to_string());
                                let (w_blk, w_trigger) = if let Some(w_obj) = p.get("Weapon") {
                                    let first = if let Some(arr) = w_obj.as_array() {
                                        arr.first()
                                    } else {
                                        Some(w_obj)
                                    };
                                    (
                                        first
                                            .and_then(|w| w.get("blk"))
                                            .and_then(|b| b.as_str())
                                            .map(|s| s.to_string()),
                                        first
                                            .and_then(|w| w.get("trigger"))
                                            .and_then(|t| t.as_str())
                                            .map(|s| s.to_string()),
                                    )
                                } else {
                                    (None, None)
                                };

                                let opt_info = WeaponPresetOption {
                                    icon_type,
                                    blk: w_blk,
                                    trigger: w_trigger,
                                };
                                slot_options.insert((s_idx, p_name.clone()), opt_info.clone());
                                named_options.insert(p_name, opt_info);
                            }
                        }
                    }

                    let mut ext_slots: Vec<serde_json::Value> = slots
                        .into_iter()
                        .filter(|s| s.get("index").and_then(|i| i.as_i64()).unwrap_or(0) >= 1)
                        .collect();

                    ext_slots.sort_by(|a, b| {
                        let tier_a = a.get("tier").and_then(|t| t.as_i64()).unwrap_or_else(|| {
                            1000 - a.get("index").and_then(|i| i.as_i64()).unwrap_or(0)
                        });
                        let tier_b = b.get("tier").and_then(|t| t.as_i64()).unwrap_or_else(|| {
                            1000 - b.get("index").and_then(|i| i.as_i64()).unwrap_or(0)
                        });
                        let idx_a = a.get("index").and_then(|i| i.as_i64()).unwrap_or(0);
                        let idx_b = b.get("index").and_then(|i| i.as_i64()).unwrap_or(0);
                        tier_b.cmp(&tier_a).then_with(|| idx_a.cmp(&idx_b))
                    });

                    for (p_idx, s) in ext_slots.iter().enumerate() {
                        let slot_idx = s.get("index").and_then(|i| i.as_i64()).unwrap_or(0);
                        let pylon_num = p_idx + 1;
                        slot_to_pylon.insert(slot_idx, pylon_num);

                        // Build selectable custom options for this pylon!
                        let mut pylon_opts = Vec::new();
                        if let Some(wp) = s.get("WeaponPreset") {
                            let p_list = if wp.is_array() {
                                wp.as_array().cloned().unwrap_or_default()
                            } else {
                                vec![wp.clone()]
                            };
                            for p in p_list {
                                let p_name = p.get("name").and_then(|n| n.as_str()).unwrap_or("");
                                if p_name.is_empty() {
                                    continue;
                                }
                                if let Some(opt_meta) =
                                    slot_options.get(&(slot_idx, p_name.to_string()))
                                {
                                    let w_tel = opt_meta
                                        .blk
                                        .as_deref()
                                        .and_then(|b| load_weapon_telemetry(dm_dir, b));
                                    let (opt_json, _, _) = classify_aircraft_weapon(
                                        p_name,
                                        "",
                                        Some(opt_meta),
                                        exp_class,
                                        w_tel.as_ref(),
                                    );
                                    pylon_opts.push(opt_json);
                                }
                            }
                        }

                        pylons_out.push(json!({
                            "pylon": pylon_num,
                            "slot": slot_idx,
                            "tier": s.get("tier"),
                            "options": pylon_opts,
                        }));
                    }
                }

                // Parse Official Weapon Presets
                let wp_dir = if dm_dir.join("flightmodels/weaponpresets").exists() {
                    dm_dir.join("flightmodels/weaponpresets")
                } else {
                    PathBuf::from(&cfg.weaponpresets_dir)
                };

                if let Some(wp) = json_val.get("weapon_presets").and_then(|w| w.get("preset")) {
                    let wp_list = if wp.is_array() {
                        wp.as_array().cloned().unwrap_or_default()
                    } else {
                        vec![wp.clone()]
                    };
                    for p in wp_list {
                        let p_name = p
                            .get("name")
                            .and_then(|n| n.as_str())
                            .unwrap_or("Default")
                            .to_string();
                        let p_blk = p
                            .get("blk")
                            .and_then(|b| b.as_str())
                            .unwrap_or("")
                            .to_string();
                        let stem = Path::new(&p_blk)
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or(&p_name)
                            .to_lowercase();

                        let pr_file = wp_dir.join(format!("{}.blkx", stem));
                        let mut cells = serde_json::Map::new();
                        let mut has_a2a = false;
                        let mut has_a2g = false;
                        let mut title_counts: HashMap<String, usize> = HashMap::new();

                        if pr_file.exists() {
                            if let Ok(pr_content) = fs::read_to_string(&pr_file) {
                                if let Ok(pr_json) =
                                    serde_json::from_str::<serde_json::Value>(&pr_content)
                                {
                                    if let Some(weapons) = pr_json.get("Weapon") {
                                        let w_list = if weapons.is_array() {
                                            weapons.as_array().cloned().unwrap_or_default()
                                        } else {
                                            vec![weapons.clone()]
                                        };
                                        for w in w_list {
                                            if let Some(slot) =
                                                w.get("slot").and_then(|s| s.as_i64())
                                            {
                                                let p_preset = w
                                                    .get("preset")
                                                    .and_then(|pr| pr.as_str())
                                                    .unwrap_or("");
                                                let p_inner_blk = w
                                                    .get("blk")
                                                    .and_then(|b| b.as_str())
                                                    .unwrap_or("");
                                                let p_num = slot_to_pylon
                                                    .get(&slot)
                                                    .copied()
                                                    .unwrap_or(slot as usize);

                                                let opt_info = slot_options
                                                    .get(&(slot, p_preset.to_string()))
                                                    .or_else(|| named_options.get(p_preset));
                                                let blk_rel = opt_info
                                                    .and_then(|o| o.blk.as_deref())
                                                    .unwrap_or(p_inner_blk);
                                                let w_tel = if !blk_rel.is_empty() {
                                                    load_weapon_telemetry(dm_dir, blk_rel)
                                                } else {
                                                    None
                                                };

                                                let (opt, is_air, is_gnd) =
                                                    classify_aircraft_weapon(
                                                        p_preset,
                                                        p_inner_blk,
                                                        opt_info,
                                                        exp_class,
                                                        w_tel.as_ref(),
                                                    );
                                                if is_air {
                                                    has_a2a = true;
                                                }
                                                if is_gnd {
                                                    has_a2g = true;
                                                }

                                                let s_name = opt
                                                    .get("shortName")
                                                    .and_then(|s| s.as_str())
                                                    .unwrap_or("")
                                                    .to_string();
                                                if !s_name.is_empty() {
                                                    *title_counts.entry(s_name).or_insert(0) += 1;
                                                }
                                                cells.insert(p_num.to_string(), opt);
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        let p_name_low = p_name.to_lowercase();
                        let is_default = p_name_low.contains("default") || p_name_low == "empty";
                        let is_really_empty = cells.is_empty()
                            && (is_default || p_name_low == "empty" || p_name_low.contains("none"));
                        let group = if is_really_empty {
                            "Without load".to_string()
                        } else if has_a2a && has_a2g {
                            "Multi-purpose weapon".to_string()
                        } else if has_a2a {
                            "To hit air targets".to_string()
                        } else if has_a2g || exp_class == "exp_helicopter" {
                            "To hit ground targets".to_string()
                        } else {
                            "To hit air targets".to_string()
                        };

                        let display_title = if is_really_empty {
                            "Without load".to_string()
                        } else if !title_counts.is_empty() {
                            let mut parts: Vec<String> = title_counts
                                .into_iter()
                                .map(|(wname, cnt)| {
                                    if wname == "Air Weapon" || wname == "Combat Load" {
                                        prettify_preset_name(&p_name, vehicle_id)
                                    } else {
                                        format!("{}x {}", cnt, wname)
                                    }
                                })
                                .collect();
                            parts.sort();
                            parts.join(", ")
                        } else {
                            prettify_preset_name(&p_name, vehicle_id)
                        };

                        presets.push(json!({
                            "id": p_name,
                            "name": display_title,
                            "group": group,
                            "blk": p_blk,
                            "cells": cells
                        }));
                    }
                }
            }
        }
    }

    if cm_count > 0 {
        cm_label = format!("{}x Flares/Chaff", cm_count);
    }
    if !has_guns {
        gun = "No fixed gun".to_string();
    } else if gun_ammo_max == 0 {
        gun_ammo_max = 300;
    }

    if presets.is_empty() {
        presets.push(json!({
            "id": format!("{}_default", vehicle_id),
            "name": "Standard Loadout",
            "group": "Without load",
            "blk": "",
            "cells": {}
        }));
    }

    let customizable = pylons_out.iter().any(|p| {
        p.get("options")
            .and_then(|o| o.as_array())
            .map(|a| !a.is_empty())
            .unwrap_or(false)
    });

    json!({
        "id": vehicle_id,
        "name": name,
        "nation": nation,
        "rank": rank,
        "br": br,
        "cat": cat,
        "expClass": exp_class,
        "branch": "aviation",
        "customizable": customizable,
        "radar": radar.unwrap_or_else(|| "No Radar".to_string()),
        "rwr": rwr.unwrap_or_else(|| "No RWR".to_string()),
        "hmd": hmd,
        "ccip": ccip,
        "thermal": thermal_label,
        "thermal_code": thermal_code,
        "maxSpeedKmh": max_speed_kmh.round(),
        "gun": gun,
        "cm": cm_label,
        "cmCount": cm_count,
        "gunAmmoMax": gun_ammo_max,
        "hasGuns": has_guns,
        "pylons": pylons_out,
        "presets": presets
    })
}

pub fn build_ship_detail(
    vehicle_id: &str,
    card: Option<&serde_json::Value>,
    cfg: &crate::config::AppConfig,
    dm_dir: &Path,
) -> serde_json::Value {
    ensure_mods_index_loaded(dm_dir);
    let raw_name = card
        .and_then(|c| c.get("name"))
        .and_then(|n| n.as_str())
        .unwrap_or(vehicle_id);
    let name = clean_vehicle_name(raw_name);
    let nation = card
        .and_then(|c| c.get("nation"))
        .and_then(|n| n.as_str())
        .unwrap_or("USA");
    let rank = card
        .and_then(|c| c.get("rank"))
        .and_then(|r| r.as_i64())
        .unwrap_or(1);
    let br = card
        .and_then(|c| c.get("br"))
        .and_then(|b| b.as_str())
        .unwrap_or("1.0");
    let cat = card
        .and_then(|c| c.get("cat"))
        .and_then(|ct| ct.as_str())
        .unwrap_or("BLUEWATER");
    let exp_class = card
        .and_then(|c| c.get("unitClass"))
        .and_then(|u| u.as_str())
        .unwrap_or("exp_destroyer");

    let ship_file = dm_dir
        .join("ships")
        .join(format!("{}.blkx", vehicle_id.to_lowercase()));
    let _ = cfg;
    let mut main_gun = "Naval Battery".to_string();
    let mut gun_blk_rel = String::new();
    let mut max_speed_kmh = card
        .and_then(|c| c.get("maxSpeedKmh"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let mut mass_tons = 0.0f64;
    let mut engine_hp = 0.0f64;
    let mut speed_yaw = "—".to_string();
    let mut reload_sec = "—".to_string();
    let mut crew_count = 0usize;
    let mut ammo_count = 200i64;
    let mut ship_presets = Vec::new();
    let mut model_mod_keys: Option<Vec<String>> = None;

    if ship_file.exists() {
        if let Ok(c) = fs::read_to_string(&ship_file) {
            if let Ok(ship) = serde_json::from_str::<serde_json::Value>(&c) {
                if let Some(ship_phys) = ship.get("ShipPhys") {
                    if let Some(ms) = ship_phys
                        .get("engines")
                        .and_then(|e| e.get("maxSpeed"))
                        .and_then(|v| v.as_f64())
                    {
                        max_speed_kmh = (ms * 3.6 * 10.0).round() / 10.0;
                    }
                    if let Some(hp) = ship_phys
                        .get("engines")
                        .and_then(|e| e.get("horsePowers"))
                        .and_then(|v| v.as_f64())
                    {
                        engine_hp = hp.round();
                    }
                    if let Some(empty_kg) = ship_phys
                        .get("mass")
                        .and_then(|m| m.get("Empty"))
                        .and_then(|v| v.as_f64())
                    {
                        if empty_kg > 0.0 {
                            mass_tons = (empty_kg / 100.0).round() / 10.0;
                        }
                    }
                }
                if max_speed_kmh <= 0.0 {
                    max_speed_kmh = ship
                        .get("maxFwdSpeed")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0);
                }

                if let Some(crew_obj) = ship.get("tank_crew").and_then(|v| v.as_object()) {
                    let sum_crew: i64 = crew_obj
                        .values()
                        .filter_map(|v| v.get("crew").and_then(|c| c.as_i64()))
                        .sum();
                    if sum_crew > 0 {
                        crew_count = sum_crew as usize;
                    } else {
                        crew_count = crew_obj
                            .keys()
                            .filter(|k| !k.starts_with("minCrew"))
                            .count();
                    }
                }

                if let Some(cw) = ship.get("commonWeapons").and_then(|w| w.get("Weapon")) {
                    let weapons = if cw.is_array() {
                        cw.as_array().cloned().unwrap_or_default()
                    } else {
                        vec![cw.clone()]
                    };
                    let primary_opt = weapons
                        .iter()
                        .find(|w| {
                            !w.get("dummy").and_then(|d| d.as_bool()).unwrap_or(false)
                                && w.get("triggerGroup").and_then(|t| t.as_str()) == Some("primary")
                        })
                        .or_else(|| {
                            weapons.iter().find(|w| {
                                !w.get("dummy").and_then(|d| d.as_bool()).unwrap_or(false)
                            })
                        });

                    if let Some(primary) = primary_opt {
                        if let Some(blk) = primary.get("blk").and_then(|b| b.as_str()) {
                            gun_blk_rel = blk.to_string();
                            let stem = Path::new(blk)
                                .file_stem()
                                .and_then(|s| s.to_str())
                                .unwrap_or("Naval Gun");
                            main_gun =
                                lookup_localized_weapon_name(stem, "en-US").unwrap_or_else(|| {
                                    stem.replace("_naval_user_cannon", "").replace('_', " ")
                                });
                        }
                        if let Some(b) = primary.get("bullets").and_then(|b| b.as_i64()) {
                            ammo_count = b;
                        }
                        if let Some(sy) = primary.get("speedYaw").and_then(|s| s.as_f64()) {
                            speed_yaw = format!("{:.1}°/s", sy);
                        }
                        if let Some(sf) = primary.get("shotFreq").and_then(|s| s.as_f64()) {
                            if sf > 0.0 {
                                reload_sec = format!("{:.1}s", 1.0 / sf);
                            }
                        } else if let Some(rt) = primary.get("reloadTime").and_then(|s| s.as_f64())
                        {
                            if rt > 0.0 {
                                reload_sec = format!("{:.1}s", rt);
                            }
                        }
                    }
                }

                if let Some(mods_obj) = ship.get("modifications").and_then(|m| m.as_object()) {
                    model_mod_keys = Some(mods_obj.keys().cloned().collect());
                }

                if let Some(wp) = ship.get("weapon_presets").and_then(|w| w.get("preset")) {
                    let p_list = if wp.is_array() {
                        wp.as_array().cloned().unwrap_or_default()
                    } else {
                        vec![wp.clone()]
                    };
                    for p in p_list {
                        if let Some(pname) = p.get("name").and_then(|n| n.as_str()) {
                            let is_def = pname.ends_with("_default") || pname == "default";
                            ship_presets.push(json!({
                                "id": if is_def { "" } else { pname },
                                "name": if is_def { "Default (Main Battery)".to_string() } else { pname.replace('_', " ") },
                                "group": "To hit ground targets",
                                "is_default": is_def,
                                "blk": p.get("blk").and_then(|b| b.as_str()).unwrap_or(""),
                                "cells": {}
                            }));
                        }
                    }
                }
            }
        }
    }

    let mut cannon_files: Vec<PathBuf> = Vec::new();
    let mut main_caliber_prefix: Option<String> = None;
    if !gun_blk_rel.is_empty() {
        let stem = Path::new(&gun_blk_rel)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();
        if let Some(mm_pos) = stem.find("mm_") {
            main_caliber_prefix = Some(stem[..mm_pos + 3].to_string());
        }
        if !stem.is_empty() {
            let naval_path = dm_dir
                .join("weapons/navalmodels_weapons")
                .join(format!("{}.blkx", stem));
            if reload_sec == "—" && naval_path.exists() {
                if let Ok(nc) = fs::read_to_string(&naval_path) {
                    if let Ok(nj) = serde_json::from_str::<serde_json::Value>(&nc) {
                        if let Some(formatted) = format_reload_from_cannon_json(&nj) {
                            reload_sec = formatted;
                        }
                    }
                }
            }
            cannon_files.push(naval_path);
        }
    }

    let mut allowed_mods: Vec<String> = model_mod_keys.unwrap_or_else(|| {
        card.and_then(|c| c.get("ammoMods"))
            .and_then(|a| a.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default()
    });
    if let Some(ref cal_pfx) = main_caliber_prefix {
        let matched: Vec<String> = allowed_mods
            .iter()
            .filter(|m| m.to_lowercase().starts_with(cal_pfx))
            .cloned()
            .collect();
        if !matched.is_empty() {
            allowed_mods = matched;
        }
    }

    let mut belts = build_vehicle_belts_from_cannons(&allowed_mods, &cannon_files);
    if belts.is_empty() {
        belts.push(json!({ "id": "", "name": main_gun, "code": "HE" }));
    }
    let ammo_slots = belts_to_ammo_slots(&belts);

    if ship_presets.is_empty() {
        ship_presets.push(json!({
            "id": "",
            "name": "Standard Naval Loadout",
            "group": "To hit ground targets",
            "blk": "",
            "cells": {}
        }));
    } else {
        ship_presets.sort_by_key(|p| {
            if p.get("is_default")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                0
            } else {
                1
            }
        });
    }

    json!({
        "id": vehicle_id,
        "name": name,
        "nation": nation,
        "branch": "ships",
        "rank": rank,
        "br": br,
        "cat": cat,
        "expClass": exp_class,
        "gun": main_gun,
        "gunBlk": gun_blk_rel,
        "reload": reload_sec,
        "speedYaw": speed_yaw,
        "ammoCount": ammo_count,
        "maxSpeedKmh": max_speed_kmh,
        "massTons": mass_tons,
        "engineHp": engine_hp,
        "crewCount": crew_count,
        "customizable": false,
        "pylons": ammo_slots.len(),
        "presets": ship_presets,
        "ammo_slots": ammo_slots,
        "belts": belts
    })
}

fn resolve_model_path(dm_dir: &Path, cfg_dir: &str, subdir: &str, vehicle_id: &str) -> PathBuf {
    let low = dm_dir
        .join(subdir)
        .join(format!("{}.blkx", vehicle_id.to_lowercase()));
    if low.exists() {
        return low;
    }
    let exact = dm_dir.join(subdir).join(format!("{}.blkx", vehicle_id));
    if exact.exists() {
        return exact;
    }
    Path::new(cfg_dir).join(format!("{}.blkx", vehicle_id))
}

fn load_weapon_telemetry(dm_dir: &Path, blk_rel: &str) -> Option<serde_json::Value> {
    let rel_clean = blk_rel
        .trim_start_matches('/')
        .to_lowercase()
        .replace('\\', "/");
    let stem_rel = rel_clean.strip_prefix("gamedata/").unwrap_or(&rel_clean);
    let blkx_rel = if stem_rel.ends_with(".blk") {
        format!("{}x", stem_rel)
    } else if stem_rel.ends_with(".blkx") {
        stem_rel.to_string()
    } else {
        format!("{}.blkx", stem_rel)
    };
    let db = crate::core::database::CatalogDatabase::get_instance();
    if let Some(cached) = db.get_weapon_stats(&blkx_rel) {
        return Some(cached);
    }
    let local_path = dm_dir.join(&blkx_rel);
    if !local_path.exists() {
        return None;
    }
    let txt = fs::read_to_string(&local_path).ok()?;
    let val: serde_json::Value = serde_json::from_str(&txt).ok()?;
    let stats = extract_air_weapon_telemetry(&val);
    db.save_weapon_stats(&blkx_rel, &stats);
    Some(stats)
}
