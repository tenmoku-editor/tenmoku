//! A plugin's background jobs in the Runs list (`docs/decisions/dqsjfernkt-plugins-stage-3.md`
//! §3.5): a queued job waits and is cancelled before it starts, starting sets `started`, the
//! progress text a job reports is shown cut to its length, a job past its wall time shows why
//! it failed, and unloading, reloading, disabling and quitting cancel the jobs. The plugin is
//! the tests' own [`JOBS`].

use std::{
    fs,
    path::Path,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use fe_command::capability::Capability;
use fe_gpui::{EditorSettings, EditorTheme};
use gpui::{App, AppContext as _, Entity, Focusable as _, TestAppContext, VisualTestContext};
use kiln::core::{
    ApiVersion,
    extension::{ExtensionApi, load_manifest},
};
use kiln_l10n::tr;
use kiln_plugin::{
    grants::GrantStore,
    manifest::{ExtensionOrigin, PluginManifest},
    runner::{Call, Reply},
};

use kiln_plugin::{
    grants::{ProgramIdentity, resolve_program},
    jobs::{JobControl, Launch as JobLaunch},
    limits::BackgroundLimits,
    paths::{Rules, to_guest},
};

use super::{JobEnd, JobId, JobsHub, JobsLoad, Load, POLL, Shown, State};
use crate::{
    Launch, Workbench,
    plugin_host::{
        DELIVERED_PER_BATCH, Found, GRANTS_FILE, HostedPlugin, PluginHost, PluginState, lock,
        plugin_kind, plugin_states, reevaluate, set_plugin_dirs, set_plugins, set_preview,
        unload_plugin,
    },
    runs::{
        FailReason, MAX_TEXT_CHARS, Run, RunKind, RunOwner, RunStatus, RunSummary, Runs, Waiting,
    },
    runs_panel::Row,
};

/// A plugin with the `jobs` capability. Its `init` shows status item `i`
/// ([`INIT`]). Its `run-command(id, args)` starts a job named and titled `id`, with `args`,
/// kind `other`, cancellable, about no root. Its worker's `run-job`, for a job whose name starts
/// with `t`, first reports its args as its progress message, half done; one whose name starts
/// with `s` shows a status item of its name; one whose name starts with `b` reads buffer 1. Then
/// every job but one whose name starts with `r` waits for `job-context.cancelled`; one whose
/// name starts with `s` then shows status item `late`. It returns an empty result. Its
/// `job-finished` does nothing ([`FINISHED`]).
const JOBS: &str = r#"
(component $C
  (import "kiln:extension/types@1.1.0" (instance $types
    (type $range (record (field "start" u64) (field "end" u64)))
    (export "range" (type (eq $range)))
    (type $error (variant
      (case "unimplemented" string)
      (case "denied" string)
      (case "not-found" string)
      (case "invalid-argument" string)
      (case "conflict" string)
      (case "failed" string)))
    (export "error" (type (eq $error)))))
  (alias export $types "range" (type $range))
  (alias export $types "error" (type $error))

  (import "kiln:extension/events@1.1.0" (instance $events
    (alias outer $C $range (type $range))
    (type $buffer-opened (record
      (field "buffer" u64)
      (field "version" u64)
      (field "path" (option string))
      (field "language" (option string))))
    (export "buffer-opened" (type $buffer-opened' (eq $buffer-opened)))
    (type $buffer-changed (record
      (field "buffer" u64) (field "version" u64) (field "ranges" (list $range))))
    (export "buffer-changed" (type $buffer-changed' (eq $buffer-changed)))
    (type $buffer-saved (record
      (field "buffer" u64) (field "version" u64) (field "path" string)))
    (export "buffer-saved" (type $buffer-saved' (eq $buffer-saved)))
    (type $selection-changed (record
      (field "buffer" u64) (field "version" u64) (field "ranges" (list $range))))
    (export "selection-changed" (type $selection-changed' (eq $selection-changed)))
    (type $event (variant
      (case "buffer-opened" $buffer-opened')
      (case "buffer-changed" $buffer-changed')
      (case "buffer-saved" $buffer-saved')
      (case "buffer-closed" u64)
      (case "focus-changed" (option u64))
      (case "selection-changed" $selection-changed')
      (case "config-reloaded")
      (case "lagged" u64)))
    (export "event" (type (eq $event)))))
  (alias export $events "event" (type $event))

  (import "kiln:extension/jobs@1.1.0" (instance $jobs
    (alias outer $C $error (type $error))
    (type $job-kind (enum "build" "test" "index" "other"))
    (export "job-kind" (type $job-kind' (eq $job-kind)))
    (type $job-spec (record
      (field "name" string)
      (field "args" string)
      (field "title" string)
      (field "kind" $job-kind')
      (field "cancellable" bool)
      (field "root" (option string))))
    (export "job-spec" (type $job-spec' (eq $job-spec)))
    (type $job-state (variant
      (case "finished" (result string (error $error)))
      (case "cancelled")
      (case "timed-out")))
    (export "job-state" (type (eq $job-state)))
    (type $waiting (enum "concurrency" "memory"))
    (export "waiting" (type $waiting' (eq $waiting)))
    (type $progress (record
      (field "fraction" (option f32))
      (field "message" (option string))))
    (export "progress" (type $progress' (eq $progress)))
    (type $job-status (variant
      (case "queued" $waiting')
      (case "running" $progress')
      (case "succeeded")
      (case "failed")
      (case "cancelled")
      (case "timed-out")
      (case "unknown")))
    (export "job-status" (type $job-status' (eq $job-status)))
    (export "start" (func (param "spec" $job-spec') (result (result u64 (error $error)))))
    (export "state" (func (param "job" u64) (result $job-status')))))
  (alias export $jobs "job-state" (type $job-state))

  (import "kiln:extension/job-context@1.1.0" (instance $job-context
    (type $progress (record
      (field "fraction" (option f32))
      (field "message" (option string))))
    (export "progress" (type $progress' (eq $progress)))
    (export "report" (func (param "progress" $progress')))
    (export "cancelled" (func (result bool)))))

  (import "fastedit:editor/status@1.1.0" (instance $status
    (alias outer $C $error (type $error))
    (type $alignment (enum "left" "right"))
    (export "alignment" (type $alignment' (eq $alignment)))
    (type $item (record
      (field "id" string)
      (field "text" string)
      (field "tooltip" (option string))
      (field "command" (option string))
      (field "alignment" $alignment')))
    (export "item" (type $item' (eq $item)))
    (export "set-item" (func (param "item" $item') (result (result (error $error)))))))

  (import "fastedit:editor/buffer@1.1.0" (instance $buffer
    (alias outer $C $range (type $range))
    (alias outer $C $error (type $error))
    (type $snapshot (record
      (field "buffer" u64)
      (field "version" u64)
      (field "text" string)
      (field "path" (option string))
      (field "language" (option string))))
    (export "snapshot" (type $snapshot' (eq $snapshot)))
    (export "read" (func (param "buffer" u64) (param "range" (option $range))
      (result (result $snapshot' (error $error)))))))

  (core module $libc
    (memory (export "memory") 1)
    (global $heap (mut i32) (i32.const 1024))
    (func (export "realloc") (param i32 i32) (param $align i32) (param $size i32) (result i32)
      (local $p i32)
      (local $end i32)
      (local $have i32)
      (local.set $p
        (i32.and
          (i32.add (global.get $heap) (i32.sub (local.get $align) (i32.const 1)))
          (i32.sub (i32.const 0) (local.get $align))))
      (local.set $end (i32.add (local.get $p) (local.get $size)))
      (local.set $have (i32.mul (memory.size) (i32.const 65536)))
      (if (i32.gt_u (local.get $end) (local.get $have))
        (then
          (if (i32.eq
                (memory.grow
                  (i32.shr_u
                    (i32.add (i32.sub (local.get $end) (local.get $have)) (i32.const 65535))
                    (i32.const 16)))
                (i32.const -1))
            (then unreachable))))
      (global.set $heap (local.get $end))
      (local.get $p)))
  (core instance $libc (instantiate $libc))
  (alias core export $libc "memory" (core memory $memory))
  (alias core export $libc "realloc" (core func $realloc))

  (alias export $jobs "start" (func $jobs-start))
  (alias export $jobs "state" (func $jobs-state))
  (alias export $job-context "report" (func $context-report))
  (alias export $job-context "cancelled" (func $context-cancelled))
  (alias export $status "set-item" (func $status-set-item))
  (alias export $buffer "read" (func $buffer-read))
  (core func $start (canon lower (func $jobs-start) (memory $memory) (realloc $realloc)))
  (core func $state (canon lower (func $jobs-state) (memory $memory) (realloc $realloc)))
  (core func $report (canon lower (func $context-report) (memory $memory)))
  (core func $cancelled (canon lower (func $context-cancelled)))
  (core func $set-item (canon lower (func $status-set-item) (memory $memory) (realloc $realloc)))
  (core func $read (canon lower (func $buffer-read) (memory $memory) (realloc $realloc)))
  (core instance $host
    (export "start" (func $start))
    (export "state" (func $state))
    (export "report" (func $report))
    (export "cancelled" (func $cancelled))
    (export "set-item" (func $set-item))
    (export "read" (func $read)))

  (core module $main
    (import "libc" "memory" (memory 1))
    (import "host" "start" (func $start (param i32 i32 i32 i32 i32 i32 i32 i32 i32 i32 i32 i32)))
    (import "host" "report" (func $report (param i32 f32 i32 i32 i32)))
    (import "host" "cancelled" (func $cancelled (result i32)))
    (import "host" "state" (func $state (param i64 i32)))
    (import "host" "set-item"
      (func $set-item (param i32 i32 i32 i32 i32 i32 i32 i32 i32 i32 i32 i32)))
    (import "host" "read" (func $read (param i64 i32 i64 i64 i32)))

    ;; Bytes 0..8 stay zero: a `result` that is `ok`. 64..88 hold `start`'s result, 128..144
    ;; `run-command`'s or `init`'s error, 160..176 `run-job`'s result, 192..208 `set-item`'s,
    ;; 256..320 `read`'s and 320..344 `state`'s.
    (data (i32.const 512) "ilate")
    (data (i32.const 600) "wait-init")
    (data (i32.const 640) "r")

    ;; Shows the status item named by the `len` bytes at `at`.
    (func $show (param $at i32) (param $len i32)
      (call $set-item
        (local.get $at) (local.get $len) (local.get $at) (local.get $len)
        (i32.const 0) (i32.const 0) (i32.const 0)
        (i32.const 0) (i32.const 0) (i32.const 0)
        (i32.const 0) (i32.const 192)))

    ;; Starts a cancellable job named and titled by the `len` bytes at `at`, its result at 64.
    (func $start-job (param $at i32) (param $len i32)
      (call $start
        (local.get $at) (local.get $len)
        (i32.const 0) (i32.const 0)
        (local.get $at) (local.get $len)
        (i32.const 3) (i32.const 1)
        (i32.const 0) (i32.const 0) (i32.const 0)
        (i32.const 64)))

    ;; Job `job`'s state's case: 0 queued, 1 running, then succeeded, failed, cancelled,
    ;; timed-out and 6 unknown.
    (func $case (param $job i64) (result i32)
      (call $state (local.get $job) (i32.const 320))
      (i32.load8_u (i32.const 320)))

    (func (export "init") (result i32)
      (call $show (i32.const 512) (i32.const 1))
      (i32.const 0)) ;; init
    (func (export "shutdown"))
    (func (export "handle-events") (param i32 i32))

    (func (export "run-command") (param $id i32) (param $id-len i32) (param $args i32) (param $args-len i32) (result i32)
      (call $start
        (local.get $id) (local.get $id-len)
        (local.get $args) (local.get $args-len)
        (local.get $id) (local.get $id-len)
        (i32.const 3) (i32.const 1)
        (i32.const 0) (i32.const 0) (i32.const 0)
        (i32.const 64))
      (if (i32.eqz (i32.load8_u (i32.const 64))) (then (return (i32.const 0))))
      (i32.store8 (i32.const 128) (i32.const 1))
      (i32.store8 (i32.const 132) (i32.load8_u (i32.const 72)))
      (i32.store (i32.const 136) (i32.load (i32.const 76)))
      (i32.store (i32.const 140) (i32.load (i32.const 80)))
      (i32.const 128))

    (func (export "run-job")
      (param $job i64) (param $name i32) (param $name-len i32) (param $args i32) (param $args-len i32)
      (param $rooted i32) (param $root i32) (param $root-len i32)
      (result i32)
      (local $first i32)
      (local.set $first (i32.load8_u (local.get $name)))
      ;; talk
      (if (i32.eq (local.get $first) (i32.const 116))
        (then
          (call $report
            (i32.const 1) (f32.const 0.5)
            (i32.const 1) (local.get $args) (local.get $args-len))))
      ;; show
      (if (i32.eq (local.get $first) (i32.const 115))
        (then (call $show (local.get $name) (local.get $name-len))))
      ;; buffer
      (if (i32.eq (local.get $first) (i32.const 98))
        (then (call $read (i64.const 1) (i32.const 0) (i64.const 0) (i64.const 0) (i32.const 256))))
      ;; return at once
      (if (i32.ne (local.get $first) (i32.const 114))
        (then
          (block $cancelled
            (loop $waiting (br_if $cancelled (call $cancelled)) (br $waiting)))))
      (if (i32.eq (local.get $first) (i32.const 115))
        (then (call $show (i32.const 513) (i32.const 4))))
      (i32.store8 (i32.const 160) (i32.const 0))
      (i32.store (i32.const 164) (i32.const 0))
      (i32.store (i32.const 168) (i32.const 0))
      (i32.const 160))

    (func (export "job-finished")
      (param $job i64) (param $case i32) (param $ok i32) (param $a i32) (param $b i32) (param $c i32)
      ) ;; finished
    )

  (core instance $main (instantiate $main
    (with "libc" (instance $libc))
    (with "host" (instance $host))))

  (func (export "init") (result (result (error $error)))
    (canon lift (core func $main "init") (memory $memory)))
  (func (export "shutdown")
    (canon lift (core func $main "shutdown")))

  (func $run-command (param "id" string) (param "args" string) (result (result (error $error)))
    (canon lift (core func $main "run-command") (memory $memory) (realloc $realloc)))
  (instance $command-handler (export "run-command" (func $run-command)))
  (export "kiln:extension/command-handler@1.1.0" (instance $command-handler))

  (func $handle-events (param "events" (list $event))
    (canon lift (core func $main "handle-events") (memory $memory) (realloc $realloc)))
  (instance $event-handler (export "handle-events" (func $handle-events)))
  (export "kiln:extension/event-handler@1.1.0" (instance $event-handler))

  (func $run-job (param "job" u64) (param "name" string) (param "args" string) (param "root" (option string))
    (result (result string (error $error)))
    (canon lift (core func $main "run-job") (memory $memory) (realloc $realloc)))
  (instance $job-runner (export "run-job" (func $run-job)))
  (export "kiln:extension/job-runner@1.1.0" (instance $job-runner))

  (func $job-finished (param "job" u64) (param "state" $job-state)
    (canon lift (core func $main "job-finished") (memory $memory) (realloc $realloc)))
  (instance $job-handler (export "job-finished" (func $job-finished)))
  (export "kiln:extension/job-handler@1.1.0" (instance $job-handler)))
"#;

/// [`JOBS`]'s `init`'s end, which [`INIT_FAILS`] and [`INIT_WAITS`] replace.
const INIT: &str = "      (i32.const 0)) ;; init";

/// An `init` that starts job `wait-init`, then fails.
const INIT_FAILS: &str = "
      (call $start-job (i32.const 600) (i32.const 9))
      (i32.store8 (i32.const 128) (i32.const 1))
      (i32.store8 (i32.const 132) (i32.const 5))
      (i32.store (i32.const 136) (i32.const 0))
      (i32.store (i32.const 140) (i32.const 0))
      (i32.const 128))";

/// An `init` that starts job `wait-init` and, if it started, returns once the job is unknown.
const INIT_WAITS: &str = "
      (call $start-job (i32.const 600) (i32.const 9))
      (if (i32.eqz (i32.load8_u (i32.const 64)))
        (then
          (block $gone
            (loop $known
              (br_if $gone (i32.eq (call $case (i64.load (i32.const 72))) (i32.const 6)))
              (br $known)))))
      (i32.const 0))";

/// [`JOBS`]'s `job-finished`'s end, which [`FINISHED_STARTS`] replaces.
const FINISHED: &str = "      ) ;; finished";

/// A `job-finished` that starts job `r`, which returns at once, and returns once it has: a
/// job's end always waits as one is delivered.
const FINISHED_STARTS: &str = "
      (local $next i64)
      (call $start-job (i32.const 640) (i32.const 1))
      (if (i32.eqz (i32.load8_u (i32.const 64)))
        (then
          (local.set $next (i64.load (i32.const 72)))
          (block $ended
            (loop $running
              (br_if $ended (i32.gt_u (call $case (local.get $next)) (i32.const 1)))
              (br $running)))))
      )";

/// [`JOBS`]'s heap's start, which a rebuilt component moves.
const HEAP: &str = "(global $heap (mut i32) (i32.const 1024))";

/// [`JOBS`] with `from` replaced by `to`.
fn jobs_with(from: &str, to: &str) -> String {
    assert!(JOBS.contains(from), "{from}");
    JOBS.replace(from, to)
}

/// `wat` rebuilt: the same plugin, another component.
fn rebuilt(wat: &str) -> String {
    assert!(wat.contains(HEAP));
    wat.replace(HEAP, "(global $heap (mut i32) (i32.const 2048))")
}

/// The jobs plugin's id.
const RUNNER: &str = "acme.jobs";

/// A test's plugins: its state, cache and extensions directories.
struct Plugins {
    dir: tempfile::TempDir,
}

impl Plugins {
    /// The jobs plugin at `version`, its component `wat`, with `[contributes.plugin.background]`
    /// `background`, granted the `jobs`, `status` and `buffer.read` capabilities.
    fn install(&self, version: &str, wat: &str, background: &str) -> Found {
        let capabilities = ["jobs", "status", "buffer.read"];
        self.install_granted(version, wat, background, &capabilities, &[])
    }

    /// As [`Self::install`], granted `capabilities` and bound to `programs`.
    fn install_granted(
        &self,
        version: &str,
        wat: &str,
        background: &str,
        capabilities: &[&str],
        programs: &[ProgramIdentity],
    ) -> Found {
        let dir = self.dir.path().join("extensions").join(RUNNER);
        fs::create_dir_all(&dir).unwrap();
        // A plugin that runs jobs must be a binary component.
        fs::write(dir.join("plugin.wasm"), wat::parse_str(wat).unwrap()).unwrap();
        fs::write(
            dir.join("extension.toml"),
            format!(
                "id = \"{RUNNER}\"\nname = \"{RUNNER}\"\nversion = \"{version}\"\n\
                 license = \"MIT\"\n\n[engine]\ntenmoku = \"1.1\"\n\n\
                 [contributes.plugin]\nwasm = \"plugin.wasm\"\ncapabilities = {listed:?}\n\n\
                 [contributes.plugin.background]\n{background}\n",
                listed = capabilities,
            ),
        )
        .unwrap();
        let grants: fe_command::Grants = (capabilities.iter())
            .map(|capability| Capability::parse(capability).unwrap())
            .collect();
        let store = GrantStore::new(self.dir.path().join("state").join(GRANTS_FILE));
        let parsed: Vec<u32> = (version.split('.')).map(|n| n.parse().unwrap()).collect();
        let version = (parsed[0], parsed[1], parsed[2]);
        store
            .grant(RUNNER, version, &grants, programs, false)
            .unwrap();
        found(&dir)
    }
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

/// The app with the workbench's commands and the plugin host's preview on, keeping its files
/// in a directory of the test's, and a window to show the runs.
fn start(cx: &mut TestAppContext) -> (Plugins, Entity<Workbench>, &mut VisualTestContext) {
    // Plugins and their jobs run on threads of their own.
    cx.executor().allow_parking();
    let dir = tempfile::tempdir().unwrap();
    let (state, cache) = (dir.path().join("state"), dir.path().join("cache"));
    cx.update(|cx| {
        fe_gpui::init(cx);
        crate::init(cx);
        set_plugin_dirs(cx, state, cache);
        set_preview(cx, true);
        // A starved thread mustn't push `init` past the real deadline.
        cx.global_mut::<PluginHost>().limits.call_deadline = Duration::from_secs(30);
    });
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
    vcx.run_until_parked();
    (Plugins { dir }, workbench, vcx)
}

/// Runs until `done`, the Runs lists synced each [`POLL`]: plugins load and jobs run on their
/// own threads.
fn until(cx: &mut TestAppContext, what: &str, done: impl Fn(&mut TestAppContext) -> bool) {
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
        cx.executor().advance_clock(POLL);
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn running(cx: &mut TestAppContext) -> bool {
    cx.update(|cx| {
        (plugin_states(cx).into_iter())
            .any(|info| info.id == RUNNER && info.state == PluginState::Running)
    })
}

/// The jobs plugin loaded now.
fn plugin(cx: &mut TestAppContext) -> Arc<Mutex<HostedPlugin>> {
    cx.update(|cx| {
        let slot = &cx.global::<PluginHost>().slots[RUNNER];
        slot.loaded.as_ref().unwrap().plugin.clone()
    })
}

/// Has the plugin start a job named `name` with `args`.
fn start_job(name: &str, args: &str, cx: &mut TestAppContext) {
    let plugin = plugin(cx);
    let reply = lock(&plugin).call(Call::RunCommand {
        id: name.to_owned(),
        args: args.to_owned(),
    });
    assert!(
        matches!(reply, Ok(Reply::RunCommand(Ok(())))),
        "{name} didn't start: {reply:?}"
    );
}

/// Installs and loads the jobs plugin with `background`.
fn load(plugins: &Plugins, background: &str, cx: &mut TestAppContext) {
    let found = plugins.install("1.0.0", JOBS, background);
    cx.update(|cx| set_plugins(cx, vec![found]));
    until(cx, "the jobs plugin to run", running);
}

fn runs_of(workbench: &Entity<Workbench>, cx: &mut TestAppContext) -> Entity<Runs> {
    workbench.read_with(cx, |workbench, _| workbench.runs().clone())
}

/// The plugin's jobs' runs in `workbench`, oldest job first.
fn job_runs(workbench: &Entity<Workbench>, cx: &mut TestAppContext) -> Vec<Run> {
    let runs = runs_of(workbench, cx);
    let mut listed: Vec<Run> = runs.read_with(cx, |runs, _| {
        (runs.list())
            .filter(|run| matches!(&run.owner, RunOwner::Plugin { id, .. } if id == RUNNER))
            .cloned()
            .collect()
    });
    listed.sort_by_key(job_of);
    listed
}

fn job_of(run: &Run) -> JobId {
    match run.owner {
        RunOwner::Plugin { job, .. } => job,
        RunOwner::Host => panic!("not a job's run"),
    }
}

/// Runs until the plugin's jobs have `count` runs, then returns them.
fn until_runs(workbench: &Entity<Workbench>, count: usize, cx: &mut TestAppContext) -> Vec<Run> {
    until(cx, "the jobs' runs", |cx| {
        job_runs(workbench, cx).len() == count
    });
    job_runs(workbench, cx)
}

/// Runs until the run of `job` is `status`, then returns it.
fn until_status(
    workbench: &Entity<Workbench>,
    job: JobId,
    what: &str,
    status: impl Fn(&RunStatus) -> bool,
    cx: &mut TestAppContext,
) -> Run {
    let find = |cx: &mut TestAppContext| {
        (job_runs(workbench, cx).into_iter()).find(|run| job_of(run) == job)
    };
    until(cx, what, |cx| {
        find(cx).is_some_and(|run| status(&run.status))
    });
    find(cx).unwrap()
}

/// Cancels `run` as the Runs list's Cancel does.
fn cancel(workbench: &Entity<Workbench>, run: &Run, cx: &mut TestAppContext) -> bool {
    let (runs, id) = (runs_of(workbench, cx), run.id);
    runs.update(cx, |runs, cx| runs.cancel(id, cx))
}

fn worker_ends(cx: &mut TestAppContext) -> Vec<(JobId, JobEnd)> {
    cx.update(|cx| cx.global::<PluginHost>().jobs.worker_ends())
}

fn worker_ended(job: JobId, end: JobEnd) -> impl Fn(&mut TestAppContext) -> bool {
    move |cx| worker_ends(cx).contains(&(job, end.clone()))
}

const QUEUED: fn(&RunStatus) -> bool = RunStatus::is_queued;
const RUNNING: fn(&RunStatus) -> bool = RunStatus::is_running;

fn cancelled(status: &RunStatus) -> bool {
    *status == RunStatus::Cancelled
}

/// Starts three waiting jobs: two run, as many as a plugin may, and the third is queued.
fn three_jobs(workbench: &Entity<Workbench>, cx: &mut TestAppContext) -> [Run; 3] {
    for name in ["wait-1", "wait-2", "wait-3"] {
        start_job(name, "", cx);
    }
    let runs = until_runs(workbench, 3, cx);
    let [first, second, third] = [0, 1, 2].map(|ix| job_of(&runs[ix]));
    let first = until_status(workbench, first, "the first to run", RUNNING, cx);
    let second = until_status(workbench, second, "the second to run", RUNNING, cx);
    let third = until_status(workbench, third, "the third to wait", QUEUED, cx);
    [first, second, third]
}

#[gpui::test]
fn queued_job_shows_waiting_and_cancels_without_starting(cx: &mut TestAppContext) {
    let (plugins, workbench, vcx) = start(cx);
    // Its window shows the runs: the rest is the app's.
    let cx: &mut TestAppContext = vcx;
    load(&plugins, "", cx);
    let [first, _, queued] = three_jobs(&workbench, cx);

    // It waits for another job, queued since it was started and not started.
    assert_eq!(
        queued.status,
        RunStatus::Queued {
            waiting: Waiting::Concurrency
        }
    );
    assert_eq!(queued.title, "wait-3");
    assert!(queued.queued_at.is_some());
    assert_eq!(queued.started, None);
    assert!(queued.cancellable);
    let row = Row::of(&queued);
    assert_eq!(row.summary, tr!("workbench-runs-waiting-concurrency"));
    assert!(row.cancellable, "a queued job shows its Cancel");
    assert!(first.started.is_some());

    // Cancelled from the Runs list, it ends without having started.
    assert!(cancel(&workbench, &queued, cx));
    let job = job_of(&queued);
    let ended = until_status(&workbench, job, "the queued job to end", cancelled, cx);
    assert_eq!(ended.started, None);
    assert!(ended.finished.is_some());
    assert!(!Row::of(&ended).cancellable, "an ended run has no Cancel");
    assert!(
        !worker_ends(cx).iter().any(|(ended, _)| *ended == job),
        "the cancelled job never ran"
    );
    // Its slot isn't taken: the two others still run, and nothing waits.
    let counts = cx.update(|cx| cx.global::<PluginHost>().jobs.counts());
    assert_eq!((counts.queued, counts.running), (0, 2));
    // Ended, it can't be cancelled again.
    assert!(!cancel(&workbench, &ended, cx));
    cx.update(|cx| unload_plugin(RUNNER, cx));
}

#[gpui::test]
fn queued_to_running_sets_started(cx: &mut TestAppContext) {
    let (plugins, workbench, vcx) = start(cx);
    // Its window shows the runs: the rest is the app's.
    let cx: &mut TestAppContext = vcx;
    load(&plugins, "", cx);
    let [first, _, queued] = three_jobs(&workbench, cx);
    assert_eq!(queued.started, None);

    // The first ends: the queued one starts in its place.
    assert!(cancel(&workbench, &first, cx));
    until_status(
        &workbench,
        job_of(&first),
        "the first to end",
        cancelled,
        cx,
    );
    let started = until_status(&workbench, job_of(&queued), "the third to run", RUNNING, cx);
    let (queued_at, started_at) = (started.queued_at.unwrap(), started.started.unwrap());
    assert!(started_at >= queued_at, "started after it was queued");
    assert_eq!(started.status, RunStatus::Running { progress: None });
    assert!(Row::of(&started).cancellable);
    cx.update(|cx| unload_plugin(RUNNER, cx));
}

#[gpui::test]
fn progress_text_is_shown(cx: &mut TestAppContext) {
    let (plugins, workbench, vcx) = start(cx);
    // Its window shows the runs: the rest is the app's.
    let cx: &mut TestAppContext = vcx;
    load(&plugins, "", cx);
    // Longer than a run's text may be, in characters of two bytes each.
    let long: String = "é".repeat(3 * MAX_TEXT_CHARS);
    start_job("talk", &long, cx);
    let job = job_of(&until_runs(&workbench, 1, cx)[0]);
    let said = |status: &RunStatus| {
        *status
            == RunStatus::Running {
                progress: Some(0.5),
            }
    };
    let run = until_status(&workbench, job, "its progress", said, cx);
    // The host cut it before it reached Runs, at a character's boundary.
    let cut: String = "é".repeat(MAX_TEXT_CHARS);
    assert_eq!(run.summary, RunSummary::Text(cut.clone()));
    assert_eq!(Row::of(&run).summary, cut);
    cx.update(|cx| unload_plugin(RUNNER, cx));
}

#[gpui::test]
fn timed_out_job_shows_reason(cx: &mut TestAppContext) {
    let (plugins, workbench, vcx) = start(cx);
    // Its window shows the runs: the rest is the app's.
    let cx: &mut TestAppContext = vcx;
    // Its jobs may run a second.
    load(&plugins, "wall = \"1s\"", cx);
    start_job("wait", "", cx);
    let job = job_of(&until_runs(&workbench, 1, cx)[0]);
    let failed = |status: &RunStatus| *status == RunStatus::Failed { code: None };
    let run = until_status(&workbench, job, "it to time out", failed, cx);
    assert_eq!(run.reason, Some(FailReason::TimedOut));
    let row = Row::of(&run);
    assert_eq!(row.summary, tr!("workbench-runs-summary-timed-out"));
    let now = Instant::now();
    assert!(
        row.name(now)
            .contains(&*tr!("workbench-runs-status-timed-out")),
        "{}",
        row.name(now)
    );
    assert!(worker_ends(cx).contains(&(job, JobEnd::TimedOut)));
    // The plugin stays loaded.
    assert!(running(cx));
}

#[gpui::test]
fn jobs_cancelled_on_unload_reload_disable_and_quit(cx: &mut TestAppContext) {
    let (plugins, workbench, vcx) = start(cx);
    // Its window shows the runs: the rest is the app's.
    let cx: &mut TestAppContext = vcx;
    load(&plugins, "", cx);

    // Unloading: the running jobs end once their workers have, the queued one at once,
    // without having started.
    let [first, second, queued] = three_jobs(&workbench, cx);
    cx.update(|cx| unload_plugin(RUNNER, cx));
    for run in [&first, &second, &queued] {
        let job = job_of(run);
        let ended = until_status(&workbench, job, "the unloaded job to end", cancelled, cx);
        assert_eq!(ended.started.is_some(), run.started.is_some());
    }
    for run in [&first, &second] {
        assert!(worker_ends(cx).contains(&(job_of(run), JobEnd::Cancelled)));
    }

    // Reloading: a rebuilt component replaces the version running.
    cx.update(reevaluate);
    until(cx, "the plugin to run again", running);
    start_job("wait", "", cx);
    let runs = until_runs(&workbench, 4, cx);
    let job = job_of(&runs[3]);
    until_status(&workbench, job, "the job to run", RUNNING, cx);
    let found = plugins.install("1.0.0", &rebuilt(JOBS), "");
    cx.update(|cx| set_plugins(cx, vec![found]));
    until_status(&workbench, job, "the reloaded job to end", cancelled, cx);
    until(
        cx,
        "the job's worker to end",
        worker_ended(job, JobEnd::Cancelled),
    );
    until(cx, "the rebuilt plugin to run", running);

    // Disabling: the preview switched off.
    start_job("wait", "", cx);
    let runs = until_runs(&workbench, 5, cx);
    let job = job_of(&runs[4]);
    until_status(&workbench, job, "the job to run", RUNNING, cx);
    cx.update(|cx| set_preview(cx, false));
    until_status(&workbench, job, "the disabled job to end", cancelled, cx);
    until(
        cx,
        "the job's worker to end",
        worker_ended(job, JobEnd::Cancelled),
    );

    // Quitting.
    cx.update(|cx| set_preview(cx, true));
    until(cx, "the plugin to run once more", running);
    start_job("wait", "", cx);
    let runs = until_runs(&workbench, 6, cx);
    let job = job_of(&runs[5]);
    until_status(&workbench, job, "the job to run", RUNNING, cx);
    cx.update(|cx: &mut App| cx.shutdown());
    until(
        cx,
        "the job's worker to end",
        worker_ended(job, JobEnd::Cancelled),
    );
}

/// The texts of the plugins' items on the status bar of the app's window, sorted.
fn status_texts(cx: &mut TestAppContext) -> Vec<String> {
    let window = cx.update(|cx| cx.windows()[0]);
    for _ in 0..2 {
        cx.run_until_parked();
        cx.update_window(window, |_, window, cx| window.draw(cx).clear(cx))
            .unwrap();
    }
    let window = window.downcast::<Workbench>().unwrap();
    let mut texts: Vec<String> = window
        .update(cx, |workbench, _, cx| {
            let items = workbench.status_bar().read(cx).items(cx);
            (items.into_iter())
                .filter(|item| item.id.starts_with("plugin:"))
                .map(|item| item.text.to_string())
                .collect()
        })
        .unwrap();
    texts.sort();
    texts
}

/// The jobs on the host's board, oldest first, read without running the main thread.
fn board_jobs(cx: &mut TestAppContext) -> Vec<JobId> {
    cx.update(|cx| {
        (lock(&cx.global::<PluginHost>().jobs.0.board)
            .keys()
            .copied())
        .collect()
    })
}

/// Waits, without running the main thread, until `job`'s worker has ended cancelled.
fn ended_unserved(job: JobId, cx: &mut TestAppContext) {
    let deadline = Instant::now() + Duration::from_secs(1);
    while !worker_ends(cx).contains(&(job, JobEnd::Cancelled)) {
        assert!(
            Instant::now() < deadline,
            "job {job} still waits on the main thread"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[gpui::test]
fn workers_publish_as_their_load_does(cx: &mut TestAppContext) {
    let (plugins, workbench, vcx) = start(cx);
    let cx: &mut TestAppContext = vcx;
    load(&plugins, "", cx);
    until(cx, "init's item", |cx| status_texts(cx) == ["i"]);

    // The interactive instance and two workers publish as one load: none clears another's.
    start_job("show-1", "", cx);
    start_job("show-2", "", cx);
    until(cx, "the workers' items", |cx| {
        status_texts(cx) == ["i", "show-1", "show-2"]
    });

    // Unloaded, the load's items go, and its workers, still running then, show nothing more.
    let runs = until_runs(&workbench, 2, cx);
    cx.update(|cx| unload_plugin(RUNNER, cx));
    for run in &runs {
        let ended = worker_ended(job_of(run), JobEnd::Cancelled);
        until(cx, "the worker to end", ended);
    }
    assert_eq!(status_texts(cx), Vec::<String>::new());
}

#[gpui::test]
fn a_stopped_job_stops_waiting_on_the_main_thread(cx: &mut TestAppContext) {
    let (plugins, _workbench, vcx) = start(cx);
    let cx: &mut TestAppContext = vcx;
    load(&plugins, "", cx);
    // Their reads wait on the main thread, which runs nothing from here until both ended.
    start_job("buffer-1", "", cx);
    start_job("buffer-2", "", cx);
    let jobs = board_jobs(cx);
    assert_eq!(jobs.len(), 2);
    std::thread::sleep(Duration::from_millis(200));

    // Cancelled, one stops waiting at once, well within its grace.
    assert!(cx.update(|cx| super::cancel(RUNNER, jobs[0], cx)));
    ended_unserved(jobs[0], cx);
    // Unloading stops the other.
    cx.update(|cx| unload_plugin(RUNNER, cx));
    ended_unserved(jobs[1], cx);
    // What they handed the main thread and it hadn't started never runs.
    cx.run_until_parked();
}

#[gpui::test]
fn a_job_started_by_a_failing_init_is_shown_and_ends(cx: &mut TestAppContext) {
    let (plugins, workbench, vcx) = start(cx);
    let cx: &mut TestAppContext = vcx;
    let found = plugins.install("1.0.0", &jobs_with(INIT, INIT_FAILS), "");
    cx.update(|cx| set_plugins(cx, vec![found]));

    // The first plugin to run jobs: its job is shown though its load failed.
    let run = until_runs(&workbench, 1, cx).remove(0);
    assert_eq!(run.title, "wait-init");
    until_status(&workbench, job_of(&run), "its job to end", cancelled, cx);
    assert!(!running(cx));
    // And nothing is left to watch.
    until(cx, "nothing left of the load", |cx| {
        !cx.update(|cx| cx.global::<PluginHost>().jobs.busy())
    });
}

#[gpui::test]
fn quitting_cancels_jobs_of_loads_in_init_and_still_to_attach(cx: &mut TestAppContext) {
    let (plugins, workbench, vcx) = start(cx);
    let cx: &mut TestAppContext = vcx;
    let (gate, gated) = async_channel::unbounded();
    cx.update(|cx| cx.global_mut::<PluginHost>().load_gate = Some(gated));
    let waits = jobs_with(INIT, INIT_WAITS);
    let found = plugins.install("1.0.0", &waits, "");
    cx.update(|cx| set_plugins(cx, vec![found]));
    // Its `init` waits on the job it started, which runs. The test's executor runs the load,
    // so it is held until `init` returns: the quit's first step, which closes the queue and
    // cancels every load's jobs, comes from another thread, once the job's worker runs.
    let hub = cx.update(|cx| cx.global::<PluginHost>().jobs.clone());
    let quit = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(30);
        while Instant::now() < deadline {
            let launched = (lock(&hub.0.board).values()).any(|entry| entry.launched.is_some());
            if launched {
                hub.close();
                return true;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        false
    });
    gate.try_send(()).unwrap();
    // Its job gone, its `init` returns.
    until(cx, "the plugin to run", running);
    assert!(quit.join().unwrap(), "init's job never ran");
    let run = until_runs(&workbench, 1, cx).remove(0);
    assert_eq!(run.title, "wait-init");
    let job = job_of(&run);
    until_status(&workbench, job, "init's job to end", cancelled, cx);
    until(
        cx,
        "init's job's worker to end",
        worker_ended(job, JobEnd::Cancelled),
    );
    cx.update(|cx: &mut App| cx.shutdown());

    // A load that attaches after the quit starts no job.
    let old = plugin(cx);
    let found = plugins.install("1.0.0", &rebuilt(&waits), "");
    cx.update(|cx| set_plugins(cx, vec![found]));
    gate.try_send(()).unwrap();
    until(cx, "the rebuilt plugin to run", |cx| {
        running(cx) && !Arc::ptr_eq(&plugin(cx), &old)
    });
    let reply = lock(&plugin(cx)).call(Call::RunCommand {
        id: "wait".to_owned(),
        args: String::new(),
    });
    assert!(
        matches!(reply, Ok(Reply::RunCommand(Err(_)))),
        "a job started after the quit: {reply:?}"
    );
    let counts = cx.update(|cx| cx.global::<PluginHost>().jobs.counts());
    assert_eq!((counts.queued, counts.running), (0, 0));
}

#[gpui::test]
fn a_chain_of_job_ends_leaves_room_for_calls(cx: &mut TestAppContext) {
    let (plugins, _workbench, vcx) = start(cx);
    let cx: &mut TestAppContext = vcx;
    let found = plugins.install("1.0.0", &jobs_with(FINISHED, FINISHED_STARTS), "");
    cx.update(|cx| set_plugins(cx, vec![found]));
    until(cx, "the plugin to run", running);

    // Each job's `job-finished` starts the next, and returns once it has ended.
    start_job("r-0", "", cx);
    until(cx, "the chain to run a while", |cx| {
        worker_ends(cx).len() > 2 * DELIVERED_PER_BATCH
    });
    // A call still gets in, between two batches.
    let asked = Instant::now();
    start_job("wait", "", cx);
    assert!(
        asked.elapsed() < Duration::from_secs(5),
        "{:?}",
        asked.elapsed()
    );
    cx.update(|cx| unload_plugin(RUNNER, cx));
}

/// A load of plugin `p` on `hub` whose queue launches nothing itself.
fn bare_load(hub: &JobsHub) -> JobsLoad {
    let grants: fe_command::Grants = [Capability::parse("jobs").unwrap()].into_iter().collect();
    let launcher = Arc::new(|launch: JobLaunch| drop(launch));
    let jobs = (hub.0.queue).attach("p", &grants, BackgroundLimits::default(), launcher);
    let handle = Load {
        plugin: "p".to_owned(),
        jobs,
        detached: Arc::default(),
    };
    lock(&hub.0.live).loads.insert(1, handle.clone());
    JobsLoad {
        hub: hub.clone(),
        load: 1,
        handle,
    }
}

fn shown() -> Shown {
    Shown {
        title: "t".to_owned(),
        kind: RunKind::Script,
        cancellable: true,
        root: None,
    }
}

/// What the Runs lists would show of `hub`'s jobs now.
fn states(hub: &JobsHub) -> Vec<State> {
    (hub.take_views().into_iter())
        .map(|view| view.state)
        .collect()
}

#[test]
fn a_detach_before_start_notes_its_job_waits_for_the_worker_launched() {
    let hub = JobsHub::default();
    let load = bare_load(&hub);
    // The queue launched the job before its `start` noted it, and the load was detached.
    let control = JobControl::default();
    assert!((hub.0).launched("p", load.load, &load.handle.detached, 7, &control));
    load.detach();
    load.noted(7, shown());
    assert!(
        matches!(&states(&hub)[..], [State::Running(_)]),
        "shown running while its worker runs"
    );
    // Its worker ends: so does its run.
    hub.0.worker_ended(7, JobEnd::Cancelled);
    assert!(matches!(
        &states(&hub)[..],
        [State::Ended(JobEnd::Cancelled)]
    ));
    assert!(!hub.busy());
}

#[test]
fn a_launch_after_its_load_detached_and_its_run_ended_never_starts() {
    let hub = JobsHub::default();
    let load = bare_load(&hub);
    load.noted(8, shown());
    load.detach();
    assert!(matches!(
        &states(&hub)[..],
        [State::Ended(JobEnd::Cancelled)]
    ));
    // The queue launches it late: no worker starts, and nothing is left to show.
    let control = JobControl::default();
    assert!(!(hub.0).launched("p", load.load, &load.handle.detached, 8, &control));
    assert!(lock(&hub.0.board).is_empty());
    assert!(!hub.busy());
    assert!(hub.idle());
}

#[test]
fn a_poll_before_start_notes_its_job_keeps_its_launch() {
    let hub = JobsHub::default();
    let load = bare_load(&hub);
    // Launched, detached and ended before its `start` noted it, with a poll in between.
    let control = JobControl::default();
    assert!((hub.0).launched("p", load.load, &load.handle.detached, 9, &control));
    load.detach();
    hub.0.worker_ended(9, JobEnd::Cancelled);
    assert!(
        states(&hub).is_empty(),
        "nothing to show before it is noted"
    );
    // Noted then, its run ends cancelled, and started: its worker ran.
    load.noted(9, shown());
    let views = hub.take_views();
    assert!(
        matches!(
            &views[..],
            [super::View {
                launched: true,
                state: State::Ended(JobEnd::Cancelled),
                ..
            }]
        ),
        "{:?}",
        views
            .iter()
            .map(|view| (view.launched, &view.state))
            .collect::<Vec<_>>()
    );
    assert!(!hub.busy());
}

/// kiln-plugin's stream fixture: jobs that start programs and write to and read from them.
const STREAM: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../kiln/crates/kiln-plugin/tests/fixtures/stream.wat"
));

/// The programs [`STREAM`]'s `huge` and `in` jobs start.
const STREAM_PROGRAMS: [&str; 2] = ["idle", "show"];

/// The test binary's allocator, noting the largest allocation while [`LARGEST_ARMED`] is set:
/// the host's own memory, apart from a guest's, which wasmtime maps itself.
struct Largest;

static LARGEST: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static LARGEST_ARMED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

impl Largest {
    fn note(size: usize) {
        if LARGEST_ARMED.load(std::sync::atomic::Ordering::Relaxed) {
            LARGEST.fetch_max(size, std::sync::atomic::Ordering::Relaxed);
        }
    }
}

// SAFETY: each call is the system allocator's, with the same arguments.
unsafe impl std::alloc::GlobalAlloc for Largest {
    unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
        Self::note(layout.size());
        // SAFETY: as the caller's.
        unsafe { std::alloc::System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: std::alloc::Layout) -> *mut u8 {
        Self::note(layout.size());
        // SAFETY: as the caller's.
        unsafe { std::alloc::System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: std::alloc::Layout, size: usize) -> *mut u8 {
        Self::note(size);
        // SAFETY: as the caller's.
        unsafe { std::alloc::System.realloc(ptr, layout, size) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: std::alloc::Layout) {
        // SAFETY: as the caller's.
        unsafe { std::alloc::System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: Largest = Largest;

/// Writes program `name` into `dir`: `sh` on Unix, `cmd` on Windows.
fn script(dir: &Path, name: &str, sh: &str, cmd: &str) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let _ = cmd;
        let path = dir.join(name);
        fs::write(&path, format!("#!/bin/sh\n{sh}\n")).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    #[cfg(windows)]
    {
        let _ = sh;
        let body: String = cmd.lines().map(|line| format!("{line}\r\n")).collect();
        fs::write(
            dir.join(format!("{name}.cmd")),
            format!("@echo off\r\n{body}"),
        )
        .unwrap();
    }
}

/// [`STREAM`] installed and loaded, its programs granted and bound to scripts of the test's,
/// with `fs.read:workspace`; and a folder of the test's open in the app, the jobs' root.
fn load_stream(plugins: &Plugins, cx: &mut TestAppContext) -> std::path::PathBuf {
    let programs = plugins.dir.path().join("programs");
    let root = plugins.dir.path().join("root");
    fs::create_dir_all(&programs).unwrap();
    fs::create_dir_all(&root).unwrap();
    script(
        &programs,
        "idle",
        "exec sleep 600",
        "ping -n 601 127.0.0.1 >nul",
    );
    script(&programs, "show", "pwd -P", "cd");
    let mut capabilities = vec!["jobs".to_owned(), "fs.read:workspace".to_owned()];
    capabilities.extend(STREAM_PROGRAMS.map(|program| format!("process.spawn:{program}")));
    let capabilities: Vec<&str> = capabilities.iter().map(String::as_str).collect();
    let bound =
        STREAM_PROGRAMS.map(|program| resolve_program(program, programs.as_os_str()).unwrap());
    let found = plugins.install_granted("1.0.0", STREAM, "", &capabilities, &bound);
    cx.update(|cx| set_plugins(cx, vec![found]));
    until(cx, "the stream plugin to run", running);
    let root = root.canonicalize().unwrap();
    cx.update(|cx| cx.global::<PluginHost>().jobs.set_roots(vec![root.clone()]));
    root
}

fn guest(path: &Path) -> String {
    to_guest(path.to_str().unwrap(), Rules::HOST).unwrap()
}

/// Runs [`STREAM`]'s job `name` with `args` to its end, and returns how it ended.
fn run_stream(
    workbench: &Entity<Workbench>,
    runs: usize,
    name: &str,
    args: &str,
    cx: &mut TestAppContext,
) -> JobEnd {
    start_job(name, args, cx);
    let job = job_of(&until_runs(workbench, runs, cx)[runs - 1]);
    until(cx, "the job's worker to end", |cx| {
        worker_ends(cx).iter().any(|(ended, _)| *ended == job)
    });
    let ends = worker_ends(cx);
    let (_, end) = ends.iter().find(|(ended, _)| *ended == job).unwrap();
    end.clone()
}

#[gpui::test]
fn a_workers_write_copies_only_what_stdin_takes(cx: &mut TestAppContext) {
    let (plugins, workbench, vcx) = start(cx);
    let cx: &mut TestAppContext = vcx;
    load_stream(&plugins, cx);
    // `huge` writes 64 MiB in each call to a program that never reads its stdin, until a write
    // takes nothing.
    LARGEST.store(0, std::sync::atomic::Ordering::SeqCst);
    LARGEST_ARMED.store(true, std::sync::atomic::Ordering::SeqCst);
    let end = run_stream(&workbench, 1, "huge", "", cx);
    LARGEST_ARMED.store(false, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(end, JobEnd::Passed);
    // None of a write's 64 MiB was copied into the host past what stdin took.
    let largest = LARGEST.load(std::sync::atomic::Ordering::SeqCst);
    assert!(
        largest < 64 << 20,
        "the host allocated {largest} bytes at once"
    );
    cx.update(|cx| unload_plugin(RUNNER, cx));
}

#[gpui::test]
fn a_workers_programs_start_in_its_roots_only(cx: &mut TestAppContext) {
    let (plugins, workbench, vcx) = start(cx);
    let cx: &mut TestAppContext = vcx;
    let root = load_stream(&plugins, cx);
    // `in` starts `show` in the folder its args name: an open root's, then one outside.
    assert_eq!(
        run_stream(&workbench, 1, "in", &guest(&root), cx),
        JobEnd::Passed
    );
    let outside = plugins.dir.path().join("programs").canonicalize().unwrap();
    let refused = run_stream(&workbench, 2, "in", &guest(&outside), cx);
    assert!(matches!(refused, JobEnd::Failed(_)), "{refused:?}");
    cx.update(|cx| unload_plugin(RUNNER, cx));
}
