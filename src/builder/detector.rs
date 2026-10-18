use crate::models::ProjectType;
use std::path::Path;

pub struct DetectedProject {
    pub project_type: ProjectType,
    pub build_cmd: Option<String>,
    pub run_cmd: String,
}

pub fn detect_project(repo_dir: &Path) -> DetectedProject {
    // 1. Rust project (Cargo.toml)
    let cargo_toml = repo_dir.join("Cargo.toml");
    if cargo_toml.exists() {
        let pkg_name = parse_cargo_package_name(&cargo_toml).unwrap_or_else(|| "app".to_string());
        return DetectedProject {
            project_type: ProjectType::Rust,
            build_cmd: Some("cargo build --release".to_string()),
            run_cmd: format!("./target/release/{}", pkg_name),
        };
    }

    // 2. Node.js project (package.json)
    let package_json = repo_dir.join("package.json");
    if package_json.exists() {
        let has_build = check_package_script(&package_json, "build");
        let build_cmd = if has_build {
            Some("npm install && npm run build".to_string())
        } else {
            Some("npm install".to_string())
        };

        let has_start = check_package_script(&package_json, "start");
        let run_cmd = if has_start {
            "npm start".to_string()
        } else if repo_dir.join("server.js").exists() {
            "node server.js".to_string()
        } else if repo_dir.join("index.js").exists() {
            "node index.js".to_string()
        } else if repo_dir.join("dist/index.js").exists() {
            "node dist/index.js".to_string()
        } else {
            "node index.js".to_string()
        };

        return DetectedProject {
            project_type: ProjectType::Node,
            build_cmd,
            run_cmd,
        };
    }

    // 3. Python project
    if repo_dir.join("requirements.txt").exists()
        || repo_dir.join("main.py").exists()
        || repo_dir.join("app.py").exists()
    {
        let build_cmd = if repo_dir.join("requirements.txt").exists() {
            Some("python3 -m pip install -r requirements.txt".to_string())
        } else {
            None
        };

        let run_cmd = if repo_dir.join("main.py").exists() {
            "python3 main.py".to_string()
        } else if repo_dir.join("app.py").exists() {
            "python3 app.py".to_string()
        } else {
            "python3 main.py".to_string()
        };

        return DetectedProject {
            project_type: ProjectType::Python,
            build_cmd,
            run_cmd,
        };
    }

    // 4. Go project
    if repo_dir.join("go.mod").exists() {
        return DetectedProject {
            project_type: ProjectType::Go,
            build_cmd: Some("go build -o app".to_string()),
            run_cmd: "./app".to_string(),
        };
    }

    // 5. Static HTML
    if repo_dir.join("index.html").exists() {
        return DetectedProject {
            project_type: ProjectType::Static,
            build_cmd: None,
            run_cmd: "python3 -m http.server 8080".to_string(),
        };
    }

    DetectedProject {
        project_type: ProjectType::Unknown,
        build_cmd: None,
        run_cmd: "./start.sh".to_string(),
    }
}

fn parse_cargo_package_name(path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    let mut in_package = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "[package]" {
            in_package = true;
            continue;
        }
        if in_package && trimmed.starts_with('[') {
            break;
        }
        if in_package && trimmed.starts_with("name") {
            let parts: Vec<&str> = trimmed.split('=').collect();
            if parts.len() == 2 {
                return Some(
                    parts[1]
                        .trim()
                        .trim_matches('"')
                        .trim_matches('\'')
                        .to_string(),
                );
            }
        }
    }
    None
}

fn check_package_script(path: &Path, script_name: &str) -> bool {
    if let Ok(content) = std::fs::read_to_string(path) {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
            return val
                .get("scripts")
                .and_then(|s| s.get(script_name))
                .is_some();
        }
    }
    false
}
