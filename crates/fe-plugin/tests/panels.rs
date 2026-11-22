//! The `fastedit:editor/panels` imports, called as a plugin's store calls them: the host
//! validates a view before it takes it, and keeps the last good one.

use std::time::{Duration, Instant};

use fe_plugin::HostState;
use fe_plugin::fastedit::editor::panels::{
    ButtonNode, Host as _, Node, NodeKind, TextNode, Tone, View,
};
use fe_plugin::panels::{
    MAX_CHILDREN, MAX_DEPTH, MAX_NODE_TEXT_BYTES, MAX_NODES, MIN_UPDATE_INTERVAL, PanelViews,
    State, Update,
};
use kiln_command::capability::{Capability, Grants};
use kiln_plugin::bindings::Error;

const PLUGIN: &str = "acme";

fn grants(grants: &[&str]) -> Grants {
    grants
        .iter()
        .map(|capability| Capability::parse(capability).unwrap())
        .collect()
}

fn declared() -> Vec<String> {
    vec!["todos".to_owned()]
}

/// Plugin `acme`'s store state, holding `granted`, with the panel `todos`, on `views`.
fn plugin(granted: &[&str], views: &PanelViews) -> HostState {
    HostState {
        panels: State::new(PLUGIN, &grants(granted), &declared(), views),
        ..HostState::default()
    }
}

fn node(id: &str, kind: NodeKind, children: &[u32]) -> Node {
    Node {
        id: id.into(),
        kind,
        children: children.to_vec(),
    }
}

fn text(text: &str) -> NodeKind {
    NodeKind::Text(TextNode {
        text: text.into(),
        tone: Tone::Normal,
        mono: false,
        wrap: false,
    })
}

fn button(label: &str) -> NodeKind {
    NodeKind::Button(ButtonNode {
        label: label.into(),
        icon: None,
        command: Some("acme.refresh".into()),
        enabled: true,
    })
}

/// A column holding a heading, a text and a button.
fn small() -> View {
    View {
        nodes: vec![
            node("root", NodeKind::Column, &[1, 2, 3]),
            node("title", NodeKind::Heading("TODOs".into()), &[]),
            node("body", text("3 items"), &[]),
            node("refresh", button("Refresh"), &[]),
        ],
    }
}

/// A column of `count` texts under the root.
fn flat(count: usize) -> View {
    let mut nodes = vec![node("root", NodeKind::Column, &[])];
    for index in 0..count {
        nodes.push(node(&format!("n{index}"), text("x"), &[]));
    }
    nodes[0].children = (1..=count as u32).collect();
    View { nodes }
}

#[test]
fn panels_valid_tree_is_stored() {
    let views = PanelViews::new();
    let mut state = plugin(&["panels"], &views);
    state.set_view("todos".into(), small()).unwrap();
    assert_eq!(views.view(PLUGIN, "todos"), Some(small()));
    // A patch replaces the subtree at its node, once the panel may update again.
    let start = Instant::now();
    let patch = View {
        nodes: vec![node("body", text("4 items"), &[])],
    };
    let at = start + MIN_UPDATE_INTERVAL * 2;
    let update = state.panels.patch_at("todos", "body", patch, at).unwrap();
    assert!(matches!(update, Update::Applied | Update::Coalesced));
    views.flush(PLUGIN, "todos", at + MIN_UPDATE_INTERVAL);
    let mut expected = small();
    expected.nodes[2].kind = text("4 items");
    assert_eq!(views.view(PLUGIN, "todos"), Some(expected));
    // A patch at a node the view lacks is not found.
    let patch = View {
        nodes: vec![node("x", text("x"), &[])],
    };
    assert!(matches!(
        state
            .panels
            .patch_at("todos", "nope", patch, at + MIN_UPDATE_INTERVAL * 4),
        Err(Error::NotFound(_))
    ));
}

#[test]
fn panels_oversize_tree_rejected_keeps_previous() {
    let views = PanelViews::new();
    let mut state = plugin(&["panels"], &views);
    state.set_view("todos".into(), small()).unwrap();
    let error = state.set_view("todos".into(), flat(MAX_NODES)).unwrap_err();
    assert!(matches!(error, Error::InvalidArgument(_)), "{error:?}");
    assert_eq!(views.view(PLUGIN, "todos"), Some(small()));
    assert!(
        !views.has_pending(PLUGIN, "todos"),
        "an invalid view is not held either"
    );
    // A list over the child limit is refused too; at it, accepted.
    let views = PanelViews::new();
    let mut state = plugin(&["panels"], &views);
    let error = state
        .set_view("todos".into(), flat(MAX_CHILDREN + 1))
        .unwrap_err();
    assert!(matches!(error, Error::InvalidArgument(_)), "{error:?}");
    state.set_view("todos".into(), flat(MAX_CHILDREN)).unwrap();
}

#[test]
fn panels_depth_limit() {
    let chain = |depth: usize| View {
        nodes: (0..depth)
            .map(|index| {
                let children: &[u32] = if index + 1 < depth {
                    &[index as u32 + 1]
                } else {
                    &[]
                };
                node(&format!("n{index}"), NodeKind::Column, children)
            })
            .collect(),
    };
    let views = PanelViews::new();
    let mut state = plugin(&["panels"], &views);
    state.set_view("todos".into(), chain(MAX_DEPTH)).unwrap();
    let views = PanelViews::new();
    let mut state = plugin(&["panels"], &views);
    let error = state
        .set_view("todos".into(), chain(MAX_DEPTH + 1))
        .unwrap_err();
    assert!(matches!(error, Error::InvalidArgument(_)), "{error:?}");
    assert_eq!(views.view(PLUGIN, "todos"), None);
    // A cycle is no tree.
    let mut cyclic = chain(3);
    cyclic.nodes[2].children = vec![0];
    assert!(matches!(
        state.set_view("todos".into(), cyclic),
        Err(Error::InvalidArgument(_))
    ));
}

#[test]
fn panels_label_length_limit() {
    let views = PanelViews::new();
    let mut state = plugin(&["panels"], &views);
    state.set_view("todos".into(), small()).unwrap();
    let mut view = small();
    view.nodes[3].kind = button(&"x".repeat(MAX_NODE_TEXT_BYTES + 1));
    let error = state.set_view("todos".into(), view).unwrap_err();
    assert!(matches!(error, Error::InvalidArgument(_)), "{error:?}");
    assert_eq!(views.view(PLUGIN, "todos"), Some(small()));
    // An interactive node without a label is refused too.
    let mut view = small();
    view.nodes[3].kind = button("  ");
    assert!(matches!(
        state.set_view("todos".into(), view),
        Err(Error::InvalidArgument(_))
    ));
}

#[test]
fn panels_update_rate_is_capped() {
    let views = PanelViews::new();
    let state = plugin(&["panels"], &views);
    let start = Instant::now();
    let at = |ms: u64| start + Duration::from_millis(ms);
    let set = |count: usize, ms: u64| state.panels.set_view_at("todos", flat(count), at(ms));
    assert_eq!(set(1, 0).unwrap(), Update::Applied);
    // Inside the window, updates are held, the latest replacing the earlier.
    assert_eq!(set(2, 10).unwrap(), Update::Coalesced);
    assert_eq!(set(3, 20).unwrap(), Update::Coalesced);
    assert_eq!(views.view(PLUGIN, "todos"), Some(flat(1)));
    assert!(views.has_pending(PLUGIN, "todos"));
    assert!(!views.flush(PLUGIN, "todos", at(30)));
    assert!(views.flush(PLUGIN, "todos", at(60)));
    assert_eq!(views.view(PLUGIN, "todos"), Some(flat(3)));
    assert!(!views.has_pending(PLUGIN, "todos"));
    // At 20 a second, a second of updates every 50 ms all apply.
    for step in 0..20u64 {
        assert_eq!(set(1, 1000 + step * 50).unwrap(), Update::Applied);
    }
    // An invalid update inside the window is refused, not held.
    assert!(set(MAX_NODES, 1000 + 19 * 50 + 1).is_err());
    assert!(!views.has_pending(PLUGIN, "todos"));
}

#[test]
fn panels_requires_panels_capability() {
    let views = PanelViews::new();
    let mut state = plugin(&["status"], &views);
    let error = state.set_view("todos".into(), small()).unwrap_err();
    assert!(matches!(error, Error::Denied(_)), "{error:?}");
    assert!(matches!(
        state.patch("todos".into(), "body".into(), small()),
        Err(Error::Denied(_))
    ));
    assert_eq!(views.view(PLUGIN, "todos"), None);
    // A panel the manifest doesn't declare is not found, granted or not.
    let mut state = plugin(&["panels"], &views);
    assert!(matches!(
        state.set_view("other".into(), small()),
        Err(Error::NotFound(_))
    ));
    // Without a store, as the default state, it is unimplemented.
    assert!(matches!(
        HostState::default().set_view("todos".into(), small()),
        Err(Error::Unimplemented(_))
    ));
}
