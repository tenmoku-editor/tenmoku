//! A plugin's panel in the sidebar, dark (Yohen) and light (Hakuji)
//! (T1708): a TODOs view a plugin could set, of a heading, a badge, a
//! button, an input, a toggle, a progress bar, a separator and a tree of
//! files and their TODOs, each in its tone's theme colour.

use std::time::Instant;

use fe_command::capability::{Capability, Grants};
use fe_config::themes;
use fe_plugin::fastedit::editor::panels::{
    ButtonNode, InputNode, ListNode, Node, NodeKind, TextNode, ToggleNode, TreeItemNode,
};
use fe_plugin::panels::{PanelViews, State, Tone, View};
use fe_workbench::{PanelEvents, PluginPanel};
use gpui::{AppContext as _, Pixels, Size, VisualTestAppContext, px, size};
use kiln_platform::screenshots::Result;
use kiln_plugin::manifest::{PanelDecl, PanelLocation};
use kiln_shell::DockView as _;

use crate::scenes::{capture_with, goldens, use_theme, write_project};

/// The sidebar and a little of the editor area beside it.
const PANEL_SIZE: Size<Pixels> = size(px(720.), px(480.));

const PLUGIN: &str = "tenmoku.todos";
const PANEL: &str = "todos";

/// Each scene: its golden and its bundled theme.
const SCENES: [(&str, &str); 2] = [
    ("plugin_panel_dark", themes::DEFAULT_DARK),
    ("plugin_panel_light", themes::DEFAULT_LIGHT),
];

fn node(id: &str, kind: NodeKind, children: &[u32]) -> Node {
    Node {
        id: id.into(),
        kind,
        children: children.to_vec(),
    }
}

fn text(text: &str, tone: Tone) -> TextNode {
    TextNode {
        text: text.into(),
        tone,
        mono: false,
        wrap: false,
    }
}

fn file(label: &str, expanded: bool) -> NodeKind {
    NodeKind::TreeItem(TreeItemNode {
        label: label.into(),
        icon: Some("file-code".into()),
        expandable: true,
        expanded,
        detail: None,
        tone: Tone::Normal,
    })
}

fn todo(label: &str, line: u32, tone: Tone) -> NodeKind {
    NodeKind::TreeItem(TreeItemNode {
        label: label.into(),
        icon: Some("list-checks".into()),
        expandable: false,
        expanded: false,
        detail: Some(format!(":{line}")),
        tone,
    })
}

/// The panel's view: every kind of node a TODOs panel would use.
fn view() -> View {
    View {
        nodes: vec![
            /* 0 */ node("root", NodeKind::Column, &[1, 2, 3, 4, 5, 6, 7, 8]),
            /* 1 */ node("title", NodeKind::Heading("TODOs in demo".into()), &[]),
            /* 2 */ node("bar", NodeKind::Row, &[9, 10, 11]),
            /* 3 */
            node(
                "filter",
                NodeKind::Input(InputNode {
                    label: "Filter".into(),
                    value: String::new(),
                    placeholder: Some("Filter TODOs".into()),
                }),
                &[],
            ),
            /* 4 */
            node(
                "mine",
                NodeKind::Toggle(ToggleNode {
                    label: "Only FIXMEs".into(),
                    on: true,
                    enabled: true,
                }),
                &[],
            ),
            /* 5 */ node("scan", NodeKind::Progress(Some(60)), &[]),
            /* 6 */ node("rule", NodeKind::Separator, &[]),
            /* 7 */
            node(
                "found",
                NodeKind::List(ListNode {
                    label: "TODOs".into(),
                    multi_select: false,
                }),
                &[12, 13, 14],
            ),
            /* 8 */
            node(
                "note",
                NodeKind::Text(text("Scanned 3 of 5 files", Tone::Muted)),
                &[],
            ),
            /* 9 */ node("count", NodeKind::Badge(text("4 open", Tone::Warning)), &[]),
            /* 10 */ node("done", NodeKind::Badge(text("2 done", Tone::Success)), &[]),
            /* 11 */
            node(
                "refresh",
                NodeKind::Button(ButtonNode {
                    label: "Rescan".into(),
                    icon: Some("refresh-cw".into()),
                    command: None,
                    enabled: true,
                }),
                &[],
            ),
            /* 12 */ node("main", file("main.rs", true), &[15, 16]),
            /* 13 */ node("lib", file("lib.rs", true), &[17, 18]),
            /* 14 */ node("util", file("util/mod.rs", false), &[19]),
            /* 15 */ node("t1", todo("TODO: count bytes too", 7, Tone::Normal), &[]),
            /* 16 */ node("t2", todo("FIXME: args may be empty", 9, Tone::Error), &[]),
            /* 17 */ node("t3", todo("TODO: document util", 1, Tone::Accent), &[]),
            /* 18 */ node("t4", todo("done: split modules", 2, Tone::Success), &[]),
            /* 19 */ node("t5", todo("TODO: tests", 1, Tone::Muted), &[]),
        ],
    }
}

/// Renders each scene and compares it with its golden, or rewrites the golden.
/// Returns whether they all matched.
pub(crate) fn check(cx: &mut VisualTestAppContext) -> Result<bool> {
    let goldens = goldens();
    let mut matched = true;
    for (scene, theme) in SCENES {
        let parent = tempfile::tempdir()?;
        let root = write_project(parent.path())?;
        let palette = use_theme(cx, theme)?;
        let views = PanelViews::new();
        let grants: Grants = [Capability::Panels].into_iter().collect();
        let slice = State::new(PLUGIN, &grants, &[PANEL.to_owned()], &views);
        slice
            .set_view_at(PANEL, view(), Instant::now())
            .map_err(|error| format!("the scene's view: {error:?}"))?;
        let decl = PanelDecl {
            id: PANEL.into(),
            title: "TODOs".into(),
            icon: "list-checks".into(),
            location: PanelLocation::Left,
            tick_ms: None,
            keys: false,
        };
        // The demo files open, as the start page beside the panel would show
        // the temporary folder's path.
        let image = capture_with(
            cx,
            &root,
            PANEL_SIZE,
            true,
            &palette,
            move |workbench, window, cx| {
                let panel =
                    cx.new(|cx| PluginPanel::new(PLUGIN, &decl, views, PanelEvents::new(), cx));
                let id = panel.read(cx).dock_id();
                workbench.add_plugin_panel(panel, cx);
                workbench
                    .dock()
                    .update(cx, |dock, cx| dock.dock_mut().show(id, window, cx));
            },
        )?;
        matched &= goldens.check(scene, &image)?;
    }
    // Leaves the default dark theme in use, as the scenes after it expect.
    use_theme(cx, themes::DEFAULT_DARK)?;
    Ok(matched)
}
