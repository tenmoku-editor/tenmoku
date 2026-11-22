//! A plugin's background jobs in the app (`docs/decisions/dqsjfernkt-plugins-stage-3.md`
//! §3.2 to §3.5): run in worker stores of this host's own, and shown in each window's
//! [`Runs`].
//!
//! - **The queue.** The app's plugins share one kiln-plugin [`JobHost`], under this computer's
//!   [`Ceilings::host`], which a plugin with the `jobs` capability is attached to as it loads
//!   ([`JobsHub::attach`]). Its `jobs` imports are this host's ([`link`]): they start, cancel
//!   and tell of the jobs of that load, and note each job started for the Runs list.
//! - **Worker stores.** A job runs in a worker store ([`Workers`]): a second instance of the
//!   plugin's component, instantiated from the same [`InstancePre`] its interactive instance
//!   was, so from the component the cache compiled, without `init`, on a thread of its own at
//!   lowered priority. Its imports run against a host state of its own made as the interactive
//!   one is ([`WorkerData`]): buffers, decorations, diagnostics, status, files, network,
//!   secrets and the plugin's data store under the plugin's grants, its data counter and its
//!   decorations, diagnostics and status attachments the interactive store's load's, so what it
//!   publishes is that load's. A wait on the main thread for the buffers ends as soon as the job
//!   is stopped. Imports that register something (commands, formatters, events)
//!   answer `denied("not in a background job")`. As kiln-plugin's runner does, the epoch stops
//!   a job that runs [`jobs::SLICE`] of wasm without entering an import, one past its wall time
//!   is stopped as timed out, and one still running [`jobs::CANCEL_GRACE`] after it was stopped
//!   ends whatever it is doing; its result is checked and charged as it is lifted, before it is
//!   copied out of the worker ([`KeptText`]).
//! - **Delivery.** A job's end is handed to the interactive instance's `job-finished` on the
//!   plugin's own thread, between calls ([`JobsLoad::take_ended`]): the queue wakes it. A few at
//!   a time, the calls and events waiting going in between.
//! - **Runs.** Every [`POLL`] while there are jobs, from the first one started, `init`'s
//!   included, each window's [`Runs`] is brought up to date ([`serve`], [`sync`]): a job is a run there, owned by its plugin ([`RunOwner::Plugin`]), of the kind
//!   its spec says, in the windows whose root is the job's (all of them for a job about no
//!   root). A queued job is [`RunStatus::Queued`] with what it waits for; a running one shows
//!   its progress, its text cut to [`crate::runs::MAX_TEXT_CHARS`] here as well as by
//!   `job-context.report`; an ended one passes, fails with its error's text, is cancelled, or
//!   fails as [`FailReason::TimedOut`]. Cancel in the Runs list calls `jobs.cancel` ([`cancel`]).
//! - **Unload.** Unloading, reloading or disabling a plugin, and the app quitting
//!   ([`quit`]), detach its load ([`JobsLoad::detach`]): its running jobs are stopped as
//!   cancelled and its queued ones removed. Their runs end cancelled, a running one's once its
//!   worker has ended; a worker the queue launches as its load detaches never starts. Quitting
//!   detaches every load attached, a plugin's still in its `init` too, and any load attaching
//!   after it is detached at once.
//!
//! [`RunStatus::Queued`]: crate::runs::RunStatus::Queued

use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap},
    future::{Future, poll_fn},
    mem,
    path::Path,
    pin::pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        mpsc::SyncSender,
    },
    task::Poll,
    thread,
    time::{Duration, Instant},
};

use gpui::{App, Entity, Global, WeakEntity};
use kiln_l10n::tr;
use kiln_plugin::{
    bindings::{Error, jobs_world::kiln::extension::jobs as wit},
    engine::PluginEngine,
    jobs::{self, Ended, JobControl, JobHost, Jobs, Launch, Launcher, Reservation, Stop},
    limits::{BackgroundLimits, Ceilings},
    paths::{Rules, root_to_guest},
};
use wasmtime::{
    CallHook, Store, UpdateDeadline,
    component::{ComponentType, InstancePre, Lift, Linker, WasmStr},
};

use super::{Data, PluginHost, Work, lock, thread_name, ticking, wake_on_tick};
use crate::{
    Workbench,
    runs::{FailReason, RunEnd, RunId, RunKind, RunOwner, RunStatus, RunSummary, Runs, Waiting},
};

pub(crate) use wit::{JobId, JobKind, JobSpec, JobStatus, Progress};

/// How often the Runs lists are brought up to date while there are jobs: at most ten updates
/// a second reach them, the latest winning.
pub(crate) const POLL: Duration = Duration::from_millis(100);

/// How long quitting waits for the jobs' workers to end, once they are stopped: their grace,
/// and a little more.
const QUIT_WAIT: Duration = Duration::from_millis(2500);

/// A job ended, as the Runs list shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum JobEnd {
    /// `run-job` returned a result.
    Passed,
    /// It failed, with this message.
    Failed(String),
    Cancelled,
    TimedOut,
}

impl JobEnd {
    fn of(ended: &Ended) -> Self {
        match ended {
            Ended::Finished(Ok(_)) => Self::Passed,
            Ended::Finished(Err(error)) => Self::Failed(message(error).to_owned()),
            Ended::Cancelled => Self::Cancelled,
            Ended::TimedOut => Self::TimedOut,
        }
    }
}

/// `error`'s message.
fn message(error: &Error) -> &str {
    match error {
        Error::Unimplemented(message)
        | Error::Denied(message)
        | Error::NotFound(message)
        | Error::InvalidArgument(message)
        | Error::Conflict(message)
        | Error::Failed(message) => message,
    }
}

/// What a job's run says of it: what its spec named, kept as it was started.
#[derive(Clone, Debug)]
struct Shown {
    title: String,
    kind: RunKind,
    cancellable: bool,
    /// The open root it works on, as a guest path.
    root: Option<String>,
}

/// One job the Runs lists show, or will once its `start` has returned.
struct Entry {
    plugin: String,
    /// Which load of the plugin started it.
    load: u64,
    /// None until `start` returned: the queue may launch a job before.
    shown: Option<Shown>,
    jobs: Option<Jobs>,
    /// Its worker's, once the queue launched it.
    launched: Option<JobControl>,
    /// How its worker ended, once it has.
    worker_end: Option<JobEnd>,
    /// Its load was detached: none delivers its end.
    detached: bool,
    /// How its run ends: as it was delivered, or as its worker ended once its load was
    /// detached.
    end: Option<JobEnd>,
}

impl Entry {
    /// Its load was detached: a job that never launched ends cancelled now, a launched one as
    /// its worker ended, once it has.
    fn detach(&mut self) {
        self.detached = true;
        if self.end.is_some() {
            return;
        }
        match (&self.launched, &self.worker_end) {
            (None, _) => self.end = Some(JobEnd::Cancelled),
            (Some(_), Some(end)) => self.end = Some(end.clone()),
            (Some(_), None) => {}
        }
    }

    fn new(plugin: &str, load: u64) -> Self {
        Self {
            plugin: plugin.to_owned(),
            load,
            shown: None,
            jobs: None,
            launched: None,
            worker_end: None,
            detached: false,
            end: None,
        }
    }
}

/// The app's plugin jobs: the queue, the jobs the Runs lists show and the plugins' threads to
/// wake for delivery. Clones share it.
#[derive(Clone)]
pub(crate) struct JobsHub(Arc<Hub>);

struct Hub {
    queue: JobHost,
    board: Mutex<BTreeMap<JobId, Entry>>,
    /// Each plugin's thread, woken when one of its jobs ends.
    wakes: Mutex<HashMap<String, (u64, SyncSender<Work>)>>,
    /// The loads attached and not detached, a plugin's still in its `init` included.
    live: Mutex<Live>,
    next_load: AtomicU64,
    /// Wakes the Runs lists' sync ([`serve`]), which sleeps while there are no jobs: sent as a
    /// job starts and as a load detaches.
    kick: async_channel::Sender<()>,
    kicked: async_channel::Receiver<()>,
    /// The workers running.
    workers: AtomicUsize,
    /// Each worker's end, for tests.
    #[cfg(test)]
    worker_ends: Mutex<Vec<(JobId, JobEnd)>>,
}

impl Default for JobsHub {
    fn default() -> Self {
        Self::new(Ceilings::host())
    }
}

/// The loads attached, and whether the app is quitting: a load attached then is detached at
/// once.
#[derive(Default)]
struct Live {
    closed: bool,
    loads: HashMap<u64, Load>,
}

/// What detaching a load needs of it. Clones share it.
#[derive(Clone)]
struct Load {
    plugin: String,
    jobs: Jobs,
    /// Set as it is detached, before anything else is: a worker launched for it after that
    /// never starts.
    detached: Arc<AtomicBool>,
}

impl JobsHub {
    /// A hub with a queue of its own under `ceilings`.
    pub(crate) fn new(ceilings: Ceilings) -> Self {
        let (kick, kicked) = async_channel::bounded(1);
        let hub = Arc::new(Hub {
            queue: JobHost::new(ceilings),
            board: Mutex::default(),
            wakes: Mutex::default(),
            live: Mutex::default(),
            next_load: AtomicU64::new(1),
            kick,
            kicked,
            workers: AtomicUsize::new(0),
            #[cfg(test)]
            worker_ends: Mutex::default(),
        });
        let weak = Arc::downgrade(&hub);
        hub.queue.on_ended(Arc::new(move |plugin: &str| {
            if let Some(hub) = weak.upgrade() {
                hub.wake(plugin);
            }
        }));
        Self(hub)
    }

    /// The roots of the open windows, as host paths, which a job may name.
    pub(crate) fn set_roots(&self, roots: Vec<std::path::PathBuf>) {
        self.0.queue.set_roots(roots);
    }

    /// Attaches plugin `id`'s new load to the queue under `limits`, its jobs run by `workers`.
    /// Once the app is quitting ([`Self::close`]), the load is detached as it attaches: it
    /// starts no jobs.
    pub(crate) fn attach(
        &self,
        id: &str,
        grants: &fe_command::Grants,
        limits: BackgroundLimits,
        workers: Workers,
    ) -> JobsLoad {
        let load = self.0.next_load.fetch_add(1, Ordering::Relaxed);
        let workers = Arc::new(workers);
        let detached = Arc::<AtomicBool>::default();
        let (hub, flag) = (self.clone(), Arc::clone(&detached));
        let launcher: Launcher = Arc::new(move |launch| workers.launch(&hub, load, &flag, launch));
        let jobs = self.0.queue.attach(id, grants, limits, launcher);
        let handle = Load {
            plugin: id.to_owned(),
            jobs,
            detached,
        };
        let closed = {
            let mut live = lock(&self.0.live);
            if !live.closed {
                live.loads.insert(load, handle.clone());
            }
            live.closed
        };
        let attached = JobsLoad {
            hub: self.clone(),
            load,
            handle,
        };
        if closed {
            attached.detach();
        }
        attached
    }

    /// As the app quits: no load attaches from now on, and every load attached is detached,
    /// a plugin's still in its `init` too.
    fn close(&self) {
        let loads = {
            let mut live = lock(&self.0.live);
            live.closed = true;
            mem::take(&mut live.loads)
        };
        for (load, handle) in loads {
            self.0.detach(load, &handle);
        }
    }

    /// Whether a load is attached, or a job is still to be shown.
    fn busy(&self) -> bool {
        !lock(&self.0.live).loads.is_empty() || !lock(&self.0.board).is_empty()
    }

    /// Whether every worker has ended.
    fn idle(&self) -> bool {
        self.0.workers.load(Ordering::Acquire) == 0
    }

    /// Each worker's end, in the order they ended, for tests.
    #[cfg(test)]
    pub(crate) fn worker_ends(&self) -> Vec<(JobId, JobEnd)> {
        lock(&self.0.worker_ends).clone()
    }

    /// The jobs queued and running in the queue, for tests.
    #[cfg(test)]
    pub(crate) fn counts(&self) -> jobs::Counts {
        self.0.queue.counts()
    }

    /// Plugin `plugin`'s job `job`, as cancel in the Runs list asks: cancelled if it may be and
    /// hasn't ended. False if nothing was asked.
    fn cancel(&self, plugin: &str, job: JobId) -> bool {
        let jobs = {
            let board = lock(&self.0.board);
            let Some(entry) = board.get(&job).filter(|entry| entry.plugin == plugin) else {
                return false;
            };
            let cancellable = entry.shown.as_ref().is_some_and(|shown| shown.cancellable);
            if !cancellable || entry.end.is_some() || entry.detached {
                return false;
            }
            entry.jobs.clone()
        };
        jobs.is_some_and(|jobs| jobs.cancel(job).is_ok())
    }

    /// The jobs to show, and how far each has got, taking out those that ended and are shown.
    fn take_views(&self) -> Vec<View> {
        let mut board = lock(&self.0.board);
        let views = (board.iter())
            .filter_map(|(&job, entry)| {
                let shown = entry.shown.clone()?;
                let state = match (&entry.end, &entry.launched) {
                    (Some(end), _) => State::Ended(end.clone()),
                    (None, Some(control)) => State::Running(control.progress()),
                    (None, None) => {
                        State::Queued(match entry.jobs.as_ref().map(|jobs| jobs.state(job)) {
                            Some(JobStatus::Queued(wit::Waiting::Memory)) => Waiting::Memory,
                            _ => Waiting::Concurrency,
                        })
                    }
                };
                Some(View {
                    job,
                    plugin: entry.plugin.clone(),
                    shown,
                    launched: entry.launched.is_some(),
                    state,
                })
            })
            .collect();
        // An ended job its `start` hasn't noted yet stays, its launch with it, until it is shown.
        board.retain(|_, entry| entry.end.is_none() || entry.shown.is_none());
        views
    }
}

impl Hub {
    fn wake(&self, plugin: &str) {
        if let Some((_, wake)) = lock(&self.wakes).get(plugin) {
            // Full, a wake waits already, or the thread is in a call: it delivers after it.
            drop(wake.try_send(Work::Jobs));
        }
    }

    /// Asks the Runs lists' sync to look, without waiting.
    fn kick(&self) {
        self.kick.try_send(()).ok();
    }

    /// Job `job` of plugin `plugin`'s `load` is being launched, watched by `control`: false,
    /// and nothing noted, if the load was detached or the job is being stopped, when its worker
    /// must not start. Else its worker is counted from now on.
    fn launched(
        &self,
        plugin: &str,
        load: u64,
        detached: &AtomicBool,
        job: JobId,
        control: &JobControl,
    ) -> bool {
        let mut board = lock(&self.board);
        // A detach sets the flag before it looks at the board, which this holds: either it
        // sees this job launched, or this sees the flag.
        if detached.load(Ordering::Acquire) || control.is_cancelled() {
            return false;
        }
        let entry = board.entry(job).or_insert_with(|| Entry::new(plugin, load));
        entry.launched = Some(control.clone());
        self.workers.fetch_add(1, Ordering::AcqRel);
        true
    }

    /// Ends `load`'s jobs ([`JobsLoad::detach`]). Once only.
    fn detach(&self, load: u64, handle: &Load) {
        if handle.detached.swap(true, Ordering::AcqRel) {
            return;
        }
        lock(&self.live).loads.remove(&load);
        handle.jobs.detach();
        {
            let mut wakes = lock(&self.wakes);
            if wakes
                .get(&handle.plugin)
                .is_some_and(|(woken, _)| *woken == load)
            {
                wakes.remove(&handle.plugin);
            }
        }
        let mut board = lock(&self.board);
        for entry in board.values_mut() {
            if entry.load == load {
                entry.detach();
            }
        }
        drop(board);
        self.kick();
    }

    /// Job `job`'s worker ended as `end`: its run ends so if its load was detached, since none
    /// delivers it then.
    fn worker_ended(&self, job: JobId, end: JobEnd) {
        #[cfg(test)]
        lock(&self.worker_ends).push((job, end.clone()));
        let mut board = lock(&self.board);
        if let Some(entry) = board.get_mut(&job) {
            if entry.detached {
                entry.end.get_or_insert_with(|| end.clone());
            }
            entry.worker_end = Some(end);
        }
    }
}

/// One load of a plugin's jobs on the [`JobsHub`]: what its imports, its thread and the host
/// reach them by. Clones share it.
#[derive(Clone)]
pub(crate) struct JobsLoad {
    hub: JobsHub,
    load: u64,
    handle: Load,
}

impl JobsLoad {
    /// Has the queue wake `wake`, the plugin's thread, when one of its jobs ends.
    pub(crate) fn set_wake(&self, wake: SyncSender<Work>) {
        let mut wakes = lock(&self.hub.0.wakes);
        // Under the lock a detach takes to remove it.
        if self.handle.detached.load(Ordering::Acquire) {
            return;
        }
        wakes.insert(self.handle.plugin.clone(), (self.load, wake));
    }

    /// Its `kiln-plugin` jobs.
    fn jobs(&self) -> &Jobs {
        &self.handle.jobs
    }

    /// `jobs.start`: queues `spec`, noting what the Runs list shows of it.
    fn start(&self, spec: JobSpec) -> Result<JobId, Error> {
        let shown = Shown {
            title: spec.title.clone(),
            kind: match spec.kind {
                JobKind::Build => RunKind::Build,
                JobKind::Test => RunKind::Test,
                JobKind::Index | JobKind::Other => RunKind::Script,
            },
            cancellable: spec.cancellable,
            root: spec.root.clone(),
        };
        let job = self.jobs().start(spec)?;
        self.noted(job, shown);
        Ok(job)
    }

    /// Job `job`, just started, is shown as `shown`: perhaps launched already, and perhaps
    /// its load detached since.
    fn noted(&self, job: JobId, shown: Shown) {
        let mut board = lock(&self.hub.0.board);
        let plugin = &self.handle.plugin;
        let entry = (board.entry(job)).or_insert_with(|| Entry::new(plugin, self.load));
        entry.shown = Some(shown);
        entry.jobs = Some(self.jobs().clone());
        // Detached since it was queued: as the detach would have ended it, a worker launched
        // first still running.
        if self.handle.detached.load(Ordering::Acquire) {
            entry.detach();
        }
        drop(board);
        self.hub.0.kick();
    }

    /// The job that ended first and hasn't been delivered, taken out to hand to `job-finished`,
    /// its run told how it ended.
    pub(crate) fn take_ended(&self) -> Option<(JobId, Ended)> {
        let (job, ended) = self.jobs().take_ended()?;
        let mut board = lock(&self.hub.0.board);
        let plugin = &self.handle.plugin;
        let entry = (board.entry(job)).or_insert_with(|| Entry::new(plugin, self.load));
        entry.end.get_or_insert_with(|| JobEnd::of(&ended));
        Some((job, ended))
    }

    /// Ends this load's jobs, as unloading, reloading or disabling the plugin does: its running
    /// jobs are stopped as cancelled, its queued ones removed and its undelivered ends dropped.
    /// A queued job's run ends cancelled now, a running one's once its worker has ended. Once
    /// only.
    pub(crate) fn detach(&self) {
        self.hub.0.detach(self.load, &self.handle);
    }
}

/// What the store data of a worker is made from, for each job it runs.
pub(crate) type WorkerData = Arc<dyn Fn(&Launch) -> Data + Send + Sync>;

/// What a plugin's worker stores are made of: its component, linked as its interactive store
/// is, so compiled once, through the cache, and the data each runs against.
pub(crate) struct Workers {
    pub(crate) id: String,
    pub(crate) engine: PluginEngine,
    pub(crate) pre: InstancePre<Data>,
    pub(crate) world: kiln_plugin::compat::World,
    pub(crate) data: WorkerData,
}

impl Workers {
    /// Runs `launch` on a thread of its own, at lowered priority. If there is none, the launch
    /// drops unfinished, so the job ends failed.
    /// One the queue launches as its load is detached, or as it is being stopped, never starts:
    /// it ends cancelled.
    fn launch(
        self: &Arc<Self>,
        hub: &JobsHub,
        load: u64,
        detached: &AtomicBool,
        mut launch: Launch,
    ) {
        let job = launch.job;
        if !(hub.0).launched(&self.id, load, detached, job, &launch.control) {
            launch.finish(Ended::Cancelled);
            return;
        }
        let (workers, hub) = (Arc::clone(self), hub.clone());
        let id = &self.id;
        let counted = hub.clone();
        let spawned = thread::Builder::new().name(thread_name(id)).spawn(move || {
            kiln::core::platform::lower_thread_priority();
            let ended = workers.run(&mut launch);
            let end = JobEnd::of(&ended);
            launch.finish(ended);
            hub.0.worker_ended(job, end);
            hub.0.workers.fetch_sub(1, Ordering::AcqRel);
        });
        if let Err(error) = spawned {
            // The launch dropped unfinished, so the queue ended it failed.
            counted.0.workers.fetch_sub(1, Ordering::AcqRel);
            let end = JobEnd::Failed(jobs::THREAD_STOPPED.to_owned());
            counted.0.worker_ended(job, end);
            log::warn!("no thread for a job of plugin {id}: {error}");
        }
    }

    /// Runs `launch`'s job in a new worker store, without `init`, and returns how it ended. The
    /// epoch stops it at the end of a slice, at its wall time and past its cancel grace; one
    /// waiting on an async import then is dropped, and the imports that wait on their own wait
    /// only until then. The store drops as it returns.
    fn run(&self, launch: &mut Launch) -> Ended {
        let mut data = (self.data)(launch);
        let wall_ends = Instant::now().checked_add(launch.wall);
        data.worker = Some(Worker {
            control: launch.control.clone(),
            wall_ends,
            slice_ends: None,
            sliced: false,
        });
        let mut store = Store::new(self.engine.engine(), data);
        store.limiter(|data| &mut data.limiter);
        // Every time the epoch ticks over running wasm.
        store.epoch_deadline_callback(move |mut store| {
            let worker = store.data_mut().worker.as_mut().expect("a worker store");
            let now = Instant::now();
            if worker.wall_ends.is_some_and(|ends| now >= ends) {
                worker.control.stop(Stop::TimedOut);
            }
            if worker.slice_ends.is_some_and(|ends| now >= ends) {
                worker.sliced = true;
                return Ok(UpdateDeadline::Interrupt);
            }
            if worker.control.past_grace(now) {
                return Ok(UpdateDeadline::Interrupt);
            }
            Ok(UpdateDeadline::Continue(1))
        });
        // On entering the host, an import that waits is told when the job must end: its wall
        // time, or its grace once it is being stopped. A slice starts each time wasm runs again:
        // as the job starts, and on each return from an import. wasmtime's own calls into the
        // host (the epoch's check, `memory.grow`, the bulk memory instructions) enter no import,
        // so they don't renew it.
        store.call_hook(|mut store, hook| {
            let data = store.data_mut();
            let worker = data.worker.as_mut().expect("a worker store");
            match hook {
                CallHook::CallingHost => {
                    let ends = worker.control.ends(worker.wall_ends);
                    data.host.kiln.deadline = ends;
                    data.call_deadline.set(ends);
                    match ends {
                        Some(ends) => (data.host.kiln.program_runs)
                            .arm(ends.saturating_duration_since(Instant::now())),
                        None => data.host.kiln.program_runs.disarm(),
                    }
                }
                CallHook::ReturningFromHost if !mem::take(&mut data.imported) => {}
                CallHook::CallingWasm | CallHook::ReturningFromHost => {
                    worker.slice_ends = Instant::now().checked_add(jobs::SLICE);
                    // What the import set of the decorations or the status items is drawn.
                    if mem::take(&mut data.painted) {
                        data.paint.repaint();
                    }
                }
                CallHook::ReturningFromWasm => {}
            }
            Ok(())
        });
        store.set_epoch_deadline(1);
        let control = launch.control.clone();
        let ticking = ticking();
        // The call runs on this thread, its result lifted here too.
        let reserving = Reserving::set(launch.reservation());
        let returned = wasmtime_wasi::runtime::in_tokio(async {
            let store = &mut store;
            let mut run = pin!(async {
                let instance = self.pre.instantiate_async(&mut *store).await?;
                let interface = self.world.interface("job-runner");
                let run_job = (instance.get_export_index(&mut *store, None, &interface))
                    .and_then(|at| instance.get_export_index(&mut *store, Some(&at), "run-job"))
                    .ok_or_else(|| {
                        let export = tr!(
                            "workbench-extensions-plugin-no-export",
                            interface = interface.as_str(),
                            name = "run-job"
                        );
                        wasmtime::format_err!("{}", export)
                    })?;
                // The result is checked and charged as it is lifted, before it is copied out of
                // the worker's memory, and before the guest's post-return may reuse that memory.
                let run_job = instance.get_typed_func::<
                    (JobId, String, String, Option<String>),
                    (Result<KeptText, GuestError>,),
                >(&mut *store, run_job)?;
                let args = (
                    launch.job,
                    mem::take(&mut launch.name),
                    mem::take(&mut launch.args),
                    launch.root.clone(),
                );
                let (returned,) = run_job.call_async(&mut *store, args).await?;
                Ok::<_, wasmtime::Error>(returned)
            });
            poll_fn(|cx| {
                if let Poll::Ready(returned) = run.as_mut().poll(cx) {
                    return Poll::Ready(Some(returned));
                }
                let now = Instant::now();
                if wall_ends.is_some_and(|ends| now >= ends) {
                    control.stop(Stop::TimedOut);
                }
                if control.past_grace(now) {
                    return Poll::Ready(None);
                }
                wake_on_tick(cx.waker());
                Poll::Pending
            })
            .await
        });
        drop(reserving);
        drop(ticking);
        // An import that returned late doesn't make the job's wall time longer.
        if wall_ends.is_some_and(|ends| Instant::now() >= ends) {
            control.stop(Stop::TimedOut);
        }
        let data = store.data_mut();
        if mem::take(&mut data.painted) {
            data.paint.repaint();
        }
        let sliced = data.worker.as_ref().is_some_and(|worker| worker.sliced);
        match (control.stopping(), returned) {
            (Some(Stop::TimedOut), _) => Ended::TimedOut,
            (Some(Stop::Cancelled), _) => Ended::Cancelled,
            _ if sliced => Ended::failed(jobs::SLICE_OVERRUN),
            (None, Some(Ok(result))) => kept(result),
            _ if data.limiter.refused() => Ended::failed(jobs::OUT_OF_MEMORY),
            (None, Some(Err(error))) => {
                // Its room is charged before it is written: with no room, it is dropped unwritten.
                let reserved = (launch.reservation())
                    .is_some_and(|reservation| reservation.reserve(jobs::MAX_DIAGNOSTIC_BYTES));
                if reserved {
                    Ended::failed(diagnostic(&error))
                } else {
                    Ended::failed(jobs::RESULT_DROPPED)
                }
            }
            (None, None) => Ended::failed(jobs::THREAD_STOPPED),
        }
    }
}

/// A worker store's own state: the job it runs, its wall time and its slice.
pub(crate) struct Worker {
    control: JobControl,
    wall_ends: Option<Instant>,
    /// When the running slice of wasm ends, unless it enters an import first.
    slice_ends: Option<Instant>,
    /// Whether the epoch stopped it at the end of a slice.
    sliced: bool,
}

/// `error`'s text, at most [`jobs::MAX_DIAGNOSTIC_BYTES`], cut at a character's boundary.
fn diagnostic(error: &wasmtime::Error) -> String {
    let mut text = format!("{error:#}");
    if text.len() > jobs::MAX_DIAGNOSTIC_BYTES {
        let mut cut = jobs::MAX_DIAGNOSTIC_BYTES;
        while !text.is_char_boundary(cut) {
            cut -= 1;
        }
        text.truncate(cut);
        text.shrink_to_fit();
    }
    text
}

/// `kiln:extension/types.error` as `run-job` returns it, its message lifted as [`KeptText`].
#[derive(ComponentType, Lift)]
#[component(variant)]
enum GuestError {
    #[component(name = "unimplemented")]
    Unimplemented(KeptText),
    #[component(name = "denied")]
    Denied(KeptText),
    #[component(name = "not-found")]
    NotFound(KeptText),
    #[component(name = "invalid-argument")]
    InvalidArgument(KeptText),
    #[component(name = "conflict")]
    Conflict(KeptText),
    #[component(name = "failed")]
    Failed(KeptText),
}

/// A string `run-job` returned, as the host keeps it: checked against
/// [`jobs::MAX_RESULT_BYTES`] and charged to the plugin's undelivered results (through the
/// worker thread's [`RESERVATION`]) while it is lifted, and only then copied out of the worker's
/// memory, as kiln-plugin's runner keeps it. A plugin that runs jobs lifts its strings as UTF-8
/// ([`jobs::lifts_utf8`]).
enum KeptText {
    Kept(String),
    /// Past [`jobs::MAX_RESULT_BYTES`]: nothing copied.
    TooLarge,
    /// It wouldn't fit the plugin's undelivered results: nothing copied.
    Dropped,
}

// What charges the result `run-job` returns, while it runs on this worker's thread.
thread_local! {
    static RESERVATION: RefCell<Option<Reservation>> = const { RefCell::new(None) };
}

/// Holds this thread's [`RESERVATION`] until dropped.
struct Reserving;

impl Reserving {
    fn set(reservation: Option<Reservation>) -> Self {
        RESERVATION.set(reservation);
        Self
    }
}

impl Drop for Reserving {
    fn drop(&mut self) {
        RESERVATION.set(None);
    }
}

impl KeptText {
    /// The string of `len` bytes at `ptr` in `memory`, kept as [`KeptText`] says.
    fn keep(memory: &[u8], ptr: u32, len: u32) -> wasmtime::Result<Self> {
        let out_of_bounds = || wasmtime::Error::msg(jobs::OUT_OF_BOUNDS);
        let (ptr, len) = (usize::try_from(ptr)?, usize::try_from(len)?);
        let bytes = (memory.get(ptr..))
            .and_then(|rest| rest.get(..len))
            .ok_or_else(out_of_bounds)?;
        if len > jobs::MAX_RESULT_BYTES {
            return Ok(Self::TooLarge);
        }
        let reserved = RESERVATION.with_borrow(|reservation| {
            (reservation.as_ref()).is_some_and(|reservation| reservation.reserve(len))
        });
        if !reserved {
            return Ok(Self::Dropped);
        }
        Ok(Self::Kept(std::str::from_utf8(bytes)?.to_owned()))
    }
}

// SAFETY: a `string`, lowered as a pointer and a length, as `WasmStr` is.
unsafe impl ComponentType for KeptText {
    type Lower = <WasmStr as ComponentType>::Lower;

    const ABI: wasmtime::component::__internal::CanonicalAbiInfo = <WasmStr as ComponentType>::ABI;

    fn typecheck(
        ty: &wasmtime::component::__internal::InterfaceType,
        types: &wasmtime::component::__internal::InstanceType<'_>,
    ) -> wasmtime::Result<()> {
        <WasmStr as ComponentType>::typecheck(ty, types)
    }
}

// SAFETY: reads the pointer and length as the canonical ABI lays a `string` out, flat or in
// memory, and the bytes only within the memory's bounds.
unsafe impl Lift for KeptText {
    fn linear_lift_from_flat(
        cx: &mut wasmtime::component::__internal::LiftContext<'_>,
        _ty: wasmtime::component::__internal::InterfaceType,
        src: &Self::Lower,
    ) -> wasmtime::Result<Self> {
        Self::keep(cx.memory(), src[0].get_u32(), src[1].get_u32())
    }

    fn linear_lift_from_memory(
        cx: &mut wasmtime::component::__internal::LiftContext<'_>,
        _ty: wasmtime::component::__internal::InterfaceType,
        bytes: &[u8],
    ) -> wasmtime::Result<Self> {
        let word = |at: usize| {
            (bytes.get(at..at + 4))
                .and_then(|word| word.try_into().ok())
                .map(u32::from_le_bytes)
                .ok_or_else(|| wasmtime::Error::msg(jobs::OUT_OF_BOUNDS))
        };
        Self::keep(cx.memory(), word(0)?, word(4)?)
    }
}

/// One of [`Error`]'s cases, made from its message.
type MakeError = fn(String) -> Error;

/// The job's end as `run-job` returned `result`.
fn kept(result: Result<KeptText, GuestError>) -> Ended {
    let (text, error): (_, Option<MakeError>) = match result {
        Ok(text) => (text, None),
        Err(GuestError::Unimplemented(text)) => (text, Some(Error::Unimplemented)),
        Err(GuestError::Denied(text)) => (text, Some(Error::Denied)),
        Err(GuestError::NotFound(text)) => (text, Some(Error::NotFound)),
        Err(GuestError::InvalidArgument(text)) => (text, Some(Error::InvalidArgument)),
        Err(GuestError::Conflict(text)) => (text, Some(Error::Conflict)),
        Err(GuestError::Failed(text)) => (text, Some(Error::Failed)),
    };
    match text {
        KeptText::Kept(text) => Ended::Finished(match error {
            None => Ok(text),
            Some(error) => Err(error(text)),
        }),
        KeptText::TooLarge => Ended::failed(jobs::RESULT_TOO_LARGE),
        KeptText::Dropped => Ended::failed(jobs::RESULT_DROPPED),
    }
}

// The imports. kiln-plugin's `jobs` slice answers only for its own runner's loads; this host's
// are these, against the store's [`JobsLoad`].

/// Adds the `jobs` and `job-context` imports to `linker`, the `jobs` ones replacing
/// kiln-plugin's with the store's own [`JobsLoad`]'s: for a plugin with the `jobs` capability.
/// In a worker store, `jobs.start` and `jobs.cancel` are denied, as kiln-plugin's are.
pub(crate) fn link(linker: &mut Linker<Data>) -> wasmtime::Result<()> {
    kiln_plugin::bindings::add_jobs_to_linker(linker, |data| {
        data.imported = true;
        &mut data.host.kiln
    })?;
    let interface = kiln_plugin::compat::World::CURRENT.interface("jobs");
    linker.allow_shadowing(true);
    let mut jobs = linker.instance(&interface)?;
    jobs.func_wrap(
        "start",
        |mut store: wasmtime::StoreContextMut<'_, Data>, (spec,): (JobSpec,)| {
            let data = store.data_mut();
            data.imported = true;
            let started = match (&data.worker, &data.jobs) {
                (Some(_), _) => Err(Error::Denied(jobs::NOT_IN_A_JOB.to_owned())),
                (None, Some(jobs)) => jobs.start(spec),
                (None, None) => kiln_plugin::bindings::unimplemented("kiln:extension/jobs.start"),
            };
            Ok((started,))
        },
    )?;
    jobs.func_wrap(
        "cancel",
        |mut store: wasmtime::StoreContextMut<'_, Data>, (job,): (JobId,)| {
            let data = store.data_mut();
            data.imported = true;
            let cancelled = match (&data.worker, &data.jobs) {
                (Some(_), _) => Err(Error::Denied(jobs::NOT_IN_A_JOB.to_owned())),
                (None, Some(jobs)) => jobs.jobs().cancel(job),
                (None, None) => kiln_plugin::bindings::unimplemented("kiln:extension/jobs.cancel"),
            };
            Ok((cancelled,))
        },
    )?;
    jobs.func_wrap(
        "state",
        |mut store: wasmtime::StoreContextMut<'_, Data>, (job,): (JobId,)| {
            let data = store.data_mut();
            data.imported = true;
            let state = match &data.jobs {
                Some(jobs) => jobs.jobs().state(job),
                None => JobStatus::Unknown,
            };
            Ok((state,))
        },
    )?;
    linker.allow_shadowing(false);
    Ok(())
}

// The Runs lists.

/// What a job's run should say.
#[derive(Clone, Debug)]
enum State {
    Queued(Waiting),
    Running(Progress),
    Ended(JobEnd),
}

/// A job, as the Runs lists show it.
struct View {
    job: JobId,
    plugin: String,
    shown: Shown,
    /// Whether its worker was launched: it started.
    launched: bool,
    state: State,
}

/// Each job's run in each window's Runs list.
#[derive(Default)]
struct JobRuns {
    runs: HashMap<JobId, Vec<(WeakEntity<Runs>, RunId)>>,
}

impl Global for JobRuns {}

/// Keeps the Runs lists up to date with `hub`'s jobs, every [`POLL`] while there are any: woken
/// as a job starts, whether or not its plugin's `init` has returned, and as a load detaches, so
/// a failed load's jobs are drained too. Started once, with the host.
pub(crate) fn serve(hub: &JobsHub, cx: &mut App) {
    let kicked = hub.0.kicked.clone();
    cx.spawn(async move |cx| {
        while kicked.recv().await.is_ok() {
            loop {
                cx.background_executor().timer(POLL).await;
                if !cx.update(sync) {
                    break;
                }
            }
        }
    })
    .detach();
}

/// Brings each window's Runs list up to date with the jobs: true while there are jobs to
/// watch, else the sync sleeps until woken.
pub(crate) fn sync(cx: &mut App) -> bool {
    let Some(hub) = cx.try_global::<PluginHost>().map(|host| host.jobs.clone()) else {
        return false;
    };
    let views = hub.take_views();
    let windows = windows(cx);
    let mut runs = mem::take(&mut cx.default_global::<JobRuns>().runs);
    for view in &views {
        let shown = runs.entry(view.job).or_default();
        shown.retain(|(runs, _)| runs.upgrade().is_some());
        // A window it isn't in yet: one opened since, or the job is new.
        for (entity, root) in &windows {
            let fits = match &view.shown.root {
                None => true,
                Some(job_root) => root
                    .as_deref()
                    .is_some_and(|root| same_root(root, job_root)),
            };
            let listed = shown.iter().any(|(runs, _)| runs == &entity.downgrade());
            if fits && !listed {
                let run = entity.update(cx, |runs, cx| {
                    let owner = RunOwner::Plugin {
                        id: view.plugin.clone(),
                        job: view.job,
                    };
                    let (kind, title) = (view.shown.kind, view.shown.title.clone());
                    runs.queue(
                        kind,
                        title,
                        owner,
                        view.shown.cancellable,
                        Waiting::Concurrency,
                        cx,
                    )
                });
                shown.push((entity.downgrade(), run));
            }
        }
        for (entity, run) in shown.iter() {
            if let Some(entity) = entity.upgrade() {
                entity.update(cx, |runs, cx| apply(runs, *run, view, cx));
            }
        }
    }
    for view in &views {
        if matches!(view.state, State::Ended(_)) {
            runs.remove(&view.job);
        }
    }
    let watching = cx.default_global::<JobRuns>();
    watching.runs = runs;
    hub.busy() || !watching.runs.is_empty()
}

/// Brings `run` up to date with `view`, changing nothing that hasn't changed.
fn apply(runs: &mut Runs, run: RunId, view: &View, cx: &mut gpui::Context<Runs>) {
    let Some(status) = runs.get(run).map(|run| run.status) else {
        return;
    };
    match &view.state {
        State::Queued(waiting) => {
            runs.wait(run, *waiting, cx);
        }
        State::Running(progress) => {
            if status.is_queued() {
                runs.start(run, cx);
            }
            let summary = summary_of(progress);
            let fraction = progress.fraction;
            let same = runs.get(run).is_some_and(|run| {
                run.summary == summary
                    && run.status
                        == RunStatus::Running {
                            progress: fraction.map(|p| p.clamp(0., 1.)),
                        }
            });
            if !same {
                runs.update(run, fraction, summary, cx);
            }
        }
        State::Ended(end) => {
            if status.is_queued() && view.launched {
                runs.start(run, cx);
            }
            match end {
                JobEnd::Passed => runs.finish(run, RunEnd::Passed, None, cx),
                JobEnd::Failed(message) => {
                    let summary = RunSummary::Text(crate::runs::cap_text(message.clone()));
                    runs.finish(run, RunEnd::Failed { code: None }, Some(summary), cx)
                }
                JobEnd::Cancelled => runs.finish(run, RunEnd::Cancelled, None, cx),
                JobEnd::TimedOut => runs.finish_because(run, None, FailReason::TimedOut, None, cx),
            };
        }
    }
}

/// What a running job's progress says, as its run's summary: its message as text, cut to
/// [`crate::runs::MAX_TEXT_CHARS`], whatever the plugin reported.
pub(crate) fn summary_of(progress: &Progress) -> RunSummary {
    match &progress.message {
        Some(message) => RunSummary::Text(crate::runs::cap_text(message.clone())),
        None => RunSummary::None,
    }
}

/// Each window's Runs, and its root as a guest path.
fn windows(cx: &App) -> Vec<(Entity<Runs>, Option<String>)> {
    cx.windows()
        .into_iter()
        .filter_map(|window| window.downcast::<Workbench>())
        .filter_map(|window| window.read(cx).ok())
        .map(|workbench| {
            let root = (workbench.root())
                .and_then(Path::to_str)
                .and_then(|root| root_to_guest(root, Rules::HOST).ok());
            (workbench.runs.clone(), root)
        })
        .collect()
}

/// Whether guest paths `a` and `b` name one root.
fn same_root(a: &str, b: &str) -> bool {
    a.trim_end_matches('/') == b.trim_end_matches('/')
}

/// Asks plugin `plugin` to cancel its job `job`, as the Runs list's cancel does: queued, it
/// never starts; running, it is stopped. False if nothing was asked: the job may not be
/// cancelled, has ended, or is gone.
pub(crate) fn cancel(plugin: &str, job: u64, cx: &App) -> bool {
    (cx.try_global::<PluginHost>()).is_some_and(|host| host.jobs.cancel(plugin, job))
}

/// As the app quits: no plugin's load attaches jobs from now on, every plugin's jobs are
/// stopped as cancelled and its queued ones removed, a plugin's still in its `init` too, then
/// the quit waits, at most [`QUIT_WAIT`], for their workers to end.
pub(crate) fn quit(cx: &mut App) -> impl Future<Output = ()> + use<> {
    let hub = cx.try_global::<PluginHost>().map(|host| host.jobs.clone());
    if let Some(hub) = &hub {
        hub.close();
    }
    let executor = cx.background_executor().clone();
    async move {
        let Some(hub) = hub else {
            return;
        };
        let until = Instant::now() + QUIT_WAIT;
        while !hub.idle() && Instant::now() < until {
            executor.timer(Duration::from_millis(10)).await;
        }
    }
}

#[cfg(test)]
#[path = "plugin_jobs_tests.rs"]
mod plugin_jobs_tests;
