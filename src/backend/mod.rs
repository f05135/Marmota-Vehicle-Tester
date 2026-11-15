pub mod routes;

use axum::{
    body::Body,
    extract::{Path as AxPath, State},
    http::{header, HeaderName, HeaderValue, Method, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use rust_embed::Embed;
use std::sync::{Arc, RwLock};
use tower_http::cors::{AllowOrigin, CorsLayer};

#[derive(Embed)]
#[folder = "ui/"]
pub struct EmbeddedUi;

pub struct AppState {
    pub config: RwLock<crate::config::AppConfig>,
    pub repo: Arc<crate::core::vehicle_repository::VehicleRepository>,
    pub session_token: Option<String>,
}

async fn request_logger(req: axum::extract::Request, next: Next) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let start = std::time::Instant::now();
    let resp = next.run(req).await;
    let duration = start.elapsed();
    let status = resp.status();

    if path.starts_with("/api") {
        tracing::info!(
            "[HTTP] {} {} -> {} ({:.2?})",
            method,
            path,
            status.as_u16(),
            duration
        );
    } else {
        tracing::trace!(
            "[HTTP.STATIC] {} {} -> {} ({:.2?})",
            method,
            path,
            status.as_u16(),
            duration
        );
    }
    resp
}

async fn auth_guard(
    State(state): State<Arc<AppState>>,
    req: axum::extract::Request,
    next: Next,
) -> Result<Response, (StatusCode, &'static str)> {
    let path = req.uri().path().to_string();

    if let Some(ref required_token) = state.session_token {
        let uri = req.uri();
        let query_has_token = uri
            .query()
            .map(|q| q.contains(&format!("token={}", required_token)))
            .unwrap_or(false);
        let header_has_token = req
            .headers()
            .get("X-Session-Token")
            .and_then(|v| v.to_str().ok())
            .map(|t| t == required_token)
            .unwrap_or(false);
        let cookie_has_token = req
            .headers()
            .get(header::COOKIE)
            .and_then(|v| v.to_str().ok())
            .map(|c| c.contains(&format!("wt_token={}", required_token)))
            .unwrap_or(false);

        if !path.starts_with("/api") || path == "/api/log" {
            let mut resp = next.run(req).await;
            if query_has_token && !cookie_has_token {
                let cookie_val = format!(
                    "wt_token={}; Path=/; SameSite=Lax; HttpOnly",
                    required_token
                );
                if let Ok(hdr) = HeaderValue::from_str(&cookie_val) {
                    resp.headers_mut().insert(header::SET_COOKIE, hdr);
                }
            }
            return Ok(resp);
        }

        if !query_has_token && !header_has_token && !cookie_has_token {
            tracing::warn!(
                "[AUTH.403] Unauthorized access attempt to {} (missing or invalid session token)",
                path
            );
            return Err((
                StatusCode::FORBIDDEN,
                "Access Forbidden: This manager is restricted to the native desktop application window.",
            ));
        }

        let mut resp = next.run(req).await;
        if query_has_token && !cookie_has_token {
            let cookie_val = format!(
                "wt_token={}; Path=/; SameSite=Lax; HttpOnly",
                required_token
            );
            if let Ok(hdr) = HeaderValue::from_str(&cookie_val) {
                resp.headers_mut().insert(header::SET_COOKIE, hdr);
            }
        }
        Ok(resp)
    } else {
        Ok(next.run(req).await)
    }
}

async fn serve_embedded_file(path: &str) -> Response {
    let clean_path = path.trim_start_matches('/');
    if let Some(file) = EmbeddedUi::get(clean_path) {
        let mime = mime_guess::from_path(clean_path).first_or_octet_stream();
        Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, mime.as_ref())
            .header(header::CACHE_CONTROL, "no-cache")
            .body(Body::from(file.data))
            .unwrap()
    } else {
        Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("404 Not Found"))
            .unwrap()
    }
}

async fn serve_index() -> impl IntoResponse {
    if !crate::config::is_packaged_app() {
        let proj_dir = crate::config::get_project_dir();
        let index_file = proj_dir.join("ui/index.html");
        if let Ok(bytes) = tokio::fs::read(&index_file).await {
            return Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
                .header(header::CACHE_CONTROL, "no-cache")
                .body(Body::from(bytes))
                .unwrap();
        }
    }
    serve_embedded_file("index.html").await
}

async fn serve_ui(AxPath(path): AxPath<String>) -> impl IntoResponse {
    if path.contains("..") {
        return Response::builder()
            .status(StatusCode::FORBIDDEN)
            .body(Body::from("Forbidden"))
            .unwrap();
    }
    if !crate::config::is_packaged_app() {
        let proj_dir = crate::config::get_project_dir();
        let disk_file = proj_dir.join("ui").join(&path);
        if let Ok(bytes) = tokio::fs::read(&disk_file).await {
            let mime = mime_guess::from_path(&path).first_or_octet_stream();
            return Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime.as_ref())
                .header(header::CACHE_CONTROL, "no-cache")
                .body(Body::from(bytes))
                .unwrap();
        }
    }
    serve_embedded_file(&path).await
}

pub fn create_router(app_state: Arc<AppState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(|origin, _| {
            origin
                .to_str()
                .map(|s| {
                    s.starts_with("http://127.0.0.1:")
                        || s == "http://127.0.0.1"
                        || s.starts_with("http://localhost:")
                        || s == "http://localhost"
                })
                .unwrap_or(false)
        }))
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([
            header::CONTENT_TYPE,
            HeaderName::from_static("x-session-token"),
        ]);

    Router::new()
        // Config & System
        .route(
            "/api/config",
            get(routes::config_routes::get_config).post(routes::config_routes::update_config),
        )
        .route(
            "/api/validate-directory",
            post(routes::config_routes::validate_directory),
        )
        .route(
            "/api/browse-directory",
            post(routes::config_routes::browse_directory),
        )
        .route("/api/launch-game", post(routes::config_routes::launch_game))
        .route("/api/open-url", post(routes::config_routes::open_url))
        .route(
            "/api/open-logs-folder",
            post(routes::config_routes::open_logs_folder),
        )
        .route(
            "/api/open-config-folder",
            post(routes::config_routes::open_config_folder),
        )
        .route("/api/log", post(routes::config_routes::log_client))
        // Active Permissions & Deep-Link
        .route(
            "/api/check-permissions",
            post(routes::config_routes::check_permissions),
        )
        .route(
            "/api/open-system-settings",
            post(routes::config_routes::open_system_settings),
        )
        // First-Run Setup & Catalog Downloader
        .route(
            "/api/setup/status",
            get(routes::config_routes::get_setup_status),
        )
        .route(
            "/api/setup/start-download",
            post(routes::config_routes::start_assets_download),
        )
        .route(
            "/api/setup/download-progress",
            get(routes::config_routes::get_assets_download_progress),
        )
        // Tech Tree & Datamine
        .route("/api/tree", get(routes::tech_tree_routes::get_tree))
        .route(
            "/api/vehicle/{vehicle_id}",
            get(routes::tech_tree_routes::get_vehicle),
        )
        .route(
            "/api/refresh-vehicles",
            post(routes::tech_tree_routes::refresh_vehicles),
        )
        .route(
            "/api/datamine/check",
            get(routes::tech_tree_routes::check_datamine),
        )
        .route(
            "/api/datamine/update",
            post(routes::tech_tree_routes::update_datamine),
        )
        // Missions
        .route("/api/missions", get(routes::mission_routes::get_missions))
        .route("/api/inject", post(routes::mission_routes::inject_vehicle))
        .route(
            "/api/open-folder",
            post(routes::mission_routes::open_folder),
        )
        .route(
            "/api/missions/open-folder",
            post(routes::mission_routes::open_folder),
        )
        .route(
            "/api/import-starters",
            post(routes::mission_routes::import_starters),
        )
        .route(
            "/api/missions/import-starters",
            post(routes::mission_routes::import_starters),
        )
        // Diagnostic
        .route(
            "/api/generate-diagnostic",
            post(routes::diagnostic_routes::generate_diagnostic),
        )
        // Assets & Branding
        .route(
            "/assets/aircraft/{filename}",
            get(routes::asset_routes::serve_aircraft_image),
        )
        .route(
            "/assets/aircrafts/{filename}",
            get(routes::asset_routes::serve_aircraft_image),
        )
        .route(
            "/assets/tank/{filename}",
            get(routes::asset_routes::serve_tank_image),
        )
        .route(
            "/assets/tanks/{filename}",
            get(routes::asset_routes::serve_tank_image),
        )
        .route(
            "/assets/ship/{filename}",
            get(routes::asset_routes::serve_ship_image),
        )
        .route(
            "/assets/ships/{filename}",
            get(routes::asset_routes::serve_ship_image),
        )
        .route(
            "/assets/weapon/{filename}",
            get(routes::asset_routes::serve_weapon_icon),
        )
        .route(
            "/assets/gameuiskin/{filename}",
            get(routes::asset_routes::serve_weapon_icon),
        )
        .route(
            "/assets/branding/{filename}",
            get(routes::asset_routes::serve_branding_image),
        )
        .route(
            "/favicon.ico",
            get(|| async {
                routes::asset_routes::serve_branding_image(AxPath("logo.png".to_string())).await
            }),
        )
        // Static UI Files
        .route("/ui/{*path}", get(serve_ui))
        .route("/", get(serve_index))
        .layer(middleware::from_fn_with_state(
            app_state.clone(),
            auth_guard,
        ))
        .layer(middleware::from_fn(request_logger))
        .layer(cors)
        .with_state(app_state)
}
