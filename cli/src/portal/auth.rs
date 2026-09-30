use std::time::Duration;

use anyhow::Result;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use colored::Colorize;
use reqwest::Url;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use super::credentials::{self, Credentials};
use super::{api_error_message, http_client, resolve_portal_url};

const LOGIN_TIMEOUT: Duration = Duration::from_secs(300);

// ---------------------------------------------------------------------------
// PKCE (RFC 7636, S256)
// ---------------------------------------------------------------------------

fn random_b64url(len: usize) -> String {
    let mut bytes = vec![0u8; len];
    rand::fill(&mut bytes[..]);
    URL_SAFE_NO_PAD.encode(bytes)
}

pub fn code_challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

// ---------------------------------------------------------------------------
// Loopback callback
// ---------------------------------------------------------------------------

#[derive(Debug, PartialEq)]
pub struct CallbackParams {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
}

/// Parses the request target of the loopback redirect (e.g. `/callback?code=..&state=..`).
/// Returns `None` for any other path (favicon, etc.).
pub fn parse_callback_target(target: &str) -> Option<CallbackParams> {
    let url = Url::parse(&format!("http://127.0.0.1{target}")).ok()?;
    if url.path() != "/callback" {
        return None;
    }
    let mut params = CallbackParams {
        code: None,
        state: None,
        error: None,
    };
    for (k, v) in url.query_pairs() {
        match k.as_ref() {
            "code" => params.code = Some(v.into_owned()),
            "state" => params.state = Some(v.into_owned()),
            "error" => params.error = Some(v.into_owned()),
            _ => {}
        }
    }
    Some(params)
}

async fn respond(stream: &mut TcpStream, status: &str, title: &str, message: &str) {
    let body = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>Goku</title></head>\
<body style=\"font-family:system-ui,sans-serif;background:#09090b;color:#fafafa;display:flex;\
align-items:center;justify-content:center;height:100vh;margin:0\">\
<div style=\"text-align:center\"><h1>{title}</h1><p>{message}</p></div></body></html>"
    );
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.shutdown().await;
}

async fn read_request_target(stream: &mut TcpStream) -> Option<String> {
    let mut buf = Vec::with_capacity(2048);
    let mut chunk = [0u8; 1024];
    while !buf.windows(4).any(|w| w == b"\r\n\r\n") && buf.len() < 16 * 1024 {
        let n = stream.read(&mut chunk).await.ok()?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    let text = String::from_utf8_lossy(&buf);
    let mut parts = text.lines().next()?.split_whitespace();
    match (parts.next(), parts.next()) {
        (Some("GET"), Some(target)) => Some(target.to_string()),
        _ => None,
    }
}

/// Waits for the browser redirect and returns the authorization code.
async fn wait_for_code(listener: &TcpListener, expected_state: &str) -> Result<String> {
    loop {
        let (mut stream, _) = listener.accept().await?;
        let Some(target) = read_request_target(&mut stream).await else {
            respond(&mut stream, "400 Bad Request", "Bad request", "").await;
            continue;
        };
        let Some(params) = parse_callback_target(&target) else {
            respond(&mut stream, "404 Not Found", "Not found", "").await;
            continue;
        };

        if params.state.as_deref() != Some(expected_state) {
            respond(
                &mut stream,
                "400 Bad Request",
                "Login failed",
                "State mismatch. Please run <code>goku login</code> again.",
            )
            .await;
            anyhow::bail!("Login failed: state mismatch in the portal redirect");
        }
        if let Some(error) = params.error {
            respond(
                &mut stream,
                "200 OK",
                "Login cancelled",
                "You can close this window and return to the terminal.",
            )
            .await;
            anyhow::bail!("Login cancelled in the portal ({error})");
        }
        let Some(code) = params.code else {
            respond(&mut stream, "400 Bad Request", "Login failed", "Missing code.").await;
            anyhow::bail!("Login failed: the portal did not return an authorization code");
        };

        respond(
            &mut stream,
            "200 OK",
            "Goku is connected",
            "You can close this window and return to the terminal.",
        )
        .await;
        return Ok(code);
    }
}

fn open_browser(url: &str) {
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open").arg(url).spawn();
    #[cfg(target_os = "windows")]
    let result = std::process::Command::new("cmd")
        .args(["/C", "start", "", url])
        .spawn();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let result = std::process::Command::new("xdg-open")
        .arg(url)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
    let _ = result;
}

// ---------------------------------------------------------------------------
// Token exchange
// ---------------------------------------------------------------------------

#[derive(Debug, serde::Deserialize)]
struct ProjectRef {
    id: String,
    name: String,
}

#[derive(Debug, serde::Deserialize)]
struct TokenResponse {
    access_token: String,
    project: ProjectRef,
    connection_id: Option<String>,
}

async fn exchange_code(
    portal_url: &str,
    code: &str,
    verifier: &str,
    redirect_uri: &str,
) -> Result<TokenResponse> {
    let res = http_client()?
        .post(format!("{portal_url}/api/v1/goku/token"))
        .json(&serde_json::json!({
            "grant_type": "authorization_code",
            "code": code,
            "code_verifier": verifier,
            "redirect_uri": redirect_uri,
        }))
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("Cannot reach {}: {}", portal_url, e))?;

    let status = res.status();
    let body = res.text().await.unwrap_or_default();
    if !status.is_success() {
        anyhow::bail!(
            "Token exchange failed ({}): {}",
            status,
            api_error_message(&body).unwrap_or(body)
        );
    }
    serde_json::from_str(&body)
        .map_err(|e| anyhow::anyhow!("Unexpected token response from portal: {}", e))
}

// ---------------------------------------------------------------------------
// goku login
// ---------------------------------------------------------------------------

pub async fn login(portal_url: Option<String>, no_browser: bool) -> Result<()> {
    let portal_url = resolve_portal_url(portal_url.as_deref());

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    let redirect_uri = format!("http://127.0.0.1:{port}/callback");

    let verifier = random_b64url(32);
    let state = random_b64url(32);
    let client_name = super::report::hostname().unwrap_or_else(|| "goku-cli".to_string());

    let authorize_url = Url::parse_with_params(
        &format!("{portal_url}/cli/goku/authorize"),
        &[
            ("redirect_uri", redirect_uri.as_str()),
            ("state", state.as_str()),
            ("code_challenge", code_challenge(&verifier).as_str()),
            ("code_challenge_method", "S256"),
            ("client_name", client_name.as_str()),
        ],
    )?;

    println!(
        "{}",
        "Log in to Observability Insight and choose the project for your reports:"
            .cyan()
            .bold()
    );
    println!("  {}", authorize_url.as_str().underline());
    if !no_browser {
        open_browser(authorize_url.as_str());
    }
    println!("Waiting for the browser (press Ctrl+C to cancel)…");

    let code = tokio::time::timeout(LOGIN_TIMEOUT, wait_for_code(&listener, &state))
        .await
        .map_err(|_| anyhow::anyhow!("Login timed out after 5 minutes"))??;

    let token = exchange_code(&portal_url, &code, &verifier, &redirect_uri).await?;

    let path = credentials::save(&Credentials {
        portal_url: portal_url.clone(),
        token: token.access_token,
        project_id: token.project.id,
        project_name: token.project.name.clone(),
        connection_id: token.connection_id,
        created_at: super::report::now_rfc3339(),
    })?;

    println!(
        "{} reports will go to project {}",
        "Logged in —".green().bold(),
        token.project.name.bold()
    );
    println!("Credentials saved to {}", path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_challenge_is_base64url_sha256() {
        // printf '<verifier>' | openssl dgst -sha256 -binary | base64 | tr '+/' '-_' | tr -d '='
        assert_eq!(
            code_challenge("dBjftJeZ4CVP-mJ92K5hqXhwHKKsIPT8DgbiN8FbZrk"),
            "gl4_ttR8EG7hp0OTNWSevfntNPWK0Wf-G1PWZVMOyKM"
        );
    }

    #[test]
    fn random_verifier_has_valid_length() {
        let v = random_b64url(32);
        assert_eq!(v.len(), 43);
        assert_ne!(v, random_b64url(32));
    }

    #[test]
    fn parse_callback_with_code_and_state() {
        let p = parse_callback_target("/callback?code=ac_123&state=xyz%3D").unwrap();
        assert_eq!(p.code.as_deref(), Some("ac_123"));
        assert_eq!(p.state.as_deref(), Some("xyz="));
        assert!(p.error.is_none());
    }

    #[test]
    fn parse_callback_with_error() {
        let p = parse_callback_target("/callback?error=access_denied&state=s").unwrap();
        assert_eq!(p.error.as_deref(), Some("access_denied"));
        assert!(p.code.is_none());
    }

    #[test]
    fn parse_callback_ignores_other_paths() {
        assert!(parse_callback_target("/favicon.ico").is_none());
    }
}
