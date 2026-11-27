---
task_uid: "01m8e8hk532ddw30tgcpy0nxyh"
task: "T758"
rev: 1
type: "refactor"
complexity: "m"
criteria:
  - id: "C1"
    text: "`cargo nextest run -p kiln-shell key_hints::` passes: `label_is_the_first_binding_in_the_stack`, `single_skips_chords`, `a_binding_outside_the_stack_is_not_found`, `an_unknown_command_has_no_label`, `the_label_follows_a_keymap_reload` (kiln/crates/kiln-shell/tests/key_hints.rs)"
    required: true
    manual: false
  - id: "C2"
    text: "The eight callers use `KeyHints` and their existing tests pass unchanged (command palette, dock tooltips, debug toolbar, git panel, pane toolbar, problems detail, setup wizard, diff view)"
    required: true
    manual: false
  - id: "C3"
    text: "No other copy remains: `rg -n 'shortcuts\\(\u0026keymap' crates kiln/crates` finds only `kiln/crates/kiln-shell/src/key_hints.rs`, `crates/fe-workbench/src/pane_toolbar.rs` (`menu_shortcut`) and `crates/fe-workbench/src/terminal_panel.rs`"
    required: true
    manual: false
---

# kiln-shell KeyHints: a command's shortcut label from the live keymap in a context stack, in one place; nine key_label copies use it (reuse audit 7) (Mac T2164)

Source: reuse audit 2026-11-26 (`/Users/daryl/.local/share/fastedit-lead/plans/reuse/reuse-audit-2026-11-26.md`), § 7 (key_label clones); the user approved acting on it. Every file:line below was re-checked on `main`.

# kiln-shell KeyHints

"The first (or first single) key that runs `command` in a given context stack, as this platform prints it, read from the keymap now" is rebuilt each time as `CommandId::parse` + `ContextStack` + `current_keymap` + `shortcuts(..)` + `.find(single)`/`.first()` + `shortcut_label`. On main:
`crates/fe-gpui/src/diff_view.rs:813` (`key_label`), `crates/fe-workbench/src/debug_toolbar.rs:186` (`key`, stack with the debugger's flags), `git_panel.rs:2820` (`commit_key`), `pane_toolbar.rs:121` (`shortcut`), `problems_detail.rs:1009`, `setup_wizard.rs:1286` (`shortcut`), `kiln/crates/kiln-shell/src/dock.rs:1487` (`shortcut`) and `command_palette.rs:175` (`key_label`, memoised, requires a binding). They differ only in the context frames and first-vs-first-single-key.
Two more stay for now: `pane_toolbar.rs:136` `menu_shortcut` ranks bindings by context depth and modifier count (a different algorithm: leave it and note it), and `terminal_panel.rs:1590` (T1813 and T2061 hold the file; a one-line follow-up once they land).

## Files
- `kiln/crates/kiln-shell/src/key_hints.rs` (new), exported from `lib.rs`: `KeyHints::new(stack: ContextStack)`, `KeyHints::in_contexts(&[&str])`, `.label(command, cx) -> Option<String>` (first key sequence) and `.single(command, cx) -> Option<String>` (first single-keystroke binding). Reads `kiln_platform::keys::current_keymap(cx)` each call so it follows reloads; `CommandId::parse` failure is `None`.
- The eight callers above use it; their context stacks stay with them.
- Tests: `kiln/crates/kiln-shell/tests/key_hints.rs` (kiln-shell's integration tests are one binary listed by build.rs: a new file needs no list edit).

## Reuse
Extends kiln-shell (it owns keymap + panes) instead of a ninth private copy; uses `kiln_core::{shortcuts, shortcut_label}`. The tooltip half (`tooltip_text`) is the next card.

## Rules
Do not widen --scope; if a file outside it is needed, stop and respec (notes and choices go in your lane's `docs/decisions/<lane>-*.md`, which is exempt). New tests go in new files, one fn per behaviour, small fixtures, no sleeps for timing. Gate tests on the affected crates only. Behaviour of the migrated sites does not change. In `## Implementation notes` name the existing components you reused or extended, and record any copy you found but could not migrate with its file:line.

## Implementation notes

- New `kiln_shell::KeyHints` (`kiln/crates/kiln-shell/src/key_hints.rs`), extending kiln-shell and reusing `kiln_core::{shortcuts, shortcut_label}`, `kiln_command::{CommandId, ContextFrame, ContextStack}` and `kiln_platform::keys::current_keymap` (which `fe_gpui::current_keymap` and `fe_keymap::{shortcuts, shortcut_label}` already were). `KeyHints::new(stack)`, `KeyHints::in_contexts(&[S: AsRef<str>])` (so the dock passes its `SharedString`s), `.stack()`, `.label(command, cx)` and `.single(command, cx)`.
- The command palette's "no binding, skip `shortcuts`" check moved into `KeyHints` for every caller: `shortcuts` copies the keymap per call, and an unbound command gives `None` either way, so no result changes. The palette keeps its memo and now holds a `KeyHints` in place of its `ContextStack` (`context_stack()` reads it back).
- The dock keeps its own "no shortcut contexts, no key" check: an empty stack would match context-free bindings.
- Migrated: `diff_view.rs` `key_label` (single), `debug_toolbar.rs` `key` (single, flagged frame), `git_panel.rs` `commit_key` (single), `pane_toolbar.rs` `shortcut` (single), `problems_detail.rs` `apply_key` (single), `setup_wizard.rs` `shortcut` (label), `dock.rs` `shortcut` (label), `command_palette.rs` `key_label` (label).
- Not migrated: `crates/fe-workbench/src/pane_toolbar.rs:128` `menu_shortcut` (ranks by context depth and modifier count, a different algorithm) and `crates/fe-workbench/src/terminal_panel.rs:1590` `header_shortcut` (outside scope while T1813/T2061 hold the file: a one-line `KeyHints::new(pane_toolbar::editor_context()).label(command, cx)` follow-up).
- C3's rg also hits test code that calls `shortcuts` directly to check it or to compute an expected label: `crates/fe-keymap/tests/close_folder_key.rs:105`, `crates/fe-keymap/tests/menu.rs:132`, `crates/fe-workbench/src/tests/changes_b3.rs:178`. They were there at base, aren't helper copies, and are outside scope.
