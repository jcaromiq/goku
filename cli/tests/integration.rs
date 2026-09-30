use assert_cmd::Command;
use httpmock::MockServer;
use predicates::prelude::*;
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn test_basic_get_requests() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method("GET").path("/api");
        then.status(200);
    });

    let mut cmd = Command::cargo_bin("goku").unwrap();
    cmd.arg("-c")
        .arg("2")
        .arg("-i")
        .arg("10")
        .arg("--target")
        .arg(server.url("/api"));

    cmd.assert()
        .success()
        .stdout(predicate::str::is_match(r"Total requests\s+10").unwrap())
        .stdout(predicate::str::is_match(r"2xx.*10").unwrap());

    mock.assert_hits(10);
}

#[test]
fn test_post_with_body_and_headers() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method("POST")
            .path("/submit")
            .header("X-Custom", "test")
            .body("{\"key\":\"value\"}");
        then.status(201);
    });

    // Create a temporary file for the body
    let mut file = NamedTempFile::new().unwrap();
    write!(file, "{{\"key\":\"value\"}}").unwrap();

    let mut cmd = Command::cargo_bin("goku").unwrap();
    cmd.arg("-c")
        .arg("1")
        .arg("-i")
        .arg("5")
        .arg("--headers")
        .arg("X-Custom:test")
        .arg("--request-body")
        .arg(file.path())
        .arg("--target")
        .arg(format!("POST {}", server.url("/submit")));

    cmd.assert()
        .success()
        .stdout(predicate::str::is_match(r"2xx.*5").unwrap());

    mock.assert_hits(5);
}

#[test]
fn test_rate_limiting() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method("GET").path("/limit");
        then.status(200);
    });

    let start = std::time::Instant::now();

    let mut cmd = Command::cargo_bin("goku").unwrap();
    // 5 requests at 2 requests per second should take at least 2 seconds
    cmd.arg("-c")
        .arg("1")
        .arg("-i")
        .arg("5")
        .arg("--rps")
        .arg("2")
        .arg("--target")
        .arg(server.url("/limit"));

    cmd.assert().success();
    mock.assert_hits(5);

    let elapsed = start.elapsed();
    assert!(elapsed.as_secs() >= 2);
}

#[test]
fn test_server_errors_5xx() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method("GET").path("/fail");
        then.status(500);
    });

    let mut cmd = Command::cargo_bin("goku").unwrap();
    cmd.arg("-c")
        .arg("2")
        .arg("-i")
        .arg("8")
        .arg("--target")
        .arg(server.url("/fail"));

    cmd.assert()
        .success()
        .stdout(predicate::str::is_match(r"5xx.*8").unwrap());

    mock.assert_hits(8);
}

#[test]
fn test_auth_bearer() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method("GET")
            .path("/protected")
            .header("Authorization", "Bearer my-secret-token");
        then.status(200);
    });

    let mut cmd = Command::cargo_bin("goku").unwrap();
    cmd.arg("-i")
        .arg("3")
        .arg("--auth-bearer")
        .arg("my-secret-token")
        .arg("--target")
        .arg(server.url("/protected"));

    cmd.assert().success();
    mock.assert_hits(3);
}

#[test]
fn test_multi_step_scenario() {
    let server = MockServer::start();
    
    // Step 1: GET /step1
    let mock1 = server.mock(|when, then| {
        when.method("GET").path("/step1");
        then.status(200);
    });
    
    // Step 2: POST /step2
    let mock2 = server.mock(|when, then| {
        when.method("POST")
            .path("/step2")
            .header("Content-Type", "application/json")
            // Can't easily match templated UUID body exactly, so we match any body
            .body_includes("uuid_");
        then.status(201);
    });

    let scenario_yaml = format!(
        r#"
clients: 2
requests: 4
steps:
  - target: "{}"
  - target: "POST {}"
    body: '{{"uuid_": "{{{{uuid}}}}"}}'
    headers:
      - key: "Content-Type"
        value: "application/json"
"#,
        server.url("/step1"),
        server.url("/step2")
    );

    let mut file = NamedTempFile::new().unwrap();
    write!(file, "{}", scenario_yaml).unwrap();

    let mut cmd = Command::cargo_bin("goku").unwrap();
    cmd.arg("--scenario").arg(file.path());

    // 2 clients * 2 iterations = 4 total sequence executions?
    // Actually, goku execution logic executes ONE step per iteration.
    // So 4 total iterations = 2 hits on step1, 2 hits on step2.
    cmd.assert()
        .success()
        .stdout(predicate::str::is_match(r"Total requests\s+4").unwrap())
        .stdout(predicate::str::is_match(r"2xx.*4").unwrap());

    mock1.assert_hits(2);
    mock2.assert_hits(2);
}

#[test]
fn test_compare_subcommand() {
    let base_json = r#"{
        "requests_per_sec": 100.0,
        "mean_ms": 50.0,
        "p50_ms": 40,
        "p95_ms": 60,
        "p99_ms": 70,
        "p999_ms": 80,
        "min_ms": 10,
        "max_ms": 90,
        "total_requests": 1000,
        "status_2xx": 1000,
        "status_4xx": 0,
        "status_5xx": 0,
        "network_errors": 0
    }"#;
    let cand_json = r#"{
        "requests_per_sec": 120.0,
        "mean_ms": 40.0,
        "p50_ms": 35,
        "p95_ms": 55,
        "p99_ms": 65,
        "p999_ms": 75,
        "min_ms": 8,
        "max_ms": 85,
        "total_requests": 1000,
        "status_2xx": 1000,
        "status_4xx": 0,
        "status_5xx": 0,
        "network_errors": 0
    }"#;

    let mut base_file = NamedTempFile::new().unwrap();
    write!(base_file, "{}", base_json).unwrap();

    let mut cand_file = NamedTempFile::new().unwrap();
    write!(cand_file, "{}", cand_json).unwrap();

    let mut cmd = Command::cargo_bin("goku").unwrap();
    cmd.arg("compare")
        .arg(base_file.path())
        .arg(cand_file.path());

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Benchmark Comparison"))
        .stdout(predicate::str::contains("Requests/sec"))
        .stdout(predicate::str::contains("+20.0%"));
}

// ---------------------------------------------------------------------------
// Observability Insight integration
// ---------------------------------------------------------------------------

fn portal_cmd(portal: &MockServer, config_dir: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("goku").unwrap();
    cmd.env("GOKU_OI_URL", portal.base_url())
        .env("GOKU_CONFIG_DIR", config_dir)
        .env_remove("GOKU_OI_TOKEN")
        .env_remove("GITHUB_SHA")
        .env_remove("CI_COMMIT_SHA");
    cmd
}

#[test]
fn test_report_sends_results_to_portal() {
    let target = MockServer::start();
    let target_mock = target.mock(|when, then| {
        when.method("GET").path("/api");
        then.status(200);
    });

    let portal = MockServer::start();
    let ingest = portal.mock(|when, then| {
        when.method("POST")
            .path("/api/v1/goku/runs")
            .header("authorization", "Bearer oi_goku_test")
            .json_body_includes(
                r#"{"schema_version":1,"name":"smoke","tags":["ci"],
                    "config":{"clients":1,"iterations":3,"duration_secs":null},
                    "results":{"total_requests":3,"status_2xx":3}}"#,
            );
        then.status(201).json_body(serde_json::json!({
            "id": "r1", "run_id": "x", "url": "https://portal.test/runs/r1"
        }));
    });

    let dir = tempfile::tempdir().unwrap();
    portal_cmd(&portal, dir.path())
        .env("GOKU_OI_TOKEN", "oi_goku_test")
        .args(["-i", "3", "--report", "--report-name", "smoke", "--report-tag", "ci"])
        .arg("--target")
        .arg(format!("{}?secret=1", target.url("/api")))
        .assert()
        .success()
        .stderr(predicate::str::contains("Report sent: https://portal.test/runs/r1"));

    target_mock.assert_calls(3);
    ingest.assert();
}

#[test]
fn test_report_fails_with_revoked_token() {
    let target = MockServer::start();
    target.mock(|when, then| {
        when.method("GET").path("/api");
        then.status(200);
    });
    let portal = MockServer::start();
    portal.mock(|when, then| {
        when.method("POST").path("/api/v1/goku/runs");
        then.status(401).json_body(serde_json::json!({"error": "unauthorized"}));
    });

    let dir = tempfile::tempdir().unwrap();
    portal_cmd(&portal, dir.path())
        .env("GOKU_OI_TOKEN", "oi_goku_revoked")
        .args(["-i", "1", "--report", "--target"])
        .arg(target.url("/api"))
        .assert()
        .failure()
        .stderr(predicate::str::contains("revoked"));
}

#[test]
fn test_report_without_credentials_fails() {
    let target = MockServer::start();
    target.mock(|when, then| {
        when.method("GET").path("/api");
        then.status(200);
    });
    let portal = MockServer::start();
    let dir = tempfile::tempdir().unwrap();
    portal_cmd(&portal, dir.path())
        .args(["-i", "1", "--report", "--target"])
        .arg(target.url("/api"))
        .assert()
        .failure()
        .stderr(predicate::str::contains("goku login"));
}

#[test]
fn test_status_shows_project() {
    let portal = MockServer::start();
    let me = portal.mock(|when, then| {
        when.method("GET")
            .path("/api/v1/goku/me")
            .header("authorization", "Bearer oi_goku_test");
        then.status(200).json_body(serde_json::json!({
            "project": {"id": "p1", "name": "Checkout API"},
            "connection": {"id": "c1", "name": "Goku"},
            "token": {"name": "CLI (laptop)", "prefix": "oi_goku_ab"}
        }));
    });

    let dir = tempfile::tempdir().unwrap();
    portal_cmd(&portal, dir.path())
        .env("GOKU_OI_TOKEN", "oi_goku_test")
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("Checkout API"))
        .stdout(predicate::str::contains("GOKU_OI_TOKEN"));
    me.assert();
}

#[test]
fn test_status_uses_credentials_file_and_logout_removes_it() {
    let portal = MockServer::start();
    let me = portal.mock(|when, then| {
        when.method("GET")
            .path("/api/v1/goku/me")
            .header("authorization", "Bearer oi_goku_file");
        then.status(200).json_body(serde_json::json!({
            "project": {"id": "p1", "name": "From file"}
        }));
    });

    let dir = tempfile::tempdir().unwrap();
    let creds = dir.path().join("credentials.json");
    std::fs::write(
        &creds,
        serde_json::json!({
            "portal_url": "https://unused.example.com",
            "token": "oi_goku_file",
            "project_id": "p1",
            "project_name": "From file",
            "connection_id": null,
            "created_at": "2026-09-29T10:00:00.000Z"
        })
        .to_string(),
    )
    .unwrap();

    portal_cmd(&portal, dir.path())
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("From file"));
    me.assert();

    portal_cmd(&portal, dir.path())
        .arg("logout")
        .assert()
        .success()
        .stdout(predicate::str::contains("Logged out"));
    assert!(!creds.exists());

    portal_cmd(&portal, dir.path())
        .arg("status")
        .assert()
        .failure()
        .stderr(predicate::str::contains("goku login"));
}

#[test]
fn test_report_name_requires_report() {
    Command::cargo_bin("goku")
        .unwrap()
        .args(["--target", "http://localhost:1", "--report-name", "x"])
        .assert()
        .failure();
}
