//! Realtime plugin panels ([`super`], T2007): a grid drawn a row of text a
//! row in the editor's font, keys and Escape at a focused panel with
//! `keys`, the tick timer of a panel with `tick_ms` following focus and
//! showing, and grid tones holding their contrast in every bundled theme.

use std::time::{Duration, Instant};

use fe_command::capability::{Capability, Grants};
use fe_config::themes;
use fe_plugin::fastedit::editor::panels::{Node, NodeKind};
use fe_plugin::panels::{Cell, GridNode, State};
use fe_ui::{GlobalTheme, Role, Theme, ThemeData};
use gpui::{
    AppContext as _, Entity, Focusable as _, Hsla, KeyDownEvent, KeyUpEvent, Keystroke, Modifiers,
    TestAppContext, VisualTestContext,
};
use kiln::core::theme::{Contrast, contrast_ratio, over};
use kiln_plugin::manifest::{PanelDecl, PanelLocation};
use kiln_shell::DockView as _;

use super::grid::{GridTones, TONES, tone_floor, tone_key};
use super::{
    KeyEvent, MIN_UPDATE_INTERVAL, PanelEvent, PanelEvents, PanelViews, PluginPanel, QueuedEvent,
    Tone, View, element_id, panel_id, selector,
};

const PLUGIN: &str = "acme";
const PANEL: &str = "board";
/// The panel's tick, in milliseconds.
const TICK_MS: u32 = 100;
const TICK: Duration = Duration::from_millis(TICK_MS as u64);

/// A game panel: keys, and a tick every [`TICK_MS`].
fn decl() -> PanelDecl {
    PanelDecl {
        id: PANEL.into(),
        title: "Board".into(),
        icon: "list-checks".into(),
        location: PanelLocation::Left,
        tick_ms: Some(TICK_MS),
        keys: true,
    }
}

fn slice(views: &PanelViews) -> State {
    let grants: Grants = [Capability::Panels].into_iter().collect();
    State::new(PLUGIN, &grants, &[PANEL.to_owned()], views)
}

fn n(id: &str, kind: NodeKind, children: &[u32]) -> Node {
    Node {
        id: id.into(),
        kind,
        children: children.to_vec(),
    }
}

fn cell(ch: &str, tone: Tone, reverse: bool) -> Cell {
    Cell {
        ch: ch.into(),
        tone,
        reverse,
    }
}

/// A view of one grid, `board`, under a column.
fn grid_view(label: &str, cols: u8, cells: Vec<Cell>) -> View {
    let rows = u8::try_from(cells.len() / usize::from(cols)).unwrap();
    View {
        nodes: vec![
            n("root", NodeKind::Column, &[1]),
            n(
                "board",
                NodeKind::Grid(GridNode {
                    label: label.into(),
                    cols,
                    rows,
                    cells,
                }),
                &[],
            ),
        ],
    }
}

/// A 4 x 3 board: a wall row, a row with a player, a reversed cursor and a
/// multi-codepoint grapheme, and a muted floor.
fn board() -> View {
    let mut cells = Vec::new();
    cells.extend((0..4).map(|_| cell("#", Tone::Normal, false)));
    cells.push(cell("@", Tone::Accent, false));
    cells.push(cell("x", Tone::Error, true));
    // e and a combining acute: one grapheme, one cell.
    cells.push(cell("e\u{301}", Tone::Success, false));
    cells.push(cell("$", Tone::Warning, false));
    cells.extend((0..4).map(|_| cell(".", Tone::Muted, false)));
    grid_view("Board, 4 by 3", 4, cells)
}

fn use_theme(name: &str, cx: &mut VisualTestContext) {
    let data = ThemeData::from_file(&themes::find(name).unwrap().file());
    cx.update(|_, cx| fe_ui::set_theme(cx, Theme::new(data)));
}

fn draw(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.run_until_parked();
}

/// A panel declared as `decl` drawing `view`, alone in an active window,
/// shown.
fn panel_with(
    decl: PanelDecl,
    view: View,
    cx: &mut TestAppContext,
) -> (Entity<PluginPanel>, PanelEvents, &mut VisualTestContext) {
    cx.update(|cx| {
        fe_gpui::init(cx);
        crate::init(cx);
    });
    let views = PanelViews::new();
    let events = PanelEvents::new();
    let state = slice(&views);
    state.set_view_at(PANEL, view, Instant::now()).unwrap();
    let (panel, cx) = cx.add_window_view({
        let (views, events) = (views.clone(), events.clone());
        move |_, cx| PluginPanel::new(PLUGIN, &decl, views, events, cx)
    });
    use_theme(themes::DEFAULT_DARK, cx);
    cx.update(|window, _| window.activate_window());
    panel.update(cx, |panel, cx| panel.set_shown(true, cx));
    events.take(PLUGIN, usize::MAX);
    drop(state);
    draw(cx);
    (panel, events, cx)
}

/// A workbench on an empty folder with the panel `decl` in its sidebar,
/// drawing `view`, in an active window.
fn in_workbench(
    view: View,
    cx: &mut TestAppContext,
) -> (
    Entity<crate::Workbench>,
    Entity<PluginPanel>,
    PanelEvents,
    &mut VisualTestContext,
    tempfile::TempDir,
) {
    let dir = tempfile::tempdir().unwrap();
    let launch = crate::Launch {
        root: Some(dir.path().to_owned()),
        file: None,
    };
    let (workbench, cx, _) = crate::tests::open(cx, launch);
    let views = PanelViews::new();
    let events = PanelEvents::new();
    slice(&views)
        .set_view_at(PANEL, view, Instant::now())
        .unwrap();
    let panel = cx.update(|_, cx| {
        cx.new(|cx| PluginPanel::new(PLUGIN, &decl(), views.clone(), events.clone(), cx))
    });
    workbench.update(cx, |workbench, cx| {
        workbench.add_plugin_panel(panel.clone(), cx)
    });
    cx.update(|window, _| window.activate_window());
    draw(cx);
    (workbench, panel, events, cx, dir)
}

/// Shows the panel by its rail button, as its command does, and lets the
/// panel's poll note it.
fn reveal(workbench: &Entity<crate::Workbench>, cx: &mut VisualTestContext) {
    let dock = workbench.read_with(cx, |workbench, _| workbench.dock().clone());
    let id = panel_id(PLUGIN, PANEL);
    dock.update_in(cx, |dock, window, cx| dock.rail_clicked(id, window, cx));
    cx.executor().advance_clock(MIN_UPDATE_INTERVAL);
    draw(cx);
}

fn drain(events: &PanelEvents) -> Vec<PanelEvent> {
    (events.take(PLUGIN, usize::MAX).into_iter())
        .map(|QueuedEvent { panel, event }| {
            assert_eq!(panel, PANEL);
            event
        })
        .collect()
}

/// The ticks queued, each handed to the plugin as the host does: taken,
/// then said to be back.
fn ticks_delivered(events: &PanelEvents) -> Vec<u32> {
    let mut ticks = Vec::new();
    for QueuedEvent { panel, event } in events.take(PLUGIN, usize::MAX) {
        if let PanelEvent::Tick(ms) = event {
            ticks.push(ms);
        }
        events.delivered(PLUGIN, &panel, &event);
    }
    ticks
}

/// Lets `time` pass on the test clock, a tick at a time, drawing between.
fn wait(time: Duration, cx: &mut VisualTestContext) {
    let mut left = time;
    while !left.is_zero() {
        let step = left.min(TICK);
        cx.executor().advance_clock(step);
        draw(cx);
        left -= step;
    }
}

/// `key` pressed and released, as the keyboard sends it.
fn press(key: &str, cx: &mut VisualTestContext) {
    let keystroke = Keystroke::parse(key).unwrap();
    cx.update(|window, cx| {
        window.dispatch_event(
            gpui::PlatformInput::KeyDown(KeyDownEvent {
                keystroke: keystroke.clone(),
                is_held: false,
                prefer_character_input: false,
            }),
            cx,
        );
        window.dispatch_event(gpui::PlatformInput::KeyUp(KeyUpEvent { keystroke }), cx);
    });
    draw(cx);
}

fn key(name: &str, shift: bool, ctrl: bool) -> PanelEvent {
    PanelEvent::Key(KeyEvent {
        key: name.into(),
        shift,
        ctrl,
        alt: false,
        secondary: false,
    })
}

fn focused_in(focus: &gpui::FocusHandle, cx: &mut VisualTestContext) -> bool {
    cx.update(|window, cx| focus.contains_focused(window, cx))
}

fn has_ring(cx: &mut VisualTestContext) -> bool {
    cx.debug_bounds("plugin-panel-focus-ring").is_some()
}

#[gpui::test]
fn grid_rows_render_in_the_editor_font(cx: &mut TestAppContext) {
    let (workbench, panel, _events, cx, _dir) = in_workbench(board(), cx);
    reveal(&workbench, cx);
    let theme = workbench.read_with(cx, |workbench, cx| {
        workbench.pane().read(cx).theme().clone()
    });
    let drawn = panel.read_with(cx, |panel, _| panel.drawn_grids["board"].clone());
    // The editor's family and size, and its line.
    assert_eq!(drawn.family, theme.font_family);
    assert_eq!(drawn.font_size, theme.font_size);
    let line = (theme.font_size * theme.line_height).round();
    let advance = cx.update(|window, _| {
        let text = window.text_system();
        let font = text.resolve_font(&theme.font());
        text.advance(font, theme.font_size, 'm').unwrap().width
    });
    assert_eq!(drawn.cell, gpui::size(advance, line));
    // One text a row, its cells' graphemes in order, the combining one a
    // single cell.
    assert_eq!(drawn.rows, ["####", "@xe\u{301}$", "...."]);
    // Each cell in its tone; the reversed one the panel's colour on its
    // tone.
    let c = cx.update(|_, cx| GridTones::of(fe_ui::ActiveTheme::theme(cx)));
    let colours = |row: usize| -> Vec<(Hsla, Option<Hsla>)> {
        (drawn.cells[row].iter())
            .map(|cell| (cell.fore, cell.back))
            .collect()
    };
    assert_eq!(colours(0), [(c.tone(Tone::Normal), None); 4]);
    assert_eq!(colours(2), [(c.tone(Tone::Muted), None); 4]);
    assert_eq!(
        colours(1),
        [
            (c.tone(Tone::Accent), None),
            (c.ground, Some(c.tone(Tone::Error))),
            (c.tone(Tone::Success), None),
            (c.tone(Tone::Warning), None),
        ]
    );
    // Every cell's glyphs inside its own column, at the editor's size.
    assert_cells_in_place(&drawn);
    // (A grapheme the font shapes wider than the cell is fitted smaller: the
    // test shaper gives e and its combining accent an advance each.)
    for cell in (drawn.cells.iter().flatten()).filter(|cell| cell.text.is_ascii()) {
        assert_eq!(cell.font_size, theme.font_size, "{:?}", cell.text);
    }
    // The board is the cells' size, its width in whole pixels.
    let bounds = cx
        .debug_bounds(selector("board").leak())
        .expect("the grid drawn");
    assert_eq!(bounds.size, gpui::size((advance * 4.).ceil(), line * 3.));
    // One region named by its label, no node a row or a cell: the panel's
    // nodes are its column and the grid.
    cx.update(|_, cx| cx.refresh_windows());
    draw(cx);
    let nodes = cx.update(|window, cx| fe_ui::nodes(window, cx));
    let grid: Vec<_> = (nodes.iter())
        .filter(|node| node.id == element_id("board"))
        .collect();
    assert_eq!(grid.len(), 1);
    assert_eq!(
        (grid[0].role, grid[0].label.to_string()),
        (Role::Image, "Board, 4 by 3".into())
    );
    for cell in ["#", "@", "x", "$", ".", "####"] {
        assert!(
            !nodes.iter().any(|node| node.label.as_ref() == cell),
            "a node for {cell:?}"
        );
    }
    let ids = [element_id("root"), element_id("board")];
    let plugin_nodes = (nodes.iter())
        .filter(|node| matches!(&node.id, gpui::ElementId::Name(name) if name.starts_with("plugin-node:")))
        .count();
    assert_eq!(plugin_nodes, ids.len());
}

/// Asserts that each of `drawn`'s cells paints inside its own column: its
/// glyphs start at or after the column's left edge and end by its right,
/// whatever came before it in the row.
fn assert_cells_in_place(drawn: &super::grid::DrawnGrid) {
    let advance = drawn.cell.width;
    let slack = gpui::px(0.01);
    for (row, cells) in drawn.cells.iter().enumerate() {
        for (col, cell) in cells.iter().enumerate() {
            let left = advance * col as f32;
            let at = format!("row {row} col {col} {:?}", cell.text);
            assert!(
                cell.x + slack >= left,
                "{at}: starts at {:?}, before {left:?}",
                cell.x
            );
            assert!(
                cell.x + cell.width <= left + advance + slack,
                "{at}: ends at {:?}, past {:?}",
                cell.x + cell.width,
                left + advance
            );
        }
    }
}

#[gpui::test]
fn odd_graphemes_keep_every_cell_in_its_column(cx: &mut TestAppContext) {
    // Each odd cell followed by an ordinary one, plain and reversed: a wide
    // CJK character, an emoji, a lone combining mark, a newline and a tab.
    let odd = ["漢", "🙂", "\u{301}", "\n", "\t"];
    let mut cells = Vec::new();
    for reverse in [false, true] {
        for ch in odd {
            cells.push(cell(ch, Tone::Accent, reverse));
            cells.push(cell("a", Tone::Normal, false));
        }
    }
    let (panel, _, cx) = panel_with(decl(), grid_view("Odd", 10, cells), cx);
    let drawn = panel.read_with(cx, |panel, _| panel.drawn_grids["board"].clone());
    let advance = drawn.cell.width;
    assert_cells_in_place(&drawn);
    for row in &drawn.cells {
        assert_eq!(row.len(), 10);
        for (col, cell) in row.iter().enumerate() {
            if col % 2 == 1 {
                // The ordinary cell after each odd one: at its own column,
                // at the font's size, as in a row of nothing but letters.
                assert_eq!(cell.text.as_ref(), "a");
                assert_eq!(cell.font_size, drawn.font_size);
                assert!(
                    (cell.x - advance * col as f32).abs() < gpui::px(0.5),
                    "{col}: {:?}",
                    cell.x
                );
            }
        }
        // A control character is a space; the wide ones fit their cell.
        assert_eq!(row[6].text.as_ref(), " ");
        assert_eq!(row[8].text.as_ref(), " ");
        for wide in [0, 2] {
            assert!(
                row[wide].width <= advance + gpui::px(0.01),
                "{:?}",
                row[wide]
            );
        }
    }
    // The reversed row's odd cells have their ground; the letters none.
    let c = cx.update(|_, cx| GridTones::of(fe_ui::ActiveTheme::theme(cx)));
    for (col, cell) in drawn.cells[1].iter().enumerate() {
        let back = (col % 2 == 0).then(|| c.tone(Tone::Accent));
        assert_eq!(cell.back, back, "{col}");
    }
    // And the row's text has no line break in it.
    assert!(drawn.rows.iter().all(|row| !row.contains('\n')));
}

#[gpui::test]
fn keys_reach_a_focused_realtime_panel(cx: &mut TestAppContext) {
    let (workbench, panel, events, cx, _dir) = in_workbench(board(), cx);
    let mine = panel.read_with(cx, |panel, cx| panel.focus_handle(cx));
    let editor = workbench.read_with(cx, |workbench, cx| workbench.focus_handle(cx));
    // Unfocused, no key reaches it.
    workbench.update_in(cx, |_, window, cx| window.focus(&editor, cx));
    draw(cx);
    drain(&events);
    press("up", cx);
    assert!(
        !drain(&events)
            .iter()
            .any(|event| matches!(event, PanelEvent::Key(_)))
    );
    // Revealed by its command, it takes focus and rings it.
    reveal(&workbench, cx);
    assert!(focused_in(&mine, cx));
    assert!(has_ring(cx));
    drain(&events);
    // Arrows, letters and keys the workbench binds (F8, diagnostics.next)
    // go to the plugin: no binding takes them first.
    for k in [
        "up", "down", "left", "right", "a", "shift-z", "f8", "ctrl-x", "space",
    ] {
        press(k, cx);
    }
    let mut expected = vec![
        key("up", false, false),
        key("down", false, false),
        key("left", false, false),
        key("right", false, false),
        key("a", false, false),
        key("z", true, false),
        key("f8", false, false),
        key("space", false, false),
    ];
    // Control is the plugin's where it isn't the command key.
    if cfg!(target_os = "macos") {
        expected.insert(7, key("x", false, true));
    }
    assert_eq!(drain(&events), expected);
    // The host's keys never reach it: the secondary modifier's, Escape and
    // Tab.
    press("secondary-z", cx);
    press("tab", cx);
    assert!(
        !drain(&events)
            .iter()
            .any(|event| matches!(event, PanelEvent::Key(_)))
    );
    assert!(!focused_in(&mine, cx));
    assert!(!has_ring(cx));
    // A click on it gives it focus again.
    let at = cx.debug_bounds(selector("board").leak()).unwrap().center();
    cx.simulate_click(at, Modifiers::none());
    draw(cx);
    assert!(focused_in(&mine, cx));
    assert!(has_ring(cx));
    press("w", cx);
    assert_eq!(drain(&events), [key("w", false, false)]);
}

#[gpui::test]
fn escape_returns_focus_to_the_editor(cx: &mut TestAppContext) {
    let (workbench, panel, events, cx, _dir) = in_workbench(board(), cx);
    let mine = panel.read_with(cx, |panel, cx| panel.focus_handle(cx));
    let editor = workbench.read_with(cx, |workbench, cx| workbench.focus_handle(cx));
    reveal(&workbench, cx);
    assert!(focused_in(&mine, cx));
    drain(&events);
    press("escape", cx);
    assert!(!focused_in(&mine, cx));
    assert!(focused_in(&editor, cx));
    assert!(!has_ring(cx));
    // The plugin never hears of it, and the panel still shows.
    assert!(
        !drain(&events)
            .iter()
            .any(|event| matches!(event, PanelEvent::Key(_)))
    );
    assert!(panel.read_with(cx, |panel, _| panel.is_shown()));
    // With a modifier too: Escape is the host's whatever is held.
    let dock = workbench.read_with(cx, |workbench, _| workbench.dock().clone());
    dock.update_in(cx, |dock, window, cx| dock.focus_active(window, cx));
    draw(cx);
    assert!(focused_in(&mine, cx));
    press("shift-escape", cx);
    assert!(focused_in(&editor, cx));
    assert!(drain(&events).is_empty());
}

#[gpui::test]
fn blur_stops_ticks(cx: &mut TestAppContext) {
    let (workbench, panel, events, cx, _dir) = in_workbench(board(), cx);
    let editor = workbench.read_with(cx, |workbench, cx| workbench.focus_handle(cx));
    // Not focused: no timer.
    wait(TICK * 3, cx);
    assert!(!panel.read_with(cx, |panel, _| panel.is_ticking()));
    assert_eq!(ticks_delivered(&events), [] as [u32; 0]);
    // Focused and shown, it ticks every TICK_MS, each tick the time since
    // the last.
    reveal(&workbench, cx);
    assert!(panel.read_with(cx, |panel, _| panel.is_ticking()));
    drain(&events);
    let ticks = {
        let mut ticks = Vec::new();
        for _ in 0..3 {
            wait(TICK, cx);
            ticks.extend(ticks_delivered(&events));
        }
        ticks
    };
    assert_eq!(ticks, [TICK_MS; 3]);
    // A tick still in the plugin holds the next back: none overlap.
    wait(TICK, cx);
    let held = events.take(PLUGIN, usize::MAX);
    assert_eq!(held.len(), 1);
    wait(TICK * 3, cx);
    assert_eq!(events.queued(PLUGIN), 0, "a tick overlapped the last");
    for QueuedEvent { panel, event } in &held {
        events.delivered(PLUGIN, panel, event);
    }
    wait(TICK, cx);
    assert_eq!(ticks_delivered(&events), [TICK_MS * 4]);
    // Focus to the editor: the timer stops in that frame.
    workbench.update_in(cx, |_, window, cx| window.focus(&editor, cx));
    draw(cx);
    assert!(!panel.read_with(cx, |panel, _| panel.is_ticking()));
    wait(TICK * 5, cx);
    assert_eq!(ticks_delivered(&events), [] as [u32; 0]);
    // Back in it, it ticks; the window losing focus stops it again.
    let dock = workbench.read_with(cx, |workbench, _| workbench.dock().clone());
    dock.update_in(cx, |dock, window, cx| dock.focus_active(window, cx));
    draw(cx);
    assert!(panel.read_with(cx, |panel, _| panel.is_ticking()));
    wait(TICK, cx);
    assert_eq!(ticks_delivered(&events).len(), 1);
    cx.deactivate_window();
    draw(cx);
    assert!(!panel.read_with(cx, |panel, _| panel.is_ticking()));
    wait(TICK * 5, cx);
    assert_eq!(ticks_delivered(&events), [] as [u32; 0]);
    // Its load gone, the timer stops at its next tick, and none is queued.
    cx.update(|window, _| window.activate_window());
    draw(cx);
    assert!(panel.read_with(cx, |panel, _| panel.is_ticking()));
    events.unload(PLUGIN);
    wait(TICK * 2, cx);
    assert!(!panel.read_with(cx, |panel, _| panel.is_ticking()));
    assert_eq!(events.queued(PLUGIN), 0);
}

#[gpui::test]
fn hidden_panel_gets_no_ticks(cx: &mut TestAppContext) {
    let (panel, events, cx) = panel_with(decl(), board(), cx);
    let mine = panel.read_with(cx, |panel, cx| panel.focus_handle(cx));
    cx.update(|window, cx| window.focus(&mine, cx));
    draw(cx);
    assert!(panel.read_with(cx, |panel, _| panel.is_ticking()));
    wait(TICK, cx);
    assert_eq!(ticks_delivered(&events), [TICK_MS]);
    // Hidden while it keeps focus: the timer goes, and no tick comes.
    panel.update(cx, |panel, cx| panel.set_shown(false, cx));
    assert!(!panel.read_with(cx, |panel, _| panel.is_ticking()));
    assert_eq!(drain(&events), [PanelEvent::Hidden]);
    cx.update(|_, cx| cx.refresh_windows());
    wait(TICK * 5, cx);
    assert!(!panel.read_with(cx, |panel, _| panel.is_ticking()));
    assert_eq!(ticks_delivered(&events), [] as [u32; 0]);
    // Shown again, focused, it ticks again.
    panel.update(cx, |panel, cx| panel.set_shown(true, cx));
    cx.update(|_, cx| cx.refresh_windows());
    draw(cx);
    assert!(panel.read_with(cx, |panel, _| panel.is_ticking()));
    wait(TICK, cx);
    assert_eq!(ticks_delivered(&events), [TICK_MS]);
}

#[gpui::test]
fn a_panel_without_tick_ms_has_no_timer(cx: &mut TestAppContext) {
    let quiet = PanelDecl {
        tick_ms: None,
        ..decl()
    };
    let (panel, events, cx) = panel_with(quiet, board(), cx);
    let mine = panel.read_with(cx, |panel, cx| panel.focus_handle(cx));
    cx.update(|window, cx| window.focus(&mine, cx));
    draw(cx);
    wait(TICK * 3, cx);
    assert!(!panel.read_with(cx, |panel, _| panel.is_ticking()));
    assert_eq!(ticks_delivered(&events), [] as [u32; 0]);
    // It still takes keys.
    press("up", cx);
    assert_eq!(drain(&events), [key("up", false, false)]);
}

/// WCAG's contrast of two GPUI colours, opaque.
fn ratio(a: Hsla, b: Hsla) -> f32 {
    let color = |c: Hsla| {
        let rgba = c.to_rgb();
        let byte = |v: f32| (v.clamp(0., 1.) * 255.).round() as u32;
        fe_ui::Color::rgba((byte(rgba.r) << 24) | (byte(rgba.g) << 16) | (byte(rgba.b) << 8) | 0xff)
    };
    contrast_ratio(color(a), color(b))
}

#[gpui::test]
fn tones_pass_contrast_in_every_theme(cx: &mut TestAppContext) {
    // Every tone, plain and reversed.
    let cells = (TONES.iter())
        .flat_map(|&tone| [cell("@", tone, false), cell("@", tone, true)])
        .collect();
    let (panel, _, cx) = panel_with(decl(), grid_view("Tones", 12, cells), cx);
    let mut checked = 0;
    for theme in themes::bundled_themes() {
        for contrast in [Contrast::Normal, Contrast::High] {
            let data = ThemeData::from_file(&theme.file());
            cx.update(|_, cx| {
                GlobalTheme::set_contrast(cx, contrast);
                fe_ui::set_theme(cx, Theme::new(data.clone()));
                cx.refresh_windows();
            });
            draw(cx);
            let active = cx.update(|_, cx| fe_ui::ActiveTheme::theme(cx).clone());
            let floor = tone_floor(contrast);
            let name = format!("{} at {contrast:?}", theme.id);
            let drawn = panel.read_with(cx, |panel, _| panel.drawn_grids["board"].clone());
            let tones = GridTones::of(&active);
            for (ix, drawn_cell) in drawn.cells[0].iter().enumerate() {
                let (fore, back) = (drawn_cell.fore, drawn_cell.back);
                let tone = TONES[ix / 2];
                assert_eq!(back.is_some(), ix % 2 == 1, "{name}");
                let ground = back.unwrap_or(tones.ground);
                let got = ratio(fore, ground);
                assert!(
                    got >= floor - 0.01,
                    "{name}: {tone:?} (reversed: {}) is {got:.2}:1, under {floor}",
                    back.is_some()
                );
                // The theme's own token where it reaches the floor.
                let tokens = &active.data().tokens;
                let panel_ground = kiln::core::theme::surface_grounds(tokens)[1];
                let token = over(*tokens.get(tone_key(tone)).unwrap(), panel_ground);
                if contrast_ratio(token, panel_ground) >= floor {
                    assert_eq!(tones.tone(tone), fe_ui::hsla(token), "{name}: {tone:?}");
                }
                checked += 1;
            }
        }
    }
    assert!(themes::bundled_themes().len() >= 2);
    assert_eq!(checked, themes::bundled_themes().len() * 2 * 12);
}

/// A plugin, `acme.game`, with no view: its `on-event` logs each key's
/// name, and once it has had a key, spins on its next tick until the host
/// stops it at the call's deadline.
const GAME: &str = r#"
(component $C
  (import "kiln:extension/types@1.1.0" (instance $types
    (type $range (record (field "start" u64) (field "end" u64)))
    (export "range" (type (eq $range)))
    (type $error (variant
      (case "unimplemented" string)
      (case "denied" string)
      (case "not-found" string)
      (case "invalid-argument" string)
      (case "conflict" string)
      (case "failed" string)))
    (export "error" (type (eq $error)))))
  (alias export $types "range" (type $range))
  (alias export $types "error" (type $error))

  (import "kiln:extension/log@1.1.0" (instance $log
    (type $level (enum "trace" "debug" "info" "warn" "error"))
    (export "level" (type $level' (eq $level)))
    (export "log" (func (param "level" $level') (param "message" string)))))

  (import "kiln:extension/events@1.1.0" (instance $events
    (alias outer $C $range (type $range))
    (type $buffer-opened (record
      (field "buffer" u64)
      (field "version" u64)
      (field "path" (option string))
      (field "language" (option string))))
    (export "buffer-opened" (type $buffer-opened' (eq $buffer-opened)))
    (type $buffer-changed (record
      (field "buffer" u64) (field "version" u64) (field "ranges" (list $range))))
    (export "buffer-changed" (type $buffer-changed' (eq $buffer-changed)))
    (type $buffer-saved (record
      (field "buffer" u64) (field "version" u64) (field "path" string)))
    (export "buffer-saved" (type $buffer-saved' (eq $buffer-saved)))
    (type $selection-changed (record
      (field "buffer" u64) (field "version" u64) (field "ranges" (list $range))))
    (export "selection-changed" (type $selection-changed' (eq $selection-changed)))
    (type $event (variant
      (case "buffer-opened" $buffer-opened')
      (case "buffer-changed" $buffer-changed')
      (case "buffer-saved" $buffer-saved')
      (case "buffer-closed" u64)
      (case "focus-changed" (option u64))
      (case "selection-changed" $selection-changed')
      (case "config-reloaded")
      (case "lagged" u64)))
    (export "event" (type (eq $event)))))
  (alias export $events "event" (type $event))

  (core module $libc
    (memory (export "memory") 1)
    (global $heap (mut i32) (i32.const 1024))
    (func (export "realloc") (param i32 i32) (param $align i32) (param $size i32) (result i32)
      (local $p i32)
      (local.set $p
        (i32.and
          (i32.add (global.get $heap) (i32.sub (local.get $align) (i32.const 1)))
          (i32.sub (i32.const 0) (local.get $align))))
      (global.set $heap (i32.add (local.get $p) (local.get $size)))
      (local.get $p)))
  (core instance $libc (instantiate $libc))
  (alias core export $libc "memory" (core memory $memory))
  (alias core export $libc "realloc" (core func $realloc))

  (alias export $log "log" (func $log-log))
  (core func $log (canon lower (func $log-log) (memory $memory)))
  (core instance $host (export "log" (func $log)))

  (core module $main
    (import "libc" "memory" (memory 1))
    (import "host" "log" (func $log (param i32 i32 i32)))
    (global $keyed (mut i32) (i32.const 0))
    ;; init's result: ok, at 64.
    (func (export "init") (result i32)
      (i32.store8 (i32.const 64) (i32.const 0))
      (i32.const 64))
    (func (export "shutdown"))
    (func (export "run-command") (param i32 i32 i32 i32) (result i32) (i32.const 0))
    (func (export "handle-events") (param i32 i32))
    ;; The panel's name, the event's case and its payload, flat: `key` (8)
    ;; starts with the key's name, `tick` (9) is its milliseconds.
    (func (export "on-event")
      (param $panel i32) (param $panel-len i32) (param $case i32)
      (param $a i32) (param $b i32) (param i32) (param i32) (param i32) (param i32)
      (if (i32.eq (local.get $case) (i32.const 8))
        (then
          (global.set $keyed (i32.const 1))
          (return (call $log (i32.const 2) (local.get $a) (local.get $b)))))
      (if (i32.and (i32.eq (local.get $case) (i32.const 9)) (global.get $keyed))
        (then (loop $spin (br $spin))))))
  (core instance $main (instantiate $main
    (with "libc" (instance $libc))
    (with "host" (instance $host))))

  (func (export "init") (result (result (error $error)))
    (canon lift (core func $main "init") (memory $memory)))
  (func (export "shutdown")
    (canon lift (core func $main "shutdown")))

  (func $run-command (param "id" string) (param "args" string) (result (result (error $error)))
    (canon lift (core func $main "run-command") (memory $memory) (realloc $realloc)))
  (instance $command-handler (export "run-command" (func $run-command)))
  (export "kiln:extension/command-handler@1.1.0" (instance $command-handler))

  (func $handle-events (param "events" (list $event))
    (canon lift (core func $main "handle-events") (memory $memory) (realloc $realloc)))
  (instance $event-handler (export "handle-events" (func $handle-events)))
  (export "kiln:extension/event-handler@1.1.0" (instance $event-handler))

  (type $key-event' (record
    (field "key" string)
    (field "shift" bool)
    (field "ctrl" bool)
    (field "alt" bool)
    (field "secondary" bool)))
  (export $key-event "key-event" (type $key-event'))
  (type $panel-event' (variant
    (case "activated" string)
    (case "toggled" (tuple string bool))
    (case "input" (tuple string string))
    (case "submitted" (tuple string string))
    (case "selected" (tuple string (list string)))
    (case "expanded" (tuple string bool))
    (case "shown")
    (case "hidden")
    (case "key" $key-event)
    (case "tick" u32)))
  (export $panel-event "panel-event" (type $panel-event'))
  (func $on-event (param "panel" string) (param "event" $panel-event)
    (canon lift (core func $main "on-event") (memory $memory) (realloc $realloc)))
  (instance $panel-handler (export "on-event" (func $on-event)))
  (export "fastedit:editor/panel-handler@1.1.0" (instance $panel-handler)))
"#;

const GAME_ID: &str = "acme.game";

/// [`GAME`] installed in `dir`'s extensions, its panel `board` in the
/// sidebar with keys and a 50 ms tick, granted `panels` and
/// `panels.realtime`.
fn install_game(dir: &std::path::Path) -> crate::plugin_host::Found {
    use kiln::core::{
        ApiVersion,
        extension::{ExtensionApi, load_manifest},
    };
    use kiln_plugin::{
        grants::GrantStore,
        manifest::{ExtensionOrigin, PluginManifest},
    };
    let extension = dir.join("extensions").join(GAME_ID);
    std::fs::create_dir_all(&extension).unwrap();
    std::fs::write(extension.join("plugin.wasm"), wat::parse_str(GAME).unwrap()).unwrap();
    std::fs::write(
        extension.join("extension.toml"),
        format!(
            "id = \"{GAME_ID}\"\nname = \"{GAME_ID}\"\nversion = \"1.0.0\"\n\
             license = \"MIT\"\n\n[engine]\ntenmoku = \"1.1\"\n\n\
             [contributes.plugin]\nwasm = \"plugin.wasm\"\n\
             capabilities = [\"panels\", \"panels.realtime\"]\n\n\
             [[contributes.plugin.panels]]\nid = \"{PANEL}\"\ntitle = \"Board\"\n\
             icon = \"list-checks\"\ndock = \"left\"\ntick_ms = 50\nkeys = true\n"
        ),
    )
    .unwrap();
    let grants: Grants = [Capability::Panels, Capability::PanelsRealtime]
        .into_iter()
        .collect();
    GrantStore::new(dir.join("state").join(crate::plugin_host::GRANTS_FILE))
        .grant(GAME_ID, (1, 0, 0), &grants, &[], false)
        .unwrap();
    let api = ExtensionApi {
        app: fe_config::paths::app_id().short,
        version: ApiVersion { major: 1, minor: 1 },
        reserved: Vec::new(),
        kinds: vec![crate::plugin_host::plugin_kind()],
        allowed_licenses: None,
        only_kinds: None,
    };
    let (manifest, diagnostics) = load_manifest(&extension, &api);
    let manifest = manifest.unwrap_or_else(|| panic!("{diagnostics:#?}"));
    let plugin = PluginManifest::from_extension(&manifest, ExtensionOrigin::User)
        .unwrap()
        .unwrap();
    crate::plugin_host::Found::new(&manifest, Ok(plugin))
}

/// Runs until `done`: plugins load and take events on their own threads.
fn until_done(cx: &mut TestAppContext, what: &str, done: impl Fn(&mut TestAppContext) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        cx.run_until_parked();
        if done(cx) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {what}: {:#?}",
            cx.update(|cx| crate::plugin_host::plugin_states(cx))
        );
        cx.executor().advance_clock(MIN_UPDATE_INTERVAL);
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// What plugin [`GAME`] logged.
fn game_logged(cx: &mut TestAppContext) -> Vec<String> {
    let prefix = format!("log {GAME_ID} on plugin {GAME_ID}: ");
    cx.update(|cx| {
        let host = cx.global::<crate::plugin_host::PluginHost>();
        let record = host
            .record
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        (record.iter())
            .filter_map(|line| line.strip_prefix(&prefix))
            .map(str::to_owned)
            .collect()
    })
}

fn game_state(cx: &mut TestAppContext) -> Option<crate::plugin_host::PluginState> {
    cx.update(|cx| {
        (crate::plugin_host::plugin_states(cx).into_iter())
            .find(|info| info.id == GAME_ID)
            .map(|info| info.state)
    })
}

#[gpui::test]
fn a_running_game_gets_keys_and_is_unloaded_when_a_tick_overruns(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let dir = tempfile::tempdir().unwrap();
    let (state, cache) = (dir.path().join("state"), dir.path().join("cache"));
    cx.update(|cx| {
        fe_gpui::init(cx);
        crate::init(cx);
        crate::plugin_host::set_plugin_dirs(cx, state, cache);
        crate::plugin_host::set_preview(cx, true);
    });
    let found = install_game(dir.path());
    let launch = crate::Launch {
        root: None,
        file: None,
    };
    let (workbench, vcx) = cx.add_window_view(move |window, cx| {
        let theme = fe_gpui::EditorTheme::default_dark(cx);
        crate::Workbench::new(
            launch,
            theme,
            fe_gpui::EditorSettings::default(),
            window,
            cx,
        )
    });
    vcx.update(|window, _| window.activate_window());
    draw(vcx);
    cx.update(|cx| crate::plugin_host::set_plugins(cx, vec![found]));
    until_done(cx, "the plugin to run", |cx| {
        game_state(cx) == Some(crate::plugin_host::PluginState::Running)
    });
    let window = cx.windows()[0];
    let dock = workbench.read_with(cx, |workbench, _| workbench.dock().clone());
    let id = panel_id(GAME_ID, PANEL);
    cx.update_window(window, |_, window, cx| {
        dock.update(cx, |dock, cx| dock.rail_clicked(id, window, cx))
    })
    .unwrap();
    let panel = workbench.read_with(cx, |workbench, cx| {
        workbench.dock().read(cx).state().panel::<PluginPanel>()
    });
    let panel = panel.expect("the game's panel in the sidebar");
    until_done(cx, "the panel to tick", |cx| {
        panel.read_with(cx, |panel, _| panel.is_ticking())
    });
    // Ticks come and go back before any key: the plugin keeps running.
    for _ in 0..6 {
        cx.executor().advance_clock(MIN_UPDATE_INTERVAL);
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(20));
    }
    cx.run_until_parked();
    assert_eq!(
        game_state(cx),
        Some(crate::plugin_host::PluginState::Running)
    );
    // A key, through the host's delivery, in the WIT's shape.
    cx.update_window(window, |_, window, cx| {
        let keystroke = Keystroke::parse("k").unwrap();
        window.dispatch_event(
            gpui::PlatformInput::KeyDown(KeyDownEvent {
                keystroke,
                is_held: false,
                prefer_character_input: false,
            }),
            cx,
        );
    })
    .unwrap();
    until_done(cx, "the key's delivery", |cx| !game_logged(cx).is_empty());
    assert_eq!(game_logged(cx), ["k"]);
    // Its next tick spins past the call's deadline: the plugin is stopped
    // for it, as any call would be, and its panel goes.
    until_done(cx, "the overrunning tick to stop the plugin", |cx| {
        matches!(
            game_state(cx),
            Some(crate::plugin_host::PluginState::Stopped(diagnostic))
                if diagnostic.reason == crate::plugin_host::StopReason::Timeout
        )
    });
    until_done(cx, "the panel to go", |cx| {
        workbench.read_with(cx, |workbench, cx| {
            workbench
                .dock()
                .read(cx)
                .state()
                .panel::<PluginPanel>()
                .is_none()
        })
    });
    assert_eq!(super::events().queued(GAME_ID), 0);
}

#[gpui::test]
fn a_tick_waiting_behind_a_busy_plugin_is_dropped_on_blur_and_hide(cx: &mut TestAppContext) {
    let (panel, events, cx) = panel_with(decl(), board(), cx);
    let mine = panel.read_with(cx, |panel, cx| panel.focus_handle(cx));
    cx.update(|window, cx| window.focus(&mine, cx));
    draw(cx);
    // The plugin is busy: a tick waits in the queue.
    wait(TICK, cx);
    assert_eq!(events.queued(PLUGIN), 1);
    // The window loses focus: the waiting tick goes with the timer.
    cx.deactivate_window();
    draw(cx);
    assert!(!panel.read_with(cx, |panel, _| panel.is_ticking()));
    assert_eq!(events.queued(PLUGIN), 0);
    // Back, the next tick isn't held behind the dropped one.
    cx.update(|window, _| window.activate_window());
    draw(cx);
    wait(TICK, cx);
    // The host takes it into a batch, then the panel is hidden before the
    // call: the host is told not to make it.
    let taken = events.take(PLUGIN, usize::MAX);
    assert!(matches!(
        taken[..],
        [QueuedEvent {
            event: PanelEvent::Tick(_),
            ..
        }]
    ));
    panel.update(cx, |panel, cx| panel.set_shown(false, cx));
    assert!(!events.tick_is_current(PLUGIN, PANEL));
    drain(&events);
    // Shown again, ticks resume, and the host makes those calls.
    panel.update(cx, |panel, cx| panel.set_shown(true, cx));
    cx.update(|_, cx| cx.refresh_windows());
    draw(cx);
    wait(TICK, cx);
    let taken = events.take(PLUGIN, usize::MAX);
    let ticks: Vec<_> = (taken.iter())
        .filter(|queued| matches!(queued.event, PanelEvent::Tick(_)))
        .collect();
    assert_eq!(ticks.len(), 1);
    assert!(events.tick_is_current(PLUGIN, PANEL));
}

/// A game panel's view with controls beside its board: an input and a list.
fn board_with_controls() -> View {
    use fe_plugin::fastedit::editor::panels::{InputNode, ListNode, TextNode};
    let mut view = board();
    view.nodes[0].children.extend([2, 3]);
    view.nodes.push(n(
        "name",
        NodeKind::Input(InputNode {
            label: "Name".into(),
            value: String::new(),
            placeholder: None,
        }),
        &[],
    ));
    view.nodes.push(n(
        "scores",
        NodeKind::List(ListNode {
            label: "Scores".into(),
            multi_select: false,
        }),
        &[4, 5],
    ));
    for row in ["first", "second"] {
        view.nodes.push(n(
            row,
            NodeKind::Text(TextNode {
                text: row.into(),
                tone: Tone::Normal,
                mono: false,
                wrap: false,
            }),
            &[],
        ));
    }
    view
}

#[gpui::test]
fn escape_from_a_control_in_a_realtime_panel_returns_to_the_editor(cx: &mut TestAppContext) {
    let (workbench, panel, events, cx, _dir) = in_workbench(board_with_controls(), cx);
    let mine = panel.read_with(cx, |panel, cx| panel.focus_handle(cx));
    let editor = workbench.read_with(cx, |workbench, cx| workbench.focus_handle(cx));
    reveal(&workbench, cx);
    let input = panel.read_with(cx, |panel, cx| panel.inputs["name"].field.focus_handle(cx));
    let list = panel.read_with(cx, |panel, _| panel.lists["scores"].focus.clone());
    for (what, control) in [("the input", input), ("the list", list)] {
        let dock = workbench.read_with(cx, |workbench, _| workbench.dock().clone());
        dock.update_in(cx, |dock, window, cx| dock.focus_active(window, cx));
        draw(cx);
        cx.update(|window, cx| window.focus(&control, cx));
        draw(cx);
        assert!(focused_in(&control, cx), "{what}");
        assert!(panel.read_with(cx, |panel, _| panel.is_ticking()), "{what}");
        drain(&events);
        press("escape", cx);
        assert!(
            focused_in(&editor, cx),
            "{what}: escape left focus in the panel"
        );
        assert!(!focused_in(&mine, cx), "{what}");
        assert!(
            !panel.read_with(cx, |panel, _| panel.is_ticking()),
            "{what}"
        );
        assert!(
            !drain(&events)
                .iter()
                .any(|event| matches!(event, PanelEvent::Key(_))),
            "{what}"
        );
    }
}

#[gpui::test]
fn the_apps_chords_still_run_from_a_focused_realtime_panel(cx: &mut TestAppContext) {
    let (workbench, panel, events, cx, _dir) = in_workbench(board(), cx);
    let mine = panel.read_with(cx, |panel, cx| panel.focus_handle(cx));
    reveal(&workbench, cx);
    assert!(focused_in(&mine, cx));
    drain(&events);
    // `fe_gpui::init` installs the macOS keymap on every host; install the
    // host's own so `secondary` means what the keymap binds.
    let source = if cfg!(target_os = "macos") {
        fe_keymap::DEFAULT_MACOS
    } else {
        fe_keymap::DEFAULT_LINUX
    };
    let keymap = fe_keymap::load_keymap(source, None).0;
    cx.update(|_, cx| fe_gpui::set_user_keymap(cx, keymap));
    // `secondary-k secondary-t`: Change Color Theme.
    press("secondary-k", cx);
    press("secondary-t", cx);
    assert!(workbench.read_with(cx, |workbench, _| workbench.theme_picker_open()));
    assert!(
        !drain(&events)
            .iter()
            .any(|event| matches!(event, PanelEvent::Key(_)))
    );
    press("escape", cx);
    assert!(!workbench.read_with(cx, |workbench, _| workbench.theme_picker_open()));
    // `secondary-k z`, a chord a plain key ends: Zen Mode, the `z` the
    // chord's, not the plugin's.
    let dock = workbench.read_with(cx, |workbench, _| workbench.dock().clone());
    dock.update_in(cx, |dock, window, cx| dock.focus_active(window, cx));
    draw(cx);
    assert!(focused_in(&mine, cx));
    drain(&events);
    press("secondary-k", cx);
    press("z", cx);
    assert!(workbench.read_with(cx, |workbench, _| workbench.zen_mode()));
    assert!(
        !drain(&events)
            .iter()
            .any(|event| matches!(event, PanelEvent::Key(_)))
    );
}

#[gpui::test]
fn ctrl_tab_switches_tabs_from_a_focused_realtime_panel(cx: &mut TestAppContext) {
    let (workbench, panel, events, cx, dir) = in_workbench(board(), cx);
    let pane = workbench.read_with(cx, |workbench, _| workbench.pane().clone());
    for name in ["a.txt", "b.txt"] {
        let path = dir.path().join(name);
        std::fs::write(&path, name).unwrap();
        cx.update(|window, cx| pane.update(cx, |pane, cx| pane.open(path, window, cx)));
        draw(cx);
    }
    let shown = |cx: &mut VisualTestContext| {
        pane.read_with(cx, |pane, cx| {
            pane.active_item().unwrap().tab_title(cx).to_string()
        })
    };
    assert_eq!(shown(cx), "b.txt");
    let mine = panel.read_with(cx, |panel, cx| panel.focus_handle(cx));
    reveal(&workbench, cx);
    assert!(focused_in(&mine, cx));
    drain(&events);
    // Held ctrl, ctrl-tab: the most-recently-used switcher, whose release
    // shows the tab before; ctrl isn't the command key on macOS, so only
    // the hold on Tab keeps it the workbench's.
    cx.simulate_modifiers_change(Modifiers::control());
    draw(cx);
    cx.simulate_keystrokes("ctrl-tab");
    draw(cx);
    assert!(workbench.read_with(cx, |workbench, cx| workbench.tab_switcher(cx).is_some()));
    cx.simulate_modifiers_change(Modifiers::none());
    draw(cx);
    assert_eq!(shown(cx), "a.txt");
    // And ctrl-shift-tab, back.
    reveal_again(&workbench, cx);
    assert!(focused_in(&mine, cx));
    cx.simulate_modifiers_change(Modifiers::control());
    draw(cx);
    cx.simulate_keystrokes("ctrl-shift-tab");
    draw(cx);
    cx.simulate_modifiers_change(Modifiers::none());
    draw(cx);
    assert_eq!(shown(cx), "b.txt");
    assert!(
        !drain(&events)
            .iter()
            .any(|event| matches!(event, PanelEvent::Key(_)))
    );
}

/// Focuses the sidebar's shown panel again.
fn reveal_again(workbench: &Entity<crate::Workbench>, cx: &mut VisualTestContext) {
    let dock = workbench.read_with(cx, |workbench, _| workbench.dock().clone());
    dock.update_in(cx, |dock, window, cx| dock.focus_active(window, cx));
    draw(cx);
}

/// A workbench window of its own holding the panel `decl()` on `views` and
/// `events`, shown in its sidebar.
fn game_window(
    views: &PanelViews,
    events: &PanelEvents,
    cx: &mut TestAppContext,
) -> (gpui::WindowHandle<crate::Workbench>, Entity<PluginPanel>) {
    let launch = crate::Launch {
        root: None,
        file: None,
    };
    let window = cx.add_window(move |window, cx| {
        let theme = fe_gpui::EditorTheme::default_dark(cx);
        crate::Workbench::new(
            launch,
            theme,
            fe_gpui::EditorSettings::default(),
            window,
            cx,
        )
    });
    let (views, events) = (views.clone(), events.clone());
    let panel = window
        .update(cx, |workbench, _, cx| {
            let panel = cx.new(|cx| PluginPanel::new(PLUGIN, &decl(), views, events, cx));
            workbench.add_plugin_panel(panel.clone(), cx);
            panel
        })
        .unwrap();
    draw_window(window, cx);
    (window, panel)
}

fn draw_window(window: gpui::WindowHandle<crate::Workbench>, cx: &mut TestAppContext) {
    cx.run_until_parked();
    // Not through the typed handle, which holds the workbench meanwhile.
    let _ = cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx));
    cx.run_until_parked();
}

/// Shows and focuses the panel in `window`, the window active.
fn focus_game(window: gpui::WindowHandle<crate::Workbench>, cx: &mut TestAppContext) {
    let id = panel_id(PLUGIN, PANEL);
    let dock = window
        .read_with(cx, |workbench, _| workbench.dock().clone())
        .unwrap();
    cx.update_window(window.into(), |_, window, cx| {
        window.activate_window();
        dock.update(cx, |dock, cx| dock.rail_clicked(id, window, cx));
    })
    .unwrap();
    draw_window(window, cx);
}

fn close_window(window: gpui::WindowHandle<crate::Workbench>, cx: &mut TestAppContext) {
    window
        .update(cx, |_, window, _| window.remove_window())
        .unwrap();
    cx.run_until_parked();
}

/// Lets the poll note the panel in `window` shows, takes what that queued
/// (`shown`), then lets its first tick fall due, which waits in the queue.
fn one_tick(
    window: gpui::WindowHandle<crate::Workbench>,
    events: &PanelEvents,
    cx: &mut TestAppContext,
) {
    cx.executor().advance_clock(MIN_UPDATE_INTERVAL);
    draw_window(window, cx);
    events.take(PLUGIN, usize::MAX);
    cx.executor().advance_clock(TICK - MIN_UPDATE_INTERVAL);
    draw_window(window, cx);
}

fn ticking(panel: &Entity<PluginPanel>, cx: &mut TestAppContext) -> bool {
    panel.read_with(cx, |panel, _| panel.is_ticking())
}

#[gpui::test]
fn closing_a_window_drops_its_panels_ticks_and_no_one_elses(cx: &mut TestAppContext) {
    cx.update(|cx| {
        fe_gpui::init(cx);
        crate::init(cx);
    });
    let views = PanelViews::new();
    let events = PanelEvents::new();
    slice(&views)
        .set_view_at(PANEL, board(), Instant::now())
        .unwrap();
    let (first, first_panel) = game_window(&views, &events, cx);
    let (second, second_panel) = game_window(&views, &events, cx);
    let (third, _) = game_window(&views, &events, cx);
    // The first window's panel ticks; the host has taken its tick into a
    // batch when the window closes: the host is told not to make the call.
    focus_game(first, cx);
    assert!(ticking(&first_panel, cx));
    one_tick(first, &events, cx);
    let taken = events.take(PLUGIN, usize::MAX);
    assert!(matches!(
        taken[..],
        [QueuedEvent {
            event: PanelEvent::Tick(_),
            ..
        }]
    ));
    // Closed with the panel and its workbench still held: no clock moves, no
    // release, and the tick is stale at once.
    let first_workbench = first.root(cx).unwrap();
    close_window(first, cx);
    assert!(!ticking(&first_panel, cx));
    assert!(!events.tick_is_current(PLUGIN, PANEL));
    // The second's ticks; a tick waits in the queue. A third window, its
    // panel not ticking, closes: the second's tick stays, and goes.
    focus_game(second, cx);
    assert!(ticking(&second_panel, cx));
    one_tick(second, &events, cx);
    assert_eq!(events.queued(PLUGIN), 1);
    close_window(third, cx);
    assert_eq!(events.queued(PLUGIN), 1);
    assert!(ticking(&second_panel, cx));
    // The second window closes with its tick still queued, its panel still
    // held: the tick goes at once too.
    close_window(second, cx);
    assert!(!ticking(&second_panel, cx));
    assert_eq!(
        events.queued(PLUGIN),
        0,
        "{:?}",
        events.take(PLUGIN, usize::MAX)
    );
    drop((first_panel, first_workbench, second_panel));
}

#[test]
fn ending_one_instances_tick_session_leaves_a_later_ones_be() {
    let events = PanelEvents::new();
    let runtime = events.runtime(PLUGIN);
    let state = super::PanelState {
        shown: true,
        focused: true,
    };
    // Two windows' instances of the panel: the first's session, then the
    // second's, opened while the first's was (its window still drawing it).
    let first = events.open_ticks(PLUGIN, PANEL);
    let second = events.open_ticks(PLUGIN, PANEL);
    // Only the open session ticks.
    let now = Instant::now();
    assert_eq!(
        events.tick(PLUGIN, PANEL, first, &runtime, state, now, 100),
        super::Ticked::Skipped(super::Skipped::Unfocused)
    );
    assert_eq!(
        events.tick(PLUGIN, PANEL, second, &runtime, state, now, 100),
        super::Ticked::Queued
    );
    // The first ends (its window closes): the second's tick stays, and
    // goes.
    events.close_ticks(PLUGIN, PANEL, first);
    assert_eq!(events.queued(PLUGIN), 1);
    assert!(events.tick_is_current(PLUGIN, PANEL));
    // The second ends with its tick queued: it goes, and the runtime lets
    // the next one go.
    events.close_ticks(PLUGIN, PANEL, second);
    assert_eq!(events.queued(PLUGIN), 0);
    let third = events.open_ticks(PLUGIN, PANEL);
    let later = now + Duration::from_secs(1);
    assert_eq!(
        events.tick(PLUGIN, PANEL, third, &runtime, state, later, 100),
        super::Ticked::Queued
    );
}
