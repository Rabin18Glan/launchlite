use crate::models::{GithubUser, UserSession};
use rusqlite::{Connection, Result, params};

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

pub fn save_session(conn: &Connection, session: &UserSession) -> Result<()> {
    let user_json = serde_json::to_string(&session.user).unwrap_or_else(|_| "{}".to_string());
    conn.execute(
        "INSERT OR REPLACE INTO sessions (session_id, access_token, user_json, created_at)
         VALUES (?1, ?2, ?3, ?4)",
        params![
            session.session_id,
            session.access_token,
            user_json,
            session.created_at
        ],
    )?;
    Ok(())
}

pub fn get_session(conn: &Connection, session_id: &str) -> Result<Option<UserSession>> {
    let mut stmt = conn.prepare(
        "SELECT session_id, access_token, user_json, created_at FROM sessions WHERE session_id = ?1",
    )?;

    let mut rows = stmt.query(params![session_id])?;
    if let Some(row) = rows.next()? {
        let user_json: String = row.get(2)?;
        let user: GithubUser = serde_json::from_str(&user_json).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::new(e))
        })?;

        Ok(Some(UserSession {
            session_id: row.get(0)?,
            access_token: row.get(1)?,
            user,
            created_at: row.get(3)?,
        }))
    } else {
        Ok(None)
    }
}

pub fn get_latest_session(conn: &Connection) -> Result<Option<UserSession>> {
    let mut stmt = conn.prepare(
        "SELECT session_id, access_token, user_json, created_at FROM sessions ORDER BY created_at DESC LIMIT 1",
    )?;

    let mut rows = stmt.query([])?;
    if let Some(row) = rows.next()? {
        let user_json: String = row.get(2)?;
        let user: GithubUser = serde_json::from_str(&user_json).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::new(e))
        })?;

        Ok(Some(UserSession {
            session_id: row.get(0)?,
            access_token: row.get(1)?,
            user,
            created_at: row.get(3)?,
        }))
    } else {
        Ok(None)
    }
}

pub fn delete_session(conn: &Connection, session_id: &str) -> Result<bool> {
    let count = conn.execute(
        "DELETE FROM sessions WHERE session_id = ?1",
        params![session_id],
    )?;
    Ok(count > 0)
}
