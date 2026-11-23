---
task_uid: "01m71szkq85jcj6hrddya1x2f5"
task: "T1708"
rev: 8
type: "feature"
complexity: "m"
criteria:
  - id: "C1"
    text: "`plugin_panel.rs` renders the view tree in a dock using kiln-ui: tones and icons come from the theme, lists and trees are virtualized, and user events go back to the plugin. Known limit (codex review yq01qn F13, follow-up T2102): a measured list whose rows all become tree items (drawn uniform) and then measured again reopens at its top row instead of keeping its scroll position; within one renderer, scroll is kept across new views."
    required: true
    manual: false
  - id: "C2"
    text: "AccessKit roles and labels are set for every node kind; keyboard navigation (arrows, Enter, Tab) works; tests in plugin_panel_tests.rs: `panel_nodes_have_roles_and_labels`, `panel_list_virtualizes_ten_thousand_rows`, `panel_keyboard_activation_sends_event`, `panel_theme_tones_follow_theme`."
    required: true
    manual: false
  - id: "C3"
    text: "A screenshot golden under crates/fe-workbench/examples/screenshots/golden/plugin_panel*.png is added through the screenshot suite and is stable on re-run."
    required: true
    manual: false
  - id: "C4"
    text: "Event delivery to the plugin is queue-capped by the host; excess events are dropped oldest-first and counted."
    required: true
    manual: false
---

# Render plugin panels in docks with kiln-ui: tones, virtual lists, events, a11y, golden (A4, 4.2)

# Render plugin panels in docks with kiln-ui: tones, virtual lists, events, a11y, golden (A4, 4.2)

Draft id A4 of the plugins stage 3 plan (docs/decisions/dqsjfernkt-plugins-stage-3.md, section 6). The design is approved by the user (2026-11-09); questions Q1 to Q7 follow the design's proposals.

Section 4.2. Mac-only: the screenshot golden needs this Mac. The glob in the scope is intentional (golden file names are generated).

Read the design sections named above before starting. Do not widen --scope; if a file outside it is needed, stop and say so.

## Implementation notes

- **Scope (lead-approved respecs).** `dock/mod.rs` doesn't exist: the dock is `src/dock.rs`.
  The screenshot suite's goldens live in `examples/screenshots/golden/`, so C3 names
  `plugin_panel_dark.png` and `plugin_panel_light.png` there, made by `plugin_panel_scene.rs`.
  `workbench_chrome.rs` was added for `dock = "bottom"`, and `plugin_host.rs` (after T1706)
  for the wiring. New strings are in `extensions.ftl`. After review round 1:
  `crates/fe-plugin/src/panels.rs` and `crates/fe-plugin/tests/panels.rs` for view revisions
  (F3), and `dock_leave.rs` for the dock focus commands (F7). After round 2:
  `kiln/crates/kiln-ui/src/text_field.rs` for `TextField::tab_stop` (F8).
- **Docks.** `dock = "left"` and `"right"` put the panel in the sidebar: the workbench has
  one, on the side `workbench.rail` gives it. `"bottom"` puts it in the bottom panel with a
  button at the rail's foot, after the bottom tool windows'. Plugin panels stay where the
  manifest puts them: they aren't moved by drag or by saved placements.
- **Rendering.** Lists and trees are virtualized: only the rows in view are built. One whose
  rows are all tree items is drawn by `uniform_list`, each row one row high; any other is
  drawn by GPUI's measured `list`, each row as tall as its content (wrapped text, a column, a
  nested list) and at least a row. A new view keeps a measured list where it was scrolled:
  the same rows are measured again in place, and other rows keep the top row by its node.
  Known limit (F13, T2102): a list that is drawn uniform and then measured again reopens at
  its top, because its measured rows are emptied so their focus is registered again.
  Each measured row has its own focus, so a row holding the focused control is still drawn
  once scrolled out of view, and keys still reach it. Inputs are tab stops (`TextField::tab_stop`). A panel logs
  each unknown icon name once, keeping at most 64 names of at most 64 bytes. A list holding tree items is a tree, and an expanded item's children follow it.
  A run of tree items outside a list is drawn the same way, as one tree (AccessKit name
  "Tree"), with the same clicks, keys and expansion; its `selected` events name the run by its
  first item's id. The plugin owns `expanded` and toggle values; the user's change shows at
  once and lasts until the plugin shows a view again, even an identical one: `PanelViews`
  numbers each shown view (`revision`, `view_with_revision`), so the plugin refusing a change
  by re-sending its view is seen. A list's cursor and its range anchor are node ids, so a new
  view that reorders or shrinks the list keeps them on their nodes or drops them. In a
  multi-select list, Shift with a move extends the selection from the anchor, the platform's
  command key with a move moves the cursor alone, and Space adds or removes the cursor's row.
  An input with focus takes none of the view's changes; once left, it takes a changed value,
  and a new label or placeholder makes the field again with its text, since a `TextField`
  can't change those. A `grid` node (T2006) is drawn as an empty region, named by its label,
  with role `Image`: drawing its cells is T2007's. Containers are AccessKit `Group`s with no name (GPUI rejects
  `GenericContainer`), and a separator is a `Splitter`.
- **Ten thousand rows.** A valid view holds at most 10,000 nodes and 5,000 children per node,
  so `panel_list_virtualizes_ten_thousand_rows` draws the largest list a view can hold:
  9,999 rows (two tree items of 4,999 and 4,998), the list being the 10,000th node.
- **Updates.** A panel reads its view from `PanelViews` every 50 ms (the update interval)
  while it shows, flushing a held update when it's due. Hidden, it only checks whether it
  shows, sending `shown` and `hidden`.
- **Events.** `PanelEvents` queues at most 256 events per plugin; past that it drops the
  oldest and counts it. Each push wakes the plugin's thread (`Work::Panels`), which hands up
  to 8 at a time to the optional `fastedit:editor/panel-handler@1.1.0#on-event` export. A
  plugin that doesn't export it keeps its events queued, capped. An input's text is sent
  150 ms after the last keystroke. A button's `command` runs only if it is one of the
  plugin's own commands.
- **Failed loads.** A load that fails after its panels slice is made (its `init` traps after
  setting a view, say) forgets the plugin's panel views and events, as it forgets its paint
  and formatting state.
- **Linker.** fe-workbench's plugin linker didn't link `fastedit:editor/panels` (T1705 linked
  it only in fe-plugin's own `add_to_linker`), so `set-view` failed to link. It is linked now.
- **Not done here.** `panels.reveal` still answers `unimplemented`: its host import is
  fe-plugin's (`src/panels.rs`), and its rule ("only while a command the user ran is being
  handled") needs the user-action marking A8 brings.
