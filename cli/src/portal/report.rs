use anyhow::Result;
use chrono::{DateTime, SecondsFormat, Utc};
use colored::Colorize;
use goku_core::settings::Settings;
use serde::Serialize;

use super::credentials::resolve_auth;
use super::{api_error_message, http_client, NOT_LOGGED_IN};
use crate::output::RunSummary;

// ---------------------------------------------------------------------------
// Options & context
// ---------------------------------------------------------------------------

/// `--report*` flags.
#[derive(Debug, Clone, Default)]
pub struct ReportOptions {
    pub name: Option<String>,
    pub tags: Vec<String>,
}

/// Everything about a finished run that the report needs besides the results.
pub struct ReportContext<'a> {
    pub settings: &'a Settings,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Payload (schema_version 1)
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct RunPayload<'a> {
    pub schema_version: u32,
    pub run_id: String,
    pub name: Option<String>,
    pub tags: Vec<String>,
    pub started_at: String,
    pub finished_at: String,
    pub goku_version: &'static str,
    pub config: RunConfig,
    pub results: &'a RunSummary,
    pub environment: RunEnvironment,
}

#[derive(Debug, Serialize)]
pub struct RunConfig {
    pub targets: Vec<String>,
    pub clients: u32,
    pub iterations: Option<u32>,
    pub duration_secs: Option<u64>,
    pub rps_limit: Option<u32>,
    pub http2: bool,
    pub ramp_up_secs: Option<u64>,
    pub timeout_ms: u64,
}

#[derive(Debug, Default, PartialEq, Serialize)]
pub struct RunEnvironment {
    pub git_commit: Option<String>,
    pub git_branch: Option<String>,
    pub ci: bool,
    pub ci_provider: Option<String>,
    pub hostname: Option<String>,
}

pub fn now_rfc3339() -> String {
    to_rfc3339(Utc::now())
}

fn to_rfc3339(t: DateTime<Utc>) -> String {
    t.to_rfc3339_opts(SecondsFormat::Millis, true)
}

pub fn hostname() -> Option<String> {
    ["HOSTNAME", "COMPUTERNAME"]
        .iter()
        .find_map(|k| super::non_empty_env(k))
}

/// Drops query string, fragment and userinfo — they may carry secrets.
/// String based on purpose: parsing with `Url` would percent-encode `{{templates}}`.
pub fn sanitize_url(raw: &str) -> String {
    let without_query = raw.split(['?', '#']).next().unwrap_or("");
    match without_query.find("://") {
        Some(scheme_end) => {
            let rest = &without_query[scheme_end + 3..];
            let authority_end = rest.find('/').unwrap_or(rest.len());
            let authority = &rest[..authority_end];
            let host = authority.rsplit('@').next().unwrap_or(authority);
            format!(
                "{}://{}{}",
                &without_query[..scheme_end],
                host,
                &rest[authority_end..]
            )
        }
        None => without_query.to_string(),
    }
}

fn method_name(op: goku_core::settings::Operation) -> String {
    format!("{:?}", op).to_uppercase()
}

fn targets(settings: &Settings) -> Vec<String> {
    if settings.steps.is_empty() {
        vec![format!(
            "{} {}",
            method_name(settings.operation()),
            sanitize_url(&settings.target_url())
        )]
    } else {
        settings
            .steps
            .iter()
            .map(|s| format!("{} {}", method_name(s.operation()), sanitize_url(&s.url())))
            .collect()
    }
}

pub fn detect_environment(get: impl Fn(&str) -> Option<String>) -> RunEnvironment {
    let mut env = RunEnvironment {
        hostname: get("HOSTNAME").or_else(|| get("COMPUTERNAME")),
        ..Default::default()
    };
    if let Some(sha) = get("GITHUB_SHA") {
        env.git_commit = Some(sha);
        env.git_branch = get("GITHUB_REF_NAME");
        env.ci_provider = Some("github_actions".to_string());
    } else if let Some(sha) = get("CI_COMMIT_SHA") {
        env.git_commit = Some(sha);
        env.git_branch = get("CI_COMMIT_REF_NAME");
        env.ci_provider = Some("gitlab".to_string());
    }
    let generic_ci = get("CI")
        .map(|v| !matches!(v.to_lowercase().as_str(), "false" | "0"))
        .unwrap_or(false);
    env.ci = env.ci_provider.is_some() || generic_ci;
    env
}

pub fn build_payload<'a>(
    ctx: &ReportContext<'_>,
    options: &ReportOptions,
    summary: &'a RunSummary,
) -> RunPayload<'a> {
    let s = ctx.settings;
    RunPayload {
        schema_version: 1,
        run_id: uuid::Uuid::new_v4().to_string(),
        name: options
            .name
            .as_ref()
            .map(|n| n.trim().to_string())
            .filter(|n| !n.is_empty()),
        tags: options
            .tags
            .iter()
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .collect(),
        started_at: to_rfc3339(ctx.started_at),
        finished_at: to_rfc3339(ctx.finished_at),
        goku_version: env!("CARGO_PKG_VERSION"),
        config: RunConfig {
            targets: targets(s),
            clients: s.clients,
            iterations: if s.duration.is_some() {
                None
            } else {
                Some(s.requests)
            },
            duration_secs: s.duration,
            rps_limit: s.rps.filter(|r| *r > 0),
            http2: s.http2,
            ramp_up_secs: s.ramp_up,
            timeout_ms: s.timeout.as_millis() as u64,
        },
        results: summary,
        environment: detect_environment(super::non_empty_env),
    }
}

// ---------------------------------------------------------------------------
// Sending
// ---------------------------------------------------------------------------

#[derive(Debug, serde::Deserialize)]
struct IngestResponse {
    url: Option<String>,
}

pub async fn send_report(
    ctx: ReportContext<'_>,
    options: &ReportOptions,
    summary: &RunSummary,
) -> Result<()> {
    let Some(auth) = resolve_auth()? else {
        anyhow::bail!("Cannot send report. {}", NOT_LOGGED_IN);
    };
    let payload = build_payload(&ctx, options, summary);

    let res = http_client()?
        .post(format!("{}/api/v1/goku/runs", auth.portal_url))
        .bearer_auth(&auth.token)
        .json(&payload)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("Cannot send report to {}: {}", auth.portal_url, e))?;

    let status = res.status();
    let body = res.text().await.unwrap_or_default();
    match status.as_u16() {
        200 | 201 => {
            let url = serde_json::from_str::<IngestResponse>(&body)
                .ok()
                .and_then(|r| r.url);
            match url {
                Some(url) => eprintln!("{} {}", "Report sent:".green().bold(), url),
                None => eprintln!("{}", "Report sent.".green().bold()),
            }
            Ok(())
        }
        401 => anyhow::bail!(
            "Report rejected: the Observability Insight token is invalid or has been revoked. Run `goku login` again."
        ),
        403 => anyhow::bail!(
            "Report rejected: the Goku connector is not enabled for this project ({}).",
            api_error_message(&body).unwrap_or(body)
        ),
        422 => anyhow::bail!(
            "Report rejected: invalid payload ({}).",
            api_error_message(&body).unwrap_or(body)
        ),
        _ => anyhow::bail!(
            "Report failed ({}): {}",
            status,
            api_error_message(&body).unwrap_or(body)
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::time::Duration;

    fn settings(target: &str) -> Settings {
        Settings {
            clients: 5,
            requests: 100,
            target: target.to_string(),
            keep_alive: None,
            body: None,
            headers: None,
            duration: None,
            verbose: false,
            timeout: Duration::from_millis(30_000),
            http2: false,
            ramp_up: None,
            output: Default::default(),
            insecure: false,
            rps: Some(0),
            auth: None,
            output_file: None,
            results_log: None,
            steps: vec![],
            live_stats: None,
            pool_idle_timeout: None,
            disable_keepalive: false,
        }
    }

    fn summary() -> RunSummary {
        RunSummary {
            concurrency: 5,
            duration_secs: 1.5,
            total_requests: 100,
            requests_per_sec: 66.67,
            mean_ms: 12.5,
            min_ms: 1,
            max_ms: 90,
            p50_ms: 10,
            p95_ms: 40,
            p99_ms: 80,
            p999_ms: 90,
            status_2xx: 98,
            status_4xx: 1,
            status_5xx: 1,
            status_other: 0,
            network_errors: 0,
        }
    }

    #[test]
    fn sanitize_strips_query_fragment_and_userinfo() {
        assert_eq!(
            sanitize_url("https://user:secret@api.example.com:8443/v1/users?token=abc#frag"),
            "https://api.example.com:8443/v1/users"
        );
        assert_eq!(sanitize_url("http://localhost:3000"), "http://localhost:3000");
        assert_eq!(
            sanitize_url("http://host/items/{{uuid}}?x=1"),
            "http://host/items/{{uuid}}"
        );
    }

    #[test]
    fn detect_github_actions() {
        let vars: HashMap<&str, &str> = [
            ("GITHUB_SHA", "abc123"),
            ("GITHUB_REF_NAME", "main"),
            ("CI", "true"),
        ]
        .into();
        let env = detect_environment(|k| vars.get(k).map(|v| v.to_string()));
        assert_eq!(env.git_commit.as_deref(), Some("abc123"));
        assert_eq!(env.git_branch.as_deref(), Some("main"));
        assert_eq!(env.ci_provider.as_deref(), Some("github_actions"));
        assert!(env.ci);
    }

    #[test]
    fn detect_local_run() {
        let env = detect_environment(|k| (k == "HOSTNAME").then(|| "laptop".to_string()));
        assert_eq!(
            env,
            RunEnvironment {
                hostname: Some("laptop".into()),
                ..Default::default()
            }
        );
    }

    #[test]
    fn payload_has_contract_shape() {
        let s = settings("POST https://api.example.com/users?key=secret");
        let summary = summary();
        let ctx = ReportContext {
            settings: &s,
            started_at: Utc::now(),
            finished_at: Utc::now(),
        };
        let options = ReportOptions {
            name: Some(" smoke ".into()),
            tags: vec!["ci".into(), " ".into()],
        };
        let v = serde_json::to_value(build_payload(&ctx, &options, &summary)).unwrap();

        assert_eq!(v["schema_version"], 1);
        assert_eq!(v["run_id"].as_str().unwrap().len(), 36);
        assert_eq!(v["name"], "smoke");
        assert_eq!(v["tags"], serde_json::json!(["ci"]));
        assert!(v["started_at"].as_str().unwrap().ends_with('Z'));
        assert_eq!(
            v["config"]["targets"],
            serde_json::json!(["POST https://api.example.com/users"])
        );
        assert_eq!(v["config"]["iterations"], 100);
        assert!(v["config"]["duration_secs"].is_null());
        assert!(v["config"]["rps_limit"].is_null());
        assert_eq!(v["config"]["timeout_ms"], 30_000);
        assert_eq!(v["results"]["p95_ms"], 40);
        assert_eq!(v["results"]["status_other"], 0);
        assert!(v["environment"].get("ci").is_some());
    }

    #[test]
    fn duration_runs_have_null_iterations() {
        let mut s = settings("https://api.example.com");
        s.duration = Some(10);
        let summary = summary();
        let ctx = ReportContext {
            settings: &s,
            started_at: Utc::now(),
            finished_at: Utc::now(),
        };
        let v = serde_json::to_value(build_payload(&ctx, &ReportOptions::default(), &summary))
            .unwrap();
        assert!(v["config"]["iterations"].is_null());
        assert_eq!(v["config"]["duration_secs"], 10);
        assert_eq!(
            v["config"]["targets"],
            serde_json::json!(["GET https://api.example.com"])
        );
        assert!(v["name"].is_null());
    }
}
