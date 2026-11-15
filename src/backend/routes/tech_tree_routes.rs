use crate::backend::AppState;
use crate::core::vehicle_classifier::classify_vehicle;
use axum::{
    extract::{Path as AxPath, Query, State},
    Json,
};
use serde::Deserialize;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct TreeQuery {
    #[serde(default)]
    pub refresh: bool,
}

#[derive(Debug, Deserialize)]
pub struct DatamineCheckQuery {
    #[serde(default)]
    pub auto: bool,
}

#[derive(Debug, Deserialize)]
pub struct DatamineUpdateBody {
    #[serde(default)]
    pub force: bool,
}

pub async fn get_tree(
    State(state): State<Arc<AppState>>,
    Query(query): Query<TreeQuery>,
) -> Json<serde_json::Value> {
    let repo = state.repo.clone();
    let refresh = query.refresh;
    let tree = tokio::task::spawn_blocking(move || repo.get_full_tree(refresh))
        .await
        .unwrap_or_else(|_| state.repo.get_full_tree(refresh));

    Json(json!({
        "version": tree.version,
        "total": tree.total,
        "aviationCount": tree.aviation_count,
        "armyCount": tree.army_count,
        "shipsCount": tree.ships_count,
        "nations": tree.nations,
        "vehicles": tree.vehicles,
    }))
}

async fn ensure_vehicle_assets_ready(state: &Arc<AppState>, vid_clean: &str) {
    let repo = state.repo.clone();
    let cfg = state.config.read().unwrap().clone();
    let dm_dir = repo.get_datamine_dir();
    let vid_owned = vid_clean.to_string();
    let vid_low = vid_clean.to_lowercase();

    let (branch_str, local_file, rel_remote) = tokio::task::spawn_blocking(move || {
        let summary = repo.get_vehicle_summary(&vid_owned);
        let summary_branch = summary
            .as_ref()
            .and_then(|c| c.get("branch").and_then(|b| b.as_str()))
            .unwrap_or("");
        let classif = classify_vehicle(&vid_owned);
        let b_str = if !summary_branch.is_empty() {
            summary_branch.to_string()
        } else {
            classif.branch.as_str().to_string()
        };
        let (loc, rem) = match b_str.as_str() {
            "army" => (
                dm_dir.join("tankmodels").join(format!("{}.blkx", vid_low)),
                format!(
                    "aces.vromfs.bin_u/gamedata/units/tankmodels/{}.blkx",
                    vid_low
                ),
            ),
            "ships" => (
                dm_dir.join("ships").join(format!("{}.blkx", vid_low)),
                format!("aces.vromfs.bin_u/gamedata/units/ships/{}.blkx", vid_low),
            ),
            _ => (
                dm_dir
                    .join("flightmodels")
                    .join(format!("{}.blkx", vid_low)),
                format!("aces.vromfs.bin_u/gamedata/flightmodels/{}.blkx", vid_low),
            ),
        };
        (b_str, loc, rem)
    })
    .await
    .unwrap_or_else(|_| {
        let vid_low2 = vid_clean.to_lowercase();
        let dm_dir2 = state.repo.get_datamine_dir();
        (
            "aviation".to_string(),
            dm_dir2
                .join("flightmodels")
                .join(format!("{}.blkx", vid_low2)),
            format!("aces.vromfs.bin_u/gamedata/flightmodels/{}.blkx", vid_low2),
        )
    });

    let updater = crate::core::datamine_updater::DatamineUpdater::new(&cfg);
    if !local_file.exists() {
        let ok = updater.download_file(&rel_remote, &local_file).await;
        tracing::info!(
            "[ACTION.FETCH_MODEL] Downloaded BLKX model for '{}' ({}) -> ok={}",
            vid_clean,
            branch_str,
            ok
        );
    }

    let dm_dir_scan = state.repo.get_datamine_dir();
    let local_file_scan = local_file.clone();
    let branch_scan = branch_str.clone();
    let mut jobs: Vec<(String, PathBuf)> = tokio::task::spawn_blocking(move || {
        let mut collected: Vec<(String, PathBuf)> = Vec::new();
        if !local_file_scan.exists() {
            return collected;
        }
        let Ok(c) = std::fs::read_to_string(&local_file_scan) else {
            return collected;
        };
        let Ok(json_val) = serde_json::from_str::<serde_json::Value>(&c) else {
            return collected;
        };

        if branch_scan == "army" || branch_scan == "ships" {
            if let Some(cw) = json_val.get("commonWeapons").and_then(|w| w.get("Weapon")) {
                let list = if cw.is_array() { cw.as_array().cloned().unwrap_or_default() } else { vec![cw.clone()] };
                for w in list {
                    if w.get("dummy").and_then(|d| d.as_bool()).unwrap_or(false) { continue; }
                    if let Some(blk) = w.get("blk").and_then(|b| b.as_str()) {
                        let blk_low = blk.to_lowercase().replace('\\', "/");
                        let stem = Path::new(&blk_low).file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
                        if stem.is_empty() { continue; }
                        if blk_low.contains("navalmodels_weapons") {
                            let dest = dm_dir_scan.join("weapons/navalmodels_weapons").join(format!("{}.blkx", stem));
                            if !dest.exists() {
                                collected.push((format!("aces.vromfs.bin_u/gamedata/weapons/navalmodels_weapons/{}.blkx", stem), dest));
                            }
                        } else if blk_low.contains("groundmodels_weapons") {
                            let dest = dm_dir_scan.join("weapons/groundmodels_weapons").join(format!("{}.blkx", stem));
                            if !dest.exists() {
                                collected.push((format!("aces.vromfs.bin_u/gamedata/weapons/groundmodels_weapons/{}.blkx", stem), dest));
                            }
                        }
                    }
                }
            }
        } else {
            let wp_dir = dm_dir_scan.join("flightmodels/weaponpresets");
            if let Some(wp) = json_val.get("weapon_presets").and_then(|w| w.get("preset")) {
                let wp_list = if wp.is_array() { wp.as_array().cloned().unwrap_or_default() } else { vec![wp.clone()] };
                for p in &wp_list {
                    if let Some(p_blk) = p.get("blk").and_then(|b| b.as_str()) {
                        let stem = Path::new(p_blk).file_stem().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
                        if !stem.is_empty() {
                            let dest = wp_dir.join(format!("{}.blkx", stem));
                            if !dest.exists() {
                                collected.push((format!("aces.vromfs.bin_u/gamedata/flightmodels/weaponpresets/{}.blkx", stem), dest));
                            }
                        }
                    }
                }
            }
            if let Some(ws) = json_val.get("WeaponSlots").and_then(|w| w.get("WeaponSlot")) {
                let slots = if ws.is_array() { ws.as_array().cloned().unwrap_or_default() } else { vec![ws.clone()] };
                for s in slots {
                    if let Some(wp) = s.get("WeaponPreset") {
                        let p_list = if wp.is_array() { wp.as_array().cloned().unwrap_or_default() } else { vec![wp.clone()] };
                        for p in p_list {
                            if let Some(w_obj) = p.get("Weapon") {
                                let w_list = if w_obj.is_array() { w_obj.as_array().cloned().unwrap_or_default() } else { vec![w_obj.clone()] };
                                for w in w_list {
                                    if let Some(blk) = w.get("blk").and_then(|b| b.as_str()) {
                                        let rel_clean = blk.trim_start_matches('/').to_lowercase().replace('\\', "/");
                                        if let Some(after_gd) = rel_clean.strip_prefix("gamedata/") {
                                            let blkx_rel = if after_gd.ends_with(".blk") {
                                                format!("{}x", after_gd)
                                            } else {
                                                after_gd.to_string()
                                            };
                                            let dest = dm_dir_scan.join(&blkx_rel);
                                            if !dest.exists() {
                                                collected.push((format!("aces.vromfs.bin_u/gamedata/{}", blkx_rel), dest));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        collected
    })
    .await
    .unwrap_or_default();

    if !jobs.is_empty() {
        let mut seen = std::collections::HashSet::new();
        jobs.retain(|(_, dest)| seen.insert(dest.clone()));
        let count = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            updater.download_many(jobs, 12),
        )
        .await
        .unwrap_or(0);
        if count > 0 {
            tracing::info!(
                "[ACTION.SYNC_PRESETS] Synchronized {} referenced weapon/preset files for '{}'",
                count,
                vid_clean
            );
        }
    }
}

pub async fn get_vehicle(
    State(state): State<Arc<AppState>>,
    AxPath(vehicle_id): AxPath<String>,
    Query(query): Query<TreeQuery>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    let vid_clean = Path::new(vehicle_id.trim())
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(vehicle_id.trim())
        .to_string();

    if query.refresh {
        state.repo.invalidate_vehicle_detail(&vid_clean);
    }

    ensure_vehicle_assets_ready(&state, &vid_clean).await;

    let repo = state.repo.clone();
    let vid_for_task = vid_clean.clone();
    let detail_opt = tokio::task::spawn_blocking(move || repo.get_vehicle_detail(&vid_for_task))
        .await
        .ok()
        .flatten();

    if let Some(detail) = detail_opt {
        let preset_count = detail
            .get("presets")
            .and_then(|p| p.as_array())
            .map(|a| a.len())
            .unwrap_or(0);
        let pylon_count = detail
            .get("pylons")
            .and_then(|p| p.as_array())
            .map(|a| a.len())
            .unwrap_or(0);
        tracing::info!(
            "[ACTION.VEHICLE_LOADED] Vehicle '{}' ready: {} presets, {} pylons/slots",
            vid_clean,
            preset_count,
            pylon_count
        );
        Ok(Json(detail))
    } else {
        tracing::warn!(
            "[ACTION.VEHICLE_NOT_FOUND] Vehicle detail could not be generated for: '{}'",
            vid_clean
        );
        Err(axum::http::StatusCode::NOT_FOUND)
    }
}

pub async fn refresh_vehicles(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let repo = state.repo.clone();
    let tree = tokio::task::spawn_blocking(move || {
        crate::core::vehicle_classifier::VehicleClassifier::get_instance().reload_indexes();
        repo.get_full_tree(true)
    })
    .await
    .unwrap_or_else(|_| state.repo.get_full_tree(true));

    Json(json!({
        "ok": true,
        "total": tree.total,
        "aviationCount": tree.aviation_count,
        "armyCount": tree.army_count,
        "shipsCount": tree.ships_count,
    }))
}

pub async fn check_datamine(
    State(state): State<Arc<AppState>>,
    Query(q): Query<DatamineCheckQuery>,
) -> Json<serde_json::Value> {
    let cfg = state.config.read().unwrap().clone();
    let updater = crate::core::datamine_updater::DatamineUpdater::new(&cfg);
    let res = updater.check_for_updates(q.auto).await;
    Json(res)
}

pub async fn update_datamine(
    State(state): State<Arc<AppState>>,
    Json(body): Json<DatamineUpdateBody>,
) -> Json<serde_json::Value> {
    let cfg = state.config.read().unwrap().clone();
    let updater = crate::core::datamine_updater::DatamineUpdater::new(&cfg);
    let mut res = updater.perform_update(body.force).await;

    if res.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
        let repo = state.repo.clone();
        let tree = tokio::task::spawn_blocking(move || repo.get_full_tree(true))
            .await
            .unwrap_or_else(|_| state.repo.get_full_tree(true));
        if let Some(obj) = res.as_object_mut() {
            obj.insert("total".to_string(), json!(tree.total));
            obj.insert("aviationCount".to_string(), json!(tree.aviation_count));
            obj.insert("armyCount".to_string(), json!(tree.army_count));
            obj.insert("shipsCount".to_string(), json!(tree.ships_count));
        }
    }

    Json(res)
}
