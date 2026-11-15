use serde_json::json;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use tracing::info;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

pub fn prune_logs_and_bundles(logs_dir: &Path, max_age_days: u64, max_zip_files: usize) {
    let now = std::time::SystemTime::now();
    let cutoff = now - std::time::Duration::from_secs(max_age_days * 86400);

    if let Ok(entries) = fs::read_dir(logs_dir) {
        let mut zips: Vec<(PathBuf, std::time::SystemTime)> = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if let Ok(meta) = path.metadata() {
                if let Ok(mtime) = meta.modified() {
                    if name.starts_with("wt_diagnostic_") && name.ends_with(".zip") {
                        zips.push((path, mtime));
                    }
                }
            }
        }
        zips.sort_by_key(|a| std::cmp::Reverse(a.1));
        for (idx, (p, mtime)) in zips.iter().enumerate() {
            if idx >= max_zip_files || *mtime < cutoff {
                let _ = fs::remove_file(p);
                info!("Pruned old diagnostic archive: {:?}", p.file_name());
            }
        }
    }
}

pub fn create_diagnostic_bundle(
    config: &crate::config::AppConfig,
    total_missions: usize,
    vehicle_count: usize,
) -> Result<(PathBuf, String), String> {
    let logs_dir = crate::config::get_logs_dir();
    let _ = fs::create_dir_all(&logs_dir);
    prune_logs_and_bundles(&logs_dir, 7, 5);

    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let zip_filename = format!("wt_diagnostic_{}.zip", ts);
    let zip_path = logs_dir.join(&zip_filename);

    let file = File::create(&zip_path).map_err(|e| e.to_string())?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    let db_path = crate::core::database::CatalogDatabase::get_instance().db_path();

    // 1. System report JSON
    let system_report = json!({
        "app": {
            "name": "Marmota Vehicle Tester",
            "version": env!("CARGO_PKG_VERSION"),
            "engine": "Rust (SQLite WAL + Tokio/Axum)",
        },
        "platform": {
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
        },
        "environment": {
            "resources_dir": config.resources_dir,
            "resources_dir_exists": Path::new(&config.resources_dir).exists(),
            "user_missions_dir": config.user_missions_dir,
            "user_missions_dir_exists": Path::new(&config.user_missions_dir).exists(),
            "user_skins_dir": config.user_skins_dir,
            "user_skins_dir_exists": Path::new(&config.user_skins_dir).exists(),
            "sqlite_db_path": db_path.to_string_lossy(),
            "sqlite_db_exists": db_path.exists(),
            "total_user_missions": total_missions,
            "cached_vehicles_count": vehicle_count,
            "language": config.language,
            "theme": config.theme,
            "first_run_completed": config.first_run_completed,
        }
    });

    zip.start_file("system_report.json", options)
        .map_err(|e| e.to_string())?;
    let report_str = serde_json::to_string_pretty(&system_report).map_err(|e| e.to_string())?;
    zip.write_all(report_str.as_bytes())
        .map_err(|e| e.to_string())?;

    // 2. Instructions
    let instructions = "=======================================================================\n\
WAR THUNDER CUSTOM VEHICLE & LOADOUT MANAGER — DIAGNOSTIC REPORT\n\
=======================================================================\n\n\
This bundle was generated automatically to assist in troubleshooting and debugging.\n\
Drag and drop this .zip file directly into a GitHub Issue.\n";

    zip.start_file("README_BUG_REPORT.txt", options)
        .map_err(|e| e.to_string())?;
    zip.write_all(instructions.as_bytes())
        .map_err(|e| e.to_string())?;

    // 3. Application log files
    for log_name in &["app.log", "error.log"] {
        let p = logs_dir.join(log_name);
        if p.exists() {
            if let Ok(bytes) = fs::read(&p) {
                if zip.start_file(*log_name, options).is_ok() {
                    let _ = zip.write_all(&bytes);
                }
            }
        }
    }

    zip.finish().map_err(|e| e.to_string())?;
    info!("Diagnostic bundle generated successfully: {:?}", zip_path);
    Ok((zip_path, zip_filename))
}

pub fn reveal_in_os_file_manager(file_path: &Path) -> bool {
    if !file_path.exists() {
        return false;
    }

    #[cfg(target_os = "macos")]
    {
        if std::process::Command::new("open")
            .arg("-R")
            .arg(file_path)
            .spawn()
            .is_ok()
        {
            return true;
        }
    }

    #[cfg(target_os = "windows")]
    {
        if std::process::Command::new("explorer")
            .arg(format!("/select,{}", file_path.display()))
            .spawn()
            .is_ok()
        {
            return true;
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Some(parent) = file_path.parent() {
            if std::process::Command::new("xdg-open")
                .arg(parent)
                .spawn()
                .is_ok()
            {
                return true;
            }
        }
    }

    false
}
