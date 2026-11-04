use common::AppConfig;
use runtime::repository::AppRepository;

#[test]
fn apps_round_trip_update_and_delete() {
    let repository = AppRepository::new(db::Database::memory().unwrap());
    let mut app: AppConfig = serde_json::from_value(serde_json::json!({
        "id": "test-app", "name": "Test", "executable": "/bin/true", "env": {"PORT": "8080"}, "port": 8080
    })).unwrap();
    repository.save_app(&app).unwrap();
    app.domain = Some("test.example.com".into());
    repository.save_app(&app).unwrap();
    let apps = repository.get_all_apps().unwrap();
    assert_eq!(apps.len(), 1);
    assert_eq!(apps[0].domain, app.domain);
    assert_eq!(apps[0].env, app.env);
    assert!(repository.delete_app("test-app").unwrap());
    assert!(repository.get_all_apps().unwrap().is_empty());
}

#[test]
fn failed_write_leaves_no_app_and_repository_can_recover() {
    let database = db::Database::memory().unwrap();
    let repository = AppRepository::new(database.clone());
    database.connection().unwrap().execute_batch(
        "CREATE TRIGGER reject_app BEFORE INSERT ON apps BEGIN SELECT RAISE(ABORT, 'write rejected'); END;"
    ).unwrap();
    let app: AppConfig = serde_json::from_value(serde_json::json!({
        "id": "test", "name": "Test", "executable": "/unused/server"
    }))
    .unwrap();
    let error = repository.save_app(&app).unwrap_err();
    assert!(matches!(error, db::Error::Sqlite(_)));
    assert!(error.to_string().contains("write rejected"));
    assert!(repository.get_all_apps().unwrap().is_empty());
    database
        .connection()
        .unwrap()
        .execute_batch("DROP TRIGGER reject_app")
        .unwrap();
    repository.save_app(&app).unwrap();
    assert_eq!(repository.get_all_apps().unwrap()[0].id, "test");
}
