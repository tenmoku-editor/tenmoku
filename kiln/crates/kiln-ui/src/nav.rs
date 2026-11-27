// SPDX-License-Identifier: Apache-2.0

//! Moving through rows by key: the keys [`crate::List`] and [`crate::Tree`]
//! share (moving a selection, a page at a time, and confirming it), and
//! [`step_wrapping`] for a menu's highlight, which wraps around.

use gpui::{KeyDownEvent, UniformListScrollHandle};

/// What a key asks a list of rows for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavKey {
    /// Select this row.
    To(usize),
    /// Confirm the selected row.
    Confirm,
    /// `left`: collapse, or go to the parent, in a tree.
    Left,
    /// `right`: expand, or go to the first child, in a tree.
    Right,
}

/// What `event` asks of `count` rows with `selected` selected, `page`
/// rows to a screen; `None` for a key the rows don't take, or a key with
/// modifiers, which go on to the app's bindings.
pub fn nav_key(
    event: &KeyDownEvent,
    selected: Option<usize>,
    count: usize,
    page: usize,
) -> Option<NavKey> {
    if event.keystroke.modifiers.modified() {
        return None;
    }
    let last = count.checked_sub(1);
    let page = page.max(1);
    let to = |ix: Option<usize>| ix.map(NavKey::To);
    match event.keystroke.key.as_str() {
        "up" => to(match selected {
            Some(ix) => Some(ix.saturating_sub(1)),
            None => last,
        }),
        "down" => to(match selected {
            Some(ix) => last.map(|last| (ix + 1).min(last)),
            None => last.map(|_| 0),
        }),
        "home" => to(last.map(|_| 0)),
        "end" => to(last),
        "pageup" => to(last.map(|_| selected.unwrap_or(0).saturating_sub(page))),
        "pagedown" => to(last.map(|last| (selected.unwrap_or(0) + page).min(last))),
        "enter" => selected.map(|_| NavKey::Confirm),
        "left" => Some(NavKey::Left),
        "right" => Some(NavKey::Right),
        _ => None,
    }
}

/// How many rows of `count` the list behind `scroll` showed in its last
/// frame, or 10 before it's been drawn.
pub fn page_rows(scroll: &UniformListScrollHandle, count: usize) -> usize {
    let state = scroll.0.borrow();
    // `item` is the list's own size, `contents` all its rows'.
    match state.last_item_size {
        Some(size) if count > 0 && size.contents.height > gpui::px(0.) => {
            let row = size.contents.height / count as f32;
            ((size.item.height / row).floor() as usize).max(1)
        }
        _ => 10,
    }
}

/// Where Down (or with `down` false, Up) moves a highlight from `from`
/// among `count` entries: the next one `enabled` takes, wrapping around;
/// from none, the first or the last it takes. `None` if it takes none.
pub fn step_wrapping(
    count: usize,
    from: Option<usize>,
    down: bool,
    enabled: impl Fn(usize) -> bool,
) -> Option<usize> {
    let enabled = |ix: &usize| enabled(*ix);
    if down {
        let after = from.map_or(0, |from| from.saturating_add(1));
        (after..count).chain(0..count).find(enabled)
    } else {
        let before = from.map_or(count, |from| from.min(count));
        (0..before).rev().chain((0..count).rev()).find(enabled)
    }
}
