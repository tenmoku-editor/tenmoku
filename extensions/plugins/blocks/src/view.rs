// SPDX-License-Identifier: Apache-2.0

//! The panel's two views, as the host's declarative tree: the start screen, and the board with
//! the score and the next piece beside it.
//!
//! The board is a grid node, one cell per square of the well. A piece's cells show its letter,
//! reversed (a solid block) in its kind's tone, whether falling or settled; the ghost, where the
//! falling piece would land, shows the letter in the muted tone, not reversed; an empty square
//! is a muted dot. Every tone is a theme token, so each theme colours it.

use crate::blocks_core::{COLS, Game, Kind, Phase, ROWS};
use crate::fastedit::editor::panels::{Cell, GridNode, Node, NodeKind, TextNode, Tone, View};

/// The start screen's controls, a line each.
pub const CONTROLS: [&str; 7] = [
    "← →  or  h l   move",
    "↑  or  k       rotate",
    "↓  or  j       soft drop",
    "Space          hard drop",
    "P              pause",
    "R              restart",
    "Esc            back to the editor",
];

/// The one line of controls beside the board while playing.
pub const HINT: &str =
    "←→ move · ↑ rotate · ↓ soft drop · Space drop · P pause · R restart · Esc leave";

/// The tone a piece of `kind` draws in: the five tones that aren't muted, which is the ghost's
/// and the empty squares'. Two pairs share a tone, and their letters tell them apart.
pub fn tone(kind: Kind) -> Tone {
    match kind {
        Kind::I | Kind::J => Tone::Accent,
        Kind::O | Kind::L => Tone::Warning,
        Kind::T => Tone::Normal,
        Kind::S => Tone::Success,
        Kind::Z => Tone::Error,
    }
}

/// A tree built a node at a time, as the flat arena the host takes.
struct Tree {
    nodes: Vec<Node>,
}

impl Tree {
    fn new(root: NodeKind) -> Tree {
        Tree {
            nodes: vec![Node {
                id: "root".to_owned(),
                kind: root,
                children: Vec::new(),
            }],
        }
    }

    /// Adds `kind` as the last child of node `parent`; its index.
    fn add(&mut self, parent: u32, id: &str, kind: NodeKind) -> u32 {
        let index = self.nodes.len() as u32;
        self.nodes.push(Node {
            id: id.to_owned(),
            kind,
            children: Vec::new(),
        });
        self.nodes[parent as usize].children.push(index);
        index
    }

    fn text(&mut self, parent: u32, id: &str, text: &str, tone: Tone, mono: bool) -> u32 {
        self.add(
            parent,
            id,
            NodeKind::Text(TextNode {
                text: text.to_owned(),
                tone,
                mono,
                wrap: !mono,
            }),
        )
    }

    fn view(self) -> View {
        View { nodes: self.nodes }
    }
}

fn points(score: u32) -> String {
    match score {
        1 => "1 point".to_owned(),
        score => format!("{score} points"),
    }
}

/// The start screen: what the game is, the key that starts it and the controls, with the last
/// game's score once one has ended and the best score when it's kept.
pub fn start(last: Option<u32>, best: Option<u32>) -> View {
    let mut tree = Tree::new(NodeKind::Column);
    tree.add(0, "title", NodeKind::Heading("Blocks".to_owned()));
    tree.text(
        0,
        "about",
        "A falling-blocks game. Clear lines to score; the blocks fall faster each level.",
        Tone::Muted,
        false,
    );
    if let Some(score) = last {
        tree.text(
            0,
            "over",
            &format!("Game over: {}.", points(score)),
            Tone::Warning,
            false,
        );
    }
    if let Some(best) = best {
        tree.text(
            0,
            "best",
            &format!("Best: {}", points(best)),
            Tone::Normal,
            false,
        );
    }
    let again = match last {
        Some(_) => "Press Enter to play again.",
        None => "Press Enter to start.",
    };
    tree.text(0, "start", again, Tone::Accent, false);
    tree.add(0, "rule", NodeKind::Separator);
    tree.add(
        0,
        "controls-title",
        NodeKind::Heading("Controls".to_owned()),
    );
    for (index, line) in CONTROLS.iter().enumerate() {
        tree.text(0, &format!("control-{index}"), line, Tone::Normal, true);
    }
    tree.view()
}

/// The board, and beside it the score, level, lines, best score, next piece and the controls
/// line; "Paused" while paused.
pub fn board(game: &Game, best: Option<u32>) -> View {
    let mut tree = Tree::new(NodeKind::Row);
    tree.add(
        0,
        "board",
        NodeKind::Grid(GridNode {
            label: format!("Blocks board, {COLS} by {ROWS}"),
            cols: COLS as u8,
            rows: ROWS as u8,
            cells: cells(game),
        }),
    );
    let side = tree.add(0, "side", NodeKind::Column);
    tree.text(
        side,
        "score",
        &format!("Score {}", game.score()),
        Tone::Normal,
        false,
    );
    tree.text(
        side,
        "level",
        &format!("Level {}", game.level()),
        Tone::Normal,
        false,
    );
    tree.text(
        side,
        "lines",
        &format!("Lines {}", game.lines()),
        Tone::Normal,
        false,
    );
    if let Some(best) = best {
        tree.text(side, "best", &format!("Best {best}"), Tone::Muted, false);
    }
    let next = game.next();
    tree.text(
        side,
        "next",
        &format!("Next: {}", next.letter()),
        Tone::Normal,
        false,
    );
    for (index, line) in shape(next).iter().enumerate() {
        tree.text(side, &format!("next-{index}"), line, tone(next), true);
    }
    if game.phase() == Phase::Paused {
        tree.text(side, "paused", "Paused. P to resume.", Tone::Accent, false);
    }
    tree.text(side, "hint", HINT, Tone::Muted, false);
    tree.view()
}

/// The board's cells, row-major: the settled ones, the ghost and the falling piece over them.
fn cells(game: &Game) -> Vec<Cell> {
    let mut cells: Vec<Cell> = (0..ROWS * COLS)
        .map(|index| match game.board().get(index % COLS, index / COLS) {
            Some(kind) => block(kind),
            None => Cell {
                ch: "·".to_owned(),
                tone: Tone::Muted,
                reverse: false,
            },
        })
        .collect();
    let mut put = |(col, row): (i32, i32), cell: Cell| {
        if let (Ok(col), Ok(row)) = (usize::try_from(col), usize::try_from(row))
            && col < COLS
            && row < ROWS
        {
            cells[row * COLS + col] = cell;
        }
    };
    let piece = game.piece();
    for at in game.ghost().cells() {
        put(
            at,
            Cell {
                ch: piece.kind.letter().to_string(),
                tone: Tone::Muted,
                reverse: false,
            },
        );
    }
    for at in piece.cells() {
        put(at, block(piece.kind));
    }
    cells
}

fn block(kind: Kind) -> Cell {
    Cell {
        ch: kind.letter().to_string(),
        tone: tone(kind),
        reverse: true,
    }
}

/// `kind` drawn in its letter as it enters: two lines, or one for the I.
fn shape(kind: Kind) -> Vec<String> {
    let mut lines = [[' '; 4]; 2];
    let piece = crate::blocks_core::Piece::spawn(kind);
    let top = piece.cells().iter().map(|&(_, row)| row).min().unwrap_or(0);
    let left = piece.cells().iter().map(|&(col, _)| col).min().unwrap_or(0);
    for (col, row) in piece.cells() {
        lines[(row - top) as usize][(col - left) as usize] = kind.letter();
    }
    (lines.iter())
        .map(|line| line.iter().collect::<String>().trim_end().to_owned())
        .filter(|line| !line.is_empty())
        .collect()
}
