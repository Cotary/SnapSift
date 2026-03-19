use std::path::Path;
use std::sync::Mutex;

use rusqlite::{Connection, Result};

use crate::models::{DuplicateGroup, FileRecord, Project, ProjectDetail, SourceFolder};

pub struct Database {
    pub conn: Mutex<Connection>,
}

impl Database {
    pub fn new(app_data_dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(app_data_dir).ok();
        let db_path = app_data_dir.join("realphoto.db");
        let conn = Connection::open(db_path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        let db = Self {
            conn: Mutex::new(conn),
        };
        db.init_tables()?;
        Ok(db)
    }

    fn init_tables(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS projects (
                id          TEXT PRIMARY KEY,
                name        TEXT NOT NULL,
                target_dir  TEXT,
                created_at  TEXT NOT NULL,
                updated_at  TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS source_folders (
                id          TEXT PRIMARY KEY,
                project_id  TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                path        TEXT NOT NULL,
                created_at  TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS files (
                id          TEXT PRIMARY KEY,
                project_id  TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                path        TEXT NOT NULL,
                file_name   TEXT NOT NULL,
                file_size   INTEGER NOT NULL,
                file_type   TEXT NOT NULL,
                taken_at    TEXT,
                phash       INTEGER,
                md5         TEXT,
                group_id    TEXT,
                feature_vector BLOB,
                created_at  TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_files_project ON files(project_id);
            CREATE INDEX IF NOT EXISTS idx_files_phash ON files(phash);
            CREATE INDEX IF NOT EXISTS idx_files_md5 ON files(md5);
            CREATE INDEX IF NOT EXISTS idx_source_folders_project ON source_folders(project_id);
            ",
        )?;

        // Migration: add feature_vector column if missing (for existing databases)
        let has_col: bool = conn
            .prepare("SELECT feature_vector FROM files LIMIT 0")
            .is_ok();
        if !has_col {
            conn.execute_batch("ALTER TABLE files ADD COLUMN feature_vector BLOB;")?;
        }

        // Migration: ensure unique index on files.path
        let has_unique_idx: bool = conn
            .prepare("SELECT 1 FROM sqlite_master WHERE type='index' AND name='idx_files_path'")
            .and_then(|mut s| s.query_row([], |_| Ok(true)))
            .unwrap_or(false);
        if !has_unique_idx {
            // Remove duplicate path entries first, keeping the newest record per path
            conn.execute_batch(
                "DELETE FROM files WHERE id NOT IN (
                    SELECT MAX(id) FROM files GROUP BY path
                );"
            )?;
            conn.execute_batch(
                "CREATE UNIQUE INDEX idx_files_path ON files(path);"
            )?;
        }

        Ok(())
    }

    pub fn create_project(&self, name: &str) -> Result<Project> {
        let conn = self.conn.lock().unwrap();
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO projects (id, name, target_dir, created_at, updated_at) VALUES (?1, ?2, NULL, ?3, ?4)",
            rusqlite::params![id, name, now, now],
        )?;
        Ok(Project {
            id,
            name: name.to_string(),
            target_dir: None,
            created_at: now.clone(),
            updated_at: now,
        })
    }

    pub fn list_projects(&self) -> Result<Vec<Project>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT id, name, target_dir, created_at, updated_at FROM projects ORDER BY created_at DESC")?;
        let rows = stmt.query_map([], |row| {
            Ok(Project {
                id: row.get(0)?,
                name: row.get(1)?,
                target_dir: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        })?;
        rows.collect()
    }

    pub fn get_project(&self, project_id: &str) -> Result<Option<Project>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, target_dir, created_at, updated_at FROM projects WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(rusqlite::params![project_id], |row| {
            Ok(Project {
                id: row.get(0)?,
                name: row.get(1)?,
                target_dir: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        })?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    pub fn delete_project(&self, project_id: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let affected = conn.execute("DELETE FROM projects WHERE id = ?1", rusqlite::params![project_id])?;
        Ok(affected > 0)
    }

    pub fn set_target_dir(&self, project_id: &str, path: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        let affected = conn.execute(
            "UPDATE projects SET target_dir = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![path, now, project_id],
        )?;
        Ok(affected > 0)
    }

    pub fn add_source_folders(
        &self,
        project_id: &str,
        paths: &[String],
    ) -> Result<Vec<SourceFolder>> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        let mut folders = Vec::new();

        for path in paths {
            let existing: bool = conn.query_row(
                "SELECT COUNT(*) > 0 FROM source_folders WHERE project_id = ?1 AND path = ?2",
                rusqlite::params![project_id, path],
                |row| row.get(0),
            )?;
            if existing {
                continue;
            }

            let id = uuid::Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO source_folders (id, project_id, path, created_at) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![id, project_id, path, now],
            )?;
            folders.push(SourceFolder {
                id,
                project_id: project_id.to_string(),
                path: path.clone(),
                created_at: now.clone(),
            });
        }

        let updated_at = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE projects SET updated_at = ?1 WHERE id = ?2",
            rusqlite::params![updated_at, project_id],
        )?;

        Ok(folders)
    }

    pub fn get_source_folders(&self, project_id: &str) -> Result<Vec<SourceFolder>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, project_id, path, created_at FROM source_folders WHERE project_id = ?1 ORDER BY created_at",
        )?;
        let rows = stmt.query_map(rusqlite::params![project_id], |row| {
            Ok(SourceFolder {
                id: row.get(0)?,
                project_id: row.get(1)?,
                path: row.get(2)?,
                created_at: row.get(3)?,
            })
        })?;
        rows.collect()
    }

    pub fn remove_source_folder(&self, folder_id: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let affected = conn.execute(
            "DELETE FROM source_folders WHERE id = ?1",
            rusqlite::params![folder_id],
        )?;
        Ok(affected > 0)
    }

    pub fn get_project_detail(&self, project_id: &str) -> Result<Option<ProjectDetail>> {
        let project = self.get_project(project_id)?;
        match project {
            Some(project) => {
                let source_folders = self.get_source_folders(project_id)?;
                Ok(Some(ProjectDetail {
                    project,
                    source_folders,
                }))
            }
            None => Ok(None),
        }
    }

    pub fn get_existing_paths(&self, project_id: &str) -> Result<std::collections::HashSet<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT path FROM files WHERE project_id = ?1")?;
        let rows = stmt.query_map(rusqlite::params![project_id], |row| {
            row.get::<_, String>(0)
        })?;
        let mut set = std::collections::HashSet::new();
        for r in rows {
            set.insert(r?);
        }
        Ok(set)
    }

    pub fn insert_files_batch(&self, files: &[crate::models::FileRecord]) -> Result<u64> {
        let conn = self.conn.lock().unwrap();
        let mut inserted = 0u64;
        let tx_result: Result<()> = (|| {
            for file in files {
                let affected = conn.execute(
                    "INSERT OR IGNORE INTO files (id, project_id, path, file_name, file_size, file_type, taken_at, phash, md5, group_id, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                    rusqlite::params![
                        file.id, file.project_id, file.path, file.file_name,
                        file.file_size, file.file_type, file.taken_at, file.phash,
                        file.md5, file.group_id, file.created_at,
                    ],
                )?;
                inserted += affected as u64;
            }
            Ok(())
        })();
        tx_result?;
        Ok(inserted)
    }

    pub fn get_project_files(&self, project_id: &str) -> Result<Vec<FileRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, project_id, path, file_name, file_size, file_type, taken_at, phash, md5, group_id, created_at
             FROM files WHERE project_id = ?1 ORDER BY taken_at ASC",
        )?;
        let rows = stmt.query_map(rusqlite::params![project_id], |row| {
            Ok(FileRecord {
                id: row.get(0)?,
                project_id: row.get(1)?,
                path: row.get(2)?,
                file_name: row.get(3)?,
                file_size: row.get(4)?,
                file_type: row.get(5)?,
                taken_at: row.get(6)?,
                phash: row.get(7)?,
                md5: row.get(8)?,
                group_id: row.get(9)?,
                created_at: row.get(10)?,
            })
        })?;
        rows.collect()
    }

    pub fn get_files_with_phash(
        &self,
        project_id: &str,
        path_prefix: Option<&str>,
    ) -> Result<Vec<FileRecord>> {
        let conn = self.conn.lock().unwrap();
        let (sql, params): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = match path_prefix {
            Some(prefix) => {
                let like_pattern = format!("{}%", prefix.replace('\\', "/"));
                (
                    "SELECT id, project_id, path, file_name, file_size, file_type, taken_at, phash, md5, group_id, created_at
                     FROM files WHERE project_id = ?1 AND phash IS NOT NULL AND REPLACE(path, '\\', '/') LIKE ?2 ORDER BY phash".to_string(),
                    vec![
                        Box::new(project_id.to_string()) as Box<dyn rusqlite::types::ToSql>,
                        Box::new(like_pattern),
                    ],
                )
            }
            None => (
                "SELECT id, project_id, path, file_name, file_size, file_type, taken_at, phash, md5, group_id, created_at
                 FROM files WHERE project_id = ?1 AND phash IS NOT NULL ORDER BY phash".to_string(),
                vec![Box::new(project_id.to_string()) as Box<dyn rusqlite::types::ToSql>],
            ),
        };
        let mut stmt = conn.prepare(&sql)?;
        let params_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let rows = stmt.query_map(params_refs.as_slice(), |row| {
            Ok(FileRecord {
                id: row.get(0)?,
                project_id: row.get(1)?,
                path: row.get(2)?,
                file_name: row.get(3)?,
                file_size: row.get(4)?,
                file_type: row.get(5)?,
                taken_at: row.get(6)?,
                phash: row.get(7)?,
                md5: row.get(8)?,
                group_id: row.get(9)?,
                created_at: row.get(10)?,
            })
        })?;
        rows.collect()
    }

    pub fn update_group_id(&self, file_id: &str, group_id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE files SET group_id = ?1 WHERE id = ?2",
            rusqlite::params![group_id, file_id],
        )?;
        Ok(())
    }

    pub fn clear_group_ids(&self, project_id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE files SET group_id = NULL WHERE project_id = ?1",
            rusqlite::params![project_id],
        )?;
        Ok(())
    }

    pub fn get_duplicate_groups(&self, project_id: &str) -> Result<Vec<DuplicateGroup>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT DISTINCT group_id FROM files WHERE project_id = ?1 AND group_id IS NOT NULL ORDER BY group_id",
        )?;
        let group_ids: Vec<String> = stmt
            .query_map(rusqlite::params![project_id], |row| row.get(0))?
            .collect::<Result<Vec<String>>>()?;

        let mut groups = Vec::new();
        let mut file_stmt = conn.prepare(
            "SELECT id, project_id, path, file_name, file_size, file_type, taken_at, phash, md5, group_id, created_at
             FROM files WHERE project_id = ?1 AND group_id = ?2 ORDER BY file_size DESC",
        )?;

        for gid in group_ids {
            let files: Vec<FileRecord> = file_stmt
                .query_map(rusqlite::params![project_id, gid], |row| {
                    Ok(FileRecord {
                        id: row.get(0)?,
                        project_id: row.get(1)?,
                        path: row.get(2)?,
                        file_name: row.get(3)?,
                        file_size: row.get(4)?,
                        file_type: row.get(5)?,
                        taken_at: row.get(6)?,
                        phash: row.get(7)?,
                        md5: row.get(8)?,
                        group_id: row.get(9)?,
                        created_at: row.get(10)?,
                    })
                })?
                .collect::<Result<Vec<FileRecord>>>()?;

            if files.len() >= 2 {
                groups.push(DuplicateGroup {
                    group_id: gid,
                    files,
                });
            }
        }
        Ok(groups)
    }

    pub fn get_feature_vector(&self, file_id: &str) -> Result<Option<Vec<u8>>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT feature_vector FROM files WHERE id = ?1")?;
        let result: Option<Vec<u8>> = stmt
            .query_row(rusqlite::params![file_id], |row| row.get(0))
            .ok();
        Ok(result)
    }

    pub fn set_feature_vector(&self, file_id: &str, vector: &[u8]) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE files SET feature_vector = ?1 WHERE id = ?2",
            rusqlite::params![vector, file_id],
        )?;
        Ok(())
    }

    pub fn cleanup_stale_target_files(&self, project_id: &str, target_dir: &str) -> Result<u64> {
        let conn = self.conn.lock().unwrap();
        let normalized_target = target_dir.replace('\\', "/");
        let like_pattern = format!("{}%", normalized_target);
        let mut stmt = conn.prepare(
            "SELECT id, path FROM files WHERE project_id = ?1 AND REPLACE(path, '\\', '/') LIKE ?2",
        )?;
        let rows: Vec<(String, String)> = stmt
            .query_map(rusqlite::params![project_id, like_pattern], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>>>()?;

        let mut removed = 0u64;
        for (id, path) in &rows {
            if !std::path::Path::new(path).exists() {
                conn.execute("DELETE FROM files WHERE id = ?1", rusqlite::params![id])?;
                removed += 1;
            }
        }
        Ok(removed)
    }

    pub fn delete_files_by_paths(&self, paths: &[String], target_dir: &str) -> Result<u64> {
        let conn = self.conn.lock().unwrap();
        let normalized_target = target_dir.replace('\\', "/");
        let mut deleted = 0u64;
        for path in paths {
            let normalized_path = path.replace('\\', "/");
            if !normalized_path.starts_with(&normalized_target) {
                log::warn!("Skipping deletion of non-target file: {}", path);
                continue;
            }
            if std::path::Path::new(path).exists() {
                if let Err(e) = std::fs::remove_file(path) {
                    log::warn!("Failed to delete file {}: {}", path, e);
                    continue;
                }
            }
            let affected = conn.execute("DELETE FROM files WHERE path = ?1", rusqlite::params![path])?;
            deleted += affected as u64;
        }
        Ok(deleted)
    }
}
