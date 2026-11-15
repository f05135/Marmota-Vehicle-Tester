use crate::backend::AppState;
use crate::core::database::CatalogDatabase;
use crate::core::vehicle_classifier::{classify_vehicle, VehicleBranch};
use axum::{extract::State, Json};
use serde::Deserialize;
use serde_json::json;
use std::path::Path;
use std::sync::Arc;
use tracing::{info, warn};

#[derive(Debug, Deserialize)]
pub struct InjectRequest {
    pub vehicle_id: String,
    pub preset_id: Option<String>,
    pub preset_blk: Option<String>,
    pub missions: Vec<String>,
    #[serde(default)]
    pub bullets0: Option<String>,
    #[serde(default)]
    pub bullets1: Option<String>,
    #[serde(default)]
    pub bullets2: Option<String>,
    #[serde(default)]
    pub bullets3: Option<String>,
    #[serde(default)]
    pub bullets_count0: Option<i32>,
    #[serde(default)]
    pub bullets_count1: Option<i32>,
    #[serde(default)]
    pub bullets_count2: Option<i32>,
    #[serde(default)]
    pub bullets_count3: Option<i32>,
    pub skin: Option<String>,
}

pub async fn get_missions(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let cfg = state.config.read().unwrap().clone();
    let um_dir = cfg.user_missions_dir.clone();
    let missions = tokio::task::spawn_blocking(move || {
        let repo = crate::core::mission_repository::MissionRepository::new(&um_dir);
        repo.list_missions()
    })
    .await
    .unwrap_or_default();

    let mission_names: Vec<String> = missions.iter().map(|m| m.filename.clone()).collect();
    info!(
        "[ACTION.SCAN_MISSIONS] UserMissions directory: {:?} • Found {} valid .blk missions: {:?}",
        cfg.user_missions_dir,
        missions.len(),
        mission_names
    );
    Json(json!({
        "user_missions_dir": cfg.user_missions_dir,
        "missions": missions,
    }))
}

pub async fn inject_vehicle(
    State(state): State<Arc<AppState>>,
    Json(req): Json<InjectRequest>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    if req.missions.is_empty() {
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            "Select at least one target mission.".to_string(),
        ));
    }

    let cfg = state.config.read().unwrap().clone();
    let safe_vid = Path::new(&req.vehicle_id)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(&req.vehicle_id)
        .to_string();

    let repo_clone = state.repo.clone();
    let vid_clone = safe_vid.clone();
    let (detail, summary_branch, classification) = tokio::task::spawn_blocking(move || {
        let det = repo_clone.get_vehicle_detail(&vid_clone);
        let sum_branch = repo_clone.get_vehicle_summary(&vid_clone).and_then(|c| {
            c.get("branch")
                .and_then(|b| b.as_str())
                .map(|s| s.to_string())
        });
        let classif = classify_vehicle(&vid_clone);
        (det, sum_branch, classif)
    })
    .await
    .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let is_ship =
        summary_branch.as_deref() == Some("ships") || classification.branch == VehicleBranch::Naval;
    let is_ground =
        !is_ship && (summary_branch.as_deref() == Some("army") || classification.is_ground);
    let target_block_type = if is_ship {
        "ships"
    } else if is_ground {
        "tankModels"
    } else {
        "armada"
    };
    let exp_class = classification.exp_class.clone();
    let custom_unit_class = format!("custom_{}", safe_vid);

    let fallback_preset = req
        .preset_blk
        .as_deref()
        .and_then(|b| Path::new(b).file_stem().and_then(|s| s.to_str()))
        .unwrap_or("");
    let raw_weapons = req
        .preset_id
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(fallback_preset);
    let weapons = if (is_ground || is_ship)
        && (raw_weapons == "tank_weapon_preset_default"
            || raw_weapons == format!("{}_default", safe_vid))
    {
        ""
    } else {
        raw_weapons
    }
    .to_string();

    let default_ammo = detail
        .as_ref()
        .and_then(|d| d.get("ammoCount"))
        .and_then(|a| a.as_i64())
        .unwrap_or(40) as i32;
    let detail_belts = detail
        .as_ref()
        .and_then(|d| d.get("belts"))
        .and_then(|b| b.as_array());

    let pick_belt = |idx: usize, req_val: Option<&str>| -> Option<String> {
        if let Some(v) = req_val {
            let trimmed = v.trim();
            return if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            };
        }
        detail_belts
            .and_then(|arr| arr.get(idx))
            .and_then(|b| b.get("id"))
            .and_then(|i| i.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
    };

    let eff_bullet0 = pick_belt(0, req.bullets0.as_deref());
    let eff_bullet1 = if is_ground || is_ship {
        if req.bullets1.is_some() {
            pick_belt(1, req.bullets1.as_deref())
        } else {
            pick_belt(1, None).or_else(|| eff_bullet0.clone())
        }
    } else {
        None
    };
    let eff_bullet2 = if is_ground {
        pick_belt(2, req.bullets2.as_deref())
    } else {
        None
    };
    let eff_bullet3 = if is_ground {
        pick_belt(3, req.bullets3.as_deref())
    } else {
        None
    };

    let eff_c0 = if is_ground || is_ship {
        req.bullets_count0
            .filter(|&c| c > 0)
            .unwrap_or(default_ammo.max(1))
    } else {
        req.bullets_count0.unwrap_or(default_ammo)
    };
    let eff_c1 = req.bullets_count1.filter(|&c| c > 0).unwrap_or(eff_c0);
    let eff_c2 = req.bullets_count2.filter(|&c| c > 0).unwrap_or(20);
    let eff_c3 = req.bullets_count3.filter(|&c| c > 0).unwrap_or(20);

    let cfg_for_block = cfg.clone();
    let req_missions = req.missions.clone();
    let skin_clone = req.skin.clone();
    let weapons_clone = weapons.clone();
    let vid_for_gen = safe_vid.clone();
    let unit_class_clone = custom_unit_class.clone();

    let (succeeded, failed, details) = tokio::task::spawn_blocking(move || {
        let gen = crate::core::vehicle_generator::VehicleGenerator::new(&cfg_for_block);
        if let Err(e) = gen.generate_custom_vehicle_with_loadout(
            &vid_for_gen,
            Some(is_ground),
            Some(is_ship),
            Some(&exp_class),
            None,
        ) {
            warn!("║ [ACTION.GENERATE_BLK] Generator returned warning/error: {}", e);
        }

        let miss_repo = crate::core::mission_repository::MissionRepository::new(&cfg_for_block.user_missions_dir);
        let mut succeeded = Vec::new();
        let mut failed = Vec::new();
        let mut details = Vec::new();

        for m_filename in &req_missions {
            let Some(path) = miss_repo.resolve_mission_path(m_filename) else {
                warn!("║   ✗ Mission not found or outside UserMissions: {:?}", m_filename);
                failed.push(json!({ "mission": m_filename, "error": "Mission file not found inside UserMissions" }));
                details.push(json!({ "mission": m_filename, "ok": false }));
                continue;
            };

            let bullets: [Option<&str>; 4] = [
                eff_bullet0.as_deref(),
                eff_bullet1.as_deref(),
                eff_bullet2.as_deref(),
                eff_bullet3.as_deref(),
            ];
            let counts: [Option<i32>; 4] = if is_ground || is_ship {
                [Some(eff_c0), Some(eff_c1), Some(eff_c2), Some(eff_c3)]
            } else {
                [Some(eff_c0), None, None, None]
            };

            match miss_repo.inject_mission_unit_typed(
                &path,
                &unit_class_clone,
                Some(&weapons_clone),
                skin_clone.as_deref(),
                target_block_type,
                bullets,
                counts,
                false,
                None,
            ) {
                Ok(_) => {
                    CatalogDatabase::get_instance().record_injection(
                        &vid_for_gen,
                        &unit_class_clone,
                        &weapons_clone,
                        m_filename,
                        None,
                        target_block_type,
                    );
                    succeeded.push(m_filename.clone());
                    details.push(json!({ "mission": m_filename, "ok": true }));
                }
                Err(e) => {
                    warn!("║   ✗ Failed mission update for {:?}: {}", m_filename, e);
                    failed.push(json!({ "mission": m_filename, "error": e }));
                    details.push(json!({ "mission": m_filename, "ok": false }));
                }
            }
        }
        (succeeded, failed, details)
    })
    .await
    .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    info!(
        "[ACTION.APPLY_RESULT] {} ({}) -> {} succeeded, {} failed.",
        custom_unit_class,
        target_block_type,
        succeeded.len(),
        failed.len()
    );

    Ok(Json(json!({
        "ok": !succeeded.is_empty(),
        "vehicle_id": custom_unit_class,
        "weapons": weapons,
        "skin": req.skin.as_deref().unwrap_or("default"),
        "block_type": target_block_type,
        "injected_count": succeeded.len(),
        "affected_missions": succeeded,
        "mission_report": {
            "succeeded": succeeded,
            "failed": failed,
            "details": details,
        },
        "backup_created": false,
    })))
}

pub async fn open_folder(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let cfg = state.config.read().unwrap().clone();
    let um_dir = cfg.user_missions_dir.clone();
    let ok = tokio::task::spawn_blocking(move || {
        let repo = crate::core::mission_repository::MissionRepository::new(&um_dir);
        repo.open_folder()
    })
    .await
    .unwrap_or(false);
    Json(json!({
        "ok": ok,
        "path": cfg.user_missions_dir
    }))
}

pub async fn import_starters(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let cfg = state.config.read().unwrap().clone();
    let um_dir = cfg.user_missions_dir.clone();
    let res = tokio::task::spawn_blocking(move || {
        let repo = crate::core::mission_repository::MissionRepository::new(&um_dir);
        repo.import_starter_missions()
    })
    .await
    .unwrap_or_else(|e| Err(e.to_string()));

    match res {
        Ok(count) => Json(json!({ "ok": true, "count": count, "imported": count })),
        Err(e) => Json(json!({ "ok": false, "error": e.clone(), "detail": e })),
    }
}
