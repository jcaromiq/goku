//! Observability Insight portal integration: `goku login`, `goku logout`,
//! `goku status` and `--report`.

mod auth;
mod credentials;
mod report;

use std::time::Duration;

use anyhow::Result;
use colored::Colorize;

pub use auth::login;
pub use report::{send_report, ReportContext, ReportOptions};

use credentials::{resolve_auth, CredentialSource};

pub const DEFAULT_PORTAL_URL: &str = "https://portal.observabilityinsight.com";
pub const ENV_TOKEN: &str = "GOKU_OI_TOKEN";
pub const ENV_URL: &str = "GOKU_OI_URL";

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Portal URL precedence: explicit flag > `GOKU_OI_URL` > default.
fn resolve_portal_url(explicit: Option<&str>) -> String {
    let raw = explicit
        .map(str::to_string)
        .or_else(|| non_empty_env(ENV_URL))
        .unwrap_or_else(|| DEFAULT_PORTAL_URL.to_string());
    raw.trim().trim_end_matches('/').to_string()
}

fn non_empty_env(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn http_client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent(concat!("goku/", env!("CARGO_PKG_VERSION")))
        .build()?)
}

/// Extracts `message` (or `error`) from a portal JSON error body.
fn api_error_message(body: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    v.get("message")
        .or_else(|| v.get("error"))
        .and_then(|m| m.as_str())
        .map(str::to_string)
}

const NOT_LOGGED_IN: &str =
    "Not logged in to Observability Insight. Run `goku login` or set GOKU_OI_TOKEN.";

// ---------------------------------------------------------------------------
// logout
// ---------------------------------------------------------------------------

pub fn logout() -> Result<()> {
    if credentials::delete()? {
        println!("{}", "Logged out.".green().bold());
        println!(
            "The token remains valid until you revoke it in the Goku connector settings of your project."
        );
    } else {
        println!("Not logged in, nothing to do.");
    }
    if non_empty_env(ENV_TOKEN).is_some() {
        println!("Note: GOKU_OI_TOKEN is still set in your environment.");
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// status
// ---------------------------------------------------------------------------

#[derive(Debug, serde::Deserialize)]
struct NamedRef {
    id: String,
    name: String,
}

#[derive(Debug, serde::Deserialize)]
struct TokenInfo {
    name: Option<String>,
    prefix: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct MeResponse {
    project: NamedRef,
    connection: Option<NamedRef>,
    token: Option<TokenInfo>,
}

pub async fn status() -> Result<()> {
    let Some(auth) = resolve_auth()? else {
        anyhow::bail!(NOT_LOGGED_IN);
    };

    let res = http_client()?
        .get(format!("{}/api/v1/goku/me", auth.portal_url))
        .bearer_auth(&auth.token)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("Cannot reach {}: {}", auth.portal_url, e))?;

    let code = res.status();
    let body = res.text().await.unwrap_or_default();
    if code == reqwest::StatusCode::UNAUTHORIZED {
        anyhow::bail!("The Observability Insight token is invalid or has been revoked. Run `goku login` again.");
    }
    if !code.is_success() {
        anyhow::bail!(
            "Observability Insight returned {}: {}",
            code,
            api_error_message(&body).unwrap_or(body)
        );
    }
    let me: MeResponse = serde_json::from_str(&body)
        .map_err(|e| anyhow::anyhow!("Unexpected response from portal: {}", e))?;

    let source = match auth.source {
        CredentialSource::Env => "GOKU_OI_TOKEN",
        CredentialSource::File => "credentials file",
    };
    println!("{}", "Logged in to Observability Insight".green().bold());
    println!("  {:<12} {}", "Portal".yellow().bold(), auth.portal_url);
    println!(
        "  {:<12} {} ({})",
        "Project".yellow().bold(),
        me.project.name,
        me.project.id
    );
    if let Some(conn) = me.connection {
        println!("  {:<12} {} ({})", "Connector".yellow().bold(), conn.name, conn.id);
    }
    if let Some(token) = me.token {
        let label = match (token.name, token.prefix) {
            (Some(n), Some(p)) => format!("{n} ({p}…)"),
            (Some(n), None) => n,
            (None, Some(p)) => format!("{p}…"),
            (None, None) => "-".to_string(),
        };
        println!("  {:<12} {}", "Token".yellow().bold(), label);
    }
    println!("  {:<12} {}", "Source".yellow().bold(), source);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_portal_url_wins_and_is_trimmed() {
        assert_eq!(
            resolve_portal_url(Some("http://localhost:3000/ ")),
            "http://localhost:3000"
        );
    }

    #[test]
    fn api_error_message_prefers_message() {
        assert_eq!(
            api_error_message(r#"{"error":"invalid_grant","message":"Code expired"}"#).as_deref(),
            Some("Code expired")
        );
        assert_eq!(
            api_error_message(r#"{"error":"feature_disabled"}"#).as_deref(),
            Some("feature_disabled")
        );
        assert!(api_error_message("not json").is_none());
    }
}
