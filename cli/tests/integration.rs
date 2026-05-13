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
            .body_contains("uuid_");
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
