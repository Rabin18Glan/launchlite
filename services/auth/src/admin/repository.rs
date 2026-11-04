use db::{Database, Result};
use rusqlite::{OptionalExtension, params};

#[derive(Clone)]
pub struct AdminRepository {
    database: Database,
}

pub struct AdminAccount {
    pub username: String,
    pub password_hash: String,
}

impl AdminRepository {
    pub fn new(database: Database) -> Self {
        Self { database }
    }

    pub fn account(&self) -> Result<Option<AdminAccount>> {
        Ok(self
            .database
            .connection()?
            .query_row(
                "SELECT username, password_hash FROM admin_account WHERE id = 1",
                [],
                |row| {
                    Ok(AdminAccount {
                        username: row.get(0)?,
                        password_hash: row.get(1)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn create_account(&self, username: &str, password_hash: &str, now: u64) -> Result<bool> {
        Ok(self.database.connection()?.execute(
            "INSERT INTO admin_account (id, username, password_hash, created_at) VALUES (1, ?1, ?2, ?3) ON CONFLICT(id) DO NOTHING",
            params![username, password_hash, now],
        )? == 1)
    }

    pub fn save_session(&self, token_hash: &str, now: u64, expires_at: u64) -> Result<()> {
        let mut conn = self.database.connection()?;
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM admin_sessions WHERE expires_at <= ?1", [now])?;
        tx.execute(
            "INSERT INTO admin_sessions (token_hash, expires_at) VALUES (?1, ?2)",
            params![token_hash, expires_at],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn session_valid(&self, token_hash: &str, now: u64) -> Result<bool> {
        Ok(self.database.connection()?.query_row(
            "SELECT EXISTS(SELECT 1 FROM admin_sessions WHERE token_hash = ?1 AND expires_at > ?2)",
            params![token_hash, now],
            |row| row.get(0),
        )?)
    }

    pub fn delete_session(&self, token_hash: &str) -> Result<()> {
        self.database.connection()?.execute(
            "DELETE FROM admin_sessions WHERE token_hash = ?1",
            [token_hash],
        )?;
        Ok(())
    }
}
