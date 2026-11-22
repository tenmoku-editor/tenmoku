//! Draft from diff's headless core: the request built from the staged
//! diff, and the reply read as a commit message, over the scripted
//! provider.

use fe_chat::{
    ChatRequest, ChatSettings, CommitDraft, DIFF_BUDGET, DraftError, Effort, MIN_SECTION,
    ReplyScript, SUBJECT_LIMIT, ScriptedProvider, StagedChange, StagedDiff, StagedFile, StopReason,
    commit_message_request, parse_commit_message, run_commit_message,
};
use futures::executor::block_on;

/// `path`'s patch as git writes it, adding `rows` lines that name `tag`:
/// about `rows * 20` bytes after its headers.
fn patch(path: &str, tag: &str, rows: usize) -> String {
    let mut patch =
        format!("diff --git a/{path} b/{path}\n--- a/{path}\n+++ b/{path}\n@@ -0,0 +1,{rows} @@\n");
    for row in 0..rows {
        patch.push_str(&format!("+{tag} line {row:05}\n"));
    }
    patch
}

/// `path`'s patch, exactly `len` bytes long.
fn patch_of_len(path: &str, len: usize) -> String {
    let mut patch = format!("diff --git a/{path} b/{path}\n@@ -0,0 +1 @@\n+");
    patch.push_str(&"x".repeat(len - patch.len() - 1));
    patch.push('\n');
    assert_eq!(patch.len(), len);
    patch
}

fn file(path: &str, change: StagedChange, diff: StagedDiff) -> StagedFile {
    StagedFile {
        path: path.to_owned(),
        change,
        diff,
    }
}

/// The request's one user text.
fn user_text(request: &ChatRequest) -> String {
    assert_eq!(request.messages.len(), 1);
    let content = &request.messages[0].content;
    assert_eq!(content.len(), 1);
    content[0]["text"].as_str().unwrap().to_owned()
}

/// What's between `<diff>` and `</diff>`.
fn diff_of(text: &str) -> &str {
    let start = text.find("<diff>\n").unwrap() + "<diff>\n".len();
    let end = text.find("</diff>").unwrap();
    &text[start..end]
}

/// The reply's events: `text`, then the end of the turn.
fn reply(text: &str) -> Vec<fe_chat::ChatEvent> {
    ReplyScript::new("claude-opus-5-5", "msg")
        .text(text)
        .stop(StopReason::EndTurn)
}

#[test]
fn the_request_lists_files_and_cuts_the_diff_to_24_kb_largest_first() {
    let (small, medium, large) = (
        patch("src/a.rs", "small", 40),
        patch("src/b.rs", "medium", 300),
        patch("src/c.rs", "large", 2000),
    );
    let files = [
        file(
            "src/a.rs",
            StagedChange::Modified,
            StagedDiff::Text(small.clone()),
        ),
        file(
            "src/b.rs",
            StagedChange::Added,
            StagedDiff::Text(medium.clone()),
        ),
        file(
            "src/c.rs",
            StagedChange::Deleted,
            StagedDiff::Text(large.clone()),
        ),
    ];
    let settings = ChatSettings::default();
    let request = commit_message_request(&files, &settings);
    assert_eq!(request.model, settings.model);
    assert!(request.tools.is_empty());
    assert_eq!(request.effort, Effort::Low);
    let system = &request.system[0].text;
    assert!(system.contains("at most 72 characters"), "{system}");
    assert!(system.contains("a blank line"), "{system}");

    let text = user_text(&request);
    assert!(
        text.starts_with("<staged-files>\nM src/a.rs\nA src/b.rs\nD src/c.rs\n</staged-files>\n")
    );
    let diff = diff_of(&text);
    assert!(diff.len() <= DIFF_BUDGET, "{} bytes", diff.len());
    // The two smaller patches whole; the largest cut at a line's end,
    // saying so.
    assert!(diff.starts_with(&format!("{small}{medium}")));
    let large_at = diff.find("diff --git a/src/c.rs b/src/c.rs\n").unwrap();
    let kept = &diff[large_at..];
    assert!(kept.len() > 10_000, "cut to {} bytes", kept.len());
    let note = kept.lines().last().unwrap();
    assert!(
        note.starts_with('[') && note.ends_with(" more lines cut]"),
        "{note}"
    );
    let cut_lines: usize = note[1..note.find(' ').unwrap()].parse().unwrap();
    assert_eq!(kept.lines().count() - 1 + cut_lines, large.lines().count());
    assert!(!text.contains("more files"));

    // Too many to cut them all to a useful size: the largest are left out
    // whole, and counted.
    let files: Vec<StagedFile> = (0..60)
        .map(|ix| {
            let path = format!("src/f{ix:02}.rs");
            file(
                &path,
                StagedChange::Modified,
                StagedDiff::Text(patch(&path, &path, 30 + ix)),
            )
        })
        .collect();
    let text = user_text(&commit_message_request(&files, &settings));
    let diff = diff_of(&text);
    assert!(diff.len() <= DIFF_BUDGET, "{} bytes", diff.len());
    for ix in 0..60 {
        assert!(
            text.contains(&format!("\nM src/f{ix:02}.rs\n")),
            "f{ix:02} is listed"
        );
        let header = format!("diff --git a/src/f{ix:02}.rs");
        assert_eq!(diff.contains(&header), ix < 47, "f{ix:02}'s diff");
    }
    assert!(
        diff.ends_with("[the diffs of 13 more files left out]\n"),
        "{diff}"
    );

    // Patches of exactly the least share: the note saying some are left
    // out still fits.
    let files: Vec<StagedFile> = (0..49)
        .map(|ix| {
            let path = format!("x{ix:02}");
            let diff = StagedDiff::Text(patch_of_len(&path, MIN_SECTION));
            file(&path, StagedChange::Added, diff)
        })
        .collect();
    let text = user_text(&commit_message_request(&files, &settings));
    let diff = diff_of(&text);
    assert!(diff.len() <= DIFF_BUDGET, "{} bytes", diff.len());
    assert!(!diff.contains("lines cut"));
    assert!(
        diff.ends_with("[the diffs of 2 more files left out]\n"),
        "{diff}"
    );
}

#[test]
fn denied_and_binary_paths_are_left_out_of_the_text() {
    let files = [
        file(".env", StagedChange::Modified, StagedDiff::Denied),
        file("assets/logo.png", StagedChange::Added, StagedDiff::Binary),
        file("data/dump.sql", StagedChange::Added, StagedDiff::TooLarge),
        file(
            "src/lib.rs",
            StagedChange::Modified,
            StagedDiff::Text(patch("src/lib.rs", "lib", 3)),
        ),
    ];
    let text = user_text(&commit_message_request(&files, &ChatSettings::default()));
    for path in [".env", "logo.png", "dump.sql"] {
        assert!(!text.contains(path), "{path} in {text}");
    }
    assert!(text.contains("\nM src/lib.rs\n"));
    assert!(text.contains("[3 more files not shown: binary, too large or private]"));
    assert!(text.contains("diff --git a/src/lib.rs b/src/lib.rs\n"));
    assert_eq!(diff_of(&text).matches("diff --git").count(), 1);
}

#[test]
fn the_reply_parses_to_subject_and_body_and_drops_fences() {
    let plain = parse_commit_message("Add the draft button\n\nIt asks the chat.\nOnce.  \n\n");
    assert_eq!(
        plain,
        Some(CommitDraft {
            subject: "Add the draft button".to_owned(),
            body: "It asks the chat.\nOnce.".to_owned(),
        })
    );
    assert_eq!(
        plain.unwrap().message(),
        "Add the draft button\n\nIt asks the chat.\nOnce."
    );
    let fenced =
        "Here it is:\n```text\n  Fix the counter\n\nIt counted bytes.\n```\nHope it helps.";
    let fenced = parse_commit_message(fenced).unwrap();
    assert_eq!(fenced.subject, "Fix the counter");
    assert_eq!(fenced.body, "It counted bytes.");
    let subject_only = parse_commit_message("\n\nBump the version\n").unwrap();
    assert_eq!(subject_only.body, "");
    assert_eq!(subject_only.message(), "Bump the version");
    assert_eq!(parse_commit_message("```\n```"), None);
    assert_eq!(parse_commit_message("  \n"), None);

    // Through the provider: the request is sent, and the reply read whole.
    let files = [file(
        "src/lib.rs",
        StagedChange::Modified,
        StagedDiff::Text(patch("src/lib.rs", "lib", 3)),
    )];
    let provider = ScriptedProvider::new([reply("```\nAdd lib lines\n\nThree of them.\n```")]);
    let settings = ChatSettings::default();
    let draft = block_on(run_commit_message(&provider, &files, &settings)).unwrap();
    assert_eq!(draft.message(), "Add lib lines\n\nThree of them.");
    assert_eq!(
        provider.requests(),
        [commit_message_request(&files, &settings)]
    );
    let empty = ScriptedProvider::new([reply("")]);
    assert_eq!(
        block_on(run_commit_message(&empty, &files, &settings)),
        Err(DraftError::Empty)
    );
}

#[test]
fn a_subject_longer_than_72_is_reported() {
    let at_limit = "a".repeat(SUBJECT_LIMIT);
    let draft = parse_commit_message(&format!("{at_limit}\n\nBody.")).unwrap();
    assert_eq!(draft.long_subject(), None);
    // Characters, not bytes.
    let accented = "é".repeat(SUBJECT_LIMIT);
    assert_eq!(
        parse_commit_message(&accented).unwrap().long_subject(),
        None
    );
    let over = format!("{at_limit}bc");
    let draft = parse_commit_message(&format!("{over}\n\nBody.")).unwrap();
    assert_eq!(draft.long_subject(), Some(SUBJECT_LIMIT + 2));
    assert_eq!(draft.subject, over);
}
