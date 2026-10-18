use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;

pub async fn clone_or_pull(
    repo_url: &str,
    branch: &str,
    dest_dir: &Path,
    auth_token: Option<&str>,
) -> Result<String, String> {
    let authenticated_url = if let Some(token) = auth_token {
        inject_token_into_url(repo_url, token)
    } else {
        repo_url.to_string()
    };

    if dest_dir.join(".git").exists() {
        // Repository already cloned, pull latest changes
        let fetch_res = Command::new("git")
            .current_dir(dest_dir)
            .args(["fetch", "origin", branch])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| format!("git fetch failed: {}", e))?;

        if !fetch_res.status.success() {
            let err = String::from_utf8_lossy(&fetch_res.stderr);
            return Err(format!("git fetch error: {}", err));
        }

        let checkout_res = Command::new("git")
            .current_dir(dest_dir)
            .args(["checkout", branch])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| format!("git checkout failed: {}", e))?;

        if !checkout_res.status.success() {
            let err = String::from_utf8_lossy(&checkout_res.stderr);
            return Err(format!("git checkout error: {}", err));
        }

        let reset_res = Command::new("git")
            .current_dir(dest_dir)
            .args(["reset", "--hard", &format!("origin/{}", branch)])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| format!("git reset failed: {}", e))?;

        if !reset_res.status.success() {
            let err = String::from_utf8_lossy(&reset_res.stderr);
            return Err(format!("git reset error: {}", err));
        }
    } else {
        // Fresh clone
        if let Some(parent) = dest_dir.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }

        let clone_res = Command::new("git")
            .args([
                "clone",
                "--depth",
                "1",
                "--branch",
                branch,
                &authenticated_url,
                dest_dir.to_str().unwrap(),
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| format!("git clone failed: {}", e))?;

        if !clone_res.status.success() {
            let err = String::from_utf8_lossy(&clone_res.stderr);
            return Err(format!("git clone error: {}", err));
        }
    }

    // Retrieve latest commit hash
    let rev_res = Command::new("git")
        .current_dir(dest_dir)
        .args(["rev-parse", "HEAD"])
        .output()
        .await
        .map_err(|e| format!("git rev-parse failed: {}", e))?;

    let hash = String::from_utf8_lossy(&rev_res.stdout).trim().to_string();
    Ok(hash)
}

fn inject_token_into_url(url: &str, token: &str) -> String {
    if let Some(stripped) = url.strip_prefix("https://") {
        format!("https://x-access-token:{}@{}", token, stripped)
    } else {
        url.to_string()
    }
}
