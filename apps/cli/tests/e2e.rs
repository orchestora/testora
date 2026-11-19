//! Runs the real `testora` binary against the demo API served in-process on
//! a random loopback port, so tests never clash with local services.

use std::{
    process::{Command, Output},
    time::{Duration, Instant},
};

use tokio::runtime::Runtime;
use uuid::Uuid;

struct DemoApi {
    url: String,
    // Dropping the runtime stops the server.
    _runtime: Runtime,
}

fn start_api() -> DemoApi {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let listener = runtime
        .block_on(tokio::net::TcpListener::bind("127.0.0.1:0"))
        .unwrap();
    let addr = listener.local_addr().unwrap();
    runtime.spawn(async move { axum::serve(listener, testora_api::app()).await.unwrap() });
    DemoApi {
        url: format!("http://{addr}"),
        _runtime: runtime,
    }
}

fn testora(api_url: &str, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_testora"))
        .env("TESTORA_API_URL", api_url)
        .args(args)
        .output()
        .expect("run testora binary")
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

#[track_caller]
fn assert_ok(out: &Output) -> String {
    assert!(
        out.status.success(),
        "expected success, got {:?}\nstdout: {}\nstderr: {}",
        out.status.code(),
        stdout(out),
        stderr(out)
    );
    stdout(out)
}

#[track_caller]
fn assert_fails(out: &Output, code: i32, needle: &str) {
    assert_eq!(
        out.status.code(),
        Some(code),
        "unexpected exit code\nstdout: {}\nstderr: {}",
        stdout(out),
        stderr(out)
    );
    assert!(
        stderr(out).contains(needle),
        "expected {needle:?} in stderr: {}",
        stderr(out)
    );
}

fn created_id(out: &str) -> Uuid {
    out.split_whitespace()
        .next()
        .and_then(|id| id.parse().ok())
        .unwrap_or_else(|| panic!("create output should start with a UUID: {out:?}"))
}

#[test]
fn ping_reports_healthy_api() {
    let api = start_api();
    let out = assert_ok(&testora(&api.url, &["ping"]));
    assert_eq!(out.trim(), "testora-api: ok");
}

#[test]
fn full_environment_lifecycle() {
    let api = start_api();

    let out = assert_ok(&testora(&api.url, &["env", "list"]));
    assert!(
        out.trim().is_empty(),
        "fresh API should list nothing: {out:?}"
    );

    let out = assert_ok(&testora(
        &api.url,
        &["env", "create", "--project", "demo", "--region", "us-east"],
    ));
    let id = created_id(&out);
    assert!(out.contains("Requested"), "{out}");

    let out = assert_ok(&testora(&api.url, &["env", "list"]));
    assert_eq!(out.lines().count(), 1);
    assert!(out.contains(&id.to_string()) && out.contains("demo") && out.contains("us-east"));

    let out = assert_ok(&testora(&api.url, &["env", "status", &id.to_string()]));
    assert!(out.contains("Requested"), "{out}");

    for _ in 0..2 {
        let out = assert_ok(&testora(&api.url, &["env", "destroy", &id.to_string()]));
        assert!(
            out.contains("DestroyRequested"),
            "destroy must be repeatable: {out}"
        );
    }

    let out = assert_ok(&testora(&api.url, &["env", "status", &id.to_string()]));
    assert!(out.contains("DestroyRequested"), "{out}");
}

#[test]
fn default_region_is_used_when_omitted() {
    let api = start_api();
    let id = created_id(&assert_ok(&testora(
        &api.url,
        &["env", "create", "--project", "demo"],
    )));
    let out = assert_ok(&testora(&api.url, &["env", "status", &id.to_string()]));
    assert!(out.contains("us-east"), "{out}");
}

#[test]
fn unknown_environment_reports_api_error() {
    let api = start_api();
    for cmd in ["status", "destroy"] {
        let out = testora(&api.url, &["env", cmd, &Uuid::new_v4().to_string()]);
        assert_fails(&out, 1, "environment not found");
        assert!(stderr(&out).contains("404"), "{}", stderr(&out));
    }
}

#[test]
fn invalid_arguments_fail_before_any_request() {
    let api = start_api();
    for args in [
        &["env", "create", "--project", "demo app"][..],
        &["env", "create", "--project", "demo", "--region", "US_EAST"],
        &["env", "status", "not-a-uuid"],
    ] {
        assert_fails(&testora(&api.url, args), 2, "invalid value");
    }
    let out = assert_ok(&testora(&api.url, &["env", "list"]));
    assert!(out.trim().is_empty(), "nothing may be created: {out:?}");
}

#[test]
fn non_loopback_api_urls_are_refused() {
    for url in [
        "http://127.0.0.1:8080@evil.example",
        "https://api.example.com",
        "http://10.0.0.1:8080",
    ] {
        assert_fails(&testora(url, &["ping"]), 1, "API URL");
    }
}

#[test]
fn unreachable_api_fails_fast_with_hint() {
    // Reserve a port by binding without listening, and hold it for the whole
    // test. Connections are refused, and no parallel test can claim the
    // port. (Binding a listener and dropping it raced: another test's server
    // could reuse the freed port.)
    let reserved = tokio::net::TcpSocket::new_v4().unwrap();
    reserved.bind("127.0.0.1:0".parse().unwrap()).unwrap();
    let port = reserved.local_addr().unwrap().port();
    let started = Instant::now();
    let out = testora(&format!("http://127.0.0.1:{port}"), &["ping"]);
    assert_fails(&out, 1, "could not reach the Testora API");
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "should fail fast, took {:?}",
        started.elapsed()
    );
}
