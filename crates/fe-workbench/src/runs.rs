//! The runs one workbench knows: builds, tests and scripts, and later agent
//! tasks, in one list of one shape (`docs/design/design-system.md`, "Runs:
//! builds, tests and agent tasks in one list"). Each has a kind, a title, a
//! status, a one-line summary and the times that give its age; a running one
//! may know how far along it is.
//!
//! Titles and summaries are data: whatever lists them formats the text. Task
//! runs come here from [`crate::task_run`]; nothing makes an
//! [`RunKind::Agent`] run yet.
//!
//! A plugin's background jobs come here from [`crate::plugin_host::plugin_jobs`]
//! (`docs/decisions/dqsjfernkt-plugins-stage-3.md` §3.5): each is owned by
//! its plugin ([`RunOwner::Plugin`]), so [`Runs::cancel`] reaches the job. A
//! job waits [`RunStatus::Queued`] until it may start, with `queued_at` set
//! and `started` none; [`Runs::start`] sets `started`, and one cancelled
//! before then ends with `started` still none. What a job reports of itself
//! is a [`RunSummary::Text`], and one past its wall time fails with
//! [`FailReason::TimedOut`].
//!
//! Two of those differ from §3.5's sketch, as the lead decided for T1704:
//! the reason a run failed is the run's own ([`Run::reason`], set as it
//! ends), so [`RunStatus::Failed`] keeps its shape; and a run the workbench
//! begins ([`Runs::begin`]) is [`RunOwner::Host`]'s, cancelled by closing
//! its terminal tab as before, while only a plugin's job names its owner.

use std::{collections::VecDeque, time::Instant};

use gpui::{Context, EventEmitter};

/// How many finished runs are kept: those that finished longest ago go
/// first. Running and queued ones are never dropped.
pub const MAX_FINISHED: usize = 100;

/// One run's identity, unique in its [`Runs`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RunId(u64);

/// What a run does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunKind {
    Build,
    Test,
    /// Anything else a task runs.
    Script,
    /// An agent's task, for the lanes.
    Agent,
    /// A debug session ([`crate::debug_runs`]). The operations list shows
    /// it by its own entry, not as a running run.
    Debug,
}

/// What a queued run waits for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Waiting {
    /// Another job to end: as many run as may.
    Concurrency,
    /// Memory: the jobs running hold as much as all may together.
    Memory,
}

/// Where a run is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RunStatus {
    /// Not started yet, waiting for what `waiting` says.
    Queued {
        waiting: Waiting,
    },
    /// Still going, `progress` of the way there, from 0 to 1, if known.
    Running {
        progress: Option<f32>,
    },
    Passed,
    /// Ended badly, with its exit code if it had one: none when a signal
    /// ended it or it couldn't start.
    Failed {
        code: Option<i32>,
    },
    /// Stopped before it ended.
    Cancelled,
}

impl RunStatus {
    pub fn is_running(&self) -> bool {
        matches!(self, Self::Running { .. })
    }

    pub fn is_queued(&self) -> bool {
        matches!(self, Self::Queued { .. })
    }

    /// Whether it hasn't ended: queued or running.
    pub fn is_active(&self) -> bool {
        self.is_queued() || self.is_running()
    }
}

/// Why a run failed, beyond its exit code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailReason {
    /// It ran past the time it was allowed, and was stopped.
    TimedOut,
}

/// Who a run belongs to: who [`Runs::cancel`] asks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunOwner {
    /// The workbench: a task's run, cancelled by closing its terminal tab.
    Host,
    /// Background job `job` of plugin `id`, cancelled through `jobs.cancel`.
    Plugin { id: String, job: u64 },
}

/// How a run ended, for [`Runs::finish`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunEnd {
    Passed,
    Failed { code: Option<i32> },
    Cancelled,
}

impl From<RunEnd> for RunStatus {
    fn from(end: RunEnd) -> Self {
        match end {
            RunEnd::Passed => Self::Passed,
            RunEnd::Failed { code } => Self::Failed { code },
            RunEnd::Cancelled => Self::Cancelled,
        }
    }
}

/// What a run's one line says, as data.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum RunSummary {
    /// Nothing yet.
    #[default]
    None,
    /// It exited with this code.
    Exit(i32),
    /// Tests counted so far: `total` once it's known.
    Tests {
        passed: usize,
        failed: usize,
        total: Option<usize>,
    },
    /// A build's crates built so far, and what they said.
    Build {
        crates: usize,
        warnings: usize,
        errors: usize,
    },
    /// A debug session that hasn't ended.
    Debug(DebugPhase),
    /// What the run says of itself, as plain text: a plugin job's progress
    /// message, at most [`MAX_TEXT_CHARS`] characters. Shown as data, never
    /// as markup.
    Text(String),
}

/// The most characters a [`RunSummary::Text`] holds: [`Runs::update`] cuts
/// a longer one.
pub const MAX_TEXT_CHARS: usize = 200;

impl RunSummary {
    /// `self`, its text cut to [`MAX_TEXT_CHARS`] characters if it's a
    /// [`RunSummary::Text`].
    fn capped(self) -> Self {
        match self {
            Self::Text(text) => Self::Text(cap_text(text)),
            summary => summary,
        }
    }
}

/// `text`, cut to its first [`MAX_TEXT_CHARS`] characters, copied when cut so
/// a long text's allocation isn't kept.
pub(crate) fn cap_text(text: String) -> String {
    match text.char_indices().nth(MAX_TEXT_CHARS) {
        Some((cut, _)) => text[..cut].to_owned(),
        None => text,
    }
}

/// Where a debug session is, as its run's summary says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DebugPhase {
    /// The adapter is starting, or the session being configured.
    Starting,
    /// The program runs.
    Running,
    /// The program is stopped, at a breakpoint or a step.
    Stopped,
}

/// One build, test run, script or agent task.
#[derive(Clone, Debug, PartialEq)]
pub struct Run {
    pub id: RunId,
    pub kind: RunKind,
    /// What it runs, as a task's label: shown in mono.
    pub title: String,
    pub status: RunStatus,
    pub summary: RunSummary,
    /// Who it belongs to: who cancels it.
    pub owner: RunOwner,
    /// Whether [`Runs::cancel`] may cancel it.
    pub cancellable: bool,
    /// When it was queued, if it waited to start: none for a run that
    /// began running.
    pub queued_at: Option<Instant>,
    /// When it began running: none while it's queued, and for one
    /// cancelled before it started.
    pub started: Option<Instant>,
    /// When it stopped running: none while it runs.
    pub finished: Option<Instant>,
    /// A line of its own to show in place of the summary: why it failed.
    /// Already in the user's language.
    pub note: Option<String>,
    /// Why it failed, beyond its exit code: set as it ends.
    pub reason: Option<FailReason>,
}

impl Run {
    /// How long it's been since it stopped, or, while it runs, since it
    /// started, or, while it's queued, since it was queued.
    pub fn age(&self, now: Instant) -> std::time::Duration {
        now.saturating_duration_since(self.since())
    }

    /// What its age counts from: when it ended, else when it started, else
    /// when it was queued.
    pub fn since(&self) -> Instant {
        (self.finished.or(self.started).or(self.queued_at))
            .expect("a run is queued or started when it's made")
    }
}

/// What [`Runs`] tells those that observe it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunsEvent {
    /// This run began, changed or finished: it may have pushed the oldest
    /// finished ones out.
    Changed(RunId),
}

/// The runs, oldest first, kept by the workbench.
#[derive(Debug, Default)]
pub struct Runs {
    runs: VecDeque<Run>,
    /// The finished runs kept, in the order they finished.
    finished: VecDeque<RunId>,
    next_id: u64,
}

impl EventEmitter<RunsEvent> for Runs {}

impl Runs {
    /// Adds a running run of `kind` named `title`, with no progress known,
    /// the workbench's.
    pub fn begin(
        &mut self,
        kind: RunKind,
        title: impl Into<String>,
        cx: &mut Context<Self>,
    ) -> RunId {
        let id = RunId(self.next_id);
        self.next_id += 1;
        self.runs.push_back(Run {
            id,
            kind,
            title: title.into(),
            status: RunStatus::Running { progress: None },
            summary: RunSummary::None,
            owner: RunOwner::Host,
            cancellable: false,
            queued_at: None,
            started: Some(Instant::now()),
            finished: None,
            note: None,
            reason: None,
        });
        self.changed(id, cx);
        id
    }

    /// Adds a queued run of `kind` named `title`, waiting for what
    /// `waiting` says, owned by `owner`, which [`Self::cancel`] may cancel
    /// if `cancellable`. It's queued now and hasn't started.
    pub fn queue(
        &mut self,
        kind: RunKind,
        title: impl Into<String>,
        owner: RunOwner,
        cancellable: bool,
        waiting: Waiting,
        cx: &mut Context<Self>,
    ) -> RunId {
        let id = RunId(self.next_id);
        self.next_id += 1;
        self.runs.push_back(Run {
            id,
            kind,
            title: title.into(),
            status: RunStatus::Queued { waiting },
            summary: RunSummary::None,
            owner,
            cancellable,
            queued_at: Some(Instant::now()),
            started: None,
            finished: None,
            note: None,
            reason: None,
        });
        self.changed(id, cx);
        id
    }

    /// Says what the queued run `id` waits for now. One not queued, or
    /// gone, is left as it was: false then.
    pub fn wait(&mut self, id: RunId, waiting: Waiting, cx: &mut Context<Self>) -> bool {
        let Some(run) = self.runs.iter_mut().find(|run| run.id == id) else {
            return false;
        };
        if !run.status.is_queued() {
            return false;
        }
        if run.status != (RunStatus::Queued { waiting }) {
            run.status = RunStatus::Queued { waiting };
            self.changed(id, cx);
        }
        true
    }

    /// Starts the queued run `id`: it's running from now, with no progress
    /// known. One not queued, or gone, is left as it was: false then.
    pub fn start(&mut self, id: RunId, cx: &mut Context<Self>) -> bool {
        let Some(run) = self.runs.iter_mut().find(|run| run.id == id) else {
            return false;
        };
        if !run.status.is_queued() {
            return false;
        }
        run.status = RunStatus::Running { progress: None };
        run.started = Some(Instant::now());
        self.changed(id, cx);
        true
    }

    /// Asks the owner of run `id` to cancel it, if it hasn't ended and may
    /// be cancelled: a plugin's job through `jobs.cancel`, queued or
    /// running. It ends once its owner says it has. False if nothing was
    /// asked.
    pub fn cancel(&mut self, id: RunId, cx: &mut Context<Self>) -> bool {
        let Some(run) = self.get(id) else {
            return false;
        };
        if !(run.cancellable && run.status.is_active()) {
            return false;
        }
        match run.owner.clone() {
            RunOwner::Host => false,
            RunOwner::Plugin { id, job } => crate::plugin_host::plugin_jobs::cancel(&id, job, cx),
        }
    }

    /// Says how far along the running run `id` is, `progress` clamped to 0
    /// to 1, and what it's done so far, a [`RunSummary::Text`] cut to
    /// [`MAX_TEXT_CHARS`]. One that's queued or finished, or gone, is left
    /// as it was: false then.
    pub fn update(
        &mut self,
        id: RunId,
        progress: Option<f32>,
        summary: RunSummary,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(run) = self.running_mut(id) else {
            return false;
        };
        let progress = progress.filter(|p| !p.is_nan()).map(|p| p.clamp(0.0, 1.0));
        run.status = RunStatus::Running { progress };
        run.summary = summary.capped();
        self.changed(id, cx);
        true
    }

    /// Ends the running or queued run `id` as `end` says, with `summary` if
    /// it's given, else what it said last: a queued one ends without having
    /// started. One already finished, or gone, is left as it was: false
    /// then. The runs that finished longest ago, past [`MAX_FINISHED`], go.
    pub fn finish(
        &mut self,
        id: RunId,
        end: RunEnd,
        summary: Option<RunSummary>,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(run) = self.active_mut(id) else {
            return false;
        };
        run.status = end.into();
        run.finished = Some(Instant::now());
        if let Some(summary) = summary {
            run.summary = summary.capped();
        }
        self.finished.push_back(id);
        self.evict();
        self.changed(id, cx);
        true
    }

    /// As [`finish`](Self::finish), with `note` as the line it shows.
    pub fn finish_with_note(
        &mut self,
        id: RunId,
        end: RunEnd,
        summary: Option<RunSummary>,
        note: String,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.finish(id, end, summary, cx) {
            return false;
        }
        if let Some(run) = self.runs.iter_mut().find(|run| run.id == id) {
            run.note = Some(note);
        }
        self.changed(id, cx);
        true
    }

    /// As [`finish`](Self::finish), failed for `reason`.
    pub fn finish_because(
        &mut self,
        id: RunId,
        code: Option<i32>,
        reason: FailReason,
        summary: Option<RunSummary>,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.finish(id, RunEnd::Failed { code }, summary, cx) {
            return false;
        }
        if let Some(run) = self.runs.iter_mut().find(|run| run.id == id) {
            run.reason = Some(reason);
        }
        self.changed(id, cx);
        true
    }

    /// The runs, newest first.
    pub fn list(&self) -> impl Iterator<Item = &Run> {
        self.runs.iter().rev()
    }

    /// The run `id`, if it's kept.
    pub fn get(&self, id: RunId) -> Option<&Run> {
        self.runs.iter().find(|run| run.id == id)
    }

    fn running_mut(&mut self, id: RunId) -> Option<&mut Run> {
        (self.runs.iter_mut()).find(|run| run.id == id && run.status.is_running())
    }

    fn active_mut(&mut self, id: RunId) -> Option<&mut Run> {
        (self.runs.iter_mut()).find(|run| run.id == id && run.status.is_active())
    }

    /// Drops the runs that finished longest ago, past [`MAX_FINISHED`].
    fn evict(&mut self) {
        if self.finished.len() <= MAX_FINISHED {
            return;
        }
        let excess = self.finished.len() - MAX_FINISHED;
        let gone: Vec<RunId> = self.finished.drain(..excess).collect();
        self.runs.retain(|run| !gone.contains(&run.id));
    }

    fn changed(&mut self, id: RunId, cx: &mut Context<Self>) {
        cx.emit(RunsEvent::Changed(id));
        cx.notify();
    }
}
