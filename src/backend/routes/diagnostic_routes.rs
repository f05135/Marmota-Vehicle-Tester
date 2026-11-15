use crate::backend::AppState;
use axum::{extract::State, Json};
use serde_json::json;
use std::sync::Arc;

pub async fn generate_diagnostic(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let cfg = state.config.read().unwrap().clone();
    let repo = state.repo.clone();

    let result = tokio::task::spawn_blocking(move || {
        let missions =
            crate::core::mission_repository::MissionRepository::new(&cfg.user_missions_dir)
                .list_missions();
        let vehicles_count = repo.get_full_tree(false).total;
        crate::core::diagnostic::create_diagnostic_bundle(&cfg, missions.len(), vehicles_count)
    })
    .await
    .unwrap_or_else(|e| Err(e.to_string()));

    match result {
        Ok((zip_path, filename)) => {
            let zip_for_reveal = zip_path.clone();
            let revealed = tokio::task::spawn_blocking(move || {
                crate::core::diagnostic::reveal_in_os_file_manager(&zip_for_reveal)
            })
            .await
            .unwrap_or(false);
            let size_bytes = std::fs::metadata(&zip_path).map(|m| m.len()).unwrap_or(0);
            Json(json!({
                "ok": true,
                "filename": filename,
                "path": zip_path.to_string_lossy(),
                "size_bytes": size_bytes,
                "revealed_in_explorer": revealed,
                "github_issues_url": "https://github.com/f05135/Marmota-Vehicle-Tester/issues",
            }))
        }
        Err(e) => Json(json!({
            "ok": false,
            "error": e,
            "detail": "Failed to create diagnostic ZIP package."
        })),
    }
}
