//! `lsp.rename` with an add-on beside the primary
//! (docs/decisions/q5j03ax2af-lsp-add-on-servers.md §6.2, §7): the rename
//! picker carries the owner of the prepared rename, the server whose
//! `prepareRename` accepted, through confirmation. The rename goes to it
//! alone; if it restarted since, nothing is renamed, no other server is
//! asked, and a toast says so.

use std::path::Path;

use fe_command::{ContextFrame, ContextStack, Invocation};
use fe_gpui::{EditorView, commands::RunCommand};
use fe_lsp::fake::{Reply, Scripted};
use gpui::{Entity, TestAppContext};
use lsp_types::{
    OneOf, RenameOptions, ServerCapabilities,
    request::{PrepareRenameRequest, Rename, Request as _},
};
use serde_json::{Value, json};

use super::lsp_add_ons::{ADD_ON, AddOns, STYLE, TEXT, script};
use crate::{lsp_rename::RenamePicker, picker::Picker, workbench::Palette};

/// A server that prepares renames.
fn renames(capabilities: &mut ServerCapabilities) {
    capabilities.rename_provider = Some(OneOf::Right(RenameOptions {
        prepare_provider: Some(true),
        work_done_progress_options: Default::default(),
    }));
}

/// `red` in [`TEXT`], the symbol renamed.
fn red() -> Value {
    json!({
        "start": { "line": 0, "character": 11 },
        "end": { "line": 0, "character": 14 },
    })
}

/// Both servers renaming, only the add-on accepting preparation, and
/// [`STYLE`] open with its caret in `red`.
fn setup(cx: &mut TestAppContext) -> (AddOns, Entity<EditorView>) {
    let mut s = AddOns::new(cx, script(renames), script(renames), false);
    s.primary.set_reply(
        PrepareRenameRequest::METHOD,
        Scripted::now(Reply::Result(Value::Null)),
    );
    s.add_on.set_reply(
        PrepareRenameRequest::METHOD,
        Scripted::now(Reply::Result(red())),
    );
    let uri = s.uri(STYLE);
    let edit = json!({ "changes": { uri.as_str(): [{ "range": red(), "newText": "blue" }] } });
    for fake in [&s.primary, &s.add_on] {
        fake.set_reply(Rename::METHOD, Scripted::now(Reply::Result(edit.clone())));
    }
    let editor = s.open_style();
    s.cx.update(|_, cx| {
        editor.update(cx, |view, cx| {
            view.model_mut().set_cursor(12);
            cx.notify();
        })
    });
    s.draw();
    (s, editor)
}

/// Runs `lsp.rename` as the editor's context would, and waits for the
/// picker.
fn rename(s: &mut AddOns) {
    let context = ContextStack::new()
        .with(ContextFrame::new(crate::KEY_CONTEXT))
        .with(ContextFrame::new(fe_editor::commands::CONTEXT));
    let run = RunCommand(Invocation::of("lsp.rename").unwrap().context(context));
    s.cx.update(|window, cx| window.dispatch_action(Box::new(run), cx));
    s.until("the picker", |s| picker(s).is_some());
}

/// The rename picker, if it shows.
fn picker(s: &mut AddOns) -> Option<Entity<Picker<RenamePicker>>> {
    s.workbench.read_with(&s.cx, |workbench, cx| {
        let layer = workbench.shell().read(cx).modals().read(cx);
        (workbench.palette == Some(Palette::Rename))
            .then(|| layer.modal_view().unwrap().clone().downcast().unwrap())
    })
}

fn text(s: &mut AddOns, editor: &Entity<EditorView>) -> String {
    editor.read_with(&s.cx, |view, _| view.model().text().to_string())
}

fn toasts(s: &AddOns) -> Vec<String> {
    let layer = s.workbench.read_with(&s.cx, |w, _| w.toasts().clone());
    layer.read_with(&s.cx, |layer, _| {
        (layer.queue().shown().iter())
            .map(|entry| entry.toast.message.to_string())
            .collect()
    })
}

#[gpui::test]
fn only_add_on_accepts_prepare_rename_and_primary_gets_no_rename(cx: &mut TestAppContext) {
    let (mut s, editor) = setup(cx);
    rename(&mut s);
    // Both were asked to prepare; the add-on, the first to accept, owns it.
    assert_eq!(
        AddOns::asked(&s.primary, PrepareRenameRequest::METHOD).len(),
        1
    );
    assert_eq!(
        AddOns::asked(&s.add_on, PrepareRenameRequest::METHOD).len(),
        1
    );
    s.cx.simulate_input("blue");
    s.draw();
    s.cx.simulate_keystrokes("enter");
    s.until("the rename", |s| {
        text(s, &editor) == TEXT.replace("red", "blue")
    });
    assert!(picker(&mut s).is_none());
    assert_eq!(AddOns::asked(&s.add_on, Rename::METHOD).len(), 1);
    assert!(AddOns::asked(&s.primary, Rename::METHOD).is_empty());
    assert!(s.cx.pending_prompt().is_none());
}

#[gpui::test]
fn owner_restarted_before_confirm_renames_nothing_and_toasts(cx: &mut TestAppContext) {
    let (mut s, editor) = setup(cx);
    rename(&mut s);
    s.cx.simulate_input("blue");
    s.draw();

    // The add-on restarts between preparation and confirmation, and opens
    // the document again; the primary stays as it was.
    let root = s.root.clone();
    s.workbench.update(&mut s.cx, |workbench, _| {
        let manager = workbench.lsp.manager().expect("a manager");
        let restarted = manager.lock().unwrap().restart(ADD_ON, Path::new(&root));
        assert!(matches!(restarted, Ok(Some(_))), "{restarted:?}");
    });
    let uri = s.uri(STYLE);
    s.until("the add-on's new launch to open the stylesheet", |s| {
        AddOns::sent(&s.add_on, 2, "textDocument/didOpen", &uri) == 1
    });

    s.cx.simulate_keystrokes("enter");
    s.until("the picker to close", |s| picker(s).is_none());
    s.until("the toast", |s| !toasts(s).is_empty());
    assert_eq!(toasts(&s), ["the language server restarted"]);
    assert_eq!(text(&mut s, &editor), TEXT, "nothing is renamed");
    assert!(AddOns::asked(&s.add_on, Rename::METHOD).is_empty());
    assert!(AddOns::asked(&s.primary, Rename::METHOD).is_empty());
    assert!(s.cx.pending_prompt().is_none());
}
