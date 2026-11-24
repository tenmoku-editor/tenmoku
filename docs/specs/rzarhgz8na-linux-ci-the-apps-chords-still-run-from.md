---
task_uid: "01m84vp8r806sgvvrzarhgz8na"
task: "T2112"
rev: 1
type: "chore"
complexity: "xs"
criteria:
  - id: "C1"
    text: "plugin_panel::plugin_panel_realtime_tests::the_apps_chords_still_run_from_a_focused_realtime_panel passes on Linux (evidence: a Linux nextest run of it) and on macOS; the fix installs the host keymap via fe_gpui::set_user_keymap as crates/fe-workbench/src/path_commands_tests.rs:292-299 does"
    required: true
    manual: false
---

# Linux CI: the_apps_chords_still_run_from_a_focused_realtime_panel installs the host's keymap (DEFAULT_LINUX off macOS) before pressing secondary chords; fe_gpui::init's DEFAULT_MACOS binds cmd-k cmd-t (T2007 follow-up)

Main fails this T2007 test on Linux CI since b9bcb459b (run 45120415469; again in PR 12's run 45234489214, :1113 theme_picker_open). Test bug: crate::tests::open calls fe_gpui::init, which installs fe_keymap::DEFAULT_MACOS on every host (crates/fe-gpui/src/keymap.rs:68-71), so the keymap binds cmd-k cmd-t while the test presses secondary-k, which is ctrl-k off macOS. The product path is fine on Linux (kiln keys_plain::the_secondary_modifier_and_its_chords_stay_the_apps passes there). Fix only the test. Blocks T2098's CI evidence run.
