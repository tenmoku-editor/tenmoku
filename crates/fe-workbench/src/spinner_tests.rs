//! The workbench's spinners ([`fe_ui::Spinner`]): the status bar's
//! operation and its list, and the Runs panel's running rows. Each turns
//! while it's drawn and stands still, asking for no frames, with reduced
//! motion. The shared element's own tests are kiln-ui's.

use std::{fs, time::Duration};

use fe_buffer::Buffer;
use fe_command::{ContextFrame, ContextStack, Invocation};
use fe_gpui::commands::RunCommand;
use fe_ui::{DrawnSpinner, take_drawn_spinners};
use gpui::{Entity, Focusable as _, Modifiers, TestAppContext, VisualTestContext};

use crate::{
    Launch, Workbench,
    runs::{RunEnd, RunKind},
    tests::open,
};

fn opened(
    cx: &mut TestAppContext,
) -> (Entity<Workbench>, &mut VisualTestContext, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.txt");
    fs::write(&path, "a\n").unwrap();
    let launch = Launch {
        root: Some(dir.path().to_owned()),
        file: Some((path, Some(Buffer::new("a\n").into()))),
    };
    let (workbench, cx, _) = open(cx, launch);
    cx.update(|window, _| window.activate_window());
    cx.update(|window, cx| window.focus(&workbench.focus_handle(cx), cx));
    draw(cx);
    (workbench, cx, dir)
}

fn draw(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.run_until_parked();
}

/// What a fresh paint of the whole window draws.
fn painted(cx: &mut VisualTestContext) -> Vec<DrawnSpinner> {
    cx.run_until_parked();
    take_drawn_spinners();
    cx.update(|window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    });
    take_drawn_spinners()
}

/// Delivers the next frame, as a 60 Hz display would (and then some), then
/// draws: how many had asked for it.
fn next_frame(cx: &mut VisualTestContext) -> usize {
    cx.executor().advance_clock(Duration::from_millis(20));
    let asked = cx.update(|window, cx| window.simulate_next_frame(cx));
    draw(cx);
    asked
}

/// Whether, once the frames already asked for have come, nothing asks for
/// another.
fn quiet(cx: &mut VisualTestContext) -> bool {
    next_frame(cx);
    next_frame(cx) == 0 && next_frame(cx) == 0
}

/// Whether `spinner` is the one drawn in `selector`'s bounds.
fn at(spinner: &DrawnSpinner, selector: &'static str, cx: &mut VisualTestContext) -> bool {
    let within = cx.debug_bounds(selector).expect("drawn");
    within.contains(&spinner.bounds.center())
}

#[gpui::test]
fn the_status_bar_and_its_list_stand_still_with_reduced_motion(cx: &mut TestAppContext) {
    let (workbench, cx, _dir) = opened(cx);
    let operations = workbench.read_with(cx, |workbench, _| workbench.operations().clone());
    operations.update(cx, |operations, cx| operations.begin("Indexing", cx));
    draw(cx);
    let item = painted(cx);
    assert_eq!(item.len(), 1, "{item:?}");
    assert!(item[0].turning);
    assert!(at(&item[0], "status-operation-spinner", cx));

    // The list's row has one too, turning with the item's.
    let center = cx.debug_bounds("status-operation").unwrap().center();
    cx.simulate_click(center, Modifiers::none());
    draw(cx);
    let both = painted(cx);
    assert_eq!(both.len(), 2, "{both:?}");
    assert!(both.iter().all(|spinner| spinner.turning));
    assert!(
        both.iter().any(|s| at(s, "status-operations-row:0", cx)),
        "{both:?}"
    );

    cx.update(|_, cx| cx.set_reduce_motion(true));
    let still = painted(cx);
    assert_eq!(still.len(), 2, "{still:?}");
    assert!(
        still.iter().all(|s| s.angle == 0. && !s.turning),
        "{still:?}"
    );
    assert!(quiet(cx));
}

/// The spinners a fresh paint draws in the Runs panel's first row: a run is
/// an operation too, so the status bar has its own beside it.
fn in_row(drawn: Vec<DrawnSpinner>, cx: &mut VisualTestContext) -> Vec<DrawnSpinner> {
    let Some(row) = cx.debug_bounds("runs-row:0") else {
        return Vec::new();
    };
    (drawn.into_iter())
        .filter(|spinner| row.contains(&spinner.bounds.center()))
        .collect()
}

#[gpui::test]
fn the_runs_spinner_turns_while_drawn_and_stops_when_done_or_hidden(cx: &mut TestAppContext) {
    let (workbench, cx, _dir) = opened(cx);
    let runs = workbench.read_with(cx, |workbench, _| workbench.runs().clone());
    let going = runs.update(cx, |runs, cx| runs.begin(RunKind::Test, "cargo test", cx));
    let stack = ContextStack::new().with(ContextFrame::new(crate::KEY_CONTEXT));
    let show = RunCommand(Invocation::of("runs.show").unwrap().context(stack));
    let shown = show.clone();
    cx.update(|window, cx| window.dispatch_action(Box::new(shown), cx));
    draw(cx);
    let drawn = painted(cx);
    let first = in_row(drawn, cx);
    assert_eq!(first.len(), 1, "{first:?}");
    assert!(first[0].turning);

    // The frame it asked for draws it turned on.
    std::thread::sleep(Duration::from_millis(40));
    take_drawn_spinners();
    next_frame(cx);
    let second = take_drawn_spinners();
    let second = in_row(second, cx);
    assert_ne!(second.last().expect("painted").angle, first[0].angle);

    // Over a minute old, past the ages' ticks, it turns still.
    let panel = workbench.read_with(cx, |workbench, _| workbench.runs_panel().clone());
    let list = panel.read_with(cx, |panel, _| panel.list().clone());
    list.update(cx, |list, cx| {
        list.delegate_mut().age_by(Duration::from_secs(120));
        cx.notify();
    });
    draw(cx);
    let drawn = painted(cx);
    assert!(in_row(drawn, cx)[0].turning);

    // Once it ends: no spinner, and no more frames.
    runs.update(cx, |runs, cx| runs.finish(going, RunEnd::Passed, None, cx));
    draw(cx);
    assert!(quiet(cx));
    assert_eq!(painted(cx), []);

    // A running one in a hidden panel draws none there: the status bar's is
    // the only one left.
    runs.update(cx, |runs, cx| runs.begin(RunKind::Build, "cargo build", cx));
    draw(cx);
    assert_eq!(painted(cx).len(), 2);
    workbench.update(cx, |workbench, cx| workbench.hide_bottom_panel(cx));
    draw(cx);
    let left = painted(cx);
    assert_eq!(left.len(), 1, "{left:?}");
    let item = cx.debug_bounds("status-operation-spinner").expect("drawn");
    assert_eq!(left[0].bounds, item);

    // With reduced motion every one stands still, asking for no frames.
    cx.update(|_, cx| cx.set_reduce_motion(true));
    cx.update(|window, cx| window.dispatch_action(Box::new(show), cx));
    draw(cx);
    let still = painted(cx);
    assert_eq!(still.len(), 2, "{still:?}");
    assert!(
        still.iter().all(|s| s.angle == 0. && !s.turning),
        "{still:?}"
    );
    assert!(quiet(cx));
}
