// SPDX-License-Identifier: Apache-2.0

//! The shared spinner ([`super::Spinner`]): its angle, the frames it asks
//! for, reduced motion, and the widgets that draw it.

use std::time::Duration;

use gpui::{
    AnyElement, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, Styled as _, TestAppContext, VisualTestContext, Window, div, px,
};

use super::{DrawnSpinner, SPINNER_TURN, Spinner, spin_angle, take_drawn_spinners};
use crate::{Button, Icon, IconRef, Tab, ui_theme};

/// What the window shows.
#[derive(Clone, Copy)]
enum Shows {
    /// Two spinners.
    Pair,
    /// One pushed out of a clipping box.
    Clipped,
    /// A button, a tab and an [`IconRef`] given `Icon`.
    Widgets(Icon),
    Nothing,
}

struct Host(Shows);

/// A view of its own with one spinner.
struct Leaf;

impl Render for Leaf {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        Spinner::new(px(12.), gpui::black())
    }
}

/// A window of two views, the second shown once asked.
struct Leaves(Entity<Leaf>, Option<Entity<Leaf>>);

impl Render for Leaves {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .child(self.0.clone())
            .children(self.1.clone())
    }
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let color = gpui::black();
        let spinner = || Spinner::new(px(12.), color);
        let children: Vec<AnyElement> = match self.0 {
            Shows::Pair => vec![
                spinner()
                    .debug_selector(|| "first".into())
                    .into_any_element(),
                spinner().into_any_element(),
            ],
            Shows::Clipped => vec![
                div()
                    .w(px(40.))
                    .h(px(20.))
                    .overflow_hidden()
                    .child(div().pt(px(100.)).child(spinner()))
                    .into_any_element(),
            ],
            Shows::Widgets(icon) => {
                let theme = ui_theme(cx);
                vec![
                    Button::icon_only("button", icon)
                        .name("Running")
                        .into_any_element(),
                    Tab::new("tab", "task", &theme)
                        .icon(Some(icon))
                        .into_any_element(),
                    IconRef::Tinted(icon, "agent.base").element(px(14.), color),
                ]
            }
            Shows::Nothing => Vec::new(),
        };
        // Each in a 40 px row, so a tab doesn't push the next out of sight.
        let rows = (children.into_iter()).map(|child| div().flex().h(px(40.)).child(child));
        div().flex().flex_col().size_full().children(rows)
    }
}

fn open(cx: &mut TestAppContext, shows: Shows) -> (Entity<Host>, &mut VisualTestContext) {
    let (host, cx) = cx.add_window_view(|_, _| Host(shows));
    cx.run_until_parked();
    (host, cx)
}

/// What a fresh paint of the whole window draws.
fn painted(cx: &mut VisualTestContext) -> Vec<DrawnSpinner> {
    take_drawn_spinners();
    cx.update(|window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    });
    take_drawn_spinners()
}

/// Delivers the next frame, as a 60 Hz display would (and then some): how
/// many asked for it.
fn next_frame(cx: &mut VisualTestContext) -> usize {
    cx.executor().advance_clock(Duration::from_millis(20));
    let asked = cx.update(|window, cx| window.simulate_next_frame(cx));
    cx.run_until_parked();
    asked
}

#[test]
fn a_turn_takes_a_second() {
    use std::f32::consts::{PI, TAU};
    assert_eq!(SPINNER_TURN, Duration::from_secs(1));
    assert_eq!(spin_angle(Duration::ZERO), 0.);
    assert!((spin_angle(Duration::from_millis(500)) - PI).abs() < 1e-5);
    assert!((spin_angle(Duration::from_millis(50)) - TAU / 20.).abs() < 1e-5);
    assert_eq!(spin_angle(Duration::from_secs(7)), 0.);
    // Still exact after weeks.
    let weeks = Duration::from_secs(3_000_000) + Duration::from_millis(250);
    assert!((spin_angle(weeks) - TAU / 4.).abs() < 1e-5);
}

#[gpui::test]
fn it_turns_with_time_and_asks_for_each_next_frame(cx: &mut TestAppContext) {
    let (_, cx) = open(cx, Shows::Pair);
    let first = painted(cx);
    assert_eq!(first.len(), 2);
    assert!(first.iter().all(|spinner| spinner.turning));
    // Together: one epoch.
    assert!((first[0].angle - first[1].angle).abs() < 0.1, "{first:?}");
    let bounds = cx.debug_bounds("first").expect("the caller's selector");
    assert_eq!(first[0].bounds, bounds);
    assert_eq!(bounds.size, gpui::size(px(12.), px(12.)));

    std::thread::sleep(Duration::from_millis(50));
    take_drawn_spinners();
    assert!(next_frame(cx) > 0);
    // The frame it asked for draws it turned on.
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let second = take_drawn_spinners();
    assert!(!second.is_empty(), "the next frame painted it");
    assert_ne!(second.last().unwrap().angle, first[1].angle);
    assert!(next_frame(cx) > 0, "and asked again");
}

/// How many of the frames that come after each of `gaps` paint the spinners
/// again.
fn draws(gaps: impl IntoIterator<Item = Duration>, cx: &mut VisualTestContext) -> usize {
    (gaps.into_iter())
        .filter(|gap| {
            cx.executor().advance_clock(*gap);
            take_drawn_spinners();
            cx.update(|window, cx| {
                window.simulate_next_frame(cx);
            });
            cx.run_until_parked();
            !take_drawn_spinners().is_empty()
        })
        .count()
}

/// A second of a `hz` display's frames.
fn second_at(hz: u32) -> impl Iterator<Item = Duration> {
    std::iter::repeat_n(Duration::from_secs(1) / hz, hz as usize)
}

#[gpui::test]
fn it_is_drawn_at_most_60_times_a_second(cx: &mut TestAppContext) {
    let (_, cx) = open(cx, Shows::Pair);
    painted(cx);
    // Every other frame of a 120 Hz display, every one of a 60 Hz one.
    assert_eq!(draws(second_at(120), cx), 60);
    assert_eq!(draws(second_at(60), cx), 60);
    // Rates whose frames don't divide a 60th of a second.
    for hz in [75, 90, 100, 144, 165, 240] {
        let drawn = draws(second_at(hz), cx);
        assert!((59..=61).contains(&drawn), "{drawn} at {hz} Hz");
    }
    // A paint between two frames, as typing causes, adds no more.
    painted(cx);
    painted(cx);
    let after = draws(second_at(120), cx);
    assert!((59..=61).contains(&after), "{after}");
    let waiting = cx.update(|window, cx| window.simulate_next_frame(cx));
    assert!(waiting <= 1, "one wait for the window, not {waiting}");
}

#[gpui::test]
fn uneven_frames_are_held_to_60_a_second_too(cx: &mut TestAppContext) {
    let (_, cx) = open(cx, Shows::Pair);
    painted(cx);
    let ms = Duration::from_micros;
    // A 60 Hz display that jitters by a millisecond draws every frame.
    let jitter = [ms(15_667), ms(17_667)];
    assert_eq!(draws(jitter.into_iter().cycle().take(60), cx), 60);
    // Frames from 3 to 40 ms apart: never more than the time allows.
    let uneven = [3, 25, 5, 40, 4, 4, 4, 11, 30, 3, 3, 3, 9].map(Duration::from_millis);
    let gaps: Vec<_> = uneven.into_iter().cycle().take(260).collect();
    let periods = (gaps.iter().sum::<Duration>().as_secs_f64() * 60.) as usize;
    let drawn = draws(gaps, cx);
    assert!(drawn <= periods + 2, "{drawn} in {periods} periods");
    // After a stall, the missed frames aren't made up in a burst.
    cx.executor().advance_clock(Duration::from_secs(2));
    let after = draws(second_at(240).take(24), cx);
    assert!(after <= 7, "{after} in a tenth of a second");
}

#[gpui::test]
fn a_windows_views_share_its_60_a_second(cx: &mut TestAppContext) {
    let (leaves, cx) = cx.add_window_view(|_, cx| Leaves(cx.new(|_| Leaf), None));
    cx.run_until_parked();
    // The second view joins half a period out of step with the first.
    cx.executor().advance_clock(Duration::from_micros(8_333));
    leaves.update(cx, |leaves, cx| {
        leaves.1 = Some(cx.new(|_| Leaf));
        cx.notify();
    });
    cx.run_until_parked();
    assert_eq!(painted(cx).len(), 2);
    let drawn = draws(second_at(120), cx);
    assert!((59..=61).contains(&drawn), "{drawn}");
}

#[gpui::test]
fn removed_or_clipped_it_asks_for_nothing(cx: &mut TestAppContext) {
    let (host, cx) = open(cx, Shows::Clipped);
    let drawn = painted(cx);
    assert!(drawn.iter().all(|spinner| !spinner.turning), "{drawn:?}");
    next_frame(cx);
    assert_eq!(next_frame(cx), 0);

    host.update(cx, |host, cx| {
        host.0 = Shows::Pair;
        cx.notify();
    });
    cx.run_until_parked();
    assert!(next_frame(cx) > 0);
    host.update(cx, |host, cx| {
        host.0 = Shows::Nothing;
        cx.notify();
    });
    cx.run_until_parked();
    // The last frame asked for comes, and none after it.
    next_frame(cx);
    assert_eq!(painted(cx), []);
    assert_eq!(next_frame(cx), 0);
}

#[gpui::test]
fn reduced_motion_stands_still_and_asks_for_no_frames(cx: &mut TestAppContext) {
    cx.update(|cx| cx.set_reduce_motion(true));
    let (_, cx) = open(cx, Shows::Pair);
    for _ in 0..2 {
        let drawn = painted(cx);
        assert_eq!(drawn.len(), 2);
        assert!(
            drawn.iter().all(|s| s.angle == 0. && !s.turning),
            "{drawn:?}"
        );
        assert_eq!(next_frame(cx), 0);
        std::thread::sleep(Duration::from_millis(30));
    }
}

#[gpui::test]
fn buttons_tabs_and_icon_refs_draw_the_spinner(cx: &mut TestAppContext) {
    let (host, cx) = open(cx, Shows::Widgets(Icon::LoaderCircle));
    let drawn = painted(cx);
    assert_eq!(drawn.len(), 3);
    assert!(drawn.iter().all(|spinner| spinner.turning), "{drawn:?}");
    assert!(drawn.iter().all(|s| s.bounds.size.width == px(14.)));
    let icon = cx
        .debug_bounds("button-chevron")
        .expect("the button's icon");
    assert!(drawn.iter().any(|spinner| spinner.bounds == icon));

    // Any other icon is still.
    host.update(cx, |host, cx| {
        host.0 = Shows::Widgets(Icon::Check);
        cx.notify();
    });
    cx.run_until_parked();
    next_frame(cx);
    assert_eq!(painted(cx), []);
    assert_eq!(next_frame(cx), 0);
}
