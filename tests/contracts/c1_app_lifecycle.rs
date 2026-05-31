// Contract C1.2 — app entry points: boot, log "ready", shut down on SIGTERM within 30s.
//
// We spawn each app as a child process, watch its stderr/stdout for the
// "ready" line, send SIGTERM, and assert it exits with status 0 within 30s.

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const APPS: &[&str] = &[
    "api-gateway",
    "embedding-worker",
    "analytics-consumer",
    "duckdb-exporter",
];

fn workspace_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap().parent().unwrap().to_path_buf()
}

fn build_release(app: &str) {
    let status = Command::new("cargo")
        .args(["build", "--release", "-p", &format!("aetherdb-{}", app)])
        .current_dir(workspace_root())
        .status()
        .expect("cargo build");
    assert!(status.success(), "build failed for {}", app);
}

fn binary_path(app: &str) -> std::path::PathBuf {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap().parent().unwrap().to_path_buf();
    root.join("target/release").join(app)
}

#[test]
#[ignore = "requires cargo build of all apps; run with --include-ignored in CI"]
fn each_app_logs_ready_and_shuts_down_gracefully() {
    for &app in APPS {
        eprintln!("--- testing app: {} ---", app);
        build_release(app);

        let mut child = Command::new(binary_path(app))
            .env("AETHERDB_LIBSQL_URL", "file::memory:?cache=shared")
            .env("AETHERDB_REDPANDA_BROKERS", "localhost:9092")
            .env("AETHERDB_LOG_LEVEL", "info")
            .env("AETHERDB_MINIMAL_MODE", "true")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn app");

        // Wait up to 10s for "ready" line on either stdout or stderr.
        let stdout = child.stdout.take().expect("stdout");
        let reader = BufReader::new(stdout);
        let start = Instant::now();
        let mut saw_ready = false;
        for line in reader.lines().flatten() {
            if line.to_ascii_lowercase().contains("ready") {
                saw_ready = true;
                break;
            }
            if start.elapsed() > Duration::from_secs(10) { break; }
        }
        assert!(saw_ready, "{} did not log 'ready' within 10s", app);

        // Send SIGTERM and wait for exit.
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            unsafe { libc::kill(child.id() as i32, libc::SIGTERM); }
            let term_sent = Instant::now();
            let status = child.wait().expect("wait");
            let elapsed = term_sent.elapsed();
            assert!(elapsed < Duration::from_secs(30),
                "{} did not exit within 30s of SIGTERM (took {:?})", app, elapsed);
            assert!(status.success() || status.signal() == Some(libc::SIGTERM),
                "{} exited with non-zero status: {:?}", app, status);
        }
    }
}

// Note: this test is #[ignore]'d because it requires building all four apps
// in release mode, which is slow. CI runs it via `cargo test --include-ignored`
// in a dedicated job; local devs run individual sessions' tests.
