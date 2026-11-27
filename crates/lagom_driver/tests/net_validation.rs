//! The M2 validation gate (docs/10-roadmap.md): "a small HTTP client+server
//! project passes validation". `validation/http_server.lagom` and
//! `validation/http_client.lagom` run as real processes and runs, and every
//! backend pairing must complete the same HTTP/1.1 exchange:
//!
//! - native server with native client,
//! - native server with interpreted client,
//! - interpreted server with native client,
//! - interpreted server with interpreted client.
//!
//! The interpreter and the native runtime keep separate net registries with
//! mirrored shapes and messages (`lagom_interp::net_op` ↔
//! `lagom_rt::rt_net_op`); these pairings assert the observable contract at
//! both ends.
//!
//! Coordination: the server serves exactly two connections, so two client
//! runs close its serve loop. A client that runs before the server binds
//! reports the honest connect failure and the harness retries it (bounded)
//! — readiness races are absorbed there, never by the programs.

use std::io::BufRead;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::time::Duration;

static SEQ: AtomicUsize = AtomicUsize::new(0);

/// The corpus sources.
fn read_case(name: &str) -> String {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../validation")
        .canonicalize()
        .expect("validation dir");
    std::fs::read_to_string(dir.join(format!("{name}.lagom"))).expect("validation source")
}

/// Build a native executable from Lagom source (the differential suite's
/// path). Returns the executable path.
fn build_native_named(name: &str, src: &str, dev: bool) -> std::path::PathBuf {
    let suffix = if cfg!(windows) { ".exe" } else { "" };
    let n = SEQ.fetch_add(1, Ordering::SeqCst);
    let out = std::env::temp_dir().join(format!(
        "lagom_net_{}_{}_{}{suffix}",
        name,
        std::process::id(),
        n
    ));
    let _ = std::fs::remove_file(&out);
    lagom_driver::build_native(src, &out, dev, true)
        .unwrap_or_else(|e| panic!("building `{name}` failed: {e:?}"))
}

/// Run one native client against the server; returns its combined output.
fn run_native_client(exe: &std::path::Path) -> String {
    let mut child = Command::new(exe)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn the client");
    let _ = child.stdin.take();
    let out = child.wait_with_output().expect("wait for the client");
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

/// The interpreted client run, in-process — the same backend
/// `lagom run --trace` drives. Returns the program's output lines.
fn run_interp_client(src: &str) -> Vec<String> {
    let (host, outcome) = lagom_driver::run_interpreted(src, Vec::new(), None);
    assert!(
        matches!(outcome, lagom_interp::RunOutcome::Completed),
        "the interp client must complete: {outcome:?} — stdout {:?}",
        host.stdout
    );
    host.stdout
}

/// The response every client must report, whatever way `take` chunks it.
fn assert_response(client_lines: &[String]) {
    let out = client_lines.join("\n");
    assert!(
        out.contains("HTTP/1.1 200 OK"),
        "the client must see the status line: {client_lines:?}"
    );
    assert!(
        out.contains("Content-Length: 5"),
        "the client must see the header: {client_lines:?}"
    );
    assert!(
        out.contains("hello"),
        "the client must see the body: {client_lines:?}"
    );
}

const SUCCESS_MARK: &str = "served 2 connections";

/// One pairing. `port_patch` rewrites the validation sources' port so
/// parallel tests never bind the same one.
fn pairing(server_native: bool, client_native: bool, port: u16) {
    let port = port.to_string();
    let server_src = read_case("http_server").replace("8093", &port);
    let client_src = read_case("http_client").replace("8093", &port);

    // The server side. Native: a child process (compile → link → bind →
    // accept). Interpreted: a thread owns the interpreter; the run's output
    // and outcome land on a channel when it completes.
    enum ServerSide {
        Native(std::process::Child),
        Interpreted(mpsc::Receiver<(Vec<String>, lagom_interp::RunOutcome)>),
    }
    let server: ServerSide = if server_native {
        let exe = build_native_named("http_server", &server_src, true);
        let child = Command::new(&exe)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn the native server");
        ServerSide::Native(child)
    } else {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut fe = match lagom_driver::frontend(&server_src) {
                Ok(fe) => fe,
                Err(e) => panic!("the interp server must front-end: {e:?}"),
            };
            let mut it = lagom_interp::Interp::dev(&mut fe.mir);
            it.host = lagom_interp::Host::with_seed(Vec::new(), 1);
            let outcome = it.run(&fe.mir, &server_src);
            let host = std::mem::replace(&mut it.host, lagom_interp::Host::new(Vec::new()));
            let _ = tx.send((host.stdout, outcome));
        });
        ServerSide::Interpreted(rx)
    };

    // The client loops: two completed exchanges close the server's loop.
    let mut successes = 0;
    for attempt in 0..200 {
        let lines: Vec<String> = if client_native {
            let exe = build_native_named("http_client", &client_src, true);
            run_native_client(&exe).lines().map(|l| l.to_string()).collect()
        } else {
            run_interp_client(&client_src)
        };
        if lines.iter().any(|l| l.contains("hello")) {
            assert_response(&lines);
            successes += 1;
            if successes == 2 {
                break;
            }
        } else {
            // Not ready yet (the server is still binding): retry.
            assert!(
                attempt < 199,
                "the client never completed the exchange: {lines:?}"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    // The server must now have finished its serve loop and said the marker.
    match server {
        // Native: read until the pipe closes, then exit.
        ServerSide::Native(mut child) => {
            let mut reader =
                std::io::BufReader::new(child.stdout.take().expect("server stdout pipe"));
            let mut text = String::new();
            loop {
                let mut line = String::new();
                match reader.read_line(&mut line) {
                    Ok(0) => break,
                    Ok(_) => text.push_str(&line),
                    Err(_) => break,
                }
                if text.contains(SUCCESS_MARK) {
                    break;
                }
            }
            let _ = child.wait();
            assert!(
                text.contains(SUCCESS_MARK),
                "the native server must serve both connections: {text:?}"
            );
        }
        // Interpreted: the thread's channel carries the run's full output.
        ServerSide::Interpreted(rx) => {
            let (lines, outcome) = rx.recv().expect("the interp server thread must finish");
            assert!(
                matches!(outcome, lagom_interp::RunOutcome::Completed),
                "the interp server must complete: {outcome:?}"
            );
            assert!(
                lines.iter().any(|l| l.contains(SUCCESS_MARK)),
                "the interp server must report the marker: {lines:?}"
            );
        }
    }
}

#[test]
fn native_server_with_native_client() {
    pairing(true, true, 8091);
}

#[test]
fn native_server_with_interp_client() {
    pairing(true, false, 8092);
}

#[test]
fn interp_server_with_native_client() {
    pairing(false, true, 8094);
}

#[test]
fn interp_server_with_interp_client() {
    pairing(false, false, 8095);
}
