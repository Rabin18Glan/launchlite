use crate::models::{GithubRepo, GithubUser};
use reqwest::header::{ACCEPT, AUTHORIZATION, HeaderMap, HeaderValue, USER_AGENT};
use serde::Deserialize;

#[derive(Clone)]
pub struct GithubClient {
    client: reqwest::Client,
}

#[derive(Deserialize)]
struct OAuthTokenResponse {
    access_token: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

impl GithubClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .unwrap_or_default(),
        }
    }

    pub async fn exchange_code(
        &self,
        client_id: &str,
        client_secret: &str,
        code: &str,
        redirect_uri: &str,
    ) -> Result<String, String> {
        let params = [
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("code", code),
            ("redirect_uri", redirect_uri),
        ];

        let res = self
            .client
            .post("https://github.com/login/oauth/access_token")
            .header(ACCEPT, "application/json")
            .header(USER_AGENT, "Rust-Serverless-Platform")
            .form(&params)
            .send()
            .await
            .map_err(|e| format!("Failed to send token request: {}", e))?;

        if !res.status().is_success() {
            return Err(format!(
                "GitHub token exchange failed: HTTP {}",
                res.status()
            ));
        }

        let body: OAuthTokenResponse = res
            .json()
            .await
            .map_err(|e| format!("Failed to parse GitHub token response: {}", e))?;

        if let Some(err) = body.error {
            let desc = body.error_description.unwrap_or_default();
            return Err(format!("GitHub OAuth error: {} - {}", err, desc));
        }

        body.access_token
            .ok_or_else(|| "No access token in GitHub response".to_string())
    }

    pub async fn get_user(&self, token: &str) -> Result<GithubUser, String> {
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", token)).map_err(|e| e.to_string())?,
        );
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static("Rust-Serverless-Platform"),
        );
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/vnd.github+json"),
        );

        let res = self
            .client
            .get("https://api.github.com/user")
            .headers(headers)
            .send()
            .await
            .map_err(|e| format!("Failed to fetch user from GitHub: {}", e))?;

        if !res.status().is_success() {
            return Err(format!("GitHub user request failed: HTTP {}", res.status()));
        }

        let user: GithubUser = res
            .json()
            .await
            .map_err(|e| format!("Failed to parse user JSON: {}", e))?;

        Ok(user)
    }

    pub async fn get_repos(&self, token: &str) -> Result<Vec<GithubRepo>, String> {
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", token)).map_err(|e| e.to_string())?,
        );
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static("Rust-Serverless-Platform"),
        );
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/vnd.github+json"),
        );

        let res = self
            .client
            .get("https://api.github.com/user/repos?sort=updated&per_page=50")
            .headers(headers)
            .send()
            .await
            .map_err(|e| format!("Failed to fetch repos from GitHub: {}", e))?;

        if !res.status().is_success() {
            return Err(format!(
                "GitHub repos request failed: HTTP {}",
                res.status()
            ));
        }

        let repos: Vec<GithubRepo> = res
            .json()
            .await
            .map_err(|e| format!("Failed to parse repos JSON: {}", e))?;

        Ok(repos)
    }
}
