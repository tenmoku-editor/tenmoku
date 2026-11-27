//! Blocks, the falling-blocks plugin in `extensions/plugins/blocks` (T2009),
//! as the app runs it: its checked-in component, packed as the registry
//! packs it (`scripts/build-plugin-registry.sh`), listed in a signed index,
//! installed through the verified install (T1709), granted, and loaded by
//! the workbench's plugin host in a headless window. Its panel is opened
//! from its rail button, and keys and ticks reach it through the realtime
//! panels' delivery (`fe_plugin::panels::PanelRuntime`, T2005/T2007) under
//! the host's real 100 ms call deadline.
//!
//! The panel's view is read back as the host holds it. Ticks come from the
//! test clock, one 50 ms tick at a time; to read the board between them, a
//! test pauses the game with `p` and waits for the paused view: the plugin
//! takes keys and ticks in one queue, in order, so the paused view comes
//! after every tick before it.
//!
//! The game's rules have native tests of their own, in the plugin's
//! `tests/logic.rs`; they are declared here too, so the workspace's test run
//! runs them.

#[path = "../../../extensions/plugins/blocks/tests/logic.rs"]
mod logic;

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Write as _,
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard},
    time::{Duration, Instant},
};

use fe_command::Invocation;
use fe_command::capability::{Capability, Grants};
use fe_gpui::{EditorSettings, EditorTheme, commands::RunCommand};
use fe_keymap::{CatalogError, CommandCatalog as _};
use fe_plugin::HostState;
use fe_plugin::fastedit::editor::panels::{GridNode, Host as _, NodeKind, Tone, View};
use fe_plugin::panels::{PanelViews, State};
use fe_registry::grammars::{Index, SCHEMA};
use fe_registry::plugins::{PluginEntry, verified_install};
use fe_registry::signing::{generate_key, sig_path, sign_index};
use fe_workbench::{
    GRANTS_FILE, Launch, ListedArchive, PluginPanel, PluginState, Workbench,
    install_listed_archive, plugin_kind, plugin_states, set_plugin_dirs, set_plugin_extensions,
};
use gpui::{
    Action as _, AnyWindowHandle, AppContext as _, Entity, Focusable as _, KeyDownEvent,
    KeyUpEvent, Keystroke, TestAppContext,
};
use kiln::core::{
    ApiVersion,
    extension::{ExtensionApi, ExtensionSet, load_manifest},
};
use kiln_platform::config::{self as platform_config, ConfigService};
use kiln_plugin::{
    grants::GrantStore,
    manifest::{ExtensionOrigin, PluginManifest},
};
use kiln_shell::DockView as _;

/// The plugin's extension id.
const ID: &str = "tenmoku.blocks";
/// The one panel it declares.
const PANEL: &str = "board";
/// The panel's id in the dock: the plugin's id, then the panel's.
const DOCK_ID: &str = "tenmoku.blocks.board";
/// `Blocks: Play`.
const PLAY: &str = "tenmoku.blocks.play";
/// The panel's `tick_ms`, the test clock's step.
const TICK: Duration = Duration::from_millis(50);
/// Gravity at level 1: a row a 500 ms of ticks.
const GRAVITY: Duration = Duration::from_millis(500);
/// The well.
const COLS: usize = 10;
const ROWS: usize = 20;

static SERIAL: Mutex<()> = Mutex::new(());

/// Runs the rest of a test with no other of this binary's running, and the
/// state directory `state`.
fn serially(state: &Path) -> MutexGuard<'static, ()> {
    let guard = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    // SAFETY: every test takes the lock before it starts gpui.
    unsafe { std::env::set_var("TENMOKU_STATE_DIR", state) };
    for var in ["TENMOKU_CONFIG_DIR", "XDG_CONFIG_HOME"] {
        // SAFETY: as above.
        unsafe { std::env::remove_var(var) };
    }
    guard
}

/// The plugin's directory in the checkout, read when the test runs.
fn plugin_dir() -> PathBuf {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("run by cargo");
    Path::new(&manifest).join("../../extensions/plugins/blocks")
}

/// The extension API this app checks manifests against.
fn api() -> ExtensionApi {
    ExtensionApi {
        app: fe_config::paths::app_id().short,
        version: ApiVersion { major: 1, minor: 1 },
        reserved: Vec::new(),
        kinds: vec![plugin_kind()],
        allowed_licenses: None,
        only_kinds: None,
    }
}

/// The capabilities the plugin's manifest asks for.
fn asked() -> Grants {
    [
        Capability::Panels,
        Capability::PanelsRealtime,
        Capability::Commands,
    ]
    .into_iter()
    .collect()
}

/// The plugin's archive as the registry script makes it: its manifest, its
/// component and its LICENSE and README, sorted, with no times or owners.
/// The bytes and how many entries it holds.
fn archive() -> (Vec<u8>, usize) {
    let dir = plugin_dir();
    let names = ["LICENSE", "README.md", "blocks.wasm", "extension.toml"];
    let mut tar = tar::Builder::new(Vec::new());
    for name in names {
        let bytes = fs::read(dir.join(name)).unwrap();
        let mut header = tar::Header::new_gnu();
        header.set_path(name).unwrap();
        header.set_size(bytes.len() as u64);
        header.set_mode(0o644);
        header.set_mtime(0);
        header.set_uid(0);
        header.set_gid(0);
        header.set_cksum();
        tar.append(&header, bytes.as_slice()).unwrap();
    }
    let tar = tar.into_inner().unwrap();
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    gz.write_all(&tar).unwrap();
    (gz.finish().unwrap(), names.len())
}

/// A signed index listing `entry`, in `dir`: its bytes, its signature and
/// the registry's public key.
fn signed_index(dir: &Path, entry: PluginEntry) -> (Vec<u8>, String, String) {
    let (public, private) = generate_key();
    let registry = dir.join("registry.toml");
    fs::write(
        &registry,
        format!(
            "schema = 1\nurl_template = \"u\"\nindex_url = \"u\"\npublic_key = \"{public}\"\n[packs]\n"
        ),
    )
    .unwrap();
    let index = Index {
        schema: SCHEMA,
        serial: 1,
        url_template: "https://example.com/{tag}/{file}".into(),
        packs: BTreeMap::new(),
        languages: BTreeMap::new(),
        plugins: BTreeMap::from([(entry.id.clone(), entry)]),
    };
    let path = dir.join("index.json");
    fs::write(&path, index.to_json()).unwrap();
    sign_index(&path, &registry, &private).unwrap();
    let signature = fs::read_to_string(sig_path(&path)).unwrap();
    (fs::read(&path).unwrap(), signature, public)
}

/// Installs the plugin into `extensions` from a signed index fixture, as an
/// install from the registry does: the index's signature and the archive's
/// size and hash are checked before it is unpacked, and its manifest is held
/// to the listed id, version and capabilities.
fn install_from_registry(dir: &Path, extensions: &Path) -> fe_workbench::Installed {
    let (archive, entries) = archive();
    let sha256 = sha2::Digest::finalize(<sha2::Sha256 as sha2::Digest>::new_with_prefix(&archive))
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let file = format!("{ID}-0.1.0.tar.gz");
    let listing = PluginEntry {
        id: ID.into(),
        name: "Blocks".into(),
        version: "0.1.0".into(),
        api_minor: 1,
        license: "Apache-2.0".into(),
        file: file.clone(),
        sha256,
        size: archive.len() as u64,
        entries: entries as u32,
        capabilities: vec!["panels".into(), "panels.realtime".into(), "commands".into()],
    };
    let index_dir = dir.join("registry");
    fs::create_dir_all(&index_dir).unwrap();
    let (index, signature, public) = signed_index(&index_dir, listing);
    let (entry, serial) = verified_install(&index, &signature, &public, None, ID, &archive)
        .expect("the archive is the one the signed index lists");
    assert_eq!(serial, 1);
    let downloaded = dir.join(&entry.file);
    fs::write(&downloaded, &archive).unwrap();
    let version = (0, 1, 0);
    let listed = ListedArchive {
        id: entry.id.clone(),
        version,
        sha256: entry.sha256.clone(),
        size: entry.size,
        entries: entry.entries as usize,
        capabilities: entry.capabilities.clone(),
    };
    let installed = install_listed_archive(&downloaded, &listed, extensions)
        .expect("the listed archive installs");
    assert_eq!((installed.id.as_str(), installed.version), (ID, version));
    assert!(installed.plugin);
    installed
}

/// The app, with the workbench's commands, the settings service's
/// `extensions.plugins_preview` on, and the plugin host keeping its files in
/// the test's directory, which is dropped last.
struct App {
    window: AnyWindowHandle,
    workbench: Entity<Workbench>,
    state: PathBuf,
    extensions: PathBuf,
    _service: gpui::Task<()>,
    _sender: platform_config::BatchSender,
    _serial: MutexGuard<'static, ()>,
    dir: tempfile::TempDir,
}

/// The app in a window of its own, active, with nothing installed.
fn app(cx: &mut TestAppContext) -> App {
    // Plugins run on threads of their own.
    cx.executor().allow_parking();
    let dir = tempfile::tempdir().unwrap();
    let state = dir.path().join("state");
    let cache = dir.path().join("cache");
    let extensions = dir.path().join("extensions");
    fs::create_dir_all(&extensions).unwrap();
    let serial = serially(&state);
    let user = dir.path().join("settings.toml");
    fs::write(&user, "[extensions]\nplugins_preview = true\n").unwrap();
    let (sender, batches) = platform_config::batch_channel();
    let store = fe_config::SettingsStore::load(Some(user), None);
    let service = cx.update(|cx| {
        fe_gpui::init(cx);
        fe_workbench::init(cx);
        let service = ConfigService::new()
            .settings(store.into_inner())
            .start_with(batches, cx);
        set_plugin_dirs(cx, state.clone(), cache);
        service
    });
    cx.run_until_parked();
    let launch = Launch {
        root: None,
        file: None,
    };
    let (workbench, vcx) = cx.add_window_view(move |window, cx| {
        let theme = EditorTheme::default_dark(cx);
        Workbench::new(launch, theme, EditorSettings::default(), window, cx)
    });
    vcx.update(|window, cx| {
        window.activate_window();
        window.focus(&workbench.focus_handle(cx), cx);
    });
    let window = cx.windows()[0];
    let app = App {
        window,
        workbench,
        state,
        extensions,
        _service: service,
        _sender: sender,
        _serial: serial,
        dir,
    };
    draw(cx, &app);
    app
}

impl App {
    /// Installs the plugin from the registry, grants what it asks for, as
    /// the install prompt would on Allow, and hands the extensions to the
    /// host; waits for it to run.
    fn install(&self, cx: &mut TestAppContext) {
        install_from_registry(self.dir.path(), &self.extensions);
        GrantStore::new(self.state.join(GRANTS_FILE))
            .grant(ID, (0, 1, 0), &asked(), &[], false)
            .unwrap();
        self.discover(cx);
        until(cx, "the plugin to run", |cx| {
            state(cx) == Some(PluginState::Running)
        });
    }

    /// Hands the host the extensions installed now.
    fn discover(&self, cx: &mut TestAppContext) {
        let set = ExtensionSet::discover(&self.extensions, &api());
        cx.update(|cx| set_plugin_extensions(cx, &set));
        cx.run_until_parked();
    }

    /// Clicks the panel's rail button: it shows, or hides if shown.
    fn click_rail(&self, cx: &mut TestAppContext) {
        let dock = self
            .workbench
            .read_with(cx, |workbench, _| workbench.dock().clone());
        let id = dock.read_with(cx, |dock, _| {
            (dock.state().panel_ids().into_iter())
                .find(|id| id.0 == DOCK_ID)
                .expect("the panel's rail button")
        });
        cx.update_window(self.window, |_, window, cx| {
            dock.update(cx, |dock, cx| dock.rail_clicked(id, window, cx))
        })
        .unwrap();
        draw(cx, self);
    }

    /// Opens the panel from its rail button, and waits for its ticks to run:
    /// it is shown and has focus.
    fn open(&self, cx: &mut TestAppContext) -> Entity<PluginPanel> {
        self.click_rail(cx);
        let panel = self.panel(cx).expect("the panel in the sidebar");
        until(cx, "the panel to tick", |cx| {
            panel.read_with(cx, |panel, _| panel.is_ticking())
        });
        panel
    }

    /// Runs Blocks: Play, as the palette does, where the window's focus is.
    fn play(&self, cx: &mut TestAppContext) {
        cx.update_window(self.window, |_, window, cx| {
            let stack = kiln_platform::keys::context_stack(&window.context_stack(), cx);
            let play = Invocation::new(PLAY.parse().unwrap()).context(stack);
            window.dispatch_action(RunCommand(play).boxed_clone(), cx);
        })
        .unwrap();
        draw(cx, self);
    }

    /// Runs Blocks: Play in `window`, as the palette does there, whichever
    /// window is active; runs nothing else.
    fn play_in(&self, cx: &mut TestAppContext, window: AnyWindowHandle) {
        cx.update_window(window, |_, window, cx| {
            let stack = kiln_platform::keys::context_stack(&window.context_stack(), cx);
            let play = Invocation::new(PLAY.parse().unwrap()).context(stack);
            kiln_platform::command_actions::dispatch_invocation(play, window, cx).unwrap();
        })
        .unwrap();
    }

    /// Waits for the panel to show with focus, by no click of the test's:
    /// it ticks only while it shows in the active window and has focus.
    fn revealed(&self, cx: &mut TestAppContext) -> Entity<PluginPanel> {
        let mut found = None;
        until(cx, "the panel to show with focus", |cx| {
            draw(cx, self);
            found =
                (self.panel(cx)).filter(|panel| panel.read_with(cx, |panel, _| panel.is_ticking()));
            found.is_some()
        });
        found.expect("found above")
    }

    fn panel(&self, cx: &mut TestAppContext) -> Option<Entity<PluginPanel>> {
        self.workbench.read_with(cx, |workbench, cx| {
            workbench.dock().read(cx).state().panel::<PluginPanel>()
        })
    }

    /// `key` pressed and released, as the keyboard sends it.
    fn press(&self, cx: &mut TestAppContext, key: &str) {
        let keystroke = Keystroke::parse(key).unwrap();
        cx.update_window(self.window, |_, window, cx| {
            window.dispatch_event(
                gpui::PlatformInput::KeyDown(KeyDownEvent {
                    keystroke: keystroke.clone(),
                    is_held: false,
                    prefer_character_input: false,
                }),
                cx,
            );
            window.dispatch_event(gpui::PlatformInput::KeyUp(KeyUpEvent { keystroke }), cx);
        })
        .unwrap();
        cx.run_until_parked();
    }

    /// One tick of the panel's: the test clock moves on by its `tick_ms`,
    /// and the plugin has a moment to take it. One it is still busy with
    /// the last isn't queued: its time goes with the next tick.
    fn tick(&self, cx: &mut TestAppContext) {
        cx.executor().advance_clock(TICK);
        draw(cx, self);
        std::thread::sleep(Duration::from_millis(2));
    }

    /// Pauses the game and reads the board once every key and tick before
    /// the pause has reached the plugin; resumes it before returning.
    fn settled_view(&self, cx: &mut TestAppContext, panel: &Entity<PluginPanel>) -> View {
        self.press(cx, "p");
        let paused = until_view(cx, panel, "the paused board", |view| {
            has_node(view, "paused")
        });
        self.press(cx, "p");
        until_view(cx, panel, "the board playing again", |view| {
            grid(view).is_some() && !has_node(view, "paused")
        });
        paused
    }
}

fn draw(cx: &mut TestAppContext, app: &App) {
    for _ in 0..2 {
        cx.run_until_parked();
        cx.update_window(app.window, |_, window, cx| window.draw(cx).clear(cx))
            .unwrap();
    }
    cx.run_until_parked();
}

/// Where the plugin stands, if the host has found it.
fn state(cx: &mut TestAppContext) -> Option<PluginState> {
    cx.update(|cx| {
        (plugin_states(cx).into_iter())
            .find(|info| info.id == ID)
            .map(|info| info.state)
    })
}

/// Runs until `done`: plugins load and take events on their own threads.
fn until(cx: &mut TestAppContext, what: &str, mut done: impl FnMut(&mut TestAppContext) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        cx.run_until_parked();
        if done(cx) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {what}: {:#?}",
            cx.update(|cx| plugin_states(cx))
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// The panel's view now: the plugin's latest the host has taken, once it
/// may show it (at most 20 a second).
fn view(cx: &mut TestAppContext, panel: &Entity<PluginPanel>) -> Option<View> {
    panel.update(cx, |panel, cx| {
        panel.refresh(cx);
        panel.view().cloned()
    })
}

/// Waits, with the test clock still, until the panel's view is one `wanted`
/// takes.
fn until_view(
    cx: &mut TestAppContext,
    panel: &Entity<PluginPanel>,
    what: &str,
    wanted: impl Fn(&View) -> bool,
) -> View {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        cx.run_until_parked();
        if let Some(view) = view(cx, panel)
            && wanted(&view)
        {
            return view;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {what}: {:#?}\n{:#?}",
            view(cx, panel),
            cx.update(|cx| plugin_states(cx))
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn has_node(view: &View, id: &str) -> bool {
    view.nodes.iter().any(|node| node.id == id)
}

/// The view's board, if it shows one.
fn grid(view: &View) -> Option<&GridNode> {
    view.nodes.iter().find_map(|node| match &node.kind {
        NodeKind::Grid(grid) => Some(grid),
        _ => None,
    })
}

/// Every text the view shows, headings included, in order.
fn texts(view: &View) -> Vec<String> {
    (view.nodes.iter())
        .filter_map(|node| match &node.kind {
            NodeKind::Text(text) => Some(text.text.clone()),
            NodeKind::Heading(text) => Some(text.clone()),
            _ => None,
        })
        .collect()
}

/// The text of node `id`.
fn text_of(view: &View, id: &str) -> Option<String> {
    view.nodes
        .iter()
        .find(|node| node.id == id)
        .and_then(|node| match &node.kind {
            NodeKind::Text(text) => Some(text.text.clone()),
            _ => None,
        })
}

/// A square of the board: its column, row and letter.
type Square = (usize, usize, String);

/// The board's blocks, falling or settled: the reversed cells.
fn blocks(view: &View) -> BTreeSet<Square> {
    squares(view, |cell| cell.reverse)
}

/// The ghost's squares: a letter in the muted tone, not reversed.
fn ghost(view: &View) -> BTreeSet<Square> {
    squares(view, |cell| {
        !cell.reverse && cell.tone == Tone::Muted && cell.ch != "·"
    })
}

fn squares(
    view: &View,
    keep: impl Fn(&fe_plugin::fastedit::editor::panels::Cell) -> bool,
) -> BTreeSet<Square> {
    let grid = grid(view).expect("the board");
    (grid.cells.iter().enumerate())
        .filter(|(_, cell)| keep(cell))
        .map(|(index, cell)| (index % COLS, index / COLS, cell.ch.clone()))
        .collect()
}

/// `squares` moved `rows` rows down.
fn down(squares: &BTreeSet<Square>, rows: usize) -> BTreeSet<Square> {
    (squares.iter())
        .map(|(col, row, ch)| (*col, row + rows, ch.clone()))
        .collect()
}

/// The squares of `squares` in rows `range`.
fn in_rows(squares: &BTreeSet<Square>, range: std::ops::Range<usize>) -> BTreeSet<Square> {
    (squares.iter())
        .filter(|(_, row, _)| range.contains(row))
        .cloned()
        .collect()
}

/// Checks `view` as the host checks every view a plugin sets, by setting it
/// on a panels store of the plugin's: the limits, the labels and the tree.
fn assert_valid(view: &View) {
    let grants: Grants = asked();
    let mut store = HostState {
        panels: State::new(ID, &grants, &[PANEL.to_owned()], &PanelViews::new()),
        ..HostState::default()
    };
    store
        .set_view(PANEL.into(), view.clone())
        .unwrap_or_else(|error| panic!("the view is refused: {error:?}\n{view:#?}"));
}

#[test]
fn the_manifest_asks_for_panels_realtime_and_commands_only() {
    let dir = plugin_dir();
    let (manifest, diagnostics) = load_manifest(&dir, &api());
    assert_eq!(diagnostics, [], "the manifest has problems");
    let manifest = manifest.unwrap();
    assert_eq!(manifest.id, ID);
    assert_eq!(manifest.license.expr, "Apache-2.0");
    let plugin = PluginManifest::from_extension(&manifest, ExtensionOrigin::User)
        .unwrap()
        .expect("it contributes a plugin");
    // No fs, net or process: the game reads nothing and runs nothing.
    assert_eq!(plugin.capabilities, asked());
    let panels: Vec<_> = (plugin.panels.iter())
        .map(|panel| (panel.id.as_str(), panel.tick_ms, panel.keys))
        .collect();
    assert_eq!(panels, [(PANEL, Some(50), true)]);
    // Listed in the signed index as an optional install: registry.toml is
    // the listing's marker (docs/release/plugin-registry.md).
    assert!(dir.join("registry.toml").is_file());
    assert!(
        manifest
            .description
            .as_deref()
            .is_some_and(|description| description.contains("falling-blocks game")
                && description.contains("Blocks: Play")),
        "{:?}",
        manifest.description
    );
}

#[gpui::test]
fn the_command_catalog_lists_blocks_only_once_installed(cx: &mut TestAppContext) {
    let app = app(cx);
    // A fresh install: nothing in the extensions folder, and no Blocks
    // command anywhere the palette or a keymap looks.
    app.discover(cx);
    assert_eq!(state(cx), None);
    let listed = |cx: &mut TestAppContext| -> Vec<(String, String)> {
        draw(cx, &app);
        cx.update_window(app.window, |_, window, cx| {
            let stack = kiln_platform::keys::context_stack(&window.context_stack(), cx);
            (kiln_shell::command_palette::commands_in(&stack, cx).into_iter())
                .filter(|entry| entry.id.as_str().starts_with("tenmoku.blocks."))
                .map(|entry| (entry.id.to_string(), entry.title.to_string()))
                .collect()
        })
        .unwrap()
    };
    assert_eq!(listed(cx), []);
    let catalog = fe_workbench::command_catalog();
    assert_eq!(
        catalog.check(&PLAY.parse().unwrap(), &serde_json::Value::Null),
        Err(CatalogError::UnknownCommand)
    );
    assert!(
        !(fe_workbench::registry_commands().iter())
            .any(|(id, _)| id.as_str().starts_with("tenmoku.blocks."))
    );

    // Installed from the registry and granted: Blocks: Play is listed, and
    // the Extensions list has it as any other plugin.
    app.install(cx);
    let installed = app.extensions.join(ID);
    assert!(
        installed.join(".registry-verified").is_file(),
        "installed as the signed index listed it"
    );
    assert_eq!(listed(cx), [(PLAY.to_owned(), "Blocks: Play".to_owned())]);
    let info = cx.update(|cx| plugin_states(cx).into_iter().find(|info| info.id == ID));
    let info = info.expect("the plugin in the extensions list");
    assert_eq!(info.name, "Blocks");
    assert_eq!(info.granted, asked());

    // Blocks: Play starts a game, and shows its panel with the board.
    app.play(cx);
    let panel = app.revealed(cx);
    let view = until_view(cx, &panel, "the board", |view| grid(view).is_some());
    assert_eq!(blocks(&view).len(), 4, "one piece entering");
    assert_eq!(state(cx), Some(PluginState::Running));
}

#[gpui::test]
fn blocks_panel_view_is_valid(cx: &mut TestAppContext) {
    let app = app(cx);
    app.install(cx);
    let panel = app.open(cx);
    // The start screen, as the host took it.
    let start = until_view(cx, &panel, "the start screen", |view| {
        has_node(view, "start")
    });
    assert_valid(&start);
    assert!(grid(&start).is_none());

    // The board.
    app.press(cx, "enter");
    let board = until_view(cx, &panel, "the board", |view| grid(view).is_some());
    assert_valid(&board);
    let grid = grid(&board).unwrap();
    assert_eq!((grid.cols as usize, grid.rows as usize), (COLS, ROWS));
    assert_eq!(grid.cells.len(), COLS * ROWS);
    assert!(!grid.label.is_empty());
    // One piece entering, in its kind's letter and a tone that isn't muted;
    // its ghost below it in the muted tone; empty squares a muted dot.
    let falling = blocks(&board);
    assert_eq!(falling.len(), 4, "{falling:?}");
    let letters: BTreeSet<&str> = falling.iter().map(|(_, _, ch)| ch.as_str()).collect();
    assert_eq!(letters.len(), 1);
    let letter = *letters.first().unwrap();
    assert!("IOTSZJL".contains(letter), "{letter}");
    let ghost = ghost(&board);
    assert_eq!(ghost.len(), 4, "{ghost:?}");
    assert!(
        ghost
            .iter()
            .all(|(_, row, ch)| ch == letter && *row >= ROWS - 2)
    );
    for cell in &grid.cells {
        match (cell.reverse, cell.ch.as_str()) {
            (true, _) => assert_ne!(cell.tone, Tone::Muted),
            (false, "·") => assert_eq!(cell.tone, Tone::Muted),
            (false, ch) => assert_eq!((ch, cell.tone), (letter, Tone::Muted)),
        }
    }
    // Beside it, the score and the next piece as text.
    assert_eq!(text_of(&board, "score").as_deref(), Some("Score 0"));
    assert_eq!(text_of(&board, "level").as_deref(), Some("Level 1"));
    let next = text_of(&board, "next").expect("the next piece");
    assert!(next.starts_with("Next: "), "{next}");
    assert!(has_node(&board, "next-0"));

    // Paused, it is still a valid view.
    app.press(cx, "p");
    let paused = until_view(cx, &panel, "the paused board", |view| {
        has_node(view, "paused")
    });
    assert_valid(&paused);
}

#[gpui::test]
fn a_tick_moves_the_piece_down_one_row(cx: &mut TestAppContext) {
    let app = app(cx);
    app.install(cx);
    let panel = app.open(cx);
    app.press(cx, "enter");
    let start = app.settled_view(cx, &panel);
    let (entering, landing) = (blocks(&start), ghost(&start));
    assert_eq!(entering.len(), 4);
    // Ticks add up to gravity's 500 ms, then one moves the piece a row; the
    // ghost, where it will land, stays.
    let most = GRAVITY.as_millis() / TICK.as_millis();
    let mut ticks = 0;
    let moved = loop {
        app.tick(cx);
        ticks += 1;
        let view = app.settled_view(cx, &panel);
        if blocks(&view) != entering {
            break view;
        }
        assert_eq!(ghost(&view), landing);
        assert!(ticks < most, "{ticks} ticks and the piece hasn't moved");
    };
    assert_eq!(blocks(&moved), down(&entering, 1), "after {ticks} ticks");
    assert_eq!(ghost(&moved), landing);
    // The next tick doesn't move it again: gravity starts over.
    app.tick(cx);
    assert_eq!(blocks(&app.settled_view(cx, &panel)), down(&entering, 1));
}

#[gpui::test]
fn hard_drop_then_ticks_ends_in_a_stable_board(cx: &mut TestAppContext) {
    let app = app(cx);
    app.install(cx);
    let panel = app.open(cx);
    app.press(cx, "enter");
    let start = app.settled_view(cx, &panel);
    let landing = ghost(&start);
    // Space drops the piece onto its ghost, and the next comes in at the
    // top.
    app.press(cx, "space");
    let dropped = app.settled_view(cx, &panel);
    let settled = in_rows(&blocks(&dropped), ROWS / 2..ROWS);
    let as_blocks: BTreeSet<(usize, usize)> = settled.iter().map(|(c, r, _)| (*c, *r)).collect();
    let as_ghost: BTreeSet<(usize, usize)> = landing.iter().map(|(c, r, _)| (*c, *r)).collect();
    assert_eq!(as_blocks, as_ghost, "the piece settled where its ghost was");
    let entering = in_rows(&blocks(&dropped), 0..ROWS / 2);
    assert_eq!(entering.len(), 4, "the next piece came in");
    let score = text_of(&dropped, "score").unwrap();
    assert_ne!(score, "Score 0", "a hard drop scores its rows");

    // Ticks move the new piece down; what settled stays as it is.
    for _ in 0..3 * (GRAVITY.as_millis() / TICK.as_millis()) {
        app.tick(cx);
    }
    let later = app.settled_view(cx, &panel);
    assert_eq!(in_rows(&blocks(&later), ROWS / 2..ROWS), settled);
    let falling = in_rows(&blocks(&later), 0..ROWS / 2);
    let fell = (1..ROWS / 2)
        .find(|&rows| down(&entering, rows) == falling)
        .unwrap_or_else(|| panic!("{entering:?} didn't fall as one piece: {falling:?}"));
    assert!(fell <= 3, "{fell} rows in 1.5 s of ticks");
    assert_eq!(text_of(&later, "score").unwrap(), score);

    // A second piece dropped settles on the board too, and ticks leave both.
    app.press(cx, "space");
    let second = app.settled_view(cx, &panel);
    let stack: BTreeSet<Square> = blocks(&second)
        .into_iter()
        .filter(|(_, row, _)| *row >= ROWS / 2)
        .collect();
    assert_eq!(stack.len(), 8, "{stack:?}");
    assert!(settled.is_subset(&stack));
    for _ in 0..GRAVITY.as_millis() / TICK.as_millis() {
        app.tick(cx);
    }
    let after = app.settled_view(cx, &panel);
    assert_eq!(in_rows(&blocks(&after), ROWS / 2..ROWS), stack);
    assert_eq!(state(cx), Some(PluginState::Running));
}

#[gpui::test]
fn a_tick_stays_under_the_call_deadline(cx: &mut TestAppContext) {
    // The host holds every call to its real deadline, 100 ms, and stops a
    // plugin that overruns one (StopReason::Timeout). Several games are
    // played through here, ticks and hard drops filling the well to the top
    // and restarting; the plugin must still be running at the end.
    let app = app(cx);
    app.install(cx);
    let panel = app.open(cx);
    app.press(cx, "enter");
    let mut over = 0;
    let started = Instant::now();
    for step in 0..400 {
        app.tick(cx);
        if step % 3 == 0 {
            app.press(cx, "space");
        }
        if step % 25 == 24 {
            app.press(cx, "left");
            app.press(cx, "up");
        }
        if let Some(view) = view(cx, &panel)
            && has_node(&view, "over")
        {
            over += 1;
            app.press(cx, "enter");
        }
        if let Some(PluginState::Stopped(diagnostic)) = state(cx) {
            panic!("the plugin stopped at step {step}: {diagnostic:?}");
        }
    }
    let elapsed = started.elapsed();
    until(cx, "the last keys and ticks to be taken", |cx| {
        view(cx, &panel).is_some_and(|view| grid(&view).is_some() || has_node(&view, "over"))
    });
    assert_eq!(state(cx), Some(PluginState::Running), "after {elapsed:?}");
    assert!(over >= 2, "{over} games over: the well wasn't filled");
}

#[gpui::test]
fn no_ticks_when_hidden(cx: &mut TestAppContext) {
    let app = app(cx);
    app.install(cx);
    let panel = app.open(cx);
    app.press(cx, "enter");
    let before = app.settled_view(cx, &panel);
    // Hidden, the panel has no ticks, however long the clock runs.
    app.click_rail(cx);
    until(cx, "the panel to hide", |cx| {
        panel.read_with(cx, |panel, _| !panel.is_shown() && !panel.is_ticking())
    });
    for _ in 0..4 * (GRAVITY.as_millis() / TICK.as_millis()) {
        app.tick(cx);
        assert!(!panel.read_with(cx, |panel, _| panel.is_ticking()));
    }
    // Shown again, the board is as it was: no tick reached the game.
    let panel = app.open(cx);
    let back = app.settled_view(cx, &panel);
    assert_eq!(grid(&back).unwrap().cells, grid(&before).unwrap().cells);
    // And the ticks are back.
    let mut ticks = 0;
    while blocks(&app.settled_view(cx, &panel)) == blocks(&before) {
        app.tick(cx);
        ticks += 1;
        assert!(ticks <= 10, "the ticks didn't come back");
    }
}

#[gpui::test]
fn the_start_screen_shows_the_controls_hint(cx: &mut TestAppContext) {
    let app = app(cx);
    app.install(cx);
    let panel = app.open(cx);
    // Opened, the panel names the start key and the controls.
    let start = until_view(cx, &panel, "the start screen", |view| {
        has_node(view, "start")
    });
    let shown = texts(&start).join("\n");
    assert!(shown.contains("Press Enter to start."), "{shown}");
    for control in [
        "move",
        "rotate",
        "soft drop",
        "Space",
        "hard drop",
        "pause",
        "restart",
        "Esc",
        "back to the editor",
        "h l",
    ] {
        assert!(shown.contains(control), "no {control:?} in:\n{shown}");
    }
    assert!(!shown.contains("Best"), "no best score before a game");

    // Playing, one line of controls stays beside the board.
    app.press(cx, "enter");
    let board = until_view(cx, &panel, "the board", |view| grid(view).is_some());
    let hint = text_of(&board, "hint").expect("the controls line");
    assert!(!hint.contains('\n'));
    for control in ["move", "rotate", "Space", "P pause", "R restart", "Esc"] {
        assert!(hint.contains(control), "no {control:?} in {hint:?}");
    }

    // Over, the start screen is back, with the score, the best score kept
    // in the plugin's data, and the key to play again.
    // Hard drops where the pieces come in fill the well's middle to the top.
    let mut last = board;
    let over = loop {
        app.press(cx, "space");
        let now = until_view(cx, &panel, "the drop", |view| *view != last);
        if has_node(&now, "over") {
            break now;
        }
        last = now;
    };
    let shown = texts(&over).join("\n");
    assert!(shown.contains("Game over: "), "{shown}");
    assert!(shown.contains("Press Enter to play again."), "{shown}");
    assert!(shown.contains("move") && shown.contains("Esc"), "{shown}");
    let best = text_of(&over, "best").expect("the best score");
    let kept = fs::read_to_string(app.state.join("plugin-data").join(ID).join("best-score"))
        .expect("the best score in the plugin's data");
    assert_eq!(best, format!("Best: {kept} points"));
    // And Enter plays again.
    app.press(cx, "enter");
    until_view(cx, &panel, "a new game", |view| {
        grid(view).is_some() && text_of(view, "score").as_deref() == Some("Score 0")
    });
}

#[gpui::test]
fn blocks_play_reveals_and_focuses_the_panel(cx: &mut TestAppContext) {
    let app = app(cx);
    app.install(cx);
    // Never opened: no click on its rail button, and Blocks: Play shows it,
    // focused, so its keys reach the game.
    let panel = app.panel(cx).expect("the panel in the sidebar");
    assert!(!panel.read_with(cx, |panel, _| panel.is_shown() || panel.is_ticking()));
    app.play(cx);
    let panel = app.revealed(cx);
    let board = until_view(cx, &panel, "the board", |view| grid(view).is_some());
    let focused = cx
        .update_window(app.window, |_, window, cx| {
            panel.focus_handle(cx).contains_focused(window, cx)
        })
        .unwrap();
    assert!(focused);
    app.press(cx, "p");
    until_view(cx, &panel, "the game paused by its key", |view| {
        has_node(view, "paused")
    });

    // Hidden, then Blocks: Play again: shown and focused again, and the
    // paused game resumes where it was.
    app.click_rail(cx);
    until(cx, "the panel to hide", |cx| {
        panel.read_with(cx, |panel, _| !panel.is_shown() && !panel.is_ticking())
    });
    app.play(cx);
    let panel = app.revealed(cx);
    let resumed = until_view(cx, &panel, "the game resumed", |view| {
        grid(view).is_some() && !has_node(view, "paused")
    });
    assert_eq!(blocks(&resumed), blocks(&board));
    // Its keys still reach it: a hard drop.
    app.press(cx, "space");
    until_view(cx, &panel, "the drop", |view| {
        text_of(view, "score").is_some_and(|score| score != "Score 0")
    });
    assert_eq!(state(cx), Some(PluginState::Running));
}

/// A second window of the app, active, with a workbench of its own.
fn second_window(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<Workbench>) {
    let launch = Launch {
        root: None,
        file: None,
    };
    let (workbench, vcx) = cx.add_window_view(move |window, cx| {
        let theme = EditorTheme::default_dark(cx);
        Workbench::new(launch, theme, EditorSettings::default(), window, cx)
    });
    vcx.update(|window, cx| {
        window.activate_window();
        window.focus(&workbench.focus_handle(cx), cx);
    });
    let window = *cx.windows().last().unwrap();
    draw_window(cx, window);
    (window, workbench)
}

fn draw_window(cx: &mut TestAppContext, window: AnyWindowHandle) {
    for _ in 0..2 {
        cx.run_until_parked();
        cx.update_window(window, |_, window, cx| window.draw(cx).clear(cx))
            .unwrap();
    }
    cx.run_until_parked();
}

/// Whether `workbench`'s sidebar shows the Blocks panel.
fn sidebar_shows_blocks(cx: &mut TestAppContext, workbench: &Entity<Workbench>) -> bool {
    workbench.read_with(cx, |workbench, cx| {
        let dock = workbench.dock().read(cx);
        let state = dock.state();
        let blocks = (state.panel_ids().into_iter()).find(|id| id.0 == DOCK_ID);
        blocks.is_some() && state.is_visible() && state.active() == blocks
    })
}

#[gpui::test]
fn blocks_play_in_two_windows_reveals_the_panel_in_each(cx: &mut TestAppContext) {
    let app = app(cx);
    let (second, workbench) = second_window(cx);
    app.install(cx);
    draw_window(cx, second);
    assert!(!sidebar_shows_blocks(cx, &app.workbench));
    assert!(!sidebar_shows_blocks(cx, &workbench));
    // Run in the window that isn't active, then in the one that is, both
    // before either call reaches the plugin: each run reveals the panel in
    // the window it was run in, whichever call the plugin takes first.
    cx.update_window(app.window, |_, window, _| window.activate_window())
        .unwrap();
    app.play_in(cx, second);
    until(
        cx,
        "the panel to show in the window Blocks: Play ran in",
        |cx| {
            draw_window(cx, second);
            sidebar_shows_blocks(cx, &workbench)
        },
    );
    assert!(!sidebar_shows_blocks(cx, &app.workbench));
    // Hidden there again, then run in both at once.
    let dock = workbench.read_with(cx, |workbench, _| workbench.dock().clone());
    let id = dock.read_with(cx, |dock, _| {
        (dock.state().panel_ids().into_iter())
            .find(|id| id.0 == DOCK_ID)
            .unwrap()
    });
    cx.update_window(second, |_, window, cx| {
        dock.update(cx, |dock, cx| dock.rail_clicked(id, window, cx))
    })
    .unwrap();
    draw_window(cx, second);
    assert!(!sidebar_shows_blocks(cx, &workbench));
    app.play_in(cx, second);
    app.play_in(cx, app.window);
    until(cx, "the panel to show in both windows", |cx| {
        draw(cx, &app);
        draw_window(cx, second);
        sidebar_shows_blocks(cx, &app.workbench) && sidebar_shows_blocks(cx, &workbench)
    });
    assert_eq!(state(cx), Some(PluginState::Running));
}
