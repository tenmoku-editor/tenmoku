// SPDX-License-Identifier: Apache-2.0

//! Streamed programs: `process.start` and `child` in background jobs, with each stream held to
//! 1 MiB in the host, at most 8 children per job, and each child's tree ended on a cancel and at
//! the job's wall time. Each test loads `fixtures/stream.wat` on a [`JobHost`] of its own, and
//! its programs are `support::script` scripts, so every test runs on Unix and Windows alike.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use kiln_command::{Capability, Grants};
use kiln_plugin::{
    bindings::{Error, HostState},
    engine::PluginEngine,
    grants::{GrantRecord, resolve_program},
    jobs::{self, CANCEL_GRACE, Ended, JobHost, JobId, JobStatus, Jobs},
    limits::{BackgroundLimits, Ceilings, Limits},
    paths::{self, Rules},
    process,
    process_stream::{self, MAX_CHILDREN, NOT_IN_A_JOB, STREAM_BYTES, TOO_MANY_CHILDREN},
    runner::{Call, Plugin, Reply},
};

#[path = "support/script.rs"]
mod support;

/// The test binary's allocator, noting the largest allocation since [`Largest::reset`]: the
/// host's own memory, apart from a guest's, which wasmtime maps itself.
struct Largest;

static LARGEST: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

impl Largest {
    fn reset() {
        LARGEST.store(0, std::sync::atomic::Ordering::SeqCst);
    }

    fn get() -> usize {
        LARGEST.load(std::sync::atomic::Ordering::SeqCst)
    }

    fn note(size: usize) {
        LARGEST.fetch_max(size, std::sync::atomic::Ordering::Relaxed);
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

const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/stream.wat");

/// The programs the fixture starts, each granted.
const PROGRAMS: [&str; 6] = ["idle", "flood", "gate", "tree", "cat", "show"];

/// How many 1 MiB chunks `flood` writes.
const FLOOD_CHUNKS: usize = 8;

/// One test at a time: the stream threads counted ([`process_stream::stream_threads`]) are this
/// process's, and `cargo test` runs a file's tests side by side in one process. Each starts once
/// the last one's threads are gone.
fn serially() -> MutexGuard<'static, ()> {
    static SERIAL: Mutex<()> = Mutex::new(());
    let guard = SERIAL.lock().unwrap_or_else(PoisonError::into_inner);
    assert!(wait_for(Duration::from_secs(10), || {
        process_stream::stream_threads() == 0
    }));
    guard
}

/// Waits up to `timeout` for `done`, and returns whether it came.
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

/// A test's programs, in a directory of its own, and the files they leave.
struct Programs {
    dir: PathBuf,
    /// A folder open in the app, which `show` may start in.
    root: PathBuf,
}

impl Programs {
    fn new(test: &str) -> Self {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("kiln-plugin-stream-{}-{test}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        let root = dir.join("root");
        std::fs::create_dir_all(&root).unwrap();
        let programs = Self { dir, root };
        let file = |name: &str| programs.file(name).display().to_string();
        let (chunk, progress, go) = (file("chunk"), file("progress"), file("go"));
        let waiting = file("waiting");
        let (started, termed, grandchild) = (file("started"), file("termed"), file("grandchild"));
        std::fs::write(programs.file("chunk"), vec![b'x'; STREAM_BYTES]).unwrap();
        let script = |name, sh: String, cmd: String| {
            support::script(&programs.dir, name, &sh, &cmd);
        };
        script(
            "idle",
            "exec sleep 600".into(),
            "ping -n 601 127.0.0.1 >nul".into(),
        );
        script(
            "flood",
            format!("for i in 1 2 3 4 5 6 7 8; do cat '{chunk}'; echo $i > '{progress}'; done"),
            // The redirection goes first: `echo 1> f` would redirect handle 1.
            format!("for /L %%i in (1,1,8) do (type \"{chunk}\" & >\"{progress}\" echo %%i)"),
        );
        script(
            "gate",
            format!("touch '{waiting}'\nwhile [ ! -e '{go}' ]; do sleep 0.02; done"),
            format!(
                "type nul > \"{waiting}\"\n:wait\nif exist \"{go}\" exit /b 0\n\
                 ping -n 2 127.0.0.1 >nul\ngoto wait"
            ),
        );
        // A grandchild that holds the program's stdout, and on Unix says its pid, and that
        // SIGTERM came.
        script(
            "tree",
            format!(
                "trap 'touch \"{termed}\"; exit 0' TERM\nsleep 600 &\necho $! > '{grandchild}'\n\
                 touch '{started}'\nwait"
            ),
            format!(
                "start \"\" /b ping -n 600 127.0.0.1\ntype nul > \"{started}\"\n\
                 ping -n 600 127.0.0.1 >nul"
            ),
        );
        script("cat", "exec cat".into(), "findstr \"^\"".into());
        script(
            "show",
            "echo \"$PATH\"\npwd -P".into(),
            "echo %PATH%\ncd".into(),
        );
        programs
    }

    fn file(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }

    /// The fixture loaded as `acme.stream` on `host`, its jobs under `background`, each program
    /// granted and bound to its script, with `fs.read:workspace`.
    fn load(&self, host: &JobHost, background: BackgroundLimits) -> Plugin {
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
        // The roots the runner preopens for a job, and hands its process slice.
        host.set_roots(vec![self.root.clone()]);
        let workers: jobs::WorkerHost = Arc::new(move || {
            let mut state = HostState::new(Box::new(|_, _| {}));
            state.process = process::State::new(record.clone(), &Limits::default());
            state
        });
        let mut state = HostState::new(Box::new(|_, _| {}));
        state.jobs = jobs::State::new(host.clone(), background).with_workers(workers);
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

    /// The pid `tree`'s grandchild wrote, on Unix.
    #[cfg(unix)]
    fn grandchild(&self) -> String {
        let pid = std::fs::read_to_string(self.file("grandchild")).unwrap();
        pid.trim().to_owned()
    }
}

/// A queue of its own, on a computer with plenty of RAM.
fn job_host() -> JobHost {
    JobHost::new(Ceilings::for_ram(64 << 30))
}

/// What the fixture's `run-command(id, args)` returned.
fn run(plugin: &mut Plugin, id: &str, args: &str) -> Result<(), Error> {
    let call = Call::RunCommand {
        id: id.to_owned(),
        args: args.to_owned(),
    };
    match plugin.call(call).unwrap() {
        Reply::RunCommand(result) => result,
        reply => panic!("{reply:?}"),
    }
}

fn jobs_of(plugin: &Plugin) -> Jobs {
    plugin.jobs().expect("loaded with jobs").clone()
}

/// Starts job `name` with `args`, and returns its id.
fn start(plugin: &mut Plugin, name: &str, args: &str) -> JobId {
    let before = jobs_of(plugin).ids();
    run(plugin, name, args).unwrap();
    let after = jobs_of(plugin).ids();
    *after.iter().find(|id| !before.contains(id)).unwrap()
}

/// How `job` ended, waiting up to `timeout` for it.
fn ended(plugin: &Plugin, job: JobId, timeout: Duration) -> Ended {
    let jobs = jobs_of(plugin);
    let done = |status: JobStatus| {
        !matches!(
            status,
            JobStatus::Queued(_) | JobStatus::Running(_) | JobStatus::Unknown
        )
    };
    assert!(
        wait_for(timeout, || done(jobs.state(job))),
        "{:?}",
        jobs.state(job)
    );
    let (id, ended) = jobs.take_ended().unwrap();
    assert_eq!(id, job);
    ended
}

/// Runs job `name` with `args` to its end.
fn run_job(plugin: &mut Plugin, name: &str, args: &str) -> Ended {
    let job = start(plugin, name, args);
    ended(plugin, job, Duration::from_secs(30))
}

/// Whether every stream thread has ended within 10 s: each ends with its stream, so a
/// grandchild still holding a program's stdout would keep one.
fn streams_end() -> bool {
    wait_for(Duration::from_secs(10), || {
        process_stream::stream_threads() == 0
    })
}

/// Whether the process `pid` has gone within 10 s.
#[cfg(unix)]
fn gone(pid: &str) -> bool {
    wait_for(Duration::from_secs(10), || {
        let alive = std::process::Command::new("kill")
            .args(["-0", pid])
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        !alive.success()
    })
}

fn read_trimmed(path: &Path) -> String {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .trim()
        .to_owned()
}

#[test]
fn stream_fast_writer_blocks_until_guest_reads() {
    let _serial = serially();
    let programs = Programs::new("flood");
    let host = job_host();
    let mut plugin = programs.load(&host, BackgroundLimits::default());
    let job = start(&mut plugin, "flood", "");
    let progress = programs.file("progress");

    // The first 1 MiB fills the host's buffer of stdout; the second can't be written while the
    // guest doesn't read, however long it waits.
    assert!(wait_for(Duration::from_secs(10), || read_trimmed(
        &progress
    ) == "1"));
    thread::sleep(Duration::from_secs(1));
    assert_eq!(read_trimmed(&progress), "1");
    assert!(matches!(jobs_of(&plugin).state(job), JobStatus::Running(_)));

    // Once the guest reads, the program writes the rest, and nothing was lost on the way.
    std::fs::write(programs.file("go"), "").unwrap();
    let total = (FLOOD_CHUNKS * STREAM_BYTES).to_string();
    assert_eq!(
        ended(&plugin, job, Duration::from_secs(30)),
        Ended::Finished(Ok(total))
    );
    assert_eq!(read_trimmed(&progress), FLOOD_CHUNKS.to_string());
    assert!(streams_end());
}

#[test]
fn stream_cancel_kills_grandchildren() {
    let _serial = serially();
    let programs = Programs::new("cancel");
    let host = job_host();
    let mut plugin = programs.load(&host, BackgroundLimits::default());
    let job = start(&mut plugin, "tree", "");
    assert!(wait_for(Duration::from_secs(10), || programs
        .file("started")
        .exists()));
    assert_eq!(process_stream::stream_threads(), 3);

    let cancelled = Instant::now();
    jobs_of(&plugin).cancel(job).unwrap();
    assert_eq!(
        ended(&plugin, job, Duration::from_secs(10)),
        Ended::Cancelled
    );
    let took = cancelled.elapsed();
    assert!(took < CANCEL_GRACE + Duration::from_secs(3), "{took:?}");
    // The grandchild held the program's stdout: its thread ends only once it has gone too.
    assert!(streams_end(), "{}", process_stream::stream_threads());
    assert_eq!(host.counts(), jobs::Counts::default());
    // On Unix the tree had SIGTERM first, and the grandchild's pid is known: it has gone.
    #[cfg(unix)]
    {
        assert!(programs.file("termed").exists());
        assert!(gone(&programs.grandchild()));
    }
}

#[test]
fn stream_wall_time_kills_the_tree() {
    let _serial = serially();
    // `tree` waits in `read`, `busy` runs wasm without entering the host.
    for name in ["tree", "busy"] {
        let programs = Programs::new(&format!("wall-{name}"));
        let host = job_host();
        let wall = Duration::from_secs(1);
        let background = BackgroundLimits {
            wall,
            ..BackgroundLimits::default()
        };
        let mut plugin = programs.load(&host, background);
        let started = Instant::now();
        let job = start(&mut plugin, name, "");
        assert!(
            wait_for(Duration::from_secs(10), || programs
                .file("started")
                .exists()),
            "{name}"
        );
        assert_eq!(
            ended(&plugin, job, Duration::from_secs(10)),
            Ended::TimedOut,
            "{name}"
        );
        let took = started.elapsed();
        assert!(took >= wall, "{name}: {took:?}");
        assert!(
            took < wall + CANCEL_GRACE + Duration::from_secs(3),
            "{name}: {took:?}"
        );
        assert!(
            streams_end(),
            "{name}: {}",
            process_stream::stream_threads()
        );
        #[cfg(unix)]
        {
            assert!(programs.file("termed").exists(), "{name}");
            assert!(gone(&programs.grandchild()), "{name}");
        }
    }
}

#[test]
fn stream_start_refused_in_interactive_call() {
    let _serial = serially();
    let programs = Programs::new("interactive");
    let host = job_host();
    let mut plugin = programs.load(&host, BackgroundLimits::default());
    assert_eq!(
        run(&mut plugin, "start", ""),
        Err(Error::Denied(NOT_IN_A_JOB.to_owned()))
    );
    assert_eq!(process_stream::stream_threads(), 0);
}

#[test]
fn stream_ninth_child_refused() {
    let _serial = serially();
    let programs = Programs::new("nine");
    let host = job_host();
    let mut plugin = programs.load(&host, BackgroundLimits::default());
    assert_eq!(MAX_CHILDREN, 8);
    // The ninth is refused while eight are held; once one is dropped, one more starts.
    assert_eq!(
        run_job(&mut plugin, "nine", ""),
        Ended::Finished(Ok(TOO_MANY_CHILDREN.to_owned()))
    );
    // The eight it left running ended with the job.
    assert!(streams_end(), "{}", process_stream::stream_threads());
}

#[test]
fn stream_start_needs_a_grant() {
    let _serial = serially();
    let programs = Programs::new("grant");
    let host = job_host();
    let mut plugin = programs.load(&host, BackgroundLimits::default());
    let ended = run_job(&mut plugin, "run", "nope");
    assert!(
        matches!(&ended, Ended::Finished(Err(Error::Denied(_)))),
        "{ended:?}"
    );
    // A granted program that changed since is suspended, and doesn't start either.
    support::script(
        &programs.dir,
        "idle",
        "exec sleep 6000",
        "ping -n 6001 127.0.0.1 >nul",
    );
    let ended = run_job(&mut plugin, "run", "idle");
    assert!(
        matches!(&ended, Ended::Finished(Err(Error::Denied(_)))),
        "{ended:?}"
    );
    assert_eq!(process_stream::stream_threads(), 0);
}

/// The guest path of `path`, as a plugin names it.
fn guest(path: &Path) -> String {
    paths::to_guest(path.to_str().unwrap(), Rules::HOST).unwrap()
}

#[test]
fn stream_environment_and_cwd_are_spawns() {
    let _serial = serially();
    let programs = Programs::new("show");
    let host = job_host();
    let mut plugin = programs.load(&host, BackgroundLimits::default());
    let Ended::Finished(Ok(shown)) = run_job(&mut plugin, "in", &guest(&programs.root)) else {
        panic!("show didn't run");
    };
    let lines: Vec<_> = shown.lines().map(str::trim).collect();
    let system_path = kiln_core::platform::system_path();
    assert_eq!(lines[0], system_path.to_str().unwrap());
    let root = kiln_core::platform::canonicalize(&programs.root).unwrap();
    assert_eq!(lines[1], root.to_str().unwrap());

    // Outside every root, or with a `..` in it, it is refused, and nothing starts.
    let outside = run_job(&mut plugin, "in", &guest(&programs.dir));
    assert!(
        matches!(&outside, Ended::Finished(Err(Error::Denied(_)))),
        "{outside:?}"
    );
    let dotted = format!("{}/..", guest(&programs.root));
    let dotted = run_job(&mut plugin, "in", &dotted);
    assert!(
        matches!(&dotted, Ended::Finished(Err(Error::InvalidArgument(_)))),
        "{dotted:?}"
    );
    assert_eq!(process_stream::stream_threads(), 0);
}

/// A link inside the root to a directory outside it leads out: refused with its links resolved.
#[cfg(unix)]
#[test]
fn stream_cwd_link_out_of_the_root_refused() {
    let _serial = serially();
    let programs = Programs::new("link");
    let link = programs.root.join("out");
    std::os::unix::fs::symlink(&programs.dir, &link).unwrap();
    let host = job_host();
    let mut plugin = programs.load(&host, BackgroundLimits::default());
    let ended = run_job(&mut plugin, "in", &guest(&link));
    assert!(
        matches!(&ended, Ended::Finished(Err(Error::Denied(_)))),
        "{ended:?}"
    );
}

#[test]
fn stream_stdin_reaches_the_program() {
    let _serial = serially();
    let programs = Programs::new("cat");
    let host = job_host();
    let mut plugin = programs.load(&host, BackgroundLimits::default());
    let Ended::Finished(Ok(echoed)) = run_job(&mut plugin, "cat", "hello\n") else {
        panic!("cat didn't run");
    };
    assert_eq!(echoed.trim(), "hello");
    assert!(streams_end());
}

#[test]
fn stream_kill_ends_the_tree() {
    let _serial = serially();
    let programs = Programs::new("kill");
    let host = job_host();
    let mut plugin = programs.load(&host, BackgroundLimits::default());
    // Its exit says the host killed it.
    assert_eq!(
        run_job(&mut plugin, "kill", ""),
        Ended::Finished(Ok("k".to_owned()))
    );
    assert!(streams_end());
}

#[test]
fn stream_oversized_write_copies_only_what_stdin_takes() {
    let _serial = serially();
    let programs = Programs::new("huge");
    let host = job_host();
    let mut plugin = programs.load(&host, BackgroundLimits::default());
    Largest::reset();
    // `idle` never reads its stdin: the writes fill its buffer (and the pipe), then take nothing.
    let Ended::Finished(Ok(taken)) = run_job(&mut plugin, "huge", "") else {
        panic!("huge didn't run");
    };
    let taken: usize = taken.parse().unwrap();
    assert!(taken >= STREAM_BYTES, "{taken}");
    assert!(taken < 2 * STREAM_BYTES, "{taken}");
    // Each write passed 64 MiB; none of it was copied into the host past what was taken.
    let largest = Largest::get();
    assert!(
        largest < 16 << 20,
        "the host allocated {largest} bytes at once"
    );
    assert!(streams_end());
}

/// A root that couldn't be opened as a directory is no root to start in, like the preopen that
/// failed with it.
#[test]
fn stream_cwd_in_a_root_that_failed_to_open_refused() {
    let _serial = serially();
    let programs = Programs::new("unopened");
    let missing = programs.dir.join("missing");
    let file = programs.dir.join("file");
    std::fs::write(&file, "").unwrap();
    let host = job_host();
    let mut plugin = programs.load(&host, BackgroundLimits::default());
    host.set_roots(vec![programs.root.clone(), missing.clone(), file.clone()]);
    for root in [&missing, &file] {
        let ended = run_job(&mut plugin, "in", &guest(root));
        assert!(
            matches!(&ended, Ended::Finished(Err(Error::Denied(_)))),
            "{}: {ended:?}",
            root.display()
        );
    }
    assert_eq!(process_stream::stream_threads(), 0);
}

/// A root that is a link, pointed elsewhere once the job's store is made, still names the
/// directory it named then, as its preopen does.
#[cfg(unix)]
#[test]
fn stream_cwd_root_retargeted_after_the_store_is_made_keeps_its_directory() {
    let _serial = serially();
    let programs = Programs::new("retarget");
    let outside = programs.dir.join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    let link = programs.dir.join("link");
    std::os::unix::fs::symlink(&programs.root, &link).unwrap();
    let host = job_host();
    let mut plugin = programs.load(&host, BackgroundLimits::default());
    host.set_roots(vec![link.clone()]);
    let job = start(&mut plugin, "late", &guest(&link));
    assert!(wait_for(Duration::from_secs(10), || programs
        .file("waiting")
        .exists()));
    std::fs::remove_file(&link).unwrap();
    std::os::unix::fs::symlink(&outside, &link).unwrap();
    std::fs::write(programs.file("go"), "").unwrap();
    let Ended::Finished(Ok(shown)) = ended(&plugin, job, Duration::from_secs(30)) else {
        panic!("show didn't run");
    };
    let root = kiln_core::platform::canonicalize(&programs.root).unwrap();
    assert_eq!(shown.lines().nth(1).map(str::trim), root.to_str());
}
