// SPDX-License-Identifier: Apache-2.0

//! [`step_wrapping`]: a menu's highlight, moved by Up and Down.

use crate::step_wrapping;

#[test]
fn wraps_down_from_the_last_to_the_first() {
    assert_eq!(step_wrapping(3, Some(2), true, |_| true), Some(0));
}

#[test]
fn wraps_up_from_the_first_to_the_last() {
    assert_eq!(step_wrapping(3, Some(0), false, |_| true), Some(2));
}

#[test]
fn from_none_down_is_first_and_up_is_last() {
    assert_eq!(step_wrapping(3, None, true, |_| true), Some(0));
    assert_eq!(step_wrapping(3, None, false, |_| true), Some(2));
}

#[test]
fn skips_disabled_entries_both_ways() {
    let enabled = |ix| [false, true, false, true, false][ix];
    assert_eq!(step_wrapping(5, Some(1), true, enabled), Some(3));
    assert_eq!(step_wrapping(5, Some(3), false, enabled), Some(1));
    // Around the disabled ends.
    assert_eq!(step_wrapping(5, Some(3), true, enabled), Some(1));
    assert_eq!(step_wrapping(5, Some(1), false, enabled), Some(3));
    // From none, and from a disabled entry.
    assert_eq!(step_wrapping(5, None, true, enabled), Some(1));
    assert_eq!(step_wrapping(5, None, false, enabled), Some(3));
    assert_eq!(step_wrapping(5, Some(2), true, enabled), Some(3));
    assert_eq!(step_wrapping(5, Some(2), false, enabled), Some(1));
}

#[test]
fn none_when_nothing_is_enabled() {
    for from in [None, Some(1)] {
        for down in [true, false] {
            assert_eq!(step_wrapping(3, from, down, |_| false), None);
            assert_eq!(step_wrapping(0, from, down, |_| true), None);
        }
    }
}

#[test]
fn a_single_enabled_entry_stays_put() {
    for down in [true, false] {
        assert_eq!(step_wrapping(3, Some(1), down, |ix| ix == 1), Some(1));
    }
}
