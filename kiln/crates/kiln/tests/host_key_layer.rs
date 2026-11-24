// SPDX-License-Identifier: Apache-2.0

//! Keymap layers a host sets per owner, through `kiln::init`, headless
//! (T2106 C2).
//!
//! A host layer sits at the LOWEST precedence of any layer, and that is not
//! the same as "the earliest layer". Ordinary layers are ranked by context
//! depth first and layer order only as a tie-break, so a deeper binding in
//! an earlier layer beats a shallower one in a later layer. A host layer
//! loses to any other layer's binding for the same keys - or for a chord
//! either of them starts - WHATEVER the contexts' depths. That is what
//! `the_apps_binding_wins_even_in_a_shallower_context` pins.

use std::{fs, path::Path};

use gpui::TestAppContext;
use kiln::{
    Config,
    command::{CommandId, ContextFrame, ContextPredicate, ContextStack},
    core::{Diagnostic, KeySequence, Keystroke},
    host_keys::{self, HostBinding},
    platform::{
        config::{BatchSender, batch_channel},
        keys,
    },
};

mod notes {
    kiln::commands!(
        notes,
        [
            /// New Note
            NewNote,
            /// Tidy
            Tidy,
            /// Find
            Find,
        ]
    );
}

/// The app's own keymap: `secondary-n` everywhere, `secondary-f` in an
/// editor.
const KEYMAP: &str = r#"
[[bindings]]
keys = "secondary-n"
command = "notes.new_note"

[[bindings]]
keys = "secondary-f"
context = "editor"
command = "notes.find"

[[bindings]]
keys = "secondary-k secondary-0"
command = "notes.tidy"
"#;

const MENUS: &str = "";

/// A command an owner adds, which the app does not have.
const OWNED: &str = "acme.tool.run";

struct Setup {
    #[allow(dead_code, reason = "kept so the watcher channel stays open")]
    sender: BatchSender,
}

impl Setup {
    fn new(state: &Path, user_keymap_text: Option<&str>) -> (Self, Config) {
        let user_keymap = state.join("config/keymap.toml");
        if let Some(text) = user_keymap_text {
            fs::create_dir_all(user_keymap.parent().unwrap()).unwrap();
            fs::write(&user_keymap, text).unwrap();
        }
        let (sender, batches) = batch_channel();
        let config = Config::new("com.example.Notes", "Notes")
            .commands(notes::register)
            .keymap("keymap.toml", KEYMAP)
            .menus("menus.toml", MENUS)
            .user_keymap(&user_keymap)
            .watch_with(batches);
        (Self { sender }, config)
    }
}

fn init(
    state: &Path,
    user_keymap: Option<&str>,
    cx: &mut TestAppContext,
) -> (Setup, Vec<Diagnostic>) {
    let (setup, config) = Setup::new(state, user_keymap);
    let diagnostics = cx.update(|cx| kiln::init(cx, config));
    cx.run_until_parked();
    (setup, diagnostics)
}

/// The command `keys` alone runs in the frames `context`.
fn runs(keys: &str, context: &[&str], cx: &mut TestAppContext) -> Option<String> {
    let keystroke = Keystroke::parse(keys).unwrap();
    let stack: ContextStack = context
        .iter()
        .map(|&name| ContextFrame::new(name))
        .collect();
    cx.update(|cx| keys::lookup(&keystroke, &stack, cx))
        .map(|invocation| invocation.command.to_string())
}

fn binding(keys: &str, command: &str, context: Option<&str>) -> HostBinding {
    HostBinding {
        keys: KeySequence::parse(keys).unwrap(),
        command: CommandId::parse(command).unwrap(),
        context: context.map(|text| ContextPredicate::parse(text).unwrap()),
    }
}

/// Sets `owner`'s bindings, and the commands it adds, and lets the
/// resolution that follows settle.
fn set(owner: &str, bindings: Vec<HostBinding>, commands: &[&str], cx: &mut TestAppContext) {
    let commands: Vec<CommandId> = commands
        .iter()
        .map(|id| CommandId::parse(id).unwrap())
        .collect();
    cx.update(|cx| host_keys::set(cx, owner, bindings, commands));
    cx.run_until_parked();
}

#[gpui::test]
fn a_host_binding_runs_its_command(cx: &mut TestAppContext) {
    let state = tempfile::tempdir().unwrap();
    let (_setup, diagnostics) = init(state.path(), None, cx);
    assert_eq!(diagnostics, []);

    // Nothing binds these keys to begin with.
    assert_eq!(runs("ctrl-alt-9", &[], cx), None);

    set(
        "acme.tool",
        vec![binding("ctrl-alt-9", OWNED, None)],
        &[OWNED],
        cx,
    );
    assert_eq!(runs("ctrl-alt-9", &[], cx).as_deref(), Some(OWNED));

    // And in a context, since the binding names none.
    assert_eq!(
        runs("ctrl-alt-9", &["workbench", "editor"], cx).as_deref(),
        Some(OWNED)
    );
}

#[gpui::test]
fn the_apps_binding_wins_even_in_a_shallower_context(cx: &mut TestAppContext) {
    let state = tempfile::tempdir().unwrap();
    let (_setup, diagnostics) = init(state.path(), None, cx);
    assert_eq!(diagnostics, []);

    // The app binds `secondary-n` with NO context: depth 0, the shallowest
    // there is. The host binds the same keys in `editor`, which is deeper.
    // Among ordinary layers the deeper one would win; a host layer must not.
    set(
        "acme.tool",
        vec![binding("secondary-n", OWNED, Some("editor"))],
        &[OWNED],
        cx,
    );
    assert_eq!(
        runs("secondary-n", &["workbench", "editor"], cx).as_deref(),
        Some("notes.new_note"),
        "the app's binding wins although its context is shallower"
    );

    // The same the other way round: the app's `secondary-f` is in `editor`,
    // and a host binding with no context does not take it there either.
    set(
        "acme.tool",
        vec![binding("secondary-f", OWNED, None)],
        &[OWNED],
        cx,
    );
    assert_eq!(
        runs("secondary-f", &["workbench", "editor"], cx).as_deref(),
        Some("notes.find")
    );

    // Outside `editor` the app's `secondary-f` does not apply, and the
    // host's binding is free to run: it loses only where the other applies.
    assert_eq!(
        runs("secondary-f", &["workbench"], cx).as_deref(),
        Some(OWNED),
        "a host binding runs where no other layer's applies"
    );
}

#[gpui::test]
fn the_users_binding_and_unbind_win(cx: &mut TestAppContext) {
    let state = tempfile::tempdir().unwrap();
    // The user binds the host's keys to one of the app's commands, and
    // unbinds another pair.
    let user = r#"
[[bindings]]
keys = "ctrl-alt-9"
command = "notes.tidy"

[[bindings]]
keys = "ctrl-alt-8"
command = ""
"#;
    let (_setup, diagnostics) = init(state.path(), Some(user), cx);
    assert_eq!(diagnostics, []);

    set(
        "acme.tool",
        vec![
            binding("ctrl-alt-9", OWNED, None),
            binding("ctrl-alt-8", OWNED, None),
        ],
        &[OWNED],
        cx,
    );

    // The user's binding wins over the host's.
    assert_eq!(
        runs("ctrl-alt-9", &[], cx).as_deref(),
        Some("notes.tidy"),
        "the user's binding wins"
    );
    // And the user's unbind takes the host's away entirely.
    assert_eq!(
        runs("ctrl-alt-8", &[], cx),
        None,
        "the user's unbind removes the host's binding"
    );
}

#[gpui::test]
fn a_user_binding_to_a_host_command_binds_once_it_is_added(cx: &mut TestAppContext) {
    let state = tempfile::tempdir().unwrap();
    // The user binds keys to a command no one has yet.
    let user = format!("[[bindings]]\nkeys = \"ctrl-alt-7\"\ncommand = \"{OWNED}\"\n");
    let (_setup, _diagnostics) = init(state.path(), Some(&user), cx);

    // Unknown command: the entry binds nothing.
    assert_eq!(
        runs("ctrl-alt-7", &[], cx),
        None,
        "a binding to a command nothing has runs nothing"
    );

    // The owner adds the command - and declares NO binding of its own, so
    // the only thing that can bind those keys is the user's own entry.
    set("acme.tool", Vec::new(), &[OWNED], cx);
    assert_eq!(
        runs("ctrl-alt-7", &[], cx).as_deref(),
        Some(OWNED),
        "the user's binding takes effect with no edit to their file"
    );
}

#[gpui::test]
fn removing_the_owner_takes_its_bindings_away(cx: &mut TestAppContext) {
    let state = tempfile::tempdir().unwrap();
    let (_setup, diagnostics) = init(state.path(), None, cx);
    assert_eq!(diagnostics, []);

    set(
        "acme.tool",
        vec![binding("ctrl-alt-9", OWNED, None)],
        &[OWNED],
        cx,
    );
    assert_eq!(runs("ctrl-alt-9", &[], cx).as_deref(), Some(OWNED));

    cx.update(|cx| host_keys::remove(cx, "acme.tool"));
    cx.run_until_parked();
    assert_eq!(
        runs("ctrl-alt-9", &[], cx),
        None,
        "the owner's bindings go with it"
    );

    // A second owner's bindings are untouched by the first's removal.
    set(
        "acme.other",
        vec![binding("ctrl-alt-6", "acme.other.go", None)],
        &["acme.other.go"],
        cx,
    );
    set(
        "acme.tool",
        vec![binding("ctrl-alt-9", OWNED, None)],
        &[OWNED],
        cx,
    );
    cx.update(|cx| host_keys::remove(cx, "acme.tool"));
    cx.run_until_parked();
    assert_eq!(runs("ctrl-alt-9", &[], cx), None);
    assert_eq!(
        runs("ctrl-alt-6", &[], cx).as_deref(),
        Some("acme.other.go"),
        "removing one owner leaves another's alone"
    );
}

#[gpui::test]
fn a_host_binding_loses_to_a_chord_the_app_starts(cx: &mut TestAppContext) {
    let state = tempfile::tempdir().unwrap();
    let (_setup, diagnostics) = init(state.path(), None, cx);
    assert_eq!(diagnostics, []);

    // The app's `secondary-k secondary-0` is a chord. `secondary-k` alone
    // runs nothing: it only starts one.
    assert_eq!(runs("secondary-k", &[], cx), None);

    // A host asks for `secondary-k` by itself. Taking it would make the
    // app's chord unreachable, so the host does not get it - even though
    // no binding of the app's runs on `secondary-k` alone for it to lose
    // to. `Keymap::lookup` ignores chords when it ranks, so this is the
    // `starts_with` half of the rule and nothing else pins it.
    set(
        "acme.tool",
        vec![binding("secondary-k", OWNED, None)],
        &[OWNED],
        cx,
    );
    assert_eq!(
        runs("secondary-k", &[], cx),
        None,
        "a host binding does not take the first keystroke of the app's chord"
    );

    // The chord's own keys are untouched by the attempt.
    assert_eq!(
        runs("secondary-n", &[], cx).as_deref(),
        Some("notes.new_note")
    );
}
