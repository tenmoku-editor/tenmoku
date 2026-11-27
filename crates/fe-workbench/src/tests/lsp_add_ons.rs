//! Add-on language servers in the workbench
//! (docs/decisions/q5j03ax2af-lsp-add-on-servers.md §5, §6.2, §7): one
//! stylesheet on vscode-css-language-server, its primary, and on an add-on
//! beside it, each a fake of its own. Hints, lenses and code actions come
//! from both, folds and semantic tokens from their provider alone, each
//! follow-up from its item's owner; a path keeps its holder whatever runs;
//! and an add-on's hints changed by its index, with no edit, show at its
//! next poll.

use std::{
    fs, io,
    path::{Path, PathBuf},
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use fe_gpui::{EditorView, Lens};
use fe_lsp::fake::{
    FakeLauncher, FakeServer, InitializeReply, Record, Reply, Script, Scripted, initialize_result,
};
use fe_lsp::{
    LaunchCancel, ManualClock, ServerId, ServerIo, ServerLauncher, ServerSpec,
    code_action::CodeActionQuery,
};
use gpui::{Entity, Focusable as _, TestAppContext, VisualTestContext};
use lsp_types::{
    CodeActionProviderCapability, CodeLensOptions, FoldingRangeProviderCapability,
    InlayHintOptions, InlayHintServerCapabilities, OneOf, PositionEncodingKind, SemanticTokenType,
    SemanticTokensFullOptions, SemanticTokensLegend, SemanticTokensOptions,
    SemanticTokensServerCapabilities, ServerCapabilities, Url,
};
use serde_json::{Value, json};

use super::lsp::window;
use crate::{Launch, Workbench, lsp::TestClock, open::ConfigDirOverride, set_server_launcher};

/// The built-in primary for CSS and SCSS, here on its own fake.
pub(crate) const PRIMARY: &str = "vscode-css-language-server";
/// An add-on for CSS alone.
pub(crate) const ADD_ON: &str = "css-vars";
/// The add-on's `repoll_ms`.
pub(crate) const REPOLL: Duration = Duration::from_millis(3000);
/// The stylesheet the tests open.
pub(crate) const STYLE: &str = "src/style.css";
/// The text it starts with.
pub(crate) const TEXT: &str = "a { color: red; }\n";

const HINT: &str = "textDocument/inlayHint";
const LENS: &str = "textDocument/codeLens";
const LENS_RESOLVE: &str = "codeLens/resolve";
const ACTION: &str = "textDocument/codeAction";
const FOLDING: &str = "textDocument/foldingRange";
const TOKENS: &str = "textDocument/semanticTokens/full";

/// The fakes' launcher: each of [`PRIMARY`] and [`ADD_ON`] to its own
/// fake, the primary "not found" while `primary_missing` is set, and any
/// other server not found.
struct Launchers {
    primary: Arc<FakeLauncher>,
    add_on: Arc<FakeLauncher>,
    primary_missing: Arc<AtomicBool>,
}

impl ServerLauncher for Launchers {
    fn launch(
        &self,
        spec: &ServerSpec,
        root: &Path,
        cancel: &LaunchCancel,
    ) -> io::Result<ServerIo> {
        let missing = || io::Error::new(io::ErrorKind::NotFound, "no such file");
        match spec.name.as_str() {
            PRIMARY if self.primary_missing.load(Ordering::SeqCst) => Err(missing()),
            PRIMARY => self.primary.launch(spec, root, cancel),
            ADD_ON => self.add_on.launch(spec, root, cancel),
            _ => Err(missing()),
        }
    }
}

/// A script whose server says it can do what `give` adds.
pub(crate) fn script(give: impl FnOnce(&mut ServerCapabilities)) -> Script {
    let mut init = initialize_result(Some(PositionEncodingKind::UTF8));
    give(&mut init.capabilities);
    Script {
        initialize: InitializeReply::Result(Box::new(init)),
        ..Script::default()
    }
}

/// A workbench rooted at a temporary project with `src/style.css` and
/// `src/theme.scss`, its root trusted, the primary and the add-on each on
/// a fake of its own, and every clock the manager's manual one.
pub(crate) struct AddOns {
    pub(crate) workbench: Entity<Workbench>,
    pub(crate) cx: VisualTestContext,
    pub(crate) primary: FakeServer,
    pub(crate) add_on: FakeServer,
    pub(crate) clock: Arc<ManualClock>,
    /// The project's folder, resolved, as the manager has it.
    pub(crate) root: PathBuf,
    _dir: tempfile::TempDir,
    _config: Rc<tempfile::TempDir>,
}

impl AddOns {
    /// The fakes following `primary` and `add_on`, the primary missing if
    /// `primary_missing`.
    pub(crate) fn new(
        cx: &mut TestAppContext,
        primary: Script,
        add_on: Script,
        primary_missing: bool,
    ) -> Self {
        // Server threads wake the workbench's event loop.
        cx.executor().allow_parking();
        let clock = Arc::new(ManualClock::new());
        let primary = FakeServer::with_clock(primary, clock.clone());
        let add_on = FakeServer::with_clock(add_on, clock.clone());
        let launcher = Arc::new(Launchers {
            primary: primary.launcher(),
            add_on: add_on.launcher(),
            primary_missing: Arc::new(AtomicBool::new(primary_missing)),
        });
        let config = Rc::new(tempfile::tempdir().unwrap());
        // The add-on serves CSS alone, so an SCSS alias of a stylesheet has
        // the same lead and another set of add-ons.
        let settings = format!(
            "[lsp.servers.{PRIMARY}]\ncommand = \"fake-css\"\n\n\
             [lsp.servers.{ADD_ON}]\ncommand = \"fake-vars\"\nlanguages = [\"css\"]\n\
             add_on = true\nrepoll_ms = {}\n",
            REPOLL.as_millis()
        );
        fs::write(
            config.path().join(fe_config::paths::SETTINGS_FILE),
            settings,
        )
        .unwrap();
        let manager_clock = clock.clone();
        let config_dir = config.path().to_owned();
        cx.update(|cx| {
            set_server_launcher(cx, launcher);
            cx.set_global(TestClock(manager_clock));
            cx.set_global(ConfigDirOverride(Some(config_dir)));
        });
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("src")).unwrap();
        fs::write(dir.path().join(STYLE), TEXT).unwrap();
        fs::write(dir.path().join("src/theme.scss"), "b { margin: 0; }\n").unwrap();
        let root = dunce::canonicalize(dir.path()).unwrap();
        let launch = Launch {
            root: Some(root.clone()),
            file: None,
        };
        let (workbench, cx) = window(cx, launch);
        let mut s = Self {
            workbench,
            cx,
            primary,
            add_on,
            clock,
            root,
            _dir: dir,
            _config: config,
        };
        s.draw();
        let workbench = s.workbench.clone();
        workbench.update(&mut s.cx, |workbench, cx| {
            workbench.set_root_trust(true, cx)
        });
        s.draw();
        s
    }

    /// Lets background work land, then draws a frame, twice.
    pub(crate) fn draw(&mut self) {
        for _ in 0..2 {
            self.cx.run_until_parked();
            self.cx.update(|window, cx| window.draw(cx).clear(cx));
        }
    }

    /// Draws until `done` holds, moving the manager's clock 20 ms each
    /// time so its capability polls wake. Panics after 60 s, naming `what`.
    pub(crate) fn until(&mut self, what: &str, done: impl Fn(&mut Self) -> bool) {
        let deadline = std::time::Instant::now() + Duration::from_secs(60);
        loop {
            self.draw();
            if done(self) {
                return;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "timed out waiting for {what}"
            );
            self.clock.advance(Duration::from_millis(20));
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// Moves the executor's clock, which the workbench's waits run on.
    pub(crate) fn wait(&mut self, by: Duration) {
        self.cx.executor().advance_clock(by);
        self.draw();
    }

    pub(crate) fn path(&self, name: &str) -> PathBuf {
        let mut path = self.root.clone();
        path.extend(name.split('/'));
        path
    }

    pub(crate) fn uri(&self, name: &str) -> Url {
        Url::from_file_path(self.path(name)).unwrap()
    }

    /// Opens `name` in the active pane, focused, and returns its editor.
    pub(crate) fn open(&mut self, name: &str) -> Entity<EditorView> {
        let pane = self.workbench.read_with(&self.cx, |w, _| w.pane().clone());
        let path = self.path(name);
        self.cx
            .update(|window, cx| pane.update(cx, |pane, cx| pane.open(path, window, cx)));
        self.draw();
        let editor = self.active_editor();
        self.cx
            .update(|window, cx| window.focus(&editor.focus_handle(cx), cx));
        self.draw();
        editor
    }

    pub(crate) fn active_editor(&mut self) -> Entity<EditorView> {
        self.workbench
            .read_with(&self.cx, |w, cx| w.active_editor(cx).cloned())
            .expect("a tab is shown")
    }

    /// Opens [`STYLE`] and waits until both servers have it.
    pub(crate) fn open_style(&mut self) -> Entity<EditorView> {
        let editor = self.open(STYLE);
        let uri = self.uri(STYLE);
        for fake in [&self.primary, &self.add_on] {
            fake.wait_for_document(&uri, "didOpen", |r| r.is_some_and(|r| r.open));
        }
        editor
    }

    /// The params of each `method` request `fake` got, in any launch.
    pub(crate) fn asked(fake: &FakeServer, method: &str) -> Vec<Value> {
        (fake.records().into_iter())
            .filter_map(|record| match record {
                Record::Request {
                    method: asked,
                    params,
                    ..
                } if asked == method => Some(params),
                _ => None,
            })
            .collect()
    }

    /// The notifications named `method` launch `launch` of `fake` got for
    /// `uri`.
    pub(crate) fn sent(fake: &FakeServer, launch: usize, method: &str, uri: &Url) -> usize {
        (fake.records_of(launch).iter())
            .filter(|record| {
                matches!(record, Record::Notification { method: m, params }
                    if m == method && params["textDocument"]["uri"] == uri.as_str())
            })
            .count()
    }

    /// `buffer`'s placements by the manager's account: whether it's
    /// served, its lead's instance, and the instances it's on.
    pub(crate) fn placed(
        &mut self,
        buffer: fe_buffer::BufferId,
    ) -> (bool, Option<ServerId>, Vec<ServerId>) {
        self.workbench.read_with(&self.cx, |w, _| {
            let manager = w.lsp.manager().expect("a manager");
            let manager = manager.lock().unwrap();
            (
                manager.is_served(buffer),
                manager.lead_of(buffer),
                manager.servers_of(buffer),
            )
        })
    }

    pub(crate) fn owners_match_manager(&mut self) -> bool {
        self.workbench
            .read_with(&self.cx, |w, _| w.lsp.owners_match_manager())
    }
}

fn buffer_of(s: &mut AddOns, editor: &Entity<EditorView>) -> fe_buffer::BufferId {
    editor.read_with(&s.cx, |view, _| view.model().buffer().id())
}

/// Hints with no resolve, lenses resolved, code actions.
fn hints_lenses_actions(capabilities: &mut ServerCapabilities) {
    capabilities.inlay_hint_provider = Some(OneOf::Right(InlayHintServerCapabilities::Options(
        InlayHintOptions {
            work_done_progress_options: Default::default(),
            resolve_provider: None,
        },
    )));
    capabilities.code_lens_provider = Some(CodeLensOptions {
        resolve_provider: Some(true),
    });
    capabilities.code_action_provider = Some(CodeActionProviderCapability::Simple(true));
}

/// A hint at `character` of the first line, labelled `label`.
fn hint(character: u32, label: &str) -> Scripted {
    Scripted::now(Reply::Result(json!([
        { "position": { "line": 0, "character": character }, "label": label, "kind": 1 },
    ])))
}

/// The hints `editor` shows, by offset and label.
fn hints(s: &AddOns, editor: &Entity<EditorView>) -> Vec<(usize, String)> {
    editor.read_with(&s.cx, |view, _| view.inlay_hints())
}

/// The lens rows `editor` shows: row, then the item titles.
fn lenses(s: &AddOns, editor: &Entity<EditorView>) -> Vec<(u32, Vec<String>)> {
    editor.read_with(&s.cx, |view, _| {
        let snapshot = view.model().snapshot().clone();
        (view.lens_providers().iter())
            .flat_map(|provider| provider.borrow_mut().lenses(&snapshot))
            .filter(|lens: &Lens| !lens.items.is_empty())
            .map(|lens| {
                let titles = lens.items.iter().map(|i| i.title.to_string()).collect();
                (lens.row, titles)
            })
            .collect()
    })
}

/// A lens over the first line, titled `title`, or with no command (to be
/// resolved) if `None`.
fn lens(title: Option<&str>) -> Scripted {
    let range = json!({
        "start": { "line": 0, "character": 0 },
        "end": { "line": 0, "character": 1 },
    });
    let lens = match title {
        Some(title) => json!({ "range": range, "command": { "title": title, "command": "x" } }),
        None => json!({ "range": range, "data": 7 }),
    };
    Scripted::now(Reply::Result(json!([lens])))
}

/// An action titled `title`, a quick fix with no edit.
fn action(title: &str) -> Scripted {
    Scripted::now(Reply::Result(json!([
        { "title": title, "kind": "quickfix", "command": { "title": title, "command": "fix" } },
    ])))
}

#[gpui::test]
fn hints_lenses_and_actions_come_from_both_servers(cx: &mut TestAppContext) {
    let mut s = AddOns::new(
        cx,
        script(hints_lenses_actions),
        script(hints_lenses_actions),
        false,
    );
    s.primary.set_reply(HINT, hint(1, "p"));
    s.add_on.set_reply(HINT, hint(4, "v"));
    s.primary.set_reply(LENS, lens(Some("primary lens")));
    // The add-on's lens comes unresolved: its resolve goes to it alone.
    s.add_on.set_reply(LENS, lens(None));
    s.add_on.set_reply(
        LENS_RESOLVE,
        Scripted::now(Reply::Result(json!({
            "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 1 } },
            "command": { "title": "add-on lens", "command": "y" },
        }))),
    );
    s.primary.set_reply(ACTION, action("primary fix"));
    s.add_on.set_reply(ACTION, action("add-on fix"));
    let editor = s.open_style();

    // Hints: merged, by position, then server.
    s.wait(Duration::from_millis(300));
    s.until("both servers' hints", |s| hints(s, &editor).len() == 2);
    assert_eq!(
        hints(&s, &editor),
        [(1, "p".to_owned()), (4, "v".to_owned())]
    );
    assert_eq!(AddOns::asked(&s.primary, HINT).len(), 1);
    assert_eq!(AddOns::asked(&s.add_on, HINT).len(), 1);

    // Lenses: both on the row, each resolved by its owner.
    s.until("both servers' lenses", |s| {
        lenses(s, &editor)
            .iter()
            .map(|(_, t)| t.len())
            .sum::<usize>()
            == 2
    });
    assert_eq!(
        lenses(&s, &editor),
        [(0, vec!["primary lens".to_owned(), "add-on lens".to_owned()])]
    );
    assert_eq!(AddOns::asked(&s.add_on, LENS_RESOLVE).len(), 1);
    assert!(AddOns::asked(&s.primary, LENS_RESOLVE).is_empty());

    // Actions: both servers', in server order.
    let buffer = buffer_of(&mut s, &editor);
    let version = editor.read_with(&s.cx, |view, _| view.model().buffer().version());
    let actioner = s
        .workbench
        .read_with(&s.cx, |w, _| w.lsp_code_actioner(buffer))
        .expect("the buffer is served");
    let asked = actioner.ask(
        buffer,
        version,
        0..1,
        Vec::new(),
        CodeActionQuery::default(),
    );
    let answer = Arc::new(Mutex::new(None));
    let answered = answer.clone();
    s.cx.spawn(async move |_| {
        let actions = asked.await;
        *answered.lock().unwrap() = Some(actions);
    })
    .detach();
    s.until("the actions", |_| answer.lock().unwrap().is_some());
    let actions = answer.lock().unwrap().take().unwrap().expect("actions");
    let titles: Vec<&str> = actions.iter().map(|a| a.title.as_str()).collect();
    assert_eq!(titles, ["primary fix", "add-on fix"]);
    assert!(!AddOns::asked(&s.primary, ACTION).is_empty());
    assert!(!AddOns::asked(&s.add_on, ACTION).is_empty());
}

/// Full semantic tokens with one type.
fn tokens(capabilities: &mut ServerCapabilities) {
    capabilities.semantic_tokens_provider = Some(
        SemanticTokensServerCapabilities::SemanticTokensOptions(SemanticTokensOptions {
            legend: SemanticTokensLegend {
                token_types: vec![SemanticTokenType::PROPERTY],
                token_modifiers: Vec::new(),
            },
            full: Some(SemanticTokensFullOptions::Bool(true)),
            ..Default::default()
        }),
    );
}

#[gpui::test]
fn folds_and_tokens_come_from_the_provider(cx: &mut TestAppContext) {
    // The primary has tokens and no folds; the add-on folds and has no
    // tokens. Each is asked for what it provides alone.
    let mut s = AddOns::new(
        cx,
        script(tokens),
        script(|capabilities| {
            capabilities.folding_range_provider =
                Some(FoldingRangeProviderCapability::Simple(true));
        }),
        false,
    );
    s.primary.set_reply(
        TOKENS,
        Scripted::now(Reply::Result(json!({ "data": [0, 4, 5, 0, 0] }))),
    );
    s.add_on.set_reply(
        FOLDING,
        Scripted::now(Reply::Result(
            json!([{ "startLine": 0, "endLine": 1, "kind": "region" }]),
        )),
    );
    let _editor = s.open_style();
    s.until("the provider of each asked", |s| {
        s.wait(Duration::from_millis(300));
        !AddOns::asked(&s.primary, TOKENS).is_empty()
            && !AddOns::asked(&s.add_on, FOLDING).is_empty()
    });
    s.wait(Duration::from_millis(600));
    assert!(AddOns::asked(&s.add_on, TOKENS).is_empty());
    assert!(AddOns::asked(&s.primary, FOLDING).is_empty());
}

#[gpui::test]
fn primary_that_cannot_start_keeps_path_owner_with_served_add_on(cx: &mut TestAppContext) {
    let mut s = AddOns::new(cx, Script::default(), Script::default(), true);
    let holder = s.open(STYLE);
    let uri = s.uri(STYLE);
    s.add_on
        .wait_for_document(&uri, "didOpen", |r| r.is_some_and(|r| r.open));
    let buffer = buffer_of(&mut s, &holder);
    let (served, lead, servers) = s.placed(buffer);
    assert!(served, "the add-on serves it");
    assert_eq!(lead, None, "the primary never started");
    assert_eq!(servers.len(), 1);
    // The configured lead comes first, by name and root, with no instance
    // now: the workbench's route keeps the one a failed launch had for a
    // moment, so the manager's is asked.
    let route = s
        .workbench
        .read_with(&s.cx, |w, _| w.lsp.route(buffer).to_vec());
    let names: Vec<&str> = route.iter().map(|server| server.name.as_str()).collect();
    assert_eq!(names, [PRIMARY, ADD_ON]);
    let placed = s.workbench.read_with(&s.cx, |w, _| {
        let manager = w.lsp.manager().expect("a manager");
        let manager = manager.lock().unwrap();
        let route = manager.route_servers(buffer);
        (route.iter())
            .map(|server| (server.name.clone(), server.placed.is_some()))
            .collect::<Vec<_>>()
    });
    assert_eq!(
        placed,
        [(PRIMARY.to_owned(), false), (ADD_ON.to_owned(), true)]
    );
    let lead = s.workbench.read_with(&s.cx, |w, _| {
        w.lsp.lead(buffer).map(|lead| lead.name.clone())
    });
    assert_eq!(lead.as_deref(), Some(PRIMARY));

    // An alias of the path waits: its edit goes nowhere.
    let alias = "src/../src/style.css";
    s.open(alias);
    s.cx.simulate_input("x");
    s.draw();
    assert!(s.owners_match_manager());
    assert_eq!(AddOns::sent(&s.add_on, 1, "textDocument/didOpen", &uri), 1);
    assert_eq!(
        AddOns::sent(&s.add_on, 1, "textDocument/didChange", &uri),
        0
    );

    // Closing the holder hands the path to the alias, on the add-on.
    s.open(STYLE);
    s.cx.simulate_keystrokes("cmd-w");
    s.draw();
    let replica = s
        .add_on
        .wait_for_document(&uri, "the alias's didOpen", |r| {
            r.is_some_and(|r| r.open && r.text == format!("x{TEXT}"))
        });
    assert!(replica.is_some());
    assert_eq!(AddOns::sent(&s.add_on, 1, "textDocument/didClose", &uri), 1);
    assert!(s.owners_match_manager());
}

#[gpui::test]
fn css_and_scss_aliases_with_different_add_on_sets_keep_owners_match_manager(
    cx: &mut TestAppContext,
) {
    let mut s = AddOns::new(cx, Script::default(), Script::default(), false);
    let theme = "src/theme.scss";
    let uri = s.uri(theme);
    // The SCSS buffer holds the path for the primary, the add-on not
    // serving SCSS.
    let scss = s.open(theme);
    s.primary
        .wait_for_document(&uri, "didOpen", |r| r.is_some_and(|r| r.open));
    // Its alias, set to CSS: the same lead, with the add-on too. It waits
    // on every server.
    s.open("src/../src/theme.scss");
    let css = s.active_editor();
    s.cx.update(|_, cx| css.update(cx, |view, cx| view.set_language(Some("css"), cx)));
    s.draw();
    s.cx.simulate_input("x");
    s.draw();
    assert!(s.owners_match_manager());
    let css_buffer = buffer_of(&mut s, &css);
    assert_eq!(s.placed(css_buffer), (false, None, Vec::new()));
    let scss_buffer = buffer_of(&mut s, &scss);
    assert!(s.placed(scss_buffer).0);
    assert_eq!(AddOns::sent(&s.primary, 1, "textDocument/didOpen", &uri), 1);
    assert!(AddOns::asked(&s.add_on, "initialize").is_empty());

    // Closing the SCSS holder hands the path to the CSS alias, on both.
    s.cx.update(|window, cx| {
        let pane = s.workbench.read(cx).pane().clone();
        pane.update(cx, |pane, cx| pane.activate_at(0, window, cx));
    });
    s.draw();
    assert_eq!(s.active_editor(), scss);
    s.cx.simulate_keystrokes("cmd-w");
    s.draw();
    for fake in [&s.primary, &s.add_on] {
        fake.wait_for_document(&uri, "the alias's didOpen", |r| {
            r.is_some_and(|r| r.open && r.text.starts_with('x'))
        });
    }
    let (served, lead, servers) = s.placed(css_buffer);
    assert!(served && lead.is_some());
    assert_eq!(servers.len(), 2);
    assert!(s.owners_match_manager());
}

#[gpui::test]
fn hint_changed_by_add_on_index_shows_at_next_poll(cx: &mut TestAppContext) {
    let provides = |capabilities: &mut ServerCapabilities| {
        capabilities.inlay_hint_provider = Some(OneOf::Left(true));
    };
    let mut s = AddOns::new(cx, script(provides), script(provides), false);
    s.primary.set_reply(HINT, hint(1, "p"));
    s.add_on.set_reply(HINT, hint(4, "old"));
    let editor = s.open_style();
    s.wait(Duration::from_millis(300));
    s.until("the hints", |s| hints(s, &editor).len() == 2);
    assert_eq!(hints(&s, &editor)[1], (4, "old".to_owned()));
    let asked = AddOns::asked(&s.add_on, HINT).len();

    // The add-on's index changes, with no edit and no notification: the
    // hint shown stays until its next poll.
    s.add_on.set_reply(HINT, hint(4, "new"));
    s.wait(Duration::from_millis(300));
    assert_eq!(hints(&s, &editor)[1], (4, "old".to_owned()));
    assert_eq!(AddOns::asked(&s.add_on, HINT).len(), asked);
    s.clock.advance(REPOLL);
    s.until("the polled hint", |s| {
        hints(s, &editor).get(1).map(|hint| hint.1.as_str()) == Some("new")
    });
    assert_eq!(
        hints(&s, &editor),
        [(1, "p".to_owned()), (4, "new".to_owned())]
    );
    assert_eq!(AddOns::asked(&s.add_on, HINT).len(), asked + 1);
}
