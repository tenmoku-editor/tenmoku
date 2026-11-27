// SPDX-License-Identifier: Apache-2.0

//! A popup menu's state and input, around [`MenuPanel`]: which entry is
//! highlighted, the keys that move and choose it, the pointer's hover, a
//! press outside, and, if it takes it, its own focus. Its host (a view
//! with a right-click menu, a `+` button's menu) keeps a [`PopupMenu`] in
//! its own state while the menu is open, decides what's in it and where
//! it's drawn, and is told what the menu's input asks of it
//! ([`MenuInput`]): run an entry, or close.
//!
//! - **Entries** are [`MenuEntry`]s; separators and
//!   [disabled](PopupMenu::disabled) items are passed over by the keys and
//!   take no hover or click.
//! - **Keys** ([`PopupMenu::key`]), without a modifier unless the menu
//!   [takes some](PopupMenu::modifiers): Up and Down move
//!   the highlight, wrapping ([`step_wrapping`]); from none, to the first
//!   or the last item. Home and End go to the first and last item. Enter
//!   chooses the highlighted one, Escape asks to close.
//! - **The pointer**: hovering an item highlights it, clicking one chooses
//!   it, and a press anywhere outside the menu is told to the host with
//!   where it was, which closes it unless the press is the opener's own
//!   (whose click closes it, rather than open it again).
//! - **Focus.** By default the host keeps focus and hands the menu the
//!   keys its focused element gets ([`PopupMenu::key`]), with a key
//!   context of its own swapped in while the menu is open so its bindings
//!   wait. Or the menu [takes focus](PopupMenu::take_focus): its element
//!   tracks a handle of its own, with a key context carrying kiln's
//!   [`HOLDS_KEYS`] so no binding runs for a key without the platform
//!   modifier, and every key reaches the menu, none the host. Closing
//!   ([`PopupMenu::close`]) hands focus back to the opener if the menu
//!   still has it; focus leaving for anything else tells the host, which
//!   closes it. That focus is a [`MenuFocus`], which a menu drawing its own
//!   rows can take by itself.
//!
//! [`HOLDS_KEYS`]: kiln_platform::keys::HOLDS_KEYS

use gpui::{
    App, Context, Div, FocusHandle, InteractiveElement, KeyContext, KeyDownEvent, Modifiers,
    MouseDownEvent, ParentElement as _, Pixels, Point, Stateful, Subscription, Window, div,
};

use crate::{MenuEntry, MenuPanel, step_wrapping};

/// What a popup menu's input asks of its host.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MenuInput {
    /// A key the menu took and handled itself: it moved the highlight, or
    /// was Enter with nothing highlighted. The host redraws.
    Handled,
    /// Enter on the highlighted item, or a click on an item: the host
    /// closes the menu and runs entry `ix`.
    Chosen(usize),
    /// Escape: the host closes the menu.
    Escape,
    /// A mouse press outside the menu, at this point in the window: the
    /// host closes the menu, unless the press is on its opener, whose own
    /// click closes it.
    PressedOutside(Point<Pixels>),
}

/// Where a host keeps its open menu: `menu(host)` finds it, if it's still
/// open.
pub type MenuOf<V> = fn(&mut V) -> Option<&mut PopupMenu>;

/// What a host does with its menu's input.
pub type OnMenuInput<V> = fn(&mut V, MenuInput, &mut Window, &mut Context<V>);

/// Which modifiers a key may be held with and still be a popup menu's.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MenuModifiers {
    /// None: a key with any modifier goes on, to the host and the
    /// bindings.
    #[default]
    None,
    /// Shift, which is ignored; a key with any other goes on.
    Shift,
    /// Any, all ignored: Shift-Down moves the highlight as Down does.
    Any,
}

impl MenuModifiers {
    /// Whether a key held with `modifiers` may be the menu's.
    pub fn allow(self, modifiers: &Modifiers) -> bool {
        match self {
            MenuModifiers::None => !modifiers.modified(),
            MenuModifiers::Shift => {
                !(modifiers.control || modifiers.alt || modifiers.platform || modifiers.function)
            }
            MenuModifiers::Any => true,
        }
    }
}

/// A popup menu's own focus while it's open: a new handle its element
/// tracks, and the opener's to give back. See the [module](self).
pub struct MenuFocus {
    handle: FocusHandle,
    /// What had focus as it opened, which gets it back.
    opener: FocusHandle,
    /// Tells the host once focus leaves the handle.
    _left: Subscription,
}

impl MenuFocus {
    /// Focuses a new handle for a menu `opener`'s view opened, calling
    /// `left` on the view once focus leaves it for anything else (or the
    /// element tracking it is no longer drawn). GPUI tells focus changes as
    /// a frame is drawn in an active window: `left` runs in the draw's
    /// focus phase, where a notify asks for no frame.
    pub fn take<V: 'static>(
        opener: &FocusHandle,
        window: &mut Window,
        cx: &mut Context<V>,
        mut left: impl FnMut(&mut V, &mut Window, &mut Context<V>) + 'static,
    ) -> Self {
        let handle = cx.focus_handle();
        let left = cx.on_focus_out(&handle, window, move |view, _, window, cx| {
            left(view, window, cx)
        });
        window.focus(&handle, cx);
        Self {
            handle,
            opener: opener.clone(),
            _left: left,
        }
    }

    /// The handle the menu's element tracks, with a [`menu_key_context`].
    pub fn handle(&self) -> &FocusHandle {
        &self.handle
    }

    /// Hands focus back to the opener, if the menu still has it.
    pub fn give_back(self, window: &mut Window, cx: &mut App) {
        self.give_back_later().finish(window, cx);
    }

    /// For a close without a window: what gives focus back at the host's
    /// next render.
    pub fn give_back_later(self) -> FocusReturn {
        FocusReturn {
            handle: self.handle,
            opener: self.opener,
        }
    }
}

/// An open popup menu's state: see the [module](self).
pub struct PopupMenu {
    entries: Vec<MenuEntry>,
    /// Whether each entry is an item the highlight stops on.
    enabled: Vec<bool>,
    highlighted: Option<usize>,
    item_nodes: bool,
    modifiers: MenuModifiers,
    /// Its own focus, if it took it, and its element's key context.
    focus: Option<(MenuFocus, &'static str)>,
}

impl PopupMenu {
    /// A menu of `entries`, nothing highlighted, every item enabled; the
    /// host keeps focus.
    pub fn new(entries: Vec<MenuEntry>) -> Self {
        let enabled = entries.iter().map(is_item).collect();
        Self {
            entries,
            enabled,
            highlighted: None,
            item_nodes: false,
            modifiers: MenuModifiers::None,
            focus: None,
        }
    }

    /// Disables the items at `entries` (indices in the entries): drawn
    /// muted, passed over by the keys, and taking no hover or click.
    pub fn disabled(mut self, entries: impl IntoIterator<Item = usize>) -> Self {
        for ix in entries {
            if let Some(enabled) = self.enabled.get_mut(ix) {
                *enabled = false;
            }
        }
        self
    }

    /// Makes each item a menu item for assistive technology
    /// ([`MenuPanel::item_nodes`]).
    pub fn item_nodes(mut self) -> Self {
        self.item_nodes = true;
        self
    }

    /// Lets keys held with `modifiers` be the menu's ([`Self::key`]).
    pub fn modifiers(mut self, modifiers: MenuModifiers) -> Self {
        self.modifiers = modifiers;
        self
    }

    /// Focuses a new handle of the menu's own ([`MenuFocus`]), whose
    /// element carries key context `key_context` and kiln's `HOLDS_KEYS`;
    /// `opener` gets focus back as it closes ([`Self::close`]). Once focus
    /// leaves the handle for anything else (or the menu is no longer
    /// drawn), `left` runs on the host, which closes it, in the draw's
    /// focus phase.
    pub fn take_focus<V: 'static>(
        mut self,
        opener: &FocusHandle,
        key_context: &'static str,
        window: &mut Window,
        cx: &mut Context<V>,
        left: impl FnMut(&mut V, &mut Window, &mut Context<V>) + 'static,
    ) -> Self {
        self.focus = Some((MenuFocus::take(opener, window, cx, left), key_context));
        self
    }

    pub fn entries(&self) -> &[MenuEntry] {
        &self.entries
    }

    /// The highlighted entry, if any.
    pub fn highlighted(&self) -> Option<usize> {
        self.highlighted
    }

    /// Highlights entry `ix`, or nothing; an entry the highlight doesn't
    /// stop on highlights nothing.
    pub fn set_highlighted(&mut self, ix: Option<usize>) {
        self.highlighted = ix.filter(|&ix| self.is_enabled(ix));
    }

    /// Whether entry `ix` is an item the highlight stops on.
    pub fn is_enabled(&self, ix: usize) -> bool {
        self.enabled.get(ix).copied().unwrap_or(false)
    }

    /// The menu's own focus handle, if it [took focus](Self::take_focus).
    pub fn focus_handle(&self) -> Option<&FocusHandle> {
        Some(self.focus.as_ref()?.0.handle())
    }

    /// Puts `entries` in place of the menu's, every item enabled, nothing
    /// highlighted.
    pub fn set_entries(&mut self, entries: Vec<MenuEntry>) {
        self.enabled = entries.iter().map(is_item).collect();
        self.entries = entries;
        self.highlighted = None;
    }

    /// Inserts `entries` at `at`, those at `disabled` (indices in
    /// `entries`) disabled. The highlight stays on its item.
    pub fn insert(
        &mut self,
        at: usize,
        entries: Vec<MenuEntry>,
        disabled: impl IntoIterator<Item = usize>,
    ) {
        let at = at.min(self.entries.len());
        let count = entries.len();
        let mut enabled: Vec<bool> = entries.iter().map(is_item).collect();
        for ix in disabled {
            if let Some(on) = enabled.get_mut(ix) {
                *on = false;
            }
        }
        self.entries.splice(at..at, entries);
        self.enabled.splice(at..at, enabled);
        if let Some(highlighted) = self.highlighted.filter(|&ix| ix >= at) {
            self.highlighted = Some(highlighted + count);
        }
    }

    /// Moves the highlight to the next item, or with `down` false the one
    /// before, wrapping; from none, to the first or the last. Whether it
    /// moved.
    pub fn step(&mut self, down: bool) -> bool {
        let enabled = &self.enabled;
        let next = step_wrapping(enabled.len(), self.highlighted, down, |ix| enabled[ix]);
        let moved = next.is_some() && next != self.highlighted;
        if next.is_some() {
            self.highlighted = next;
        }
        moved
    }

    /// The pointer over entry `ix`: highlights it if it's an enabled
    /// item. Whether the highlight moved.
    pub fn hover(&mut self, ix: usize) -> bool {
        if self.highlighted == Some(ix) || !self.is_enabled(ix) {
            return false;
        }
        self.highlighted = Some(ix);
        true
    }

    /// What `event` asks of the menu: see the [module](self). `None` for a
    /// key it doesn't take, or one held with a modifier it doesn't
    /// ([`Self::modifiers`]), which go on.
    pub fn key(&mut self, event: &KeyDownEvent) -> Option<MenuInput> {
        let keystroke = &event.keystroke;
        if !self.modifiers.allow(&keystroke.modifiers) {
            return None;
        }
        let first = self.enabled.iter().position(|&on| on);
        let last = self.enabled.iter().rposition(|&on| on);
        match keystroke.key.as_str() {
            "up" | "down" => {
                self.step(keystroke.key == "down");
            }
            "home" => self.highlighted = first.or(self.highlighted),
            "end" => self.highlighted = last.or(self.highlighted),
            "enter" => {
                return Some(
                    self.highlighted
                        .map_or(MenuInput::Handled, MenuInput::Chosen),
                );
            }
            "escape" => return Some(MenuInput::Escape),
            _ => return None,
        }
        Some(MenuInput::Handled)
    }

    /// Closes the menu: focus goes back to the opener if the menu took it
    /// and still has it. Focus that left for elsewhere stays there.
    pub fn close(self, window: &mut Window, cx: &mut App) {
        if let Some((focus, _)) = self.focus {
            focus.give_back(window, cx);
        }
    }

    /// For a close without a window: what gives focus back at the host's
    /// next render ([`FocusReturn::finish`]), if the menu took it.
    pub fn close_later(self) -> Option<FocusReturn> {
        self.focus.map(|(focus, _)| focus.give_back_later())
    }

    /// The menu's element for host `V`, which keeps the menu at `menu` and
    /// handles its input with `on_input`: the panel of its entries, the
    /// highlighted one on the selection colour, with element id and debug
    /// selector `id`. The host draws it where the menu goes.
    pub fn render<V: 'static>(
        &self,
        id: &'static str,
        cx: &mut Context<V>,
        menu: MenuOf<V>,
        on_input: OnMenuInput<V>,
    ) -> Stateful<Div> {
        let disabled = (0..self.entries.len())
            .filter(|&ix| is_item(&self.entries[ix]) && !self.is_enabled(ix));
        let (clicked, hovered) = (cx.entity().downgrade(), cx.entity().downgrade());
        let mut panel = MenuPanel::new(self.entries.clone(), &crate::ui_theme(cx))
            .highlighted(self.highlighted)
            .disabled(disabled)
            .on_click(move |ix, window, cx| {
                let chosen = |view: &mut V, cx: &mut Context<V>| {
                    on_input(view, MenuInput::Chosen(ix), window, cx)
                };
                (clicked.update(cx, chosen)).ok();
            })
            .on_hover(move |ix, _, cx| {
                (hovered.update(cx, |view, cx| {
                    if menu(view).is_some_and(|menu| menu.hover(ix)) {
                        cx.notify();
                    }
                }))
                .ok();
            });
        if self.item_nodes {
            panel = panel.item_nodes();
        }
        let element = div().id(id).debug_selector(move || id.into()).child(panel);
        self.wire(element, cx, menu, on_input)
    }

    /// `element`, the menu's or one a host draws its own menu's rows in,
    /// taking the menu's input for host `V` as [`Self::render`] does: a
    /// press outside it, and, if the menu took focus, its focus and every
    /// key. A host that keeps focus hands it keys with [`Self::key`].
    pub fn wire<V: 'static, E: InteractiveElement>(
        &self,
        element: E,
        cx: &mut Context<V>,
        menu: MenuOf<V>,
        on_input: OnMenuInput<V>,
    ) -> E {
        let element = element.on_mouse_down_out(cx.listener(
            move |view, event: &MouseDownEvent, window, cx| {
                if menu(view).is_some() {
                    on_input(view, MenuInput::PressedOutside(event.position), window, cx);
                }
            },
        ));
        let Some((focus, key_context)) = &self.focus else {
            return element;
        };
        element
            .track_focus(focus.handle())
            .key_context(menu_key_context(key_context))
            .on_key_down(cx.listener(move |view, event: &KeyDownEvent, window, cx| {
                // Every key is the menu's.
                cx.stop_propagation();
                let Some(input) = menu(view).and_then(|menu| menu.key(event)) else {
                    return;
                };
                on_input(view, input, window, cx);
            }))
    }
}

/// Whether `entry` is an item, which the highlight can stop on.
fn is_item(entry: &MenuEntry) -> bool {
    matches!(entry, MenuEntry::Item { .. })
}

/// The key context of a menu with focus, named `name`: its name, and
/// kiln's `HOLDS_KEYS`, so no binding runs for a key without the platform
/// modifier.
pub fn menu_key_context(name: &str) -> KeyContext {
    let mut context = KeyContext::default();
    context.add(name);
    context.add(kiln_platform::keys::HOLDS_KEYS);
    context
}

/// Focus a closed menu gives back, at its host's next render, if it still
/// has it: [`PopupMenu::close_later`], [`MenuFocus::give_back_later`].
pub struct FocusReturn {
    handle: FocusHandle,
    opener: FocusHandle,
}

impl FocusReturn {
    /// Focuses the opener, if the menu's handle still has focus.
    pub fn finish(self, window: &mut Window, cx: &mut App) {
        if self.handle.is_focused(window) {
            window.focus(&self.opener, cx);
        }
    }
}
