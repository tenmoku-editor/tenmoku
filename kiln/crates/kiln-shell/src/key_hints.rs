// SPDX-License-Identifier: Apache-2.0

//! The key a tooltip, a button or a palette row shows for a command: the
//! first key sequence (or the first single keystroke) that runs it in a
//! context stack, as this platform prints it ([`shortcut_label`]). Each
//! lookup reads the keymap the app matches against now
//! ([`keys::current_keymap`]), so it follows reloads.

use gpui::App;
use kiln_command::{CommandId, ContextFrame, ContextStack, Value};
use kiln_core::{KeySequence, shortcut_label, shortcuts};
use kiln_platform::keys;

/// Looks up the keys that run commands, without args, in one context
/// stack.
#[derive(Clone, Debug, Default)]
pub struct KeyHints {
    stack: ContextStack,
}

impl KeyHints {
    /// Keys for commands run in `stack`.
    pub fn new(stack: ContextStack) -> Self {
        Self { stack }
    }

    /// Keys for commands run in a stack of plain frames, `contexts`,
    /// outermost first.
    pub fn in_contexts<S: AsRef<str>>(contexts: &[S]) -> Self {
        Self::new(
            (contexts.iter())
                .map(|context| ContextFrame::new(context.as_ref()))
                .collect(),
        )
    }

    /// The context stack it looks keys up in.
    pub fn stack(&self) -> &ContextStack {
        &self.stack
    }

    /// The first key sequence, chords too, that runs `command` in the
    /// stack, as this platform prints it: `None` if none does, or if
    /// `command` isn't a command ID.
    pub fn label(&self, command: &str, cx: &App) -> Option<String> {
        self.first(command, cx, |_| true)
    }

    /// The first single keystroke that runs `command` in the stack, as this
    /// platform prints it: a chord isn't shown. `None` if none does, or if
    /// `command` isn't a command ID.
    pub fn single(&self, command: &str, cx: &App) -> Option<String> {
        self.first(command, cx, |keys| keys.keystrokes().len() == 1)
    }

    /// The first of the keys that run `command` in the stack that `wanted`
    /// takes, labelled.
    fn first(
        &self,
        command: &str,
        cx: &App,
        wanted: impl Fn(&KeySequence) -> bool,
    ) -> Option<String> {
        let command = CommandId::parse(command).ok()?;
        let keymap = keys::current_keymap(cx);
        // `shortcuts` copies the keymap to replay each binding: skip that
        // for a command nothing binds.
        if keymap.bindings_for(&command).is_empty() {
            return None;
        }
        (shortcuts(&keymap, &command, &Value::Null, &self.stack).iter())
            .find(|keys| wanted(keys))
            .map(shortcut_label)
    }
}
