// SPDX-License-Identifier: Apache-2.0

//! The `grid` node: a fixed-width mono cell grid, a tone a cell, which the host validates whole
//! and updates by rows (T437).

use std::time::Instant;

use fe_plugin::fastedit::editor::panels::{Cell, GridNode, Node, NodeKind, Tone, View};
use fe_plugin::panels::{PanelViews, State};
use kiln_command::capability::{Capability, Grants};
use kiln_plugin::bindings::Error;

const PLUGIN: &str = "acme";
const PANEL: &str = "game";

fn plugin(views: &PanelViews) -> State {
    let grants: Grants = [Capability::parse("panels").unwrap()].into_iter().collect();
    State::new(PLUGIN, &grants, &[PANEL.to_owned()], views)
}

fn cell(ch: &str) -> Cell {
    Cell {
        ch: ch.into(),
        tone: Tone::Normal,
        reverse: false,
    }
}

fn grid_node(label: &str, cols: u8, rows: u8, cells: Vec<Cell>) -> NodeKind {
    NodeKind::Grid(GridNode {
        label: label.into(),
        cols,
        rows,
        cells,
    })
}

fn view_of(kind: NodeKind) -> View {
    View {
        nodes: vec![
            Node {
                id: "root".into(),
                kind: NodeKind::Column,
                children: vec![1],
            },
            Node {
                id: "board".into(),
                kind,
                children: vec![],
            },
        ],
    }
}

fn filled(cols: u8, rows: u8, ch: &str) -> View {
    let cells = (0..usize::from(cols) * usize::from(rows))
        .map(|_| cell(ch))
        .collect();
    view_of(grid_node("Board", cols, rows, cells))
}

fn board(views: &PanelViews) -> GridNode {
    match &views.view(PLUGIN, PANEL).unwrap().nodes[1].kind {
        NodeKind::Grid(grid) => grid.clone(),
        other => panic!("not a grid: {}", matches!(other, NodeKind::Column)),
    }
}

fn is_invalid(result: Result<impl Sized, Error>) -> bool {
    matches!(result, Err(Error::InvalidArgument(_)))
}

#[test]
fn grid_with_matching_cells_is_stored() {
    let views = PanelViews::new();
    let state = plugin(&views);
    let view = filled(20, 10, ".");
    state
        .set_view_at(PANEL, view.clone(), Instant::now())
        .unwrap();
    let grid = board(&views);
    assert_eq!((grid.cols, grid.rows, grid.cells.len()), (20, 10, 200));
    assert_eq!(grid.label, "Board");
}

#[test]
fn grid_size_mismatch_is_refused() {
    let views = PanelViews::new();
    let state = plugin(&views);
    let short = view_of(grid_node("Board", 3, 3, vec![cell("."); 8]));
    assert!(is_invalid(state.set_view_at(PANEL, short, Instant::now())));
    assert!(views.view(PLUGIN, PANEL).is_none());
}

#[test]
fn oversize_grid_is_refused_keeping_the_last_view() {
    let views = PanelViews::new();
    let state = plugin(&views);
    let good = filled(4, 4, "#");
    state
        .set_view_at(PANEL, good.clone(), Instant::now())
        .unwrap();
    let wide = view_of(grid_node("Board", 65, 1, vec![cell("."); 65]));
    assert!(is_invalid(state.set_view_at(PANEL, wide, Instant::now())));
    let tall = view_of(grid_node("Board", 1, 65, vec![cell("."); 65]));
    assert!(is_invalid(state.set_view_at(PANEL, tall, Instant::now())));
    // Five grids in one view is over the four a view may have.
    let mut many = filled(2, 2, ".");
    for index in 0..4 {
        let next = many.nodes.len() as u32;
        many.nodes[0].children.push(next);
        many.nodes.push(Node {
            id: format!("extra{index}"),
            kind: grid_node("Extra", 1, 1, vec![cell(".")]),
            children: vec![],
        });
    }
    assert!(is_invalid(state.set_view_at(PANEL, many, Instant::now())));
    assert_eq!(board(&views).cells.len(), 16);
    assert_eq!(board(&views).cells[0].ch, "#");
}

#[test]
fn row_update_changes_only_those_rows() {
    let views = PanelViews::new();
    let state = plugin(&views);
    state
        .set_view_at(PANEL, filled(3, 3, "."), Instant::now())
        .unwrap();
    let later = Instant::now() + std::time::Duration::from_secs(1);
    let row = vec![cell("X"), cell("Y"), cell("Z")];
    state
        .set_grid_rows_at(PANEL, "board", 1, vec![row], later)
        .unwrap();
    let grid = board(&views);
    let chars: Vec<&str> = grid.cells.iter().map(|cell| cell.ch.as_str()).collect();
    assert_eq!(chars, [".", ".", ".", "X", "Y", "Z", ".", ".", "."]);
    // A wrong-width row, a row past the end and a node that is no grid are refused.
    let at = later + std::time::Duration::from_secs(1);
    assert!(is_invalid(state.set_grid_rows_at(
        PANEL,
        "board",
        0,
        vec![vec![cell("a")]],
        at
    )));
    assert!(is_invalid(state.set_grid_rows_at(
        PANEL,
        "board",
        3,
        vec![vec![cell("a"), cell("b"), cell("c")]],
        at
    )));
    assert!(is_invalid(state.set_grid_rows_at(
        PANEL,
        "root",
        0,
        vec![],
        at
    )));
}

#[test]
fn multi_codepoint_grapheme_cell_is_one_cell() {
    let views = PanelViews::new();
    let state = plugin(&views);
    // A family emoji joined with ZWJs, a flag and an e with a combining accent: one cell each.
    let start = Instant::now();
    for (index, ch) in ["👨‍👩‍👧", "🇨🇦", "e\u{301}"].into_iter().enumerate() {
        let view = view_of(grid_node("Board", 1, 1, vec![cell(ch)]));
        // Each a second after the last, clear of the panel's update rate.
        let at = start + std::time::Duration::from_secs(index as u64);
        state.set_view_at(PANEL, view, at).unwrap();
        assert_eq!(board(&views).cells[0].ch, ch);
    }
    // Two graphemes, or none, is not a cell.
    for ch in ["ab", ""] {
        let view = view_of(grid_node("Board", 1, 1, vec![cell(ch)]));
        assert!(is_invalid(state.set_view_at(PANEL, view, Instant::now())));
    }
}

#[test]
fn empty_label_is_refused() {
    let views = PanelViews::new();
    let state = plugin(&views);
    for label in ["", "   "] {
        let view = view_of(grid_node(label, 1, 1, vec![cell(".")]));
        assert!(is_invalid(state.set_view_at(PANEL, view, Instant::now())));
    }
}
