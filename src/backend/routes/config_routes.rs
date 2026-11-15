use crate::backend::AppState;
use axum::{extract::State, Json};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct ConfigUpdateRequest {
    pub language: Option<String>,
    pub theme: Option<String>,
    pub first_run_completed: Option<bool>,
    pub first_run: Option<bool>,
    pub resources_dir: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ValidateDirectoryRequest {
    pub path: String,
}

#[derive(Debug, Deserialize)]
pub struct CheckPermissionsRequest {
    pub resources_dir: String,
}

pub async fn get_config(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let cfg = state.config.read().unwrap().clone();
    Json(json!({
        "resources_dir": cfg.resources_dir,
        "flightmodels_dir": cfg.flightmodels_dir,
        "weaponpresets_dir": cfg.weaponpresets_dir,
        "sensors_dir": cfg.sensors_dir,
        "pkg_local_dir": cfg.pkg_local_dir,
        "pkg_local_weaponpresets_dir": cfg.pkg_local_weaponpresets_dir,
        "user_missions_dir": cfg.user_missions_dir,
        "datamine_root": cfg.datamine_root,
        "datamine_repo_url": "https://github.com/gszabi99/War-Thunder-Datamine",
        "shop_blkx_path": cfg.shop_blkx_path,
        "wpcost_blkx_path": cfg.wpcost_blkx_path,
        "units_csv_path": cfg.units_csv_path,
        "weaponry_csv_path": cfg.weaponry_csv_path,
        "tex_dir": cfg.tex_dir,
        "aircrafts_tex_dir": cfg.aircrafts_tex_dir,
        "atlases_dir": cfg.atlases_dir,
        "units_icons_dir": cfg.units_icons_dir,
        "gameuiskin_dir": cfg.gameuiskin_dir,
        "tankmodels_dir": cfg.tankmodels_dir,
        "tanks_tex_dir": cfg.tanks_tex_dir,
        "pkg_local_tankmodels_dir": cfg.pkg_local_tankmodels_dir,
        "pkg_local_ships_dir": cfg.pkg_local_ships_dir,
        "user_skins_dir": cfg.user_skins_dir,
        "language": cfg.language,
        "theme": cfg.theme,
        "os": std::env::consts::OS,
        "first_run_completed": cfg.first_run_completed,
        "detected_ok": cfg.detected_ok,
        "system_language": cfg.system_language.unwrap_or_else(crate::config::detect_system_language),
        "assets_installed": crate::config::are_assets_installed(),
    }))
}

pub async fn update_config(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ConfigUpdateRequest>,
) -> Json<serde_json::Value> {
    tracing::info!(
        "[ACTION.CONFIG_UPDATE] Settings update: lang={:?}, theme={:?}, first_run_completed={:?}, path={:?}",
        payload.language,
        payload.theme,
        payload.first_run_completed.or(payload.first_run),
        payload.resources_dir
    );

    let current_cfg = state.config.read().unwrap().clone();
    let updated_cfg = tokio::task::spawn_blocking(move || {
        let mut cfg = current_cfg;
        if let Some(lang) = payload.language {
            cfg.language = lang;
        }
        if let Some(theme) = payload.theme {
            cfg.theme = crate::config::normalize_theme(&theme);
        }
        if let Some(fr) = payload.first_run_completed.or(payload.first_run) {
            cfg.first_run_completed = fr;
        }
        if let Some(res) = payload.resources_dir {
            let clean = res.trim().to_string();
            if !clean.is_empty() {
                let re_detected = crate::config::detect_paths(Some(&clean));
                cfg.resources_dir = re_detected.resources_dir;
                cfg.user_missions_dir = re_detected.user_missions_dir;
                cfg.user_skins_dir = re_detected.user_skins_dir;
                cfg.pkg_local_dir = re_detected.pkg_local_dir;
                cfg.pkg_local_weaponpresets_dir = re_detected.pkg_local_weaponpresets_dir;
                cfg.pkg_local_tankmodels_dir = re_detected.pkg_local_tankmodels_dir;
                cfg.pkg_local_ships_dir = re_detected.pkg_local_ships_dir;
                cfg.flightmodels_dir = re_detected.flightmodels_dir;
                cfg.tankmodels_dir = re_detected.tankmodels_dir;
                cfg.detected_ok = re_detected.detected_ok;
            }
        }
        let _ = crate::config::save_config(&cfg);
        cfg
    })
    .await
    .unwrap_or_else(|_| state.config.read().unwrap().clone());

    if let Ok(mut g) = state.config.write() {
        *g = updated_cfg.clone();
    }
    state.repo.update_config(updated_cfg.clone());
    tracing::info!("[ACTION.CONFIG_SAVED] Config successfully written to disk.");

    Json(json!({
        "ok": true,
        "message": "Configuration updated successfully.",
        "detected_ok": updated_cfg.detected_ok,
        "resources_dir": updated_cfg.resources_dir,
        "user_missions_dir": updated_cfg.user_missions_dir,
        "theme": updated_cfg.theme,
    }))
}

pub async fn validate_directory(
    Json(payload): Json<ValidateDirectoryRequest>,
) -> Json<serde_json::Value> {
    let (valid, message, resolved) = crate::config::validate_game_directory(&payload.path);
    tracing::info!(
        "[ACTION.VALIDATE_PATH] Validating directory '{}': valid={}, resolved={:?}",
        payload.path,
        valid,
        resolved
    );
    Json(json!({
        "valid": valid,
        "message": message,
        "resolved_path": resolved,
    }))
}

pub async fn check_permissions(
    Json(payload): Json<CheckPermissionsRequest>,
) -> Json<serde_json::Value> {
    let target = payload.resources_dir.clone();
    let result =
        tokio::task::spawn_blocking(move || crate::config::check_directory_permissions(&target))
            .await
            .unwrap_or_else(|_| crate::config::PermissionCheckResult {
                valid_directory: false,
                user_missions_writable: false,
                pkg_local_writable: false,
                permissions_ok: false,
                message: "Permission check task failed.".to_string(),
                error_details: None,
                is_macos: cfg!(target_os = "macos"),
                os_name: std::env::consts::OS.to_string(),
            });

    tracing::info!(
        "[ACTION.CHECK_PERMISSIONS] Target: '{}' -> ok={}, writable_missions={}, writable_pkg={}",
        payload.resources_dir,
        result.permissions_ok,
        result.user_missions_writable,
        result.pkg_local_writable
    );
    Json(serde_json::to_value(&result).unwrap_or(json!({ "ok": false })))
}

pub async fn open_system_settings() -> Json<serde_json::Value> {
    #[cfg(target_os = "macos")]
    {
        let res = std::process::Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles")
            .spawn();
        if res.is_ok() {
            return Json(
                json!({ "ok": true, "message": "macOS Security & Privacy settings opened." }),
            );
        }
        let _ = std::process::Command::new("open")
            .arg("/System/Applications/System Settings.app")
            .spawn();
        return Json(json!({ "ok": true, "message": "macOS System Settings opened." }));
    }

    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("explorer.exe")
            .arg("ms-settings:privacy")
            .spawn();
        Json(json!({ "ok": true, "message": "Windows Privacy Settings opened." }))
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Json(json!({ "ok": false, "message": "Unsupported operating system." }))
    }
}

pub async fn browse_directory(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let initial_dir = state.config.read().unwrap().resources_dir.clone();

    let chosen_opt = tokio::task::spawn_blocking(move || {
        let mut dialog =
            rfd::FileDialog::new().set_title("Select War Thunder Installation Directory");
        if !initial_dir.is_empty() && std::path::Path::new(&initial_dir).exists() {
            dialog = dialog.set_directory(&initial_dir);
        }
        dialog.pick_folder()
    })
    .await
    .ok()
    .flatten();

    if let Some(folder) = chosen_opt {
        let p = folder.to_string_lossy().to_string();
        let perm = crate::config::check_directory_permissions(&p);
        return Json(json!({
            "ok": true,
            "cancelled": false,
            "path": p,
            "valid": perm.valid_directory,
            "permissions_ok": perm.permissions_ok,
            "message": perm.message,
            "error_details": perm.error_details,
            "is_macos": perm.is_macos,
        }));
    }

    Json(json!({ "ok": false, "cancelled": true, "path": null }))
}

pub async fn launch_game(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let res_dir = state.config.read().unwrap().resources_dir.clone();
    tracing::info!(
        "[ACTION.LAUNCH_GAME] Initiating War Thunder launch from: {:?}",
        res_dir
    );
    let (ok, msg) =
        tokio::task::spawn_blocking(move || crate::core::launcher::launch_game(&res_dir))
            .await
            .unwrap_or((false, "Launcher task failed.".to_string()));
    tracing::info!(
        "[ACTION.LAUNCH_GAME] Launch process result: ok={}, message={}",
        ok,
        msg
    );
    Json(json!({ "ok": ok, "message": msg }))
}

pub async fn get_setup_status(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let assets_installed = crate::config::are_assets_installed();
    if assets_installed {
        let live_cfg = state.repo.get_config();
        if let Ok(mut g) = state.config.write() {
            *g = live_cfg;
        }
    }
    let cfg = state.config.read().unwrap().clone();
    let download_progress = crate::core::asset_downloader::get_download_status();
    let repo = state.repo.clone();
    let total_vehicles = if assets_installed {
        tokio::task::spawn_blocking(move || repo.get_full_tree(false).vehicles.len())
            .await
            .unwrap_or(0)
    } else {
        0
    };

    Json(json!({
        "assets_installed": assets_installed,
        "assets_dir": crate::config::get_assets_dir().to_string_lossy(),
        "total_vehicles": total_vehicles,
        "first_run_completed": cfg.first_run_completed,
        "detected_ok": cfg.detected_ok,
        "resources_dir": cfg.resources_dir,
        "download_status": download_progress.status,
        "download_percent": download_progress.percent,
        "download_message": download_progress.message,
    }))
}

pub async fn start_assets_download(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    tracing::info!("[ACTION.SETUP_DOWNLOAD] Starting datamine download from GitHub CDN...");
    match crate::core::asset_downloader::start_download(state.repo.clone()) {
        Ok(_) => Json(json!({ "ok": true, "message": "Download task initiated." })),
        Err(e) => {
            tracing::error!("[ACTION.SETUP_DOWNLOAD] Download failed to start: {}", e);
            Json(json!({ "ok": false, "error": e }))
        }
    }
}

pub async fn get_assets_download_progress() -> Json<serde_json::Value> {
    let progress = crate::core::asset_downloader::get_download_status();
    Json(serde_json::to_value(&progress).unwrap_or(json!({ "status": "idle" })))
}

#[derive(Debug, Deserialize)]
pub struct ClientLogRequest {
    pub level: Option<String>,
    pub message: Option<String>,
    pub source: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct OpenUrlRequest {
    pub url: String,
}

pub async fn open_url(Json(payload): Json<OpenUrlRequest>) -> Json<serde_json::Value> {
    let url = payload.url.trim().to_string();
    if url.starts_with("http://") || url.starts_with("https://") {
        let ok = tokio::task::spawn_blocking(move || open::that(url).is_ok())
            .await
            .unwrap_or(false);
        Json(json!({ "ok": ok }))
    } else {
        Json(json!({ "ok": false, "error": "Invalid URL protocol" }))
    }
}

pub async fn log_client(Json(payload): Json<ClientLogRequest>) -> Json<serde_json::Value> {
    let lvl = payload.level.as_deref().unwrap_or("INFO");
    let msg = payload.message.as_deref().unwrap_or("");
    let src = payload.source.as_deref().unwrap_or("UI");
    tracing::info!("[{}.{}] {}", src, lvl, msg);
    Json(json!({ "ok": true }))
}

pub async fn open_logs_folder() -> Json<serde_json::Value> {
    let logs_dir = crate::config::get_logs_dir();
    let _ = std::fs::create_dir_all(&logs_dir);
    let path_str = logs_dir.to_string_lossy().to_string();
    let ok = tokio::task::spawn_blocking(move || open::that(&logs_dir).is_ok())
        .await
        .unwrap_or(false);
    Json(json!({ "ok": ok, "path": path_str }))
}

pub async fn open_config_folder() -> Json<serde_json::Value> {
    let cfg_path = crate::config::get_config_path();
    let target = cfg_path.parent().unwrap_or(&cfg_path).to_path_buf();
    let _ = std::fs::create_dir_all(&target);
    let path_str = target.to_string_lossy().to_string();
    let ok = tokio::task::spawn_blocking(move || open::that(&target).is_ok())
        .await
        .unwrap_or(false);
    Json(json!({ "ok": ok, "path": path_str }))
}
