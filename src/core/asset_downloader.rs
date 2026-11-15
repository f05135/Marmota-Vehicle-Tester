use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;
use tracing::{error, info, warn};

pub const DATAMINE_RAW_BASE: &str =
    "https://raw.githubusercontent.com/gszabi99/War-Thunder-Datamine/master";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadStatus {
    pub status: String, // "idle", "downloading", "extracting", "completed", "error"
    pub percent: f64,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub speed_mbps: f64,
    pub message: String,
    pub error: Option<String>,
}

static TRACKER: OnceLock<Arc<Mutex<DownloadStatus>>> = OnceLock::new();

fn get_tracker() -> Arc<Mutex<DownloadStatus>> {
    TRACKER
        .get_or_init(|| {
            Arc::new(Mutex::new(DownloadStatus {
                status: "idle".to_string(),
                percent: 0.0,
                downloaded_bytes: 0,
                total_bytes: 0,
                speed_mbps: 0.0,
                message: String::new(),
                error: None,
            }))
        })
        .clone()
}

pub fn get_download_status() -> DownloadStatus {
    let tracker = get_tracker();
    let res = tracker.lock().unwrap().clone();
    res
}

fn normalize_downloaded_version_file(path: &Path) {
    let Ok(raw) = fs::read_to_string(path) else {
        return;
    };
    let trimmed = raw.trim();
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) {
        if v.get("version")
            .and_then(|s| s.as_str())
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false)
        {
            return;
        }
    }
    if !trimmed.is_empty() && trimmed.len() < 64 && !trimmed.starts_with('{') {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let state = crate::core::datamine_updater::DatamineState {
            version: trimmed.to_string(),
            last_checked: now,
            last_updated: now,
        };
        if let Ok(json_str) = serde_json::to_string_pretty(&state) {
            let _ = fs::write(path, json_str);
        }
    }
}

fn refresh_config_preserving_prefs(repo: &crate::core::vehicle_repository::VehicleRepository) {
    let prev = repo.get_config();
    let explicit_res = (!prev.resources_dir.is_empty() && Path::new(&prev.resources_dir).exists())
        .then_some(prev.resources_dir.as_str());
    let mut updated = crate::config::detect_paths(explicit_res);
    updated.language = prev.language;
    updated.theme = crate::config::normalize_theme(&prev.theme);
    updated.first_run_completed = prev.first_run_completed;
    if !prev.user_missions_dir.is_empty() && Path::new(&prev.user_missions_dir).exists() {
        updated.user_missions_dir = prev.user_missions_dir;
    }
    if !prev.user_skins_dir.is_empty() && Path::new(&prev.user_skins_dir).exists() {
        updated.user_skins_dir = prev.user_skins_dir;
    }
    let _ = crate::config::save_config(&updated);
    repo.update_config(updated);
}

struct DatamineFileEntry {
    remote_path: &'static str,
    local_subpath: &'static str,
    est_bytes: u64,
    label: &'static str,
}

const CORE_DATAMINE_FILES: &[DatamineFileEntry] = &[
    DatamineFileEntry {
        remote_path: "char.vromfs.bin_u/config/shop.blkx",
        local_subpath: "datamine/shop.blkx",
        est_bytes: 383_147,
        label: "shop.blkx",
    },
    DatamineFileEntry {
        remote_path: "char.vromfs.bin_u/config/wpcost.blkx",
        local_subpath: "datamine/wpcost.blkx",
        est_bytes: 33_358_408,
        label: "wpcost.blkx",
    },
    DatamineFileEntry {
        remote_path: "lang.vromfs.bin_u/lang/units.csv",
        local_subpath: "datamine/units.csv",
        est_bytes: 6_028_962,
        label: "units.csv",
    },
    DatamineFileEntry {
        remote_path: "lang.vromfs.bin_u/lang/units_weaponry.csv",
        local_subpath: "datamine/units_weaponry.csv",
        est_bytes: 3_893_941,
        label: "units_weaponry.csv",
    },
    DatamineFileEntry {
        remote_path: "char.vromfs.bin_u/config/unittags.blkx",
        local_subpath: "datamine/unittags.blkx",
        est_bytes: 2_946_186,
        label: "unittags.blkx",
    },
    DatamineFileEntry {
        remote_path: "char.vromfs.bin_u/config/modifications.blkx",
        local_subpath: "datamine/modifications.blkx",
        est_bytes: 892_683,
        label: "modifications.blkx",
    },
    DatamineFileEntry {
        remote_path: "version",
        local_subpath: "datamine/version.json",
        est_bytes: 10,
        label: "version",
    },
    DatamineFileEntry {
        remote_path: "atlases.vromfs.bin_u/gameuiskin/air_to_air_missile.png",
        local_subpath: "gameuiskin/air_to_air_missile.png",
        est_bytes: 8_013,
        label: "air_to_air_missile.png",
    },
    DatamineFileEntry {
        remote_path: "atlases.vromfs.bin_u/gameuiskin/air_to_air_midrange_missile.png",
        local_subpath: "gameuiskin/air_to_air_midrange_missile.png",
        est_bytes: 8_715,
        label: "air_to_air_midrange_missile.png",
    },
    DatamineFileEntry {
        remote_path: "atlases.vromfs.bin_u/gameuiskin/bombs_large.png",
        local_subpath: "gameuiskin/bombs_large.png",
        est_bytes: 9_899,
        label: "bombs_large.png",
    },
    DatamineFileEntry {
        remote_path: "atlases.vromfs.bin_u/gameuiskin/nar_ap_large.png",
        local_subpath: "gameuiskin/nar_ap_large.png",
        est_bytes: 18_928,
        label: "nar_ap_large.png",
    },
    DatamineFileEntry {
        remote_path: "atlases.vromfs.bin_u/gameuiskin/apdsfs_tank.png",
        local_subpath: "gameuiskin/apdsfs_tank.png",
        est_bytes: 5_757,
        label: "apdsfs_tank.png",
    },
    DatamineFileEntry {
        remote_path: "atlases.vromfs.bin_u/gameuiskin/heat_fs_tank.png",
        local_subpath: "gameuiskin/heat_fs_tank.png",
        est_bytes: 5_500,
        label: "heat_fs_tank.png",
    },
    DatamineFileEntry {
        remote_path: "atlases.vromfs.bin_u/gameuiskin/smoke_tank.png",
        local_subpath: "gameuiskin/smoke_tank.png",
        est_bytes: 5_500,
        label: "smoke_tank.png",
    },
    DatamineFileEntry {
        remote_path: "atlases.vromfs.bin_u/gameuiskin/bombs_middle.png",
        local_subpath: "gameuiskin/bombs_middle.png",
        est_bytes: 9_200,
        label: "bombs_middle.png",
    },
    DatamineFileEntry {
        remote_path: "atlases.vromfs.bin_u/gameuiskin/bombs_small.png",
        local_subpath: "gameuiskin/bombs_small.png",
        est_bytes: 8_500,
        label: "bombs_small.png",
    },
    DatamineFileEntry {
        remote_path: "atlases.vromfs.bin_u/gameuiskin/guided_bomb_middle_jdam.png",
        local_subpath: "gameuiskin/guided_bomb_middle_jdam.png",
        est_bytes: 10_500,
        label: "guided_bomb_middle_jdam.png",
    },
    DatamineFileEntry {
        remote_path: "atlases.vromfs.bin_u/gameuiskin/agm_missile.png",
        local_subpath: "gameuiskin/agm_missile.png",
        est_bytes: 9_100,
        label: "agm_missile.png",
    },
    DatamineFileEntry {
        remote_path: "atlases.vromfs.bin_u/gameuiskin/atgm_type_helfire_x2.png",
        local_subpath: "gameuiskin/atgm_type_helfire_x2.png",
        est_bytes: 11_200,
        label: "atgm_type_helfire_x2.png",
    },
    DatamineFileEntry {
        remote_path: "atlases.vromfs.bin_u/gameuiskin/atgm_heli_rocket.png",
        local_subpath: "gameuiskin/atgm_heli_rocket.png",
        est_bytes: 12_400,
        label: "atgm_heli_rocket.png",
    },
    DatamineFileEntry {
        remote_path: "atlases.vromfs.bin_u/gameuiskin/he_frag_tank.png",
        local_subpath: "gameuiskin/he_frag_tank.png",
        est_bytes: 6_200,
        label: "he_frag_tank.png",
    },
];

pub fn start_download(
    repo: Arc<crate::core::vehicle_repository::VehicleRepository>,
) -> Result<(), String> {
    let tracker = get_tracker();
    {
        let mut g = tracker.lock().unwrap();
        if g.status == "downloading" || g.status == "extracting" {
            return Ok(());
        }
        *g = DownloadStatus {
            status: "downloading".to_string(),
            percent: 0.0,
            downloaded_bytes: 0,
            total_bytes: 47_503_337,
            speed_mbps: 0.0,
            message: "Connecting to GitHub Datamine CDN...".to_string(),
            error: None,
        };
    }

    tokio::spawn(async move {
        let tracker = get_tracker();

        // 1. Check if a local pre-packaged assets bundle exists (fast-path)
        let proj_dir = crate::config::get_project_dir();
        let app_data = crate::config::get_app_data_dir();
        let bundle_res = crate::config::get_bundle_resources_dir();
        let home_dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));

        let mut local_candidates = vec![
            proj_dir.join("assets-bundle.zip"),
            proj_dir.join("assets.zip"),
            home_dir.join("Downloads/assets-bundle.zip"),
            home_dir.join("Downloads/wt_custom_manager/assets-bundle.zip"),
            app_data.join("assets-bundle.zip"),
            PathBuf::from("assets-bundle.zip"),
        ];
        if let Some(res_dir) = bundle_res {
            local_candidates.insert(0, res_dir.join("assets.zip"));
            local_candidates.insert(0, res_dir.join("assets-bundle.zip"));
        }

        let found_local = local_candidates
            .iter()
            .find(|p| p.exists() && p.is_file())
            .cloned();

        if let Some(local_zip) = found_local {
            info!(
                "Found local assets bundle at {:?}, extracting directly with live progress...",
                local_zip
            );
            {
                let mut g = tracker.lock().unwrap();
                g.status = "extracting".to_string();
                g.percent = 5.0;
                g.message = "Extracting local vehicle catalog bundle...".to_string();
            }

            let zip_target = local_zip.clone();
            let app_data_target = app_data.clone();
            let tracker_for_extract = tracker.clone();

            let extract_res = tokio::task::spawn_blocking(move || {
                extract_zip_bundle(
                    &zip_target,
                    &app_data_target,
                    tracker_for_extract,
                    5.0,
                    95.0,
                )
            })
            .await;

            match extract_res {
                Ok(Ok(_)) => {
                    info!("Local assets bundle extracted successfully.");
                    {
                        let mut g = tracker.lock().unwrap();
                        g.percent = 96.0;
                        g.message = "Indexing 2,700+ vehicles and building cache...".to_string();
                    }

                    let repo_bg = repo.clone();
                    let count = tokio::task::spawn_blocking(move || {
                        refresh_config_preserving_prefs(&repo_bg);
                        crate::core::vehicle_classifier::VehicleClassifier::get_instance()
                            .reload_indexes();
                        repo_bg.get_full_tree(true).total
                    })
                    .await
                    .unwrap_or(0);

                    let mut g = tracker.lock().unwrap();
                    g.status = "completed".to_string();
                    g.percent = 100.0;
                    g.message = format!("Vehicle catalog ready! ({} vehicles loaded)", count);
                    info!(
                        "Local asset bundle installed & verified with {} vehicles",
                        count
                    );
                    return;
                }
                Ok(Err(e)) => {
                    warn!("Failed to extract local assets bundle: {}; falling back to datamine stream", e);
                }
                Err(e) => {
                    warn!(
                        "Extraction task panicked: {}; falling back to datamine stream",
                        e
                    );
                }
            }
        }

        // 2. Direct Network Download from War Thunder Datamine (GitHub CDN)
        let custom_zip_url = std::env::var("MARMOTA_ASSETS_URL").ok();
        if let Some(zip_url) = custom_zip_url {
            if zip_url.ends_with(".zip") {
                info!("Using custom archive download URL: {}", zip_url);
                download_and_extract_zip(&zip_url, &app_data, tracker.clone(), repo.clone()).await;
                return;
            }
        }

        let base_url =
            std::env::var("MARMOTA_DATAMINE_URL").unwrap_or_else(|_| DATAMINE_RAW_BASE.to_string());

        let assets_dir = crate::config::get_assets_dir();
        let target_dm_dir = assets_dir.join("datamine");
        if let Err(e) = fs::create_dir_all(&target_dm_dir) {
            let err_msg = format!(
                "Failed to create datamine directory {:?}: {}",
                target_dm_dir, e
            );
            error!("{}", err_msg);
            let mut g = tracker.lock().unwrap();
            g.status = "error".to_string();
            g.error = Some(err_msg.clone());
            g.message = err_msg;
            return;
        }

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .user_agent("MarmotaVehicleTester/1.0 (Datamine Downloader)")
            .build();

        let client = match client {
            Ok(c) => c,
            Err(e) => {
                let err_msg = format!("Failed to initialize HTTP client: {}", e);
                error!("{}", err_msg);
                let mut g = tracker.lock().unwrap();
                g.status = "error".to_string();
                g.error = Some(err_msg.clone());
                g.message = err_msg;
                return;
            }
        };

        let total_expected_bytes: u64 = CORE_DATAMINE_FILES.iter().map(|f| f.est_bytes).sum();
        {
            let mut g = tracker.lock().unwrap();
            g.status = "downloading".to_string();
            g.total_bytes = total_expected_bytes;
            g.downloaded_bytes = 0;
            g.percent = 0.0;
            g.message = "Starting download from War Thunder Datamine CDN...".to_string();
        }

        info!(
            "Starting datamine download: 7 core files (~{:.1} MB) from {}",
            total_expected_bytes as f64 / 1_048_576.0,
            base_url
        );

        let mut cumulative_bytes: u64 = 0;
        let start_time = Instant::now();
        let mut last_speed_time = Instant::now();
        let mut last_speed_bytes = 0u64;
        let mut current_speed_mbps = 0.0;

        let total_files = CORE_DATAMINE_FILES.len();

        for (idx, entry) in CORE_DATAMINE_FILES.iter().enumerate() {
            let file_url = format!("{}/{}", base_url.trim_end_matches('/'), entry.remote_path);
            let dest_file = assets_dir.join(entry.local_subpath);
            if let Some(parent) = dest_file.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let tmp_file = dest_file.with_extension(format!("tmp.{}", std::process::id()));

            info!(
                "Downloading [{}/{}] {} -> {:?}",
                idx + 1,
                total_files,
                entry.label,
                dest_file
            );

            {
                let mut g = tracker.lock().unwrap();
                let pct = ((cumulative_bytes as f64 / total_expected_bytes as f64) * 90.0)
                    .clamp(0.0, 90.0);
                g.percent = (pct * 10.0).round() / 10.0;
                g.message = format!("Connecting: {} ({}/{})", entry.label, idx + 1, total_files);
            }

            let mut resp = match client.get(&file_url).send().await {
                Ok(r) if r.status().is_success() => r,
                Ok(r) => {
                    let err_msg = format!(
                        "HTTP error {} downloading {} from Datamine CDN",
                        r.status(),
                        entry.label
                    );
                    error!("{}", err_msg);
                    let mut g = tracker.lock().unwrap();
                    g.status = "error".to_string();
                    g.error = Some(err_msg.clone());
                    g.message = err_msg;
                    let _ = fs::remove_file(&tmp_file);
                    return;
                }
                Err(e) => {
                    let err_msg = format!("Network failure downloading {}: {}", entry.label, e);
                    error!("{}", err_msg);
                    let mut g = tracker.lock().unwrap();
                    g.status = "error".to_string();
                    g.error = Some(err_msg.clone());
                    g.message = err_msg;
                    let _ = fs::remove_file(&tmp_file);
                    return;
                }
            };

            let mut out = match File::create(&tmp_file) {
                Ok(f) => f,
                Err(e) => {
                    let err_msg = format!("Cannot create temporary file {:?}: {}", tmp_file, e);
                    error!("{}", err_msg);
                    let mut g = tracker.lock().unwrap();
                    g.status = "error".to_string();
                    g.error = Some(err_msg.clone());
                    g.message = err_msg;
                    return;
                }
            };

            loop {
                match resp.chunk().await {
                    Ok(Some(chunk)) => {
                        if let Err(e) = out.write_all(&chunk) {
                            let err_msg =
                                format!("Disk write error writing {}: {}", entry.label, e);
                            error!("{}", err_msg);
                            let _ = fs::remove_file(&tmp_file);
                            let mut g = tracker.lock().unwrap();
                            g.status = "error".to_string();
                            g.error = Some(err_msg.clone());
                            g.message = err_msg;
                            return;
                        }
                        cumulative_bytes += chunk.len() as u64;

                        if last_speed_time.elapsed().as_millis() >= 120 {
                            let elapsed_sec = last_speed_time.elapsed().as_secs_f64();
                            let delta = cumulative_bytes.saturating_sub(last_speed_bytes);
                            if elapsed_sec > 0.0 {
                                current_speed_mbps = (delta as f64 / 1_048_576.0) / elapsed_sec;
                            }
                            last_speed_time = Instant::now();
                            last_speed_bytes = cumulative_bytes;

                            let pct = ((cumulative_bytes as f64 / total_expected_bytes as f64)
                                * 90.0)
                                .clamp(0.0, 90.0);
                            let dl_mb = cumulative_bytes as f64 / 1_048_576.0;
                            let tot_mb = total_expected_bytes as f64 / 1_048_576.0;

                            let mut g = tracker.lock().unwrap();
                            g.percent = (pct * 10.0).round() / 10.0;
                            g.downloaded_bytes = cumulative_bytes;
                            g.speed_mbps = current_speed_mbps;
                            g.message = format!(
                                "Downloading: {} ({}/{}) • {:.1} MB / {:.1} MB ({:.1} MB/s)",
                                entry.label,
                                idx + 1,
                                total_files,
                                dl_mb,
                                tot_mb,
                                current_speed_mbps
                            );
                        }
                    }
                    Ok(None) => break,
                    Err(e) => {
                        let err_msg = format!("Connection error streaming {}: {}", entry.label, e);
                        error!("{}", err_msg);
                        let _ = fs::remove_file(&tmp_file);
                        let mut g = tracker.lock().unwrap();
                        g.status = "error".to_string();
                        g.error = Some(err_msg.clone());
                        g.message = err_msg;
                        return;
                    }
                }
            }

            let _ = out.flush();
            drop(out);

            if let Err(e) = fs::rename(&tmp_file, &dest_file) {
                let err_msg = format!("Failed to move {:?} -> {:?}: {}", tmp_file, dest_file, e);
                error!("{}", err_msg);
                let mut g = tracker.lock().unwrap();
                g.status = "error".to_string();
                g.error = Some(err_msg.clone());
                g.message = err_msg;
                return;
            }
            if entry.remote_path == "version" {
                normalize_downloaded_version_file(&dest_file);
            }
        }

        let elapsed_total = start_time.elapsed().as_secs_f64();
        info!(
            "Datamine download completed in {:.1}s ({:.1} MB). Building vehicle catalog cache...",
            elapsed_total,
            cumulative_bytes as f64 / 1_048_576.0
        );

        {
            let mut g = tracker.lock().unwrap();
            g.status = "extracting".to_string();
            g.percent = 92.0;
            g.downloaded_bytes = cumulative_bytes;
            g.speed_mbps = 0.0;
            g.message = "Indexing 2,700+ vehicles and building cache...".to_string();
        }

        // Reload configuration preserving user preferences, classifier indexes, and index vehicle repository into SQLite WAL
        let repo_bg = repo.clone();
        let count = tokio::task::spawn_blocking(move || {
            refresh_config_preserving_prefs(&repo_bg);
            crate::core::vehicle_classifier::VehicleClassifier::get_instance().reload_indexes();
            repo_bg.get_full_tree(true).total
        })
        .await
        .unwrap_or(0);

        if crate::config::are_assets_installed() {
            let mut g = tracker.lock().unwrap();
            g.status = "completed".to_string();
            g.percent = 100.0;
            g.message = format!("Vehicle catalog ready! ({} vehicles loaded)", count);
            info!(
                "Datamine assets catalog installed & verified with {} vehicles!",
                count
            );
        } else {
            let err_msg =
                "Download finished but 'assets/datamine/units.csv' was not found on disk."
                    .to_string();
            error!("{}", err_msg);
            let mut g = tracker.lock().unwrap();
            g.status = "error".to_string();
            g.error = Some(err_msg.clone());
            g.message = err_msg;
        }
    });

    Ok(())
}

async fn download_and_extract_zip(
    url: &str,
    app_data: &Path,
    tracker: Arc<Mutex<DownloadStatus>>,
    repo: Arc<crate::core::vehicle_repository::VehicleRepository>,
) {
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(600))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            let err_msg = format!("HTTP client error: {}", e);
            let mut g = tracker.lock().unwrap();
            g.status = "error".to_string();
            g.error = Some(err_msg.clone());
            g.message = err_msg;
            return;
        }
    };

    let response = match client.get(url).send().await {
        Ok(r) if r.status().is_success() => r,
        Ok(r) => {
            let err_msg = format!("HTTP error {} downloading archive", r.status());
            let mut g = tracker.lock().unwrap();
            g.status = "error".to_string();
            g.error = Some(err_msg.clone());
            g.message = err_msg;
            return;
        }
        Err(e) => {
            let err_msg = format!("Network failure downloading archive: {}", e);
            let mut g = tracker.lock().unwrap();
            g.status = "error".to_string();
            g.error = Some(err_msg.clone());
            g.message = err_msg;
            return;
        }
    };

    let total_size = response.content_length().unwrap_or(50 * 1024 * 1024);
    let download_path = app_data.join("assets_bundle_download.tmp");
    let mut file = match File::create(&download_path) {
        Ok(f) => f,
        Err(e) => {
            let err_msg = format!("Cannot create temporary file: {}", e);
            let mut g = tracker.lock().unwrap();
            g.status = "error".to_string();
            g.error = Some(err_msg.clone());
            g.message = err_msg;
            return;
        }
    };

    let mut downloaded: u64 = 0;
    let mut stream = response;

    loop {
        match stream.chunk().await {
            Ok(Some(chunk)) => {
                if let Err(e) = file.write_all(&chunk) {
                    let err_msg = format!("Disk error: {}", e);
                    let _ = fs::remove_file(&download_path);
                    let mut g = tracker.lock().unwrap();
                    g.status = "error".to_string();
                    g.error = Some(err_msg.clone());
                    g.message = err_msg;
                    return;
                }
                downloaded += chunk.len() as u64;
                let pct = if total_size > 0 {
                    ((downloaded as f64 / total_size as f64) * 70.0).clamp(0.0, 70.0)
                } else {
                    0.0
                };
                let mut g = tracker.lock().unwrap();
                g.percent = (pct * 10.0).round() / 10.0;
                g.message = format!(
                    "Downloading archive: {:.1} MB / {:.1} MB",
                    downloaded as f64 / 1_048_576.0,
                    total_size as f64 / 1_048_576.0
                );
            }
            Ok(None) => break,
            Err(e) => {
                let err_msg = format!("Stream error: {}", e);
                let _ = fs::remove_file(&download_path);
                let mut g = tracker.lock().unwrap();
                g.status = "error".to_string();
                g.error = Some(err_msg.clone());
                g.message = err_msg;
                return;
            }
        }
    }

    let _ = file.flush();
    drop(file);

    let dl_path_target = download_path.clone();
    let app_data_target = app_data.to_path_buf();
    let tracker_extract = tracker.clone();

    let extract_res = tokio::task::spawn_blocking(move || {
        extract_zip_bundle(
            &dl_path_target,
            &app_data_target,
            tracker_extract,
            70.0,
            95.0,
        )
    })
    .await;

    let _ = fs::remove_file(&download_path);

    if let Ok(Ok(_)) = extract_res {
        let repo_bg = repo.clone();
        let count = tokio::task::spawn_blocking(move || {
            refresh_config_preserving_prefs(&repo_bg);
            crate::core::vehicle_classifier::VehicleClassifier::get_instance().reload_indexes();
            repo_bg.get_full_tree(true).total
        })
        .await
        .unwrap_or(0);

        let mut g = tracker.lock().unwrap();
        g.status = "completed".to_string();
        g.percent = 100.0;
        g.message = format!("Vehicle catalog ready! ({} vehicles loaded)", count);
    }
}

fn extract_zip_bundle(
    zip_path: &Path,
    target_dir: &Path,
    tracker: Arc<Mutex<DownloadStatus>>,
    start_pct: f64,
    max_pct: f64,
) -> Result<(), String> {
    let file = File::open(zip_path).map_err(|e| format!("Cannot open zip file: {}", e))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("Invalid zip format: {}", e))?;

    let mut has_assets_prefix = false;
    for i in 0..archive.len().min(10) {
        if let Ok(entry) = archive.by_index(i) {
            if entry.name().starts_with("assets/") || entry.name().starts_with("assets\\") {
                has_assets_prefix = true;
                break;
            }
        }
    }

    let extract_root = if has_assets_prefix {
        target_dir.to_path_buf()
    } else {
        target_dir.join("assets")
    };

    fs::create_dir_all(&extract_root)
        .map_err(|e| format!("Cannot create extraction directory: {}", e))?;

    let total = archive.len();

    for i in 0..total {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("Corrupt zip entry {}: {}", i, e))?;
        let entry_name = entry
            .enclosed_name()
            .ok_or_else(|| "Zip slip attack detected in filename".to_string())?;

        let outpath = extract_root.join(entry_name.clone());

        if entry.is_dir() {
            fs::create_dir_all(&outpath)
                .map_err(|e| format!("Failed to create folder {:?}: {}", outpath, e))?;
        } else {
            if let Some(parent) = outpath.parent() {
                if !parent.exists() {
                    fs::create_dir_all(parent)
                        .map_err(|e| format!("Failed to create parent {:?}: {}", parent, e))?;
                }
            }
            let mut outfile = File::create(&outpath)
                .map_err(|e| format!("Failed to create output file {:?}: {}", outpath, e))?;
            std::io::copy(&mut entry, &mut outfile)
                .map_err(|e| format!("Failed to write file {:?}: {}", outpath, e))?;
        }

        if i % 25 == 0 || i == total - 1 {
            let pct = start_pct + (i as f64 / total as f64) * (max_pct - start_pct);
            let fname = entry_name
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("files");
            let mut g = tracker.lock().unwrap();
            g.percent = (pct * 10.0).round() / 10.0;
            g.message = format!("Extracting: {} ({}/{})", fname, i + 1, total);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    async fn spawn_mock_datamine_server() -> String {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind mock listener");
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            loop {
                let Ok((mut sock, _)) = listener.accept().await else {
                    break;
                };
                tokio::spawn(async move {
                    let mut buf = [0u8; 2048];
                    let n = sock.read(&mut buf).await.unwrap_or(0);
                    let req = String::from_utf8_lossy(&buf[..n]);
                    let path = req
                        .lines()
                        .next()
                        .and_then(|l| l.split_whitespace().nth(1))
                        .unwrap_or("/");
                    let (status, body): (&str, Vec<u8>) = match path {
                        "/char.vromfs.bin_u/config/shop.blkx" => ("200 OK", b"{}".to_vec()),
                        "/char.vromfs.bin_u/config/wpcost.blkx" => ("200 OK", b"{}".to_vec()),
                        "/lang.vromfs.bin_u/lang/units.csv" => ("200 OK", b"ID;English\n".to_vec()),
                        "/lang.vromfs.bin_u/lang/units_weaponry.csv" => {
                            ("200 OK", b"ID;English\n".to_vec())
                        }
                        "/char.vromfs.bin_u/config/unittags.blkx" => ("200 OK", b"{}".to_vec()),
                        "/char.vromfs.bin_u/config/modifications.blkx" => {
                            ("200 OK", b"{}".to_vec())
                        }
                        "/version" => ("200 OK", b"2.55.0.21\n".to_vec()),
                        p if p.ends_with(".png") => ("200 OK", b"\x89PNG\r\n\x1a\n".to_vec()),
                        _ => ("404 Not Found", b"not found".to_vec()),
                    };
                    let header = format!(
                        "HTTP/1.1 {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        status,
                        body.len()
                    );
                    let _ = sock.write_all(header.as_bytes()).await;
                    let _ = sock.write_all(&body).await;
                });
            }
        });
        format!("http://{}", addr)
    }

    #[tokio::test]
    async fn test_mock_datamine_download_and_version_normalization() {
        let base_url = spawn_mock_datamine_server().await;
        let temp_dir = std::env::temp_dir().join(format!(
            "marmota_dl_test_{}_{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&temp_dir);
        let assets_dir = temp_dir.join("assets");
        fs::create_dir_all(&assets_dir).unwrap();

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap();

        for entry in CORE_DATAMINE_FILES.iter().take(7) {
            let file_url = format!("{}/{}", base_url, entry.remote_path);
            let dest_file = assets_dir.join(entry.local_subpath);
            if let Some(parent) = dest_file.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let resp = client.get(&file_url).send().await.expect("fetch from mock");
            assert!(resp.status().is_success(), "Failed for {}", entry.label);
            let bytes = resp.bytes().await.expect("read bytes");
            fs::write(&dest_file, &bytes).expect("write file");
            if entry.remote_path == "version" {
                normalize_downloaded_version_file(&dest_file);
            }
        }

        assert!(assets_dir.join("datamine/units.csv").exists());
        assert!(assets_dir.join("datamine/shop.blkx").exists());
        assert!(assets_dir.join("datamine/wpcost.blkx").exists());
        assert!(assets_dir.join("datamine/units_weaponry.csv").exists());
        assert!(assets_dir.join("datamine/unittags.blkx").exists());
        assert!(assets_dir.join("datamine/modifications.blkx").exists());

        let ver_txt = fs::read_to_string(assets_dir.join("datamine/version.json")).unwrap();
        let ver_json: serde_json::Value =
            serde_json::from_str(&ver_txt).expect("version.json must be valid JSON");
        assert_eq!(
            ver_json.get("version").and_then(|v| v.as_str()),
            Some("2.55.0.21")
        );

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_normalize_downloaded_version_file_handles_raw_and_json() {
        let temp_dir = std::env::temp_dir().join(format!(
            "marmota_ver_norm_{}_{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::create_dir_all(&temp_dir);
        let vf = temp_dir.join("version.json");

        fs::write(&vf, "2.53.0.14\n").unwrap();
        normalize_downloaded_version_file(&vf);
        let parsed: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&vf).unwrap()).unwrap();
        assert_eq!(
            parsed.get("version").and_then(|s| s.as_str()),
            Some("2.53.0.14")
        );

        // Already valid JSON should remain valid
        normalize_downloaded_version_file(&vf);
        let parsed2: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&vf).unwrap()).unwrap();
        assert_eq!(
            parsed2.get("version").and_then(|s| s.as_str()),
            Some("2.53.0.14")
        );

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
