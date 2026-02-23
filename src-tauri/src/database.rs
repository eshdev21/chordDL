use crate::error::{AppError, AppResult};
use crate::state::{DownloadStatus, DownloadTask};
use rusqlite::{params, Connection};
use tauri::{AppHandle, Manager};

pub struct Db {
    pub conn: Connection,
}

impl Db {
    /// Initialize a fresh database (delete existing if present)
    pub fn init_fresh(app: &AppHandle) -> AppResult<Self> {
        let app_local_dir = app
            .path()
            .app_local_data_dir()
            .map_err(|e| AppError::Internal(e.to_string()))?;

        let db_path = app_local_dir.join("chord.db");

        if db_path.exists() {
            // Close any existing connections? (Assuming this is startup, no connections exist yet)
            // Ideally we'd ensure no other process has it locked, but for single-instance app this is fine
            std::fs::remove_file(&db_path)
                .map_err(|e| AppError::Database(format!("Failed to delete old DB: {}", e)))?;
        }

        Self::new(app)
    }

    pub fn new(app: &AppHandle) -> AppResult<Self> {
        let app_local_dir = app
            .path()
            .app_local_data_dir()
            .map_err(|e| AppError::Internal(e.to_string()))?;

        if !app_local_dir.exists() {
            std::fs::create_dir_all(&app_local_dir).map_err(AppError::Io)?;
        }

        let db_path = app_local_dir.join("chord.db");
        let conn = Connection::open(db_path).map_err(|e| AppError::Database(e.to_string()))?;

        // 1. Performance: Enable Write-Ahead Logging for concurrent access
        let _ = conn.pragma_update(None, "journal_mode", "WAL");
        conn.busy_timeout(std::time::Duration::from_secs(5))
            .map_err(|e| AppError::Database(e.to_string()))?;

        // 2. Main tasks table schema (Stores UUID, URLs, and status)
        conn.execute(
            "CREATE TABLE IF NOT EXISTS tasks (
                id TEXT PRIMARY KEY,
                url TEXT NOT NULL,
                title TEXT NOT NULL,
                media_type TEXT NOT NULL,
                format TEXT NOT NULL,
                quality TEXT NOT NULL,
                output_path TEXT NOT NULL,
                is_playlist INTEGER NOT NULL,
                timestamp INTEGER NOT NULL,
                args TEXT NOT NULL DEFAULT '{}',
                temp_path TEXT NOT NULL DEFAULT '',
                status TEXT NOT NULL DEFAULT '\"Queued\"',
                children TEXT NOT NULL DEFAULT '[]'
            )",
            [],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;

        // 3. Robust Migrations: Only add columns if they don't exist
        Self::ensure_column(&conn, "args", "TEXT NOT NULL DEFAULT '{}'")?;
        Self::ensure_column(&conn, "temp_path", "TEXT NOT NULL DEFAULT ''")?;
        Self::ensure_column(&conn, "children", "TEXT NOT NULL DEFAULT '[]'")?;

        // 4. Indexing for performance (Duplicate checks)
        let _ = conn.execute("CREATE INDEX IF NOT EXISTS idx_tasks_url ON tasks(url)", []);

        Ok(Self { conn })
    }

    /// Helper to safely add a column if it's missing
    fn ensure_column(conn: &Connection, name: &str, definition: &str) -> AppResult<()> {
        let mut stmt = conn
            .prepare("PRAGMA table_info(tasks)")
            .map_err(|e| AppError::Database(e.to_string()))?;
        let columns = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .map_err(|e| AppError::Database(e.to_string()))?;

        let mut exists = false;
        for c in columns.flatten() {
            if c == name {
                exists = true;
                break;
            }
        }

        if !exists {
            let _ = conn.execute(
                &format!("ALTER TABLE tasks ADD COLUMN {} {}", name, definition),
                [],
            );
        }
        Ok(())
    }

    pub fn save_task(&self, task: &DownloadTask) -> AppResult<()> {
        let args_json = serde_json::to_string(&task.args).unwrap_or_default();
        let status_json = serde_json::to_string(&task.status).unwrap_or_default();
        let children_json = serde_json::to_string(&task.children).unwrap_or_default();

        let result = self.conn.execute(
            "INSERT OR REPLACE INTO tasks (
                id, url, title, media_type, format, quality, output_path, 
                is_playlist, timestamp, args, temp_path, status, children
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                task.id,
                task.url,
                task.title,
                task.media_type,
                task.format,
                task.quality,
                task.output_path,
                if task.is_playlist { 1 } else { 0 },
                task.timestamp,
                args_json,
                task.temp_path,
                status_json,
                children_json,
            ],
        );

        if let Err(e) = &result {
            eprintln!("Failed to save task {} to database: {}", task.id, e);
        }

        result.map_err(|e| AppError::Database(format!("Operation failed: {}", e)))?;
        Ok(())
    }

    pub fn delete_task(&self, id: &str) -> AppResult<()> {
        let result = self
            .conn
            .execute("DELETE FROM tasks WHERE id = ?1", params![id]);

        if let Err(e) = &result {
            eprintln!("Failed to delete task {} from database: {}", id, e);
        }

        result.map_err(|e| AppError::Database(format!("Operation failed: {}", e)))?;
        Ok(())
    }

    pub fn load_active_tasks(&self) -> AppResult<Vec<DownloadTask>> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM tasks")
            .map_err(|e| AppError::Database(e.to_string()))?;
        let task_iter = stmt
            .query_map([], |row| {
                let args_json: String = row.get("args")?;
                let status_json: String = row.get("status")?;
                let children_json: String = row.get("children")?;

                Ok(DownloadTask {
                    id: row.get("id")?,
                    url: row.get("url")?,
                    title: row.get("title")?,
                    media_type: row.get("media_type")?,
                    format: row.get("format")?,
                    quality: row.get("quality")?,
                    output_path: row.get("output_path")?,
                    is_playlist: row.get::<_, i32>("is_playlist")? != 0,
                    timestamp: row.get("timestamp")?,
                    args: serde_json::from_str(&args_json).unwrap_or_default(),
                    temp_path: row.get("temp_path")?,
                    status: serde_json::from_str(&status_json).unwrap_or(DownloadStatus::Queued),
                    pid: None,
                    current_file: None,
                    children: serde_json::from_str(&children_json).unwrap_or_default(),
                })
            })
            .map_err(|e| AppError::Database(e.to_string()))?;

        let mut tasks = Vec::new();
        for task in task_iter {
            tasks.push(task.map_err(|e| AppError::Database(format!("Operation failed: {}", e)))?);
        }
        Ok(tasks)
    }
}
