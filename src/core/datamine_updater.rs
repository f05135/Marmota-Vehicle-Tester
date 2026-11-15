use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

pub const GITHUB_RAW_BASE: &str =
    "https://raw.githubusercontent.com/gszabi99/War-Thunder-Datamine/master";
pub const USER_AGENT: &str =
    "WTCustomVehicleManager/2.0 (Rust; +https://github.com/gszabi99/War-Thunder-Datamine)";
pub const AUTO_CHECK_INTERVAL_SEC: u64 = 6 * 3600;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatamineState {
    pub version: String,
    pub last_checked: u64,
    pub last_updated: u64,
}

impl Default for DatamineState {
    fn default() -> Self {
        Self {
            version: "local".to_string(),
            last_checked: 0,
            last_updated: 0,
        }
    }
}

pub struct DatamineUpdater {
    pub client: reqwest::Client,
    pub datamine_dir: PathBuf,
    pub fm_dir: PathBuf,
    pub wp_dir: PathBuf,
    pub sensors_dir: PathBuf,
    pub tm_dir: PathBuf,
    pub ships_dir: PathBuf,
    pub version_file: PathBuf,
    pub shop_blkx_path: PathBuf,
}

impl DatamineUpdater {
    pub fn new(config: &crate::config::AppConfig) -> Self {
        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .unwrap_or_default();

        let dm_dir = PathBuf::from(&config.datamine_root);
        let fm_dir = PathBuf::from(&config.flightmodels_dir);
        let wp_dir = PathBuf::from(&config.weaponpresets_dir);
        let sensors_dir = PathBuf::from(&config.sensors_dir);
        let tm_dir = if !config.tankmodels_dir.is_empty() {
            PathBuf::from(&config.tankmodels_dir)
        } else {
            dm_dir.join("tankmodels")
        };
        let ships_dir = dm_dir.join("ships");
        let v_file = dm_dir.join("version.json");
        let shop_path = PathBuf::from(&config.shop_blkx_path);

        Self {
            client,
            datamine_dir: dm_dir,
            fm_dir,
            wp_dir,
            sensors_dir,
            tm_dir,
            ships_dir,
            version_file: v_file,
            shop_blkx_path: shop_path,
        }
    }

    pub fn get_local_state(&self) -> DatamineState {
        if self.version_file.exists() {
            if let Ok(c) = fs::read_to_string(&self.version_file) {
                if let Ok(st) = serde_json::from_str::<DatamineState>(&c) {
                    return st;
                }
            }
        }
        DatamineState::default()
    }

    pub fn save_local_state(
        &self,
        version: &str,
        last_checked: Option<u64>,
        last_updated: Option<u64>,
    ) {
        let curr = self.get_local_state();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let new_st = DatamineState {
            version: if version.is_empty() {
                curr.version
            } else {
                version.to_string()
            },
            last_checked: last_checked.unwrap_or(now),
            last_updated: last_updated.unwrap_or(curr.last_updated),
        };
        let _ = fs::create_dir_all(&self.datamine_dir);
        if let Ok(bytes) = serde_json::to_string_pretty(&new_st) {
            let _ = fs::write(&self.version_file, bytes);
        }
    }

    pub async fn fetch_remote_version(&self) -> Option<String> {
        let url = format!("{}/version", GITHUB_RAW_BASE);
        match self.client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => {
                if let Ok(text) = resp.text().await {
                    let v = text.trim();
                    if !v.is_empty() {
                        return Some(v.to_string());
                    }
                }
            }
            Ok(resp) => {
                warn!("HTTP {} fetching datamine version", resp.status());
            }
            Err(e) => {
                warn!("Failed to fetch remote datamine version: {}", e);
            }
        }
        None
    }

    pub async fn download_file(&self, rel_remote_path: &str, dest_path: &Path) -> bool {
        let clean_rel = rel_remote_path.trim_start_matches('/').to_lowercase();
        let url = format!("{}/{}", GITHUB_RAW_BASE, clean_rel);
        let tmp_path = dest_path.with_extension(format!(
            "{}.{}.tmp",
            dest_path
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("bin"),
            std::process::id()
        ));

        if let Some(parent) = dest_path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        match self.client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => {
                if let Ok(bytes) = resp.bytes().await {
                    if !bytes.is_empty() {
                        if let Ok(mut f) = File::create(&tmp_path) {
                            if f.write_all(&bytes).is_ok() {
                                let _ = f.flush();
                                drop(f);
                                if fs::rename(&tmp_path, dest_path).is_ok() {
                                    return true;
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        let _ = fs::remove_file(&tmp_path);
        false
    }

    pub async fn download_many(
        &self,
        jobs: Vec<(String, PathBuf)>,
        max_concurrency: usize,
    ) -> usize {
        self.download_many_completed(jobs, max_concurrency, false)
            .await
            .len()
    }

    pub async fn download_many_completed(
        &self,
        jobs: Vec<(String, PathBuf)>,
        max_concurrency: usize,
        overwrite_existing: bool,
    ) -> Vec<PathBuf> {
        use std::sync::Arc;
        use tokio::sync::Semaphore;

        let sem = Arc::new(Semaphore::new(max_concurrency.max(1)));
        let mut set = tokio::task::JoinSet::new();

        for (rel, dest) in jobs {
            if !overwrite_existing && dest.exists() {
                continue;
            }
            let permit_sem = sem.clone();
            let client = self.client.clone();
            set.spawn(async move {
                let Ok(_permit) = permit_sem.acquire().await else {
                    return None;
                };
                let clean_rel = rel.trim_start_matches('/').to_lowercase();
                let url = format!("{}/{}", GITHUB_RAW_BASE, clean_rel);
                if let Some(parent) = dest.parent() {
                    let _ = tokio::fs::create_dir_all(parent).await;
                }
                let tmp = dest.with_extension(format!(
                    "{}.{}.tmp",
                    dest.extension().and_then(|s| s.to_str()).unwrap_or("bin"),
                    std::process::id()
                ));
                if let Ok(resp) = client.get(&url).send().await {
                    if resp.status().is_success() {
                        if let Ok(bytes) = resp.bytes().await {
                            if !bytes.is_empty()
                                && tokio::fs::write(&tmp, &bytes).await.is_ok()
                                && tokio::fs::rename(&tmp, &dest).await.is_ok()
                            {
                                return Some(dest);
                            }
                        }
                    }
                }
                let _ = tokio::fs::remove_file(&tmp).await;
                None
            });
        }

        let mut completed = Vec::new();
        while let Some(res) = set.join_next().await {
            if let Ok(Some(path)) = res {
                completed.push(path);
            }
        }
        completed
    }

    pub fn extract_shop_unit_ids(
        shop_json: &serde_json::Value,
    ) -> (HashSet<String>, HashSet<String>, HashSet<String>) {
        #[derive(Copy, Clone)]
        enum BranchKind {
            Air,
            Army,
            Ship,
        }

        fn is_excluded_unit_id(uid: &str) -> bool {
            let excluded_suffixes = [
                "_football",
                "_tutorial",
                "_race",
                "_test",
                "_space",
                "_killstreak",
                "_snowball",
            ];
            excluded_suffixes.iter().any(|s| uid.ends_with(s))
                || uid.starts_with("nt_")
                || uid == "uav_quadcopter"
                || uid.starts_with("ucav_recon_micro")
        }

        fn walk(
            node: &serde_json::Value,
            kind: BranchKind,
            air: &mut HashSet<String>,
            army: &mut HashSet<String>,
            ships: &mut HashSet<String>,
        ) {
            if let Some(obj) = node.as_object() {
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
                        walk(v, kind, air, army, ships);
                        continue;
                    }
                    if let Some(inner) = v.as_object() {
                        if inner.contains_key("rank") || inner.contains_key("unit_class") {
                            let uid = k.trim().to_lowercase();
                            if !uid.is_empty() && !is_excluded_unit_id(&uid) {
                                match kind {
                                    BranchKind::Air => {
                                        air.insert(uid);
                                    }
                                    BranchKind::Army => {
                                        army.insert(uid);
                                    }
                                    BranchKind::Ship => {
                                        ships.insert(uid);
                                    }
                                }
                            }
                        } else {
                            walk(v, kind, air, army, ships);
                        }
                    } else if let Some(arr) = v.as_array() {
                        for item in arr {
                            walk(item, kind, air, army, ships);
                        }
                    }
                }
            }
        }

        let mut air_ids = HashSet::new();
        let mut army_ids = HashSet::new();
        let mut ship_ids = HashSet::new();

        if let Some(obj) = shop_json.as_object() {
            for (_country, country_block) in obj {
                if let Some(c_obj) = country_block.as_object() {
                    for branch in &["aviation", "helicopters"] {
                        if let Some(b_node) = c_obj.get(*branch) {
                            walk(
                                b_node,
                                BranchKind::Air,
                                &mut air_ids,
                                &mut army_ids,
                                &mut ship_ids,
                            );
                        }
                    }
                    if let Some(army_node) = c_obj.get("army") {
                        walk(
                            army_node,
                            BranchKind::Army,
                            &mut air_ids,
                            &mut army_ids,
                            &mut ship_ids,
                        );
                    }
                    for branch in &["ships", "boats"] {
                        if let Some(s_node) = c_obj.get(*branch) {
                            walk(
                                s_node,
                                BranchKind::Ship,
                                &mut air_ids,
                                &mut army_ids,
                                &mut ship_ids,
                            );
                        }
                    }
                }
            }
        }

        (air_ids, army_ids, ship_ids)
    }

    pub async fn check_for_updates(&self, auto_mode: bool) -> serde_json::Value {
        let state = self.get_local_state();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        if auto_mode
            && state.last_checked > 0
            && (now.saturating_sub(state.last_checked) < AUTO_CHECK_INTERVAL_SEC)
        {
            return json!({
                "ok": true,
                "skipped": true,
                "reason": "Interval not elapsed",
                "local_version": state.version,
                "update_available": false
            });
        }

        let remote_ver = match self.fetch_remote_version().await {
            Some(v) => v,
            None => {
                return json!({
                    "ok": false,
                    "error": "Could not connect to GitHub Datamine",
                    "local_version": state.version,
                    "update_available": false
                });
            }
        };

        let shop_path = self.shop_blkx_path.clone();
        let fm_dir = self.fm_dir.clone();
        let tm_dir = self.tm_dir.clone();
        let ships_dir = self.ships_dir.clone();
        let (missing_air, missing_army, missing_ships) = tokio::task::spawn_blocking(move || {
            Self::find_missing_in_dirs(&shop_path, &fm_dir, &tm_dir, &ships_dir)
        })
        .await
        .unwrap_or_default();

        let has_version_diff = state.version != remote_ver;
        let update_available = has_version_diff
            || !missing_air.is_empty()
            || !missing_army.is_empty()
            || !missing_ships.is_empty();

        self.save_local_state(&state.version, Some(now), None);

        json!({
            "ok": true,
            "skipped": false,
            "local_version": state.version,
            "remote_version": remote_ver,
            "update_available": update_available,
            "missing_units_count": missing_air.len() + missing_army.len() + missing_ships.len(),
            "missing_air": missing_air,
            "missing_army": missing_army,
            "missing_ships": missing_ships
        })
    }

    fn find_missing_in_dirs(
        shop_blkx_path: &Path,
        fm_dir: &Path,
        tm_dir: &Path,
        ships_dir: &Path,
    ) -> (Vec<String>, Vec<String>, Vec<String>) {
        if !shop_blkx_path.exists() {
            return (Vec::new(), Vec::new(), Vec::new());
        }
        let Ok(content) = fs::read_to_string(shop_blkx_path) else {
            return (Vec::new(), Vec::new(), Vec::new());
        };
        let Ok(shop_json) = serde_json::from_str::<serde_json::Value>(&content) else {
            return (Vec::new(), Vec::new(), Vec::new());
        };
        let (air_ids, army_ids, ship_ids) = Self::extract_shop_unit_ids(&shop_json);

        let mut missing_air: Vec<String> = air_ids
            .into_iter()
            .filter(|id| !fm_dir.join(format!("{}.blkx", id)).exists())
            .collect();
        missing_air.sort();
        let mut missing_army: Vec<String> = army_ids
            .into_iter()
            .filter(|id| !tm_dir.join(format!("{}.blkx", id)).exists())
            .collect();
        missing_army.sort();
        let mut missing_ships: Vec<String> = ship_ids
            .into_iter()
            .filter(|id| !ships_dir.join(format!("{}.blkx", id)).exists())
            .collect();
        missing_ships.sort();
        (missing_air, missing_army, missing_ships)
    }

    pub async fn perform_update(&self, force_core: bool) -> serde_json::Value {
        let t0 = std::time::Instant::now();
        let state = self.get_local_state();
        let local_ver = state.version.clone();

        let remote_ver = match self.fetch_remote_version().await {
            Some(v) => v,
            None => {
                return json!({
                    "ok": false,
                    "error": "Could not connect to GitHub Datamine",
                    "local_version": local_ver,
                    "updated": false
                });
            }
        };

        let need_core = force_core || (local_ver != remote_ver);

        // 1. Download core index files concurrently if needed
        if need_core {
            info!(
                "Datamine update: syncing core index files from GitHub (remote v{})...",
                remote_ver
            );
            let core_jobs = vec![
                (
                    "char.vromfs.bin_u/config/shop.blkx".to_string(),
                    self.datamine_dir.join("shop.blkx"),
                ),
                (
                    "char.vromfs.bin_u/config/wpcost.blkx".to_string(),
                    self.datamine_dir.join("wpcost.blkx"),
                ),
                (
                    "char.vromfs.bin_u/config/unittags.blkx".to_string(),
                    self.datamine_dir.join("unittags.blkx"),
                ),
                (
                    "char.vromfs.bin_u/config/modifications.blkx".to_string(),
                    self.datamine_dir.join("modifications.blkx"),
                ),
                (
                    "lang.vromfs.bin_u/lang/units.csv".to_string(),
                    self.datamine_dir.join("units.csv"),
                ),
                (
                    "lang.vromfs.bin_u/lang/units_weaponry.csv".to_string(),
                    self.datamine_dir.join("units_weaponry.csv"),
                ),
            ];
            let _ = self.download_many_completed(core_jobs, 6, true).await;
        }

        // 2. Discover missing units asynchronously off the executor thread
        let shop_path = self.shop_blkx_path.clone();
        let fm_dir = self.fm_dir.clone();
        let tm_dir = self.tm_dir.clone();
        let ships_dir = self.ships_dir.clone();
        let (missing_air, missing_army, missing_ships) = tokio::task::spawn_blocking(move || {
            Self::find_missing_in_dirs(&shop_path, &fm_dir, &tm_dir, &ships_dir)
        })
        .await
        .unwrap_or_default();

        info!(
            "Datamine discovery: {} missing aircraft, {} missing ground, {} missing ships.",
            missing_air.len(),
            missing_army.len(),
            missing_ships.len()
        );

        let air_jobs: Vec<(String, PathBuf)> = missing_air
            .iter()
            .map(|vid| {
                (
                    format!("aces.vromfs.bin_u/gamedata/flightmodels/{}.blkx", vid),
                    self.fm_dir.join(format!("{}.blkx", vid)),
                )
            })
            .collect();
        let army_jobs: Vec<(String, PathBuf)> = missing_army
            .iter()
            .map(|vid| {
                (
                    format!("aces.vromfs.bin_u/gamedata/units/tankmodels/{}.blkx", vid),
                    self.tm_dir.join(format!("{}.blkx", vid)),
                )
            })
            .collect();
        let ship_jobs: Vec<(String, PathBuf)> = missing_ships
            .iter()
            .map(|vid| {
                (
                    format!("aces.vromfs.bin_u/gamedata/units/ships/{}.blkx", vid),
                    self.ships_dir.join(format!("{}.blkx", vid)),
                )
            })
            .collect();

        let (dl_air_paths, dl_army_paths, dl_ship_paths) = tokio::join!(
            self.download_many_completed(air_jobs, 10, false),
            self.download_many_completed(army_jobs, 10, false),
            self.download_many_completed(ship_jobs, 10, false),
        );

        let paths_to_ids = |paths: &[PathBuf]| -> Vec<String> {
            paths
                .iter()
                .filter_map(|p| {
                    p.file_stem()
                        .and_then(|s| s.to_str())
                        .map(|s| s.to_string())
                })
                .collect()
        };
        let new_air_downloaded = paths_to_ids(&dl_air_paths);
        let new_army_downloaded = paths_to_ids(&dl_army_paths);
        let new_ships_downloaded = paths_to_ids(&dl_ship_paths);

        // 3. Inspect newly downloaded aircraft for missing weapon presets & sensors off-thread
        let wp_dir_scan = self.wp_dir.clone();
        let sensors_dir_scan = self.sensors_dir.clone();
        let fm_dir_scan = self.fm_dir.clone();
        let air_scan_ids = new_air_downloaded.clone();
        let (missing_presets, missing_sensors) = tokio::task::spawn_blocking(move || {
            let mut mp: HashSet<String> = HashSet::new();
            let mut ms: HashSet<String> = HashSet::new();
            for vid in &air_scan_ids {
                let fm_file = fm_dir_scan.join(format!("{}.blkx", vid));
                if let Ok(c) = fs::read_to_string(&fm_file) {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&c) {
                        if let Some(presets) =
                            json.get("weapon_presets").and_then(|w| w.get("preset"))
                        {
                            let list = if presets.is_array() {
                                presets.as_array().cloned().unwrap_or_default()
                            } else {
                                vec![presets.clone()]
                            };
                            for p in list {
                                if let Some(blk_ref) = p.get("blk").and_then(|b| b.as_str()) {
                                    let stem = Path::new(&blk_ref.replace('\\', "/"))
                                        .file_stem()
                                        .and_then(|s| s.to_str())
                                        .unwrap_or("")
                                        .to_lowercase();
                                    if !stem.is_empty()
                                        && !wp_dir_scan.join(format!("{}.blkx", stem)).exists()
                                    {
                                        mp.insert(stem);
                                    }
                                }
                            }
                        }
                        if let Some(sensors) = json.get("sensors").and_then(|s| s.get("sensor")) {
                            let list = if sensors.is_array() {
                                sensors.as_array().cloned().unwrap_or_default()
                            } else {
                                vec![sensors.clone()]
                            };
                            for s in list {
                                if let Some(s_ref) = s.get("blk").and_then(|b| b.as_str()) {
                                    let stem = Path::new(&s_ref.replace('\\', "/"))
                                        .file_stem()
                                        .and_then(|s| s.to_str())
                                        .unwrap_or("")
                                        .to_lowercase();
                                    if !stem.is_empty()
                                        && !sensors_dir_scan.join(format!("{}.blkx", stem)).exists()
                                    {
                                        ms.insert(stem);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            (mp, ms)
        })
        .await
        .unwrap_or_default();

        let preset_jobs: Vec<(String, PathBuf)> = missing_presets
            .into_iter()
            .map(|stem| {
                (
                    format!(
                        "aces.vromfs.bin_u/gamedata/flightmodels/weaponpresets/{}.blkx",
                        stem
                    ),
                    self.wp_dir.join(format!("{}.blkx", stem)),
                )
            })
            .collect();
        let sensor_jobs: Vec<(String, PathBuf)> = missing_sensors
            .into_iter()
            .map(|stem| {
                (
                    format!("aces.vromfs.bin_u/gamedata/sensors/{}.blkx", stem),
                    self.sensors_dir.join(format!("{}.blkx", stem)),
                )
            })
            .collect();

        let (downloaded_presets, downloaded_sensors) = tokio::join!(
            self.download_many(preset_jobs, 12),
            self.download_many(sensor_jobs, 12),
        );

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.save_local_state(&remote_ver, Some(now), Some(now));
        if need_core {
            tokio::task::spawn_blocking(|| {
                crate::core::vehicle_classifier::VehicleClassifier::get_instance().reload_indexes();
            })
            .await
            .ok();
        }

        let elapsed = t0.elapsed().as_secs_f64();
        info!(
            "Datamine update complete in {:.2}s: version={}, +{} air, +{} army, +{} ships, +{} presets, +{} sensors.",
            elapsed, remote_ver, new_air_downloaded.len(), new_army_downloaded.len(), new_ships_downloaded.len(), downloaded_presets, downloaded_sensors
        );

        json!({
            "ok": true,
            "updated": need_core || !new_air_downloaded.is_empty() || !new_army_downloaded.is_empty() || !new_ships_downloaded.is_empty(),
            "previous_version": local_ver,
            "version": remote_ver,
            "new_aircraft": new_air_downloaded,
            "new_tanks": new_army_downloaded,
            "new_ships": new_ships_downloaded,
            "new_vehicles_count": new_air_downloaded.len() + new_army_downloaded.len() + new_ships_downloaded.len(),
            "new_presets_count": downloaded_presets,
            "new_sensors_count": downloaded_sensors,
            "elapsed_sec": (elapsed * 100.0).round() / 100.0
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_shop_unit_ids_includes_naval_and_excludes_clutter() {
        let shop = json!({
            "country_usa": {
                "aviation": { "range": [{ "fa_18e_block_2": { "rank": 9 }, "nt_b_52h": { "rank": 7 } }] },
                "helicopters": { "range": [{ "ah_64e": { "rank": 7 }, "h_34_killstreak": { "rank": 5 } }] },
                "army": { "range": [{ "us_m1a2_sep3_abrams": { "rank": 8 }, "uav_quadcopter": { "rank": 7 } }] },
                "ships": { "range": [{ "us_destroyer_fletcher": { "rank": 2 } }] },
                "boats": { "range": [{ "us_pt_810": { "rank": 3 } }] }
            }
        });
        let (air, army, ships) = DatamineUpdater::extract_shop_unit_ids(&shop);
        assert!(air.contains("fa_18e_block_2"));
        assert!(air.contains("ah_64e"));
        assert!(!air.contains("nt_b_52h"));
        assert!(!air.contains("h_34_killstreak"));
        assert!(army.contains("us_m1a2_sep3_abrams"));
        assert!(!army.contains("uav_quadcopter"));
        assert!(ships.contains("us_destroyer_fletcher"));
        assert!(ships.contains("us_pt_810"));
    }
}
