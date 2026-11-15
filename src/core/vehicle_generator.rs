use crate::core::vehicle_classifier::{classify_vehicle, VehicleBranch};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomPylonAssignment {
    pub slot: i64,
    pub preset: String,
}

pub struct VehicleGenerator {
    pub pkg_local_fm_dir: PathBuf,
    pub pkg_local_wp_dir: PathBuf,
    pub pkg_local_tm_dir: PathBuf,
    pub pkg_local_ships_dir: PathBuf,
}

impl VehicleGenerator {
    pub fn new(config: &crate::config::AppConfig) -> Self {
        let fm = PathBuf::from(&config.pkg_local_dir);
        let wp = if !config.pkg_local_weaponpresets_dir.is_empty() {
            PathBuf::from(&config.pkg_local_weaponpresets_dir)
        } else {
            fm.join("weaponPresets")
        };
        let tm = if !config.pkg_local_tankmodels_dir.is_empty() {
            PathBuf::from(&config.pkg_local_tankmodels_dir)
        } else {
            fm.parent()
                .map(|p| p.join("units/tankModels"))
                .unwrap_or_else(|| fm.join("tankModels"))
        };
        let ships = if !config.pkg_local_ships_dir.is_empty() {
            PathBuf::from(&config.pkg_local_ships_dir)
        } else {
            fm.parent()
                .map(|p| p.join("units/ships"))
                .unwrap_or_else(|| fm.join("ships"))
        };

        let _ = fs::create_dir_all(&fm);
        let _ = fs::create_dir_all(&wp);
        let _ = fs::create_dir_all(&tm);
        let _ = fs::create_dir_all(&ships);

        Self {
            pkg_local_fm_dir: fm,
            pkg_local_wp_dir: wp,
            pkg_local_tm_dir: tm,
            pkg_local_ships_dir: ships,
        }
    }

    /// Generates the custom `<id>.blk` wrapper inside `content/pkg_local/gameData/...`
    /// and optionally writes a custom `weaponPresets` BLK file when `custom_pylons` is provided.
    /// Returns `(custom_vehicle_blk_path, optional_custom_preset_name)`.
    pub fn generate_custom_vehicle_with_loadout(
        &self,
        vehicle_id: &str,
        is_ground_override: Option<bool>,
        is_ship_override: Option<bool>,
        exp_class_override: Option<&str>,
        custom_pylons: Option<&[CustomPylonAssignment]>,
    ) -> Result<PathBuf, String> {
        let safe_id = Path::new(vehicle_id)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(vehicle_id)
            .trim_start_matches("custom_");

        let classification = classify_vehicle(safe_id);
        let is_ship = is_ship_override
            .unwrap_or(classification.is_ship || classification.branch == VehicleBranch::Naval);
        let is_ground = !is_ship && is_ground_override.unwrap_or(classification.is_ground);
        let exp_class = exp_class_override.unwrap_or(&classification.exp_class);

        if is_ship {
            let out_file = self
                .pkg_local_ships_dir
                .join(format!("custom_{}.blk", safe_id));
            let content = format!(
                "include \"#/develop/gameBase/gameData/units/ships/{}.blk\"\n\nexpClass:t=\"{}\"\n",
                safe_id, exp_class
            );
            fs::write(&out_file, content).map_err(|e| e.to_string())?;
            info!(
                "[GENERATOR] Generated Naval BLK in 'units/ships': {:?}",
                out_file
            );
            return Ok(out_file);
        }

        if is_ground {
            let out_file = self
                .pkg_local_tm_dir
                .join(format!("custom_{}.blk", safe_id));
            let content = format!(
                "include \"#/develop/gameBase/gameData/units/tankModels/{}.blk\"\n\nexpClass:t=\"{}\"\n",
                safe_id, exp_class
            );
            fs::write(&out_file, content).map_err(|e| e.to_string())?;
            info!(
                "[GENERATOR] Generated Ground Vehicle BLK in 'units/tankModels': {:?}",
                out_file
            );
            return Ok(out_file);
        }

        let out_file = self
            .pkg_local_fm_dir
            .join(format!("custom_{}.blk", safe_id));
        let mut content = format!(
            "include \"#/develop/gameBase/gameData/flightModels/{}.blk\"\n\nexpClass:t=\"{}\"\n",
            safe_id, exp_class
        );

        if let Some(pylons) = custom_pylons {
            let valid_pylons: Vec<&CustomPylonAssignment> = pylons
                .iter()
                .filter(|p| p.slot >= 1 && !p.preset.trim().is_empty())
                .collect();
            if !valid_pylons.is_empty() {
                let preset_name = format!("custom_{}_loadout", safe_id);
                let preset_file = self.pkg_local_wp_dir.join(format!("{}.blk", preset_name));
                let mut wp_blk = String::new();
                for p in &valid_pylons {
                    wp_blk.push_str(&format!(
                        "Weapon{{\n  slot:i={}\n  preset:t=\"{}\"\n}}\n\n",
                        p.slot,
                        p.preset.trim()
                    ));
                }
                fs::write(&preset_file, wp_blk).map_err(|e| e.to_string())?;
                info!(
                    "[GENERATOR] Generated custom WeaponPreset BLK: {:?}",
                    preset_file
                );

                content.push_str(&format!(
                    "\nweapon_presets{{\n  preset{{\n    name:t=\"{}\"\n    blk:t=\"gameData/flightModels/weaponPresets/{}.blk\"\n  }}\n}}\n",
                    preset_name, preset_name
                ));
            }
        }

        fs::write(&out_file, content).map_err(|e| e.to_string())?;
        info!(
            "[GENERATOR] Generated Aviation BLK in 'flightModels': {:?}",
            out_file
        );
        Ok(out_file)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_custom_blks_all_branches_and_pylons() {
        let dir = std::env::temp_dir().join(format!("wt_gen_test_{}", std::process::id()));
        let fm = dir.join("content/pkg_local/gameData/flightModels");
        let wp = fm.join("weaponPresets");
        let tm = dir.join("content/pkg_local/gameData/units/tankModels");
        let ships = dir.join("content/pkg_local/gameData/units/ships");

        let mut cfg = crate::config::detect_paths(None);
        cfg.pkg_local_dir = fm.to_string_lossy().to_string();
        cfg.pkg_local_weaponpresets_dir = wp.to_string_lossy().to_string();
        cfg.pkg_local_tankmodels_dir = tm.to_string_lossy().to_string();
        cfg.pkg_local_ships_dir = ships.to_string_lossy().to_string();

        let gen = VehicleGenerator::new(&cfg);

        let tank_blk = gen
            .generate_custom_vehicle_with_loadout(
                "us_m1a2_sep3_abrams",
                Some(true),
                Some(false),
                Some("exp_tank"),
                None,
            )
            .expect("tank blk");
        let tank_txt = fs::read_to_string(&tank_blk).unwrap();
        assert!(tank_txt.contains(
            "include \"#/develop/gameBase/gameData/units/tankModels/us_m1a2_sep3_abrams.blk\""
        ));
        assert!(tank_txt.contains("expClass:t=\"exp_tank\""));

        let ship_blk = gen
            .generate_custom_vehicle_with_loadout(
                "us_destroyer_fletcher",
                Some(false),
                Some(true),
                Some("exp_destroyer"),
                None,
            )
            .expect("ship blk");
        let ship_txt = fs::read_to_string(&ship_blk).unwrap();
        assert!(ship_txt.contains(
            "include \"#/develop/gameBase/gameData/units/ships/us_destroyer_fletcher.blk\""
        ));

        let pylons = vec![
            CustomPylonAssignment {
                slot: 1,
                preset: "us_aim_9m_sidewinder".to_string(),
            },
            CustomPylonAssignment {
                slot: 6,
                preset: "us_aim_120b_amraam_x2".to_string(),
            },
        ];
        let air_blk = gen
            .generate_custom_vehicle_with_loadout(
                "fa_18e_block_2",
                Some(false),
                Some(false),
                Some("exp_fighter"),
                Some(&pylons),
            )
            .expect("air blk");
        let air_txt = fs::read_to_string(&air_blk).unwrap();
        assert!(air_txt
            .contains("include \"#/develop/gameBase/gameData/flightModels/fa_18e_block_2.blk\""));
        assert!(air_txt.contains("weaponPresets/custom_fa_18e_block_2_loadout.blk"));
        let wp_txt = fs::read_to_string(wp.join("custom_fa_18e_block_2_loadout.blk")).unwrap();
        assert!(wp_txt.contains("slot:i=1"));
        assert!(wp_txt.contains("preset:t=\"us_aim_9m_sidewinder\""));

        let _ = fs::remove_dir_all(&dir);
    }
}
