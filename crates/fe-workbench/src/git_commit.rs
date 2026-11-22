//! Committing the staged changes from the Changes panel's message box (see
//! `git_panel`): `git.commit` and `git.commit_amend`, routed here by
//! `git_commands`, the box's button and `git.commit_box.commit`, and the
//! host commands typed in the box.
//!
//! The workbench makes the box when the panel's first drawn, or a commit
//! command needs it: an editor over a path-less buffer in the editors'
//! theme, soft-wrapped, registered with [`Workbench::own_editor`] so its
//! host commands come here. `git.commit` (`:Git commit`, `cc` in the panel)
//! shows the panel and focuses the box; `git.commit_amend` (`:Git commit
//! --amend`, `ca`) does so with Amend checked. Where the panel has found
//! no repository they answer so.
//!
//! A commit runs [`Repo::commit`] on the background executor, in the
//! repository containing the root, in its turn after the staging asked
//! before it (`git_stage`'s queue), the button saying "Committing…". When
//! it's made, the box empties, Amend is unchecked and the workbench's git
//! state is read again. With nothing staged it asks, as VS Code's smart
//! commit does, "There are no staged changes to commit.", and "Stage All
//! and Commit" stages every change and commits. A hook that fails is a
//! warning naming it with what it printed, the message staying in the box;
//! git failing otherwise is "Git: commit failed" with its output, and an
//! empty message turns the box's placeholder to a warning.
//!
//! In the box, vim's `:wq`, `:x` and `ZZ` commit; `:w` keeps the message
//! (`:w {path}` and `:sav` aren't supported);
//! `:q!` and `ZQ` empty the box, as fugitive's abort does, and focus the
//! active editor again; `:q` focuses it if the box is empty, and answers
//! `E37` if not. `:Git …` runs as in any editor; `:e`, `:wa` and `:qa` run
//! in the active editor.
//!
//! Draft from diff, the box's button and `git.draft_commit_message`, asks
//! the chat's provider (`chat_panel::provider`) for a message for the
//! staged changes ([`fe_chat::run_commit_message`]): each staged file's
//! patch as `git diff --cached` writes it ([`Repo::staged_patch`]), read
//! on the background executor, with a binary file, one over the chat's
//! 1 MiB, and one the chat never reads (`chat_context::refusal`) left out.
//! The reply fills an empty box as one undo step; over a message already
//! in the box it asks first, showing the draft. A subject over 72
//! characters is noted. The button is disabled, its tooltip saying why,
//! while nothing is staged, the folder isn't trusted or the chat has no
//! key, disabled while a commit runs, and says "Drafting…" while a draft
//! is asked for. The command shows the panel and focuses the box, as
//! `git.commit` does, and says why in a toast when it can't run.
//!
//! A draft lives with the box until it's in it: its read, its request and
//! its question are one task the box holds (`GitPanel::set_drafting`),
//! referring to the box only weakly. Untrusting the folder, changing the
//! root, or starting a commit drops it, cancelling the request or
//! forgetting the question's answer; the read done, the folder's trust and
//! the chat's key are checked again before anything is sent.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use fe_buffer::Buffer;
use fe_chat::{
    ChatProvider, ChatSettings, DraftError, SUBJECT_LIMIT, StagedChange, StagedDiff, StagedFile,
    run_commit_message,
};
use fe_editor::EditorModel;
use fe_git::{ChangeKind, CommitError, GitError, Repo};
use fe_gpui::EditorView;
use fe_vim::HostCommand;
use futures::channel::oneshot;
use gpui::{
    App, AppContext as _, AsyncWindowContext, Context, Entity, Focusable as _, PromptButton,
    PromptLevel, SharedString, Task, WeakEntity, Window,
};
use kiln_l10n::tr;

use super::{
    Workbench,
    git_commands::{self, GitCommand, GitOp, Owner},
    git_stage,
};
use crate::{
    GitPanel, RootChanged, TrustChanged,
    chat_panel::{self, chat_context},
    dock::DockOp,
    git::GitState,
    git_panel::{CommitRequest, set_box_text},
    workbench::toast::{self, Toast},
};

/// What `:w` in the box answers.
fn kept() -> SharedString {
    tr!("workbench-changes-message-kept")
}

/// What `:q` on a box with a message answers.
fn e37() -> SharedString {
    tr!("workbench-changes-e37")
}

/// What a diff command typed in the box answers.
fn e99() -> SharedString {
    tr!("workbench-changes-e99")
}

/// What a command the box has no use for answers.
fn not_in_the_box() -> SharedString {
    tr!("workbench-changes-not-in-the-box")
}

/// What committing outside a repository says.
fn no_repo() -> SharedString {
    tr!("workbench-changes-no-repository")
}

/// The button that stages everything and commits.
pub(crate) fn stage_all_and_commit() -> SharedString {
    tr!("workbench-changes-stage-all-and-commit")
}

/// The most of a failed hook's output its warning shows.
const HOOK_OUTPUT: usize = 2000;

/// Runs `op`, a commit command: shows the Changes panel and focuses the
/// box, with Amend checked for `git.commit_amend`. Outside a repository it
/// answers so.
pub(super) fn run(
    workbench: &mut Workbench,
    op: GitOp,
    window: &mut Window,
    cx: &mut Context<Workbench>,
) {
    let changes = workbench.dock.read(cx).changes().clone();
    if changes.read(cx).not_a_repository() {
        return workbench.fail_git_op(&op, no_repo().into(), cx);
    }
    let amend = op.command == GitCommand::CommitAmend;
    workbench.focus_commit_box(amend, window, cx);
}

/// Runs `command`, typed in `editor`, the commit box.
pub(super) fn host_command(
    workbench: &mut Workbench,
    editor: Entity<EditorView>,
    command: HostCommand,
    window: &mut Window,
    cx: &mut Context<Workbench>,
) {
    let message = |editor: &Entity<EditorView>, text: &str, cx: &mut Context<Workbench>| {
        editor.update(cx, |editor, cx| editor.set_vim_message(text, cx))
    };
    let error = |editor: &Entity<EditorView>, text: &str, cx: &mut Context<Workbench>| {
        editor.update(cx, |editor, cx| editor.set_vim_error(text, cx))
    };
    match command {
        HostCommand::Write { path: None, .. } => message(&editor, &kept(), cx),
        // The box has no file to save as, nor a copy to write.
        HostCommand::Write { path: Some(_), .. } | HostCommand::SaveAs { .. } => {
            error(&editor, &not_in_the_box(), cx)
        }
        HostCommand::WriteQuit { .. } => workbench.commit_from_box(false, window, cx),
        HostCommand::Quit { force } => {
            let blank = editor.read(cx).model().text().to_string().trim().is_empty();
            if !force && !blank {
                return error(&editor, &e37(), cx);
            }
            let changes = workbench.dock.read(cx).changes().clone();
            changes.update(cx, |changes, cx| {
                if force {
                    changes.clear_box(cx);
                }
                changes.leave_box();
            });
            window.focus(&workbench.focus_handle(cx), cx);
        }
        HostCommand::User {
            name,
            bang,
            range,
            args,
        } => match git_commands::ex_command(&name, bang, range.is_some(), &args) {
            Ok((command, args)) => {
                let op = GitOp {
                    command,
                    args,
                    range,
                    editor: Some(editor),
                    typed: true,
                };
                workbench.run_git_op(op, window, cx);
            }
            Err(answer) => error(&editor, &answer, cx),
        },
        HostCommand::DiffGet { .. } | HostCommand::DiffPut { .. } => error(&editor, &e99(), cx),
        HostCommand::Edit { .. } | HostCommand::WriteAll | HostCommand::QuitAll { .. } => {
            match workbench.active_editor(cx).cloned() {
                Some(active) => {
                    let changes = workbench.dock.read(cx).changes().clone();
                    changes.update(cx, |changes, _| changes.leave_box());
                    workbench.run_vim_host_command(&active.downgrade(), command, window, cx)
                }
                None => error(&editor, &not_in_the_box(), cx),
            }
        }
        HostCommand::DiffSplit { .. }
        | HostCommand::Read { .. }
        | HostCommand::Filter { .. }
        | HostCommand::ReadCommand { .. } => error(&editor, &not_in_the_box(), cx),
        HostCommand::Explore
        | HostCommand::OldFiles
        | HostCommand::ReplaceInFiles { .. }
        | HostCommand::Pick(_)
        | HostCommand::Buffer(_)
        | HostCommand::Window(_)
        | HostCommand::Tab(_)
        | HostCommand::Quickfix(_) => workbench.run_window_command(&editor, command, window, cx),
    }
}

impl Workbench {
    /// Gives `changes`, the Changes panel, its commit box, unless it has
    /// one: an editor over a path-less buffer, soft-wrapped, with no
    /// gutter, in the editors' theme, whose host commands come here.
    pub(crate) fn attach_commit_box(
        &mut self,
        changes: &Entity<GitPanel>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if changes.read(cx).commit_box().is_some() {
            return;
        }
        let theme = self.active.read(cx).theme().clone();
        let editor = cx.new(|cx| {
            let mut model = EditorModel::new(Buffer::new(""), None);
            model.set_soft_wrap(true);
            let mut view = EditorView::new(model, theme, cx);
            view.set_gutter(false, cx);
            view
        });
        self.own_editor(&editor, Owner::CommitBox);
        changes.update(cx, |changes, cx| changes.set_commit_box(editor, window, cx));
        // Draft from diff needs the chat's key: read it now, and redraw
        // the box once it's read.
        chat_panel::provider(self, cx);
        if let Some(chat) = self.dock.read(cx).chat().cloned() {
            changes.update(cx, |changes, cx| changes.watch_chat(&chat, cx));
        }
    }

    /// Shows the Changes panel and focuses its commit box, with `amend`
    /// Amend checked.
    pub(crate) fn focus_commit_box(
        &mut self,
        amend: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.run_dock_op(DockOp::ShowChanges, window, cx);
        let changes = self.dock.read(cx).changes().clone();
        self.attach_commit_box(&changes, window, cx);
        changes.update(cx, |changes, cx| {
            if amend {
                changes.set_amend(true, window, cx);
            }
            changes.focus_commit_box(window, cx);
        });
    }

    /// Runs `command`, which the commit box's vim left, as any editor's
    /// host commands run: here, as the box's owner.
    pub(crate) fn commit_box_host_command(
        &mut self,
        editor: Entity<EditorView>,
        command: HostCommand,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.run_vim_host_command(&editor.downgrade(), command, window, cx);
    }

    /// Commits the commit box's message in the background, unless a commit
    /// is running; then shows how it went, and once it's made pushes the
    /// branch if `push` (the button's "and push").
    pub(crate) fn commit_from_box(
        &mut self,
        push: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let changes = self.dock.read(cx).changes().clone();
        let Some(request) = changes.update(cx, |changes, cx| changes.begin_commit(push, cx)) else {
            return;
        };
        self.run_commit(changes, request, false, window, cx);
    }

    /// Commits `request`, from `changes`' box, in the background, first
    /// staging every change if `stage_all`, in its turn among the work that
    /// changes a repository (`git_stage`), so it commits the index the
    /// staging asked before it leaves; then shows how it went.
    fn run_commit(
        &mut self,
        changes: Entity<GitPanel>,
        request: CommitRequest,
        stage_all: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let workbench = cx.weak_entity();
        let window = window.window_handle();
        git_stage::mutate(cx, move |cx| {
            #[cfg(test)]
            let gate = git_stage::test_gate(cx);
            let committed = cx.background_spawn(async move {
                #[cfg(test)]
                gate.await;
                let committed = commit(&request, stage_all);
                (request, committed)
            });
            cx.spawn(async move |cx| {
                let (request, committed) = committed.await;
                (cx.update_window(window, |_, window, cx| {
                    (workbench.update(cx, |workbench, cx| {
                        workbench.committed(changes, request, committed, window, cx)
                    }))
                    .ok();
                }))
                .ok();
            })
        });
    }

    /// Whether `changes` is still the Changes panel: not once the root has
    /// changed, which makes another.
    fn is_changes(&self, changes: &Entity<GitPanel>, cx: &App) -> bool {
        self.dock.read(cx).changes() == changes
    }

    /// `request`, a commit from `changes`' box, has landed: the box
    /// empties once it's made, and `git.push` runs if the request asked to
    /// push; otherwise the message stays and the failure shows. Either way
    /// the git state is read again, a hook having maybe staged files. Once
    /// the root has changed, nothing shows and nothing is pushed.
    fn committed(
        &mut self,
        changes: Entity<GitPanel>,
        request: CommitRequest,
        committed: Result<String, CommitError>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let empty = matches!(committed, Err(CommitError::EmptyMessage));
        changes.update(cx, |changes, cx| match committed {
            Ok(_) => changes.committed(cx),
            Err(_) => changes.commit_refused(empty, cx),
        });
        self.git.update(cx, GitState::refresh);
        if !self.is_changes(&changes, cx) {
            return;
        }
        let error = match committed {
            Ok(_) if request.push => {
                return self.run_git_command(GitCommand::Push, window, cx);
            }
            Ok(_) => return,
            Err(error) => error,
        };
        let (level, message, detail) = match error {
            CommitError::EmptyMessage => return,
            CommitError::NothingStaged => {
                return self.ask_to_stage_all(changes, request, window, cx);
            }
            CommitError::HookFailed { hook, output } => (
                PromptLevel::Warning,
                tr!("workbench-changes-hook-failed", hook = hook),
                truncated(&output, HOOK_OUTPUT),
            ),
            CommitError::Failed { output, .. } => (
                PromptLevel::Critical,
                tr!("workbench-changes-commit-failed"),
                output,
            ),
            CommitError::Git(error) => (
                PromptLevel::Critical,
                tr!("workbench-changes-commit-failed"),
                error.to_string(),
            ),
        };
        let detail = Some(detail.as_str()).filter(|detail| !detail.is_empty());
        let ok = [PromptButton::ok(tr!("workbench-changes-ok"))];
        drop(window.prompt(level, &message, detail, &ok, cx));
    }

    /// Asks whether to stage every change and commit `request`, refused
    /// from `changes`' box as nothing is staged, and does if so: the
    /// message and Amend as submitted, in its repository, unless the root
    /// has changed or another commit is running by then.
    fn ask_to_stage_all(
        &mut self,
        changes: Entity<GitPanel>,
        request: CommitRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let buttons = [
            PromptButton::ok(stage_all_and_commit()),
            PromptButton::cancel(tr!("workbench-changes-cancel")),
        ];
        let answer = window.prompt(
            PromptLevel::Warning,
            &tr!("workbench-changes-nothing-staged"),
            Some(&tr!("workbench-changes-stage-all-detail")),
            &buttons,
            cx,
        );
        cx.spawn_in(window, async move |workbench, cx| {
            if answer.await != Ok(0) {
                return;
            }
            (workbench.update_in(cx, |workbench, window, cx| {
                let resumed = workbench.is_changes(&changes, cx)
                    && changes.update(cx, |changes, cx| changes.resume_commit(&request, cx));
                if resumed {
                    workbench.run_commit(changes, request, true, window, cx);
                }
            }))
            .ok();
        })
        .detach();
    }

    /// Why Draft from diff can't run with `staged` files staged: with none,
    /// or as [`Self::draft_blocked`] says. What the button's tooltip says.
    pub(crate) fn draft_unavailable(&self, staged: usize, cx: &App) -> Option<SharedString> {
        match staged {
            0 => Some(nothing_staged()),
            _ => self.draft_blocked(cx),
        }
    }

    /// Why Draft from diff can't run whatever's staged: in an untrusted
    /// folder, or with the chat keyless.
    fn draft_blocked(&self, cx: &App) -> Option<SharedString> {
        if !self.root_trusted() {
            return Some(tr!("workbench-changes-draft-untrusted"));
        }
        let keyed = chat_panel::has_key(self, cx);
        (!keyed).then(|| tr!("workbench-changes-draft-no-key"))
    }

    /// Runs Draft from diff: shows the Changes panel and focuses its box,
    /// as `git.commit` does, then reads the staged changes, which
    /// [`run_draft`] asks the chat's provider about; unless a draft is
    /// being asked for or a commit runs, or [`Self::draft_blocked`] says
    /// why not, which a toast says. Untrusting the folder or changing the
    /// root drops the draft.
    pub(crate) fn draft_commit_message(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let changes = self.dock.read(cx).changes().clone();
        if changes.read(cx).not_a_repository() {
            return toast::show(cx, Toast::info(no_repo().to_string()));
        }
        self.focus_commit_box(false, window, cx);
        if changes.read(cx).drafting() || changes.read(cx).committing() {
            return;
        }
        if let Some(reason) = self.draft_blocked(cx) {
            return toast::show(cx, Toast::info(reason.to_string()));
        }
        let Some(repo_root) = changes.read(cx).repository_root() else {
            return;
        };
        let root = self.root().map(Path::to_owned);
        let read = cx.background_spawn(async move { staged_files(&repo_root, root.as_deref()) });
        let panel = changes.downgrade();
        let task = cx.spawn_in(window, async move |workbench, cx| {
            run_draft(&workbench, &panel, read, cx).await;
            (panel.update(cx, |panel, cx| panel.end_draft(cx))).ok();
        });
        let (untrusted, rerooted) = (changes.downgrade(), changes.downgrade());
        let watches = vec![
            cx.subscribe_self(move |_, TrustChanged(trusted), cx| {
                if !*trusted {
                    (untrusted.update(cx, |panel, cx| panel.cancel_draft(cx))).ok();
                }
            }),
            cx.subscribe_self(move |_, _: &RootChanged, cx| {
                (rerooted.update(cx, |panel, cx| panel.cancel_draft(cx))).ok();
            }),
        ];
        changes.update(cx, |changes, cx| changes.set_drafting(task, watches, cx));
    }

    /// The provider to send a draft for `panel`'s box to, its read done:
    /// while it's still the Changes panel, no commit runs, the folder's
    /// trusted and the chat has a key.
    fn draft_provider(
        &mut self,
        panel: &WeakEntity<GitPanel>,
        cx: &mut Context<Self>,
    ) -> Option<(Arc<dyn ChatProvider>, ChatSettings)> {
        let panel = panel.upgrade()?;
        let current = self.is_changes(&panel, cx) && !panel.read(cx).committing();
        if !current || self.draft_blocked(cx).is_some() {
            return None;
        }
        chat_panel::provider(self, cx)
    }

    /// The draft for `panel`'s box came, or why not: an empty box takes it
    /// ([`fill_with_draft`]), and over a message in it the user is asked,
    /// whose answer, the box and the draft it gives back. While a commit
    /// runs, or once the root has changed, nothing shows.
    fn drafted(
        &mut self,
        panel: &WeakEntity<GitPanel>,
        drafted: Result<fe_chat::CommitDraft, Drafted>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<(
        oneshot::Receiver<usize>,
        Entity<EditorView>,
        fe_chat::CommitDraft,
    )> {
        let changes = panel.upgrade()?;
        if !self.is_changes(&changes, cx) || changes.read(cx).committing() {
            return None;
        }
        let draft = match drafted {
            Ok(draft) => draft,
            Err(Drafted::NothingStaged) => {
                toast::show(cx, Toast::info(nothing_staged().to_string()));
                return None;
            }
            Err(Drafted::Read(error)) => {
                let toast = Toast::warning(tr!("workbench-changes-draft-read-failed").to_string());
                toast::show(cx, toast.detail(error.to_string()));
                return None;
            }
            Err(Drafted::Draft(DraftError::Failed(error))) => {
                let message = tr!("workbench-changes-draft-failed", error = error.message);
                toast::show(cx, Toast::warning(message.to_string()));
                return None;
            }
            Err(Drafted::Draft(DraftError::Empty)) => {
                let message = tr!("workbench-changes-draft-empty");
                toast::show(cx, Toast::info(message.to_string()));
                return None;
            }
        };
        let editor = changes.read(cx).commit_box().cloned()?;
        if editor.read(cx).model().text().to_string().trim().is_empty() {
            fill_with_draft(&editor, &draft, cx);
            return None;
        }
        let buttons = [
            PromptButton::ok(tr!("workbench-changes-draft-replace-button")),
            PromptButton::cancel(tr!("workbench-changes-cancel")),
        ];
        let answer = window.prompt(
            PromptLevel::Info,
            &tr!("workbench-changes-draft-replace"),
            Some(&draft.message()),
            &buttons,
            cx,
        );
        Some((answer, editor, draft))
    }
}

/// Draft from diff for `panel`'s box, once `read` has the staged files:
/// asks the provider, if [`Workbench::draft_provider`] still gives one,
/// then puts the reply in the box ([`Workbench::drafted`]), asking first
/// over a message, and filling it on Replace if it's still the Changes
/// panel's and no commit runs.
async fn run_draft(
    workbench: &WeakEntity<Workbench>,
    panel: &WeakEntity<GitPanel>,
    read: Task<Result<Vec<StagedFile>, GitError>>,
    cx: &mut AsyncWindowContext,
) {
    let files = read.await;
    let provider = workbench.update(cx, |workbench, cx| workbench.draft_provider(panel, cx));
    let Ok(Some((provider, settings))) = provider else {
        return;
    };
    let drafted = match files {
        Ok(files) if files.is_empty() => Err(Drafted::NothingStaged),
        Ok(files) => {
            (run_commit_message(&*provider, &files, &settings).await).map_err(Drafted::Draft)
        }
        Err(error) => Err(Drafted::Read(error)),
    };
    let asked = workbench.update_in(cx, |workbench, window, cx| {
        workbench.drafted(panel, drafted, window, cx)
    });
    let Ok(Some((answer, editor, draft))) = asked else {
        return;
    };
    if answer.await != Ok(0) {
        return;
    }
    (workbench.update(cx, |workbench, cx| {
        let current = panel
            .upgrade()
            .is_some_and(|panel| workbench.is_changes(&panel, cx) && !panel.read(cx).committing());
        if current {
            fill_with_draft(&editor, &draft, cx);
        }
    }))
    .ok();
}

/// What Draft from diff says with nothing staged.
fn nothing_staged() -> SharedString {
    tr!("workbench-changes-draft-nothing-staged")
}

/// Puts `draft` in `editor`, the commit box, as one undo step, noting a
/// subject over [`SUBJECT_LIMIT`].
fn fill_with_draft(editor: &Entity<EditorView>, draft: &fe_chat::CommitDraft, cx: &mut App) {
    set_box_text(editor, &draft.message(), cx);
    if let Some(count) = draft.long_subject() {
        let message = tr!(
            "workbench-changes-draft-long-subject",
            count = count,
            limit = SUBJECT_LIMIT
        );
        toast::show(cx, Toast::info(message.to_string()));
    }
}

/// Why Draft from diff gave no draft.
enum Drafted {
    /// The index was found to have nothing staged.
    NothingStaged,
    /// The staged changes couldn't be read.
    Read(GitError),
    Draft(DraftError),
}

/// The staged files of the repository containing `repo_root`, each with
/// its diff, HEAD to index: [`StagedDiff::Denied`] where the chat, at
/// `root`, never reads the file (`chat_context::refusal`). Runs on the
/// background executor.
fn staged_files(repo_root: &Path, root: Option<&Path>) -> Result<Vec<StagedFile>, GitError> {
    let repo = Repo::discover(repo_root)?.ok_or_else(|| GitError::Io(no_repo().into()))?;
    let workdir = repo.workdir().to_owned();
    // Spelled alike, so a path in the repository is found under the root.
    let canonical = |path: &Path| dunce::canonicalize(path).unwrap_or_else(|_| path.to_owned());
    let (real_workdir, root) = (canonical(&workdir), root.map(canonical));
    let mut files = Vec::new();
    for status in repo.status_split()? {
        let change = match status.staged {
            Some(ChangeKind::Added) => StagedChange::Added,
            Some(ChangeKind::Modified) => StagedChange::Modified,
            Some(ChangeKind::Deleted) => StagedChange::Deleted,
            _ => continue,
        };
        if status.unstaged == Some(ChangeKind::Conflicted) {
            continue;
        }
        let Ok(relative) = status.path.strip_prefix(&workdir) else {
            continue;
        };
        let denied = chat_context::refusal(root.as_deref(), &real_workdir.join(relative));
        let diff = match denied {
            Some(_) => StagedDiff::Denied,
            None => staged_diff(&repo, &status.path, change)?,
        };
        let path = (relative.components())
            .map(|component| component.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        files.push(StagedFile { path, change, diff });
    }
    Ok(files)
}

/// The diff of `path`, staged as `change`: its patch, or why it has none
/// shown. Binary by the blobs' bytes, HEAD's or the index's, as git
/// would judge them, whatever the patch shows: a mode change alone, or
/// `.gitattributes` asking for a textual diff, doesn't send one.
fn staged_diff(repo: &Repo, path: &Path, change: StagedChange) -> Result<StagedDiff, GitError> {
    let most = chat_context::MAX_FILE as u64;
    let head_size = match change {
        StagedChange::Added => None,
        _ => repo.head_blob_size(path)?,
    };
    let index_size = match change {
        StagedChange::Deleted => None,
        _ => repo.index_blob_size(path)?,
    };
    if head_size.max(index_size).is_some_and(|size| size > most) {
        return Ok(StagedDiff::TooLarge);
    }
    // A blob that's there has text unless it's binary.
    let head_binary = head_size.is_some() && repo.head_text(path)?.is_none();
    let index_binary = index_size.is_some() && repo.index_text(path)?.is_none();
    if head_binary || index_binary {
        return Ok(StagedDiff::Binary);
    }
    let patch = repo.staged_patch(path)?;
    // One file's patch has one header, and no line of a hunk starts so:
    // more would be files that weren't vetted.
    let headers = patch.lines().filter(|line| line.starts_with("diff --git "));
    if headers.count() > 1 {
        return Ok(StagedDiff::Denied);
    }
    Ok(match is_binary(&patch) {
        true => StagedDiff::Binary,
        false => StagedDiff::Text(patch),
    })
}

/// Whether `patch`, git's, is of a binary file: it says so, and has no
/// hunks. A line of a text file's hunk starts with its sign, so never
/// passes for either.
fn is_binary(patch: &str) -> bool {
    let mut lines = patch.lines();
    let said = lines
        .clone()
        .any(|line| line.starts_with("Binary files ") || line == "GIT binary patch");
    said && !lines.any(|line| line.starts_with("@@ "))
}

/// Commits `request`'s message in the repository containing its root,
/// first staging every change if `stage_all`. Runs on the background
/// executor.
fn commit(request: &CommitRequest, stage_all: bool) -> Result<String, CommitError> {
    let CommitRequest {
        root,
        message,
        amend,
        ..
    } = request;
    let repo = Repo::discover(root)?.ok_or_else(|| GitError::Io(no_repo().into()))?;
    if stage_all {
        let changed: Vec<PathBuf> = (repo.status_split()?.into_iter())
            .filter(|status| status.unstaged.is_some())
            .map(|status| status.path)
            .collect();
        if !changed.is_empty() {
            repo.stage(&changed)?;
        }
    }
    repo.commit(message, *amend)
}

/// The first `most` characters of `text`, then `…` if there were more.
fn truncated(text: &str, most: usize) -> String {
    match text.char_indices().nth(most) {
        Some((end, _)) => format!("{}…", &text[..end]),
        None => text.to_owned(),
    }
}

#[cfg(test)]
#[path = "git_commit_tests.rs"]
mod git_commit_tests;
