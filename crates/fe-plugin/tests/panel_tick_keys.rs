// SPDX-License-Identifier: Apache-2.0

//! Ticks and keys reaching a plugin's panel: the host owns focus, so a tick comes only while the
//! panel is shown and focused, one at a time, at most 20 a second, and a missed one is dropped
//! and counted rather than delivered late; a key comes only while the panel is focused, and never
//! one the host keeps (T2005 C2, C3).
//!
//! `realtime_fields_without_the_capability_are_refused` is C1's and lives in
//! `kiln/crates/kiln-plugin/tests/panel_realtime.rs`: the manifest and the capability are
//! kiln-plugin's, and that half waits for T1706, which edits the same manifest file.

use std::time::{Duration, Instant};

use fe_plugin::panels::{Delivery, KeyEvent, MIN_TICK_INTERVAL, PanelRuntime, PanelState, Skipped};

const PANEL: &str = "todos";

fn shown_and_focused() -> PanelState {
    PanelState {
        shown: true,
        focused: true,
    }
}

/// A key with no modifiers.
fn key(name: &str) -> KeyEvent {
    KeyEvent {
        key: name.to_owned(),
        shift: false,
        ctrl: false,
        alt: false,
        secondary: false,
    }
}

#[test]
fn tick_is_not_delivered_while_hidden_or_unfocused() {
    let runtime = PanelRuntime::new();
    let now = Instant::now();
    for state in [
        PanelState {
            shown: false,
            focused: false,
        },
        PanelState {
            shown: true,
            focused: false,
        },
        PanelState {
            shown: false,
            focused: true,
        },
    ] {
        assert_eq!(
            runtime.deliver_tick(PANEL, state, now),
            Delivery::Skipped(Skipped::Unfocused),
            "{state:?}"
        );
    }
    // Shown and focused, the same instant: the panel has had no tick yet, so this one is due.
    assert_eq!(
        runtime.deliver_tick(PANEL, shown_and_focused(), now),
        Delivery::Delivered
    );
    // None of the three refusals counts as a missed tick: the plugin was never owed them.
    assert_eq!(runtime.dropped_ticks(PANEL), 0);
}

#[test]
fn a_slow_tick_is_dropped_not_queued() {
    let runtime = PanelRuntime::new();
    let start = Instant::now();
    assert_eq!(
        runtime.deliver_tick(PANEL, shown_and_focused(), start),
        Delivery::Delivered
    );
    // The call has not come back. Every tick due meanwhile is dropped, and counted, however
    // many fall due and however long the call takes.
    for tick in 1..=3 {
        let at = start + MIN_TICK_INTERVAL * tick * 2;
        assert_eq!(
            runtime.deliver_tick(PANEL, shown_and_focused(), at),
            Delivery::Skipped(Skipped::Busy)
        );
        assert_eq!(runtime.dropped_ticks(PANEL), u64::from(tick));
    }
    // Once it comes back, the next tick is delivered - one tick, not the three that were
    // dropped: they are gone, not queued behind the slow call.
    runtime.tick_finished(PANEL);
    let at = start + MIN_TICK_INTERVAL * 8;
    assert_eq!(
        runtime.deliver_tick(PANEL, shown_and_focused(), at),
        Delivery::Delivered
    );
    assert_eq!(runtime.dropped_ticks(PANEL), 3);
}

#[test]
fn tick_rate_is_capped_at_20_hz() {
    assert_eq!(MIN_TICK_INTERVAL, Duration::from_millis(50));
    let runtime = PanelRuntime::new();
    let start = Instant::now();
    assert_eq!(
        runtime.deliver_tick(PANEL, shown_and_focused(), start),
        Delivery::Delivered
    );
    runtime.tick_finished(PANEL);
    // A tick sooner than the interval is refused even though the plugin is idle, and is not a
    // missed tick: the renderer asked too early.
    let too_soon = start + MIN_TICK_INTERVAL - Duration::from_millis(1);
    assert_eq!(
        runtime.deliver_tick(PANEL, shown_and_focused(), too_soon),
        Delivery::Skipped(Skipped::TooSoon)
    );
    assert_eq!(runtime.dropped_ticks(PANEL), 0);
    // At the interval it is due again.
    assert_eq!(
        runtime.deliver_tick(PANEL, shown_and_focused(), start + MIN_TICK_INTERVAL),
        Delivery::Delivered
    );
    // Over a second, ticking as fast as the runtime allows, a panel takes 20 and no more.
    let runtime = PanelRuntime::new();
    let start = Instant::now();
    let mut delivered = 0;
    for step in 0..1000 {
        let at = start + Duration::from_millis(step);
        if runtime.deliver_tick(PANEL, shown_and_focused(), at) == Delivery::Delivered {
            delivered += 1;
            runtime.tick_finished(PANEL);
        }
    }
    assert_eq!(delivered, 20);
}

#[test]
fn reserved_keys_never_reach_the_plugin() {
    let runtime = PanelRuntime::new();
    // The host keeps Escape, which returns focus to the editor, and Tab, which moves it.
    for name in ["escape", "Escape", "ESCAPE", "tab", "Tab"] {
        assert_eq!(
            runtime.deliver_key(PANEL, &key(name), shown_and_focused()),
            Delivery::Skipped(Skipped::Reserved),
            "{name}"
        );
    }
    // Anything with the secondary modifier is the host's, whatever the key is.
    for name in ["a", "k", "f1", "enter"] {
        let secondary = KeyEvent {
            secondary: true,
            ..key(name)
        };
        assert_eq!(
            runtime.deliver_key(PANEL, &secondary, shown_and_focused()),
            Delivery::Skipped(Skipped::Reserved),
            "{name}"
        );
    }
    // Other keys, with the other modifiers, reach the plugin.
    for name in ["a", "k", "f1", "enter", "up", "space"] {
        assert_eq!(
            runtime.deliver_key(PANEL, &key(name), shown_and_focused()),
            Delivery::Delivered,
            "{name}"
        );
    }
    for modified in [
        KeyEvent {
            shift: true,
            ..key("a")
        },
        KeyEvent {
            ctrl: true,
            ..key("a")
        },
        KeyEvent {
            alt: true,
            ..key("a")
        },
    ] {
        assert_eq!(
            runtime.deliver_key(PANEL, &modified, shown_and_focused()),
            Delivery::Delivered,
            "{modified:?}"
        );
    }
    // Unfocused, no key reaches it at all, reserved or not.
    let unfocused = PanelState {
        shown: true,
        focused: false,
    };
    assert_eq!(
        runtime.deliver_key(PANEL, &key("a"), unfocused),
        Delivery::Skipped(Skipped::Unfocused)
    );
    assert_eq!(
        runtime.deliver_key(PANEL, &key("escape"), unfocused),
        Delivery::Skipped(Skipped::Unfocused)
    );
}

#[test]
fn each_call_still_has_the_100ms_deadline() {
    // A tick and a key are ordinary calls into the plugin: neither buys more time than any other
    // import, and the figure is kiln-plugin's, not a second copy of it here.
    assert_eq!(PanelRuntime::call_deadline(), Duration::from_millis(100));
    assert_eq!(
        PanelRuntime::call_deadline(),
        kiln_plugin::limits::CALL_DEADLINE
    );
    // Nothing here asserts a relationship between the deadline and MIN_TICK_INTERVAL. An earlier
    // draft did, and a mutation check showed why not to: changing the tick interval failed this
    // test as well as the one about the rate, which would send a reader to the wrong place.
    // `tick_rate_is_capped_at_20_hz` owns the interval; this test owns the deadline.
}
