//! The What's New marker across preview versions.

use std::fs;

use fe_config::whats_new::{MARKER_FILE, WhatsNew, decide};
use tempfile::TempDir;

/// A state directory whose marker says `version` was seen.
fn seen(version: &str) -> TempDir {
    let state = TempDir::new().unwrap();
    let text = format!("seen = \"{version}\"\n");
    fs::write(state.path().join(MARKER_FILE), text).unwrap();
    state
}

#[test]
fn a_preview_after_a_stable_or_an_earlier_preview_shows_whats_new() {
    let stable = seen("0.3.0");
    assert_eq!(decide(stable.path(), "0.3.1-preview.1"), WhatsNew::Show);
    let preview = seen("0.3.1-preview.1");
    assert_eq!(decide(preview.path(), "0.3.1-preview.2"), WhatsNew::Show);
    assert_eq!(decide(preview.path(), "0.3.1-preview.1"), WhatsNew::Seen);
}
