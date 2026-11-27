//! The branches menu's keys held with Shift (T2155): Shift is ignored, as
//! it was before the menu's keys were [`fe_ui::PopupMenu`]'s, so Shift-Down
//! moves the highlight, Shift-Enter and Shift-Right open a branch's
//! actions, Shift-Left and Shift-Escape close them, and Shift-Escape closes
//! the menu.

use gpui::{Entity, TestAppContext, VisualTestContext};

use super::super::git_branch_tests::{draw, fixture, init, window};
use crate::{Workbench, workbench::git_commands::GitCommand};

fn highlighted(workbench: &Entity<Workbench>, cx: &mut VisualTestContext) -> Option<String> {
    workbench.read_with(cx, |workbench, cx| {
        workbench.branches_menu(cx)?.read(cx).highlighted_line()
    })
}

fn actions_open(workbench: &Entity<Workbench>, cx: &mut VisualTestContext) -> bool {
    workbench.read_with(cx, |workbench, cx| {
        (workbench.branches_menu(cx)).is_some_and(|menu| menu.read(cx).actions().is_some())
    })
}

fn keys(keys: &str, cx: &mut VisualTestContext) {
    cx.simulate_keystrokes(keys);
    draw(cx);
}

#[gpui::test]
fn shift_held_keys_are_the_menus(cx: &mut TestAppContext) {
    let dir = fixture();
    init(cx);
    let (workbench, mut cx) = window(cx, &dir.path().join("work"));
    let cx = &mut cx;
    cx.update(|window, cx| {
        workbench.update(cx, |workbench, cx| {
            workbench.run_git_command(GitCommand::BranchesMenu, window, cx)
        })
    });
    draw(cx);

    keys("shift-down shift-down", cx);
    assert_eq!(highlighted(&workbench, cx).as_deref(), Some("Commit…"));
    keys("shift-up", cx);
    assert_eq!(
        highlighted(&workbench, cx).as_deref(),
        Some("Update Project")
    );

    // On a branch: Shift-Right and Shift-Enter open its actions, Shift-Left
    // and Shift-Escape close them.
    keys("shift-up", cx);
    assert!(highlighted(&workbench, cx).is_some_and(|line| line.starts_with("  ")));
    keys("shift-right", cx);
    assert!(actions_open(&workbench, cx));
    keys("shift-left", cx);
    assert!(!actions_open(&workbench, cx));
    keys("shift-enter", cx);
    assert!(actions_open(&workbench, cx));
    keys("shift-escape", cx);
    assert!(!actions_open(&workbench, cx));

    keys("shift-escape", cx);
    let shown = workbench.read_with(cx, |workbench, cx| workbench.branches_menu(cx).is_some());
    assert!(!shown, "Shift-Escape closes the menu");
}
