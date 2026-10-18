use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub db_path: String,
    pub data_dir: String,
    pub github_client_id: Option<String>,
    pub github_client_secret: Option<String>,
    pub redirect_uri: String,
}

impl Config {
    pub fn from_env() -> Self {
        let host = env::var("PLATFORM_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
        let port = env::var("PLATFORM_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(3000);

        let data_dir = env::var("PLATFORM_DATA_DIR").unwrap_or_else(|_| "./data".to_string());
        let db_path =
            env::var("PLATFORM_DB").unwrap_or_else(|_| format!("{}/platform.db", data_dir));

        let github_client_id = env::var("GITHUB_CLIENT_ID").ok().filter(|s| !s.is_empty());
        let github_client_secret = env::var("GITHUB_CLIENT_SECRET")
            .ok()
            .filter(|s| !s.is_empty());

        let redirect_uri = env::var("GITHUB_REDIRECT_URI")
            .unwrap_or_else(|_| format!("http://{}:{}/auth/github/callback", host, port));

        Self {
            host,
            port,
            db_path,
            data_dir,
            github_client_id,
            github_client_secret,
            redirect_uri,
        }
    }

    pub fn has_github_oauth(&self) -> bool {
        self.github_client_id.is_some() && self.github_client_secret.is_some()
    }
}
