use super::repository::AdminRepository;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier, password_hash::SaltString};
use common::current_timestamp;
use rand::{RngCore, rngs::OsRng};
use sha2::{Digest, Sha256};
use std::{
    collections::VecDeque,
    io::{Read, Write},
    path::Path,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use subtle::ConstantTimeEq;

pub const SESSION_SECONDS: u64 = 12 * 60 * 60;
pub const COOKIE_NAME: &str = "launchlite_admin";

#[derive(Debug)]
pub enum AuthError {
    InvalidCredentials,
    InvalidSetupToken,
    AlreadyConfigured,
    InvalidInput(&'static str),
    RateLimited,
    Internal(String),
}

impl From<db::Error> for AuthError {
    fn from(error: db::Error) -> Self {
        Self::Internal(error.to_string())
    }
}

#[derive(Clone)]
pub struct AdminService {
    repository: AdminRepository,
    setup_token_hash: Option<[u8; 32]>,
    pub secure_cookies: bool,
    attempts: Arc<Mutex<VecDeque<Instant>>>,
}

impl AdminService {
    pub fn new(
        repository: AdminRepository,
        setup_token: Option<&str>,
        secure_cookies: bool,
    ) -> Self {
        Self {
            repository,
            setup_token_hash: setup_token.map(digest),
            secure_cookies,
            attempts: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    pub fn from_data_dir(
        repository: AdminRepository,
        data_dir: &str,
        secure_cookies: bool,
    ) -> Result<Self, String> {
        if repository.account().map_err(|e| e.to_string())?.is_some() {
            return Ok(Self::new(repository, None, secure_cookies));
        }
        std::fs::create_dir_all(data_dir).map_err(|e| e.to_string())?;
        let path = Path::new(data_dir).join("initial-admin-token");
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&path) {
            Ok(mut file) => {
                let token =
                    random_token().map_err(|e| format!("Cannot generate setup token: {e:?}"))?;
                file.write_all(token.as_bytes())
                    .and_then(|_| file.sync_all())
                    .map_err(|e| e.to_string())?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.to_string()),
        }
        let mut token = String::new();
        std::fs::File::open(&path)
            .and_then(|file| file.take(129).read_to_string(&mut token))
            .map_err(|e| e.to_string())?;
        let token = token.trim();
        if token.len() != 64 || !token.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(format!(
                "Invalid initial admin token file: {}",
                path.display()
            ));
        }
        tracing::info!(path = %path.display(), "Admin setup required. Read this server-local token file and visit /setup");
        Ok(Self::new(repository, Some(token), secure_cookies))
    }

    pub fn setup_required(&self) -> Result<bool, AuthError> {
        Ok(self.repository.account()?.is_none())
    }

    pub fn setup(&self, token: &str, username: &str, password: &str) -> Result<String, AuthError> {
        if !self.setup_required()? {
            return Err(AuthError::AlreadyConfigured);
        }
        self.check_attempt_limit()?;
        if !self
            .setup_token_hash
            .as_ref()
            .is_some_and(|expected| bool::from(expected.ct_eq(&digest(token))))
        {
            return Err(AuthError::InvalidSetupToken);
        }
        let username = username.trim();
        if !(3..=64).contains(&username.len())
            || !username
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        {
            return Err(AuthError::InvalidInput(
                "Username must be 3–64 characters: letters, numbers, dots, underscores, or hyphens.",
            ));
        }
        if !(12..=1024).contains(&password.len()) {
            return Err(AuthError::InvalidInput(
                "Password must be between 12 and 1024 bytes.",
            ));
        }
        let salt = SaltString::generate(&mut OsRng);
        let hash = Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| AuthError::Internal(e.to_string()))?
            .to_string();
        if !self
            .repository
            .create_account(username, &hash, current_timestamp())?
        {
            return Err(AuthError::AlreadyConfigured);
        }
        self.issue_session()
    }

    pub fn login(&self, username: &str, password: &str) -> Result<String, AuthError> {
        self.check_attempt_limit()?;
        if username.len() > 64 || password.len() > 1024 {
            return Err(AuthError::InvalidCredentials);
        }
        let account = self
            .repository
            .account()?
            .ok_or(AuthError::InvalidCredentials)?;
        let hash = PasswordHash::new(&account.password_hash)
            .map_err(|e| AuthError::Internal(e.to_string()))?;
        let valid_password = Argon2::default()
            .verify_password(password.as_bytes(), &hash)
            .is_ok();
        if !valid_password || account.username != username.trim() {
            return Err(AuthError::InvalidCredentials);
        }
        self.issue_session()
    }

    fn issue_session(&self) -> Result<String, AuthError> {
        let token = random_token()?;
        let now = current_timestamp();
        self.repository
            .save_session(&token_hash(&token), now, now + SESSION_SECONDS)?;
        Ok(token)
    }

    pub fn authenticated(&self, token: Option<&str>) -> Result<bool, AuthError> {
        let Some(token) =
            token.filter(|token| token.len() == 64 && token.bytes().all(|b| b.is_ascii_hexdigit()))
        else {
            return Ok(false);
        };
        Ok(self
            .repository
            .session_valid(&token_hash(token), current_timestamp())?)
    }

    pub fn logout(&self, token: &str) -> Result<(), AuthError> {
        self.repository.delete_session(&token_hash(token))?;
        Ok(())
    }

    fn check_attempt_limit(&self) -> Result<(), AuthError> {
        let mut attempts = self
            .attempts
            .lock()
            .map_err(|e| AuthError::Internal(e.to_string()))?;
        let now = Instant::now();
        while attempts
            .front()
            .is_some_and(|time| now.duration_since(*time) >= Duration::from_secs(60))
        {
            attempts.pop_front();
        }
        if attempts.len() >= 10 {
            return Err(AuthError::RateLimited);
        }
        attempts.push_back(now);
        Ok(())
    }
}

pub fn random_token() -> Result<String, AuthError> {
    let mut bytes = [0u8; 32];
    OsRng
        .try_fill_bytes(&mut bytes)
        .map_err(|e| AuthError::Internal(e.to_string()))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

fn digest(token: &str) -> [u8; 32] {
    Sha256::digest(token.as_bytes()).into()
}
pub fn token_hash(token: &str) -> String {
    digest(token).iter().map(|b| format!("{b:02x}")).collect()
}
