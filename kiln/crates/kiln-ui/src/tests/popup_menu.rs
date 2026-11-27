// SPDX-License-Identifier: Apache-2.0

//! [`PopupMenu`]: its keys, the pointer, and its own focus, in a host that
//! opens it from a focused element.

use gpui::{
    Context, FocusHandle, InteractiveElement as _, IntoElement, KeyDownEvent, Keystroke, Modifiers,
    MouseButton, ParentElement as _, Render, Styled as _, TestAppContext, VisualTestContext,
    Window, anchored, deferred, div, point, px,
};

use crate::{MenuEntry, MenuInput, MenuModifiers, PopupMenu};

fn item(title: &'static str) -> MenuEntry {
    MenuEntry::Item {
        title: title.into(),
        shortcut: None,
    }
}

/// Cut, Copy | Paste (disabled), Delete.
fn menu() -> PopupMenu {
    let entries = vec![
        item("Cut"),
        item("Copy"),
        MenuEntry::Separator,
        item("Paste"),
        item("Delete"),
    ];
    PopupMenu::new(entries).disabled([3])
}

fn key(menu: &mut PopupMenu, key: &str) -> Option<MenuInput> {
    let event = KeyDownEvent {
        keystroke: Keystroke::parse(key).unwrap(),
        is_held: false,
        prefer_character_input: false,
    };
    menu.key(&event)
}

#[test]
fn down_and_up_wrap_and_skip_separators_and_disabled_entries() {
    let mut menu = menu();
    let mut downs = Vec::new();
    for _ in 0..4 {
        assert_eq!(key(&mut menu, "down"), Some(MenuInput::Handled));
        downs.push(menu.highlighted());
    }
    assert_eq!(downs, [Some(0), Some(1), Some(4), Some(0)]);
    key(&mut menu, "up");
    assert_eq!(menu.highlighted(), Some(4), "up wraps to the last");
    key(&mut menu, "up");
    assert_eq!(menu.highlighted(), Some(1), "past Paste and the separator");
    key(&mut menu, "end");
    assert_eq!(menu.highlighted(), Some(4));
    key(&mut menu, "home");
    assert_eq!(menu.highlighted(), Some(0));
    // A key with a modifier, or one it doesn't take, goes on.
    assert_eq!(key(&mut menu, "shift-down"), None);
    assert_eq!(key(&mut menu, "a"), None);
    assert_eq!(menu.highlighted(), Some(0));
}

#[test]
fn keys_held_with_a_modifier_it_takes_are_its_own() {
    let mut shift = menu().modifiers(MenuModifiers::Shift);
    assert_eq!(key(&mut shift, "shift-down"), Some(MenuInput::Handled));
    assert_eq!(shift.highlighted(), Some(0));
    assert_eq!(key(&mut shift, "shift-enter"), Some(MenuInput::Chosen(0)));
    assert_eq!(key(&mut shift, "shift-escape"), Some(MenuInput::Escape));
    assert_eq!(key(&mut shift, "ctrl-down"), None, "only Shift");

    let mut any = menu().modifiers(MenuModifiers::Any);
    assert_eq!(key(&mut any, "ctrl-alt-down"), Some(MenuInput::Handled));
    assert_eq!(key(&mut any, "cmd-enter"), Some(MenuInput::Chosen(0)));
}

/// A view with a focusable opener that opens the menu, focused, and what
/// the menu chose.
struct Host {
    opener: FocusHandle,
    menu: Option<PopupMenu>,
    chosen: Vec<usize>,
}

impl Host {
    fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let menu = menu().take_focus(&self.opener, "test_menu", window, cx, |host, _, cx| {
            host.menu = None;
            cx.notify();
        });
        self.menu = Some(menu);
        cx.notify();
    }

    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(menu) = self.menu.take() {
            menu.close(window, cx);
            cx.notify();
        }
    }

    fn on_input(&mut self, input: MenuInput, window: &mut Window, cx: &mut Context<Self>) {
        match input {
            MenuInput::Handled => cx.notify(),
            MenuInput::Chosen(ix) => {
                self.close(window, cx);
                self.chosen.push(ix);
            }
            MenuInput::Escape | MenuInput::PressedOutside(_) => self.close(window, cx),
        }
    }
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let menu = (self.menu.as_ref()).map(|menu| {
            let element = menu.render("test-menu", cx, |host| host.menu.as_mut(), Self::on_input);
            deferred(
                anchored()
                    .position(point(px(200.), px(200.)))
                    .child(element),
            )
        });
        div()
            .size_full()
            .child(
                div()
                    .id("opener")
                    .debug_selector(|| "opener".into())
                    .track_focus(&self.opener)
                    .size(px(40.)),
            )
            .children(menu)
    }
}

fn window(cx: &mut TestAppContext) -> (gpui::Entity<Host>, &mut VisualTestContext) {
    let (host, cx) = cx.add_window_view(|_, cx| Host {
        opener: cx.focus_handle(),
        menu: None,
        chosen: Vec::new(),
    });
    cx.update(|window, cx| {
        window.activate_window();
        let opener = host.read(cx).opener.clone();
        window.focus(&opener, cx);
        host.update(cx, |host, cx| host.open(window, cx));
    });
    cx.run_until_parked();
    (host, cx)
}

fn is_open(host: &gpui::Entity<Host>, cx: &mut VisualTestContext) -> bool {
    host.read_with(cx, |host, _| host.menu.is_some())
}

fn chosen(host: &gpui::Entity<Host>, cx: &mut VisualTestContext) -> Vec<usize> {
    host.read_with(cx, |host, _| host.chosen.clone())
}

#[gpui::test]
fn enter_runs_the_highlighted_entry_and_closes(cx: &mut TestAppContext) {
    let (host, cx) = window(cx);
    cx.simulate_keystrokes("down down enter");
    cx.run_until_parked();
    assert_eq!(chosen(&host, cx), [1]);
    assert!(!is_open(&host, cx));
}

#[gpui::test]
fn escape_closes_without_running(cx: &mut TestAppContext) {
    let (host, cx) = window(cx);
    cx.simulate_keystrokes("down escape");
    cx.run_until_parked();
    assert!(!is_open(&host, cx));
    assert!(chosen(&host, cx).is_empty());
}

#[gpui::test]
fn a_mouse_down_outside_closes(cx: &mut TestAppContext) {
    let (host, cx) = window(cx);
    let menu = cx.debug_bounds("test-menu").expect("the menu is drawn");
    // A press in it keeps it.
    cx.simulate_mouse_down(menu.center(), MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(menu.center(), MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    assert!(is_open(&host, cx));
    let outside = point(px(5.), px(100.));
    assert!(!menu.contains(&outside));
    cx.simulate_mouse_down(outside, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    assert!(!is_open(&host, cx));
    assert!(chosen(&host, cx).is_empty());
}

#[gpui::test]
fn hover_moves_the_highlight(cx: &mut TestAppContext) {
    let (host, cx) = window(cx);
    let highlighted = |cx: &mut VisualTestContext| {
        host.read_with(cx, |host, _| host.menu.as_ref().unwrap().highlighted())
    };
    let delete = cx
        .debug_bounds("menu-item:Delete")
        .expect("Delete is drawn");
    cx.simulate_mouse_move(delete.center(), None, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(highlighted(cx), Some(4));
    // A disabled item takes no hover.
    let paste = cx.debug_bounds("menu-item:Paste").expect("Paste is drawn");
    cx.simulate_mouse_move(paste.center(), None, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(highlighted(cx), Some(4));
    // Enter chooses what the pointer highlighted.
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert_eq!(chosen(&host, cx), [4]);
}

#[gpui::test]
fn closing_returns_focus_to_the_opener(cx: &mut TestAppContext) {
    let (host, cx) = window(cx);
    let (opener, own) = host.read_with(cx, |host, _| {
        let menu = host.menu.as_ref().unwrap();
        (host.opener.clone(), menu.focus_handle().unwrap().clone())
    });
    cx.update(|window, _| {
        assert!(own.is_focused(window), "the menu has its own focus");
        let context = window.context_stack().last().cloned().unwrap();
        assert!(context.contains("test_menu"), "{context:?}");
        assert!(context.contains(kiln_platform::keys::HOLDS_KEYS));
    });
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert!(cx.update(|window, _| opener.is_focused(window)));
}
