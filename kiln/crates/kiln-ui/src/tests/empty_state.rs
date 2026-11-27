// SPDX-License-Identifier: Apache-2.0

//! [`EmptyState`]: what it draws, where, and in which type and colour.

use gpui::{
    Bounds, Context, InteractiveElement as _, IntoElement, ParentElement as _, Pixels, Render,
    Styled as _, TestAppContext, VisualTestContext, Window, div, px, rgb,
};

use crate::{EmptyState, Icon, Theme, UI_HEADING, UiTheme, set_theme};

/// An empty state, built afresh on each draw, filling a 400 by 300 box.
struct Host(fn() -> EmptyState);

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .debug_selector(|| "host".into())
            .flex()
            .flex_col()
            .w(px(400.))
            .h(px(300.))
            .child((self.0)())
    }
}

fn draw(state: fn() -> EmptyState, cx: &mut TestAppContext) -> &mut VisualTestContext {
    let (_, cx) = cx.add_window_view(|_, _| Host(state));
    cx.run_until_parked();
    cx
}

fn bounds(selector: &'static str, cx: &mut VisualTestContext) -> Bounds<Pixels> {
    cx.debug_bounds(selector).expect(selector)
}

/// Asserts `parts` run top to bottom, each centred across the block, and
/// are centred down it as a group.
fn assert_centred_column(parts: &[&'static str], cx: &mut VisualTestContext) {
    let block = bounds("empty", cx);
    assert_eq!(block, bounds("host", cx), "it fills its parent");
    let parts: Vec<_> = parts.iter().map(|part| bounds(part, cx)).collect();
    for part in &parts {
        assert!(
            (part.center().x - block.center().x).abs() <= px(1.),
            "{part:?} off centre in {block:?}"
        );
    }
    for pair in parts.windows(2) {
        assert!(pair[0].bottom() < pair[1].top(), "{pair:?} out of order");
    }
    let above = parts[0].top() - block.top();
    let below = block.bottom() - parts[parts.len() - 1].bottom();
    assert!((above - below).abs() <= px(1.), "{above:?} {below:?}");
}

#[gpui::test]
fn renders_icon_heading_and_hint_centred(cx: &mut TestAppContext) {
    let cx = draw(
        || {
            EmptyState::new()
                .icon(Icon::Search)
                .heading("No results")
                .hint("Try another search")
                .selector("empty")
        },
        cx,
    );
    assert_centred_column(
        &[
            "empty-state-icon",
            "empty-state-heading",
            "empty-state-hint",
        ],
        cx,
    );
    let icon = bounds("empty-state-icon", cx);
    assert_eq!(icon.size, gpui::size(px(24.), px(24.)));
}

#[gpui::test]
fn heading_and_icon_are_optional(cx: &mut TestAppContext) {
    let cx = draw(
        || EmptyState::new().hint("Not debugging").selector("empty"),
        cx,
    );
    assert_centred_column(&["empty-state-hint"], cx);
    assert_eq!(cx.debug_bounds("empty-state-icon"), None);
    assert_eq!(cx.debug_bounds("empty-state-heading"), None);
}

#[gpui::test]
fn action_renders_below_the_hint(cx: &mut TestAppContext) {
    let cx = draw(
        || {
            let action = |selector: &'static str| {
                div()
                    .debug_selector(|| selector.into())
                    .w(px(60.))
                    .h(px(20.))
            };
            EmptyState::new()
                .action(action("first"))
                .hint("Not debugging")
                .action(action("second"))
                .selector("empty")
        },
        cx,
    );
    assert_centred_column(&["empty-state-hint", "first", "second"], cx);
}

#[gpui::test]
fn heading_uses_the_ui_heading_role_and_theme_text_colour(cx: &mut TestAppContext) {
    let theme = Theme::from_ui_theme(&UiTheme {
        text: rgb(0x123456).into(),
        ..UiTheme::default_dark()
    });
    cx.update(|cx| set_theme(cx, theme));
    cx.update(|cx| {
        let mut heading = EmptyState::heading_element("No results".into(), cx);
        let text = &heading.style().text;
        assert_eq!(text.font_size, Some(UI_HEADING.size.into()));
        assert_eq!(text.line_height, Some(UI_HEADING.line_height.into()));
        let weight = gpui::FontWeight(UI_HEADING.weight as f32);
        assert_eq!(text.font_weight, Some(weight));
        assert_eq!(text.color, Some(rgb(0x123456).into()));
    });
}
