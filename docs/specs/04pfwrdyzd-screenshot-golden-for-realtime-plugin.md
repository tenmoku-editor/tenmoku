---
task_uid: "01m83e5b2d69k15q04pfwrdyzd"
task: "T2103"
rev: 1
type: "chore"
complexity: "s"
criteria:
  - id: "C1"
    text: "plugin_panel_scene.rs gains a second scene pair, plugin_panel_grid_dark (themes::DEFAULT_DARK, Yohen) and plugin_panel_grid_light (themes::DEFAULT_LIGHT, Hakuji), through the existing capture_with/goldens().check path; no change to plugin_panel.rs, plugin_panel_grid.rs, main.rs or scenes.rs (if focusing the panel needs one, stop and respec)."
    required: true
    manual: false
  - id: "C2"
    text: "The scene's panel decl has keys = true and the panel is shown and focused so the focus ring is drawn; its view is one grid with all 6 tones (TONES in plugin_panel_grid.rs) once plain and once reversed, plus a fitted wide grapheme (a CJK or emoji cell) and an e + combining acute cell."
    required: true
    manual: false
  - id: "C3"
    text: "Goldens crates/fe-workbench/examples/screenshots/golden/plugin_panel_grid_dark.png and plugin_panel_grid_light.png are added on this Mac with UPDATE_SCREENSHOTS=1 cargo run -p fe-workbench --example screenshots, and a second plain run (cargo run -p fe-workbench --example screenshots) matches every scene, existing plugin_panel_dark/light included, unchanged."
    required: true
    manual: false
  - id: "C4"
    text: "The two existing goldens plugin_panel_dark.png and plugin_panel_light.png are byte-identical to main's."
    required: true
    manual: false
  - id: "C5"
    text: "Finish note says what the goldens show (reversed grounds, wide grapheme fitted in its cell, ring) after looking at both PNGs; no new Rust unit tests are needed (the colour/position/size tests are in plugin_panel_realtime_tests.rs from T2007)."
    required: true
    manual: false
---

# Screenshot golden for realtime plugin panels: a grid with every tone plain and reversed, wide graphemes and a focused ring, in Yohen and Hakuji (T2007 follow-up)
