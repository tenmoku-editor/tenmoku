//! Draft from diff (T1592): the commit box's button and
//! `git.draft_commit_message` ask the chat's provider, here a scripted one,
//! for a message for the staged changes, on a repository built with the
//! `git` CLI in a temporary directory.

use std::{fs, path::Path, process::Command, rc::Rc, sync::Arc};

use fe_chat::{ChatEvent, ChatProvider, ReplyScript, ScriptedProvider, StopReason};
use fe_command::{ContextFrame, ContextStack, Invocation};
use fe_gpui::{EditorView, commands::RunCommand};
use gpui::{Entity, Focusable as _, TestAppContext, VisualTestContext};
use kiln::core::secrets::MemoryStore;
use kiln_l10n::tr;

use crate::{
    GitPanel, Launch, Workbench,
    chat_panel::{ChatPanel, KeySource},
    git_panel::{DrawnDraft, set_box_text},
    tests::open,
};

const DRAFT: &str = "Say staged in a.rs\n\nIt said head.";

/// Runs `git` in `dir`, with the developer's own config shut out.
fn git(dir: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(["-c", "init.defaultBranch=main"])
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// A repository whose `a.rs` was committed, then changed and, if
/// `staged`, staged, with a staged `.env` too.
fn fixture(staged: bool) -> tempfile::TempDir {
    let temp = dunce::canonicalize(std::env::temp_dir()).unwrap();
    let dir = tempfile::tempdir_in(temp).unwrap();
    let root = dir.path();
    git(root, &["init", "-q"]);
    for (key, value) in [("user.name", "t"), ("user.email", "t@t")] {
        git(root, &["config", key, value]);
    }
    fs::write(root.join("a.rs"), "fn a() {}\nhead\n").unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-q", "-m", "init"]);
    fs::write(root.join("a.rs"), "fn a() {}\nstaged\n").unwrap();
    fs::write(root.join(".env"), "TOKEN=hush\n").unwrap();
    if staged {
        git(root, &["add", "a.rs", ".env"]);
    }
    dir
}

/// A reply of `text`.
fn reply(text: &str) -> Vec<ChatEvent> {
    ReplyScript::new("claude-opus-5-5", "msg")
        .text(text)
        .stop(StopReason::EndTurn)
}

struct Fixture {
    workbench: Entity<Workbench>,
    panel: Entity<GitPanel>,
    commit_box: Entity<EditorView>,
    provider: Arc<ScriptedProvider>,
    _dir: tempfile::TempDir,
}

/// A workbench on `fixture(staged)`, trusted if `trusted`, whose chat has
/// a key if `key` and answers with `replies`, showing the Changes panel
/// and its box.
fn setup(
    cx: &mut TestAppContext,
    staged: bool,
    trusted: bool,
    key: bool,
    replies: Vec<Vec<ChatEvent>>,
) -> (Fixture, &mut VisualTestContext) {
    setup_in(cx, fixture(staged), trusted, key, replies)
}

/// [`setup`] on the repository in `dir`.
fn setup_in(
    cx: &mut TestAppContext,
    dir: tempfile::TempDir,
    trusted: bool,
    key: bool,
    replies: Vec<Vec<ChatEvent>>,
) -> (Fixture, &mut VisualTestContext) {
    let launch = Launch {
        root: Some(dir.path().to_owned()),
        file: None,
    };
    let (workbench, cx, _) = open(cx, launch);
    cx.update(|window, _| window.activate_window());
    cx.update(|window, cx| window.focus(&workbench.focus_handle(cx), cx));
    let provider = Arc::new(ScriptedProvider::new(replies));
    let shared = provider.clone();
    cx.update(|_, cx| {
        ChatPanel::set_provider_factory(
            cx,
            Rc::new(move |_, _| shared.clone() as Arc<dyn ChatProvider>),
        );
        cx.set_global(KeySource {
            env: key.then(|| "key".to_owned()),
            store: Arc::new(MemoryStore::new()),
        });
    });
    draw(cx);
    workbench.update(cx, |workbench, cx| workbench.set_root_trust(trusted, cx));
    dispatch("git.show_changes", cx);
    draw(cx);
    let panel = workbench.read_with(cx, |workbench, cx| {
        workbench.dock().read(cx).changes().clone()
    });
    let commit_box = panel.read_with(cx, |panel, _| panel.commit_box().cloned());
    let commit_box = commit_box.expect("the panel's drawn with its box");
    let fixture = Fixture {
        workbench,
        panel,
        commit_box,
        provider,
        _dir: dir,
    };
    (fixture, cx)
}

/// Lets background work land, then draws a frame.
fn draw(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.run_until_parked();
}

/// Runs workbench command `id` as an unbound key would.
fn dispatch(id: &str, cx: &mut VisualTestContext) {
    start(id, cx);
    cx.run_until_parked();
}

/// Runs workbench command `id`, leaving what it started in the
/// background to run.
fn start(id: &str, cx: &mut VisualTestContext) {
    let run = RunCommand(
        Invocation::of(id)
            .unwrap()
            .context(ContextStack::new().with(ContextFrame::new("workbench"))),
    );
    cx.update(|window, cx| window.dispatch_action(Box::new(run), cx));
}

fn box_text(commit_box: &Entity<EditorView>, cx: &mut VisualTestContext) -> String {
    commit_box.read_with(cx, |editor, _| editor.model().text().to_string())
}

/// Draft from diff's button as the next frame draws it.
fn drawn(panel: &Entity<GitPanel>, cx: &mut VisualTestContext) -> DrawnDraft {
    panel.read_with(cx, |panel, _| panel.take_drawn_draft());
    draw(cx);
    let drawn = panel.read_with(cx, |panel, _| panel.take_drawn_draft());
    drawn.expect("the button's drawn")
}

/// What the window's shown toasts say, top first.
fn toasts(workbench: &Entity<Workbench>, cx: &mut VisualTestContext) -> Vec<String> {
    let layer = workbench.read_with(cx, |workbench, _| workbench.toasts().clone());
    layer.read_with(cx, |layer, _| {
        (layer.queue().shown().iter())
            .map(|entry| entry.toast.message.clone())
            .collect()
    })
}

/// The one request's user text.
fn sent(provider: &ScriptedProvider) -> String {
    let requests = provider.requests();
    assert_eq!(requests.len(), 1);
    requests[0].messages[0].content[0]["text"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[gpui::test]
fn fills_an_empty_box_as_one_undoable_edit(cx: &mut TestAppContext) {
    let (s, cx) = setup(
        cx,
        true,
        true,
        true,
        vec![reply(&format!("```\n{DRAFT}\n```"))],
    );
    let enabled = DrawnDraft {
        label: tr!("workbench-changes-draft"),
        enabled: true,
        reason: None,
    };
    assert_eq!(drawn(&s.panel, cx), enabled);
    dispatch("git.draft_commit_message", cx);
    draw(cx);
    assert_eq!(box_text(&s.commit_box, cx), DRAFT);
    assert_eq!(drawn(&s.panel, cx), enabled);
    // The staged diff went, HEAD to index; the secret only counted.
    let text = sent(&s.provider);
    assert!(text.contains("<staged-files>\nM a.rs\n"), "{text}");
    assert!(
        text.contains("--- a/a.rs\n+++ b/a.rs\n@@ -1,2 +1,2 @@\n fn a() {}\n-head\n+staged\n"),
        "{text}"
    );
    assert!(!text.contains(".env") && !text.contains("hush"), "{text}");
    assert_eq!(toasts(&s.workbench, cx), Vec::<String>::new());
    // One undo step empties the box again.
    s.commit_box.update(cx, |editor, cx| {
        editor.model_mut().undo().unwrap();
        cx.notify();
    });
    assert_eq!(box_text(&s.commit_box, cx), "");
}

#[gpui::test]
fn asks_before_replacing_a_typed_message(cx: &mut TestAppContext) {
    let replies = vec![reply(DRAFT), reply(DRAFT)];
    let (s, cx) = setup(cx, true, true, true, replies);
    cx.update(|_, cx| set_box_text(&s.commit_box, "typed", cx));
    draw(cx);
    dispatch("git.draft_commit_message", cx);
    draw(cx);
    assert!(cx.has_pending_prompt());
    cx.simulate_prompt_answer(&tr!("workbench-changes-cancel"));
    draw(cx);
    assert_eq!(box_text(&s.commit_box, cx), "typed");

    dispatch("git.draft_commit_message", cx);
    draw(cx);
    assert!(cx.has_pending_prompt());
    cx.simulate_prompt_answer(&tr!("workbench-changes-draft-replace-button"));
    draw(cx);
    assert_eq!(box_text(&s.commit_box, cx), DRAFT);
    assert_eq!(s.provider.requests().len(), 2);
    s.commit_box.update(cx, |editor, cx| {
        editor.model_mut().undo().unwrap();
        cx.notify();
    });
    assert_eq!(box_text(&s.commit_box, cx), "typed");
}

#[gpui::test]
fn disabled_with_a_reason_when_nothing_is_staged_or_chat_is_unavailable(cx: &mut TestAppContext) {
    let disabled = |reason: &str| DrawnDraft {
        label: tr!("workbench-changes-draft"),
        enabled: false,
        reason: Some(tr!(reason)),
    };
    let check = |s: &Fixture, reason: &str, cx: &mut VisualTestContext| {
        assert_eq!(drawn(&s.panel, cx), disabled(reason));
        dispatch("git.draft_commit_message", cx);
        draw(cx);
        assert_eq!(s.provider.requests().len(), 0);
        assert_eq!(box_text(&s.commit_box, cx), "");
        assert_eq!(toasts(&s.workbench, cx), [tr!(reason).to_string()]);
    };

    let (s, cx) = setup(cx, false, true, true, vec![reply(DRAFT)]);
    check(&s, "workbench-changes-draft-nothing-staged", cx);
    drop(s);

    let mut app = cx.cx.clone();
    let (s, cx) = setup(&mut app, true, true, false, vec![reply(DRAFT)]);
    check(&s, "workbench-changes-draft-no-key", cx);
    drop(s);

    let mut app = cx.cx.clone();
    let (s, cx) = setup(&mut app, true, false, true, vec![reply(DRAFT)]);
    check(&s, "workbench-changes-draft-untrusted", cx);
    // Trusting the folder enables it.
    s.workbench
        .update(cx, |workbench, cx| workbench.set_root_trust(true, cx));
    let enabled = drawn(&s.panel, cx);
    assert!(enabled.enabled && enabled.reason.is_none(), "{enabled:?}");
}

#[gpui::test]
fn untrusting_rerooting_or_committing_drops_the_draft(cx: &mut TestAppContext) {
    // Untrusted while the staged changes are read: nothing is sent.
    let (s, cx) = setup(cx, true, true, true, vec![reply(DRAFT)]);
    start("git.draft_commit_message", cx);
    assert!(s.panel.read_with(cx, |panel, _| panel.drafting()));
    s.workbench
        .update(cx, |workbench, cx| workbench.set_root_trust(false, cx));
    draw(cx);
    assert_eq!(s.provider.requests().len(), 0);
    assert!(!s.panel.read_with(cx, |panel, _| panel.drafting()));
    assert_eq!(box_text(&s.commit_box, cx), "");

    // Another folder opened meanwhile: nothing is sent.
    let mut app = cx.cx.clone();
    let (s, cx) = setup(&mut app, true, true, true, vec![reply(DRAFT)]);
    let other = fixture(true);
    start("git.draft_commit_message", cx);
    let root = other.path().to_owned();
    s.workbench.update_in(cx, |workbench, window, cx| {
        workbench.set_root(root, window, cx)
    });
    draw(cx);
    assert_eq!(s.provider.requests().len(), 0);
    assert_eq!(box_text(&s.commit_box, cx), "");

    // A commit made while the replacement is asked about: the answer
    // doesn't bring the draft back.
    let mut app = cx.cx.clone();
    let (s, cx) = setup(&mut app, true, true, true, vec![reply(DRAFT)]);
    cx.update(|_, cx| set_box_text(&s.commit_box, "typed", cx));
    draw(cx);
    dispatch("git.draft_commit_message", cx);
    draw(cx);
    assert!(cx.has_pending_prompt());
    s.workbench.update_in(cx, |workbench, window, cx| {
        workbench.commit_from_box(false, window, cx)
    });
    assert!(!s.panel.read_with(cx, |panel, _| panel.drafting()));
    draw(cx);
    assert_eq!(box_text(&s.commit_box, cx), "");
    cx.simulate_prompt_answer(&tr!("workbench-changes-draft-replace-button"));
    draw(cx);
    assert_eq!(box_text(&s.commit_box, cx), "");
    assert_eq!(s.provider.requests().len(), 1);
}

/// The request leaves out binary files by their bytes, and never brings
/// in a file under a path that changed between file and directory.
#[cfg(unix)]
#[gpui::test]
fn binary_blobs_and_files_under_a_changed_kind_stay_out(cx: &mut TestAppContext) {
    use std::os::unix::fs::PermissionsExt as _;

    let temp = dunce::canonicalize(std::env::temp_dir()).unwrap();
    let dir = tempfile::tempdir_in(temp).unwrap();
    let root = dir.path();
    git(root, &["init", "-q"]);
    for (key, value) in [("user.name", "t"), ("user.email", "t@t")] {
        git(root, &["config", key, value]);
    }
    fs::write(root.join(".gitattributes"), "data.txt diff\n").unwrap();
    fs::write(root.join("config"), "plain\n").unwrap();
    fs::create_dir(root.join("old")).unwrap();
    fs::write(root.join("old/.env"), "OLD=hush\n").unwrap();
    fs::write(root.join("logo.bin"), b"\x00\x01").unwrap();
    fs::write(root.join("data.txt"), b"one\n\x00two\n").unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-q", "-m", "init"]);
    // A file become a directory, and a directory become a file, each
    // with a secret under the directory.
    git(root, &["rm", "-q", "--cached", "config", "-r", "old"]);
    fs::remove_file(root.join("config")).unwrap();
    fs::remove_dir_all(root.join("old")).unwrap();
    fs::create_dir(root.join("config")).unwrap();
    fs::write(root.join("config/.env"), "NEW=hush\n").unwrap();
    fs::write(root.join("old"), "now a file\n").unwrap();
    // A binary file's mode alone, and one git is told to diff as text.
    let logo = root.join("logo.bin");
    fs::set_permissions(&logo, fs::Permissions::from_mode(0o755)).unwrap();
    fs::write(root.join("data.txt"), b"one\n\x00three\n").unwrap();
    git(root, &["add", "."]);

    let (s, cx) = setup_in(cx, dir, true, true, vec![reply(DRAFT)]);
    dispatch("git.draft_commit_message", cx);
    draw(cx);
    let text = sent(&s.provider);
    let listed = "<staged-files>\nD config\nA old\n\
        [4 more files not shown: binary, too large or private]\n</staged-files>\n";
    assert!(text.starts_with(listed), "{text}");
    for left_out in ["hush", ".env", "logo.bin", "data.txt", "three"] {
        assert!(!text.contains(left_out), "{left_out} in {text}");
    }
    assert_eq!(text.matches("diff --git").count(), 2, "{text}");
}
