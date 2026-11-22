// SPDX-License-Identifier: Apache-2.0

//! `process.start`'s `cwd` is resolved in the directory handles the WASI preopens were made
//! from, and on Unix the program starts in the open directory: a root retargeted after its
//! preopen keeps the preopened directory (C1), and a directory replaced between the final
//! resolution and the launch changes nothing (C2). On Windows the program is launched by path
//! after a recheck, and the documented window is asserted instead (C3).

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use kiln_command::{Capability, Grants};
use kiln_plugin::{
    bindings::HostState,
    engine::PluginEngine,
    fs::hooks,
    grants::{GrantRecord, resolve_program},
    jobs::{self, Ended, JobHost, JobId, JobStatus, Jobs},
    limits::{BackgroundLimits, Ceilings, Limits},
    paths::{self, Rules},
    process,
    runner::{Call, Plugin, Reply},
};

#[path = "support/script.rs"]
mod support;

const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/stream.wat");

const PROGRAMS: [&str; 6] = ["idle", "flood", "gate", "tree", "cat", "show"];

/// The hooks are process-wide: one test at a time.
fn serially() -> MutexGuard<'static, ()> {
    static SERIAL: Mutex<()> = Mutex::new(());
    SERIAL.lock().unwrap_or_else(PoisonError::into_inner)
}

fn wait_for(timeout: Duration, mut done: impl FnMut() -> bool) -> bool {
    let until = Instant::now() + timeout;
    while Instant::now() < until {
        if done() {
            return true;
        }
        thread::sleep(Duration::from_millis(10));
    }
    done()
}

struct Programs {
    dir: PathBuf,
    root: PathBuf,
}

impl Programs {
    fn new(test: &str) -> Self {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("kiln-plugin-handles-{}-{test}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        let dir = kiln_core::platform::canonicalize(&dir).unwrap();
        let root = dir.join("root");
        std::fs::create_dir_all(&root).unwrap();
        let programs = Self { dir, root };
        let (waiting, go) = (programs.file("waiting"), programs.file("go"));
        let (waiting, go) = (waiting.display().to_string(), go.display().to_string());
        support::script(
            &programs.dir,
            "show",
            "echo \"$PATH\"\npwd -P",
            "echo %PATH%\ncd",
        );
        support::script(
            &programs.dir,
            "gate",
            &format!("touch '{waiting}'\nwhile [ ! -e '{go}' ]; do sleep 0.02; done"),
            &format!(
                "type nul > \"{waiting}\"\n:wait\nif exist \"{go}\" exit /b 0\n\
                 ping -n 2 127.0.0.1 >nul\ngoto wait"
            ),
        );
        for name in ["idle", "flood", "tree", "cat"] {
            support::script(&programs.dir, name, "exit 0", "exit /b 0");
        }
        programs
    }

    fn file(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }

    fn load(&self, host: &JobHost) -> Plugin {
        let mut capabilities = vec!["jobs".to_owned(), "fs.read:workspace".to_owned()];
        capabilities.extend(PROGRAMS.map(|program| format!("process.spawn:{program}")));
        let granted: Grants = (capabilities.iter())
            .map(|capability| Capability::parse(capability).unwrap())
            .collect();
        let bound = PROGRAMS.map(|program| {
            let identity = resolve_program(program, self.dir.as_os_str()).unwrap();
            (program.to_owned(), identity)
        });
        let record = GrantRecord {
            version: Some((1, 0, 0)),
            granted: granted.clone(),
            programs: bound.into_iter().collect(),
            ..GrantRecord::default()
        };
        host.set_roots(vec![self.root.clone()]);
        let workers: jobs::WorkerHost = Arc::new(move || {
            let mut state = HostState::new(Box::new(|_, _| {}));
            state.process = process::State::new(record.clone(), &Limits::default());
            state
        });
        let mut state = HostState::new(Box::new(|_, _| {}));
        state.jobs =
            jobs::State::new(host.clone(), BackgroundLimits::default()).with_workers(workers);
        let component = wat::parse_file(FIXTURE).unwrap();
        let limits = Limits {
            call_deadline: Duration::from_secs(2),
            ..Limits::default()
        };
        Plugin::load_hosted(
            PluginEngine::shared().unwrap(),
            None,
            &component,
            "acme.stream",
            &granted,
            limits,
            state,
        )
        .unwrap()
    }
}

fn job_host() -> JobHost {
    JobHost::new(Ceilings::for_ram(64 << 30))
}

fn guest(path: &Path) -> String {
    paths::to_guest(path.to_str().unwrap(), Rules::HOST).unwrap()
}

fn jobs_of(plugin: &Plugin) -> Jobs {
    plugin.jobs().expect("loaded with jobs").clone()
}

fn start(plugin: &mut Plugin, name: &str, args: &str) -> JobId {
    let before = jobs_of(plugin).ids();
    let call = Call::RunCommand {
        id: name.to_owned(),
        args: args.to_owned(),
    };
    match plugin.call(call).unwrap() {
        Reply::RunCommand(result) => result.unwrap(),
        reply => panic!("{reply:?}"),
    }
    let after = jobs_of(plugin).ids();
    *after.iter().find(|id| !before.contains(id)).unwrap()
}

fn ended(plugin: &Plugin, job: JobId) -> Ended {
    let jobs = jobs_of(plugin);
    let done = |status: JobStatus| {
        !matches!(
            status,
            JobStatus::Queued(_) | JobStatus::Running(_) | JobStatus::Unknown
        )
    };
    assert!(
        wait_for(Duration::from_secs(30), || done(jobs.state(job))),
        "{:?}",
        jobs.state(job)
    );
    let (id, ended) = jobs.take_ended().unwrap();
    assert_eq!(id, job);
    ended
}

/// The second line `show` printed: the directory it ran in.
fn ran_in(ended: Ended) -> String {
    let Ended::Finished(Ok(shown)) = ended else {
        panic!("show didn't run: {ended:?}");
    };
    shown.lines().nth(1).map(str::trim).unwrap().to_owned()
}

/// C1: a root that is a link, retargeted between its preopen and the store's setup, keeps the
/// directory it was preopened as: `start` with a `cwd` in it runs there.
#[cfg(unix)]
#[test]
fn start_in_a_root_retargeted_after_its_preopen_runs_in_the_preopened_directory() {
    let _serial = serially();
    let programs = Programs::new("preopen");
    let outside = programs.dir.join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    let link = programs.dir.join("link");
    std::os::unix::fs::symlink(&programs.root, &link).unwrap();
    let host = job_host();
    let mut plugin = programs.load(&host);
    host.set_roots(vec![link.clone()]);
    let retarget = link.clone();
    let target = outside.clone();
    let _hook = hooks::after_preopen(move |root| {
        if root == retarget {
            std::fs::remove_file(&retarget).unwrap();
            std::os::unix::fs::symlink(&target, &retarget).unwrap();
        }
    });
    let job = start(&mut plugin, "in", &guest(&link));
    assert_eq!(
        ran_in(ended(&plugin, job)),
        programs.root.to_str().unwrap(),
        "the program ran outside the preopened directory"
    );
}

/// C2: a cwd ancestor replaced by a link out of the root between the final resolution and the
/// launch changes nothing on Unix: the program starts in the directory resolved.
#[cfg(unix)]
#[test]
fn start_runs_in_the_directory_resolved_when_an_ancestor_is_replaced_before_the_launch() {
    let _serial = serially();
    let programs = Programs::new("launch");
    let cwd = programs.root.join("a").join("b");
    std::fs::create_dir_all(&cwd).unwrap();
    let outside = programs.dir.join("outside");
    std::fs::create_dir_all(outside.join("b")).unwrap();
    let host = job_host();
    let mut plugin = programs.load(&host);
    let (ancestor, moved, out) = (
        programs.root.join("a"),
        programs.root.join("a-moved"),
        outside.clone(),
    );
    let _hook = hooks::before_launch(move |_| {
        std::fs::rename(&ancestor, &moved).unwrap();
        std::os::unix::fs::symlink(&out, &ancestor).unwrap();
    });
    let job = start(&mut plugin, "in", &guest(&cwd));
    let shown = ran_in(ended(&plugin, job));
    // The directory the program started in, which now lies under the ancestor's new name.
    assert_eq!(
        shown,
        programs.root.join("a-moved").join("b").to_str().unwrap()
    );
}

/// C3: on Windows the program is launched by path after a recheck, so the same replacement
/// before the launch is caught by the recheck (the window after it stays, as documented in
/// `wit/process.wit`): the start is `denied`.
#[cfg(windows)]
#[test]
fn start_on_windows_rechecks_the_path_and_the_documented_window_remains() {
    let _serial = serially();
    let programs = Programs::new("launch-windows");
    let cwd = programs.root.join("work");
    std::fs::create_dir_all(&cwd).unwrap();
    let host = job_host();
    let mut plugin = programs.load(&host);
    let (dir, moved) = (cwd.clone(), programs.root.join("moved"));
    let _hook = hooks::before_launch(move |_| {
        std::fs::rename(&dir, &moved).unwrap();
        std::fs::create_dir_all(&dir).unwrap();
    });
    let job = start(&mut plugin, "in", &guest(&cwd));
    let ended = ended(&plugin, job);
    assert!(
        matches!(
            &ended,
            Ended::Finished(Err(kiln_plugin::bindings::Error::Denied(_)))
        ),
        "{ended:?}"
    );
}
