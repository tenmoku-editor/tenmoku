---
task_uid: "01m82szbf8d68asmmm6vejvet0"
task: "T2102"
rev: 2
type: "bugfix"
complexity: "s"
criteria:
  - id: "C1"
    text: "A measured plugin panel list scrolled to a row and pixel offset, shown as all tree items (uniform) and then measured again with the same row ids, reopens at the same top row id and pixel offset; a test in plugin_panel_tests.rs scrolls before switching and checks both."
    required: true
    manual: false
  - id: "C2"
    text: "Rows holding focus still keep keyboard dispatch out of view after the switch (typing, Enter, Tab), as T1708's a_focused_input_scrolled_out_of_a_list_still_takes_keys checks."
    required: true
    manual: false
---

# Plugin panel lists keep their scroll across a uniform/measured renderer switch (T1708 F13 follow-up)

Follow-up to T1708 (A4, 4.2), carrying codex review yq01qn F13 (round 3).

In `crates/fe-workbench/src/plugin_panel.rs`, a plugin panel list is drawn with
`uniform_list` while every row is a tree item, and with the measured `list()` otherwise.
When a list is drawn uniform, `flatten` empties the measured list
(`measured.splice(0..count, 0)`), so that going back to measured rows registers every row's
focus handle again (T1708 r4 F12). Emptying the list also resets its logical scroll top.
A list that goes measured → uniform → measured with the same row ids therefore reopens at
row 0, offset 0 instead of where the user had scrolled it.

Fix: before emptying the measured list, keep its top row's node id and pixel offset
(`logical_scroll_top`), and restore both (`scroll_to(ListOffset)`) once the measured rows
come back, the same way a changed view keeps its top row by node id. Alternatively, rebuild
the focus registrations on the switch back without emptying the list.

Test: in plugin_panel_tests.rs, scroll a measured list well down to a non-zero pixel offset.
Then publish a view whose rows are all tree items, then the measured rows again with the
same ids. Check that the top row id and the pixel offset are restored, and that a focused
input scrolled out of view still takes typing, Enter and Tab.
