//! `files.associations` in a window: a settings reload that changes them
//! detects again each open editor whose language wasn't picked, in every
//! window, leaving one picked by hand; a launch file detects by them; a
//! project's patterns with a `/` match from the root, and apply to its
//! files only; an entry naming no language is a config problem, ignored,
//! unless its grammar is installed and still registering, which then
//! applies.

use std::{
    fs,
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
    time::Duration,
};

use fe_buffer::Buffer;
use fe_config::{Settings, SettingsEvent, SettingsStore};
use fe_gpui::{EditorSettings, EditorTheme, EditorView};
use gpui::{Entity, Task, TestAppContext, VisualTestContext, WindowHandle};
use kiln_platform::config::{self, ConfigService};

use super::open_root;
use crate::{GrammarOffers, Launch, Workbench, loader::Opened};

/// A folder of `a.zz`, `b.zz` and `sub/c.zz`, text no language claims.
fn project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("sub")).unwrap();
    for name in ["a.zz", "b.zz", "sub/c.zz"] {
        fs::write(dir.path().join(name), "key: value\n").unwrap();
    }
    dir
}

fn draw(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.run_until_parked();
}

/// Opens `path` in the active pane: its editor.
fn open(
    workbench: &Entity<Workbench>,
    path: &Path,
    cx: &mut VisualTestContext,
) -> Entity<EditorView> {
    let pane = workbench.read_with(cx, |workbench, _| workbench.pane().clone());
    let path = path.to_owned();
    cx.update(|window, cx| pane.update(cx, |pane, cx| pane.open(path, window, cx)));
    draw(cx);
    pane.read_with(cx, |pane, _| pane.active_editor().cloned())
        .expect("an editor")
}

/// The editor's language, `""` for plain text.
fn language(editor: &Entity<EditorView>, cx: &mut VisualTestContext) -> &'static str {
    editor.read_with(cx, |editor, _| {
        editor
            .syntax()
            .map_or("", |syntax| syntax.language().name())
    })
}

/// Settings whose user file associates `user` and whose project file
/// `project`.
fn settings(user: &[(&str, &str)], project: &[(&str, &str)]) -> Settings {
    let mut settings = Settings::default();
    let entries = |entries: &[(&str, &str)]| {
        (entries.iter())
            .map(|&(pattern, language)| [pattern.to_owned(), language.to_owned()])
            .collect()
    };
    for &(pattern, language) in user.iter().chain(project) {
        (settings.files.associations).insert(pattern.into(), language.into());
    }
    settings.files.user_entries = entries(user);
    settings.files.project_entries = entries(project);
    settings
}

/// This window's config problems now.
fn problems(
    workbench: &Entity<Workbench>,
    cx: &mut VisualTestContext,
) -> Vec<fe_config::Diagnostic> {
    workbench.update(cx, |workbench, cx| workbench.config_problems_now(cx))
}

/// A workbench on `root` in a window of its own.
fn window_on(root: &Path, cx: &mut TestAppContext) -> WindowHandle<Workbench> {
    let launch = Launch {
        root: Some(root.to_owned()),
        file: None,
    };
    cx.add_window(move |window, cx| {
        let theme = EditorTheme::default_dark(cx);
        Workbench::new(launch, theme, EditorSettings::default(), window, cx)
    })
}

fn init(cx: &mut TestAppContext) {
    cx.update(|cx| {
        fe_gpui::init(cx);
        crate::init(cx);
    });
}

/// Reloads `to`, from `from`.
fn reload(
    workbench: &Entity<Workbench>,
    from: &Settings,
    to: &Settings,
    cx: &mut VisualTestContext,
) {
    let event = SettingsEvent {
        settings: Arc::new(to.clone()),
        previous: Arc::new(from.clone()),
        diagnostics: Vec::new(),
    };
    workbench.update(cx, |workbench, cx| workbench.reload_settings(&event, cx));
    draw(cx);
}

#[gpui::test]
fn a_change_detects_an_unchosen_editor_again_and_leaves_a_chosen_one(cx: &mut TestAppContext) {
    let dir = project();
    let root = dir.path();
    let (workbench, cx) = open_root(cx, root);
    draw(cx);
    let a = open(&workbench, &root.join("a.zz"), cx);
    let b = open(&workbench, &root.join("b.zz"), cx);
    assert_eq!((language(&a, cx), language(&b, cx)), ("", ""));
    // Picked by hand: Plain Text.
    b.update(cx, |editor, cx| editor.set_language(None, cx));
    draw(cx);

    let none = Settings::default();
    let yaml = settings(&[("*.zz", "yaml")], &[]);
    reload(&workbench, &none, &yaml, cx);
    assert_eq!(language(&a, cx), "yaml");
    assert_eq!(language(&b, cx), "", "picked by hand");
    assert!(b.read_with(cx, |editor, _| editor.is_language_chosen()));

    // A file opened since detects by them too.
    let c = open(&workbench, &root.join("sub/c.zz"), cx);
    assert_eq!(language(&c, cx), "yaml");

    // Another language, then none: back to what the file says.
    let toml = settings(&[("*.zz", "toml")], &[]);
    reload(&workbench, &yaml, &toml, cx);
    assert_eq!(language(&a, cx), "toml");
    reload(&workbench, &toml, &none, cx);
    assert_eq!((language(&a, cx), language(&c, cx)), ("", ""));
    assert_eq!(language(&b, cx), "");
}

#[gpui::test]
fn a_projects_patterns_match_from_the_root_and_come_first(cx: &mut TestAppContext) {
    let dir = project();
    let root = dir.path();
    let (workbench, cx) = open_root(cx, root);
    draw(cx);
    let a = open(&workbench, &root.join("a.zz"), cx);
    let c = open(&workbench, &root.join("sub/c.zz"), cx);
    // The user's `sub/*.zz` is more specific, but the project's comes first.
    let both = settings(&[("sub/*.zz", "json")], &[("*.zz", "toml")]);
    reload(&workbench, &Settings::default(), &both, cx);
    assert_eq!((language(&a, cx), language(&c, cx)), ("toml", "toml"));
    let slash = settings(&[], &[("sub/*.zz", "yaml")]);
    reload(&workbench, &both, &slash, cx);
    assert_eq!((language(&a, cx), language(&c, cx)), ("", "yaml"));
}

#[gpui::test]
fn an_unknown_language_is_a_config_problem_and_ignored(cx: &mut TestAppContext) {
    let dir = project();
    let root = dir.path();
    let file = fe_config::project_settings_path(root);
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(
        &file,
        "[files.associations]\n\"*.zz\" = \"no-such-language\"\n\"b.{zz\" = \"yaml\"\n",
    )
    .unwrap();
    let (workbench, cx) = open_root(cx, root);
    draw(cx);
    let a = open(&workbench, &root.join("a.zz"), cx);
    let none = Settings::default();
    let bad = settings(&[], &[("*.zz", "no-such-language"), ("b.{zz", "yaml")]);
    reload(&workbench, &none, &bad, cx);
    assert_eq!(language(&a, cx), "", "the entry is ignored");

    let found = problems(&workbench, cx);
    assert_eq!(found.len(), 2, "{found:?}");
    let unknown = &found[0];
    assert_eq!(unknown.file, file);
    assert_eq!(unknown.severity, fe_config::Severity::Warning);
    assert_eq!(unknown.key.as_deref(), Some("files.associations.*.zz"));
    assert_eq!((unknown.line, unknown.column), (2, 10));
    assert!(unknown.message.contains("no-such-language"), "{unknown}");
    let glob = &found[1];
    assert_eq!(glob.line, 3);
    assert!(glob.message.contains("b.{zz"), "{glob}");

    reload(&workbench, &bad, &none, cx);
    assert_eq!(problems(&workbench, cx), []);
}

/// A file the launch opens, read already, detects by the settings the app
/// starts with.
#[gpui::test]
fn a_launch_file_detects_by_the_settings_it_starts_with(cx: &mut TestAppContext) {
    let dir = project();
    let root = dir.path();
    let user = root.join("user-settings.toml");
    fs::write(&user, "[files.associations]\n\"*.zz\" = \"yaml\"\n").unwrap();
    init(cx);
    let store = SettingsStore::load(Some(user), None);
    let (_sender, batches) = config::batch_channel();
    let _service = cx.update(|cx| {
        ConfigService::new()
            .settings(store.into_inner())
            .start_with(batches, cx)
    });
    let path = root.join("a.zz");
    let read = Opened::Text(Some(Buffer::new("key: value\n")));
    let launch = Launch {
        root: Some(root.to_owned()),
        file: Some((path.clone(), Some(read))),
    };
    let (workbench, cx) = cx.add_window_view(move |window, cx| {
        let theme = EditorTheme::default_dark(cx);
        Workbench::new(launch, theme, EditorSettings::default(), window, cx)
    });
    draw(cx);
    let pane = workbench.read_with(cx, |workbench, _| workbench.pane().clone());
    let editor = pane.read_with(cx, |pane, _| pane.active_editor().cloned());
    let editor = editor.expect("the launch file's editor");
    assert_eq!(
        editor.read_with(cx, |editor, _| editor.model().path().map(Path::to_owned)),
        Some(path)
    );
    assert_eq!(language(&editor, cx), "yaml");
}

/// A window's change to its root's associations detects again another
/// window's editor of a file under that root.
#[gpui::test]
fn a_change_detects_again_in_every_window(cx: &mut TestAppContext) {
    let (a, b) = (project(), project());
    init(cx);
    let first = window_on(a.path(), cx);
    let second = window_on(b.path(), cx);
    let cx = &mut VisualTestContext::from_window(second.into(), cx);
    draw(cx);
    let second = second.entity(cx).unwrap();
    let first = first.entity(cx).unwrap();
    // The second window has the first's file open.
    let editor = open(&second, &a.path().join("sub/c.zz"), cx);
    assert_eq!(language(&editor, cx), "");
    let toml = settings(&[], &[("sub/*.zz", "toml")]);
    reload(&first, &Settings::default(), &toml, cx);
    assert_eq!(language(&editor, cx), "toml", "under the first's root");
    // The second's own settings don't reach it.
    let yaml = settings(&[], &[("sub/*.zz", "yaml")]);
    reload(&second, &Settings::default(), &yaml, cx);
    assert_eq!(language(&editor, cx), "toml");
}

/// A root left behind keeps none of its associations: its files then go
/// by the user's patterns as they are now.
#[gpui::test]
fn a_root_left_keeps_no_stale_associations(cx: &mut TestAppContext) {
    let (a, b) = (project(), project());
    let (workbench, cx) = open_root(cx, a.path());
    draw(cx);
    let yaml = settings(&[("*.zz", "yaml")], &[("sub/*.zz", "json")]);
    reload(&workbench, &Settings::default(), &yaml, cx);
    let b_root = b.path().to_owned();
    cx.update(|window, cx| workbench.update(cx, |w, cx| w.set_root(b_root, window, cx)));
    draw(cx);
    let toml = settings(&[("*.zz", "toml")], &[]);
    reload(&workbench, &yaml, &toml, cx);
    let editor = open(&workbench, &a.path().join("sub/c.zz"), cx);
    assert_eq!(language(&editor, cx), "toml");
}

/// The user's pattern a project replaces still applies to files outside
/// the project.
#[gpui::test]
fn a_users_pattern_a_project_replaces_applies_outside_it(cx: &mut TestAppContext) {
    let (a, elsewhere) = (project(), project());
    let (workbench, cx) = open_root(cx, a.path());
    draw(cx);
    let both = settings(&[("*.zz", "yaml")], &[("*.zz", "toml")]);
    reload(&workbench, &Settings::default(), &both, cx);
    let inside = open(&workbench, &a.path().join("a.zz"), cx);
    let outside = open(&workbench, &elsewhere.path().join("a.zz"), cx);
    assert_eq!(
        (language(&inside, cx), language(&outside, cx)),
        ("toml", "yaml")
    );
}

/// The config problems list, open, follows an association corrected,
/// and goes once there are none.
#[gpui::test]
fn the_problems_list_follows_a_corrected_association(cx: &mut TestAppContext) {
    let dir = project();
    let (workbench, cx) = open_root(cx, dir.path());
    draw(cx);
    let two = settings(&[("*.zz", "no-such-language"), ("*.yy", "nor-this")], &[]);
    reload(&workbench, &Settings::default(), &two, cx);
    cx.update(|window, cx| workbench.update(cx, |w, cx| w.show_config_problems(window, cx)));
    draw(cx);
    let rows = |cx: &mut VisualTestContext| {
        let picker = workbench.read_with(cx, |w, _| w.config_problems().clone());
        picker.read_with(cx, |picker, _| picker.delegate.labels().len())
    };
    assert!(workbench.read_with(cx, |w, _| w.config_problems_open()));
    assert_eq!(rows(cx), 2);
    let one = settings(&[("*.zz", "no-such-language"), ("*.yy", "yaml")], &[]);
    reload(&workbench, &two, &one, cx);
    assert_eq!(rows(cx), 1);
    let none = settings(&[("*.zz", "yaml")], &[]);
    reload(&workbench, &one, &none, cx);
    assert!(!workbench.read_with(cx, |w, _| w.config_problems_open()));
}

/// The fixture grammar, as an installed extension's language whose
/// registration is still under way.
const LATER: &str = "assocpending";

fn later_spec() -> fe_syntax::GrammarSpec {
    fe_syntax::set_grammar_deadlines(fe_syntax::GrammarDeadlines {
        load: Duration::from_secs(120),
        parse: Duration::from_secs(120),
    });
    fe_syntax::GrammarSpec {
        language: LATER.into(),
        wasm: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../fe-syntax/tests/fixtures/tree-sitter-json.wasm"),
        ..fe_syntax::GrammarSpec::default()
    }
}

/// Offers that say [`LATER`] is installed.
fn later_installed(cx: &mut VisualTestContext) {
    let offers = GrammarOffers {
        catalog: Vec::new(),
        packs: Vec::new(),
        detect: None,
        installed: Rc::new(|language: &str| language == LATER),
        fetch: Rc::new(|_, _| Task::ready(Ok(Vec::new()))),
        install: Rc::new(|_, _, _| Task::ready(Ok(()))),
        never_file: None,
    };
    cx.update(|_, cx| crate::set_grammar_offers(cx, offers));
}

/// An association naming an installed grammar's language that hasn't
/// registered yet isn't a problem, and applies once it registers.
#[gpui::test]
fn a_language_still_registering_is_no_problem_and_applies_once_it_registers(
    cx: &mut TestAppContext,
) {
    let dir = project();
    let (workbench, cx) = open_root(cx, dir.path());
    later_installed(cx);
    draw(cx);
    let editor = open(&workbench, &dir.path().join("a.zz"), cx);
    let pending = settings(&[("*.zz", LATER)], &[]);
    reload(&workbench, &Settings::default(), &pending, cx);
    assert_eq!(language(&editor, cx), "");
    assert_eq!(problems(&workbench, cx), []);

    fe_syntax::register_language(&later_spec()).expect("the fixture registers");
    cx.executor().advance_clock(Duration::from_secs(1));
    draw(cx);
    let registered = language(&editor, cx);
    let after = problems(&workbench, cx);
    fe_syntax::unregister_language(LATER);
    assert_eq!(registered, LATER);
    assert_eq!(after, []);
}
