use rusqlite::{Connection, Result};

pub fn migrate(conn: &Connection) -> Result<()> {
    crate::schema::init_apps_table(conn)?;
    crate::schema::init_sessions_table(conn)?;
    crate::schema::init_builds_table(conn)?;
    crate::schema::init_admin_tables(conn)?;
    let mut stmt = conn.prepare("PRAGMA table_info(apps)")?;
    let columns = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<Vec<_>>>()?;
    if !columns.iter().any(|name| name == "domain") {
        conn.execute("ALTER TABLE apps ADD COLUMN domain TEXT", [])?;
    }
    if !columns.iter().any(|name| name == "port") {
        conn.execute("ALTER TABLE apps ADD COLUMN port INTEGER", [])?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upgrades_existing_apps_table_and_can_run_again() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE apps (id TEXT PRIMARY KEY, name TEXT); INSERT INTO apps VALUES ('existing', 'Existing');").unwrap();
        migrate(&conn).unwrap();
        migrate(&conn).unwrap();
        let result: (String, Option<String>, Option<u16>) = conn
            .query_row(
                "SELECT name, domain, port FROM apps WHERE id = 'existing'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(result, ("Existing".into(), None, None));
    }
}
