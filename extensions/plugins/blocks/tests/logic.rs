// SPDX-License-Identifier: Apache-2.0

//! The game's rules, natively: `blocks_core` has no host, so these run under `cargo test` here
//! (`cargo test --manifest-path extensions/plugins/blocks/Cargo.toml`) and in the workspace's
//! own test run, which declares this file from `crates/fe-workbench/tests/first_party_blocks.rs`.

#[path = "../src/blocks_core.rs"]
// The workspace lints unreachable `pub`; not all of the game is used here.
#[allow(dead_code, unreachable_pub)]
mod blocks_core;

use std::collections::BTreeSet;

use blocks_core::{Bag, COLS, Game, Kind, LINE_POINTS, Phase, Piece, ROWS};

/// The cells of `piece`, as a set to compare.
fn cells(piece: &Piece) -> BTreeSet<(i32, i32)> {
    piece.cells().into_iter().collect()
}

/// Fills `rows` of the well's bottom, all but column `gap`.
fn fill_bottom(game: &mut Game, rows: usize, gap: usize) {
    for row in ROWS - rows..ROWS {
        for col in (0..COLS).filter(|&col| col != gap) {
            game.board_mut().set(col, row, Some(Kind::O));
        }
    }
}

#[test]
fn bag_has_each_piece_once_per_seven() {
    for seed in [0, 1, 7, 42, u64::MAX] {
        let mut bag = Bag::new(seed);
        let mut orders = BTreeSet::new();
        for _ in 0..50 {
            let seven: Vec<Kind> = (0..7).map(|_| bag.deal()).collect();
            let kinds: BTreeSet<Kind> = seven.iter().copied().collect();
            assert_eq!(kinds.len(), 7, "seed {seed}: {seven:?}");
            orders.insert(seven);
        }
        // Shuffled, not dealt in one order every time.
        assert!(orders.len() > 1, "seed {seed}: one order only");
    }
    // A game deals from its bag too: its first seven pieces are one of each.
    let mut game = Game::new(3);
    let mut seen = BTreeSet::from([game.piece().kind]);
    while seen.len() < 7 {
        let next = game.next();
        assert!(seen.insert(next), "{next:?} came twice in the first seven");
        game.hard_drop();
    }
}

#[test]
fn rotation_kicks_off_the_wall() {
    // An I standing against the right wall: turning it flat in place would put cells past the
    // wall, so it is kicked left.
    let mut game = Game::new(0);
    let standing = Piece {
        kind: Kind::I,
        turns: 1,
        col: 7,
        row: 4,
    };
    assert!(game.place(standing));
    assert!(cells(&standing).iter().all(|&(col, _)| col == 9));
    assert!(game.rotate());
    let flat = game.piece();
    assert_eq!(flat.turns, 2);
    assert!(
        flat.cells()
            .iter()
            .all(|&(col, _)| (0..COLS as i32).contains(&col))
    );
    assert!(flat.cells().iter().any(|&(col, _)| col == 9), "{flat:?}");
    assert!(flat.col < standing.col, "kicked left: {flat:?}");

    // And off the left wall, rightwards.
    let mut game = Game::new(0);
    let standing = Piece {
        kind: Kind::I,
        turns: 3,
        col: -1,
        row: 4,
    };
    assert!(game.place(standing));
    assert!(cells(&standing).iter().all(|&(col, _)| col == 0));
    assert!(game.rotate());
    let flat = game.piece();
    assert!(
        flat.cells()
            .iter()
            .all(|&(col, _)| (0..COLS as i32).contains(&col))
    );
    assert!(flat.cells().iter().any(|&(col, _)| col == 0), "{flat:?}");
    assert!(flat.col > standing.col, "kicked right: {flat:?}");

    // A T against the wall turns with a kick, and one boxed in by the stack can't turn at all.
    let mut game = Game::new(0);
    let t = Piece {
        kind: Kind::T,
        turns: 1,
        col: -1,
        row: 8,
    };
    assert!(game.place(t));
    assert!(game.rotate());
    assert!(game.piece().cells().iter().all(|&(col, _)| col >= 0));
    let mut game = Game::new(0);
    let t = Piece {
        kind: Kind::I,
        turns: 1,
        col: 2,
        row: 10,
    };
    for row in 9..=14 {
        for col in [0, 1, 2, 3, 5, 6, 7, 8, 9] {
            game.board_mut().set(col, row, Some(Kind::O));
        }
    }
    assert!(game.place(t));
    assert!(!game.rotate(), "no room to turn: {:?}", game.piece());
    assert_eq!(game.piece(), t);
}

#[test]
fn clearing_four_lines_scores_the_tetris_bonus() {
    // Four rows full but for column 9, and an I standing in it above them.
    let mut game = Game::new(5);
    fill_bottom(&mut game, 4, 9);
    let standing = Piece {
        kind: Kind::I,
        turns: 1,
        col: 7,
        row: 0,
    };
    assert!(game.place(standing));
    let before = game.score();
    let fall = (game.ghost().row - standing.row) as u32;
    assert!(game.hard_drop());
    assert_eq!(game.lines(), 4);
    assert_eq!(
        game.board().filled(),
        0,
        "the four rows went, and the I with them"
    );
    assert_eq!(game.score() - before, 2 * fall + LINE_POINTS[4]);
    // Four lines at once are worth more than four single lines.
    assert!(LINE_POINTS[4] > 4 * LINE_POINTS[1]);

    // One line at a time scores the single-line points.
    let mut game = Game::new(5);
    fill_bottom(&mut game, 1, 0);
    let standing = Piece {
        kind: Kind::I,
        turns: 3,
        col: -1,
        row: 0,
    };
    assert!(game.place(standing));
    let fall = (game.ghost().row - standing.row) as u32;
    assert!(game.hard_drop());
    assert_eq!(game.lines(), 1);
    assert_eq!(game.score(), 2 * fall + LINE_POINTS[1]);
    // The I's three cells above the cleared line moved down a row.
    assert_eq!(game.board().filled(), 3);
}

#[test]
fn hard_drop_lands_on_the_ghost() {
    let mut game = Game::new(11);
    // A ragged stack to land on.
    for (col, height) in [3, 1, 0, 4, 2, 2, 5, 0, 1, 3].into_iter().enumerate() {
        for row in ROWS - height..ROWS {
            game.board_mut().set(col, row, Some(Kind::Z));
        }
    }
    for shift in -4i32..=4 {
        let mut game = game.clone();
        for _ in 0..shift.abs() {
            match shift < 0 {
                true => game.left(),
                false => game.right(),
            };
        }
        game.rotate();
        let piece = game.piece();
        let ghost = game.ghost();
        assert_eq!(
            (ghost.kind, ghost.turns, ghost.col),
            (piece.kind, piece.turns, piece.col)
        );
        assert!(ghost.row >= piece.row);
        let mut board = game.board().clone();
        for (col, row) in ghost.cells() {
            assert_eq!(board.get(col as usize, row as usize), None);
            board.set(col as usize, row as usize, Some(ghost.kind));
        }
        assert!(game.hard_drop());
        // Settled exactly where the ghost was (no line was full, so nothing moved).
        assert_eq!(game.lines(), 0);
        assert_eq!(game.board(), &board, "shift {shift}");
        assert_eq!(game.score(), 2 * (ghost.row - piece.row) as u32);
    }
}

#[test]
fn top_out_is_game_over() {
    let mut game = Game::new(9);
    // Every piece dropped where it enters: the middle fills, and one has no room.
    let mut drops = 0;
    while game.phase() == Phase::Playing {
        assert!(game.hard_drop());
        drops += 1;
        assert!(drops < ROWS * 2, "never topped out");
    }
    assert_eq!(game.phase(), Phase::Over);
    // Nothing moves once it's over.
    let over = game.clone();
    assert!(!game.hard_drop());
    assert!(!game.soft_drop());
    assert!(!game.left());
    assert!(!game.rotate());
    assert!(!game.tick(10_000));
    assert!(!game.toggle_pause());
    assert_eq!(game, over);

    // A piece that settles partly above the well ends the game too.
    let mut game = Game::new(9);
    for row in 1..ROWS {
        game.board_mut().set(4, row, Some(Kind::O));
    }
    let standing = Piece {
        kind: Kind::I,
        turns: 1,
        col: 2,
        row: -3,
    };
    assert!(game.place(standing));
    assert!(game.hard_drop());
    assert_eq!(game.phase(), Phase::Over);
}

#[test]
fn same_seed_same_game() {
    /// A game of `seed` played by one fixed script of moves and ticks: its state after each.
    fn play(seed: u64) -> Vec<Game> {
        let mut game = Game::new(seed);
        let mut states = vec![game.clone()];
        for step in 0..400u32 {
            match step % 7 {
                0 => game.left(),
                1 => game.rotate(),
                2 => game.tick(500),
                3 => game.right(),
                4 => game.soft_drop(),
                5 => game.tick(120),
                _ => game.hard_drop(),
            };
            states.push(game.clone());
        }
        states
    }
    assert_eq!(play(42), play(42));
    let deals = |seed| -> Vec<Kind> {
        let mut bag = Bag::new(seed);
        (0..28).map(|_| bag.deal()).collect()
    };
    assert_eq!(deals(42), deals(42));
    assert_ne!(deals(42), deals(43), "another seed deals another game");
    assert_ne!(play(42), play(43));
}

#[test]
fn gravity_speeds_up_from_500_to_80_ms() {
    let mut game = Game::new(1);
    assert_eq!(game.gravity_ms(), 500);
    // A tick short of the interval doesn't move the piece; the one that reaches it does, one
    // row, however late it is.
    let row = game.piece().row;
    assert!(!game.tick(499));
    assert_eq!(game.piece().row, row);
    assert!(game.tick(1));
    assert_eq!(game.piece().row, row + 1);
    assert!(game.tick(5_000));
    assert_eq!(game.piece().row, row + 2);
    // Paused, ticks do nothing.
    assert!(game.toggle_pause());
    assert!(!game.tick(5_000));
    assert_eq!(game.piece().row, row + 2);
    assert!(game.toggle_pause());

    // Clearing lines raises the level, and gravity gets faster down to 80 ms.
    let mut intervals = vec![game.gravity_ms()];
    while game.level() < 15 {
        let mut next = Game::new(1);
        fill_bottom(&mut next, 1, 0);
        // A cleared line a hard drop at a time, carried over into a fresh well.
        let lines = game.lines();
        *game.board_mut() = next.board().clone();
        assert!(game.place(Piece {
            kind: Kind::I,
            turns: 3,
            col: -1,
            row: 0,
        }));
        game.hard_drop();
        assert_eq!(game.lines(), lines + 1);
        intervals.push(game.gravity_ms());
        // Clear the I's leftovers.
        *game.board_mut() = Default::default();
    }
    assert!(intervals.windows(2).all(|pair| pair[1] <= pair[0]));
    assert_eq!(intervals.first(), Some(&500));
    assert_eq!(intervals.last(), Some(&80));
}

/// Clears `count` lines a hard drop at a time, leaving the well empty.
fn clear_lines(game: &mut Game, count: u32) {
    for _ in 0..count {
        let lines = game.lines();
        *game.board_mut() = Default::default();
        fill_bottom(game, 1, 0);
        assert!(game.place(Piece {
            kind: Kind::I,
            turns: 3,
            col: -1,
            row: 0,
        }));
        game.hard_drop();
        assert_eq!(game.lines(), lines + 1);
    }
    *game.board_mut() = Default::default();
}

#[test]
fn gravity_keeps_its_pace_on_50_ms_ticks() {
    // The panel ticks every 50 ms. Over 20 s of them, gravity moves the piece (or settles it)
    // as often as its interval says, at every level, the time a tick runs over carried on.
    for (lines, gravity, steps) in [(0, 500, 40), (10, 455, 43), (100, 80, 250)] {
        let mut game = Game::new(4);
        clear_lines(&mut game, lines);
        assert_eq!(game.gravity_ms(), gravity);
        let mut moved = 0;
        for _ in 0..400 {
            // An empty well each time: no top-out over 20 s of falling.
            *game.board_mut() = Default::default();
            if game.tick(50) {
                moved += 1;
            }
        }
        assert_eq!(moved, steps, "level {} at {gravity} ms", game.level());
        assert_eq!(game.phase(), Phase::Playing);
    }

    // A tick that comes very late moves the piece one row, and the next at most one more: no
    // backlog of the rows it missed.
    let mut game = Game::new(4);
    let row = game.piece().row;
    assert!(game.tick(10_000));
    assert_eq!(game.piece().row, row + 1);
    game.tick(50);
    assert!(game.piece().row <= row + 2);
    let settled = game.piece().row;
    for _ in 0..8 {
        assert!(!game.tick(50), "back to the usual pace");
    }
    assert_eq!(game.piece().row, settled);
}
