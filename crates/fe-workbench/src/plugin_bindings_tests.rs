// SPDX-License-Identifier: Apache-2.0

//! Key bindings a plugin declares in its manifest (T2106 C3).
//!
//! The host sets a loaded plugin's bindings as its own host layer, which
//! loses to every other layer whatever the contexts (see
//! `kiln::host_keys`). Before it does, it drops the bindings it must not
//! set at all: one naming a command the plugin's `init` never registered,
//! and one carrying the secondary modifier whose keys would take a
//! built-in shortcut away. Each drop is a warning in the plugin's log and
//! the rest of the plugin loads.
//!
//! These tests go through `kiln::init`, not `fe_gpui::init` alone, because
//! the keymap a plugin's layer joins is the one kiln resolves; a harness
//! that installs `fe_keymap::DEFAULT_MACOS` directly would be testing a
//! keymap the app never has.

use std::{
    fs,
    path::Path,
    time::{Duration, Instant},
};

use fe_command::capability::{Capability, Grants};
use fe_config::SettingsStore;
use fe_gpui::{EditorSettings, EditorTheme};
use gpui::{App, Entity, Focusable as _, TestAppContext, VisualTestContext};
use kiln::core::{
    ApiVersion,
    extension::{ExtensionApi, load_manifest},
};
use kiln::platform::config::batch_channel;
use kiln_plugin::{
    grants::GrantStore,
    manifest::{ExtensionOrigin, PluginManifest},
};

use super::{
    Found, GRANTS_FILE, PluginHost, PluginState, StopReason, lock, plugin_kind, plugin_states,
    set_plugin_dirs, set_plugins, unload_plugin,
};
use crate::{Launch, Workbench, plugin_log};

/// Registers `acme.tool.hello` (args schema `{"type":"object"}` and
/// `when = "editor && !read_only"`, logging its id then its args),
/// `acme.tool.spin` and `acme.tool.invoke`.
const COMMANDS: &str = include_str!("../../../kiln/crates/kiln-plugin/tests/fixtures/commands.wat");

/// The commands fixture's plugin id.
const TOOL: &str = "acme.tool";

/// A command of the app's own, bound to a function key.
///
/// A function key carries no modifiers, so a plugin may ask for it (the
/// manifest rule only refuses a bare typing key) and the host does not
/// refuse it for clashing (that refusal is for the secondary modifier).
/// It is therefore the one collision that survives to be settled by
/// precedence at runtime, on every platform - kiln's own `f10` is
/// `platform = "!macos"` and so would make the test pass for the wrong
/// reason there.
mod probe {
    kiln::commands!(
        probe,
        [
            /// Probe
            Probe,
            /// Guard
            Guard,
            /// Chord
            Chord,
            /// Deep
            Deep,
        ]
    );
}

/// The app's built-in binding both the app and a plugin want, settled by
/// precedence at runtime.
const SHARED: &str = "f9";

/// A built-in binding carrying the secondary modifier, which a plugin
/// asking for the same keys is refused outright rather than outranked.
///
/// `secondary` is cmd on macOS and ctrl elsewhere, and either satisfies
/// the manifest's first-keystroke rule, so this is the same test on every
/// platform. It is a command contributed by the test rather than one of
/// kiln's bundled keys, because `Config::default_keys` is off by default
/// and a harness without it has none of those.
const GUARDED: &str = "secondary-alt-7";

/// A built-in CHORD, so a plugin asking for the keystroke that starts it
/// is refused too: letting it have them would make the chord unreachable.
const CHORDED: &str = "secondary-alt-8 y";

/// Keys a built-in binds in NO context, which a plugin asks for in
/// `editor` - a DEEPER context.
///
/// Among ordinary layers the deeper binding wins, so ranking the two
/// together would hand these to the plugin. This is the only shape that
/// tells the real rule from a rank, and it has to be pressed rather than
/// looked up: `Keymap::lookup` and `Matcher::push` decide separately, and
/// a mutation to the matcher alone survives every other test here.
const DEEP: &str = "f8";

/// A plugin's test directories, and the keymap batch channel kept open.
struct Plugins {
    dir: tempfile::TempDir,
    #[allow(dead_code, reason = "kept so the config watcher stays open")]
    sender: kiln::platform::config::BatchSender,
}

impl Plugins {
    /// Plugin `acme.tool` asking for `capabilities`, with `bindings` put
    /// in its `[contributes.plugin]` table verbatim.
    fn install(&self, bindings: &str, capabilities: &[&str]) -> Found {
        let dir = self.dir.path().join("extensions").join(TOOL);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("plugin.wasm"), COMMANDS).unwrap();
        let capabilities: Vec<String> = capabilities
            .iter()
            .map(|capability| format!("{capability:?}"))
            .collect();
        fs::write(
            dir.join("extension.toml"),
            format!(
                "id = \"{TOOL}\"\nname = \"{TOOL}\"\nversion = \"1.0.0\"\nlicense = \"MIT\"\n\n\
                 [engine]\ntenmoku = \"1.1\"\n\n\
                 [contributes.plugin]\nwasm = \"plugin.wasm\"\ncapabilities = [{}]\n\n{bindings}",
                capabilities.join(", ")
            ),
        )
        .unwrap();
        found(&dir)
    }

    fn grant(&self, capabilities: &[&str]) {
        let grants: Grants = capabilities
            .iter()
            .map(|capability| Capability::parse(capability).unwrap())
            .collect();
        GrantStore::new(self.dir.path().join("state").join(GRANTS_FILE))
            .grant(TOOL, (1, 0, 0), &grants, &[], false)
            .unwrap();
    }
}

/// One `[[contributes.plugin.bindings]]` entry.
fn binding(keys: &str, command: &str, context: Option<&str>) -> String {
    let mut entry =
        format!("[[contributes.plugin.bindings]]\nkeys = \"{keys}\"\ncommand = \"{command}\"\n");
    if let Some(context) = context {
        entry.push_str(&format!("context = \"{context}\"\n"));
    }
    entry
}

/// The plugin in extension `dir`, as the host is handed it.
fn found(dir: &Path) -> Found {
    let api = ExtensionApi {
        app: fe_config::paths::app_id().short,
        version: ApiVersion { major: 1, minor: 1 },
        reserved: Vec::new(),
        kinds: vec![plugin_kind()],
        allowed_licenses: None,
        only_kinds: None,
    };
    let (manifest, diagnostics) = load_manifest(dir, &api);
    let manifest = manifest.unwrap_or_else(|| panic!("{diagnostics:#?}"));
    let plugin = PluginManifest::from_extension(&manifest, ExtensionOrigin::User)
        .unwrap()
        .unwrap();
    Found::new(&manifest, Ok(plugin))
}

/// The app set up through `kiln::init`, with the workbench's commands, one
/// command of its own on [`SHARED`], and the plugin host in a directory of
/// the test's with the preview on.
fn start(cx: &mut TestAppContext) -> Plugins {
    // Plugins run on threads of their own.
    cx.executor().allow_parking();
    let dir = tempfile::tempdir().unwrap();
    let settings = dir.path().join("settings.toml");
    fs::write(&settings, "[extensions]\nplugins_preview = true\n").unwrap();
    let (sender, batches) = batch_channel();
    let config = kiln::Config::new("dev.fastedit.test", "fastedit")
        .commands(|cx| {
            fe_gpui::init(cx);
            crate::init(cx);
            probe::register(cx);
            kiln::Contributions::add_commands(
                cx,
                [
                    kiln::CommandSpec::new::<probe::Probe>("Probe")
                        .keys(SHARED)
                        .context("editor"),
                    kiln::CommandSpec::new::<probe::Guard>("Guard").keys(GUARDED),
                    kiln::CommandSpec::new::<probe::Chord>("Chord").keys(CHORDED),
                    kiln::CommandSpec::new::<probe::Deep>("Deep").keys(DEEP),
                ],
            );
        })
        .settings_store(SettingsStore::load(Some(settings), None).into_inner())
        .theme_mode(kiln::ui::ThemeMode::Dark)
        .watch_with(batches);
    let (state_dir, cache_dir) = (dir.path().join("state"), dir.path().join("cache"));
    cx.update(|cx| {
        kiln::init(cx, config);
        set_plugin_dirs(cx, state_dir, cache_dir);
        cx.global_mut::<PluginHost>().limits.call_deadline = Duration::from_secs(30);
    });
    Plugins { dir, sender }
}

/// A workbench with no folder, focused, in a window of its own, active.
fn window(cx: &mut TestAppContext) -> (Entity<Workbench>, &mut VisualTestContext) {
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
    draw(vcx);
    (workbench, vcx)
}

fn draw(vcx: &mut VisualTestContext) {
    for _ in 0..2 {
        vcx.run_until_parked();
        vcx.update(|window, cx| window.draw(cx).clear(cx));
    }
}

/// Opens `path` in the active pane, focused, so the key context has
/// `editor` in it.
fn open(workbench: &Entity<Workbench>, path: &Path, vcx: &mut VisualTestContext) {
    let pane = workbench.read_with(vcx, |workbench, _| workbench.pane().clone());
    let owned = path.to_owned();
    vcx.update(|window, cx| pane.update(cx, |pane, cx| pane.open(owned, window, cx)));
    draw(vcx);
    vcx.update(|window, cx| {
        let focus = pane.read(cx).focus_target(cx);
        window.focus(&focus, cx);
    });
    draw(vcx);
}

/// Runs until `done`: plugins load and stop on their own threads.
fn until(cx: &mut TestAppContext, what: &str, done: impl Fn(&App) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        cx.run_until_parked();
        if cx.update(|cx| done(cx)) {
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

fn state(cx: &App) -> Option<PluginState> {
    plugin_states(cx)
        .into_iter()
        .find(|info| info.id == TOOL)
        .map(|info| info.state)
}

fn running(cx: &App) -> bool {
    state(cx) == Some(PluginState::Running)
}

/// What the plugin itself logged, through its `log` import.
fn logged(cx: &mut TestAppContext) -> Vec<String> {
    cx.update(|cx| {
        lock(&cx.global::<PluginHost>().record)
            .iter()
            .filter(|line| line.starts_with("log "))
            .cloned()
            .collect()
    })
}

/// The warnings the HOST put in the plugin's log, which is where a dropped
/// or refused binding is reported.
fn warnings(cx: &mut TestAppContext) -> Vec<String> {
    cx.update(|cx| {
        (plugin_log::lines(TOOL, cx).into_iter())
            .filter(|line| line.level == log::Level::Warn)
            .map(|line| line.message)
            .collect()
    })
}

/// What `keys` alone runs where focus is.
fn runs(keys: &str, vcx: &mut VisualTestContext) -> Option<String> {
    let keystroke = kiln::core::Keystroke::parse(keys).unwrap();
    vcx.update(|window, cx| {
        let stack = kiln::platform::keys::context_stack(&window.context_stack(), cx);
        kiln::platform::keys::lookup(&keystroke, &stack, cx)
            .map(|invocation| invocation.command.to_string())
    })
}

/// Loads the plugin with `bindings` and waits for it to run.
fn load<'a>(
    plugins: &Plugins,
    bindings: &str,
    cx: &'a mut TestAppContext,
) -> (Entity<Workbench>, &'a mut VisualTestContext) {
    plugins.grant(&["commands"]);
    let tool = plugins.install(bindings, &["commands"]);
    let (workbench, vcx) = window(cx);
    vcx.update(|_, cx| set_plugins(cx, vec![tool]));
    until(vcx, "the granted plugin to run", running);
    (workbench, vcx)
}

#[gpui::test]
fn a_bound_key_runs_the_plugins_command(cx: &mut TestAppContext) {
    let plugins = start(cx);
    let keys = binding("ctrl-alt-9", "acme.tool.hello", Some("editor"));
    let (workbench, vcx) = load(&plugins, &keys, cx);
    assert_eq!(warnings(vcx), Vec::<String>::new(), "nothing was refused");

    // `hello` only runs in an editor, so open one: the binding's own
    // context and the command's `when` are both checked.
    let file = plugins.dir.path().join("note.txt");
    fs::write(&file, "hello\n").unwrap();
    open(&workbench, &file, vcx);
    assert_eq!(
        runs("ctrl-alt-9", vcx).as_deref(),
        Some("acme.tool.hello"),
        "the plugin's keys are bound where its command applies"
    );

    // Pressing them really runs it: this goes through the matcher, not
    // only the keymap lookup.
    assert_eq!(logged(vcx), Vec::<String>::new());
    vcx.simulate_keystrokes("ctrl-alt-9");
    let deadline = Instant::now() + Duration::from_secs(30);
    while logged(vcx).len() < 2 {
        assert!(Instant::now() < deadline, "the bound key ran nothing");
        vcx.run_until_parked();
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(
        logged(vcx),
        [
            format!("log {TOOL} on plugin {TOOL}: acme.tool.hello"),
            format!("log {TOOL} on plugin {TOOL}: {{}}"),
        ],
        "its id, then the args a binding has none of"
    );
}

#[gpui::test]
fn a_binding_to_an_unregistered_command_is_dropped_with_a_log_warning(cx: &mut TestAppContext) {
    let plugins = start(cx);
    // `acme.tool.nope` is in the plugin's own namespace, so the manifest
    // accepts it; only `init` can say it registered no such command.
    let keys = binding("ctrl-alt-9", "acme.tool.nope", None);
    let (_workbench, vcx) = load(&plugins, &keys, cx);

    // The rest of the plugin still loaded.
    assert!(vcx.update(|_, cx| running(cx)), "the plugin still runs");

    let warned = warnings(vcx);
    assert_eq!(warned.len(), 1, "{warned:?}");
    assert!(
        warned[0].contains("ctrl-alt-9") && warned[0].contains("acme.tool.nope"),
        "the warning names the keys and the command: {:?}",
        warned[0]
    );
    assert_eq!(
        runs("ctrl-alt-9", vcx),
        None,
        "the dropped binding binds nothing"
    );
}

#[gpui::test]
fn a_secondary_binding_a_builtin_has_is_refused_with_a_log_warning(cx: &mut TestAppContext) {
    let plugins = start(cx);
    // Three ways a plugin's keys can take a built-in's away, all three
    // refused: the same keys as the app's `Guard`; keys that START the
    // app's `Guard`, which would leave it unreachable; and the keystroke
    // the app's `Chord` starts with, which would leave the chord
    // unreachable. Each names a command the plugin really does register,
    // so only the clash can be the reason.
    let keys = format!(
        "{}{}{}",
        binding(GUARDED, "acme.tool.hello", None),
        binding(&format!("{GUARDED} x"), "acme.tool.hello", None),
        binding("secondary-alt-8", "acme.tool.hello", None),
    );
    let (_workbench, vcx) = load(&plugins, &keys, cx);
    assert!(vcx.update(|_, cx| running(cx)), "the plugin still runs");

    let warned = warnings(vcx);
    assert_eq!(warned.len(), 3, "all three were refused: {warned:?}");
    let (seven, eight) = match cfg!(target_os = "macos") {
        true => ("cmd-alt-7", "cmd-alt-8"),
        false => ("ctrl-alt-7", "ctrl-alt-8"),
    };
    assert!(
        warned
            .iter()
            .any(|line| line.contains(&format!("{seven} x"))),
        "a warning names the chord that starts with the app's keys: {warned:?}"
    );
    assert!(
        warned.iter().any(|line| line.contains(eight)),
        "a warning names the keystroke the app's chord starts with: {warned:?}"
    );

    // The app still has all of them, and nothing of the plugin's is bound.
    assert_eq!(runs(GUARDED, vcx).as_deref(), Some("probe.guard"));
    assert_eq!(
        runs("secondary-alt-8", vcx),
        None,
        "the app's chord still only starts there, and the plugin took nothing"
    );
}

#[gpui::test]
fn a_builtin_binding_wins_over_a_plugin_binding(cx: &mut TestAppContext) {
    let plugins = start(cx);
    // A function key carries no secondary modifier, so this one is NOT
    // refused at load - it is set, and loses at runtime instead.
    let keys = format!(
        "{}{}",
        binding(SHARED, "acme.tool.hello", None),
        // The same keys a built-in has in no context, asked for in a
        // deeper one: a rank would give these to the plugin.
        binding(DEEP, "acme.tool.hello", Some("editor")),
    );
    let (workbench, vcx) = load(&plugins, &keys, cx);
    assert_eq!(
        warnings(vcx),
        Vec::<String>::new(),
        "a function key is not refused, only outranked"
    );

    // No editor is open yet, so the app's own binding - which is in
    // `editor` - does not apply, and the plugin's context-free binding is
    // the only one for these keys. This is what makes the rest of the test
    // mean anything: it fails outright if the host set no binding at all.
    assert_eq!(
        runs(SHARED, vcx).as_deref(),
        Some("acme.tool.hello"),
        "the plugin's binding was set, and runs where the app's does not apply"
    );

    let file = plugins.dir.path().join("note.txt");
    fs::write(&file, "hello\n").unwrap();
    open(&workbench, &file, vcx);

    // Now the app's binding applies. It wins although the plugin's is the
    // context-free one, which among ordinary layers would be the shallower
    // and so the loser: a plugin's layer is outranked whatever the depths.
    assert_eq!(runs(SHARED, vcx).as_deref(), Some("probe.probe"));

    // And the deeper plugin binding does not win either, in the editor
    // where its context holds and the built-in's absence of one does too.
    assert_eq!(
        runs(DEEP, vcx).as_deref(),
        Some("probe.deep"),
        "a plugin binding is outranked whatever the depths"
    );

    // Pressing them runs the app's commands, not the plugin's. This goes
    // through the matcher, which decides separately from the lookup above:
    // these two presses are what pin the matcher's half of the rule.
    vcx.simulate_keystrokes(SHARED);
    draw(vcx);
    vcx.run_until_parked();
    vcx.simulate_keystrokes(DEEP);
    draw(vcx);
    vcx.run_until_parked();
    assert_eq!(
        logged(vcx),
        Vec::<String>::new(),
        "the plugin's command did not run for either"
    );
}

#[gpui::test]
fn disabling_or_unloading_removes_the_bindings_and_loading_restores_them(cx: &mut TestAppContext) {
    let plugins = start(cx);
    let keys = binding("ctrl-alt-9", "acme.tool.hello", None);
    let (_workbench, vcx) = load(&plugins, &keys, cx);
    assert_eq!(runs("ctrl-alt-9", vcx).as_deref(), Some("acme.tool.hello"));

    // Unloaded, its keys go with it.
    vcx.update(|_, cx| unload_plugin(TOOL, cx));
    until(vcx, "the plugin to unload", |cx| {
        matches!(state(cx), Some(PluginState::Stopped(diagnostic))
            if diagnostic.reason == StopReason::Unloaded)
    });
    assert_eq!(
        runs("ctrl-alt-9", vcx),
        None,
        "an unloaded plugin keeps no keys"
    );

    // Loaded again, they are back, read from the manifest as it is now.
    let tool = plugins.install(&keys, &["commands"]);
    vcx.update(|_, cx| set_plugins(cx, vec![tool]));
    vcx.update(|_, cx| super::reevaluate(cx));
    until(vcx, "the plugin to run again", running);
    assert_eq!(
        runs("ctrl-alt-9", vcx).as_deref(),
        Some("acme.tool.hello"),
        "loading restores them"
    );
}

#[gpui::test]
fn an_unfocused_plugin_gets_no_key_event(cx: &mut TestAppContext) {
    let plugins = start(cx);
    let keys = binding("ctrl-alt-9", "acme.tool.hello", Some("editor"));
    let (workbench, vcx) = load(&plugins, &keys, cx);
    let file = plugins.dir.path().join("note.txt");
    fs::write(&file, "hello\n").unwrap();
    open(&workbench, &file, vcx);

    // The plugin has no panel and nothing of its is focused. Its bound
    // key runs its command - that is a command invocation, not a key
    // event - and it is told nothing about any other key.
    vcx.simulate_keystrokes("ctrl-alt-9");
    let deadline = Instant::now() + Duration::from_secs(30);
    while logged(vcx).len() < 2 {
        assert!(Instant::now() < deadline, "the bound key ran nothing");
        vcx.run_until_parked();
        std::thread::sleep(Duration::from_millis(5));
    }
    let after_binding = logged(vcx);

    // Ordinary typing, and a key nothing of the plugin's binds, reach it
    // not at all: its log is unchanged.
    vcx.simulate_keystrokes("a b c escape ctrl-alt-8");
    draw(vcx);
    vcx.run_until_parked();
    assert_eq!(
        logged(vcx),
        after_binding,
        "an unfocused plugin is told nothing about keys it did not bind"
    );
}
