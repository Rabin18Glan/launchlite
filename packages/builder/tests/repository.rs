use builder::repository::BuildRepository;
use common::{BuildJob, BuildStatus};

#[test]
fn builds_round_trip_status_updates_and_filter_by_app() {
    let repository = BuildRepository::new(db::Database::memory().unwrap());
    let mut job = BuildJob {
        id: "build-1".into(),
        app_id: "app-1".into(),
        repo_url: "https://example.com/repo.git".into(),
        branch: "main".into(),
        commit_hash: None,
        status: BuildStatus::Queued,
        started_at: 1,
        completed_at: None,
    };
    repository.save_build(&job).unwrap();
    job.status = BuildStatus::Success;
    job.completed_at = Some(2);
    repository.save_build(&job).unwrap();
    let builds = repository.get_builds_for_app("app-1").unwrap();
    assert_eq!(builds.len(), 1);
    assert_eq!(builds[0].status, BuildStatus::Success);
    assert_eq!(builds[0].completed_at, Some(2));
    assert!(repository.get_builds_for_app("app-2").unwrap().is_empty());
}

#[test]
fn failed_build_write_is_reported_without_persisting_a_job() {
    let database = db::Database::memory().unwrap();
    let repository = BuildRepository::new(database.clone());
    database.connection().unwrap().execute_batch(
        "CREATE TRIGGER reject_build BEFORE INSERT ON builds BEGIN SELECT RAISE(ABORT, 'write rejected'); END;"
    ).unwrap();
    let job = BuildJob {
        id: "rejected".into(),
        app_id: "app".into(),
        repo_url: "https://example.com/repo.git".into(),
        branch: "main".into(),
        commit_hash: None,
        status: BuildStatus::Queued,
        started_at: 1,
        completed_at: None,
    };
    let error = repository.save_build(&job).unwrap_err();
    assert!(matches!(error, db::Error::Sqlite(_)));
    assert!(error.to_string().contains("write rejected"));
    assert!(repository.get_builds_for_app("app").unwrap().is_empty());
}
