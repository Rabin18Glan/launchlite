use auth::repository::SessionRepository;
use common::{GithubUser, UserSession};

#[test]
fn sessions_round_trip_update_and_delete() {
    let repository = SessionRepository::new(db::Database::memory().unwrap());
    let mut session = UserSession {
        session_id: "test-session".into(),
        access_token: "token".into(),
        created_at: 1,
        user: GithubUser {
            id: 1,
            login: "test".into(),
            name: None,
            avatar_url: "avatar".into(),
            html_url: "profile".into(),
        },
    };
    repository.save_session(&session).unwrap();
    session.access_token = "updated-token".into();
    repository.save_session(&session).unwrap();
    assert_eq!(
        repository
            .get_session("test-session")
            .unwrap()
            .unwrap()
            .access_token,
        "updated-token"
    );
    assert_eq!(
        repository.get_latest_session().unwrap().unwrap().user.login,
        "test"
    );
    assert!(repository.delete_session("test-session").unwrap());
    assert!(repository.get_session("test-session").unwrap().is_none());
}

#[test]
fn malformed_stored_user_is_reported_as_an_error() {
    let database = db::Database::memory().unwrap();
    let repository = SessionRepository::new(database.clone());
    database.connection().unwrap().execute(
        "INSERT INTO sessions (session_id, access_token, user_json, created_at) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params!["corrupt", "token", "invalid json", 1],
    ).unwrap();
    assert!(matches!(
        repository.get_session("corrupt"),
        Err(db::Error::Sqlite(
            rusqlite::Error::FromSqlConversionFailure(..)
        ))
    ));
    assert!(matches!(
        repository.get_latest_session(),
        Err(db::Error::Sqlite(
            rusqlite::Error::FromSqlConversionFailure(..)
        ))
    ));
    assert!(repository.delete_session("corrupt").unwrap());
    assert!(repository.get_latest_session().unwrap().is_none());
}

#[test]
fn missing_session_is_distinct_from_a_database_failure() {
    let database = db::Database::memory().unwrap();
    let repository = SessionRepository::new(database.clone());
    assert!(repository.get_session("missing").unwrap().is_none());
    assert!(!repository.delete_session("missing").unwrap());
    database
        .connection()
        .unwrap()
        .execute_batch("DROP TABLE sessions")
        .unwrap();
    assert!(matches!(
        repository.get_session("missing"),
        Err(db::Error::Sqlite(_))
    ));
}
