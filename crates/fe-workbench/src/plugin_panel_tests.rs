//! A plugin's panel ([`super`]): every node kind's role and name, a long
//! list drawn a screenful at a time, keys and clicks queued for the plugin
//! as panel events, tones in the theme's colours, the host's capped event
//! queue, input debouncing, and the panel in the sidebar.

use std::time::{Duration, Instant};

use fe_command::capability::{Capability, Grants};
use fe_config::themes;
use fe_plugin::fastedit::editor::panels::{
    ButtonNode, InputNode, ListNode, Node, NodeKind, TextNode, ToggleNode, TreeItemNode,
};
use fe_plugin::panels::{MIN_UPDATE_INTERVAL, State};
use fe_ui::{ActiveTheme as _, Role, Theme, ThemeData};
use gpui::{
    AppContext as _, Entity, Focusable as _, Hsla, KeyDownEvent, KeyUpEvent, Keystroke,
    TestAppContext, VisualTestContext,
};
use kiln_l10n::tr;
use kiln_plugin::manifest::{PanelDecl, PanelLocation};
use kiln_shell::DockView as _;

use super::{
    INPUT_DEBOUNCE, MAX_QUEUED_EVENTS, PanelEvent, PanelEvents, PanelViews, PluginPanel,
    QueuedEvent, Tone, View, element_id, panel_id, role_of, tone_color,
};

const PLUGIN: &str = "acme";
const PANEL: &str = "todos";

fn decl() -> PanelDecl {
    PanelDecl {
        id: PANEL.into(),
        title: "TODOs".into(),
        icon: "list-checks".into(),
        location: PanelLocation::Left,
        tick_ms: None,
        keys: false,
    }
}

/// Plugin `acme`'s panels slice on `views`, granted `panels`, with the panel
/// `todos`.
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

fn text(text: &str, tone: Tone) -> NodeKind {
    NodeKind::Text(TextNode {
        text: text.into(),
        tone,
        mono: false,
        wrap: false,
    })
}

fn button(label: &str) -> NodeKind {
    NodeKind::Button(ButtonNode {
        label: label.into(),
        icon: Some("refresh-cw".into()),
        command: None,
        enabled: true,
    })
}

fn list(label: &str) -> NodeKind {
    NodeKind::List(ListNode {
        label: label.into(),
        multi_select: false,
    })
}

fn item(label: &str, expanded: bool) -> NodeKind {
    NodeKind::TreeItem(TreeItemNode {
        label: label.into(),
        icon: None,
        expandable: true,
        expanded,
        detail: None,
        tone: Tone::Normal,
    })
}

fn leaf(label: &str) -> NodeKind {
    NodeKind::TreeItem(TreeItemNode {
        label: label.into(),
        icon: Some("file".into()),
        expandable: false,
        expanded: false,
        detail: Some("src/lib.rs".into()),
        tone: Tone::Muted,
    })
}

/// A view with a node of every kind.
fn every_kind() -> View {
    View {
        nodes: vec![
            /* 0 */
            n(
                "root",
                NodeKind::Column,
                &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12],
            ),
            /* 1 */ n("head", NodeKind::Heading("TODOs".into()), &[]),
            /* 2 */ n("count", text("3 items", Tone::Muted), &[]),
            /* 3 */ n("bar", NodeKind::Row, &[13, 14]),
            /* 4 */ n("refresh", button("Refresh"), &[]),
            /* 5 */
            n(
                "mine",
                NodeKind::Toggle(ToggleNode {
                    label: "Only mine".into(),
                    on: false,
                    enabled: true,
                }),
                &[],
            ),
            /* 6 */
            n(
                "filter",
                NodeKind::Input(InputNode {
                    label: "Filter".into(),
                    value: String::new(),
                    placeholder: Some("Filter TODOs".into()),
                }),
                &[],
            ),
            /* 7 */ n("rule", NodeKind::Separator, &[]),
            /* 8 */ n("done", NodeKind::Progress(Some(40)), &[]),
            /* 9 */ n("files", list("Files"), &[15, 16]),
            /* 10 */ n("todos", list("Items"), &[17, 18]),
            /* 11 */ n("more", NodeKind::Scroll, &[19]),
            /* 12 */ n("loose", item("Loose", false), &[]),
            /* 13 */ n("mark", NodeKind::Icon("list-checks".into()), &[]),
            /* 14 */
            n(
                "new",
                NodeKind::Badge(TextNode {
                    text: "2 new".into(),
                    tone: Tone::Accent,
                    mono: false,
                    wrap: false,
                }),
                &[],
            ),
            /* 15 */ n("lib", item("lib.rs", true), &[20]),
            /* 16 */ n("main", leaf("main.rs"), &[]),
            /* 17 */ n("first", text("Fix the parser", Tone::Normal), &[]),
            /* 18 */ n("second", text("Write docs", Tone::Normal), &[]),
            /* 19 */ n("tail", text("…", Tone::Muted), &[]),
            /* 20 */ n("todo-1", leaf("TODO: split"), &[]),
        ],
    }
}

/// The theme `name` active, a bundled one.
fn use_theme(name: &str, cx: &mut VisualTestContext) {
    let data = ThemeData::from_file(&themes::find(name).unwrap().file());
    cx.update(|_, cx| fe_ui::set_theme(cx, Theme::new(data)));
}

/// A panel drawing `view`, set through the plugin's slice as the plugin
/// sets it, alone in a window, shown.
fn panel_with(
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
        move |_, cx| PluginPanel::new(PLUGIN, &decl(), views, events, cx)
    });
    use_theme(themes::DEFAULT_DARK, cx);
    panel.update(cx, |panel, cx| panel.set_shown(true, cx));
    events.take(PLUGIN, usize::MAX);
    // The views stay the load's after its slice goes.
    drop(state);
    draw(cx);
    (panel, events, cx)
}

fn draw(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.run_until_parked();
}

/// The recorded accessibility nodes, drawn afresh.
fn a11y(cx: &mut VisualTestContext) -> Vec<fe_ui::Node> {
    cx.update(|_, cx| cx.refresh_windows());
    draw(cx);
    cx.update(|window, cx| fe_ui::nodes(window, cx))
}

/// The (role, name) of the node with id `id`.
fn named(nodes: &[fe_ui::Node], id: gpui::ElementId) -> Option<(Role, String)> {
    (nodes.iter())
        .find(|node| node.id == id)
        .map(|node| (node.role, node.label.to_string()))
}

fn drain(events: &PanelEvents) -> Vec<PanelEvent> {
    (events.take(PLUGIN, usize::MAX).into_iter())
        .map(|QueuedEvent { panel, event }| {
            assert_eq!(panel, PANEL);
            event
        })
        .collect()
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

/// Focuses list `id` of `panel`'s view.
fn focus_list(panel: &Entity<PluginPanel>, id: &str, cx: &mut VisualTestContext) {
    let focus = panel.read_with(cx, |panel, _| panel.lists[id].focus.clone());
    cx.update(|window, cx| window.focus(&focus, cx));
    draw(cx);
}

#[gpui::test]
fn panel_nodes_have_roles_and_labels(cx: &mut TestAppContext) {
    let (_panel, _, cx) = panel_with(every_kind(), cx);
    let nodes = a11y(cx);
    let at = |id: &str| named(&nodes, element_id(id));
    let has = |role: Role, name: &str| Some((role, name.to_owned()));
    // Containers: a role, no name.
    assert_eq!(at("root"), has(Role::Group, ""));
    assert_eq!(at("bar"), has(Role::Group, ""));
    assert_eq!(at("more"), has(Role::ScrollView, ""));
    assert_eq!(at("rule"), has(Role::Splitter, ""));
    // Content: named by what it says.
    assert_eq!(at("head"), has(Role::Heading, "TODOs"));
    assert_eq!(at("count"), has(Role::Label, "3 items"));
    assert_eq!(at("new"), has(Role::Label, "2 new"));
    assert_eq!(at("mark"), has(Role::Image, "list-checks"));
    let progress = (nodes.iter()).find(|node| node.id == element_id("done"));
    let progress = progress.expect("the progress bar's node");
    assert_eq!(progress.role, Role::ProgressIndicator);
    assert_eq!(progress.label, tr!("workbench-extensions-panel-progress"));
    assert_eq!(progress.states.value.as_deref(), Some("40%"));
    // Controls: named by their labels.
    assert_eq!(at("refresh"), has(Role::Button, "Refresh"));
    assert_eq!(at("mine"), has(Role::CheckBox, "Only mine"));
    let input = (nodes.iter()).find(|node| node.role == Role::TextInput);
    assert_eq!(
        input.map(|node| node.label.to_string()),
        Some("Filter".into())
    );
    // A list of tree items is a tree of tree items, with levels and
    // places; a list of other rows a list box of options.
    assert_eq!(at("files"), has(Role::Tree, "Files"));
    assert_eq!(at("lib"), has(Role::TreeItem, "lib.rs"));
    assert_eq!(at("todo-1"), has(Role::TreeItem, "TODO: split"));
    assert_eq!(at("main"), has(Role::TreeItem, "main.rs"));
    let lib = (nodes.iter())
        .find(|node| node.id == element_id("lib"))
        .unwrap();
    assert_eq!(lib.states.expanded, Some(true));
    assert_eq!(lib.states.level, Some(1));
    let todo = (nodes.iter())
        .find(|node| node.id == element_id("todo-1"))
        .unwrap();
    assert_eq!(todo.states.level, Some(2));
    let main = (nodes.iter())
        .find(|node| node.id == element_id("main"))
        .unwrap();
    assert_eq!(
        (main.states.position_in_set, main.states.size_of_set),
        (Some(2), Some(2))
    );
    assert_eq!(at("todos"), has(Role::ListBox, "Items"));
    let options: Vec<_> = (nodes.iter())
        .filter(|node| node.role == Role::ListBoxOption)
        .map(|node| (node.label.to_string(), node.states.position_in_set))
        .collect();
    assert_eq!(
        options,
        [
            ("Fix the parser".into(), Some(1)),
            ("Write docs".into(), Some(2))
        ]
    );
    // A tree item outside a list.
    assert_eq!(at("loose"), has(Role::TreeItem, "Loose"));
    // Every kind's role, as `role_of` gives it, is the one drawn.
    let view = every_kind();
    for (index, node) in view.nodes.iter().enumerate() {
        let Some((role, _)) = at(&node.id) else {
            // The input's node is its field's, found by role above.
            assert!(
                matches!(node.kind, NodeKind::Input(_)),
                "{} drew no node",
                node.id
            );
            continue;
        };
        let tree = super::is_tree(&view, index);
        assert_eq!(role, role_of(&node.kind, tree), "{}", node.id);
    }
}

/// A list of two tree items holding 4,999 and 4,998 items, all expanded:
/// 9,999 rows, as many as a view holds (10,000 nodes, the list one of them).
fn long_tree() -> View {
    let mut nodes = vec![
        n("list", list("Results"), &[1, 2]),
        n("a", item("a", true), &[]),
        n("b", item("b", true), &[]),
    ];
    for (parent, count) in [(1usize, 4_999usize), (2, 4_998)] {
        for ix in 0..count {
            let at = nodes.len() as u32;
            nodes[parent].children.push(at);
            let id = format!("{}{ix}", nodes[parent].id);
            nodes.push(n(&id, leaf(&id), &[]));
        }
    }
    View { nodes }
}

#[gpui::test]
fn panel_list_virtualizes_ten_thousand_rows(cx: &mut TestAppContext) {
    let view = long_tree();
    assert_eq!(view.nodes.len(), fe_plugin::panels::MAX_NODES);
    let (panel, events, cx) = panel_with(view, cx);
    let rows = panel.read_with(cx, |panel, _| panel.lists["list"].rows.len());
    assert_eq!(rows, 9_999);
    // Only a screenful of rows is built.
    let drawn = panel.read_with(cx, |panel, _| panel.drawn_rows["list"].clone());
    assert_eq!(drawn.start, 0);
    assert!(drawn.len() < 100, "built {drawn:?} of {rows} rows");
    let items = a11y(cx)
        .into_iter()
        .filter(|node| node.role == Role::TreeItem)
        .count();
    assert!(items < 100, "{items} tree item nodes for {rows} rows");
    // End selects the last row, scrolled into view and built.
    focus_list(&panel, "list", cx);
    press("end", cx);
    draw(cx);
    let drawn = panel.read_with(cx, |panel, _| panel.drawn_rows["list"].clone());
    assert!(drawn.contains(&(rows - 1)), "built {drawn:?}");
    assert!(drawn.len() < 100, "built {drawn:?}");
    assert_eq!(
        drain(&events),
        [PanelEvent::Selected("list".into(), vec!["b4997".into()])]
    );
}

#[gpui::test]
fn panel_keyboard_activation_sends_event(cx: &mut TestAppContext) {
    let (panel, events, cx) = panel_with(every_kind(), cx);
    // Arrows select a list's rows, Enter activates the selected one.
    focus_list(&panel, "todos", cx);
    press("down", cx);
    press("down", cx);
    press("enter", cx);
    assert_eq!(
        drain(&events),
        [
            PanelEvent::Selected("todos".into(), vec!["first".into()]),
            PanelEvent::Selected("todos".into(), vec!["second".into()]),
            PanelEvent::Activated("second".into()),
        ]
    );
    // In the tree, Left collapses an expanded item and Right expands it,
    // shown at once; Right again moves to its first child, Left back up.
    focus_list(&panel, "files", cx);
    press("down", cx);
    press("left", cx);
    let rows = |panel: &Entity<PluginPanel>, cx: &mut VisualTestContext| {
        panel.read_with(cx, |panel, _| panel.lists["files"].rows.len())
    };
    assert_eq!(rows(&panel, cx), 2);
    press("right", cx);
    assert_eq!(rows(&panel, cx), 3);
    press("right", cx);
    press("enter", cx);
    press("left", cx);
    assert_eq!(
        drain(&events),
        [
            PanelEvent::Selected("files".into(), vec!["lib".into()]),
            PanelEvent::Expanded("lib".into(), false),
            PanelEvent::Expanded("lib".into(), true),
            PanelEvent::Selected("files".into(), vec!["todo-1".into()]),
            PanelEvent::Activated("todo-1".into()),
            PanelEvent::Selected("files".into(), vec!["lib".into()]),
        ]
    );
    // Tab goes through the window's tab stops, from the panel to its
    // button first; Enter presses the focused button, and Space the toggle
    // after it.
    let root = panel.read_with(cx, |panel, cx| panel.focus_handle(cx));
    cx.update(|window, cx| window.focus(&root, cx));
    press("tab", cx);
    press("enter", cx);
    press("tab", cx);
    press("space", cx);
    assert_eq!(
        drain(&events),
        [
            PanelEvent::Activated("refresh".into()),
            PanelEvent::Toggled("mine".into(), true),
        ]
    );
    // The toggle shows its new state until the plugin's next view.
    let checked = (a11y(cx).into_iter()).find(|node| node.id == element_id("mine"));
    assert_eq!(checked.and_then(|node| node.states.toggled), Some(true));
}

#[gpui::test]
fn panel_theme_tones_follow_theme(cx: &mut TestAppContext) {
    let tones = [
        ("normal", Tone::Normal),
        ("muted", Tone::Muted),
        ("accent", Tone::Accent),
        ("success", Tone::Success),
        ("warning", Tone::Warning),
        ("error", Tone::Error),
    ];
    let mut nodes = vec![n("root", NodeKind::Column, &[])];
    for (id, tone) in tones {
        let at = nodes.len() as u32;
        nodes[0].children.push(at);
        nodes.push(n(id, text(id, tone), &[]));
    }
    let (panel, _, cx) = panel_with(View { nodes }, cx);
    let mut seen: Vec<Vec<Hsla>> = Vec::new();
    for theme in [themes::DEFAULT_DARK, themes::DEFAULT_LIGHT] {
        use_theme(theme, cx);
        cx.update(|_, cx| cx.refresh_windows());
        draw(cx);
        let c = cx.update(|_, cx| cx.theme().colors);
        let drawn: Vec<Hsla> = tones
            .iter()
            .map(|(id, tone)| {
                let drawn = panel.read_with(cx, |panel, _| panel.drawn_colors[*id]);
                assert_eq!(drawn, tone_color(*tone, &c), "{theme}: {id}");
                drawn
            })
            .collect();
        seen.push(drawn);
    }
    // Each tone is a colour of its own, and the theme's: none stays when
    // the theme changes.
    for drawn in &seen {
        for (ix, color) in drawn.iter().enumerate() {
            assert!(!drawn[..ix].contains(color), "two tones share {color:?}");
        }
    }
    for (ix, (id, _)) in tones.iter().enumerate() {
        assert_ne!(seen[0][ix], seen[1][ix], "{id} is the same in both themes");
    }
}

#[test]
fn panel_events_drop_oldest_and_count_past_the_cap() {
    let events = PanelEvents::new();
    let extra = 10;
    for ix in 0..MAX_QUEUED_EVENTS + extra {
        let dropped = events.push(PLUGIN, PANEL, PanelEvent::Activated(format!("{ix}")));
        assert_eq!(dropped, ix >= MAX_QUEUED_EVENTS, "event {ix}");
    }
    // Another plugin's queue is its own.
    events.push("other", PANEL, PanelEvent::Shown);
    assert_eq!(events.queued(PLUGIN), MAX_QUEUED_EVENTS);
    assert_eq!(events.dropped(PLUGIN), extra as u64);
    assert_eq!(events.dropped("other"), 0);
    let taken = events.take(PLUGIN, 2);
    assert_eq!(
        taken.iter().map(|queued| &queued.event).collect::<Vec<_>>(),
        [
            &PanelEvent::Activated(format!("{extra}")),
            &PanelEvent::Activated(format!("{}", extra + 1)),
        ]
    );
    assert_eq!(events.queued(PLUGIN), MAX_QUEUED_EVENTS - 2);
    // Taken, the queue has room again: nothing more is dropped.
    assert!(!events.push(PLUGIN, PANEL, PanelEvent::Hidden));
    assert_eq!(events.dropped(PLUGIN), extra as u64);
    // The plugin's waker hears of each event queued for it, and of no
    // other plugin's, until it's unloaded.
    let woken = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = woken.clone();
    events.wake_with(PLUGIN, move || {
        counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    });
    events.push(PLUGIN, PANEL, PanelEvent::Shown);
    events.push("other", PANEL, PanelEvent::Shown);
    assert_eq!(woken.load(std::sync::atomic::Ordering::SeqCst), 1);
    events.unload(PLUGIN);
    assert_eq!((events.queued(PLUGIN), events.dropped(PLUGIN)), (0, 0));
    assert_eq!(events.queued("other"), 2);
    events.push(PLUGIN, PANEL, PanelEvent::Shown);
    assert_eq!(woken.load(std::sync::atomic::Ordering::SeqCst), 1);
    // A stopped load forgets its own waker but not a later load's.
    let first = events.wake_with(PLUGIN, || {});
    let counter = woken.clone();
    let second = events.wake_with(PLUGIN, move || {
        counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    });
    events.unwake(PLUGIN, &first);
    events.push(PLUGIN, PANEL, PanelEvent::Shown);
    assert_eq!(woken.load(std::sync::atomic::Ordering::SeqCst), 2);
    events.unwake(PLUGIN, &second);
    events.push(PLUGIN, PANEL, PanelEvent::Shown);
    assert_eq!(woken.load(std::sync::atomic::Ordering::SeqCst), 2);
}

#[gpui::test]
fn panel_input_sends_text_after_a_pause_and_on_enter(cx: &mut TestAppContext) {
    let (panel, events, cx) = panel_with(every_kind(), cx);
    let field = panel.read_with(cx, |panel, _| panel.inputs["filter"].field.clone());
    let focus = field.read_with(cx, |field, cx| field.focus_handle(cx));
    cx.update(|window, cx| window.focus(&focus, cx));
    cx.simulate_input("fi");
    cx.executor().advance_clock(INPUT_DEBOUNCE / 2);
    cx.simulate_input("x");
    cx.executor().advance_clock(INPUT_DEBOUNCE / 2);
    cx.run_until_parked();
    assert_eq!(drain(&events), []);
    cx.executor().advance_clock(INPUT_DEBOUNCE);
    cx.run_until_parked();
    assert_eq!(
        drain(&events),
        [PanelEvent::Input("filter".into(), "fix".into())]
    );
    cx.simulate_input("e");
    press("enter", cx);
    cx.executor().advance_clock(INPUT_DEBOUNCE * 2);
    cx.run_until_parked();
    assert_eq!(
        drain(&events),
        [PanelEvent::Submitted("filter".into(), "fixe".into())]
    );
}

#[gpui::test]
fn panel_follows_the_plugins_views(cx: &mut TestAppContext) {
    let (panel, _, cx) = panel_with(every_kind(), cx);
    let views = panel.read_with(cx, |panel, _| panel.views.clone());
    let state = slice(&views);
    // A new load's first view applies at once; the next within the rate is
    // held, then shown once due.
    let one = View {
        nodes: vec![n("root", text("one", Tone::Normal), &[])],
    };
    let two = View {
        nodes: vec![n("root", text("two", Tone::Normal), &[])],
    };
    state
        .set_view_at(PANEL, one.clone(), Instant::now())
        .unwrap();
    state
        .set_view_at(PANEL, two.clone(), Instant::now())
        .unwrap();
    cx.executor().advance_clock(MIN_UPDATE_INTERVAL);
    cx.run_until_parked();
    assert_eq!(
        panel.read_with(cx, |panel, _| panel.view().cloned()),
        Some(one)
    );
    std::thread::sleep(MIN_UPDATE_INTERVAL + Duration::from_millis(5));
    cx.executor().advance_clock(MIN_UPDATE_INTERVAL);
    cx.run_until_parked();
    assert_eq!(
        panel.read_with(cx, |panel, _| panel.view().cloned()),
        Some(two)
    );
    // Unloaded, the plugin's panel draws nothing.
    views.unload(PLUGIN);
    cx.executor().advance_clock(MIN_UPDATE_INTERVAL);
    cx.run_until_parked();
    assert_eq!(panel.read_with(cx, |panel, _| panel.view().cloned()), None);
}

/// A workbench on an empty folder, with plugin `acme`'s panel `todos`
/// declared at `location` added to it twice, as a reload would, the view
/// [`every_kind`] set.
fn in_workbench(
    location: PanelLocation,
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
        .set_view_at(PANEL, every_kind(), Instant::now())
        .unwrap();
    let decl = PanelDecl { location, ..decl() };
    let new = |cx: &mut VisualTestContext| {
        cx.update(|_, cx| {
            cx.new(|cx| PluginPanel::new(PLUGIN, &decl, views.clone(), events.clone(), cx))
        })
    };
    let panel = new(cx);
    workbench.update(cx, |workbench, cx| {
        workbench.add_plugin_panel(panel.clone(), cx)
    });
    let again = new(cx);
    workbench.update(cx, |workbench, cx| workbench.add_plugin_panel(again, cx));
    draw(cx);
    (workbench, panel, events, cx, dir)
}

/// What the plugin was told of its panel's showing, since last asked.
fn showings(events: &PanelEvents) -> Vec<PanelEvent> {
    (events.take(PLUGIN, usize::MAX).into_iter())
        .map(|queued| queued.event)
        .collect()
}

#[gpui::test]
fn plugin_panel_is_on_the_sidebars_rail_and_says_when_it_shows(cx: &mut TestAppContext) {
    let (workbench, panel, events, cx, _dir) = in_workbench(PanelLocation::Left, cx);
    let dock = workbench.read_with(cx, |workbench, _| workbench.dock().clone());
    let bottom = workbench.read_with(cx, |workbench, _| workbench.terminal_dock().clone());
    let id = panel_id(PLUGIN, PANEL);
    assert_eq!(id.0, "acme.todos");
    // On the rail once, in the sidebar, not the bottom panel.
    let rail = dock.read_with(cx, |dock, _| dock.state().drawn().rail_ids());
    assert_eq!(rail.iter().filter(|on| **on == id).count(), 1, "{rail:?}");
    assert!(dock.read_with(cx, |dock, _| dock.state().panel_ids().contains(&id)));
    assert!(!bottom.read_with(cx, |bottom, _| bottom.dock().panel_ids().contains(&id)));
    let icons = dock.read_with(cx, |dock, _| dock.state().drawn().rail_icons());
    assert!(icons.contains(&fe_ui::Icon::ListChecks));
    // Its rail button shows it, which the plugin hears of; again hides it.
    dock.update_in(cx, |dock, window, cx| dock.rail_clicked(id, window, cx));
    cx.executor().advance_clock(MIN_UPDATE_INTERVAL);
    draw(cx);
    assert_eq!(
        dock.read_with(cx, |dock, _| dock.state().active()),
        Some(id)
    );
    assert!(panel.read_with(cx, |panel, _| panel.is_shown()));
    let title = dock.read_with(cx, |dock, cx| dock.state().title(cx));
    assert_eq!(title.as_deref(), Some("TODOs"));
    dock.update_in(cx, |dock, window, cx| dock.rail_clicked(id, window, cx));
    cx.executor().advance_clock(MIN_UPDATE_INTERVAL);
    draw(cx);
    assert!(!panel.read_with(cx, |panel, _| panel.is_shown()));
    assert_eq!(showings(&events), [PanelEvent::Shown, PanelEvent::Hidden]);
    // Unloaded, its panels leave the dock.
    workbench.update(cx, |workbench, cx| {
        workbench.remove_plugin_panels(PLUGIN, cx)
    });
    draw(cx);
    let rail = dock.read_with(cx, |dock, _| dock.state().drawn().rail_ids());
    assert!(!rail.contains(&id), "{rail:?}");
    assert!(!dock.read_with(cx, |dock, _| dock.state().panel_ids().contains(&id)));
}

#[gpui::test]
fn plugin_panel_declared_bottom_is_in_the_bottom_panel(cx: &mut TestAppContext) {
    let (workbench, panel, events, cx, _dir) = in_workbench(PanelLocation::Bottom, cx);
    let dock = workbench.read_with(cx, |workbench, _| workbench.dock().clone());
    let bottom = workbench.read_with(cx, |workbench, _| workbench.terminal_dock().clone());
    let id = panel_id(PLUGIN, PANEL);
    // In the bottom panel, once, not the sidebar; its button is the last at
    // the rail's foot, after the bottom tool windows'.
    let registered = bottom.read_with(cx, |bottom, _| bottom.dock().panel_ids());
    assert_eq!(
        registered.iter().filter(|on| **on == id).count(),
        1,
        "{registered:?}"
    );
    assert!(!dock.read_with(cx, |dock, _| dock.state().panel_ids().contains(&id)));
    let rail = dock.read_with(cx, |dock, _| dock.state().drawn().rail_ids());
    assert_eq!(rail.last(), Some(&id), "{rail:?}");
    assert_eq!(rail.iter().filter(|on| **on == id).count(), 1, "{rail:?}");
    let icons = dock.read_with(cx, |dock, _| dock.state().drawn().rail_icons());
    assert_eq!(icons.last(), Some(&fe_ui::Icon::ListChecks));
    // The button shows it in the bottom panel, lit, which the plugin hears
    // of; again hides it.
    let click = |cx: &mut VisualTestContext| {
        dock.update_in(cx, |dock, window, cx| {
            dock.action_clicked(id.0, crate::dock::PLUGIN_PANEL_COMMAND, window, cx)
        });
        cx.executor().advance_clock(MIN_UPDATE_INTERVAL);
        draw(cx);
    };
    click(cx);
    assert!(bottom.read_with(cx, |bottom, _| bottom.dock().is_shown(id)));
    assert!(bottom.read_with(cx, |bottom, _| bottom.is_visible()));
    let lit = dock.read_with(cx, |dock, _| dock.state().drawn().rail_highlighted());
    assert!(lit.contains(&id), "{lit:?}");
    assert!(panel.read_with(cx, |panel, _| panel.is_shown()));
    // The sidebar's dock showing doesn't make it shown.
    assert!(!dock.read_with(cx, |dock, _| dock.state().active() == Some(id)));
    click(cx);
    assert!(!bottom.read_with(cx, |bottom, _| bottom.dock().is_shown(id)));
    assert!(!panel.read_with(cx, |panel, _| panel.is_shown()));
    assert_eq!(showings(&events), [PanelEvent::Shown, PanelEvent::Hidden]);
    // Unloaded, it leaves the bottom panel and the rail's foot.
    workbench.update(cx, |workbench, cx| {
        workbench.remove_plugin_panels(PLUGIN, cx)
    });
    draw(cx);
    assert!(!bottom.read_with(cx, |bottom, _| bottom.dock().panel_ids().contains(&id)));
    let rail = dock.read_with(cx, |dock, _| dock.state().drawn().rail_ids());
    assert!(!rail.contains(&id), "{rail:?}");
}

/// A plugin, `acme.todos`, whose `init` sets its panel `todos`'s view to a
/// heading `Hello panel` (node `root`) through `panels.set-view`, failing
/// the load if that's refused; its `panel-handler.on-event` logs the event's
/// node id, or `shown` and `hidden`.
const TODOS: &str = r#"
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

  (import "fastedit:editor/panels@1.1.0" (instance $panels
    (alias outer $C $error (type $error))
    (type $tone (enum "normal" "muted" "accent" "success" "warning" "error"))
    (export "tone" (type $tone' (eq $tone)))
    (type $text-node (record
      (field "text" string) (field "tone" $tone') (field "mono" bool) (field "wrap" bool)))
    (export "text-node" (type $text-node' (eq $text-node)))
    (type $button-node (record
      (field "label" string) (field "icon" (option string)) (field "command" (option string))
      (field "enabled" bool)))
    (export "button-node" (type $button-node' (eq $button-node)))
    (type $toggle-node (record (field "label" string) (field "on" bool) (field "enabled" bool)))
    (export "toggle-node" (type $toggle-node' (eq $toggle-node)))
    (type $input-node (record
      (field "label" string) (field "value" string) (field "placeholder" (option string))))
    (export "input-node" (type $input-node' (eq $input-node)))
    (type $list-node (record (field "label" string) (field "multi-select" bool)))
    (export "list-node" (type $list-node' (eq $list-node)))
    (type $tree-item-node (record
      (field "label" string) (field "icon" (option string)) (field "expandable" bool)
      (field "expanded" bool) (field "detail" (option string)) (field "tone" $tone')))
    (export "tree-item-node" (type $tree-item-node' (eq $tree-item-node)))
    (type $cell (record (field "ch" string) (field "tone" $tone') (field "reverse" bool)))
    (export "cell" (type $cell' (eq $cell)))
    (type $grid-node (record
      (field "label" string) (field "cols" u8) (field "rows" u8) (field "cells" (list $cell'))))
    (export "grid-node" (type $grid-node' (eq $grid-node)))
    (type $node-kind (variant
      (case "column") (case "row") (case "scroll")
      (case "heading" string) (case "text" $text-node') (case "icon" string)
      (case "badge" $text-node') (case "button" $button-node') (case "toggle" $toggle-node')
      (case "input" $input-node') (case "list" $list-node') (case "tree-item" $tree-item-node')
      (case "separator") (case "progress" (option u8)) (case "grid" $grid-node')))
    (export "node-kind" (type $node-kind' (eq $node-kind)))
    (type $node (record
      (field "id" string) (field "kind" $node-kind') (field "children" (list u32))))
    (export "node" (type $node' (eq $node)))
    (type $view (record (field "nodes" (list $node'))))
    (export "view" (type $view' (eq $view)))
    (export "set-view" (func (param "panel" string) (param "view" $view')
      (result (result (error $error)))))))

  (core module $libc
    (memory (export "memory") 1)
    ;; 64.. holds set-view's result; 256.. the strings; 512.. the view's node; the heap 1024..
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
  (alias export $panels "set-view" (func $panels-set-view))
  (core func $log (canon lower (func $log-log) (memory $memory)))
  (core func $set-view
    (canon lower (func $panels-set-view) (memory $memory) (realloc $realloc)))
  (core instance $host
    (export "log" (func $log))
    (export "set-view" (func $set-view)))

  (core module $main
    (import "libc" "memory" (memory 1))
    (import "host" "log" (func $log (param i32 i32 i32)))
    (import "host" "set-view" (func $set-view (param i32 i32 i32 i32 i32)))

    (data (i32.const 256) "todos")
    (data (i32.const 272) "root")
    (data (i32.const 288) "Hello panel")
    (data (i32.const 304) "shown")
    (data (i32.const 312) "hidden")

    (func (export "init") (result i32)
      ;; A node is 60 bytes: its id (0), its kind (8: the case at 8, the
      ;; payload at 12) and its children (52), none. `heading` is case 3.
      (i32.store (i32.const 512) (i32.const 272))
      (i32.store (i32.const 516) (i32.const 4))
      (i32.store8 (i32.const 520) (i32.const 3))
      (i32.store (i32.const 524) (i32.const 288))
      (i32.store (i32.const 528) (i32.const 11))
      (call $set-view (i32.const 256) (i32.const 5) (i32.const 512) (i32.const 1) (i32.const 64))
      (i32.const 64))
    (func (export "shutdown"))
    (func (export "run-command") (param i32 i32 i32 i32) (result i32) (i32.const 0))
    (func (export "handle-events") (param i32 i32))
    ;; The panel's name, the event's case and its payload, flat: every case
    ;; but `shown` (6) and `hidden` (7) starts with a node id.
    (func (export "on-event")
      (param $panel i32) (param $panel-len i32) (param $case i32)
      (param $id i32) (param $id-len i32) (param i32) (param i32) (param i32) (param i32)
      (if (i32.eq (local.get $case) (i32.const 6))
        (then (return (call $log (i32.const 2) (i32.const 304) (i32.const 5)))))
      (if (i32.eq (local.get $case) (i32.const 7))
        (then (return (call $log (i32.const 2) (i32.const 312) (i32.const 6)))))
      (call $log (i32.const 2) (local.get $id) (local.get $id-len))))
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

/// The plugin [`TODOS`]'s id.
const TODOS_ID: &str = "acme.todos";

/// [`TODOS`] installed in `dir`'s extensions, declaring its panel `todos`
/// at `dock`, granted `panels`, as the host is handed it.
fn install_todos(dir: &std::path::Path, dock: &str) -> crate::plugin_host::Found {
    install_plugin(dir, dock, TODOS)
}

/// [`install_todos`] with the plugin `wat` in place of [`TODOS`].
fn install_plugin(dir: &std::path::Path, dock: &str, wat: &str) -> crate::plugin_host::Found {
    use kiln::core::{
        ApiVersion,
        extension::{ExtensionApi, load_manifest},
    };
    use kiln_plugin::{
        grants::GrantStore,
        manifest::{ExtensionOrigin, PluginManifest},
    };
    let extension = dir.join("extensions").join(TODOS_ID);
    std::fs::create_dir_all(&extension).unwrap();
    std::fs::write(extension.join("plugin.wasm"), wat::parse_str(wat).unwrap()).unwrap();
    std::fs::write(
        extension.join("extension.toml"),
        format!(
            "id = \"{TODOS_ID}\"\nname = \"{TODOS_ID}\"\nversion = \"1.0.0\"\n\
             license = \"MIT\"\n\n[engine]\ntenmoku = \"1.1\"\n\n\
             [contributes.plugin]\nwasm = \"plugin.wasm\"\ncapabilities = [\"panels\"]\n\n\
             [[contributes.plugin.panels]]\nid = \"todos\"\ntitle = \"TODOs\"\n\
             icon = \"list-checks\"\ndock = \"{dock}\"\n"
        ),
    )
    .unwrap();
    let grants: Grants = [Capability::Panels].into_iter().collect();
    GrantStore::new(dir.join("state").join(crate::plugin_host::GRANTS_FILE))
        .grant(TODOS_ID, (1, 0, 0), &grants, &[], false)
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

/// A workbench with no folder in a window of its own.
fn plugin_window(cx: &mut TestAppContext) -> Entity<crate::Workbench> {
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
    draw(vcx);
    workbench
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

/// What plugin [`TODOS`] logged.
fn todos_logged(cx: &mut TestAppContext) -> Vec<String> {
    let prefix = format!("log {TODOS_ID} on plugin {TODOS_ID}: ");
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

/// The plugin panel in `workbench`'s sidebar, if it has one.
fn in_sidebar(
    workbench: &Entity<crate::Workbench>,
    cx: &mut TestAppContext,
) -> Option<Entity<PluginPanel>> {
    workbench.read_with(cx, |workbench, cx| {
        workbench.dock().read(cx).state().panel::<PluginPanel>()
    })
}

#[gpui::test]
fn a_running_plugins_panel_shows_its_view_and_gets_its_events(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let dir = tempfile::tempdir().unwrap();
    let (state, cache) = (dir.path().join("state"), dir.path().join("cache"));
    cx.update(|cx| {
        fe_gpui::init(cx);
        crate::init(cx);
        crate::plugin_host::set_plugin_dirs(cx, state, cache);
        crate::plugin_host::set_preview(cx, true);
        let host = cx.global_mut::<crate::plugin_host::PluginHost>();
        host.limits.call_deadline = Duration::from_secs(30);
    });
    let found = install_todos(dir.path(), "left");
    let workbench = plugin_window(cx);
    cx.update(|cx| crate::plugin_host::set_plugins(cx, vec![found]));
    until_done(cx, "the plugin to run", |cx| {
        cx.update(|cx| {
            (crate::plugin_host::plugin_states(cx).iter()).any(|info| {
                info.id == TODOS_ID && info.state == crate::plugin_host::PluginState::Running
            })
        })
    });
    // In the sidebar, its view the one `init` set.
    let panel = in_sidebar(&workbench, cx).expect("the plugin's panel in the sidebar");
    assert_eq!(
        panel.read_with(cx, |panel, _| panel.dock_id()),
        panel_id(TODOS_ID, PANEL)
    );
    let view = panel.read_with(cx, |panel, _| panel.view().cloned());
    let view = view.expect("its view");
    assert_eq!(view.nodes.len(), 1);
    assert_eq!(view.nodes[0].id, "root");
    assert_eq!(view.nodes[0].kind, NodeKind::Heading("Hello panel".into()));
    // A window opened since has it too.
    let other = plugin_window(cx);
    let theirs = in_sidebar(&other, cx).expect("the panel in the new window");
    assert!(theirs.read_with(cx, |panel, _| panel.view().is_some()));
    // Shown by its rail button, then a row activated: the plugin's
    // `on-event` gets both, in order.
    let window = cx.windows()[0];
    let dock = workbench.read_with(cx, |workbench, _| workbench.dock().clone());
    let id = panel_id(TODOS_ID, PANEL);
    cx.update_window(window, |_, window, cx| {
        dock.update(cx, |dock, cx| dock.rail_clicked(id, window, cx))
    })
    .unwrap();
    until_done(cx, "the panel to show", |cx| !todos_logged(cx).is_empty());
    panel.update(cx, |panel, _| {
        panel.send(PanelEvent::Activated("root".into()))
    });
    until_done(cx, "the events' delivery", |cx| todos_logged(cx).len() >= 2);
    assert_eq!(todos_logged(cx), ["shown", "root"]);
    assert_eq!(super::events().queued(TODOS_ID), 0);
    // Unloaded, its panel leaves both windows and its view goes.
    cx.update(|cx| crate::plugin_host::unload_plugin(TODOS_ID, cx));
    until_done(cx, "the panels to go", |cx| {
        in_sidebar(&workbench, cx).is_none() && in_sidebar(&other, cx).is_none()
    });
    assert_eq!(super::views().view(TODOS_ID, PANEL), None);
}

/// `view` drawn by `panel` at once, as a new view of the plugin's is.
fn show_view(panel: &Entity<PluginPanel>, view: View, cx: &mut VisualTestContext) {
    panel.update(cx, |panel, cx| panel.take_view(Some(view), cx));
    draw(cx);
}

/// A view of list `list` (multi-select if `multi`) of a text row per id
/// in `rows`, under a column.
fn rows_view(rows: &[&str], multi: bool) -> View {
    let mut nodes = vec![
        n("root", NodeKind::Column, &[1]),
        n(
            "list",
            NodeKind::List(ListNode {
                label: "Rows".into(),
                multi_select: multi,
            }),
            &[],
        ),
    ];
    for row in rows {
        let at = nodes.len() as u32;
        nodes[1].children.push(at);
        nodes.push(n(row, text(row, Tone::Normal), &[]));
    }
    View { nodes }
}

fn cursor(panel: &Entity<PluginPanel>, list: &str, cx: &mut VisualTestContext) -> Option<String> {
    panel.read_with(cx, |panel, _| panel.lists[list].cursor.clone())
}

#[gpui::test]
fn a_list_keeps_its_cursor_on_its_node_across_views(cx: &mut TestAppContext) {
    let (panel, events, cx) = panel_with(rows_view(&["a", "b", "c"], false), cx);
    focus_list(&panel, "list", cx);
    press("down", cx);
    press("down", cx);
    assert_eq!(cursor(&panel, "list", cx).as_deref(), Some("b"));
    // Reordered, with a row more: the cursor and the selection stay on b,
    // and Down goes on from b's new place.
    show_view(&panel, rows_view(&["c", "b", "d", "a"], false), cx);
    assert_eq!(cursor(&panel, "list", cx).as_deref(), Some("b"));
    let selected = panel.read_with(cx, |panel, _| panel.lists["list"].selected.clone());
    assert_eq!(selected, ["b"]);
    drain(&events);
    press("down", cx);
    assert_eq!(
        drain(&events),
        [PanelEvent::Selected("list".into(), vec!["d".into()])]
    );
    // The last row selected, then the list cut to fewer nodes than the
    // cursor's old row was at: no row is the cursor's, nothing is selected,
    // and Down starts again at the top.
    press("end", cx);
    show_view(&panel, rows_view(&["x"], false), cx);
    assert_eq!(cursor(&panel, "list", cx), None);
    let selected = panel.read_with(cx, |panel, _| panel.lists["list"].selected.clone());
    assert!(selected.is_empty(), "{selected:?}");
    drain(&events);
    press("down", cx);
    assert_eq!(
        drain(&events),
        [PanelEvent::Selected("list".into(), vec!["x".into()])]
    );
    // The selected row taken out, the others kept.
    show_view(&panel, rows_view(&["y", "z"], false), cx);
    assert_eq!(cursor(&panel, "list", cx), None);
}

/// A tree outside a list: under a column after a heading, a folder `big`
/// of 5,000 files, expanded, and a folder `small` of one, collapsed.
fn loose_tree() -> View {
    let mut nodes = vec![
        n("root", NodeKind::Column, &[1, 2, 3]),
        n("title", NodeKind::Heading("Files".into()), &[]),
        n("big", item("big", true), &[]),
        n("small", item("small", false), &[4]),
        n("only", leaf("only"), &[]),
    ];
    for ix in 0..5_000 {
        let at = nodes.len() as u32;
        nodes[2].children.push(at);
        let id = format!("f{ix}");
        nodes.push(n(&id, leaf(&id), &[]));
    }
    View { nodes }
}

/// Clicks the middle of node `id`'s row or field.
fn click_node(id: &str, cx: &mut VisualTestContext) {
    let selector: &'static str = super::selector(id).leak();
    let bounds = cx.debug_bounds(selector).expect("drawn");
    cx.simulate_click(bounds.center(), gpui::Modifiers::none());
    draw(cx);
}

#[gpui::test]
fn a_tree_outside_a_list_is_virtualized_and_works_by_mouse_and_keys(cx: &mut TestAppContext) {
    let (panel, events, cx) = panel_with(loose_tree(), cx);
    // One tree of the two folders' run, a screenful of its rows built.
    let rows = panel.read_with(cx, |panel, _| panel.lists["big"].rows.len());
    assert_eq!(rows, 5_002);
    let drawn = panel.read_with(cx, |panel, _| panel.drawn_rows["big"].clone());
    assert!(drawn.len() < 100, "built {drawn:?} of {rows} rows");
    let nodes = a11y(cx);
    let tree = gpui::ElementId::Name("plugin-tree:big".into());
    assert_eq!(
        named(&nodes, tree),
        Some((
            Role::Tree,
            tr!("workbench-extensions-panel-tree").to_string()
        ))
    );
    let items = (nodes.iter())
        .filter(|node| node.role == Role::TreeItem)
        .count();
    assert!(items < 100, "{items} tree item nodes for {rows} rows");
    // A click selects a row, and focuses the tree for the keys: End goes
    // to the last row, Right expands it, and Enter activates its child.
    click_node("big", cx);
    assert_eq!(
        drain(&events),
        [PanelEvent::Selected("big".into(), vec!["big".into()])]
    );
    press("end", cx);
    press("right", cx);
    press("right", cx);
    press("enter", cx);
    assert_eq!(
        drain(&events),
        [
            PanelEvent::Selected("big".into(), vec!["small".into()]),
            PanelEvent::Expanded("small".into(), true),
            PanelEvent::Selected("big".into(), vec!["only".into()]),
            PanelEvent::Activated("only".into()),
        ]
    );
    // A double click activates a row.
    let selector: &'static str = super::selector("only").leak();
    let at = cx.debug_bounds(selector).expect("drawn").center();
    cx.simulate_click(at, gpui::Modifiers::none());
    cx.simulate_event(gpui::MouseDownEvent {
        position: at,
        button: gpui::MouseButton::Left,
        modifiers: gpui::Modifiers::none(),
        click_count: 2,
        first_mouse: false,
    });
    cx.simulate_event(gpui::MouseUpEvent {
        position: at,
        button: gpui::MouseButton::Left,
        modifiers: gpui::Modifiers::none(),
        click_count: 2,
    });
    draw(cx);
    assert!(
        drain(&events).contains(&PanelEvent::Activated("only".into())),
        "a double click activates"
    );
}

#[gpui::test]
fn retained_inputs_take_the_views_changes_once_left(cx: &mut TestAppContext) {
    let (panel, events, cx) = panel_with(every_kind(), cx);
    // GPUI tells of focus leaving only in an active window.
    cx.update(|window, _| window.activate_window());
    let with_filter = |value: &str, label: &str, placeholder: &str| {
        let mut view = every_kind();
        view.nodes[6].kind = NodeKind::Input(InputNode {
            label: label.into(),
            value: value.into(),
            placeholder: Some(placeholder.into()),
        });
        view
    };
    let field = |panel: &Entity<PluginPanel>, cx: &mut VisualTestContext| {
        panel.read_with(cx, |panel, _| panel.inputs["filter"].field.clone())
    };
    let value = |panel: &Entity<PluginPanel>, cx: &mut VisualTestContext| {
        let field = field(panel, cx);
        field.read_with(cx, |field, _| field.value().to_owned())
    };
    let label = |cx: &mut VisualTestContext| {
        (a11y(cx).into_iter())
            .find(|node| node.role == Role::TextInput)
            .map(|node| node.label.to_string())
    };
    // Not focused: a new value, label and placeholder show at once.
    show_view(&panel, with_filter("abc", "Find", "Find TODOs"), cx);
    assert_eq!(value(&panel, cx), "abc");
    assert_eq!(label(cx).as_deref(), Some("Find"));
    // Focused, typing: the view's changes wait.
    let focus = field(&panel, cx).read_with(cx, |field, cx| field.focus_handle(cx));
    cx.update(|window, cx| window.focus(&focus, cx));
    draw(cx);
    cx.simulate_input("x");
    show_view(&panel, with_filter("zzz", "Search", "Search TODOs"), cx);
    assert_eq!(value(&panel, cx), "abcx");
    assert_eq!(label(cx).as_deref(), Some("Find"));
    // Left, what was typed goes, and then the view's held changes apply.
    let root = panel.read_with(cx, |panel, cx| panel.focus_handle(cx));
    cx.update(|window, cx| window.focus(&root, cx));
    draw(cx);
    assert_eq!(
        drain(&events),
        [PanelEvent::Input("filter".into(), "abcx".into())]
    );
    assert_eq!(value(&panel, cx), "zzz");
    assert_eq!(label(cx).as_deref(), Some("Search"));
    // A view whose value didn't change since leaves what the user typed.
    let focus = field(&panel, cx).read_with(cx, |field, cx| field.focus_handle(cx));
    cx.update(|window, cx| window.focus(&focus, cx));
    draw(cx);
    cx.simulate_input("y");
    cx.update(|window, cx| window.focus(&root, cx));
    draw(cx);
    show_view(&panel, with_filter("zzz", "Search", "Search TODOs"), cx);
    assert_eq!(value(&panel, cx), "zzzy");
}

#[gpui::test]
fn an_input_in_a_list_row_takes_a_click_and_typing(cx: &mut TestAppContext) {
    let view = View {
        nodes: vec![
            n("root", NodeKind::Column, &[1]),
            n("list", list("Rows"), &[2, 3]),
            n("top", text("top", Tone::Normal), &[]),
            n(
                "query",
                NodeKind::Input(InputNode {
                    label: "Query".into(),
                    value: String::new(),
                    placeholder: None,
                }),
                &[],
            ),
        ],
    };
    let (panel, _, cx) = panel_with(view, cx);
    // Focus handed on from a row to its list is told only in an active
    // window, as the app's are.
    cx.update(|window, _| window.activate_window());
    click_node("query", cx);
    let field = panel.read_with(cx, |panel, _| panel.inputs["query"].field.clone());
    let focus = field.read_with(cx, |field, cx| field.focus_handle(cx));
    assert!(cx.update(|window, _| focus.is_focused(window)));
    cx.simulate_input("hi");
    assert_eq!(
        field.read_with(cx, |field, _| field.value().to_owned()),
        "hi"
    );
    // A click on another row focuses the list.
    click_node("top", cx);
    let list = panel.read_with(cx, |panel, _| panel.lists["list"].focus.clone());
    assert!(cx.update(|window, _| list.is_focused(window)));
}

#[gpui::test]
fn keyboard_selects_several_rows_in_a_multi_select_list(cx: &mut TestAppContext) {
    let (panel, events, cx) = panel_with(rows_view(&["a", "b", "c", "d"], true), cx);
    let selected = |ids: &[&str]| {
        PanelEvent::Selected("list".into(), ids.iter().map(|id| (*id).into()).collect())
    };
    focus_list(&panel, "list", cx);
    // Shift with a move extends from the anchor; back up shrinks it.
    press("down", cx);
    press("shift-down", cx);
    press("shift-down", cx);
    press("shift-up", cx);
    assert_eq!(
        drain(&events),
        [
            selected(&["a"]),
            selected(&["a", "b"]),
            selected(&["a", "b", "c"]),
            selected(&["a", "b"]),
        ]
    );
    // The command key moves the cursor alone; Space takes its row out,
    // then adds another.
    press("secondary-up", cx);
    assert_eq!(cursor(&panel, "list", cx).as_deref(), Some("a"));
    assert_eq!(drain(&events), []);
    press("space", cx);
    press("secondary-down", cx);
    press("secondary-down", cx);
    press("secondary-down", cx);
    press("space", cx);
    assert_eq!(drain(&events), [selected(&["b"]), selected(&["b", "d"])]);
    // A plain move selects its row alone again.
    press("up", cx);
    assert_eq!(drain(&events), [selected(&["c"])]);
    // A single-select list ignores the modified moves.
    show_view(&panel, rows_view(&["a", "b", "c", "d"], false), cx);
    drain(&events);
    press("shift-down", cx);
    press("secondary-down", cx);
    assert_eq!(drain(&events), []);
}

/// Runs command `id` as its key would, from the workbench.
fn run(id: &str, cx: &mut VisualTestContext) {
    use fe_command::{ContextFrame, ContextStack, Invocation};
    let stack = ContextStack::new().with(ContextFrame::new(crate::KEY_CONTEXT));
    let run = fe_gpui::commands::RunCommand(Invocation::of(id).unwrap().context(stack));
    cx.update(|window, cx| window.dispatch_action(Box::new(run), cx));
    draw(cx);
}

fn focused_in(focus: &gpui::FocusHandle, cx: &mut VisualTestContext) -> bool {
    cx.update(|window, cx| focus.contains_focused(window, cx))
}

#[gpui::test]
fn dock_focus_commands_enter_and_leave_a_sidebar_plugin_panel(cx: &mut TestAppContext) {
    let (workbench, panel, _events, cx, _dir) = in_workbench(PanelLocation::Left, cx);
    cx.update(|window, _| window.activate_window());
    let dock = workbench.read_with(cx, |workbench, _| workbench.dock().clone());
    let id = panel_id(PLUGIN, PANEL);
    let mine = panel.read_with(cx, |panel, cx| panel.focus_handle(cx));
    let editor = workbench.read_with(cx, |workbench, cx| workbench.focus_handle(cx));
    dock.update_in(cx, |dock, window, cx| dock.rail_clicked(id, window, cx));
    draw(cx);
    assert!(focused_in(&mine, cx));
    // From the panel toward the editor.
    run("dock.focus_right", cx);
    assert!(focused_in(&editor, cx));
    // From the bottom panel over to the sidebar, which shows the panel.
    run("problems.show", cx);
    assert!(!focused_in(&mine, cx));
    run("dock.focus_left", cx);
    assert!(focused_in(&mine, cx));
    // And down from the panel into the bottom panel.
    run("dock.focus_down", cx);
    assert!(!focused_in(&mine, cx));
    assert!(!focused_in(&editor, cx));
}

#[gpui::test]
fn dock_focus_commands_enter_and_leave_a_bottom_plugin_panel(cx: &mut TestAppContext) {
    let (workbench, panel, _events, cx, _dir) = in_workbench(PanelLocation::Bottom, cx);
    cx.update(|window, _| window.activate_window());
    let dock = workbench.read_with(cx, |workbench, _| workbench.dock().clone());
    let id = panel_id(PLUGIN, PANEL);
    let mine = panel.read_with(cx, |panel, cx| panel.focus_handle(cx));
    dock.update_in(cx, |dock, window, cx| {
        dock.action_clicked(id.0, crate::dock::PLUGIN_PANEL_COMMAND, window, cx)
    });
    draw(cx);
    // Down from the sidebar's tree into the bottom panel's plugin panel,
    // which the workbench then knows has focus.
    run("dock.show_project", cx);
    let project = workbench.read_with(cx, |workbench, cx| workbench.panel().focus_handle(cx));
    assert!(focused_in(&project, cx));
    run("dock.focus_down", cx);
    assert!(focused_in(&mine, cx));
    let bottom = cx.update(|window, cx| workbench.read(cx).bottom_panel_focused(window, cx));
    assert!(bottom);
    // Up goes back where the key came from.
    run("dock.focus_up", cx);
    assert!(focused_in(&project, cx));
    // Entered by the bottom panel's own focus, up from it is the editor.
    workbench.update_in(cx, |workbench, window, cx| {
        assert!(workbench.focus_bottom_panel(window, cx));
    });
    draw(cx);
    assert!(focused_in(&mine, cx));
}

#[gpui::test]
fn a_plugin_refusing_a_toggle_or_expansion_with_the_same_view_wins(cx: &mut TestAppContext) {
    let (panel, events, cx) = panel_with(every_kind(), cx);
    let views = panel.read_with(cx, |panel, _| panel.views.clone());
    let state = slice(&views);
    state
        .set_view_at(PANEL, every_kind(), Instant::now())
        .unwrap();
    cx.executor().advance_clock(MIN_UPDATE_INTERVAL);
    draw(cx);
    let checked = |cx: &mut VisualTestContext| {
        (a11y(cx).into_iter())
            .find(|node| node.id == element_id("mine"))
            .and_then(|node| node.states.toggled)
    };
    let rows = |panel: &Entity<PluginPanel>, cx: &mut VisualTestContext| {
        panel.read_with(cx, |panel, _| panel.lists["files"].rows.len())
    };
    assert_eq!((checked(cx), rows(&panel, cx)), (Some(false), 3));
    // The user turns the toggle on and collapses lib.rs: shown at once.
    panel.update(cx, |panel, cx| {
        panel.toggle("mine", true, cx);
        panel.expand_row("files", 0, false, cx);
    });
    draw(cx);
    assert_eq!((checked(cx), rows(&panel, cx)), (Some(true), 2));
    assert_eq!(
        drain(&events),
        [
            PanelEvent::Toggled("mine".into(), true),
            PanelEvent::Expanded("lib".into(), false),
        ]
    );
    // The plugin answers with the view it had: both go back.
    std::thread::sleep(MIN_UPDATE_INTERVAL + Duration::from_millis(5));
    state
        .set_view_at(PANEL, every_kind(), Instant::now())
        .unwrap();
    cx.executor().advance_clock(MIN_UPDATE_INTERVAL);
    draw(cx);
    assert_eq!((checked(cx), rows(&panel, cx)), (Some(false), 3));
}

#[gpui::test]
fn tab_reaches_an_input_both_ways_and_keys_work_in_it(cx: &mut TestAppContext) {
    let (panel, events, cx) = panel_with(every_kind(), cx);
    let field = |panel: &Entity<PluginPanel>, cx: &mut VisualTestContext| {
        let field = panel.read_with(cx, |panel, _| panel.inputs["filter"].field.clone());
        field.read_with(cx, |field, cx| field.focus_handle(cx))
    };
    let root = panel.read_with(cx, |panel, cx| panel.focus_handle(cx));
    let walk = |panel: &Entity<PluginPanel>, cx: &mut VisualTestContext| {
        cx.update(|window, cx| window.focus(&root, cx));
        draw(cx);
        // The button, the toggle, then the input.
        press("tab", cx);
        press("tab", cx);
        press("tab", cx);
        let focus = field(panel, cx);
        cx.update(|window, _| focus.is_focused(window))
    };
    assert!(walk(&panel, cx), "Tab reaches the input");
    cx.simulate_input("ab");
    press("enter", cx);
    assert_eq!(
        drain(&events),
        [PanelEvent::Submitted("filter".into(), "ab".into())]
    );
    // Shift-Tab goes back to the toggle, which Space switches.
    press("shift-tab", cx);
    let focus = field(&panel, cx);
    assert!(!cx.update(|window, _| focus.is_focused(window)));
    press("space", cx);
    assert_eq!(drain(&events), [PanelEvent::Toggled("mine".into(), true)]);
    // A field made again for a new label is a tab stop too.
    let mut view = every_kind();
    view.nodes[6].kind = NodeKind::Input(InputNode {
        label: "Find".into(),
        value: String::new(),
        placeholder: None,
    });
    show_view(&panel, view, cx);
    assert!(walk(&panel, cx), "Tab reaches the field made again");
}

/// A list of `count` rows: a long wrapped text first, then a column of two
/// texts, then one-line texts.
fn tall_rows(count: usize) -> View {
    let long = "a long line that wraps ".repeat(200);
    let mut nodes = vec![
        n("root", NodeKind::Column, &[1]),
        n("list", list("Rows"), &[2, 3]),
        n(
            "wrapped",
            NodeKind::Text(TextNode {
                text: long,
                tone: Tone::Normal,
                mono: false,
                wrap: true,
            }),
            &[],
        ),
        n("pair", NodeKind::Column, &[4, 5]),
        n("upper", text("upper", Tone::Normal), &[]),
        n("lower", text("lower", Tone::Muted), &[]),
    ];
    for ix in 0..count {
        let at = nodes.len() as u32;
        nodes[1].children.push(at);
        let id = format!("r{ix}");
        nodes.push(n(&id, text(&id, Tone::Normal), &[]));
    }
    View { nodes }
}

fn bounds_of(id: &str, cx: &mut VisualTestContext) -> gpui::Bounds<gpui::Pixels> {
    let selector: &'static str = super::selector(id).leak();
    cx.debug_bounds(selector).expect("drawn")
}

#[gpui::test]
fn list_rows_are_as_tall_as_their_content(cx: &mut TestAppContext) {
    let (panel, events, cx) = panel_with(tall_rows(2_000), cx);
    cx.update(|window, _| window.activate_window());
    // Rows are measured, each at least a row high, none overlapping.
    let wrapped = bounds_of("wrapped", cx);
    let pair = bounds_of("pair", cx);
    let first = bounds_of("r0", cx);
    assert!(wrapped.size.height > fe_ui::ROW_HEIGHT * 2., "{wrapped:?}");
    assert!(pair.size.height > fe_ui::ROW_HEIGHT * 1.5, "{pair:?}");
    assert!(first.size.height >= fe_ui::ROW_HEIGHT, "{first:?}");
    assert!(
        pair.origin.y >= wrapped.origin.y + wrapped.size.height,
        "{wrapped:?} {pair:?}"
    );
    assert!(
        first.origin.y >= pair.origin.y + pair.size.height,
        "{pair:?} {first:?}"
    );
    // Still a screenful built of 2,002 rows.
    let drawn = panel.read_with(cx, |panel, _| panel.drawn_rows["list"].clone());
    assert!(drawn.len() < 100, "built {drawn:?}");
    // A click on the tall row selects it; End scrolls to the last.
    let at = pair.origin + gpui::point(gpui::px(20.), pair.size.height - gpui::px(4.));
    cx.simulate_click(at, gpui::Modifiers::none());
    draw(cx);
    assert_eq!(
        drain(&events),
        [PanelEvent::Selected("list".into(), vec!["pair".into()])]
    );
    press("end", cx);
    draw(cx);
    let drawn = panel.read_with(cx, |panel, _| panel.drawn_rows["list"].clone());
    assert!(drawn.contains(&2_001), "built {drawn:?}");
    assert!(drawn.len() < 100, "built {drawn:?}");
    assert_eq!(
        drain(&events),
        [PanelEvent::Selected("list".into(), vec!["r1999".into()])]
    );
}

#[gpui::test]
fn unknown_icon_names_kept_are_bounded(cx: &mut TestAppContext) {
    let (panel, _, cx) = panel_with(every_kind(), cx);
    for round in 0..20 {
        let mut nodes = vec![n("root", NodeKind::Column, &[])];
        for ix in 0..10 {
            let at = nodes.len() as u32;
            nodes[0].children.push(at);
            let name = format!("no-such-icon-{round}-{ix}-{}", "x".repeat(200));
            nodes.push(n(&format!("i{ix}"), NodeKind::Icon(name), &[]));
        }
        show_view(&panel, View { nodes }, cx);
    }
    let kept = panel.read_with(cx, |panel, _| {
        (
            panel.unknown_icons.len(),
            panel.unknown_icons.iter().map(String::len).max(),
        )
    });
    assert_eq!(kept.0, super::MAX_LOGGED_ICONS);
    assert!(kept.1.unwrap() <= 64, "{kept:?}");
}

#[gpui::test]
fn a_plugin_whose_init_fails_after_setting_a_view_leaves_no_panel_state(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let dir = tempfile::tempdir().unwrap();
    let (state, cache) = (dir.path().join("state"), dir.path().join("cache"));
    cx.update(|cx| {
        fe_gpui::init(cx);
        crate::init(cx);
        crate::plugin_host::set_plugin_dirs(cx, state, cache);
        crate::plugin_host::set_preview(cx, true);
    });
    // TODOS, trapping once its `init` has set the view.
    let set = "(call $set-view (i32.const 256) (i32.const 5) (i32.const 512) (i32.const 1) (i32.const 64))\n      (i32.const 64))";
    assert!(TODOS.contains(set));
    let failing = TODOS.replace(
        set,
        "(call $set-view (i32.const 256) (i32.const 5) (i32.const 512) (i32.const 1) (i32.const 64))\n      unreachable)",
    );
    let found = install_plugin(dir.path(), "left", &failing);
    let workbench = plugin_window(cx);
    cx.update(|cx| crate::plugin_host::set_plugins(cx, vec![found]));
    until_done(cx, "the plugin's load to fail", |cx| {
        cx.update(|cx| {
            (crate::plugin_host::plugin_states(cx).iter()).any(|info| {
                info.id == TODOS_ID
                    && matches!(&info.state,
                        crate::plugin_host::PluginState::Stopped(diagnostic)
                            if diagnostic.reason == crate::plugin_host::StopReason::Load)
            })
        })
    });
    // Its view went with the load: nothing of it is kept, and no window
    // has its panel.
    let views = super::views();
    assert_eq!(views.view_with_revision(TODOS_ID, PANEL), (None, 0));
    assert_eq!(super::events().queued(TODOS_ID), 0);
    assert!(in_sidebar(&workbench, cx).is_none());
}

fn scrolled_to(panel: &Entity<PluginPanel>, cx: &mut VisualTestContext) -> (usize, String) {
    let (top, id, _) = scrolled_by(panel, cx);
    (top, id)
}

/// The top row of `panel`'s measured list `list`, its node, and how far
/// into it the list is scrolled.
fn scrolled_by(
    panel: &Entity<PluginPanel>,
    cx: &mut VisualTestContext,
) -> (usize, String, gpui::Pixels) {
    panel.read_with(cx, |panel, _| {
        let state = &panel.lists["list"];
        let top = state.measured.logical_scroll_top();
        (
            top.item_ix,
            state.ids[top.item_ix].clone(),
            top.offset_in_item,
        )
    })
}

#[gpui::test]
fn a_measured_list_stays_where_it_was_scrolled_across_new_views(cx: &mut TestAppContext) {
    let (panel, _, cx) = panel_with(tall_rows(2_000), cx);
    panel.update(cx, |panel, _| {
        panel.lists["list"].measured.scroll_to(gpui::ListOffset {
            item_ix: 1_000,
            offset_in_item: gpui::px(5.),
        })
    });
    draw(cx);
    // Rows: wrapped, pair, then r0 on: row 1,000 is r998, 5 px into it.
    let at = (1_000, "r998".to_owned(), gpui::px(5.));
    assert_eq!(scrolled_by(&panel, cx), at);
    // The same view again.
    show_view(&panel, tall_rows(2_000), cx);
    assert_eq!(scrolled_by(&panel, cx), at);
    // Another row's text changed.
    let mut view = tall_rows(2_000);
    let id = |view: &View, id: &str| (view.nodes.iter()).position(|node| node.id == id).unwrap();
    let r3 = id(&view, "r3");
    view.nodes[r3].kind = text("r3, changed", Tone::Accent);
    show_view(&panel, view, cx);
    assert_eq!(scrolled_by(&panel, cx), at);
    // The top row grown many times over: still 5 px into it, not scaled.
    let mut view = tall_rows(2_000);
    let r998 = id(&view, "r998");
    view.nodes[r998].kind = NodeKind::Text(TextNode {
        text: "r998 grown long ".repeat(300),
        tone: Tone::Normal,
        mono: false,
        wrap: true,
    });
    show_view(&panel, view, cx);
    assert_eq!(scrolled_by(&panel, cx), at);
    // Rows taken out above it and one moved up: still r998, 5 px in.
    let mut view = tall_rows(2_000);
    let gone: Vec<u32> = (0..10)
        .map(|ix| id(&view, &format!("r{ix}")) as u32)
        .collect();
    let last = id(&view, "r1999") as u32;
    let children = &mut view.nodes[1].children;
    children.retain(|child| !gone.contains(child) && *child != last);
    children.insert(2, last);
    show_view(&panel, view, cx);
    assert_eq!(
        scrolled_by(&panel, cx),
        (991, "r998".to_owned(), gpui::px(5.))
    );
}

#[gpui::test]
fn a_focused_input_scrolled_out_of_a_list_still_takes_keys(cx: &mut TestAppContext) {
    let mut view = rows_view(&[], false);
    let at = view.nodes.len() as u32;
    view.nodes[1].children.push(at);
    view.nodes
        .push(n("first", text("first", Tone::Normal), &[]));
    let at = view.nodes.len() as u32;
    view.nodes[1].children.push(at);
    view.nodes.push(n(
        "q",
        NodeKind::Input(InputNode {
            label: "Query".into(),
            value: String::new(),
            placeholder: None,
        }),
        &[],
    ));
    for ix in 0..1_000 {
        let at = view.nodes.len() as u32;
        view.nodes[1].children.push(at);
        let id = format!("r{ix}");
        view.nodes.push(n(&id, text(&id, Tone::Normal), &[]));
    }
    // Every row a tree item for a view (drawn by `uniform_list`), then
    // measured again with the same rows: their focus is registered again.
    let mut uniform = view.clone();
    for at in uniform.nodes[1].children.clone() {
        let id = uniform.nodes[at as usize].id.clone();
        uniform.nodes[at as usize].kind = leaf(&id);
    }
    let (panel, events, cx) = panel_with(view.clone(), cx);
    cx.update(|window, _| window.activate_window());
    show_view(&panel, uniform.clone(), cx);
    assert!(panel.read_with(cx, |panel, _| panel.lists["list"].uniform));
    show_view(&panel, view.clone(), cx);
    assert!(!panel.read_with(cx, |panel, _| panel.lists["list"].uniform));
    // Scrolled far below it: its row is still drawn, and still focused.
    panel.update(cx, |panel, _| {
        panel.lists["list"].measured.scroll_to(gpui::ListOffset {
            item_ix: 900,
            offset_in_item: gpui::px(3.),
        })
    });
    draw(cx);
    assert_eq!(scrolled_to(&panel, cx).0, 900);
    // Uniform and measured again with the same ids: the top row and its
    // offset are kept.
    let (_, top_id, offset) = scrolled_by(&panel, cx);
    show_view(&panel, uniform.clone(), cx);
    assert!(panel.read_with(cx, |panel, _| panel.lists["list"].uniform));
    show_view(&panel, view.clone(), cx);
    draw(cx);
    let (_, id, back) = scrolled_by(&panel, cx);
    assert_eq!(id, top_id, "same top row id");
    assert_eq!(back, offset, "same pixel offset");
    panel.update(cx, |panel, _| {
        panel.lists["list"].measured.scroll_to(gpui::ListOffset {
            item_ix: 0,
            offset_in_item: gpui::px(0.),
        })
    });
    draw(cx);
    click_node("q", cx);
    cx.simulate_input("a");
    let field = panel.read_with(cx, |panel, _| panel.inputs["q"].field.clone());
    let focus = field.read_with(cx, |field, cx| field.focus_handle(cx));
    // Scrolled far below it: its row is still drawn, and still focused.
    panel.update(cx, |panel, _| {
        panel.lists["list"].measured.scroll_to(gpui::ListOffset {
            item_ix: 900,
            offset_in_item: gpui::px(0.),
        })
    });
    draw(cx);
    assert_eq!(scrolled_to(&panel, cx).0, 900);
    let drawn = panel.read_with(cx, |panel, _| panel.drawn_rows["list"].clone());
    assert!(drawn.contains(&1), "the input's row drawn: {drawn:?}");
    assert!(cx.update(|window, _| focus.is_focused(window)));
    // Typing, Enter and Tab still reach it.
    cx.simulate_input("b");
    assert_eq!(
        field.read_with(cx, |field, _| field.value().to_owned()),
        "ab"
    );
    press("enter", cx);
    assert!(
        drain(&events).contains(&PanelEvent::Submitted("q".into(), "ab".into())),
        "submitted"
    );
    press("tab", cx);
    assert!(
        !cx.update(|window, _| focus.is_focused(window)),
        "Tab moves on"
    );
    assert!(cx.update(|window, cx| window.focused(cx).is_some()));
}

#[gpui::test]
fn keys_on_a_control_in_a_list_row_reach_only_that_control(cx: &mut TestAppContext) {
    let view = View {
        nodes: vec![
            n("root", NodeKind::Column, &[1]),
            n(
                "list",
                NodeKind::List(ListNode {
                    label: "Rows".into(),
                    multi_select: true,
                }),
                &[2, 3, 4],
            ),
            n("a", text("a", Tone::Normal), &[]),
            n("go", button("Go"), &[]),
            n(
                "on",
                NodeKind::Toggle(ToggleNode {
                    label: "On".into(),
                    on: false,
                    enabled: true,
                }),
                &[],
            ),
        ],
    };
    let (panel, events, cx) = panel_with(view, cx);
    cx.update(|window, _| window.activate_window());
    focus_list(&panel, "list", cx);
    press("down", cx);
    assert_eq!(
        drain(&events),
        [PanelEvent::Selected("list".into(), vec!["a".into()])]
    );
    // Tab to the button in the second row: Enter clicks it, and doesn't
    // activate the list's cursor row too.
    press("tab", cx);
    let list = panel.read_with(cx, |panel, _| panel.lists["list"].focus.clone());
    assert!(!cx.update(|window, _| list.is_focused(window)));
    press("enter", cx);
    assert_eq!(drain(&events), [PanelEvent::Activated("go".into())]);
    // On to the toggle: Space switches it, and leaves the selection alone.
    press("tab", cx);
    press("space", cx);
    assert_eq!(drain(&events), [PanelEvent::Toggled("on".into(), true)]);
    // Back in the list, its keys work again.
    focus_list(&panel, "list", cx);
    press("enter", cx);
    assert_eq!(drain(&events), [PanelEvent::Activated("a".into())]);
}

#[gpui::test]
fn a_list_made_single_select_keeps_one_selected_row(cx: &mut TestAppContext) {
    let (panel, events, cx) = panel_with(rows_view(&["a", "b", "c", "d"], true), cx);
    focus_list(&panel, "list", cx);
    press("down", cx);
    press("shift-down", cx);
    press("shift-down", cx);
    drain(&events);
    let selection = |panel: &Entity<PluginPanel>, cx: &mut VisualTestContext| {
        panel.read_with(cx, |panel, _| {
            let list = &panel.lists["list"];
            (
                list.selected.clone(),
                list.cursor.clone(),
                list.anchor.clone(),
            )
        })
    };
    let some = |id: &str| Some(id.to_owned());
    assert_eq!(
        selection(&panel, cx),
        (
            vec!["a".into(), "b".into(), "c".into()],
            some("c"),
            some("a")
        )
    );
    // The same rows, single-select now: the cursor's row stays selected,
    // alone, and is the anchor too.
    show_view(&panel, rows_view(&["a", "b", "c", "d"], false), cx);
    assert_eq!(
        selection(&panel, cx),
        (vec!["c".into()], some("c"), some("c"))
    );
    let selected = |nodes: &[fe_ui::Node]| {
        (nodes.iter())
            .filter(|node| node.states.selected == Some(true))
            .count()
    };
    assert_eq!(selected(&a11y(cx)), 1);
    // A move selects from there.
    press("up", cx);
    assert_eq!(
        drain(&events),
        [PanelEvent::Selected("list".into(), vec!["b".into()])]
    );
}
