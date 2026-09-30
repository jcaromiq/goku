use std::fs;
use std::io::Write;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::{non_empty_env, resolve_portal_url, ENV_TOKEN, ENV_URL};

// ---------------------------------------------------------------------------
// Stored credentials (~/.config/goku/credentials.json)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Credentials {
    pub portal_url: String,
    pub token: String,
    pub project_id: String,
    pub project_name: String,
    pub connection_id: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CredentialSource {
    Env,
    File,
}

/// Token + portal URL used to talk to Observability Insight.
#[derive(Debug, Clone)]
pub struct PortalAuth {
    pub portal_url: String,
    pub token: String,
    pub source: CredentialSource,
}

/// `GOKU_CONFIG_DIR` overrides the config directory (mainly for tests).
fn config_dir() -> Result<PathBuf> {
    if let Some(dir) = non_empty_env("GOKU_CONFIG_DIR") {
        return Ok(PathBuf::from(dir));
    }
    dirs::config_dir()
        .map(|d| d.join("goku"))
        .ok_or_else(|| anyhow::anyhow!("Cannot determine the user configuration directory"))
}

pub fn credentials_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("credentials.json"))
}

pub fn load() -> Result<Option<Credentials>> {
    let path = credentials_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path)
        .with_context(|| format!("Cannot read credentials file '{}'", path.display()))?;
    let creds = serde_json::from_str(&raw).with_context(|| {
        format!(
            "Invalid credentials file '{}'. Run `goku login` again.",
            path.display()
        )
    })?;
    Ok(Some(creds))
}

pub fn save(creds: &Credentials) -> Result<PathBuf> {
    let path = credentials_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Cannot create directory '{}'", parent.display()))?;
    }

    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&path)
        .with_context(|| format!("Cannot write credentials file '{}'", path.display()))?;
    // The file may pre-date this version with wider permissions.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
    }
    file.write_all(serde_json::to_string_pretty(creds)?.as_bytes())?;
    Ok(path)
}

/// Returns true when a credentials file was removed.
pub fn delete() -> Result<bool> {
    let path = credentials_path()?;
    if !path.exists() {
        return Ok(false);
    }
    fs::remove_file(&path)
        .with_context(|| format!("Cannot delete credentials file '{}'", path.display()))?;
    Ok(true)
}

/// `GOKU_OI_TOKEN` (+ `GOKU_OI_URL`) take precedence over the credentials file.
pub fn resolve_auth() -> Result<Option<PortalAuth>> {
    if let Some(token) = non_empty_env(ENV_TOKEN) {
        return Ok(Some(PortalAuth {
            portal_url: resolve_portal_url(None),
            token,
            source: CredentialSource::Env,
        }));
    }
    Ok(load()?.map(|c| PortalAuth {
        portal_url: match non_empty_env(ENV_URL) {
            Some(url) => url.trim_end_matches('/').to_string(),
            None => c.portal_url.trim_end_matches('/').to_string(),
        },
        token: c.token,
        source: CredentialSource::File,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credentials_roundtrip_json() {
        let creds = Credentials {
            portal_url: "https://portal.example.com".into(),
            token: "oi_goku_abc".into(),
            project_id: "p1".into(),
            project_name: "Project".into(),
            connection_id: Some("c1".into()),
            created_at: "2026-09-29T10:00:00.000Z".into(),
        };
        let json = serde_json::to_string(&creds).unwrap();
        let back: Credentials = serde_json::from_str(&json).unwrap();
        assert_eq!(back, creds);
    }
}
