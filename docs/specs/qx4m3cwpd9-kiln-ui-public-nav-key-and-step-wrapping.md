---
task_uid: "01m8cvnrv9napm8tqx4m3cwpd9"
task: "T2154"
rev: 2
type: "refactor"
complexity: "s"
criteria:
  - id: "C1"
    text: "kiln-ui `tests::nav_step` passes: `wraps_down_from_the_last_to_the_first`, `wraps_up_from_the_first_to_the_last`, `from_none_down_is_first_and_up_is_last`, `skips_disabled_entries_both_ways`, `none_when_nothing_is_enabled`, `a_single_enabled_entry_stays_put`"
    required: true
    manual: false
  - id: "C2"
    text: "`cargo nextest run -p kiln-ui --test nav_public` passes `nav_key_and_step_wrapping_are_public` (kiln/crates/kiln-ui/tests/nav_public.rs: builds a `KeyDownEvent` for down on row 1 of 3 and expects `NavKey::To(2)` through `kiln_ui::nav_key`, and calls `kiln_ui::step_wrapping`)"
    required: true
    manual: false
  - id: "C3"
    text: "The five callers use `kiln_ui::step_wrapping` and their existing tests pass unchanged (git branch and log menus, project panel menus, shortcuts editor, fe-gpui context menu)"
    required: true
    manual: false
  - id: "C4"
    text: "No other copy remains: `rg -n 'checked_sub\\(1\\)\\.unwrap_or\\(last\\)' crates kiln/crates` and `rg -n 'fn next_highlight\\(|fn step\\(at: Option\u003cusize\u003e' crates/fe-workbench/src crates/fe-gpui/src` find nothing (the second search is narrowed so unrelated pre-existing fn step( methods do not match; lead decision 11-27)"
    required: true
    manual: false
---

# kiln-ui: public nav_key and step_wrapping; the five wrap-around menu steppers use it (reuse audit 4a)

Source: reuse audit 2026-11-26 (`/Users/daryl/.local/share/fastedit-lead/plans/reuse/reuse-audit-2026-11-26.md`), § 4a and the nav.rs half of § 4c; the user approved acting on it. Every file:line below was re-checked on `main`.

# kiln-ui: public nav_key and step_wrapping, five steppers replaced

`kiln/crates/kiln-ui/src/nav.rs` has `nav_key` (up/down/home/end/page/enter for `List` and `Tree`), `NavKey` and `page_rows`, all `pub(crate)`, so fe-workbench could not use them if it found them. Meanwhile the same wrap-around highlight arithmetic (`(None, true) => 0, (None, false) => last, (Some(at), true) if at >= last => 0, ... checked_sub(1).unwrap_or(last)`) is written five times:
- `crates/fe-workbench/src/git_branch_menu.rs:1179` (`step`, called at :725 and :736)
- `crates/fe-workbench/src/git_log_menu.rs:208` (`move_menu_highlight`)
- `crates/fe-workbench/src/project_panel_menu.rs:206` (`next_highlight`; also called from `project_panel_new_menu.rs:258`)
- `crates/fe-workbench/src/shortcuts_editor.rs:576` (`move_highlight`)
- `crates/fe-gpui/src/view/context_menu.rs:610` (`next_highlight`, called at :446-447)
Not a copy: `settings_editor_controls.rs:869` `move_highlight` stops at the ends instead of wrapping. Leave it.

## Files
- `kiln/crates/kiln-ui/src/nav.rs`: make `NavKey`, `nav_key`, `page_rows` `pub`; add `pub fn step_wrapping(count: usize, from: Option<usize>, down: bool, enabled: impl Fn(usize) -> bool) -> Option<usize>`: the next enabled index in the direction, wrapping, skipping disabled ones; from `None`, the first (down) or last (up) enabled; `None` when nothing is enabled.
- `kiln/crates/kiln-ui/src/lib.rs`: `pub use nav::{NavKey, nav_key, page_rows, step_wrapping}` and a line in the crate docs naming them.
- The five callers above call it (their `is_item` / `enabled` / `disabled` filters become the predicate) and drop their own match.
- Tests: `kiln/crates/kiln-ui/src/tests/nav_step.rs` (declared in `tests/mod.rs`) and `kiln/crates/kiln-ui/tests/nav_public.rs` (an integration test, so it can only use the public API).

## Reuse
Extends kiln-ui's `nav.rs` rather than adding a helper in fe-workbench. The popup-menu state (the key switch and click-away around these steppers) is the next card, `PopupMenu`; the ~20 panels with their own `move_selection` are the one after.

## Rules
Do not widen --scope; if a file outside it is needed, stop and respec (notes and choices go in your lane's `docs/decisions/<lane>-*.md`, which is exempt). New tests go in new files, one fn per behaviour, small fixtures, no sleeps for timing. Gate tests on the affected crates only. Behaviour of the migrated sites does not change. In `## Implementation notes` name the existing components you reused or extended, and record any copy you found but could not migrate with its file:line.

## Implementation notes

- Extended kiln-ui's `nav.rs`: `step_wrapping` is new there; `NavKey`,
  `nav_key` and `page_rows` are now public.
- The fe-workbench and fe-gpui callers import it as `fe_ui::step_wrapping`:
  `fe-ui` is the workspace's name for the `kiln-ui` package, so no manifest
  changed.
- Each caller's highlight is only ever one of its choosable entries or none,
  so passing it straight to `step_wrapping` keeps what each did before.
- Rev 2 narrowed C4's second search: the first wording also matched
  `fn step(` methods that do other jobs and were there before this task.
- Other wrap-around steppers, written with `%`, outside this task's five and
  not migrated (a follow-up can judge each):
  `kiln/crates/kiln-shell/src/dock.rs:920` (a menu highlight),
  `crates/fe-workbench/src/ai_changes_panel.rs:254`,
  `crates/fe-workbench/src/tab_switcher.rs:159`,
  `crates/fe-workbench/src/toast.rs:797`,
  `kiln/crates/kiln-ui/src/prompt.rs:95`, and
  `crates/fe-editor/src/completion.rs:198` (fe-editor is GPUI-free, so it
  can't use kiln-ui).
