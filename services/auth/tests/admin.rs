use auth::admin::{
    repository::AdminRepository,
    services::{AdminService, AuthError},
};

struct TestDirectory(std::path::PathBuf);
impl TestDirectory {
    fn new() -> Self {
        Self(std::env::temp_dir().join(
            format!("launchlite-admin-test-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()),
        ))
    }
}
impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn initial_token_is_private_persistent_and_disabled_after_setup() {
    let directory = TestDirectory::new();
    let database = db::Database::memory().unwrap();
    let repository = AdminRepository::new(database.clone());
    let service =
        AdminService::from_data_dir(repository.clone(), directory.0.to_str().unwrap(), false)
            .unwrap();
    let path = directory.0.join("initial-admin-token");
    let token = std::fs::read_to_string(&path).unwrap();
    assert_eq!(token.len(), 64);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    let restarted =
        AdminService::from_data_dir(repository.clone(), directory.0.to_str().unwrap(), false)
            .unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), token);
    let session = restarted
        .setup(&token, "owner", "a-long-test-password")
        .unwrap();
    assert!(!service.setup_required().unwrap());
    let configured =
        AdminService::from_data_dir(repository, directory.0.to_str().unwrap(), false).unwrap();
    assert!(configured.authenticated(Some(&session)).unwrap());
    assert!(matches!(
        configured.setup(&token, "other", "a-long-test-password"),
        Err(AuthError::AlreadyConfigured)
    ));
}

#[test]
fn session_storage_errors_fail_closed() {
    let database = db::Database::memory().unwrap();
    let service = AdminService::new(
        AdminRepository::new(database.clone()),
        Some("setup-token"),
        false,
    );
    let token = service
        .setup("setup-token", "owner", "a-long-test-password")
        .unwrap();
    database
        .connection()
        .unwrap()
        .execute_batch("DROP TABLE admin_sessions")
        .unwrap();
    assert!(matches!(
        service.authenticated(Some(&token)),
        Err(AuthError::Internal(_))
    ));
}
