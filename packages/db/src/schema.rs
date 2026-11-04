use rusqlite::{Connection, Result};

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

    Ok(())
}

pub fn init_sessions_table(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS sessions (
            session_id TEXT PRIMARY KEY,
            access_token TEXT NOT NULL,
            user_json TEXT NOT NULL,
            created_at INTEGER NOT NULL
        )",
        [],
    )?;
    Ok(())
}

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

pub fn init_admin_tables(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS admin_account (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            username TEXT NOT NULL UNIQUE,
            password_hash TEXT NOT NULL,
            created_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS admin_sessions (
            token_hash TEXT PRIMARY KEY,
            expires_at INTEGER NOT NULL
        );",
    )
}
