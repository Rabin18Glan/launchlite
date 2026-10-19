pub mod apps;
pub mod builds;
pub mod sessions;

use rusqlite::{Connection, Result};
use std::sync::{Arc, Mutex};

use common::models::{AppConfig, BuildJob, UserSession};

#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    pub fn new(path: &str) -> Result<Self> {
        if let Some(parent) = std::path::Path::new(path).parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let conn = Connection::open(path)?;
        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        db.init_tables()?;
        Ok(db)
    }

    #[allow(dead_code)]
    pub fn memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        db.init_tables()?;
        Ok(db)
    }

    fn init_tables(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        apps::init_apps_table(&conn)?;
        sessions::init_sessions_table(&conn)?;
        builds::init_builds_table(&conn)?;
        Ok(())
    }

    pub fn save_app(&self, app: &AppConfig) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        apps::save_app(&conn, app)
    }

    pub fn get_all_apps(&self) -> Result<Vec<AppConfig>> {
        let conn = self.conn.lock().unwrap();
        apps::get_all_apps(&conn)
    }

    pub fn delete_app(&self, id: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        apps::delete_app(&conn, id)
    }

    pub fn save_session(&self, session: &UserSession) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        sessions::save_session(&conn, session)
    }

    pub fn get_session(&self, session_id: &str) -> Result<Option<UserSession>> {
        let conn = self.conn.lock().unwrap();
        sessions::get_session(&conn, session_id)
    }

    pub fn get_latest_session(&self) -> Result<Option<UserSession>> {
        let conn = self.conn.lock().unwrap();
        sessions::get_latest_session(&conn)
    }

    pub fn delete_session(&self, session_id: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        sessions::delete_session(&conn, session_id)
    }

    pub fn save_build(&self, build: &BuildJob) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        builds::save_build(&conn, build)
    }

    pub fn get_builds_for_app(&self, app_id: &str) -> Result<Vec<BuildJob>> {
        let conn = self.conn.lock().unwrap();
        builds::get_builds_for_app(&conn, app_id)
    }
}
