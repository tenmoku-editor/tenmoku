// SPDX-License-Identifier: Apache-2.0

//! KeyHints: the key that runs a command in a context stack, as this
//! platform prints it, read from the keymap the app matches against now.

use std::path::Path;

use gpui::{App, TestAppContext};
use kiln_core::{Keymap, KeymapLayer, shortcut_label};
use kiln_platform::keys;
use kiln_shell::KeyHints;

/// `notes.archive`: two single keys, then a chord, all in `notes`; a later
/// binding wins, so the chord comes first, then f7. `notes.rename` only
/// `elsewhere`.
const KEYS: &str = r#"
[[bindings]]
keys = "f3"
command = "notes.archive"
context = "notes"

[[bindings]]
keys = "f7"
command = "notes.archive"
context = "notes"

[[bindings]]
keys = "f5 f6"
command = "notes.archive"
context = "notes"

[[bindings]]
keys = "f4"
command = "notes.rename"
context = "elsewhere"
"#;

fn keymap(source: &str) -> Keymap {
    let (layer, diagnostics) = KeymapLayer::parse(Path::new("keymap.toml"), source, None);
    assert_eq!(diagnostics, []);
    Keymap::from_layers([layer.unwrap()])
}

fn init(cx: &mut App) {
    keys::init(cx, keymap(KEYS));
}

fn label(keys: &str) -> String {
    shortcut_label(&keys.parse().unwrap())
}

#[gpui::test]
fn label_is_the_first_binding_in_the_stack(cx: &mut TestAppContext) {
    cx.update(|cx| {
        init(cx);
        let hints = KeyHints::in_contexts(&["app", "notes"]);
        assert_eq!(hints.label("notes.archive", cx), Some(label("f5 f6")));
    });
}

#[gpui::test]
fn single_skips_chords(cx: &mut TestAppContext) {
    cx.update(|cx| {
        init(cx);
        let hints = KeyHints::in_contexts(&["notes"]);
        assert_eq!(hints.single("notes.archive", cx), Some(label("f7")));
    });
}

#[gpui::test]
fn a_binding_outside_the_stack_is_not_found(cx: &mut TestAppContext) {
    cx.update(|cx| {
        init(cx);
        let hints = KeyHints::in_contexts(&["notes"]);
        assert_eq!(hints.label("notes.rename", cx), None);
        assert_eq!(hints.single("notes.rename", cx), None);
    });
}

#[gpui::test]
fn an_unknown_command_has_no_label(cx: &mut TestAppContext) {
    cx.update(|cx| {
        init(cx);
        let hints = KeyHints::in_contexts(&["notes"]);
        assert_eq!(hints.label("notes.unbound", cx), None);
        // Not a command ID at all.
        assert_eq!(hints.label("not a command", cx), None);
        assert_eq!(hints.single("", cx), None);
    });
}

#[gpui::test]
fn the_label_follows_a_keymap_reload(cx: &mut TestAppContext) {
    cx.update(|cx| {
        init(cx);
        let hints = KeyHints::in_contexts(&["notes"]);
        assert_eq!(hints.single("notes.archive", cx), Some(label("f7")));
        keys::set_keymap(
            cx,
            keymap(
                r#"
[[bindings]]
keys = "f9"
command = "notes.archive"
context = "notes"
"#,
            ),
        );
        assert_eq!(hints.single("notes.archive", cx), Some(label("f9")));
        assert_eq!(hints.label("notes.archive", cx), Some(label("f9")));
    });
}
