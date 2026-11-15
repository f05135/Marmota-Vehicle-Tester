//! Embedded SQLite (WAL mode) persistence layer for ultra-fast vehicle catalog,
//! localized weaponry names, ballistic telemetry cache, and mission injection history.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use tracing::{info, warn};

static DB_INSTANCE: OnceLock<CatalogDatabase> = OnceLock::new();

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InjectionHistoryRecord {
    pub id: i64,
    pub created_at: u64,
    pub vehicle_id: String,
    pub custom_unit_class: String,
    pub preset_id: String,
    pub mission_file: String,
    pub backup_file: Option<String>,
    pub block_type: String,
}

pub struct CatalogDatabase {
    db_path: PathBuf,
    conn: Mutex<Connection>,
}

impl CatalogDatabase {
    pub fn get_instance() -> &'static Self {
        DB_INSTANCE.get_or_init(|| {
            #[cfg(test)]
            let db_path = std::env::temp_dir()
                .join(format!("marmota_unit_test_{}", std::process::id()))
                .join("marmota_catalog.db");

            #[cfg(not(test))]
            let db_path = {
                let assets_dir = crate::config::get_assets_dir();
                let _ = std::fs::create_dir_all(&assets_dir);
                assets_dir.join("marmota_catalog.db")
            };

            Self::open(&db_path).expect("Failed to open SQLite catalog database")
        })
    }

    pub fn open(db_path: &PathBuf) -> Result<Self, String> {
        if let Some(parent) = db_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let conn = Connection::open(db_path).map_err(|e| e.to_string())?;
        conn.execute_batch(
            "
            PRAGMA busy_timeout = 5000;
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            PRAGMA journal_size_limit = 16777216;
            PRAGMA temp_store = MEMORY;
            PRAGMA cache_size = -16000;
            PRAGMA foreign_keys = ON;

            CREATE TABLE IF NOT EXISTS meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS vehicles (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                names_json TEXT NOT NULL DEFAULT '{}',
                nation TEXT NOT NULL,
                branch TEXT NOT NULL,
                category TEXT NOT NULL,
                rank INTEGER NOT NULL,
                br TEXT NOT NULL,
                economic_rank INTEGER,
                tier_type TEXT NOT NULL,
                col INTEGER NOT NULL,
                prev_id TEXT,
                folder_group TEXT,
                unit_move_type TEXT,
                unit_class TEXT,
                ammo_mods_json TEXT NOT NULL DEFAULT '[]',
                telemetry_json TEXT NOT NULL DEFAULT '{}',
                sort_order INTEGER NOT NULL DEFAULT 0
            );
            CREATE INDEX IF NOT EXISTS idx_vehicles_branch_nation ON vehicles(branch, nation, rank, col);
            CREATE INDEX IF NOT EXISTS idx_vehicles_name ON vehicles(name);

            CREATE TABLE IF NOT EXISTS weaponry_loc (
                id TEXT PRIMARY KEY,
                names_json TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS vehicle_details (
                id TEXT PRIMARY KEY,
                detail_json TEXT NOT NULL,
                datamine_version TEXT NOT NULL DEFAULT '',
                updated_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS weapon_stats (
                path TEXT PRIMARY KEY,
                stats_json TEXT NOT NULL,
                updated_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS injection_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                created_at INTEGER NOT NULL,
                vehicle_id TEXT NOT NULL,
                custom_unit_class TEXT NOT NULL,
                preset_id TEXT NOT NULL,
                mission_file TEXT NOT NULL,
                backup_file TEXT,
                block_type TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_inj_created ON injection_history(created_at DESC);
            ",
        )
        .map_err(|e| e.to_string())?;

        info!("[DB.SQLITE] Initialized WAL database at {:?}", db_path);
        Ok(Self {
            db_path: db_path.clone(),
            conn: Mutex::new(conn),
        })
    }

    pub fn db_path(&self) -> &PathBuf {
        &self.db_path
    }

    pub fn get_meta(&self, key: &str) -> Option<String> {
        let conn = self.conn.lock().ok()?;
        conn.query_row("SELECT value FROM meta WHERE key = ?1", params![key], |r| {
            r.get(0)
        })
        .ok()
    }

    pub fn set_meta(&self, key: &str, value: &str) {
        if let Ok(conn) = self.conn.lock() {
            let _ = conn.execute(
                "INSERT INTO meta(key, value) VALUES(?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![key, value],
            );
        }
    }

    pub fn replace_vehicles(
        &self,
        vehicles: &[serde_json::Value],
        datamine_version: &str,
    ) -> Result<(), String> {
        let mut conn = self.conn.lock().map_err(|e| e.to_string())?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        tx.execute("DELETE FROM vehicles", [])
            .map_err(|e| e.to_string())?;

        {
            let mut stmt = tx
                .prepare(
                    "INSERT INTO vehicles (
                        id, name, names_json, nation, branch, category, rank, br, economic_rank,
                        tier_type, col, prev_id, folder_group, unit_move_type, unit_class,
                        ammo_mods_json, telemetry_json, sort_order
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
                )
                .map_err(|e| e.to_string())?;

            for (idx, v) in vehicles.iter().enumerate() {
                let id = v.get("id").and_then(|x| x.as_str()).unwrap_or("");
                if id.is_empty() {
                    continue;
                }
                let name = v.get("name").and_then(|x| x.as_str()).unwrap_or(id);
                let names_json = v
                    .get("names")
                    .map(|x| x.to_string())
                    .unwrap_or_else(|| "{}".to_string());
                let nation = v.get("nation").and_then(|x| x.as_str()).unwrap_or("USA");
                let branch = v
                    .get("branch")
                    .and_then(|x| x.as_str())
                    .unwrap_or("aviation");
                let category = v.get("cat").and_then(|x| x.as_str()).unwrap_or("FIGHTERS");
                let rank = v.get("rank").and_then(|x| x.as_i64()).unwrap_or(1);
                let br = v.get("br").and_then(|x| x.as_str()).unwrap_or("1.0");
                let econ_rank = v.get("economicRank").and_then(|x| x.as_i64());
                let tier_type = v
                    .get("tierType")
                    .and_then(|x| x.as_str())
                    .unwrap_or("regular");
                let col = v.get("col").and_then(|x| x.as_i64()).unwrap_or(0);
                let prev_id = v.get("prev").and_then(|x| x.as_str());
                let folder_group = v.get("folderGroup").and_then(|x| x.as_str());
                let unit_move_type = v.get("unitMoveType").and_then(|x| x.as_str());
                let unit_class = v.get("unitClass").and_then(|x| x.as_str());
                let ammo_mods_json = v
                    .get("ammoMods")
                    .map(|x| x.to_string())
                    .unwrap_or_else(|| "[]".to_string());
                let telemetry_json = v
                    .get("telemetry")
                    .map(|x| x.to_string())
                    .unwrap_or_else(|| "{}".to_string());

                stmt.execute(params![
                    id,
                    name,
                    names_json,
                    nation,
                    branch,
                    category,
                    rank,
                    br,
                    econ_rank,
                    tier_type,
                    col,
                    prev_id,
                    folder_group,
                    unit_move_type,
                    unit_class,
                    ammo_mods_json,
                    telemetry_json,
                    idx as i64
                ])
                .map_err(|e| e.to_string())?;
            }
        }

        tx.execute(
            "INSERT INTO meta(key, value) VALUES('datamine_version', ?1) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![datamine_version],
        )
        .map_err(|e| e.to_string())?;

        tx.commit().map_err(|e| e.to_string())?;
        let _ = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
        Ok(())
    }

    pub fn load_all_vehicles(&self) -> Vec<serde_json::Value> {
        let Ok(conn) = self.conn.lock() else {
            return Vec::new();
        };
        let Ok(mut stmt) = conn.prepare(
            "SELECT id, name, names_json, nation, branch, category, rank, br, economic_rank,
                    tier_type, col, prev_id, folder_group, unit_move_type, unit_class,
                    ammo_mods_json, telemetry_json
             FROM vehicles ORDER BY sort_order ASC",
        ) else {
            return Vec::new();
        };

        let rows = stmt.query_map([], |row| {
            let id: String = row.get(0)?;
            let name: String = row.get(1)?;
            let names_json: String = row.get(2)?;
            let nation: String = row.get(3)?;
            let branch: String = row.get(4)?;
            let category: String = row.get(5)?;
            let rank: i64 = row.get(6)?;
            let br: String = row.get(7)?;
            let economic_rank: Option<i64> = row.get(8)?;
            let tier_type: String = row.get(9)?;
            let col: i64 = row.get(10)?;
            let prev_id: Option<String> = row.get(11)?;
            let folder_group: Option<String> = row.get(12)?;
            let unit_move_type: Option<String> = row.get(13)?;
            let unit_class: Option<String> = row.get(14)?;
            let ammo_mods_json: String = row.get(15)?;
            let telemetry_json: String = row.get(16)?;

            let names: serde_json::Value =
                serde_json::from_str(&names_json).unwrap_or_else(|_| serde_json::json!({}));
            let ammo_mods: serde_json::Value =
                serde_json::from_str(&ammo_mods_json).unwrap_or_else(|_| serde_json::json!([]));
            let telemetry: serde_json::Value =
                serde_json::from_str(&telemetry_json).unwrap_or_else(|_| serde_json::json!({}));

            let mut card = serde_json::json!({
                "id": id,
                "name": name,
                "names": names,
                "rank": rank,
                "br": br,
                "economicRank": economic_rank,
                "nation": nation,
                "category": category,
                "cat": category,
                "branch": branch,
                "tierType": tier_type,
                "col": col,
                "prev": prev_id,
                "folderGroup": folder_group,
                "unitMoveType": unit_move_type,
                "unitClass": unit_class,
                "ammoMods": ammo_mods,
                "telemetry": telemetry,
            });
            if let Some(tel_obj) = telemetry.as_object() {
                if let Some(card_obj) = card.as_object_mut() {
                    for (k, val) in tel_obj {
                        card_obj.entry(k.clone()).or_insert_with(|| val.clone());
                    }
                }
            }
            Ok(card)
        });

        match rows {
            Ok(iter) => iter.filter_map(|r| r.ok()).collect(),
            Err(e) => {
                warn!("[DB.SQLITE] Failed querying vehicles: {}", e);
                Vec::new()
            }
        }
    }

    pub fn replace_weaponry_loc(
        &self,
        entries: &[(String, serde_json::Value)],
    ) -> Result<(), String> {
        let mut conn = self.conn.lock().map_err(|e| e.to_string())?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        tx.execute("DELETE FROM weaponry_loc", [])
            .map_err(|e| e.to_string())?;
        {
            let mut stmt = tx
                .prepare("INSERT INTO weaponry_loc(id, names_json) VALUES(?1, ?2) ON CONFLICT(id) DO UPDATE SET names_json = excluded.names_json")
                .map_err(|e| e.to_string())?;
            for (k, v) in entries {
                stmt.execute(params![k, v.to_string()])
                    .map_err(|e| e.to_string())?;
            }
        }
        tx.commit().map_err(|e| e.to_string())?;
        let _ = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
        Ok(())
    }

    pub fn lookup_weaponry_name(&self, key: &str, lang: &str) -> Option<String> {
        let conn = self.conn.lock().ok()?;
        let json_str: String = conn
            .query_row(
                "SELECT names_json FROM weaponry_loc WHERE id = ?1",
                params![key.to_lowercase()],
                |r| r.get(0),
            )
            .ok()?;
        let val: serde_json::Value = serde_json::from_str(&json_str).ok()?;
        val.get(lang)
            .or_else(|| val.get("en-US"))
            .and_then(|s| s.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
    }

    pub fn get_cached_vehicle_detail(
        &self,
        id: &str,
        expected_dm_version: &str,
    ) -> Option<serde_json::Value> {
        let conn = self.conn.lock().ok()?;
        let (json_str, ver): (String, String) = conn
            .query_row(
                "SELECT detail_json, datamine_version FROM vehicle_details WHERE id = ?1",
                params![id.to_lowercase()],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .ok()?;
        if !expected_dm_version.is_empty() && ver != expected_dm_version {
            return None;
        }
        serde_json::from_str(&json_str).ok()
    }

    pub fn save_cached_vehicle_detail(
        &self,
        id: &str,
        detail: &serde_json::Value,
        dm_version: &str,
    ) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        if let Ok(conn) = self.conn.lock() {
            let _ = conn.execute(
                "INSERT INTO vehicle_details(id, detail_json, datamine_version, updated_at)
                 VALUES(?1, ?2, ?3, ?4)
                 ON CONFLICT(id) DO UPDATE SET
                   detail_json = excluded.detail_json,
                   datamine_version = excluded.datamine_version,
                   updated_at = excluded.updated_at",
                params![id.to_lowercase(), detail.to_string(), dm_version, now],
            );
        }
    }

    pub fn invalidate_vehicle_detail(&self, id: &str) {
        if let Ok(conn) = self.conn.lock() {
            let _ = conn.execute(
                "DELETE FROM vehicle_details WHERE id = ?1",
                params![id.to_lowercase()],
            );
        }
    }

    pub fn invalidate_vehicle_details_cache(&self) {
        if let Ok(conn) = self.conn.lock() {
            let _ = conn.execute("DELETE FROM vehicle_details", []);
        }
    }

    pub fn get_weapon_stats(&self, path: &str) -> Option<serde_json::Value> {
        let conn = self.conn.lock().ok()?;
        let s: String = conn
            .query_row(
                "SELECT stats_json FROM weapon_stats WHERE path = ?1",
                params![path.to_lowercase()],
                |r| r.get(0),
            )
            .ok()?;
        serde_json::from_str(&s).ok()
    }

    pub fn save_weapon_stats(&self, path: &str, stats: &serde_json::Value) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        if let Ok(conn) = self.conn.lock() {
            let _ = conn.execute(
                "INSERT INTO weapon_stats(path, stats_json, updated_at)
                 VALUES(?1, ?2, ?3)
                 ON CONFLICT(path) DO UPDATE SET stats_json = excluded.stats_json, updated_at = excluded.updated_at",
                params![path.to_lowercase(), stats.to_string(), now],
            );
        }
    }

    pub fn record_injection(
        &self,
        vehicle_id: &str,
        custom_unit_class: &str,
        preset_id: &str,
        mission_file: &str,
        backup_file: Option<&str>,
        block_type: &str,
    ) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        if let Ok(conn) = self.conn.lock() {
            let _ = conn.execute(
                "INSERT INTO injection_history (created_at, vehicle_id, custom_unit_class, preset_id, mission_file, backup_file, block_type)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![now, vehicle_id, custom_unit_class, preset_id, mission_file, backup_file, block_type],
            );
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn get_recent_injections(&self, limit: usize) -> Vec<InjectionHistoryRecord> {
        let Ok(conn) = self.conn.lock() else {
            return Vec::new();
        };
        let Ok(mut stmt) = conn.prepare(
            "SELECT id, created_at, vehicle_id, custom_unit_class, preset_id, mission_file, backup_file, block_type
             FROM injection_history ORDER BY id DESC LIMIT ?1",
        ) else {
            return Vec::new();
        };
        let rows = stmt.query_map(params![limit as i64], |r| {
            Ok(InjectionHistoryRecord {
                id: r.get(0)?,
                created_at: r.get::<_, i64>(1)? as u64,
                vehicle_id: r.get(2)?,
                custom_unit_class: r.get(3)?,
                preset_id: r.get(4)?,
                mission_file: r.get(5)?,
                backup_file: r.get(6)?,
                block_type: r.get(7)?,
            })
        });
        match rows {
            Ok(iter) => iter.filter_map(|r| r.ok()).collect(),
            Err(_) => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_sqlite_wal_roundtrip_and_history() {
        let dir = std::env::temp_dir().join(format!(
            "wt_db_test_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::create_dir_all(&dir);
        let db_file = dir.join("test_catalog.db");
        let db = CatalogDatabase::open(&db_file).expect("open temp sqlite db");
        assert_eq!(db.db_path(), &db_file);

        db.set_meta("test_key", "v2.0.0");
        assert_eq!(db.get_meta("test_key").as_deref(), Some("v2.0.0"));

        let vehicles = vec![
            json!({
                "id": "fa_18e_block_2",
                "name": "F/A-18E",
                "names": { "en-US": "F/A-18E", "pt-BR": "F/A-18E" },
                "nation": "USA",
                "branch": "aviation",
                "cat": "FIGHTERS",
                "rank": 8,
                "br": "14.3",
                "tierType": "regular",
                "col": 1
            }),
            json!({
                "id": "us_destroyer_fletcher",
                "name": "USS Fletcher",
                "nation": "USA",
                "branch": "ships",
                "cat": "DESTROYERS",
                "rank": 3,
                "br": "4.7",
                "tierType": "regular",
                "col": 0
            }),
        ];
        db.replace_vehicles(&vehicles, "sha_123")
            .expect("insert vehicles");
        let loaded = db.load_all_vehicles();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0]["id"], "fa_18e_block_2");
        assert_eq!(loaded[1]["branch"], "ships");

        db.save_weapon_stats("weapons/aim_9m.blkx", &json!({ "maxOverloadG": 30.0 }));
        assert_eq!(
            db.get_weapon_stats("weapons/aim_9m.blkx")
                .and_then(|v| v.get("maxOverloadG").and_then(|x| x.as_f64())),
            Some(30.0)
        );

        db.save_cached_vehicle_detail(
            "fa_18e_block_2",
            &json!({ "id": "fa_18e_block_2" }),
            "sha_123",
        );
        assert!(db
            .get_cached_vehicle_detail("FA_18E_BLOCK_2", "sha_123")
            .is_some());
        db.invalidate_vehicle_detail("FA_18E_BLOCK_2");
        assert!(db
            .get_cached_vehicle_detail("fa_18e_block_2", "sha_123")
            .is_none());

        db.record_injection(
            "fa_18e_block_2",
            "custom_fa_18e_block_2",
            "custom_fa_18e_block_2_loadout",
            "test_plane_sample.blk",
            Some("test_plane_sample.blk.bak"),
            "armada",
        );
        let hist = db.get_recent_injections(5);
        assert_eq!(hist.len(), 1);
        assert_eq!(hist[0].vehicle_id, "fa_18e_block_2");

        drop(db);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
