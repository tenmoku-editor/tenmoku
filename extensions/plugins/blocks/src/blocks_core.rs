// SPDX-License-Identifier: Apache-2.0

//! The game itself, with no host: a ten-wide, twenty-high well, seven pieces dealt from a
//! shuffled bag, gravity on the plugin's ticks, and the score. Nothing here calls the editor, so
//! it builds and is tested natively (`tests/logic.rs`), and the plugin (`lib.rs`) only turns
//! keys and ticks into these calls and the state into a panel view.

/// The well's width, in cells.
pub const COLS: usize = 10;
/// The well's height, in cells.
pub const ROWS: usize = 20;

/// Gravity's interval at level 1, in milliseconds.
pub const FIRST_GRAVITY_MS: u32 = 500;
/// The fastest gravity, in milliseconds: from level 11 on.
pub const FASTEST_GRAVITY_MS: u32 = 80;
/// How much faster gravity gets each level, in milliseconds.
const GRAVITY_STEP_MS: u32 = 45;
/// Lines a level lasts.
pub const LINES_PER_LEVEL: u32 = 10;
/// Points for clearing one, two, three and four lines at once, times the level.
pub const LINE_POINTS: [u32; 5] = [0, 100, 300, 500, 800];

/// One of the seven pieces.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Kind {
    I,
    O,
    T,
    S,
    Z,
    J,
    L,
}

impl Kind {
    /// Every kind, in a fixed order: one bag before it is shuffled.
    pub const ALL: [Kind; 7] = [
        Kind::I,
        Kind::O,
        Kind::T,
        Kind::S,
        Kind::Z,
        Kind::J,
        Kind::L,
    ];

    /// The letter a cell of it shows.
    pub fn letter(self) -> char {
        match self {
            Kind::I => 'I',
            Kind::O => 'O',
            Kind::T => 'T',
            Kind::S => 'S',
            Kind::Z => 'Z',
            Kind::J => 'J',
            Kind::L => 'L',
        }
    }

    /// The side of the square box it turns in.
    fn size(self) -> i32 {
        match self {
            Kind::I => 4,
            Kind::O => 2,
            _ => 3,
        }
    }

    /// Its cells unrotated, as `(column, row)` in its box.
    fn base(self) -> [(i32, i32); 4] {
        match self {
            Kind::I => [(0, 1), (1, 1), (2, 1), (3, 1)],
            Kind::O => [(0, 0), (1, 0), (0, 1), (1, 1)],
            Kind::T => [(1, 0), (0, 1), (1, 1), (2, 1)],
            Kind::S => [(1, 0), (2, 0), (0, 1), (1, 1)],
            Kind::Z => [(0, 0), (1, 0), (1, 1), (2, 1)],
            Kind::J => [(0, 0), (0, 1), (1, 1), (2, 1)],
            Kind::L => [(2, 0), (0, 1), (1, 1), (2, 1)],
        }
    }
}

/// A piece in the well: its kind, its quarter turns clockwise (0 to 3), and the top left of its
/// box, which may be above the well (a negative row) or, for a box with an empty column, past
/// its sides.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Piece {
    pub kind: Kind,
    pub turns: u8,
    pub col: i32,
    pub row: i32,
}

impl Piece {
    /// `kind` where a new piece enters: centred, its top cells in the well's top row.
    pub fn spawn(kind: Kind) -> Piece {
        let size = kind.size();
        Piece {
            kind,
            turns: 0,
            col: (COLS as i32 - size) / 2,
            // The I's cells are its box's second row.
            row: if kind == Kind::I { -1 } else { 0 },
        }
    }

    /// Its four cells in the well, as `(column, row)`.
    pub fn cells(&self) -> [(i32, i32); 4] {
        let size = self.kind.size();
        self.kind.base().map(|(mut x, mut y)| {
            for _ in 0..self.turns % 4 {
                (x, y) = (size - 1 - y, x);
            }
            (self.col + x, self.row + y)
        })
    }

    fn moved(self, cols: i32, rows: i32) -> Piece {
        Piece {
            col: self.col + cols,
            row: self.row + rows,
            ..self
        }
    }

    fn turned(self) -> Piece {
        Piece {
            turns: (self.turns + 1) % 4,
            ..self
        }
    }
}

/// The well's settled cells, row 0 at the top.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Board {
    cells: [[Option<Kind>; COLS]; ROWS],
}

impl Default for Board {
    fn default() -> Self {
        Board {
            cells: [[None; COLS]; ROWS],
        }
    }
}

impl Board {
    /// What settled at `(col, row)`; nothing outside the well.
    pub fn get(&self, col: usize, row: usize) -> Option<Kind> {
        *self.cells.get(row)?.get(col)?
    }

    /// Settles `kind` at `(col, row)`, or empties it; outside the well is ignored.
    pub fn set(&mut self, col: usize, row: usize, kind: Option<Kind>) {
        if let Some(cell) = self.cells.get_mut(row).and_then(|cells| cells.get_mut(col)) {
            *cell = kind;
        }
    }

    /// Whether `piece` fits: every cell inside the sides and above the floor, none on a settled
    /// cell. Cells above the well fit.
    pub fn fits(&self, piece: &Piece) -> bool {
        piece.cells().iter().all(|&(col, row)| {
            (0..COLS as i32).contains(&col)
                && row < ROWS as i32
                && (row < 0 || self.cells[row as usize][col as usize].is_none())
        })
    }

    /// How many cells have settled.
    #[allow(dead_code)] // For tests/logic.rs.
    pub fn filled(&self) -> usize {
        self.cells
            .iter()
            .flatten()
            .filter(|cell| cell.is_some())
            .count()
    }

    /// Removes the full rows, moving those above down; how many went.
    fn clear_lines(&mut self) -> u32 {
        let kept: Vec<[Option<Kind>; COLS]> = (self.cells.iter())
            .filter(|row| row.iter().any(Option::is_none))
            .copied()
            .collect();
        let cleared = ROWS - kept.len();
        let mut cells = [[None; COLS]; ROWS];
        cells[cleared..].copy_from_slice(&kept);
        self.cells = cells;
        cleared as u32
    }
}

/// SplitMix64: small, fast and the same everywhere, so a seed always deals the same pieces.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A number below `bound`, which is at most 7 here, so the modulo's bias is negligible.
    fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound as u64) as usize
    }
}

/// The pieces to come: each run of seven, from the first, holds every kind once, shuffled.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Bag {
    rng: Rng,
    left: Vec<Kind>,
}

impl Bag {
    pub fn new(seed: u64) -> Bag {
        Bag {
            rng: Rng(seed),
            left: Vec::new(),
        }
    }

    /// The next piece, opening a new shuffled bag when the last is used up.
    pub fn deal(&mut self) -> Kind {
        if self.left.is_empty() {
            let mut bag = Kind::ALL;
            // Fisher-Yates; the bag is dealt from its end.
            for i in (1..bag.len()).rev() {
                bag.swap(i, self.rng.below(i + 1));
            }
            self.left = bag.to_vec();
        }
        self.left.pop().expect("a bag was just filled")
    }
}

/// Where a game stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Phase {
    Playing,
    Paused,
    /// A piece had no room: nothing moves until a new game.
    Over,
}

/// One game: the well, the falling piece, the next one and the score.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Game {
    board: Board,
    piece: Piece,
    next: Kind,
    bag: Bag,
    phase: Phase,
    score: u32,
    lines: u32,
    /// Milliseconds of ticks since gravity last moved the piece.
    elapsed: u32,
}

impl Game {
    /// A game whose pieces `seed` deals, its first piece entering.
    pub fn new(seed: u64) -> Game {
        let mut bag = Bag::new(seed);
        let first = bag.deal();
        let next = bag.deal();
        Game {
            board: Board::default(),
            piece: Piece::spawn(first),
            next,
            bag,
            phase: Phase::Playing,
            score: 0,
            lines: 0,
            elapsed: 0,
        }
    }

    pub fn board(&self) -> &Board {
        &self.board
    }

    /// The well, to set a position up (tests, puzzles). The falling piece isn't checked
    /// against it until it next moves.
    #[allow(dead_code)] // For tests/logic.rs.
    pub fn board_mut(&mut self) -> &mut Board {
        &mut self.board
    }

    /// The falling piece.
    pub fn piece(&self) -> Piece {
        self.piece
    }

    /// Puts `piece` in place of the falling one, if it fits; false, changing nothing, if not.
    pub fn place(&mut self, piece: Piece) -> bool {
        let fits = self.board.fits(&piece);
        if fits {
            self.piece = piece;
        }
        fits
    }

    pub fn next(&self) -> Kind {
        self.next
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    pub fn score(&self) -> u32 {
        self.score
    }

    pub fn lines(&self) -> u32 {
        self.lines
    }

    /// 1, and one more for every [`LINES_PER_LEVEL`] lines.
    pub fn level(&self) -> u32 {
        1 + self.lines / LINES_PER_LEVEL
    }

    /// Milliseconds between gravity's moves at this level: [`FIRST_GRAVITY_MS`] at level 1,
    /// faster each level, never under [`FASTEST_GRAVITY_MS`].
    pub fn gravity_ms(&self) -> u32 {
        let faster = GRAVITY_STEP_MS.saturating_mul(self.level() - 1);
        FIRST_GRAVITY_MS
            .saturating_sub(faster)
            .max(FASTEST_GRAVITY_MS)
    }

    /// Where the falling piece would land: the ghost.
    pub fn ghost(&self) -> Piece {
        let mut ghost = self.piece;
        while self.board.fits(&ghost.moved(0, 1)) {
            ghost = ghost.moved(0, 1);
        }
        ghost
    }

    fn playing(&self) -> bool {
        self.phase == Phase::Playing
    }

    /// Moves the piece a column left; false if it can't.
    pub fn left(&mut self) -> bool {
        self.shift(-1)
    }

    /// Moves the piece a column right; false if it can't.
    pub fn right(&mut self) -> bool {
        self.shift(1)
    }

    fn shift(&mut self, cols: i32) -> bool {
        self.playing() && self.place(self.piece.moved(cols, 0))
    }

    /// Turns the piece a quarter clockwise. Against a wall or the stack it is kicked a column
    /// or two sideways, or a row up, to the first place it fits; false if none does.
    pub fn rotate(&mut self) -> bool {
        if !self.playing() {
            return false;
        }
        let turned = self.piece.turned();
        const KICKS: [(i32, i32); 6] = [(0, 0), (-1, 0), (1, 0), (-2, 0), (2, 0), (0, -1)];
        KICKS
            .iter()
            .any(|&(cols, rows)| self.place(turned.moved(cols, rows)))
    }

    /// Moves the piece a row down for a point, or settles it if it can't move; false if the
    /// game isn't being played.
    pub fn soft_drop(&mut self) -> bool {
        if !self.playing() {
            return false;
        }
        if self.place(self.piece.moved(0, 1)) {
            self.score += 1;
        } else {
            self.lock();
        }
        self.elapsed = 0;
        true
    }

    /// Drops the piece onto its ghost, two points a row, and settles it.
    pub fn hard_drop(&mut self) -> bool {
        if !self.playing() {
            return false;
        }
        let ghost = self.ghost();
        self.score += 2 * (ghost.row - self.piece.row) as u32;
        self.piece = ghost;
        self.lock();
        true
    }

    /// `ms` milliseconds of play: once they add up to [`Game::gravity_ms`], the piece moves a
    /// row down, or settles. What a tick runs past the interval counts towards the next, so
    /// ticks of any length keep gravity's pace on average (455 ms at level 2 on 50 ms ticks,
    /// not 500). At most one row a tick, and less than an interval is carried over: a late
    /// tick moves the piece one row, and at most one more on the next tick, never a backlog.
    /// True if the game changed.
    pub fn tick(&mut self, ms: u32) -> bool {
        if !self.playing() {
            return false;
        }
        let gravity = self.gravity_ms();
        let elapsed = self.elapsed.saturating_add(ms);
        if elapsed < gravity {
            self.elapsed = elapsed;
            return false;
        }
        if !self.place(self.piece.moved(0, 1)) {
            self.lock();
        }
        // After settling too: the next piece falls at the same pace.
        self.elapsed = (elapsed - gravity).min(gravity - 1);
        true
    }

    /// Pauses a game being played, or resumes a paused one; false for one that's over.
    pub fn toggle_pause(&mut self) -> bool {
        self.phase = match self.phase {
            Phase::Playing => Phase::Paused,
            Phase::Paused => Phase::Playing,
            Phase::Over => return false,
        };
        true
    }

    /// Settles the piece, clears the full lines and scores them, and brings in the next piece.
    /// The game is over if the piece settled above the well, or the next has no room.
    fn lock(&mut self) {
        let mut above = false;
        for (col, row) in self.piece.cells() {
            match usize::try_from(row) {
                Ok(row) => self.board.set(col as usize, row, Some(self.piece.kind)),
                Err(_) => above = true,
            }
        }
        let cleared = self.board.clear_lines();
        self.score += LINE_POINTS[cleared as usize] * self.level();
        self.lines += cleared;
        self.elapsed = 0;
        self.piece = Piece::spawn(self.next);
        self.next = self.bag.deal();
        if above || !self.board.fits(&self.piece) {
            self.phase = Phase::Over;
        }
    }
}
