use crate::models::{AppConfig, RestartPolicy};
use rusqlite::{Connection, Result, params};

pub fn init_apps_table(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS apps (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            executable TEXT NOT NULL,
            args_json TEXT NOT NULL,
            env_json TEXT NOT NULL,
            working_dir TEXT,
            restart_policy TEXT NOT NULL,
            health_check_url TEXT,
            health_check_interval_secs INTEGER NOT NULL,
            auto_start INTEGER NOT NULL,
            git_json TEXT,
            domain TEXT,
            port INTEGER,
            created_at INTEGER NOT NULL
        )",
        [],
    )?;

    // Migrations for existing databases
    let _ = conn.execute("ALTER TABLE apps ADD COLUMN domain TEXT", []);
    let _ = conn.execute("ALTER TABLE apps ADD COLUMN port INTEGER", []);

    Ok(())
}

pub fn save_app(conn: &Connection, app: &AppConfig) -> Result<()> {
    let args_json = serde_json::to_string(&app.args).unwrap_or_else(|_| "[]".to_string());
    let env_json = serde_json::to_string(&app.env).unwrap_or_else(|_| "{}".to_string());
    let restart_policy =
        serde_json::to_string(&app.restart_policy).unwrap_or_else(|_| "\"never\"".to_string());
    let git_json = app.git.as_ref().and_then(|g| serde_json::to_string(g).ok());

    conn.execute(
        "INSERT INTO apps (
            id, name, executable, args_json, env_json, working_dir,
            restart_policy, health_check_url, health_check_interval_secs, auto_start, git_json, domain, port, created_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
        ON CONFLICT(id) DO UPDATE SET
            name = excluded.name,
            executable = excluded.executable,
            args_json = excluded.args_json,
            env_json = excluded.env_json,
            working_dir = excluded.working_dir,
            restart_policy = excluded.restart_policy,
            health_check_url = excluded.health_check_url,
            health_check_interval_secs = excluded.health_check_interval_secs,
            auto_start = excluded.auto_start,
            git_json = excluded.git_json,
            domain = excluded.domain,
            port = excluded.port",
        params![
            app.id,
            app.name,
            app.executable,
            args_json,
            env_json,
            app.working_dir,
            restart_policy,
            app.health_check_url,
            app.health_check_interval_secs,
            if app.auto_start { 1 } else { 0 },
            git_json,
            app.domain,
            app.port,
            app.created_at,
        ],
    )?;
    Ok(())
}

pub fn get_all_apps(conn: &Connection) -> Result<Vec<AppConfig>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, executable, args_json, env_json, working_dir,
                restart_policy, health_check_url, health_check_interval_secs, auto_start, git_json,
                domain, port, created_at
         FROM apps ORDER BY created_at ASC",
    )?;

    let rows = stmt.query_map([], |row| {
        let args_json: String = row.get(3)?;
        let env_json: String = row.get(4)?;
        let policy_json: String = row.get(6)?;
        let auto_start_int: i32 = row.get(9)?;
        let git_json: Option<String> = row.get(10)?;
        let domain: Option<String> = row.get(11)?;
        let port: Option<u16> = row.get(12)?;

        let args = serde_json::from_str(&args_json).unwrap_or_default();
        let env = serde_json::from_str(&env_json).unwrap_or_default();
        let restart_policy = serde_json::from_str(&policy_json).unwrap_or(RestartPolicy::Never);
        let git = git_json.and_then(|j| serde_json::from_str(&j).ok());

        Ok(AppConfig {
            id: row.get(0)?,
            name: row.get(1)?,
            executable: row.get(2)?,
            args,
            env,
            working_dir: row.get(5)?,
            restart_policy,
            health_check_url: row.get(7)?,
            health_check_interval_secs: row.get(8)?,
            auto_start: auto_start_int == 1,
            git,
            domain,
            port,
            created_at: row.get(13)?,
        })
    })?;

    let mut apps = Vec::new();
    for app in rows {
        apps.push(app?);
    }
    Ok(apps)
}

pub fn delete_app(conn: &Connection, id: &str) -> Result<bool> {
    let count = conn.execute("DELETE FROM apps WHERE id = ?1", params![id])?;
    Ok(count > 0)
}
