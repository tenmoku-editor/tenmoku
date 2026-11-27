//! The editor's right-click menu ([`super`]) on [`fe_ui::PopupMenu`]
//! (T2155): a group added with no anchor item runs its own items whether
//! clicked or chosen with Enter, and keys held with a modifier are the
//! menu's as plain ones are.

use std::{cell::RefCell, rc::Rc};

use fe_buffer::Buffer;
use fe_command::Invocation;
use fe_editor::EditorModel;
use gpui::{
    AppContext as _, Context, Entity, Focusable as _, InteractiveElement as _, IntoElement,
    Modifiers, MouseButton, ParentElement as _, Render, Styled as _, TestAppContext,
    VisualTestContext, Window, div, px,
};

use crate::{
    EditorMenuItem, EditorTheme, EditorView, commands::RunCommand, set_editor_menu_provider,
};

/// The editor inside a parent that records the commands it passes on.
struct Host {
    editor: Entity<EditorView>,
    ran: Rc<RefCell<Vec<String>>>,
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let ran = self.ran.clone();
        div()
            .size_full()
            .on_action(move |run: &RunCommand, _, _| {
                ran.borrow_mut().push(run.0.command.as_str().to_owned());
            })
            .child(self.editor.clone())
    }
}

fn item(title: &'static str, command: &str) -> Option<EditorMenuItem> {
    Some(EditorMenuItem {
        title: title.into(),
        shortcut: None,
        invocation: Invocation::of(command).unwrap(),
        enabled: true,
    })
}

/// A focused editor in an active window whose menu offers A and B, open
/// at the text's start; and the commands run from it.
fn open(
    cx: &mut TestAppContext,
) -> (
    Entity<EditorView>,
    &mut VisualTestContext,
    Rc<RefCell<Vec<String>>>,
) {
    cx.update(crate::init);
    let ran: Rc<RefCell<Vec<String>>> = Rc::default();
    let sink = ran.clone();
    let (host, cx) = cx.add_window_view(move |_, cx| {
        let editor = cx.new(|cx| {
            let model = EditorModel::new(Buffer::new("one two\nthree\n"), Some("a.rs".into()));
            let mut theme = EditorTheme::default_dark(cx);
            theme.font_size = px(13.);
            EditorView::new(model, theme, cx)
        });
        Host { editor, ran: sink }
    });
    let view = host.read_with(cx, |host, _| host.editor.clone());
    cx.update(|window, cx| {
        window.activate_window();
        window.focus(&view.focus_handle(cx), cx);
        set_editor_menu_provider(
            cx,
            Rc::new(|_, _, _| vec![item("A", "test.a"), item("B", "test.b")]),
        );
    });
    draw(cx);
    right_click(&view, cx);
    (view, cx, ran)
}

/// A right click in the text, which opens the menu.
fn right_click(view: &Entity<EditorView>, cx: &mut VisualTestContext) {
    let at = view.update(cx, |view, _| {
        view.caret_bounds_at(1).expect("drawn").center()
    });
    cx.simulate_mouse_down(at, MouseButton::Right, Modifiers::none());
    cx.simulate_mouse_up(at, MouseButton::Right, Modifiers::none());
    draw(cx);
    assert!(view.read_with(cx, |view, _| view.editor_menu_ticket().is_some()));
}

fn draw(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.run_until_parked();
}

/// Adds X and Y to the open menu at an anchor it doesn't have, after it
/// or before it: a new last group.
fn add_group(view: &Entity<EditorView>, before: bool, cx: &mut VisualTestContext) {
    view.update(cx, |view, cx| {
        let ticket = view.editor_menu_ticket().unwrap();
        let items = vec![item("X", "test.x"), item("Y", "test.y")];
        match before {
            false => view.extend_editor_menu(ticket, "test.none", items, cx),
            true => view.extend_editor_menu_before(ticket, "test.none", items, cx),
        }
    });
    draw(cx);
}

#[gpui::test]
fn an_unanchored_group_runs_its_own_items(cx: &mut TestAppContext) {
    // Clicked.
    let (view, cx, ran) = open(cx);
    add_group(&view, false, cx);
    let y = cx.debug_bounds("menu-item:Y").expect("Y is drawn");
    cx.simulate_click(y.center(), Modifiers::none());
    draw(cx);
    assert_eq!(*ran.borrow(), ["test.y"]);

    // Chosen with Enter: A, B, the separator passed, X, Y.
    right_click(&view, cx);
    add_group(&view, true, cx);
    cx.simulate_keystrokes("down down down enter");
    draw(cx);
    assert_eq!(*ran.borrow(), ["test.y", "test.x"]);
}

#[gpui::test]
fn keys_held_with_a_modifier_are_the_menus(cx: &mut TestAppContext) {
    let (view, cx, ran) = open(cx);
    let highlighted =
        |cx: &mut VisualTestContext| view.read_with(cx, |view, _| view.editor_menu_highlighted());
    cx.simulate_keystrokes("shift-down shift-down");
    assert_eq!(highlighted(cx), Some(1));
    cx.simulate_keystrokes("shift-up");
    assert_eq!(highlighted(cx), Some(0));
    cx.simulate_keystrokes("shift-enter");
    draw(cx);
    assert_eq!(*ran.borrow(), ["test.a"]);

    right_click(&view, cx);
    cx.simulate_keystrokes("shift-escape");
    draw(cx);
    assert_eq!(
        view.read_with(cx, |view, _| view.editor_menu_ticket()),
        None
    );
    assert_eq!(ran.borrow().len(), 1, "nothing more ran");
}
