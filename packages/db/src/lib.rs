pub mod migration;
pub mod schema;

use r2d2::{Pool, PooledConnection};
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::Connection;
use std::time::Duration;

#[derive(Debug)]
pub enum Error {
    Sqlite(rusqlite::Error),
    Pool(r2d2::Error),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sqlite(error) => write!(f, "SQLite error: {error}"),
            Self::Pool(error) => write!(f, "Database pool error: {error}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Sqlite(error) => Some(error),
            Self::Pool(error) => Some(error),
        }
    }
}

impl From<rusqlite::Error> for Error {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite(error)
    }
}

impl From<r2d2::Error> for Error {
    fn from(error: r2d2::Error) -> Self {
        Self::Pool(error)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone)]
pub struct Database {
    pool: Pool<SqliteConnectionManager>,
}

impl Database {
    pub fn new(path: &str) -> Result<Self> {
        if let Some(parent) = std::path::Path::new(path).parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        // A plain :memory: path would create an isolated database per connection.
        if path == ":memory:" {
            return Self::memory();
        }

        // Set WAL before opening the pool so startup connections do not race
        // to change the database journal mode.
        let conn = Connection::open(path)?;
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        drop(conn);

        Self::from_manager(SqliteConnectionManager::file(path))
    }

    pub fn memory() -> Result<Self> {
        // The manager keeps one shared in-memory database alive for this pool.
        Self::from_manager(SqliteConnectionManager::memory())
    }

    fn from_manager(manager: SqliteConnectionManager) -> Result<Self> {
        let manager = manager.with_init(|conn| conn.busy_timeout(Duration::from_secs(5)));
        let pool = Pool::builder()
            .max_size(8)
            .min_idle(Some(1))
            .connection_timeout(Duration::from_secs(5))
            .build(manager)?;
        let db = Self { pool };
        db.init_tables()?;
        Ok(db)
    }

    pub fn connection(&self) -> Result<PooledConnection<SqliteConnectionManager>> {
        Ok(self.pool.get()?)
    }

    fn init_tables(&self) -> Result<()> {
        let conn = self.connection()?;
        migration::migrate(&conn)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_connections_share_data_but_separate_pools_are_isolated() {
        let db = Database::memory().unwrap();
        let writer = db.connection().unwrap();
        writer
            .execute_batch(
                "CREATE TABLE pool_test (value INTEGER); INSERT INTO pool_test VALUES (42);",
            )
            .unwrap();
        let reader = db.connection().unwrap();
        assert_eq!(
            reader
                .query_row("SELECT value FROM pool_test", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            42
        );
        assert!(
            Database::memory()
                .unwrap()
                .connection()
                .unwrap()
                .prepare("SELECT * FROM pool_test")
                .is_err()
        );
    }

    #[test]
    fn memory_path_uses_shared_pool() {
        let db = Database::new(":memory:").unwrap();
        let _held_connection = db.connection().unwrap();
        assert_eq!(
            db.connection()
                .unwrap()
                .query_row("SELECT COUNT(*) FROM apps", [], |row| row.get::<_, u64>(0))
                .unwrap(),
            0
        );
        assert_eq!(
            db.clone()
                .connection()
                .unwrap()
                .query_row("SELECT COUNT(*) FROM sessions", [], |row| row
                    .get::<_, u64>(0))
                .unwrap(),
            0
        );
    }

    #[test]
    fn file_database_reads_continue_during_write_transaction() {
        let path = std::env::temp_dir().join(format!(
            "launchlite-pool-{}-{}.db",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        ));
        {
            let db = Database::new(path.to_str().unwrap()).unwrap();
            let writer = db.connection().unwrap();
            assert_eq!(
                writer
                    .query_row("PRAGMA journal_mode", [], |row| row.get::<_, String>(0))
                    .unwrap(),
                "wal"
            );
            writer
                .execute_batch("BEGIN IMMEDIATE; DELETE FROM apps;")
                .unwrap();
            // This must use a different connection and return while the writer
            // still holds its transaction, rather than waiting on a global lock.
            assert_eq!(
                db.connection()
                    .unwrap()
                    .query_row("SELECT COUNT(*) FROM apps", [], |row| row.get::<_, u64>(0))
                    .unwrap(),
                0
            );
            let reader = db.connection().unwrap();
            assert_eq!(
                reader
                    .query_row("PRAGMA busy_timeout", [], |row| row.get::<_, u64>(0))
                    .unwrap(),
                5000
            );
            writer.execute_batch("ROLLBACK;").unwrap();
        }
        std::fs::remove_file(path).unwrap();
    }
}
