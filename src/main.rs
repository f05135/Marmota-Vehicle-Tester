#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod backend;
mod config;
mod core;
mod gui;

use std::sync::{Arc, RwLock};
use tracing::info;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Strip macOS LaunchServices process serial number flag (passed when double-clicking a .app bundle)
    let args: Vec<String> = std::env::args()
        .filter(|a| !a.starts_with("-psn_"))
        .collect();

    let runtime_dirs = config::ensure_runtime_directories();
    let _logs_dir = core::logger::init_logging();

    info!(
        "[STARTUP] mode={}, os={}, arch={}, config={:?}, assets={:?}, logs={:?}, webview={:?}",
        if runtime_dirs.is_packaged {
            "PACKAGED"
        } else {
            "DEV"
        },
        std::env::consts::OS,
        std::env::consts::ARCH,
        runtime_dirs.config_path,
        runtime_dirs.assets_dir,
        runtime_dirs.logs_dir,
        runtime_dirs.webview_data_dir
    );

    let cfg = config::load_config();
    let repo = Arc::new(core::vehicle_repository::VehicleRepository::new(
        cfg.clone(),
    ));

    // Eagerly warm up unified Tech Tree cache & compact classifier if assets are installed
    let repo_warm = repo.clone();
    std::thread::spawn(move || {
        if crate::config::are_assets_installed() {
            let _ = repo_warm.get_full_tree(false);
            let _ = crate::core::vehicle_classifier::VehicleClassifier::get_instance();
        }
    });

    let force_browser = args.iter().any(|a| a == "--browser");
    let headless = args.iter().any(|a| a == "--headless");
    let is_desktop_gui = !force_browser && !headless;

    // Generate a secure session token for native desktop mode to block unauthorized browser access
    let session_token = if is_desktop_gui {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(std::process::id().to_le_bytes());
        hasher.update(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
                .to_le_bytes(),
        );
        Some(format!("{:x}", hasher.finalize())[..32].to_string())
    } else {
        None
    };

    // Create AppState
    let state = Arc::new(backend::AppState {
        config: RwLock::new(cfg),
        repo: repo.clone(),
        session_token: session_token.clone(),
    });

    let router = backend::create_router(state);

    // Create a multi-threaded Tokio runtime for background server operations
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    let requested_port: Option<u16> = args.windows(2).find_map(|w| {
        if w[0] == "--port" {
            w[1].parse::<u16>().ok()
        } else {
            None
        }
    });

    // In desktop GUI mode, ALWAYS use ephemeral port 0 (OS assigns private random port)
    // In headless or browser developer mode, attempt 8080 or requested port
    let listener = rt.block_on(async {
        if let Some(p) = requested_port {
            tokio::net::TcpListener::bind(format!("127.0.0.1:{}", p)).await
        } else if headless || force_browser {
            match tokio::net::TcpListener::bind("127.0.0.1:8080").await {
                Ok(l) => Ok(l),
                Err(_) => tokio::net::TcpListener::bind("127.0.0.1:0").await,
            }
        } else {
            tokio::net::TcpListener::bind("127.0.0.1:0").await
        }
    })?;

    let local_addr = listener.local_addr()?;
    let base_url = format!("http://127.0.0.1:{}", local_addr.port());
    let launch_url = if let Some(ref tok) = session_token {
        format!("{}/?token={}", base_url, tok)
    } else {
        base_url.clone()
    };
    info!("Embedded Axum HTTP Server active on {}", base_url);

    // Spawn server in runtime
    rt.spawn(async move {
        let _ = axum::serve(listener, router).await;
    });

    if headless {
        info!("Running in headless mode (--headless flag detected)...");
        println!("Marmota Vehicle Tester v2.0.0 [Headless Mode]");
        println!("Server running at {}", base_url);
        println!("Press Ctrl+C to stop.");
        loop {
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
    } else if force_browser {
        info!("Launching in browser mode (--browser flag detected)...");
        let _ = webbrowser::open(&base_url);
        println!("Marmota Vehicle Tester v2.0.0");
        println!("Server running at {}", base_url);
        println!("Press Ctrl+C to stop.");
        // Keep running until interrupt
        loop {
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
    } else {
        info!("Launching native desktop WebView GUI...");
        if let Err(e) = gui::launch_gui(&launch_url) {
            info!(
                "Native GUI launch failed ({:?}); falling back to default browser...",
                e
            );
            let _ = webbrowser::open(&launch_url);
            loop {
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
        }
    }

    Ok(())
}
