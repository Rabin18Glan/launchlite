use crate::{client::GithubClient, repository::SessionRepository};
use common::Config;

#[derive(Clone)]
pub struct AuthState {
    pub admin: crate::admin::services::AdminService,
    pub github: GithubClient,
    pub config: Config,
    pub repository: SessionRepository,
}
