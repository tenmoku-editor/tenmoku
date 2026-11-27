---
task_uid: "01m8cvnr4c97ft1hw62eqg2t8q"
task: "T2153"
rev: 1
type: "refactor"
complexity: "s"
criteria:
  - id: "C1"
    text: "kiln-ui `tests::empty_state` passes: `renders_icon_heading_and_hint_centred`, `heading_and_icon_are_optional`, `action_renders_below_the_hint`, `heading_uses_the_ui_heading_role_and_theme_text_colour` (kiln/crates/kiln-ui/src/tests/empty_state.rs)"
    required: true
    manual: false
  - id: "C2"
    text: "problems_panel.rs, runs_panel.rs, search_panel.rs and debug_panel.rs build their empty states with `kiln_ui::EmptyState` and keep their debug selectors; the existing fe-workbench tests that read them pass unchanged"
    required: true
    manual: false
  - id: "C3"
    text: "No other copy remains: `rg -n 'size\\(px\\(24\\.\\)\\)' crates/fe-workbench/src/problems_panel.rs crates/fe-workbench/src/runs_panel.rs crates/fe-workbench/src/search_panel.rs` and `rg -n 'fn render_empty' crates/fe-workbench/src` find nothing, and `rg -n 'text_center\\(\\)' crates/fe-workbench/src/debug_panel.rs` finds nothing"
    required: true
    manual: false
  - id: "C4"
    text: "Any screenshot golden that moved is regenerated and named in Implementation notes; none moves by more than the heading-weight difference of search_panel.rs"
    required: true
    manual: false
---

# kiln-ui EmptyState: one centred icon, heading and hint element for the hand-built empty states (reuse audit 9)

Source: reuse audit 2026-11-26 (`/Users/daryl/.local/share/fastedit-lead/plans/reuse/reuse-audit-2026-11-26.md`), § 9; the user approved acting on it. Every file:line below was re-checked on `main`.

# kiln-ui EmptyState

Today four panels hand-build the same centred block: a muted icon, a heading, a hint, and sometimes an action. Three are verbatim copies: `crates/fe-workbench/src/problems_panel.rs:1141` (`render_empty`), `runs_panel.rs:768` (`render_empty`) and `search_panel.rs:2214` (`render_empty`): flex-col, items/justify centre, `gap_2`, `px_4`, `text_center`, a 24 px `svg()` in `text.muted`, a heading in `UI_HEADING`, a muted hint. The fourth, `debug_panel.rs:888` (`empty`), is the same layout with a message in muted body text, no icon or heading, and an optional `Button`.
The copies already disagree: search_panel.rs uses `FontWeight::SEMIBOLD` and `UiTheme` fields where the others use `UI_HEADING.weight` and `cx.theme().colors`. One element ends that.

The audit also named `kiln-shell/src/dock.rs:1769` and `plugin_panel.rs:1799`: on main those are a heading text style (`DockView::heading`) and the plugin UI's `NodeKind::Heading`, not empty states, so they are not migrated. `notification_inbox_view.rs:823` is a left-aligned padded text row inside a popup list, a different layout: leave it, and say so in the notes.

## Files
- `kiln/crates/kiln-ui/src/empty_state.rs` (new), exported from `lib.rs`: `EmptyState::new()` with optional `.icon(Icon)`, `.heading(impl Into<SharedString>)`, `.hint(impl Into<SharedString>)`, `.action(impl IntoElement)` (rendered below the hint; repeatable), `.selector(&'static str)` (the `debug_selector`), `.track_focus(&FocusHandle)`. It is a `RenderOnce` element that fills its parent (`flex_1`, `min_h_0`, `size_full` as the callers do today), reads `cx.theme().colors.text.{primary,muted}`, `UI_HEADING` for the heading, `UI_BODY` for the hint, 24 px icon.
- `crates/fe-workbench/src/{problems_panel,runs_panel,search_panel,debug_panel}.rs`: build their empty states with it and keep their debug selectors (`EMPTY_SELECTOR`, `"search-empty"`).
- Tests: `kiln/crates/kiln-ui/src/tests/empty_state.rs`, declared in `tests/mod.rs`.

## Reuse
Searched kiln-ui for an existing empty element (none; the P7 row of `docs/decisions/25w38k498b-ui-polish-audit.md:630` planned this file and it was never written). Uses `UI_HEADING`/`UI_BODY` (roles.rs), `Icon`, `Button`. T1832 (provider-neutral chat empty state) is told to use this element instead of adding a fifth copy.

## Rules
Do not widen --scope; if a file outside it is needed, stop and respec (notes and choices go in your lane's `docs/decisions/<lane>-*.md`, which is exempt). New tests go in new files, one fn per behaviour, small fixtures, no sleeps for timing. Gate tests on the affected crates only. Behaviour of the migrated sites does not change. In `## Implementation notes` name the existing components you reused or extended, and record any copy you found but could not migrate with its file:line.
If a screenshot golden shifts (the search panel's heading weight is the likeliest), regenerate it and say which in the notes; the card is not `linux-ok` for that reason.

## Implementation notes

**Reused:** `UI_HEADING` and `UI_BODY` (`kiln/crates/kiln-ui/src/roles.rs`), `Icon`, `ActiveTheme`'s tokens, and `Button` for the debug panel's action. fe-workbench names the new element `fe_ui::EmptyState`: `fe-ui` is the workspace's name for kiln-ui.

**Look of the migrated sites** (all as the Files section directs):
- The Problems and Runs hints are now set in `UI_BODY` (13/18). Before, they inherited the panel's 12 px.
- The debug panel's message gains `UI_BODY`'s 18 px line height, and its block gains `px_4`, `min_h_0` and centred text, which only shows if a message wraps.
- The search panel's heading looks the same: `FontWeight::SEMIBOLD` is 600, as is `UI_HEADING.weight`, and `UiTheme`'s `text` and `muted_text` are `text.primary` and `text.muted` (`kiln-ui/src/theme.rs`).

`DebugPanel::empty` keeps its signature, with `cx` now unused, because `crates/fe-workbench/src/debug_panel/console.rs:684` calls it and is outside the scope. That console empty state is drawn by `EmptyState` through it.

**Screenshot goldens:** none moved. `cargo run -p fe-workbench --example screenshots` matched every golden; no golden shows one of these empty states.

**Not migrated**, each a left-aligned padded row of muted text, a different layout:
- `crates/fe-workbench/src/notification_inbox_view.rs:823`
- `crates/fe-workbench/src/settings_editor.rs:1276`
- `crates/fe-workbench/src/shortcuts_editor.rs:1785`
