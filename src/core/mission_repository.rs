use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::info;
use walkdir::WalkDir;

use crate::core::blk_parser::{
    extract_string_field, extract_tm_altitude, find_named_block, find_unit_block_by_name,
    set_tm_altitude, strip_named_subblocks,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissionSummary {
    pub filename: String,
    pub path: String,
    pub name: String,
    pub player_wing: String,
    pub current_unit_class: String,
    pub current_weapons: String,
    pub current_skin: String,
    pub level: String,
    pub difficulty: String,
    pub restore_type: String,
    pub environment: String,
    pub weather: String,
    pub has_backup: bool,
}

pub struct MissionRepository {
    pub user_missions_dir: PathBuf,
}

impl MissionRepository {
    pub fn new(user_missions_dir: &str) -> Self {
        Self {
            user_missions_dir: PathBuf::from(user_missions_dir),
        }
    }

    /// Resolves a mission path strictly inside `user_missions_dir` to block path traversal.
    pub fn resolve_mission_path(&self, raw_filename: &str) -> Option<PathBuf> {
        let req_path = Path::new(raw_filename);
        let direct = self.user_missions_dir.join(req_path);
        if direct.exists() && is_inside_dir(&direct, &self.user_missions_dir) {
            return Some(direct);
        }

        let target_name = req_path.file_name().and_then(|n| n.to_str())?;
        for entry in WalkDir::new(&self.user_missions_dir)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_file() {
                if let Some(name) = entry.path().file_name().and_then(|n| n.to_str()) {
                    if name.eq_ignore_ascii_case(target_name) {
                        let candidate = entry.into_path();
                        if is_inside_dir(&candidate, &self.user_missions_dir) {
                            return Some(candidate);
                        }
                    }
                }
            }
        }
        None
    }

    /// Recursively lists all valid `.blk` missions inside `UserMissions`, ignoring 0-byte placeholders and `.bak`.
    pub fn list_missions(&self) -> Vec<MissionSummary> {
        let mut results: Vec<MissionSummary> = Vec::new();
        if !self.user_missions_dir.exists() {
            return results;
        }

        for entry in WalkDir::new(&self.user_missions_dir)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let p = entry.path();
            if p.is_file()
                && p.extension()
                    .and_then(|s| s.to_str())
                    .map(|e| e.eq_ignore_ascii_case("blk"))
                    .unwrap_or(false)
            {
                let fname = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
                if fname.starts_with('.')
                    || fname.ends_with(".bak")
                    || fname.eq_ignore_ascii_case("placeholder.blk")
                {
                    continue;
                }
                if let Ok(meta) = p.metadata() {
                    if meta.len() < 32 {
                        continue;
                    }
                }
                if let Some(m) = self.parse_mission_file(p) {
                    results.push(m);
                }
            }
        }
        results.sort_by(|a, b| a.filename.cmp(&b.filename));
        results
    }

    pub fn parse_mission_file(&self, filepath: &Path) -> Option<MissionSummary> {
        let content = fs::read_to_string(filepath).ok()?;
        if !content.contains("mission_settings") && !content.contains("units") {
            return None;
        }

        let filename = filepath
            .strip_prefix(&self.user_missions_dir)
            .ok()
            .map(|r| r.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|| {
                filepath
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default()
            });

        let mut name = filepath.file_stem()?.to_string_lossy().to_string();
        let mut player_wing = "player_01".to_string();
        let mut current_unit = String::new();
        let mut current_weapons = String::new();
        let mut current_skin = String::new();
        let mut level = String::new();
        let mut difficulty = "arcade".to_string();
        let mut restore_type = "attempts".to_string();
        let mut environment = "Day".to_string();
        let mut weather = "good".to_string();

        if let Some((start, _, end)) = find_named_block(&content, "player") {
            let p_block = &content[start..end];
            if let Some(w) = extract_string_field(p_block, "wing") {
                player_wing = w;
            }
        }

        if let Some((start, _, end)) = find_named_block(&content, "mission") {
            let m_block = &content[start..end];
            if let Some(n) = extract_string_field(m_block, "name")
                .or_else(|| extract_string_field(m_block, "locName"))
            {
                if !n.trim().is_empty() {
                    name = n;
                }
            }
            if let Some(l) = extract_string_field(m_block, "level") {
                level = l;
            }
            if let Some(d) = extract_string_field(m_block, "difficulty") {
                difficulty = d;
            }
            if let Some(r) = extract_string_field(m_block, "restoreType") {
                restore_type = r;
            }
            if let Some(e) = extract_string_field(m_block, "environment") {
                environment = e;
            }
            if let Some(w) = extract_string_field(m_block, "weather") {
                weather = w;
            }
        }

        if let Some((_, units_open, _)) = find_named_block(&content, "units") {
            if let Some((u_start, u_end, _)) =
                find_unit_block_by_name(&content, units_open, &player_wing)
            {
                let unit_block = &content[u_start..u_end];
                if let Some(uc) = extract_string_field(unit_block, "unit_class") {
                    current_unit = uc;
                }
                if let Some(wp) = extract_string_field(unit_block, "weapons") {
                    current_weapons = wp;
                }
                if let Some(sk) = extract_string_field(unit_block, "skin") {
                    current_skin = sk;
                }
            }
        }

        let has_backup = filepath.with_extension("blk.bak").exists();

        Some(MissionSummary {
            filename,
            path: filepath.to_string_lossy().to_string(),
            name,
            player_wing,
            current_unit_class: current_unit,
            current_weapons,
            current_skin,
            level,
            difficulty,
            restore_type,
            environment,
            weather,
            has_backup,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn inject_mission_unit_typed(
        &self,
        filepath: &Path,
        custom_unit_class: &str,
        custom_weapons: Option<&str>,
        custom_skin: Option<&str>,
        target_type: &str,
        bullets: [Option<&str>; 4],
        bullets_counts: [Option<i32>; 4],
        _auto_backup: bool,
        air_spawn_altitude: Option<f64>,
    ) -> Result<String, String> {
        if !filepath.exists() {
            return Err(format!("File does not exist: {:?}", filepath));
        }

        let is_ground = target_type == "tankModels";
        let is_ship = target_type == "ships";
        let mut content = fs::read_to_string(filepath).map_err(|e| e.to_string())?;

        let mut player_wing = "player_01".to_string();
        let mut player_army = 1i32;
        if let Some((start, _, end)) = find_named_block(&content, "player") {
            let p_block = &content[start..end];
            if let Some(w) = extract_string_field(p_block, "wing") {
                player_wing = w;
            }
            let army_re = Regex::new(r#"\barmy(?::i)?\s*=\s*(\d+)"#).unwrap();
            if let Some(cap) = army_re.captures(p_block) {
                if let Ok(a) = cap[1].parse::<i32>() {
                    player_army = a;
                }
            }
        }

        if find_named_block(&content, "units").is_none() {
            content.push_str("\nunits {\n}\n");
        }
        let (_, units_open, _) =
            find_named_block(&content, "units").ok_or("Missing units block")?;

        if let Some((start, end, old_type)) =
            find_unit_block_by_name(&content, units_open, &player_wing)
        {
            info!(
                "[MISSION.APPLY] Updating existing unit block '{}' ({}) -> {} in {:?}",
                player_wing,
                old_type,
                target_type,
                filepath.file_name()
            );
            let old_block = &content[start..end];
            let updated_block = self.update_existing_unit_block(
                old_block,
                &old_type,
                target_type,
                &player_wing,
                player_army,
                custom_unit_class,
                custom_weapons,
                custom_skin,
                is_ground,
                is_ship,
                bullets,
                bullets_counts,
                air_spawn_altitude,
            );
            content.replace_range(start..end, &updated_block);
        } else {
            info!(
                "[MISSION.APPLY] No matching unit block found for '{}'; writing new {} block into {:?}",
                player_wing, target_type, filepath.file_name()
            );
            let new_block = self.build_unit_block(
                target_type,
                &player_wing,
                player_army,
                custom_unit_class,
                custom_weapons,
                custom_skin,
                is_ground,
                is_ship,
                bullets,
                bullets_counts,
                air_spawn_altitude,
            );
            content.insert_str(units_open + 1, &format!("\n{}", new_block));
        }

        fs::write(filepath, content).map_err(|e| e.to_string())?;
        info!(
            "[MISSION.APPLY] Applied cleanly {} into {:?}",
            custom_unit_class,
            filepath.file_name()
        );
        Ok(player_wing)
    }

    #[allow(clippy::too_many_arguments)]
    fn update_existing_unit_block(
        &self,
        old_block: &str,
        old_type: &str,
        target_type: &str,
        _player_wing: &str,
        player_army: i32,
        custom_unit_class: &str,
        custom_weapons: Option<&str>,
        custom_skin: Option<&str>,
        is_ground: bool,
        is_ship: bool,
        bullets: [Option<&str>; 4],
        bullets_counts: [Option<i32>; 4],
        air_spawn_altitude: Option<f64>,
    ) -> String {
        let mut b = old_block.to_string();

        if !old_type.is_empty() && old_type != target_type {
            if let Some(pos) = b.find(old_type) {
                if let Some(brace) = b.find('{') {
                    if pos < brace {
                        b.replace_range(pos..pos + old_type.len(), target_type);
                    }
                }
            }
        }

        let uc_regex = Regex::new(r#"\bunit_class(?::t)?\s*=\s*"[^"]*""#).unwrap();
        let new_uc = format!(r#"unit_class:t="{}""#, custom_unit_class);
        if uc_regex.is_match(&b) {
            b = uc_regex.replace(&b, new_uc.as_str()).to_string();
        } else {
            let name_regex = Regex::new(r#"\bname(?::t)?\s*=\s*"[^"]*""#).unwrap();
            if let Some(m) = name_regex.find(&b) {
                b.insert_str(m.end(), &format!("\n    {}", new_uc));
            } else if let Some(brace) = b.find('{') {
                b.insert_str(brace + 1, &format!("\n    {}", new_uc));
            }
        }

        let mut wp_str = custom_weapons.unwrap_or("");
        if (is_ground || is_ship) && wp_str == "tank_weapon_preset_default" {
            wp_str = "";
        }
        let wp_regex = Regex::new(r#"\bweapons(?::t)?\s*=\s*"[^"]*""#).unwrap();
        let new_wp = format!(r#"weapons:t="{}""#, wp_str);
        if wp_regex.is_match(&b) {
            b = wp_regex.replace(&b, new_wp.as_str()).to_string();
        } else {
            let uc_re = Regex::new(r#"\bunit_class(?::t)?\s*=\s*"[^"]*""#).unwrap();
            if let Some(m) = uc_re.find(&b) {
                b.insert_str(m.end(), &format!("\n    {}", new_wp));
            }
        }

        let skin_regex = Regex::new(r#"\bskin(?::t)?\s*=\s*"[^"]*""#).unwrap();
        if let Some(sk) = custom_skin {
            let s = sk.trim();
            if !s.is_empty() && s.to_lowercase() != "default" {
                let new_sk = format!(r#"skin:t="{}""#, s);
                if skin_regex.is_match(&b) {
                    b = skin_regex.replace(&b, new_sk.as_str()).to_string();
                } else if let Some(m) = wp_regex.find(&b) {
                    b.insert_str(m.end(), &format!("\n    {}", new_sk));
                }
            } else if skin_regex.is_match(&b) {
                b = skin_regex.replace(&b, r#"skin:t="default""#).to_string();
            }
        }

        let mods_re = Regex::new(r#"\bapplyAllMods(?::b)?\s*=\s*\w+"#).unwrap();
        if mods_re.is_match(&b) {
            b = mods_re.replace(&b, "applyAllMods:b=yes").to_string();
        } else if let Some(m) = wp_regex.find(&b) {
            b.insert_str(m.end(), "\n    applyAllMods:b=yes");
        }

        let crew_re = Regex::new(r#"\bcrewSkillK(?::r)?\s*=\s*[\d.]+"#).unwrap();
        if crew_re.is_match(&b) {
            b = crew_re.replace(&b, "crewSkillK:r=999.0").to_string();
        } else if let Some(m) = wp_regex.find(&b) {
            b.insert_str(m.end(), "\n    crewSkillK:r=999.0");
        }

        if is_ground || is_ship {
            b = strip_named_subblocks(&b, "plane");

            let dummy_wp_re =
                Regex::new(r#"\bweapons(?::t)?\s*=\s*"tank_weapon_preset_default""#).unwrap();
            if dummy_wp_re.is_match(&b) {
                b = dummy_wp_re.replace(&b, r#"weapons:t="""#).to_string();
            }

            let primary_count = bullets_counts[0].unwrap_or(40).max(1);
            let bc0_re = Regex::new(r#"\bbulletsCount0(?::i)?\s*=\s*\d+"#).unwrap();
            let new_bc0 = format!("bulletsCount0:i={}", primary_count);
            if bc0_re.is_match(&b) {
                b = bc0_re.replace(&b, new_bc0.as_str()).to_string();
            } else if let Some(m) = wp_regex.find(&b) {
                b.insert_str(m.end(), &format!("\n    {}", new_bc0));
            }

            for (offset, opt_cnt) in bullets_counts.iter().skip(1).enumerate() {
                let idx = offset + 1;
                let default_cnt = if idx == 1 { primary_count } else { 20 };
                let cnt = opt_cnt.unwrap_or(default_cnt).max(1);
                let bc_re =
                    Regex::new(&format!(r#"\bbulletsCount{}(?::i)?\s*=\s*\d+"#, idx)).unwrap();
                let new_bc = format!("bulletsCount{}:i={}", idx, cnt);
                if bc_re.is_match(&b) {
                    b = bc_re.replace(&b, new_bc.as_str()).to_string();
                } else {
                    let prev_bc =
                        Regex::new(&format!(r#"\bbulletsCount{}(?::i)?\s*=\s*\d+"#, idx - 1))
                            .unwrap();
                    if let Some(m) = prev_bc.find(&b) {
                        b.insert_str(m.end(), &format!("\n    {}", new_bc));
                    } else if let Some(m) = wp_regex.find(&b) {
                        b.insert_str(m.end(), &format!("\n    {}", new_bc));
                    }
                }
            }

            for (idx, opt_bullet) in bullets.iter().enumerate() {
                let bullet_name = opt_bullet.unwrap_or("");
                let b_re =
                    Regex::new(&format!(r#"\bbullets{}(?::t)?\s*=\s*"[^"]*""#, idx)).unwrap();
                let new_b = format!(r#"bullets{}:t="{}""#, idx, bullet_name);
                if b_re.is_match(&b) {
                    b = b_re.replace(&b, new_b.as_str()).to_string();
                } else if opt_bullet.is_some() {
                    if let Some(m) = wp_regex.find(&b) {
                        b.insert_str(m.end(), &format!("\n    {}", new_b));
                    }
                }
            }
        } else {
            // Aircraft / helicopter injection: clear any leftover ground/naval multi-slot ammo fields
            let b0_re = Regex::new(r#"\bbullets0(?::t)?\s*=\s*"[^"]*""#).unwrap();
            let new_b0 = format!(r#"bullets0:t="{}""#, bullets[0].unwrap_or(""));
            if b0_re.is_match(&b) {
                b = b0_re.replace(&b, new_b0.as_str()).to_string();
            }
            if let Some(c0) = bullets_counts[0] {
                let bc0_re = Regex::new(r#"\bbulletsCount0(?::i)?\s*=\s*\d+"#).unwrap();
                let new_bc0 = format!("bulletsCount0:i={}", c0.max(0));
                if bc0_re.is_match(&b) {
                    b = bc0_re.replace(&b, new_bc0.as_str()).to_string();
                }
            }
            for idx in 1..4 {
                let b_re =
                    Regex::new(&format!(r#"\bbullets{}(?::t)?\s*=\s*"[^"]*""#, idx)).unwrap();
                if b_re.is_match(&b) {
                    b = b_re
                        .replace(&b, format!(r#"bullets{}:t="""#, idx).as_str())
                        .to_string();
                }
                let bc_re =
                    Regex::new(&format!(r#"\bbulletsCount{}(?::i)?\s*=\s*\d+"#, idx)).unwrap();
                if bc_re.is_match(&b) {
                    b = bc_re
                        .replace(&b, format!("bulletsCount{}:i=0", idx).as_str())
                        .to_string();
                }
            }

            if let Some(alt) = air_spawn_altitude {
                if alt > 50.0 {
                    if let Some(cur_y) = extract_tm_altitude(&b) {
                        if cur_y < 150.0 || alt > cur_y {
                            b = set_tm_altitude(&b, alt);
                        }
                    }
                }
            }
        }

        // Synchronize unit's props.army with mission_settings.player.army
        if let Some((p_start, p_open, p_end)) = find_named_block(&b, "props") {
            let mut props_slice = b[p_start..p_end].to_string();
            let army_re = Regex::new(r#"\barmy(?::i)?\s*=\s*\d+"#).unwrap();
            let new_army = format!("army:i={}", player_army);
            if army_re.is_match(&props_slice) {
                props_slice = army_re.replace(&props_slice, new_army.as_str()).to_string();
            } else {
                let brace_rel = p_open - p_start;
                props_slice.insert_str(brace_rel + 1, &format!("\n      {}", new_army));
            }
            b.replace_range(p_start..p_end, &props_slice);
        }

        // Preserve existing tm:m untouched, or restore default if absent
        let tm_re = Regex::new(r#"\btm(?::m)?\s*=\s*\[\["#).unwrap();
        if !tm_re.is_match(&b) {
            let y = if is_ground || is_ship {
                217.5
            } else {
                air_spawn_altitude.unwrap_or(1500.0)
            };
            let name_regex = Regex::new(r#"\bname(?::t)?\s*=\s*"[^"]*""#).unwrap();
            if let Some(m) = name_regex.find(&b) {
                b.insert_str(m.end(), &format!("\n    tm:m=[[1.0, 0.0, 0.0] [0.0, 1.0, 0.0] [0.0, 0.0, 1.0] [1690.0, {:.1}, 1444.0]]", y));
            }
        }

        b
    }

    #[allow(clippy::too_many_arguments)]
    fn build_unit_block(
        &self,
        block_type: &str,
        player_wing: &str,
        player_army: i32,
        custom_unit_class: &str,
        custom_weapons: Option<&str>,
        custom_skin: Option<&str>,
        is_ground: bool,
        is_ship: bool,
        bullets: [Option<&str>; 4],
        bullets_counts: [Option<i32>; 4],
        air_spawn_altitude: Option<f64>,
    ) -> String {
        let mut b = String::new();
        let spawn_y = if is_ground || is_ship {
            217.5
        } else {
            air_spawn_altitude.unwrap_or(1500.0)
        };
        b.push_str(&format!("  {}{{\n", block_type));
        b.push_str(&format!("    name:t=\"{}\"\n", player_wing));
        b.push_str(&format!(
            "    tm:m=[[1.0, 0.0, 0.0] [0.0, 1.0, 0.0] [0.0, 0.0, 1.0] [1690.0, {:.1}, 1444.0]]\n",
            spawn_y
        ));
        b.push_str(&format!("    unit_class:t=\"{}\"\n", custom_unit_class));
        b.push_str("    applyAllMods:b=yes\n");
        b.push_str("    crewSkillK:r=999.0\n");

        if let Some(wp) = custom_weapons {
            let wp_val = if (is_ground || is_ship) && wp == "tank_weapon_preset_default" {
                ""
            } else {
                wp
            };
            b.push_str(&format!("    weapons:t=\"{}\"\n", wp_val));
        } else {
            b.push_str("    weapons:t=\"\"\n");
        }

        if let Some(sk) = custom_skin {
            let s = sk.trim();
            if !s.is_empty() && s.to_lowercase() != "default" {
                b.push_str(&format!("    skin:t=\"{}\"\n", s));
            }
        }

        if is_ground || is_ship {
            let primary_count = bullets_counts[0].unwrap_or(40).max(1);
            let c1 = bullets_counts[1].unwrap_or(primary_count).max(1);
            let c2 = bullets_counts[2].unwrap_or(20).max(1);
            let c3 = bullets_counts[3].unwrap_or(20).max(1);

            b.push_str(&format!("    bulletsCount0:i={}\n", primary_count));
            b.push_str(&format!("    bulletsCount1:i={}\n", c1));
            b.push_str(&format!("    bulletsCount2:i={}\n", c2));
            b.push_str(&format!("    bulletsCount3:i={}\n", c3));

            for (idx, opt_bullet) in bullets.iter().enumerate() {
                let bullet_name = opt_bullet.unwrap_or("");
                b.push_str(&format!("    bullets{}:t=\"{}\"\n", idx, bullet_name));
            }
        }

        b.push_str("    props{\n");
        b.push_str(&format!("      army:i={}\n", player_army));
        b.push_str("      count:i=1\n");
        b.push_str("      formation_type:t=\"rows\"\n");
        b.push_str("      formation_div:i=3\n");
        b.push_str("      formation_step:p2=2.5, 2\n");
        b.push_str("      formation_noise:p2=0.1, 0.1\n");
        b.push_str("      uniqueName:t=\"\"\n");
        b.push_str("      attack_type:t=\"fire_at_will\"\n");
        b.push_str("    }\n");
        b.push_str("    way{\n");
        b.push_str("    }\n");
        b.push_str("  }\n");

        b
    }

    pub fn import_starter_missions(&self) -> Result<usize, String> {
        let assets_dir = crate::config::get_assets_dir();
        let proj_dir = crate::config::get_project_dir();
        let starters_candidates = [
            proj_dir.join("src/starter_missions"),
            assets_dir.join("custom_missions"),
            proj_dir.join("assets/custom_missions"),
            proj_dir.join("custom_missions"),
        ];

        let _ = fs::create_dir_all(&self.user_missions_dir);
        let mut count = 0;

        const EMBEDDED_GROUND: &str = include_str!("../starter_missions/ground_vehicle_test.blk");
        const EMBEDDED_AVIATION: &str =
            include_str!("../starter_missions/aviation_test_flight.blk");
        const EMBEDDED_HELICOPTER: &str =
            include_str!("../starter_missions/helicopter_test_flight.blk");

        let starters = [
            ("ground_vehicle_test.blk", EMBEDDED_GROUND),
            ("aviation_test_flight.blk", EMBEDDED_AVIATION),
            ("helicopter_test_flight.blk", EMBEDDED_HELICOPTER),
        ];

        for (filename, embedded_content) in &starters {
            let dest = self.user_missions_dir.join(filename);
            let mut copied = false;
            for cand in &starters_candidates {
                let src = cand.join(filename);
                if src.exists() && fs::copy(&src, &dest).is_ok() {
                    copied = true;
                    count += 1;
                    break;
                }
            }
            if !copied && fs::write(&dest, embedded_content).is_ok() {
                count += 1;
            }
        }

        if count == 0 {
            return Err(
                "Failed to import starter missions: no starter mission source found.".to_string(),
            );
        }
        Ok(count)
    }

    pub fn open_folder(&self) -> bool {
        open::that(&self.user_missions_dir).is_ok()
    }
}

fn is_inside_dir(candidate: &Path, root: &Path) -> bool {
    match (candidate.canonicalize(), root.canonicalize()) {
        (Ok(c), Ok(r)) => c.starts_with(r),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_and_inject_spaa_into_aircraft_mission() {
        let temp_dir = std::env::temp_dir().join("wt_mission_test_spaa_v2");
        let _ = fs::remove_dir_all(&temp_dir);
        let _ = fs::create_dir_all(&temp_dir);
        let mission_path = temp_dir.join("test_aviation.blk");

        let sample_blk = r#"selected_tag:t = ""
// comment with player { wing:t = "fake_wing" } inside
mission_settings {
	player {
		army:i = 1
		wing:t = "armada_01"
	}
	mission {
		name:t = "test_flight_ladoga"
		level:t = "levels/air_ladoga.bin"
		type:t = "testFlight"
		environment:t = "Day"
		weather:t = "good"
	}
}
units {
	armada {
		name:t = "armada_01"
		tm:m = [[0.00142521, 0.0, 0.999999] [0.0, 1.0, 0.0] [-0.999999, 0.0, 0.00142521] [-9016.01, 48.5698, -2683.6]]
		unit_class:t = "mig-21_s"
		weapons:t = ""
		bulletsCount0:i = 0
		props {
			army:i = 1
			plane {
				wing_formation:t = "Diamond"
			}
		}
	}
}
"#;
        fs::write(&mission_path, sample_blk).unwrap();

        let repo = MissionRepository::new(&temp_dir.to_string_lossy());
        let summary = repo
            .parse_mission_file(&mission_path)
            .expect("Failed to parse mission");
        assert_eq!(summary.player_wing, "armada_01");
        assert_eq!(summary.current_unit_class, "mig-21_s");
        assert_eq!(summary.environment, "Day");

        let res = repo.inject_mission_unit_typed(
            &mission_path,
            "custom_germ_flakpz_1a2_gepard",
            Some("tank_weapon_preset_default"),
            None,
            "tankModels",
            [Some("35mm_ger_ap"), None, None, None],
            [Some(640), None, None, None],
            true,
            None,
        );
        assert!(res.is_ok());

        let modified = fs::read_to_string(&mission_path).unwrap();
        assert!(modified.contains("tankModels"));
        assert!(modified.contains(r#"unit_class:t="custom_germ_flakpz_1a2_gepard""#));
        assert!(!modified.contains("plane {"));
        assert!(modified.contains("bulletsCount0:i=640"));
        assert!(modified.contains("bulletsCount1:i=640"));
        assert!(modified.contains(r#"weapons:t="""#));
        assert!(modified.contains("[-9016.01, 48.5698, -2683.6]"));
        assert!(modified.contains("applyAllMods:b=yes"));
        assert!(modified.contains("crewSkillK:r=999.0"));

        // Verify .blk.bak backup files are not created automatically
        assert!(!mission_path.with_extension("blk.bak").exists());

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_recursive_mission_listing_skips_placeholders_and_baks() {
        let temp_dir = std::env::temp_dir().join("wt_mission_test_walkdir_v2");
        let _ = fs::remove_dir_all(&temp_dir);
        let sub_dir = temp_dir.join("subfolder");
        let _ = fs::create_dir_all(&sub_dir);

        let m1 = temp_dir.join("root_mission.blk");
        let m2 = sub_dir.join("nested_mission.blk");
        let bak = temp_dir.join("ignored.blk.bak");
        let placeholder = temp_dir.join("placeholder.blk");

        let basic_blk = "mission_settings { player { wing:t = \"p1\" } } units {}";
        let _ = fs::write(&m1, basic_blk);
        let _ = fs::write(&m2, basic_blk);
        let _ = fs::write(&bak, basic_blk);
        let _ = fs::write(&placeholder, "");

        let repo = MissionRepository::new(&temp_dir.to_string_lossy());
        let list = repo.list_missions();
        assert_eq!(
            list.len(),
            2,
            "Must find exactly 2 missions, ignoring .bak and placeholder.blk"
        );

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_ground_vehicle_multi_slot_ammunition_and_weapon_sanitization() {
        let temp_dir = std::env::temp_dir().join("wt_tank_ammo_test_v2");
        let _ = fs::remove_dir_all(&temp_dir);
        let _ = fs::create_dir_all(&temp_dir);
        let mission_path = temp_dir.join("test_ground.blk");

        let sample_blk = r#"
mission_settings {
	player {
		wing:t = "player_01"
	}
}
units {
	tankModels {
		name:t = "player_01"
		tm:m = [[1.0, 0.0, 0.0] [0.0, 1.0, 0.0] [0.0, 0.0, 1.0] [1686.49, 219.665, 1474.27]]
		unit_class:t = "us_m1a2_sep_abrams"
		weapons:t = "tank_weapon_preset_default"
		bullets0:t = ""
		bullets1:t = ""
		bullets2:t = "old_shell"
		bullets3:t = ""
		bulletsCount0:i = 0
		bulletsCount1:i = 0
		bulletsCount2:i = 0
		bulletsCount3:i = 0
		crewSkillK:r = 0
		applyAllMods:b = no
		props {
			army:i = 1
		}
	}
}
"#;
        fs::write(&mission_path, sample_blk).unwrap();

        let repo = MissionRepository::new(&temp_dir.to_string_lossy());
        let res = repo.inject_mission_unit_typed(
            &mission_path,
            "custom_us_mim_72_chaparral",
            Some("tank_weapon_preset_default"),
            None,
            "tankModels",
            [
                Some("127mm_MIM72A_aam"),
                Some("127mm_MIM72C_aam"),
                None,
                None,
            ],
            [Some(12), Some(12), Some(20), Some(20)],
            false,
            None,
        );
        assert!(res.is_ok());

        let modified = fs::read_to_string(&mission_path).unwrap();
        assert!(modified.contains(r#"weapons:t="""#));
        assert!(!modified.contains("tank_weapon_preset_default"));
        assert!(
            !modified.contains("old_shell"),
            "Stale bullets2 must be cleared"
        );
        assert!(modified.contains(r#"bullets0:t="127mm_MIM72A_aam""#));
        assert!(modified.contains(r#"bullets1:t="127mm_MIM72C_aam""#));
        assert!(modified.contains("bulletsCount0:i=12"));
        assert!(modified.contains("bulletsCount1:i=12"));
        assert!(modified.contains("bulletsCount2:i=20"));
        assert!(modified.contains("bulletsCount3:i=20"));

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_inject_synchronizes_player_props_army() {
        let temp_dir = std::env::temp_dir().join("wt_army_sync_test");
        let _ = fs::remove_dir_all(&temp_dir);
        let _ = fs::create_dir_all(&temp_dir);
        let mission_path = temp_dir.join("sample_ship_like.blk");

        let sample_blk = r#"
mission_settings {
  player {
    army:i = 1
    wing:t = "tracked_vehicles_01"
  }
  player_teamB {
    army:i = 2
  }
}
units {
  ships {
    name:t = "tracked_vehicles_01"
    unit_class:t = "us_destroyer_fletcher"
    weapons:t = ""
    props {
      army:i = 2
      count:i = 1
    }
  }
}
"#;
        fs::write(&mission_path, sample_blk).unwrap();
        let repo = MissionRepository::new(&temp_dir.to_string_lossy());
        let res = repo.inject_mission_unit_typed(
            &mission_path,
            "custom_us_destroyer_fletcher",
            Some(""),
            None,
            "ships",
            [Some("127mm_usa_mk32_common_sap"), None, None, None],
            [Some(200), Some(100), None, None],
            false,
            None,
        );
        assert!(res.is_ok());

        let modified = fs::read_to_string(&mission_path).unwrap();
        assert!(
            modified.contains("army:i=1"),
            "Unit props must be synced to player army=1: {}",
            modified
        );
        assert!(
            modified.contains("player_teamB {\n    army:i = 2\n  }"),
            "player_teamB must remain untouched"
        );
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
