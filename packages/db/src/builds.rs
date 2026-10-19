use common::models::{BuildJob, BuildStatus};
use rusqlite::{Connection, Result, params};

pub fn init_builds_table(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS builds (
            id TEXT PRIMARY KEY,
            app_id TEXT NOT NULL,
            repo_url TEXT NOT NULL,
            branch TEXT NOT NULL,
            commit_hash TEXT,
            status_json TEXT NOT NULL,
            started_at INTEGER NOT NULL,
            completed_at INTEGER
        )",
        [],
    )?;
    Ok(())
}

pub fn save_build(conn: &Connection, build: &BuildJob) -> Result<()> {
    let status_json =
        serde_json::to_string(&build.status).unwrap_or_else(|_| "\"queued\"".to_string());
    conn.execute(
        "INSERT INTO builds (id, app_id, repo_url, branch, commit_hash, status_json, started_at, completed_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(id) DO UPDATE SET
            status_json = excluded.status_json,
            commit_hash = excluded.commit_hash,
            completed_at = excluded.completed_at",
        params![
            build.id,
            build.app_id,
            build.repo_url,
            build.branch,
            build.commit_hash,
            status_json,
            build.started_at,
            build.completed_at,
        ],
    )?;
    Ok(())
}

pub fn get_builds_for_app(conn: &Connection, app_id: &str) -> Result<Vec<BuildJob>> {
    let mut stmt = conn.prepare(
        "SELECT id, app_id, repo_url, branch, commit_hash, status_json, started_at, completed_at
         FROM builds WHERE app_id = ?1 ORDER BY started_at DESC LIMIT 20",
    )?;

    let rows = stmt.query_map(params![app_id], |row| {
        let status_json: String = row.get(5)?;
        let status: BuildStatus = serde_json::from_str(&status_json)
            .unwrap_or(BuildStatus::Failed("corrupt".to_string()));
        Ok(BuildJob {
            id: row.get(0)?,
            app_id: row.get(1)?,
            repo_url: row.get(2)?,
            branch: row.get(3)?,
            commit_hash: row.get(4)?,
            status,
            started_at: row.get(6)?,
            completed_at: row.get(7)?,
        })
    })?;

    let mut list = Vec::new();
    for b in rows {
        list.push(b?);
    }
    Ok(list)
}
