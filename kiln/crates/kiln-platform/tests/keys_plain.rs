// SPDX-License-Identifier: Apache-2.0

//! `HOLDS_PLAIN_KEYS`: a focused element that reads plain keys itself gets
//! every keystroke without the secondary modifier, however the bindings
//! around it are set, while the app keeps the secondary modifier's
//! bindings and the chords they start, continuations, timeouts and replays
//! included.

use std::{cell::RefCell, path::Path, rc::Rc};

use gpui::{
    Context, FocusHandle, InteractiveElement as _, IntoElement, KeyContext, ParentElement as _,
    Render, Styled as _, TestAppContext, VisualTestContext, Window, div,
};
use kiln_command::Invocation;
use kiln_core::{Keymap, KeymapLayer, Os};
use kiln_platform::{
    commands::{self, RunCommand},
    keys::{self, CHORD_TIMEOUT, HOLDS_PLAIN_KEYS, chord_pending},
};

/// Plain keys, the secondary modifier's, its chords (one ending in a plain
/// key) and a prefix with a binding of its own, all in `pad`, and plain
/// keys in `app` around it.
const KEYS: &str = r#"
[[bindings]]
keys = "left"
command = "pad.move_left"
context = "pad"

[[bindings]]
keys = "f8"
command = "app.next_problem"
context = "app"

[[bindings]]
keys = "alt-x"
command = "app.extend"
context = "app"

[[bindings]]
keys = "secondary-w"
command = "pad.close"
context = "pad"

[[bindings]]
keys = "ctrl-tab"
command = "app.next_tab"
context = "app"

[[bindings]]
keys = "ctrl-shift-tab"
command = "app.previous_tab"
context = "app"

[[bindings]]
keys = "secondary-k secondary-t"
command = "app.pick_theme"
context = "app"

[[bindings]]
keys = "secondary-k z"
command = "app.zen"
context = "app"

[[bindings]]
keys = "secondary-g"
command = "app.go"
context = "app"

[[bindings]]
keys = "secondary-g j"
command = "app.go_down"
context = "app"
"#;

/// `keys` with this platform's `secondary`, in GPUI's spelling.
fn sec(keys: &str) -> String {
    let secondary = match Os::current() {
        Os::MacOs => "cmd",
        _ => "ctrl",
    };
    keys.replace("secondary", secondary)
}

#[derive(Default)]
struct Seen {
    ran: Vec<Invocation>,
    keys: Vec<String>,
}

type Log = Rc<RefCell<Seen>>;

/// A focusable element in `context`, inside a frame in `app`, that logs the
/// commands it runs and the keystrokes its key listener gets.
struct Pad {
    focus: FocusHandle,
    context: &'static str,
    log: Log,
}

impl Render for Pad {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .key_context(KeyContext::parse("app").unwrap())
            .on_action(cx.listener(|this, run: &RunCommand, _, _| {
                this.log.borrow_mut().ran.push(run.0.clone());
            }))
            .child(
                div()
                    .track_focus(&self.focus)
                    .key_context(KeyContext::parse(self.context).unwrap())
                    .size_full()
                    .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _, _| {
                        this.log.borrow_mut().keys.push(event.keystroke.unparse());
                    })),
            )
    }
}

fn ran(log: &Log) -> Vec<String> {
    std::mem::take(&mut log.borrow_mut().ran)
        .iter()
        .map(|invocation| invocation.command.as_str().to_owned())
        .collect()
}

fn keys_seen(log: &Log) -> Vec<String> {
    std::mem::take(&mut log.borrow_mut().keys)
}

/// `keystrokes`, each as the pad's listener spells it.
fn spelled(keystrokes: &str) -> Vec<String> {
    (keystrokes.split(' '))
        .map(|keystroke| gpui::Keystroke::parse(keystroke).unwrap().unparse())
        .collect()
}

/// A focused pad in `context`, matching keys against [`KEYS`].
fn pad<'a>(cx: &'a mut TestAppContext, context: &'static str) -> (Log, &'a mut VisualTestContext) {
    let (layer, diagnostics) = KeymapLayer::parse(Path::new("keymap.toml"), KEYS, None);
    assert_eq!(diagnostics, []);
    let keymap = Keymap::from_layers([layer.unwrap()]);
    cx.update(|cx| {
        commands::init(cx, "Pad");
        keys::init(cx, keymap);
    });
    let (view, cx) = cx.add_window_view(move |_, cx| Pad {
        focus: cx.focus_handle(),
        context,
        log: Log::default(),
    });
    let (focus, log) = view.update(cx, |pad, _| (pad.focus.clone(), pad.log.clone()));
    cx.update(|window, cx| window.focus(&focus, cx));
    cx.run_until_parked();
    (log, cx)
}

fn time_out(cx: &mut TestAppContext) {
    cx.executor().advance_clock(CHORD_TIMEOUT * 2);
    cx.run_until_parked();
}

#[gpui::test]
fn plain_keys_reach_the_listener_whatever_is_bound(cx: &mut TestAppContext) {
    let context = Box::leak(format!("pad {HOLDS_PLAIN_KEYS}").into_boxed_str());
    let (log, cx) = pad(cx, context);
    // Bound in the element's own context and around it, with modifiers
    // other than the secondary one: none runs, the listener has them.
    let plain = "left f8 alt-x x shift-a";
    cx.simulate_keystrokes(plain);
    assert_eq!(ran(&log), Vec::<String>::new());
    assert_eq!(keys_seen(&log), spelled(plain));
    // The other platform's command key is a plain modifier here.
    let other = match Os::current() {
        Os::MacOs => "ctrl-w",
        _ => "alt-w",
    };
    cx.simulate_keystrokes(other);
    assert_eq!(ran(&log), Vec::<String>::new());
    assert_eq!(keys_seen(&log), spelled(other));
}

#[gpui::test]
fn the_secondary_modifier_and_its_chords_stay_the_apps(cx: &mut TestAppContext) {
    let context = Box::leak(format!("pad {HOLDS_PLAIN_KEYS}").into_boxed_str());
    let (log, cx) = pad(cx, context);
    cx.simulate_keystrokes(&sec("secondary-w"));
    assert_eq!(ran(&log), ["pad.close"]);
    // A chord of the secondary modifier's.
    cx.simulate_keystrokes(&sec("secondary-k"));
    assert!(cx.update(|_, cx| chord_pending(cx)));
    cx.simulate_keystrokes(&sec("secondary-t"));
    assert_eq!(ran(&log), ["app.pick_theme"]);
    // One continued by a plain key: the key is the chord's, not the
    // listener's.
    cx.simulate_keystrokes(&sec("secondary-k z"));
    assert_eq!(ran(&log), ["app.zen"]);
    assert_eq!(keys_seen(&log), Vec::<String>::new());
    // A chord a plain key fails: that key is matched afresh, and held, so
    // the listener has it and `left` runs nothing.
    cx.simulate_keystrokes(&sec("secondary-k left"));
    assert_eq!(ran(&log), Vec::<String>::new());
    assert_eq!(keys_seen(&log), spelled("left"));
    assert!(!cx.update(|_, cx| chord_pending(cx)));
    // A prefix with a binding of its own waits for its plain
    // continuation, which completes the chord.
    cx.simulate_keystrokes(&sec("secondary-g j"));
    assert_eq!(ran(&log), ["app.go_down"]);
    assert_eq!(keys_seen(&log), Vec::<String>::new());
    // Alone, it runs at the timeout, after which plain keys are the
    // listener's again.
    cx.simulate_keystrokes(&sec("secondary-g"));
    assert_eq!(ran(&log), Vec::<String>::new());
    time_out(cx);
    assert_eq!(ran(&log), ["app.go"]);
    cx.simulate_keystrokes("j left");
    assert_eq!(ran(&log), Vec::<String>::new());
    assert_eq!(keys_seen(&log), spelled("j left"));
}

#[gpui::test]
fn tab_stays_the_apps_with_any_modifier(cx: &mut TestAppContext) {
    let context = Box::leak(format!("pad {HOLDS_PLAIN_KEYS}").into_boxed_str());
    let (log, cx) = pad(cx, context);
    // Bound, with control (not the secondary modifier on macOS): the app's.
    cx.simulate_keystrokes("ctrl-tab ctrl-shift-tab");
    assert_eq!(ran(&log), ["app.next_tab", "app.previous_tab"]);
    assert_eq!(keys_seen(&log), Vec::<String>::new());
    // Bound to nothing, Tab still reaches the listener, to move focus.
    cx.simulate_keystrokes("tab shift-tab");
    assert_eq!(ran(&log), Vec::<String>::new());
    assert_eq!(keys_seen(&log), spelled("tab shift-tab"));
}

#[gpui::test]
fn without_the_flag_the_bindings_take_plain_keys(cx: &mut TestAppContext) {
    let (log, cx) = pad(cx, "pad");
    cx.simulate_keystrokes("left f8 alt-x");
    assert_eq!(
        ran(&log),
        ["pad.move_left", "app.next_problem", "app.extend"]
    );
    assert_eq!(keys_seen(&log), Vec::<String>::new());
}
