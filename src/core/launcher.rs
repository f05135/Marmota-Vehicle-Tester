use std::path::PathBuf;
use std::process::Command;
use tracing::{info, warn};

pub const STEAM_APP_ID: &str = "236390";

pub fn launch_game(resources_dir: &str) -> (bool, String) {
    info!(
        "Attempting to launch War Thunder (os={})...",
        std::env::consts::OS
    );
    let steam_uri = format!("steam://rungameid/{}", STEAM_APP_ID);

    let is_steam_install = resources_dir.to_lowercase().contains("steamapps");

    // 1. Primary: Steam URI when installed via Steam or as first option
    if is_steam_install {
        #[cfg(target_os = "macos")]
        {
            if let Ok(mut child) = Command::new("open").arg(&steam_uri).spawn() {
                let _ = child.wait();
                info!("Launched War Thunder via macOS 'open steam://...'");
                return (true, "War Thunder launch initiated via Steam.".to_string());
            }
        }

        #[cfg(target_os = "windows")]
        {
            if open::that(&steam_uri).is_ok() {
                info!("Launched War Thunder via Windows Steam URI.");
                return (true, "War Thunder launch initiated via Steam.".to_string());
            }
        }

        #[cfg(target_os = "linux")]
        {
            if let Ok(mut child) = Command::new("xdg-open").arg(&steam_uri).spawn() {
                let _ = child.wait();
                info!("Launched War Thunder via Linux xdg-open steam://...");
                return (true, "War Thunder launch initiated via Steam.".to_string());
            }
        }
        warn!("Steam URI launch failed or not available. Trying local binary fallback...");
    }

    // 2. Local executable fallback
    if !resources_dir.is_empty() {
        let res_path = PathBuf::from(resources_dir);
        if res_path.exists() {
            #[cfg(target_os = "macos")]
            {
                let candidates = [
                    res_path
                        .ancestors()
                        .nth(4)
                        .map(|p| p.join("WarThunderLauncher.app")),
                    res_path
                        .ancestors()
                        .nth(2)
                        .map(std::path::Path::to_path_buf),
                    Some(PathBuf::from("/Applications/WarThunder.app")),
                ];
                for cand in candidates.into_iter().flatten() {
                    if cand.exists() && cand.to_string_lossy().ends_with(".app") {
                        if let Ok(mut child) = Command::new("open").arg(&cand).spawn() {
                            let _ = child.wait();
                            info!("Launched War Thunder via app: {:?}", cand);
                            return (
                                true,
                                format!(
                                    "War Thunder launched via {:?}",
                                    cand.file_name().unwrap_or_default()
                                ),
                            );
                        }
                    }
                }
            }

            #[cfg(target_os = "windows")]
            {
                let candidates = [
                    res_path.join("launcher.exe"),
                    res_path.join("win64/aces.exe"),
                    res_path.join("win32/aces.exe"),
                    res_path.join("aces.exe"),
                ];
                for p in &candidates {
                    if p.exists() {
                        let work_dir = p.parent().unwrap_or(&res_path);
                        if Command::new(p).current_dir(work_dir).spawn().is_ok() {
                            info!("Launched War Thunder binary: {:?}", p);
                            let name = p
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("War Thunder");
                            return (true, format!("War Thunder launched via {}", name));
                        }
                    }
                }
            }

            #[cfg(target_os = "linux")]
            {
                for bin_name in &["launcher", "linux64/aces", "aces", "aces.x86_64"] {
                    let p = res_path.join(bin_name);
                    if p.exists() {
                        if Command::new(&p).current_dir(&res_path).spawn().is_ok() {
                            info!("Launched War Thunder binary: {:?}", p);
                            return (true, format!("War Thunder launched via {}", bin_name));
                        }
                    }
                }
            }
        }
    }

    // 3. Final fallback: Steam URI even if directory wasn't matched as steamapps
    if !is_steam_install && open::that(&steam_uri).is_ok() {
        info!("Launched War Thunder via Steam URI fallback.");
        return (true, "War Thunder launch initiated via Steam.".to_string());
    }

    (
        false,
        "Could not launch War Thunder automatically. Please start the game via Steam or Launcher."
            .to_string(),
    )
}
