use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub resources_dir: String,
    #[serde(default)]
    pub flightmodels_dir: String,
    #[serde(default)]
    pub weaponpresets_dir: String,
    #[serde(default)]
    pub sensors_dir: String,
    #[serde(default)]
    pub pkg_local_dir: String,
    #[serde(default)]
    pub pkg_local_weaponpresets_dir: String,
    #[serde(default)]
    pub user_missions_dir: String,

    #[serde(default)]
    pub datamine_root: String,
    #[serde(default)]
    pub shop_blkx_path: String,
    #[serde(default)]
    pub wpcost_blkx_path: String,
    #[serde(default)]
    pub units_csv_path: String,
    #[serde(default)]
    pub weaponry_csv_path: String,

    #[serde(default)]
    pub tex_dir: String,
    #[serde(default)]
    pub aircrafts_tex_dir: String,
    #[serde(default)]
    pub atlases_dir: String,
    #[serde(default)]
    pub units_icons_dir: String,
    #[serde(default)]
    pub gameuiskin_dir: String,

    #[serde(default)]
    pub tankmodels_dir: String,
    #[serde(default)]
    pub tanks_tex_dir: String,
    #[serde(default)]
    pub pkg_local_tankmodels_dir: String,
    #[serde(default)]
    pub pkg_local_ships_dir: String,
    #[serde(default)]
    pub user_skins_dir: String,

    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default)]
    pub first_run_completed: bool,

    #[serde(default)]
    pub detected_ok: bool,
    #[serde(default)]
    pub system_language: Option<String>,
}

fn default_language() -> String {
    "en-US".to_string()
}

fn default_theme() -> String {
    "dark".to_string()
}

pub fn normalize_theme(raw: &str) -> String {
    match raw.trim().to_lowercase().as_str() {
        "light" => "light".to_string(),
        "auto" | "system" => "auto".to_string(),
        _ => "dark".to_string(),
    }
}

/// Resolves the project root in development mode by walking ancestors of the executable or CWD.
pub fn get_project_dir() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        let mut curr = exe.parent();
        for _ in 0..5 {
            if let Some(p) = curr {
                if p.join("ui").exists()
                    && (p.join("assets").exists() || p.join("Cargo.toml").exists())
                {
                    return p.to_path_buf();
                }
                curr = p.parent();
            } else {
                break;
            }
        }
    }
    if cfg!(debug_assertions) {
        if let Ok(cwd) = std::env::current_dir() {
            if cwd.join("ui").exists() {
                return cwd;
            }
        }
    }
    PathBuf::from(".")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeDirectories {
    pub is_packaged: bool,
    pub project_dir: PathBuf,
    pub app_data_dir: PathBuf,
    pub assets_dir: PathBuf,
    pub logs_dir: PathBuf,
    pub config_path: PathBuf,
    pub webview_data_dir: PathBuf,
}

fn parse_env_bool(val: &str) -> Option<bool> {
    let trimmed = val.trim();
    if trimmed == "1" || trimmed.eq_ignore_ascii_case("true") {
        Some(true)
    } else if trimmed == "0" || trimmed.eq_ignore_ascii_case("false") {
        Some(false)
    } else {
        None
    }
}

pub fn is_packaged_app() -> bool {
    for key in ["WT_FORCE_PACKAGED_PATHS", "MARMOTA_PACKAGED"] {
        if let Ok(v) = std::env::var(key) {
            if let Some(parsed) = parse_env_bool(&v) {
                return parsed;
            }
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        let exe_str = exe.to_string_lossy().replace('\\', "/");
        if exe_str.contains(".app/Contents/MacOS") {
            return true;
        }
    }
    if !cfg!(debug_assertions) {
        let launched_by_cargo = std::env::var_os("CARGO").is_some();
        if !launched_by_cargo {
            return true;
        }
        return !get_project_dir().join("Cargo.toml").exists();
    }
    !get_project_dir().join("Cargo.toml").exists()
}

/// Resolves bundled read-only resources beside a packaged `.app` or `.exe` executable, if present.
pub fn get_bundle_resources_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let exe_dir = exe.parent()?;
    let mac_resources = exe_dir.parent().map(|p| p.join("Resources"));
    if let Some(ref res) = mac_resources {
        if res.exists()
            && exe_dir
                .to_string_lossy()
                .replace('\\', "/")
                .contains(".app/Contents/MacOS")
        {
            return Some(res.clone());
        }
    }
    let sibling_res = exe_dir.join("resources");
    if sibling_res.exists() {
        return Some(sibling_res);
    }
    if is_packaged_app() {
        return Some(exe_dir.to_path_buf());
    }
    None
}

fn default_packaged_data_root() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("MarmotaVehicleTester")
}

fn default_packaged_webview_root() -> PathBuf {
    dirs::data_local_dir()
        .or_else(dirs::data_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("MarmotaVehicleTester")
}

pub fn resolve_runtime_directories_for(
    packaged: bool,
    project_dir: &Path,
    packaged_data_root: &Path,
    packaged_webview_root: &Path,
    data_override: Option<&str>,
) -> RuntimeDirectories {
    if let Some(raw) = data_override {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            let root = PathBuf::from(trimmed);
            return RuntimeDirectories {
                is_packaged: packaged,
                project_dir: project_dir.to_path_buf(),
                app_data_dir: root.clone(),
                assets_dir: root.join("assets"),
                logs_dir: root.join("logs"),
                config_path: root.join("config.json"),
                webview_data_dir: root.join("webview2"),
            };
        }
    }

    if packaged {
        RuntimeDirectories {
            is_packaged: true,
            project_dir: project_dir.to_path_buf(),
            app_data_dir: packaged_data_root.to_path_buf(),
            assets_dir: packaged_data_root.join("assets"),
            logs_dir: packaged_data_root.join("logs"),
            config_path: packaged_data_root.join("config.json"),
            webview_data_dir: packaged_webview_root.join("webview2"),
        }
    } else {
        RuntimeDirectories {
            is_packaged: false,
            project_dir: project_dir.to_path_buf(),
            app_data_dir: project_dir.to_path_buf(),
            assets_dir: project_dir.join("assets"),
            logs_dir: project_dir.join("logs"),
            config_path: project_dir.join("config.json"),
            webview_data_dir: project_dir.join("webview2"),
        }
    }
}

pub fn get_runtime_directories() -> RuntimeDirectories {
    let packaged = is_packaged_app();
    let proj_dir = get_project_dir();
    let pkg_data = default_packaged_data_root();
    let pkg_webview = default_packaged_webview_root();
    let override_env = std::env::var("MARMOTA_DATA_DIR").ok();
    resolve_runtime_directories_for(
        packaged,
        &proj_dir,
        &pkg_data,
        &pkg_webview,
        override_env.as_deref(),
    )
}

/// Ensures all writable runtime directories (config, assets, datamine, icons, logs, WebView profile) exist.
pub fn ensure_runtime_directories() -> RuntimeDirectories {
    let dirs = get_runtime_directories();
    let _ = fs::create_dir_all(&dirs.app_data_dir);
    let _ = fs::create_dir_all(dirs.assets_dir.join("datamine"));
    let _ = fs::create_dir_all(dirs.assets_dir.join("gameuiskin"));
    let _ = fs::create_dir_all(&dirs.logs_dir);
    let _ = fs::create_dir_all(&dirs.webview_data_dir);
    if let Some(cfg_parent) = dirs.config_path.parent() {
        let _ = fs::create_dir_all(cfg_parent);
    }
    dirs
}

pub fn get_app_data_dir() -> PathBuf {
    let dir = get_runtime_directories().app_data_dir;
    let _ = fs::create_dir_all(&dir);
    dir
}

pub fn get_assets_dir() -> PathBuf {
    let dir = get_runtime_directories().assets_dir;
    let _ = fs::create_dir_all(&dir);
    dir
}

pub fn are_assets_installed() -> bool {
    let assets = get_assets_dir();
    assets.join("datamine/units.csv").exists()
        && assets.join("datamine/shop.blkx").exists()
        && assets.join("datamine/wpcost.blkx").exists()
}

pub fn get_logs_dir() -> PathBuf {
    let dir = get_runtime_directories().logs_dir;
    let _ = fs::create_dir_all(&dir);
    dir
}

pub fn get_config_path() -> PathBuf {
    get_runtime_directories().config_path
}

pub fn get_webview_data_dir() -> PathBuf {
    let dir = get_runtime_directories().webview_data_dir;
    let _ = fs::create_dir_all(&dir);
    dir
}

impl AppConfig {
    pub fn webview_data_dir() -> PathBuf {
        get_webview_data_dir()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionCheckResult {
    pub valid_directory: bool,
    pub user_missions_writable: bool,
    pub pkg_local_writable: bool,
    pub permissions_ok: bool,
    pub message: String,
    pub error_details: Option<String>,
    pub is_macos: bool,
    pub os_name: String,
}

fn probe_writable_dir(dir: &Path, label: &str) -> Result<(), String> {
    if let Err(e) = fs::create_dir_all(dir) {
        return Err(format!(
            "Failed to access or create {} directory: {}",
            label, e
        ));
    }
    let probe = dir.join(format!(".marmota_perm_test_{}.tmp", std::process::id()));
    match fs::write(&probe, b"marmota_perm_check") {
        Ok(_) => {
            let _ = fs::read(&probe);
            let _ = fs::remove_file(&probe);
            Ok(())
        }
        Err(e) => Err(format!("{} write failed: {}", label, e)),
    }
}

pub fn resolve_game_root(base: &Path) -> PathBuf {
    if base.join("game").exists() {
        base.join("game")
    } else {
        base.to_path_buf()
    }
}

pub fn check_directory_permissions(raw_path: &str) -> PermissionCheckResult {
    let is_macos = cfg!(target_os = "macos");
    let os_name = std::env::consts::OS.to_string();
    let (valid, val_msg, resolved_opt) = validate_game_directory(raw_path);
    if !valid {
        return PermissionCheckResult {
            valid_directory: false,
            user_missions_writable: false,
            pkg_local_writable: false,
            permissions_ok: false,
            message: val_msg,
            error_details: None,
            is_macos,
            os_name,
        };
    }

    let target_dir = PathBuf::from(resolved_opt.as_deref().unwrap_or(raw_path));
    let game_root = resolve_game_root(&target_dir);

    let user_missions = game_root.join("UserMissions");
    if let Err(e) = probe_writable_dir(&user_missions, "UserMissions") {
        return PermissionCheckResult {
            valid_directory: true,
            user_missions_writable: false,
            pkg_local_writable: false,
            permissions_ok: false,
            message: "Write permission denied for UserMissions folder.".to_string(),
            error_details: Some(e),
            is_macos,
            os_name,
        };
    }

    let pkg_local = game_root.join("content/pkg_local/gameData");
    if let Err(e) = probe_writable_dir(&pkg_local, "content/pkg_local") {
        return PermissionCheckResult {
            valid_directory: true,
            user_missions_writable: true,
            pkg_local_writable: false,
            permissions_ok: false,
            message: "Write permission denied for content/pkg_local folder.".to_string(),
            error_details: Some(e),
            is_macos,
            os_name,
        };
    }

    PermissionCheckResult {
        valid_directory: true,
        user_missions_writable: true,
        pkg_local_writable: true,
        permissions_ok: true,
        message: "Read and write permissions verified successfully.".to_string(),
        error_details: None,
        is_macos,
        os_name,
    }
}

pub fn detect_system_language() -> String {
    if let Some(locale) = sys_locale::get_locale() {
        if let Some(code) = match_lang_code(&locale) {
            return code.to_string();
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = std::process::Command::new("defaults")
            .args(["read", "-g", "AppleLanguages"])
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    let cleaned = line.trim().trim_matches(|c| {
                        c == '"' || c == '\'' || c == '(' || c == ')' || c == ','
                    });
                    if !cleaned.is_empty() {
                        if let Some(code) = match_lang_code(cleaned) {
                            return code.to_string();
                        }
                    }
                }
            }
        }
    }

    for var in &["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(val) = std::env::var(var) {
            if let Some(code) = match_lang_code(&val) {
                return code.to_string();
            }
        }
    }

    "en-US".to_string()
}

pub fn match_lang_code(val: &str) -> Option<&'static str> {
    let lower = val.trim().to_lowercase().replace('_', "-");
    if lower.is_empty() {
        return None;
    }
    if lower.starts_with("pt") {
        return Some("pt-BR");
    }
    if lower.starts_with("en") {
        return Some("en-US");
    }
    if lower.starts_with("es") {
        return Some("es");
    }
    if lower.starts_with("ru") {
        return Some("ru");
    }
    if lower.starts_with("fr") {
        return Some("fr");
    }
    if lower.starts_with("zh") {
        return Some("zh");
    }
    if lower.starts_with("ja") || lower.starts_with("jp") {
        return Some("ja");
    }
    if lower.starts_with("de") {
        return Some("de");
    }
    None
}

pub fn validate_game_directory(path_str: &str) -> (bool, String, Option<String>) {
    let raw = path_str.trim();
    if raw.is_empty() {
        return (false, "Directory path cannot be empty.".to_string(), None);
    }
    let path = PathBuf::from(raw);
    if !path.exists() {
        return (false, format!("Directory does not exist: {}", raw), None);
    }

    // 1. Check if direct Resources folder on macOS
    let mac_direct = path.join("game");
    if mac_direct.exists()
        && (path.join("warThunder.icns").exists() || path.join("aces.vromfs.bin").exists())
    {
        return (
            true,
            "Valid War Thunder directory detected OK.".to_string(),
            Some(path.to_string_lossy().to_string()),
        );
    }

    // 2. Check if root containing WarThunderLauncher.app
    let mac_app = path.join("WarThunderLauncher.app/Contents/WarThunder.app/Contents/Resources");
    if mac_app.exists()
        && (mac_app.join("game").exists() || mac_app.join("warThunder.icns").exists())
    {
        return (
            true,
            "Valid War Thunder directory detected OK.".to_string(),
            Some(mac_app.to_string_lossy().to_string()),
        );
    }

    // 3. Direct game folder (Windows/Linux/Steam)
    let indicators = [
        "aces.vromfs.bin",
        "aces.vromfs.bin_u",
        "char.vromfs.bin",
        "launcher.exe",
        "aces.exe",
        "win64/aces.exe",
        "win32/aces.exe",
        "aces",
        "aces.x86_64",
        "UserMissions",
    ];
    for ind in &indicators {
        if path.join(ind).exists() {
            return (
                true,
                "Valid War Thunder directory detected OK.".to_string(),
                Some(path.to_string_lossy().to_string()),
            );
        }
    }

    (
        false,
        format!(
            "Folder '{}' does not appear to contain a valid War Thunder installation.",
            raw
        ),
        None,
    )
}

fn collect_steam_library_candidates() -> Vec<PathBuf> {
    let mut steam_roots: Vec<PathBuf> = Vec::new();
    let mut results: Vec<PathBuf> = Vec::new();

    #[cfg(windows)]
    {
        use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
        use winreg::RegKey;

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        if let Ok(key) = hkcu.open_subkey("Software\\Valve\\Steam") {
            if let Ok(p) = key.get_value::<String, _>("SteamPath") {
                steam_roots.push(PathBuf::from(p));
            }
        }
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        for sub in &[
            "SOFTWARE\\WOW6432Node\\Valve\\Steam",
            "SOFTWARE\\Valve\\Steam",
        ] {
            if let Ok(key) = hklm.open_subkey(sub) {
                if let Ok(p) = key.get_value::<String, _>("InstallPath") {
                    steam_roots.push(PathBuf::from(p));
                }
            }
        }
        steam_roots.push(PathBuf::from(r"C:\Program Files (x86)\Steam"));
        steam_roots.push(PathBuf::from(r"C:\Program Files\Steam"));

        // Also check Windows Uninstall registry for Gaijin/Steam War Thunder
        for root in [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER] {
            let rk = RegKey::predef(root);
            for uninstall_path in &[
                r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
                r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
            ] {
                if let Ok(u_key) = rk.open_subkey(uninstall_path) {
                    for name in u_key.enum_keys().flatten() {
                        if name.contains("War Thunder") || name.contains("236390") {
                            if let Ok(app_key) = u_key.open_subkey(&name) {
                                if let Ok(loc) = app_key.get_value::<String, _>("InstallLocation") {
                                    results.push(PathBuf::from(loc));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[cfg(not(windows))]
    {
        let home_dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        steam_roots.push(home_dir.join("Library/Application Support/Steam"));
        steam_roots.push(home_dir.join(".local/share/Steam"));
        steam_roots.push(home_dir.join(".steam/steam"));
    }

    let path_re = Regex::new(r#"(?i)"path"\s+"([^"]+)""#).ok();
    for root in steam_roots {
        results.push(root.join("steamapps/common/War Thunder"));
        let vdf_path = root.join("steamapps/libraryfolders.vdf");
        if vdf_path.exists() {
            if let Ok(content) = fs::read_to_string(&vdf_path) {
                if let Some(ref re) = path_re {
                    for cap in re.captures_iter(&content) {
                        if let Some(m) = cap.get(1) {
                            let lib_path = m.as_str().replace(r"\\", r"\");
                            results
                                .push(PathBuf::from(lib_path).join("steamapps/common/War Thunder"));
                        }
                    }
                }
            }
        }
    }

    results
}

fn build_install_candidates() -> Vec<PathBuf> {
    let mut candidates = collect_steam_library_candidates();
    let home_dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));

    #[cfg(windows)]
    {
        if let Some(local_app) = dirs::data_local_dir() {
            candidates.push(local_app.join("WarThunder"));
        }
        for drive in 'C'..='Z' {
            let root = format!("{}:\\", drive);
            if Path::new(&root).exists() {
                candidates.push(PathBuf::from(format!("{}WarThunder", root)));
                candidates.push(PathBuf::from(format!("{}War Thunder", root)));
                candidates.push(PathBuf::from(format!("{}Games\\War Thunder", root)));
                candidates.push(PathBuf::from(format!("{}Games\\WarThunder", root)));
                candidates.push(PathBuf::from(format!(
                    "{}SteamLibrary\\steamapps\\common\\War Thunder",
                    root
                )));
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        candidates.push(PathBuf::from("/Applications/WarThunderLauncher.app"));
        candidates.push(home_dir.join("Applications/WarThunderLauncher.app"));
    }

    #[cfg(target_os = "linux")]
    {
        candidates.push(home_dir.join("WarThunder"));
    }

    let mut unique = Vec::new();
    let mut seen = HashSet::new();
    for c in candidates {
        let key = c.to_string_lossy().to_lowercase();
        if seen.insert(key) {
            unique.push(c);
        }
    }
    let _ = home_dir;
    unique
}

pub fn detect_paths(custom_resources_dir: Option<&str>) -> AppConfig {
    let proj_dir = get_project_dir();
    let home_dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));

    let mut chosen_resources: Option<PathBuf> = None;
    let mut detected_ok = false;

    if let Some(custom) = custom_resources_dir {
        let (valid, _, resolved) = validate_game_directory(custom);
        if valid {
            if let Some(r) = resolved {
                chosen_resources = Some(PathBuf::from(r));
                detected_ok = true;
            }
        }
    }

    if chosen_resources.is_none() {
        for cand in build_install_candidates() {
            let (valid, _, resolved) = validate_game_directory(&cand.to_string_lossy());
            if valid {
                if let Some(r) = resolved {
                    chosen_resources = Some(PathBuf::from(r));
                    detected_ok = true;
                    break;
                }
            }
        }
    }

    let chosen =
        chosen_resources.unwrap_or_else(|| proj_dir.parent().unwrap_or(&proj_dir).to_path_buf());

    let assets_dir = get_assets_dir();
    let local_dm_dir = assets_dir.join("datamine");
    let local_air_dir = assets_dir.join("aircrafts");
    let local_skin_dir = assets_dir.join("gameuiskin");
    let local_fm_dir = local_dm_dir.join("flightmodels");
    let local_tank_dir = assets_dir.join("tanks");
    let local_tm_dir = local_dm_dir.join("tankmodels");

    let dm_fallback = home_dir.join("Downloads/War-Thunder-Datamine-master");

    let (fm_dir, wp_dir, sensors_dir) = if local_fm_dir.exists() {
        (
            local_fm_dir.clone(),
            local_fm_dir.join("weaponpresets"),
            local_dm_dir.join("sensors"),
        )
    } else if dm_fallback
        .join("aces.vromfs.bin_u/gamedata/flightmodels")
        .exists()
    {
        let fm = dm_fallback.join("aces.vromfs.bin_u/gamedata/flightmodels");
        let wp = fm.join("weaponpresets");
        let sn = dm_fallback.join("aces.vromfs.bin_u/gamedata/sensors");
        (fm, wp, sn)
    } else {
        (
            local_fm_dir,
            local_dm_dir.join("flightmodels/weaponpresets"),
            local_dm_dir.join("sensors"),
        )
    };

    let game_root = resolve_game_root(&chosen);

    let pkg_local_fm = game_root.join("content/pkg_local/gameData/flightModels");
    let pkg_local_wp = pkg_local_fm.join("weaponPresets");
    let pkg_local_tm = game_root.join("content/pkg_local/gameData/units/tankModels");
    let pkg_local_ships = game_root.join("content/pkg_local/gameData/units/ships");
    let user_missions = game_root.join("UserMissions");
    let mut user_skins = game_root.join("UserSkins");
    if !user_skins.exists() && chosen.join("UserSkins").exists() {
        user_skins = chosen.join("UserSkins");
    }

    let (shop_blkx, wpcost_blkx, units_csv, weaponry_csv, datamine_root) =
        if local_dm_dir.join("shop.blkx").exists() {
            (
                local_dm_dir.join("shop.blkx"),
                local_dm_dir.join("wpcost.blkx"),
                local_dm_dir.join("units.csv"),
                local_dm_dir.join("units_weaponry.csv"),
                local_dm_dir.clone(),
            )
        } else if dm_fallback
            .join("char.vromfs.bin_u/config/shop.blkx")
            .exists()
        {
            (
                dm_fallback.join("char.vromfs.bin_u/config/shop.blkx"),
                dm_fallback.join("char.vromfs.bin_u/config/wpcost.blkx"),
                dm_fallback.join("lang.vromfs.bin_u/lang/units.csv"),
                dm_fallback.join("lang.vromfs.bin_u/lang/units_weaponry.csv"),
                dm_fallback.clone(),
            )
        } else {
            (
                local_dm_dir.join("shop.blkx"),
                local_dm_dir.join("wpcost.blkx"),
                local_dm_dir.join("units.csv"),
                local_dm_dir.join("units_weaponry.csv"),
                local_dm_dir.clone(),
            )
        };

    let (tex_dir, aircrafts_tex, units_dir) = if local_air_dir.exists() {
        (
            assets_dir.clone(),
            local_air_dir.clone(),
            local_air_dir.clone(),
        )
    } else if dm_fallback.join("tex.vromfs.bin_u/aircrafts").exists() {
        let t = dm_fallback.join("tex.vromfs.bin_u");
        let a = t.join("aircrafts");
        let u = dm_fallback.join("atlases.vromfs.bin_u/units");
        (t, a, u)
    } else {
        (assets_dir.clone(), local_air_dir.clone(), local_air_dir)
    };

    let (atlases_dir, skin_dir) = if local_skin_dir.exists() {
        (assets_dir.clone(), local_skin_dir)
    } else if dm_fallback.join("atlases.vromfs.bin_u/gameuiskin").exists() {
        let a = dm_fallback.join("atlases.vromfs.bin_u");
        let s = a.join("gameuiskin");
        (a, s)
    } else {
        (assets_dir.clone(), local_skin_dir)
    };

    let tm_dir = if local_tm_dir.exists() {
        local_tm_dir
    } else if dm_fallback
        .join("aces.vromfs.bin_u/gamedata/units/tankmodels")
        .exists()
    {
        dm_fallback.join("aces.vromfs.bin_u/gamedata/units/tankmodels")
    } else {
        local_tm_dir
    };

    let tanks_tex = if local_tank_dir.exists() {
        local_tank_dir
    } else if dm_fallback.join("tex.vromfs.bin_u/tanks").exists() {
        dm_fallback.join("tex.vromfs.bin_u/tanks")
    } else {
        local_tank_dir
    };

    let sys_lang = detect_system_language();

    AppConfig {
        resources_dir: chosen.to_string_lossy().to_string(),
        flightmodels_dir: fm_dir.to_string_lossy().to_string(),
        weaponpresets_dir: wp_dir.to_string_lossy().to_string(),
        sensors_dir: sensors_dir.to_string_lossy().to_string(),
        pkg_local_dir: pkg_local_fm.to_string_lossy().to_string(),
        pkg_local_weaponpresets_dir: pkg_local_wp.to_string_lossy().to_string(),
        user_missions_dir: user_missions.to_string_lossy().to_string(),
        user_skins_dir: user_skins.to_string_lossy().to_string(),
        datamine_root: datamine_root.to_string_lossy().to_string(),
        shop_blkx_path: shop_blkx.to_string_lossy().to_string(),
        wpcost_blkx_path: wpcost_blkx.to_string_lossy().to_string(),
        units_csv_path: units_csv.to_string_lossy().to_string(),
        weaponry_csv_path: weaponry_csv.to_string_lossy().to_string(),
        tex_dir: tex_dir.to_string_lossy().to_string(),
        aircrafts_tex_dir: aircrafts_tex.to_string_lossy().to_string(),
        atlases_dir: atlases_dir.to_string_lossy().to_string(),
        units_icons_dir: units_dir.to_string_lossy().to_string(),
        gameuiskin_dir: skin_dir.to_string_lossy().to_string(),
        tankmodels_dir: tm_dir.to_string_lossy().to_string(),
        tanks_tex_dir: tanks_tex.to_string_lossy().to_string(),
        pkg_local_tankmodels_dir: pkg_local_tm.to_string_lossy().to_string(),
        pkg_local_ships_dir: pkg_local_ships.to_string_lossy().to_string(),
        language: sys_lang.clone(),
        theme: default_theme(),
        first_run_completed: false,
        detected_ok,
        system_language: Some(sys_lang),
    }
}

pub fn load_config() -> AppConfig {
    let config_path = get_config_path();
    if config_path.exists() {
        if let Ok(content) = fs::read_to_string(&config_path) {
            if let Ok(mut cfg) = serde_json::from_str::<AppConfig>(&content) {
                let (valid, _, _) = validate_game_directory(&cfg.resources_dir);
                if !valid {
                    // Self-heal stale paths (e.g., copied config from another OS or moved drive)
                    let healed = detect_paths(None);
                    if healed.detected_ok {
                        cfg.resources_dir = healed.resources_dir;
                        cfg.user_missions_dir = healed.user_missions_dir;
                        cfg.user_skins_dir = healed.user_skins_dir;
                        cfg.pkg_local_dir = healed.pkg_local_dir;
                        cfg.pkg_local_weaponpresets_dir = healed.pkg_local_weaponpresets_dir;
                        cfg.pkg_local_tankmodels_dir = healed.pkg_local_tankmodels_dir;
                        cfg.pkg_local_ships_dir = healed.pkg_local_ships_dir;
                        cfg.detected_ok = true;
                        let _ = save_config(&cfg);
                    } else {
                        cfg.detected_ok = false;
                    }
                } else {
                    cfg.detected_ok = true;
                    if cfg.pkg_local_ships_dir.is_empty() {
                        let game_root = resolve_game_root(Path::new(&cfg.resources_dir));
                        cfg.pkg_local_ships_dir = game_root
                            .join("content/pkg_local/gameData/units/ships")
                            .to_string_lossy()
                            .to_string();
                    }
                }
                cfg.system_language = Some(detect_system_language());
                return cfg;
            }
        }
    }

    let default_cfg = detect_paths(None);
    let _ = save_config(&default_cfg);
    default_cfg
}

pub fn save_config(cfg: &AppConfig) -> Result<(), String> {
    let config_path = get_config_path();
    if let Some(parent) = config_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let json_bytes = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    fs::write(&config_path, json_bytes).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_match_lang_code_and_directory_validation() {
        assert_eq!(match_lang_code("pt_BR"), Some("pt-BR"));
        assert_eq!(match_lang_code("en-US"), Some("en-US"));
        assert_eq!(match_lang_code("es-AR"), Some("es"));
        assert_eq!(match_lang_code("ru_RU"), Some("ru"));
        assert_eq!(match_lang_code("fr_FR"), Some("fr"));
        assert_eq!(match_lang_code("zh-CN"), Some("zh"));
        assert_eq!(match_lang_code("zh_TW"), Some("zh"));
        assert_eq!(match_lang_code("ja_JP"), Some("ja"));

        let (ok_empty, _, _) = validate_game_directory("");
        assert!(!ok_empty);

        let temp_wt = std::env::temp_dir().join(format!("wt_cfg_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(temp_wt.join("win64"));
        let _ = std::fs::write(temp_wt.join("win64/aces.exe"), b"stub");
        let (ok_syn, _, resolved) = validate_game_directory(temp_wt.to_str().unwrap());
        assert!(ok_syn);
        assert!(resolved.is_some());
        let _ = std::fs::remove_dir_all(&temp_wt);
    }

    #[test]
    fn test_dev_vs_packaged_directory_matrix() {
        let proj = PathBuf::from("/workspace/wt_customer");
        let pkg_data =
            PathBuf::from("/Users/test/Library/Application Support/MarmotaVehicleTester");
        let pkg_web = PathBuf::from("/Users/test/Library/Application Support/MarmotaVehicleTester");

        let dev_dirs = resolve_runtime_directories_for(false, &proj, &pkg_data, &pkg_web, None);
        assert!(!dev_dirs.is_packaged);
        assert_eq!(dev_dirs.app_data_dir, proj);
        assert_eq!(dev_dirs.assets_dir, proj.join("assets"));
        assert_eq!(dev_dirs.logs_dir, proj.join("logs"));
        assert_eq!(dev_dirs.config_path, proj.join("config.json"));
        assert_eq!(dev_dirs.webview_data_dir, proj.join("webview2"));

        let pkg_dirs = resolve_runtime_directories_for(true, &proj, &pkg_data, &pkg_web, None);
        assert!(pkg_dirs.is_packaged);
        assert_eq!(pkg_dirs.app_data_dir, pkg_data);
        assert_eq!(pkg_dirs.assets_dir, pkg_data.join("assets"));
        assert_eq!(pkg_dirs.logs_dir, pkg_data.join("logs"));
        assert_eq!(pkg_dirs.config_path, pkg_data.join("config.json"));
        assert_eq!(pkg_dirs.webview_data_dir, pkg_web.join("webview2"));

        let custom = resolve_runtime_directories_for(
            true,
            &proj,
            &pkg_data,
            &pkg_web,
            Some("/tmp/marmota_override"),
        );
        assert_eq!(
            custom.config_path,
            PathBuf::from("/tmp/marmota_override/config.json")
        );
        assert_eq!(
            custom.assets_dir,
            PathBuf::from("/tmp/marmota_override/assets")
        );
    }
}
