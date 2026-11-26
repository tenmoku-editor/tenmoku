//! A server's error output reaches the app log at warn, a bounded number
//! of lines a session.

use std::path::Path;

use lsp_types::notification::LogMessage;
use lsp_types::{LogMessageParams, MessageType};

use super::{ROOT, capture_logs, logs_at, manager, wait_until};
use crate::ServerSpec;
use crate::fake::{FakeServer, Script};

/// Starts a fake named `name` whose thread's records are captured.
fn started(name: &str) -> (FakeServer, crate::LspManager) {
    capture_logs(&format!("fe-lsp {name}"));
    let fake = FakeServer::new(Script::default());
    let (mut manager, events) = manager(fake.launcher(), None);
    std::mem::forget(events);
    manager
        .start(ServerSpec::new(name, "fake-server"), Path::new(ROOT))
        .unwrap();
    fake.wait_initialized();
    (fake, manager)
}

fn warns(name: &str) -> Vec<String> {
    logs_at(&format!("fe-lsp {name}"), log::Level::Warn)
}

#[test]
fn error_lines_reach_the_app_log_at_warn() {
    let (fake, manager) = started("errors");
    fake.stderr(b"2026-11-23T10:00:00Z ERROR failed to run build scripts\n");
    fake.stderr(b"INFO indexing\n");
    wait_until("both in the ring", || {
        let log = manager.log("errors", Path::new(ROOT));
        log.contains("failed to run build scripts") && log.contains("INFO indexing")
    });
    // Errors are warned before they reach the ring.
    let warned: Vec<_> = warns("errors")
        .into_iter()
        .filter(|line| line.contains("failed to run build scripts"))
        .collect();
    assert_eq!(warned.len(), 1, "{warned:?}");
    assert!(
        warns("errors")
            .iter()
            .all(|line| !line.contains("indexing"))
    );
}

#[test]
fn a_line_split_across_reads_is_forwarded_once() {
    let (fake, manager) = started("split");
    fake.stderr(b"2026-11-23 ERR");
    fake.stderr(b"OR split ");
    fake.stderr(b"across reads\n");
    wait_until("the ring", || {
        manager
            .log("split", Path::new(ROOT))
            .contains("across reads\n")
    });
    wait_until("the warn", || !warns("split").is_empty());
    let warned = warns("split");
    assert_eq!(warned.len(), 1, "{warned:?}");
    assert!(warned[0].contains("ERROR split across reads"), "{warned:?}");
}

#[test]
fn forwarding_stops_at_the_cap_with_one_notice() {
    let (fake, manager) = started("capped");
    for n in 0..80 {
        fake.stderr(format!("ERROR number {n}\n").as_bytes());
    }
    wait_until("the ring", || {
        manager
            .log("capped", Path::new(ROOT))
            .contains("number 79\n")
    });
    wait_until("the notice", || {
        warns("capped")
            .iter()
            .any(|line| line.contains("Show Language Server Log"))
    });
    let warned = warns("capped");
    let notices = warned
        .iter()
        .filter(|line| line.contains("Show Language Server Log"))
        .count();
    assert_eq!(notices, 1, "{warned:?}");
    assert_eq!(warned.len(), 51, "{warned:?}");
    assert!(warned.iter().all(|line| !line.contains("number 79")));
}

#[test]
fn error_log_messages_are_warned() {
    let (fake, manager) = started("logmsg");
    fake.notify::<LogMessage>(LogMessageParams {
        typ: MessageType::ERROR,
        message: "proc macro crashed".into(),
    });
    fake.notify::<LogMessage>(LogMessageParams {
        typ: MessageType::INFO,
        message: "just chatting".into(),
    });
    wait_until("both in the ring", || {
        let log = manager.log("logmsg", Path::new(ROOT));
        log.contains("proc macro crashed") && log.contains("just chatting")
    });
    wait_until("the warn", || !warns("logmsg").is_empty());
    let warned = warns("logmsg");
    assert_eq!(warned.len(), 1, "{warned:?}");
    assert!(warned[0].contains("proc macro crashed"));
}

#[test]
fn an_unterminated_error_is_forwarded_when_the_output_ends() {
    let (fake, manager) = started("unterminated");
    fake.stderr(b"ERROR fatal, no newline");
    wait_until("the ring", || {
        manager
            .log("unterminated", Path::new(ROOT))
            .contains("no newline")
    });
    assert!(warns("unterminated").is_empty());
    fake.crash();
    wait_until("the warn", || {
        warns("unterminated")
            .iter()
            .any(|line| line.contains("ERROR fatal, no newline"))
    });
    let warned = warns("unterminated");
    let hits = warned.iter().filter(|l| l.contains("no newline")).count();
    assert_eq!(hits, 1, "{warned:?}");
}
