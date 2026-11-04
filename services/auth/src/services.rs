use crate::client::GithubClient;
use crate::repository::SessionRepository;
use crate::state::AuthState;
use common::config::Config;
use common::models::{UserSession, current_timestamp};

pub fn get_authorization_url(config: &Config) -> Result<String, String> {
    let client_id = config
        .github_client_id
        .as_ref()
        .ok_or_else(|| "GITHUB_CLIENT_ID is not configured".to_string())?;

    let url = format!(
        "https://github.com/login/oauth/authorize?client_id={}&redirect_uri={}&scope=repo,read:user",
        client_id,
        urlencoding_simple(&config.redirect_uri)
    );
    Ok(url)
}

pub async fn handle_callback(
    config: &Config,
    github: &GithubClient,
    repository: &SessionRepository,
    code: &str,
) -> Result<UserSession, String> {
    let client_id = config
        .github_client_id
        .as_ref()
        .ok_or_else(|| "GITHUB_CLIENT_ID not configured".to_string())?;
    let client_secret = config
        .github_client_secret
        .as_ref()
        .ok_or_else(|| "GITHUB_CLIENT_SECRET not configured".to_string())?;

    let token = github
        .exchange_code(client_id, client_secret, code, &config.redirect_uri)
        .await?;

    let user = github.get_user(&token).await?;

    let session_id = crate::admin::services::random_token()
        .map_err(|e| format!("Cannot create GitHub session: {e:?}"))?;
    let session = UserSession {
        session_id,
        access_token: token,
        user,
        created_at: current_timestamp(),
    };

    repository
        .save_session(&session)
        .map_err(|e| e.to_string())?;
    Ok(session)
}

pub async fn handle_direct_token(
    github: &GithubClient,
    repository: &SessionRepository,
    token: &str,
) -> Result<UserSession, String> {
    let user = github.get_user(token).await?;

    let session_id = crate::admin::services::random_token()
        .map_err(|e| format!("Cannot create GitHub session: {e:?}"))?;
    let session = UserSession {
        session_id,
        access_token: token.to_string(),
        user,
        created_at: current_timestamp(),
    };

    repository
        .save_session(&session)
        .map_err(|e| e.to_string())?;
    Ok(session)
}

fn urlencoding_simple(s: &str) -> String {
    s.replace(':', "%3A").replace('/', "%2F")
}

pub fn find_session(state: &AuthState, session_id: Option<&str>) -> Option<UserSession> {
    session_id.and_then(|id| state.repository.get_session(id).ok().flatten())
}

pub fn logout(state: &AuthState, session_id: &str) {
    let _ = state.repository.delete_session(session_id);
}

pub async fn list_repositories(
    state: &AuthState,
    session: &UserSession,
) -> Result<Vec<common::GithubRepo>, String> {
    state.github.get_repos(&session.access_token).await
}
