---
task_uid: "01m7th39mrq5w6r6cen602e3zt"
task: "T2009"
rev: 4
type: "feature"
complexity: "m"
criteria:
  - id: "C1"
    text: "extensions/plugins/blocks builds as a wasm32-wasip2 component (own Cargo.toml, outside the default workspace members like the template) and declares only `panels`, `panels.realtime`, `commands`; no fs, net or process. A `registry.toml` marks it optional for GM4; it is not listed in any bundle, resource or default-enabled list."
    required: true
    manual: false
  - id: "C2"
    text: "Gameplay: ten-wide by twenty-high board drawn with the grid node (one letter per piece kind, six tones plus muted for the ghost piece), seven pieces from a shuffled bag, rotate, soft and hard drop, line clears, level speed-up from a 500 ms tick down to 80 ms, score and next piece beside the board as text nodes, pause, game over. Keys: arrows or h/j/k/l, space = hard drop, p pause, r restart, Escape = leave (host). A start screen (when the panel opens and after game over) names the start key and the controls, and a one-line controls hint stays beside the board while playing. The best score is kept in plugin data (T1702) when available, else not kept."
    required: true
    manual: false
  - id: "C3"
    text: "Pure game logic lives in a `blocks-core` module with native `cargo test` tests (new file extensions/plugins/blocks/tests/logic.rs): `bag_has_each_piece_once_per_seven`, `rotation_kicks_off_the_wall`, `clearing_four_lines_scores_the_tetris_bonus`, `hard_drop_lands_on_the_ghost`, `top_out_is_game_over`, `same_seed_same_game`."
    required: true
    manual: false
  - id: "C4"
    text: "crates/fe-workbench/tests/first_party_blocks.rs (as T1713's first_party_cargo.rs does): loads the built component in the headless host, opens its panel, feeds ticks and keys through GM1's runtime, and asserts: `blocks_panel_view_is_valid`, `a_tick_moves_the_piece_down_one_row`, `hard_drop_then_ticks_ends_in_a_stable_board`, `a_tick_stays_under_the_call_deadline`, `no_ticks_when_hidden`, `the_start_screen_shows_the_controls_hint`."
    required: true
    manual: false
  - id: "C5"
    text: "Installable from the registry fixture index through T1709's verified install path; a fresh default install has no Blocks command in the palette (test: the command catalog does not list `tenmoku.blocks.*` until installed)."
    required: true
    manual: false
  - id: "C6"
    text: "Easter egg with hints (user decision 2026-11-20): the command is `Blocks: Play` (palette). No UI nag or promotion in the app outside the plugin; the Extensions view lists it under a normal entry. The registry and Extensions description (\"a falling-blocks game started with Blocks: Play\") is T2171's, a known limit here. The finish note says how the author tested it by hand if the app was run."
    required: true
    manual: false
  - id: "C7"
    text: "Review F1: the host implements `fastedit:editor/panels.reveal` (crates/fe-plugin/src/panels.rs, no longer Unimplemented) for a granted plugin's own panel: the workbench shows the panel's dock and focuses the panel, so keys reach it. `Blocks: Play` reveals and focuses the Blocks panel, from both a hidden and a never-opened panel. Revealing another plugin's panel or an undeclared one returns an error. Tests: an fe-plugin test of the grant and ownership checks, and a fe-workbench test (crates/fe-workbench/tests/plugin_panel_reveal.rs or first_party_blocks.rs; src/*_tests.rs is held by T2123) that a plugin command calling reveal shows and focuses the panel."
    required: true
    manual: false
---

# Blocks: a falling-blocks game as an optional wasm plugin (game 5)

# Blocks: the game plugin

The user: "a plugin that was a game, not Pong but something like that, a game you can play in the editor. Not installed by default, keeps people entertained, and proves out the plugin technology."

## Why falling blocks
It is turn-timed (gravity ticks of 80-500 ms, under the 20 Hz update cap and 100 ms call deadline), needs only arrow keys, fits a 10x20 character grid with six theme tones (no pixels or sprites), has real game logic worth testing natively, and is instantly recognisable. Alternatives rejected: Snake (too thin to prove much), a roguelike in a buffer (large content), type-racer on the user's own code (needs buffer reads and typing capture, which the panel key model deliberately excludes). The name avoids a trademarked one.

## What it proves
Sandboxed wasm with a frame tick, host-routed key input, the declarative panel pipeline, optional install from the signed registry, and per-plugin data. Discoverability (user decision 2026-11-20): with hints, not pure discovery. Once installed, the game says how to play: its registry and Extensions entry describe it as a falling-blocks game started with `Blocks: Play`, the panel opens on a start screen naming the start key and the controls, and a short controls line stays beside the board while playing. Still no nag or promotion outside the plugin.

Depends on GM3 (render, key focus, ticks), GM4 (publishing path) and transitively T1705, T1708, T1709. Licence of the plugin: Apache-2.0 like the app; no third-party assets.


Read the files named before starting. Do not widen --scope; if a file outside it is needed, stop and respec. New tests go in new files.
