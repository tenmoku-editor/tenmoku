// SPDX-License-Identifier: Apache-2.0

//! Blocks: a falling-blocks game in a panel, and an optional first-party plugin. It shows what a
//! plugin can do with a realtime panel and nothing else: a grid drawn from theme tones, the
//! host's ticks for gravity, the keys the host routes to a focused panel, and the plugin's own
//! data file for the best score. It asks for `panels`, `panels.realtime` and `commands` only.
//!
//! The game is [`blocks_core`], which has no host; this file turns the panel's keys and ticks
//! into its calls and its state into the panel's view, a start screen or the board.

mod blocks_core;
mod view;

use std::cell::RefCell;
use std::hash::{BuildHasher, RandomState};

use blocks_core::{Game, Phase};

wit_bindgen::generate!({
    // The editor's world, with the panel handler a plugin with panels exports.
    inline: "
        package tenmoku:blocks;

        world blocks {
            include fastedit:editor/editor-extension@1.1.0;
            export fastedit:editor/panel-handler@1.1.0;
        }
    ",
    // kiln:extension is read from its one copy, as the host's bindgen reads it.
    path: ["../../../kiln/crates/kiln-plugin/wit", "../../../crates/fe-plugin/wit"],
    world: "tenmoku:blocks/blocks",
    generate_all,
});

use exports::fastedit::editor::panel_handler::{KeyEvent, PanelEvent};
use fastedit::editor::panels;
use kiln::extension::commands::{self, Command};
use kiln::extension::data;
use kiln::extension::log::{Level, log};

/// The panel the manifest declares.
const PANEL: &str = "board";
/// `Blocks: Play`: starts a game, or resumes a paused one.
const PLAY: &str = "tenmoku.blocks.play";
/// The plugin data file holding the best score, as decimal text.
const BEST_FILE: &str = "best-score";

/// What the panel shows.
#[derive(Default)]
struct Blocks {
    /// The game on the board, or the last one, over; none before the first.
    game: Option<Game>,
    /// The best score: from the plugin's data file, or none when there is no data to keep it
    /// in.
    best: Option<u32>,
    /// Whether the best score can be kept.
    keeps_best: bool,
    /// Bumped for each game, so two games started together still deal differently.
    games: u64,
}

impl Blocks {
    /// The panel's view now, the start screen unless a game is being played or paused.
    fn view(&self) -> panels::View {
        match &self.game {
            Some(game) if game.phase() != Phase::Over => view::board(game, self.best),
            over => view::start(over.as_ref().map(Game::score), self.best),
        }
    }

    fn show(&self) {
        if let Err(error) = panels::set_view(PANEL, &self.view()) {
            log(Level::Warn, &format!("the view was refused: {error:?}"));
        }
    }

    /// A new game, with a seed no earlier game had.
    fn start(&mut self) {
        self.games += 1;
        let seed = RandomState::new().hash_one(self.games);
        self.game = Some(Game::new(seed));
    }

    /// Notes a game that just ended: a new best is kept.
    fn ended(&mut self) {
        let Some(score) = self.game.as_ref().map(Game::score) else {
            return;
        };
        if self.best.is_some_and(|best| best >= score) || !self.keeps_best {
            return;
        }
        self.best = Some(score);
        if let Err(error) = data::replace(BEST_FILE, score.to_string().as_bytes()) {
            log(
                Level::Warn,
                &format!("the best score wasn't kept: {error:?}"),
            );
            self.keeps_best = false;
            self.best = None;
        }
    }

    /// A key at the focused panel; true if it changed what the panel shows.
    fn key(&mut self, key: &KeyEvent) -> bool {
        // The host keeps the secondary modifier's keys; ctrl and alt combinations aren't the
        // game's either.
        if key.ctrl || key.alt || key.secondary {
            return false;
        }
        let name = key.key.to_ascii_lowercase();
        let Some(game) = self
            .game
            .as_mut()
            .filter(|game| game.phase() != Phase::Over)
        else {
            return match name.as_str() {
                "enter" | "r" => {
                    self.start();
                    true
                }
                _ => false,
            };
        };
        let changed = match name.as_str() {
            "left" | "h" => game.left(),
            "right" | "l" => game.right(),
            "up" | "k" => game.rotate(),
            "down" | "j" => game.soft_drop(),
            "space" => game.hard_drop(),
            "p" => game.toggle_pause(),
            "r" => {
                self.start();
                return true;
            }
            _ => false,
        };
        if changed && game.phase() == Phase::Over {
            self.ended();
        }
        changed
    }

    /// A tick of `ms` milliseconds; true if it changed the board.
    fn tick(&mut self, ms: u32) -> bool {
        let Some(game) = self.game.as_mut() else {
            return false;
        };
        let changed = game.tick(ms);
        if changed && game.phase() == Phase::Over {
            self.ended();
        }
        changed
    }
}

thread_local! {
    static BLOCKS: RefCell<Blocks> = RefCell::default();
}

struct Plugin;

impl Guest for Plugin {
    fn init() -> Result<(), Error> {
        commands::register(&Command {
            id: PLAY.to_owned(),
            title: "Blocks: Play".to_owned(),
            args_schema: None,
            when: None,
            mutates_buffer: false,
        })?;
        BLOCKS.with_borrow_mut(|blocks| {
            // A host without plugin data keeps no best score; a first game has none yet.
            (blocks.best, blocks.keeps_best) = match data::read(BEST_FILE, 0, 32) {
                Ok(bytes) => (
                    std::str::from_utf8(&bytes)
                        .ok()
                        .and_then(|text| text.trim().parse().ok()),
                    true,
                ),
                Err(Error::NotFound(_)) => (None, true),
                Err(_) => (None, false),
            };
            blocks.show();
        });
        Ok(())
    }

    fn shutdown() {}
}

impl exports::kiln::extension::command_handler::Guest for Plugin {
    fn run_command(id: String, _args: String) -> Result<(), Error> {
        if id != PLAY {
            return Err(Error::NotFound(id));
        }
        BLOCKS.with_borrow_mut(|blocks| {
            match blocks.game.as_mut() {
                Some(game) if game.phase() == Phase::Paused => {
                    game.toggle_pause();
                }
                Some(game) if game.phase() == Phase::Playing => {}
                _ => blocks.start(),
            }
            blocks.show();
        });
        // Show the panel and give it focus, so the keys reach the game. A host from before
        // `reveal` leaves it to the panel's rail button.
        match panels::reveal(PANEL) {
            Ok(()) | Err(Error::Unimplemented(_)) => Ok(()),
            Err(error) => Err(error),
        }
    }
}

impl exports::kiln::extension::event_handler::Guest for Plugin {
    fn handle_events(_events: Vec<kiln::extension::events::Event>) {}
}

impl exports::fastedit::editor::panel_handler::Guest for Plugin {
    fn on_event(panel: String, event: PanelEvent) {
        if panel != PANEL {
            return;
        }
        BLOCKS.with_borrow_mut(|blocks| {
            let changed = match event {
                PanelEvent::Key(key) => blocks.key(&key),
                PanelEvent::Tick(ms) => blocks.tick(ms),
                PanelEvent::Shown => true,
                _ => false,
            };
            if changed {
                blocks.show();
            }
        });
    }
}

export!(Plugin);
