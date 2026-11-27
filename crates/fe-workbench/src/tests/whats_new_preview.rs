//! The What's New tab's Markdown on a preview version.

use crate::whats_new::notes_markdown;

#[test]
fn the_tab_shows_a_running_preview_and_not_the_next_stable_draft() {
    let notes = [
        ("0.3.0", "## New\n\n- Stable feature.\n"),
        ("0.3.1-preview.1", "## New\n\n- Preview feature.\n"),
        ("0.3.1", "## New\n\n- Draft feature.\n"),
    ]
    .map(|(version, text)| (version.to_owned(), text.to_owned()));
    let markdown = notes_markdown(&notes, "0.3.1-preview.1");
    // The heading escapes the version's punctuation; compare it unescaped.
    let headings: Vec<String> = (markdown.lines())
        .filter_map(|line| line.strip_prefix("# "))
        .map(|heading| heading.replace('\\', ""))
        .collect();
    assert_eq!(
        headings,
        ["0.3.1-preview.1", "0.3.0"].map(|version| format!("{} {version}", fe_config::APP_NAME))
    );
    assert!(markdown.contains("Preview feature."), "{markdown}");
    assert!(!markdown.contains("Draft feature."), "{markdown}");
}
