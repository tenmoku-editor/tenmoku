---
task_uid: "01m8cvnsh99sd4fd8n2qghyxjh"
task: "T2155"
rev: 2
type: "refactor"
complexity: "m"
criteria:
  - id: "C1"
    text: "kiln-ui `tests::popup_menu` passes: `down_and_up_wrap_and_skip_separators_and_disabled_entries`, `enter_runs_the_highlighted_entry_and_closes`, `escape_closes_without_running`, `a_mouse_down_outside_closes`, `hover_moves_the_highlight`, `closing_returns_focus_to_the_opener`"
    required: true
    manual: false
  - id: "C2"
    text: "The four menus' existing tests (fe-gpui context menu, project panel menus, git log menu, git branch menu) pass unchanged"
    required: true
    manual: false
  - id: "C3"
    text: "No hand-built copy remains: `rg -n 'on_mouse_down_out' crates/fe-gpui/src/view/context_menu.rs crates/fe-workbench/src/project_panel_menu.rs crates/fe-workbench/src/project_panel_new_menu.rs crates/fe-workbench/src/git_log_menu.rs crates/fe-workbench/src/git_branch_menu.rs` finds nothing and none of them has its own `\"up\" | \"down\"` key switch"
    required: true
    manual: false
---

# kiln-ui PopupMenu: highlight, up/down/enter/escape and click-away dismissal in one entity; the four hand-built menus use it (reuse audit 4b)

Source: reuse audit 2026-11-26 (`/Users/daryl/.local/share/fastedit-lead/plans/reuse/reuse-audit-2026-11-26.md`), § 4b; the user approved acting on it. Every file:line below was re-checked on `main`.

# kiln-ui PopupMenu

`kiln-ui` has `MenuPanel`/`MenuEntry` (`kiln/crates/kiln-ui/src/menu_bar.rs:125,139`), which only renders. Highlight state, the up/down/enter/escape key switch, hover-to-highlight and `on_mouse_down_out` dismissal are re-implemented by hand in four menus, about 1,500 lines:
`crates/fe-gpui/src/view/context_menu.rs:446`, `crates/fe-workbench/src/project_panel_menu.rs:313-408`, `project_panel_new_menu.rs:242-351`, `git_log_menu.rs:310-343`, and `git_branch_menu.rs` (key handling beside `move_highlight` at :722).

## Files
- `kiln/crates/kiln-ui/src/popup_menu.rs` (new), exported from `lib.rs`: a `PopupMenu` entity wrapping `MenuPanel`: entries (separators, disabled), highlight (using `step_wrapping` from the card before this one), up/down/home/end/enter/escape, hover highlight, activation callback, dismissal on mouse-down outside and on escape, focus handed back to the opener on close.
- The four callers above drive it and drop their own state and key switch. Menu contents, labels, shortcuts and anchor positions stay with the callers.
- Tests: `kiln/crates/kiln-ui/src/tests/popup_menu.rs`.

## Notes
Focus and `on_mouse_down_out` ordering is the fiddly part: popup menu keys need a key context swapped in (the menu's keys must win over the host's bindings), and a click on the opener while open must close, not reopen. The existing tests of the four menus pin this; run them first and keep them passing unchanged. If the four do not fit one entity, build it for the three fe-workbench menus plus fe-gpui's, say so, and block for a respec rather than stretching scope.
The ~20 panels with their own `uniform_list` + `selected` + `move_selection` are not in this card (see the single-select panels card).

## Reuse
Builds on `MenuPanel` (render) and `step_wrapping` (stepping); adds the state both lacked.

## Rules
Do not widen --scope; if a file outside it is needed, stop and respec (notes and choices go in your lane's `docs/decisions/<lane>-*.md`, which is exempt). New tests go in new files, one fn per behaviour, small fixtures, no sleeps for timing. Gate tests on the affected crates only. Behaviour of the migrated sites does not change. In `## Implementation notes

- **Reused / extended:** `MenuPanel` and `MenuEntry` (kiln-ui `menu_bar.rs`) draw every `PopupMenu`, and `step_wrapping` (kiln-ui `nav.rs`) moves its highlight. fe-gpui's `MenuFocus`/`FocusReturn`/`key_context` (`crates/fe-gpui/src/view/menu_focus.rs`) moved down to kiln-ui as `MenuFocus`, `FocusReturn` and `menu_key_context`. `PopupMenu::take_focus` is built on them, and the breakpoint menu (`breakpoints.rs`) now imports them from `fe_ui`; `menu_focus.rs` is deleted (review F4, rev 2 scope). The branches-menu Shift test reuses `git_branch_tests.rs`'s `fixture`, `init`, `window` and `draw`.
- **Shape:** `PopupMenu` is state the host keeps in its own open-menu struct, not a separate gpui `Entity`. The four menus' tests read the highlight through `&self` accessors with no `App` (`menu_highlighted()`, `new_menu_highlighted()`, `editor_menu_highlighted()`, `highlighted_line()`, `actions()`), and C2 freezes those call shapes. The host passes `render`/`wire` a lens to the menu (`MenuOf<V>`) and a handler (`OnMenuInput<V>`), and gets back `MenuInput::{Handled, Chosen, Escape, PressedOutside}`. See `docs/decisions/8n2qghyxjh-popup-menu.md`.
- **Focus:** the editor menu takes its own focus (`take_focus`, key context `editor_menu` plus `HOLDS_KEYS`), as before. The project tree, New and Graph menus leave focus on their panel, because their tests pin `panel.focus_handle().is_focused()` while the menu is open. Their panel's key listener hands keys to `PopupMenu::key`: `on_menu_key` / `on_new_menu_key`, and for the Graph `git_log_menu.rs` `on_menu_key`, called from `git_log.rs` in place of its own switch (review F3, rev 2 scope). The branches menu keeps focus in its search field and hands keys to its rows' menu, or to the open branch's actions' menu.
- **Modifiers (review F2):** `MenuModifiers` keeps each menu's earlier policy. The project tree, New and Graph menus take no key held with a modifier (the default, `None`). The editor menu takes any key (`Any`). The branches menu ignores Shift on its rows, its actions and Left/Right (`Shift`).
- **Review F1:** a group added to the editor menu with no anchor item now puts its separator in the entries and in their `PopupMenu` alike. Before, it was added to the entries only, so the two were one entry apart and a click ran the item above. `context_menu_more_tests.rs` `an_unanchored_group_runs_its_own_items` catches it: with the old code, choosing Y ran `test.x`. That file has a small editor-in-a-window fixture of its own, because `context_menu_tests.rs`'s helpers are private to that module and C2 freezes it. Each fe-gpui test file has its own, as is the crate's custom.
- **Still hand-built, out of scope:** the breakpoint menu's own highlight, key handler and `on_mouse_down_out` (`crates/fe-gpui/src/view/breakpoints.rs:483,554-620`) could move onto `PopupMenu` too. Its focus is already the shared `MenuFocus`.
- **Small differences, noted:** the project tree, New and Graph menus gain Home/End (the spec's `PopupMenu` key set). The tree and New menus read their items' shortcuts from the keymap when they open, not on every draw.
