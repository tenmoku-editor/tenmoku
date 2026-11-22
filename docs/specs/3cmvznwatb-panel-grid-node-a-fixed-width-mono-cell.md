---
task_uid: "01m81dwsyjq2myrc3cmvznwatb"
task: "T437"
rev: 2
type: "feature"
complexity: "s"
criteria:
  - id: "C1"
    text: "`node-kind` gains `grid(grid-node)`: `{ label: string, cols: u8, rows: u8, cells: list\u003ccell\u003e }` with `cell { ch: string (one grapheme), tone: tone, reverse: bool }`, row-major, `cells.len() == cols*rows` else `invalid-argument`; cols \u003c= 64, rows \u003c= 64; `label` is its accessible name (a non-empty, read as one image region, not per cell); a `grid-update` call replaces only changed rows (`set-grid-rows(panel, id, first-row, rows)`) so a 20x10 board costs less than a full view."
    required: true
    manual: false
  - id: "C2"
    text: "Limits: 4096 cells a grid, 4 grids a view, counted toward the view caps; tones come from the six existing tones only (no colours in WIT, so every theme styles it)."
    required: true
    manual: false
  - id: "C3"
    text: "Tests in crates/fe-plugin/tests/panel_grid.rs: `grid_with_matching_cells_is_stored`, `grid_size_mismatch_is_refused`, `oversize_grid_is_refused_keeping_the_last_view`, `row_update_changes_only_those_rows`, `multi_codepoint_grapheme_cell_is_one_cell`, `empty_label_is_refused`."
    required: true
    manual: false
  - id: "C4"
    text: "No GPUI code in this card; Linux-testable."
    required: true
    manual: false
---

# Panel grid node: a fixed-width mono cell grid with a tone per cell (game 2) (Mac T2006)

# Grid node

A game on a text surface: monospace cells, each a character and one of six theme tones. This reuses the panel pipeline and theming instead of a pixel surface. T1956 (blocked, a design for viewers) proposes a pixel surface for a PDF viewer; that is a different need (arbitrary pixels, big) and this card does not wait for it. If T1956's decision later subsumes the grid, a plugin can move to it; say so in the finish note.

Depends on GM1 only to serialize edits to panels.wit and panels.rs (same files), after T1705.


Read the files named before starting. Do not widen --scope; if a file outside it is needed, stop and respec. New tests go in new files.
