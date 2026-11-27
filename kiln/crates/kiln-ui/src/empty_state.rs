// SPDX-License-Identifier: Apache-2.0

//! [`EmptyState`]: what a panel shows in place of its content when it has
//! none.

use gpui::{
    AnyElement, App, Div, FocusHandle, InteractiveElement as _, IntoElement, ParentElement as _,
    Pixels, RenderOnce, SharedString, Styled as _, Window, div, prelude::FluentBuilder as _, px,
    svg,
};

use crate::{ActiveTheme as _, Icon, TextRole, UI_BODY, UI_HEADING};

/// The icon's size.
const ICON_SIZE: Pixels = px(24.);

/// A centred block filling its parent: an icon, a heading, a hint and any
/// actions, top to bottom, each optional. The icon and hint are in
/// `text.muted`, the hint in [`UI_BODY`]; the heading is in [`UI_HEADING`]
/// and `text.primary`.
///
/// ```ignore
/// EmptyState::new()
///     .icon(Icon::Search)
///     .heading(heading)
///     .hint(hint)
///     .selector("search-empty")
/// ```
#[derive(IntoElement, Default)]
pub struct EmptyState {
    icon: Option<Icon>,
    heading: Option<SharedString>,
    hint: Option<SharedString>,
    actions: Vec<AnyElement>,
    selector: Option<&'static str>,
    focus: Option<FocusHandle>,
}

impl EmptyState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn heading(mut self, heading: impl Into<SharedString>) -> Self {
        self.heading = Some(heading.into());
        self
    }

    pub fn hint(mut self, hint: impl Into<SharedString>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// Adds an element, such as a button, below the hint and any actions
    /// added before it.
    pub fn action(mut self, action: impl IntoElement) -> Self {
        self.actions.push(action.into_any_element());
        self
    }

    /// The block's `debug_selector`, by which a test finds it.
    pub fn selector(mut self, selector: &'static str) -> Self {
        self.selector = Some(selector);
        self
    }

    /// Makes the block `focus`'s element, for a panel whose focus has
    /// nowhere else to be while it's empty.
    pub fn track_focus(mut self, focus: &FocusHandle) -> Self {
        self.focus = Some(focus.clone());
        self
    }

    /// The heading's element.
    pub(crate) fn heading_element(heading: SharedString, cx: &App) -> Div {
        text(UI_HEADING)
            .debug_selector(|| "empty-state-heading".into())
            .font_weight(gpui::FontWeight(UI_HEADING.weight as f32))
            .text_color(cx.theme().colors.text.primary)
            .child(heading)
    }
}

/// A `div` set in `role`'s size and line height.
fn text(role: TextRole) -> Div {
    div().text_size(role.size).line_height(role.line_height)
}

impl RenderOnce for EmptyState {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let muted = cx.theme().colors.text.muted;
        div()
            .when_some(self.selector, |block, selector| {
                block.debug_selector(|| selector.into())
            })
            .when_some(self.focus, |block, focus| block.track_focus(&focus))
            .flex_1()
            .min_h_0()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_2()
            .px_4()
            .text_center()
            .when_some(self.icon, |block, icon| {
                block.child(
                    svg()
                        .debug_selector(|| "empty-state-icon".into())
                        .path(icon.path())
                        .size(ICON_SIZE)
                        .flex_none()
                        .text_color(muted),
                )
            })
            .when_some(self.heading, |block, heading| {
                block.child(Self::heading_element(heading, cx))
            })
            .when_some(self.hint, |block, hint| {
                block.child(
                    text(UI_BODY)
                        .debug_selector(|| "empty-state-hint".into())
                        .text_color(muted)
                        .child(hint),
                )
            })
            .children(self.actions)
    }
}
