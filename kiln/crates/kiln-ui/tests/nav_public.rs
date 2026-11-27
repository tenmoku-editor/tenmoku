// SPDX-License-Identifier: Apache-2.0

//! A host that draws its own rows reaches kiln-ui's key handling for them.

use gpui::{KeyDownEvent, Keystroke};
use kiln_ui::{NavKey, nav_key, step_wrapping};

#[test]
fn nav_key_and_step_wrapping_are_public() {
    let down = KeyDownEvent {
        keystroke: Keystroke::parse("down").unwrap(),
        is_held: false,
        prefer_character_input: false,
    };
    assert_eq!(nav_key(&down, Some(1), 3, 10), Some(NavKey::To(2)));
    assert_eq!(step_wrapping(3, Some(2), true, |_| true), Some(0));
}
