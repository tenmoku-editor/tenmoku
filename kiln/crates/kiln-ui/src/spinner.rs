// SPDX-License-Identifier: Apache-2.0

//! [`Spinner`]: the turning [`Icon::LoaderCircle`] every widget shows while
//! something runs. [`Button`](crate::Button) and [`IconRef::element`] (so
//! tabs and tree rows) draw one when asked for that icon.
//!
//! Its angle is the time since one process-wide epoch, a turn a
//! [`SPINNER_TURN`], so every spinner on screen turns together. Each time
//! one is painted where it can be seen it asks to be drawn again, so it
//! turns on the display's frames while it's drawn and costs nothing once it
//! isn't. A window's spinners ask for at most 60 frames a second between
//! them, whatever its display's rate ([`FRAME_PERIOD`]): a frame redraws the
//! window, which at 120 a second took over a third of a core. With reduced
//! motion (`App::reduce_motion`) it stands still and asks for none.

use std::{
    collections::{HashMap, HashSet},
    mem,
    sync::LazyLock,
    time::{Duration, Instant},
};

use gpui::{
    App, Bounds, Element, ElementId, EntityId, Global, GlobalElementId, Hitbox, Hsla,
    InspectorElementId, InteractiveElement, Interactivity, IntoElement, LayoutId, Pixels,
    StyleRefinement, Styled, Svg, Transformation, Window, WindowId, radians, svg,
};

use crate::Icon;

/// How long a spinner takes to turn once.
pub const SPINNER_TURN: Duration = Duration::from_secs(1);

/// A 60th of a second: how far apart the frames a window's spinners ask for
/// are due.
const FRAME_PERIOD: Duration = Duration::from_nanos(16_666_667);

/// How long before it's due a frame may come and still be taken, so a 60 Hz
/// display that jitters draws them on every frame.
const EARLY: Duration = Duration::from_millis(2);

/// What every spinner's angle counts from.
static EPOCH: LazyLock<Instant> = LazyLock::new(Instant::now);

/// A spinner's angle `since` the epoch, in radians: one turn a
/// [`SPINNER_TURN`].
pub fn spin_angle(since: Duration) -> f32 {
    let turn = SPINNER_TURN.as_nanos();
    (since.as_nanos() % turn) as f32 / turn as f32 * std::f32::consts::TAU
}

/// What a window's spinners have asked of its display.
struct Pace {
    /// The views to draw again on the next due frame. While there are any, a
    /// next-frame callback is waiting for it.
    views: HashSet<EntityId>,
    /// When that frame is due.
    due: Instant,
}

/// Each window's [`Pace`].
#[derive(Default)]
struct Paces(HashMap<WindowId, Pace>);

impl Global for Paces {}

/// Asks for the view being painted to be drawn again, on the window's next
/// due frame. However many spinners ask, and however often they're painted
/// meanwhile, the window waits once.
fn draw_again(window: &mut Window, cx: &mut App) {
    let view = window.current_view();
    let id = window.window_handle().window_id();
    let now = cx.background_executor().now();
    let pace = (cx.default_global::<Paces>().0)
        .entry(id)
        .or_insert_with(|| Pace {
            views: HashSet::new(),
            due: now + FRAME_PERIOD,
        });
    let first = pace.views.is_empty();
    pace.views.insert(view);
    if first {
        wait(window);
    }
}

/// Notifies the waiting views on the first frame that's due.
fn wait(window: &Window) {
    window.on_next_frame(|window, cx| {
        let now = cx.background_executor().now();
        let id = window.window_handle().window_id();
        let Some(pace) = cx.default_global::<Paces>().0.get_mut(&id) else {
            return;
        };
        if now + EARLY < pace.due {
            wait(window);
            return;
        }
        // The next is due a period after this one was, not after this frame
        // came, so a display whose frames don't divide a period (75 or
        // 144 Hz) averages 60 a second too. A frame a whole period late
        // starts the schedule afresh: missed frames aren't made up.
        let kept = now < pace.due + FRAME_PERIOD;
        pace.due = if kept { pace.due } else { now } + FRAME_PERIOD;
        for view in mem::take(&mut pace.views) {
            cx.notify(view);
        }
    });
}

/// A turning [`Icon::LoaderCircle`]: see the module docs. It takes GPUI's
/// styling and interaction methods, as an `svg()` does.
pub struct Spinner {
    svg: Svg,
}

impl Spinner {
    /// A spinner `size` square in `color`, which doesn't shrink.
    #[track_caller]
    pub fn new(size: Pixels, color: Hsla) -> Self {
        Self {
            svg: svg()
                .path(Icon::LoaderCircle.path())
                .size(size)
                .flex_none()
                .text_color(color),
        }
    }
}

impl Styled for Spinner {
    fn style(&mut self) -> &mut StyleRefinement {
        self.svg.style()
    }
}

impl InteractiveElement for Spinner {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.svg.interactivity()
    }
}

impl IntoElement for Spinner {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for Spinner {
    type RequestLayoutState = <Svg as Element>::RequestLayoutState;
    type PrepaintState = Option<Hitbox>;

    fn id(&self) -> Option<ElementId> {
        Element::id(&self.svg)
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        Element::source_location(&self.svg)
    }

    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        Element::request_layout(&mut self.svg, id, inspector_id, window, cx)
    }

    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        state: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        Element::prepaint(&mut self.svg, id, inspector_id, bounds, state, window, cx)
    }

    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        state: &mut Self::RequestLayoutState,
        hitbox: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let still = cx.reduce_motion();
        let angle = if still {
            0.
        } else {
            spin_angle(EPOCH.elapsed())
        };
        // One scrolled or clipped out of sight asks for nothing.
        let turning = !still && window.content_mask().bounds.intersects(&bounds);
        self.svg = std::mem::replace(&mut self.svg, svg())
            .with_transformation(Transformation::rotate(radians(angle)));
        Element::paint(
            &mut self.svg,
            id,
            inspector_id,
            bounds,
            state,
            hitbox,
            window,
            cx,
        );
        if turning {
            draw_again(window, cx);
        }
        #[cfg(any(test, feature = "test-support"))]
        record::push(DrawnSpinner {
            bounds,
            angle,
            turning,
        });
    }
}

/// A spinner as it was painted, for tests.
#[cfg(any(test, feature = "test-support"))]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DrawnSpinner {
    pub bounds: Bounds<Pixels>,
    /// The angle it was drawn at, in radians.
    pub angle: f32,
    /// Whether it asked to be drawn again.
    pub turning: bool,
}

/// The spinners painted on this thread since the last call, oldest first.
#[cfg(any(test, feature = "test-support"))]
pub fn take_drawn_spinners() -> Vec<DrawnSpinner> {
    record::take()
}

#[cfg(any(test, feature = "test-support"))]
mod record {
    use std::cell::RefCell;

    use super::DrawnSpinner;

    thread_local! {
        static DRAWN: RefCell<Vec<DrawnSpinner>> = const { RefCell::new(Vec::new()) };
    }

    pub(super) fn push(spinner: DrawnSpinner) {
        DRAWN.with_borrow_mut(|drawn| drawn.push(spinner));
    }

    pub(super) fn take() -> Vec<DrawnSpinner> {
        DRAWN.take()
    }
}

// Beside this file, not under `tests/`: they use what the base doesn't have.
#[cfg(test)]
#[path = "spinner_tests.rs"]
mod tests;
