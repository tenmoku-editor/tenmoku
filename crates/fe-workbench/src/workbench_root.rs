//! The window's root folder and what follows it: re-rooting
//! ([`Workbench::set_root`]), the project panel's and the Changes panel's
//! events, the git state, and settings reloads, `files.associations` among
//! them ([`RootAssociations`]).

use std::{
    collections::{BTreeSet, HashMap},
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use fe_config::{FilesSettings, settings::IconTheme};
use fe_gpui::{DiffView, EditorView, SettingsEvent};
use fe_syntax::FileAssociations;
use gpui::{App, AppContext as _, Context, Entity, EntityId, Focusable, Subscription, Window};

use super::workbench_chrome::terminal_dir;
use super::{Launch, Palette, RootChanged, RootClosed, Workbench, git_blame, recent_picker};
use crate::{
    GitPanel, GitPanelEvent, ProjectPanel, git::GitState, project_panel::PanelEvent, title_bar,
};

/// The `workbench.icon_theme` installed last, as a GPUI global; there once
/// [`Workbench::install_icon_theme`] follows the app's settings.
struct InstalledIconTheme(IconTheme);

impl gpui::Global for InstalledIconTheme {}

impl Workbench {
    /// The git state for `root`, which `changes` reads through.
    pub(super) fn new_git(
        root: Option<PathBuf>,
        changes: &Entity<GitPanel>,
        cx: &mut Context<Self>,
    ) -> Entity<GitState> {
        let workbench = cx.weak_entity();
        let git = cx.new(|cx| GitState::new(root, changes, workbench, cx));
        changes.update(cx, |changes, _| changes.connect(git.downgrade()));
        git
    }

    /// Has kiln's config service watch `root`'s `.tenmoku/settings.toml`,
    /// with no root nothing, so a Settings tab on it re-reads an edit
    /// ([`kiln::platform::config::ChangedFiles`]) though the service loads
    /// only the launch root's.
    pub(super) fn watch_root_settings(
        root: Option<&Path>,
        cx: &mut App,
    ) -> Option<kiln::platform::config::FileWatch> {
        follow_undo_policy(cx);
        let file = fe_config::project_settings_path(root?);
        Some(kiln::platform::config::watch_files(cx, vec![file]))
    }

    /// Subscribes to a root's project panel. Its tree opening reads the git
    /// status, so the tree shows it without the Changes panel drawn.
    pub(super) fn follow_root(
        panel: &Entity<ProjectPanel>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Subscription {
        cx.subscribe_in(
            panel,
            window,
            |workbench, _, event, window, cx| match event {
                PanelEvent::OpenFile(path) => {
                    workbench.open_from_tree(path.clone(), false, window, cx)
                }
                PanelEvent::OpenFileToSide(path) => {
                    workbench.open_to_side(path.clone(), window, cx)
                }
                PanelEvent::PreviewFile(path) => {
                    workbench.open_from_tree(path.clone(), true, window, cx)
                }
                PanelEvent::FsChanged(changes) => {
                    workbench
                        .quick_open
                        .update(cx, |picker, cx| picker.delegate.fs_changed(changes, cx));
                    workbench.nav_state.fs_changed(changes, cx);
                    workbench
                        .git
                        .update(cx, |git, cx| git.fs_changed(changes, cx));
                    workbench.disk_fs_changed(changes, window, cx);
                    workbench.lsp_fs_changed(changes, cx);
                    crate::pane::editorconfigs_changed(changes, cx);
                }
                PanelEvent::Opened => workbench.git.update(cx, GitState::refresh_once),
                PanelEvent::Renamed { from, to, moves } => {
                    workbench.lsp_files_renamed(from, to);
                    workbench.panel_renamed(moves, window, cx)
                }
                PanelEvent::Created(paths) => {
                    workbench.lsp_files_created(paths);
                    crate::pane::editorconfigs_changed(&tree_changes(paths), cx);
                }
                PanelEvent::OpenTerminalIn(dir) => workbench.open_terminal_in(dir, window, cx),
                PanelEvent::FindInFolder(dir) => workbench.find_in_folder(dir, window, cx),
                PanelEvent::Deleted(paths) => {
                    workbench.lsp_files_deleted(paths);
                    crate::pane::editorconfigs_changed(&tree_changes(paths), cx);
                }
            },
        )
    }

    /// Subscribes to a root's Changes panel: its rows open as the project
    /// panel's do, the project panel shows the statuses it lists, and the
    /// rail's Changes button counts them.
    pub(super) fn follow_changes(
        changes: &Entity<GitPanel>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Subscription {
        cx.subscribe_in(
            changes,
            window,
            |workbench, changes, event, window, cx| match event {
                GitPanelEvent::OpenFile(path) => workbench
                    .active
                    .update(cx, |pane, cx| pane.open(path.clone(), window, cx)),
                GitPanelEvent::Listed => {
                    let statuses: Vec<_> = (changes.read(cx).statuses())
                        .map(|(path, kind)| (path.to_owned(), kind))
                        .collect();
                    let count = statuses.len();
                    (workbench.dock).update(cx, |dock, cx| dock.set_changed_files(count, cx));
                    (workbench.panel).update(cx, |panel, cx| panel.set_git_statuses(statuses, cx));
                    workbench.tint_tabs(cx);
                }
            },
        )
    }

    /// Re-roots the workbench at `root`, in this window: a new project
    /// panel lists it in the background, and a new quick-open palette
    /// indexes it once opened. The old ones are dropped, and with them the
    /// old tree, its watcher, and the old index's walk; their batches land
    /// nowhere. A new Changes panel reads `root`'s repository, the old
    /// one's status landing nowhere, and a new git state gives the open
    /// editors their bases in `root`'s repository, the old one's refresh
    /// landing nowhere. Search searches `root` from now on,
    /// its running search cancelled and its results cleared, the query and
    /// options kept. The
    /// dock keeps its visibility and active panel, and open tabs, dirty
    /// ones included, stay open. Emits [`RootChanged`].
    ///
    /// The config service's watcher watches `root`'s
    /// `.tenmoku/settings.toml` in place of the old root's, so a Settings
    /// tab on it follows its edits; the service still loads only the launch
    /// root's.
    pub fn set_root(&mut self, root: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        self.re_root(Some(root.clone()), window, cx);
        recent_picker::record(root.clone(), true, cx);
        cx.emit(RootChanged(root));
        cx.notify();
    }

    /// Leaves the workbench with no root, as [`Self::set_root`] re-roots
    /// it, once its tabs are closed (File > Close Folder): the project
    /// panel shows no folder, quick open, search and navigation have
    /// nothing to look in, the git state and the root's settings watch go,
    /// and the next launch reopens the window with no folder. A new
    /// terminal session starts in the home folder; one running stays. The
    /// folder stays in Open Recent. Emits [`RootClosed`], which stops the
    /// root's language servers.
    pub(crate) fn close_root(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.root.is_none() {
            return;
        }
        self.re_root(None, window, cx);
        cx.emit(RootClosed);
        cx.notify();
    }

    /// What [`Self::set_root`] and [`Self::close_root`] share: everything
    /// the root reaches follows `root`, or none.
    fn re_root(&mut self, root: Option<PathBuf>, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(
            self.palette,
            Some(Palette::Toolbar | Palette::Definitions | Palette::Symbols)
        ) {
            // Their paths are relative to the old root.
            self.dismiss_palette(window, cx);
        }
        if self.palette == Some(Palette::QuickOpen) {
            // Dismissed as Escape would be, as the palette goes.
            let picker = self.quick_open.clone();
            self.dismiss_palette(window, cx);
            if picker.focus_handle(cx).contains_focused(window, cx) {
                window.focus(&self.focus_handle(cx), cx);
            }
        }
        // Their searches and locations are the old root's.
        self.close_reference_peeks(window, cx);
        let panel_focused = self.panel.focus_handle(cx).contains_focused(window, cx);
        let changes_focused =
            (self.dock.read(cx).changes().focus_handle(cx)).contains_focused(window, cx);
        let show_ignored = self.panel.read(cx).lists_ignored();
        let panel = cx.new(|cx| ProjectPanel::new(root.clone(), show_ignored, cx));
        let changes = cx.new(|cx| GitPanel::new(root.clone(), cx));
        let quick_open = Self::new_quick_open(root.clone(), self.active.downgrade(), cx);
        // The next session starts in the new root, or with none at home;
        // one running stays.
        let cwd = terminal_dir(&Launch {
            root: root.clone(),
            file: None,
        });
        (self.terminal).update(cx, |terminal, _| terminal.set_cwd(cwd));
        self.root_subscription = Self::follow_root(&panel, window, cx);
        self.changes_subscription = Self::follow_changes(&changes, window, cx);
        self.git.update(cx, |git, _| git.close());
        self.git = Self::new_git(root.clone(), &changes, cx);
        let search = self.dock.update(cx, |dock, cx| {
            dock.set_project(panel.clone(), cx);
            dock.set_changes(changes.clone(), cx);
            dock.search().clone()
        });
        search.update(cx, |search, cx| search.set_root(root.clone(), cx));
        let references = self.dock.read(cx).references().clone();
        references.update(cx, |references, cx| references.clear(cx));
        self.nav_state.set_root(root.clone(), cx);
        self.panel = panel;
        self.quick_open = quick_open;
        if panel_focused {
            window.focus(&self.panel.focus_handle(cx), cx);
        }
        if changes_focused {
            window.focus(&changes.focus_handle(cx), cx);
        }
        // So the new tree reveals the active file, if it's under the root.
        self.revealed = None;
        self.root = root.clone();
        // Its saved layout names the new root, or none, for the next launch.
        *self.layout_keys.root.borrow_mut() = root.clone();
        self.layout_keys_changed(cx);
        self.root_settings = Self::watch_root_settings(root.as_deref(), cx);
        for pane in self.panes(cx) {
            let root = root.clone();
            pane.update(cx, |pane, cx| {
                pane.update_toolbar(|toolbar| toolbar.root = root, cx)
            });
        }
        self.resolve_trust(root.clone(), true, window, cx);
        self.track_editors(cx);
        // Patterns with a `/` match from the new root, with the settings
        // it has until its own load (if they're the same, no load says so).
        self.publish_associations(None, cx);
        let name = title_bar::name(root.as_deref());
        self.shell.update(cx, |shell, cx| shell.set_title(name, cx));
    }

    /// Whether the config service watches a root's settings file for this
    /// window.
    #[cfg(test)]
    pub(crate) fn watches_root_settings(&self) -> bool {
        self.root_settings.is_some()
    }

    /// The folder the workbench is rooted at, if any.
    pub fn root(&self) -> Option<&Path> {
        self.root.as_deref()
    }

    #[cfg(test)]
    pub(crate) fn git(&self) -> &Entity<GitState> {
        &self.git
    }

    /// What the Git Log lists, as each commit's summary, and what it shows
    /// in place of the list, for tests of the restored first listing.
    #[doc(hidden)]
    pub fn git_log_listing(&self, cx: &App) -> Option<(Vec<String>, Option<String>)> {
        let panel = self.dock.read(cx).git_log()?;
        Some(panel.read(cx).listing())
    }

    /// Installs the icon set `workbench.icon_theme` names, redrawing every
    /// window, and the first time follows the settings service's from then
    /// on.
    ///
    /// The set is the app's, so only the app's settings choose it: one
    /// observer of [`kiln::GlobalSettings`], never a window's settings
    /// event. A window on another project gets settings loaded for it
    /// afresh, which fall back to the default when the user's file doesn't
    /// parse, and may land after a newer choice; the service keeps the
    /// user's last good value.
    pub(crate) fn install_icon_theme(theme: IconTheme, cx: &mut App) {
        if !cx.has_global::<InstalledIconTheme>() {
            cx.observe_global::<kiln::GlobalSettings<fe_config::Settings>>(|cx| {
                let global = cx.global::<kiln::GlobalSettings<fe_config::Settings>>();
                let theme = global.settings().workbench.icon_theme;
                if cx.global::<InstalledIconTheme>().0 != theme {
                    Self::set_icon_theme(theme, cx);
                }
            })
            .detach();
        }
        Self::set_icon_theme(theme, cx);
    }

    /// Installs `theme`'s set, redrawing every window.
    fn set_icon_theme(theme: IconTheme, cx: &mut App) {
        cx.set_global(InstalledIconTheme(theme));
        match theme {
            IconTheme::Tenmoku => fe_ui::set_icon_set(Arc::new(fe_ui::TintedIcons), cx),
            IconTheme::Material => fe_ui::set_icon_set(Arc::new(fe_ui::ColouredIcons), cx),
            IconTheme::Minimal => fe_ui::reset_icon_set(cx),
        }
    }

    /// Has editors detect by this workbench's `files.associations`
    /// ([`RootAssociations`]), the app's settings' until its own arrive,
    /// until it's released: call once, from `new`. Its editors opened
    /// already are detected again.
    pub(crate) fn follow_file_associations(&mut self, cx: &mut Context<Self>) {
        if !cx.has_global::<RootAssociations>() {
            EditorView::set_language_associations(cx, associated_language);
        }
        let id = cx.entity_id();
        cx.on_release(move |_, cx| {
            if cx.has_global::<RootAssociations>() {
                (cx.global_mut::<RootAssociations>().workbenches).remove(&id);
            }
        })
        .detach();
        let files = kiln::platform::config::settings::<fe_config::Settings>(cx)
            .files
            .clone();
        self.publish_associations(Some(&files), cx);
    }

    /// Records the root, and `files` if given, as this workbench's
    /// associations. If that changes them, detects again every window's
    /// editors (any may have a file under this root), lists this window's
    /// config problems again, and waits for languages on their way
    /// ([`watch_pending_languages`]).
    fn publish_associations(&mut self, files: Option<&FilesSettings>, cx: &mut Context<Self>) {
        let id = cx.entity_id();
        let associations = cx.default_global::<RootAssociations>();
        let kept = associations.workbenches.get(&id);
        let files = match (files, kept) {
            (Some(files), _) => files.clone(),
            (None, Some(kept)) => kept.files.clone(),
            (None, None) => return,
        };
        if kept.is_some_and(|kept| kept.root == self.root && kept.files == files) {
            return;
        }
        associations.serial += 1;
        let entry = WindowAssociations {
            root: self.root.clone(),
            compiled: Arc::new(FileAssociations::new(
                files.project_associations(),
                files.user_associations(),
            )),
            user: Arc::new(FileAssociations::new([], files.user_associations())),
            files,
            serial: associations.serial,
        };
        associations.workbenches.insert(id, entry);
        self.detect_languages_again(cx);
        for other in Self::other_workbenches(cx) {
            other.update(cx, |other, cx| other.detect_languages_again(cx));
        }
        self.sync_config_problems_soon(cx);
        watch_pending_languages(cx);
    }

    /// Lists this window's config problems again once this update is
    /// over: [`Workbench::sync_config_problems`] wants its window.
    fn sync_config_problems_soon(&self, cx: &mut Context<Self>) {
        let id = cx.entity_id();
        cx.defer(move |cx| {
            each_workbench(cx, |workbench, window, cx| {
                if cx.entity_id() == id {
                    workbench.sync_config_problems(window, cx);
                }
            });
        });
    }

    /// The `files.associations` in effect for this workbench, as
    /// [`RootAssociations`] keeps them.
    pub(crate) fn root_files(&self, cx: &Context<Self>) -> Option<FilesSettings> {
        let associations = cx.try_global::<RootAssociations>()?;
        let entry = associations.workbenches.get(&cx.entity_id())?;
        Some(entry.files.clone())
    }

    /// Detects again the language of each open editor, and each side of
    /// a diff, whose language wasn't picked: for when
    /// `files.associations` may say another. A diff's sides with no file
    /// go by the file compared ([`DiffView::detect_languages_again`]).
    pub(crate) fn detect_languages_again(&self, cx: &mut Context<Self>) {
        let mut editors = self.open_editors(cx);
        let mut diffs = Vec::new();
        for pane in self.panes(cx) {
            for tab in pane.read(cx).tabs() {
                if let Some(diff) = tab.item().downcast_ref::<DiffView>() {
                    let view = diff.read(cx);
                    let sides = [view.left().clone(), view.right().clone()];
                    let with_files = sides
                        .into_iter()
                        .filter(|side| side.read(cx).language_path().is_some());
                    editors.extend(with_files);
                    diffs.push(diff.clone());
                }
            }
        }
        for editor in editors {
            if !editor.read(cx).is_language_chosen() {
                editor.update(cx, |editor, cx| editor.detect_language(cx));
            }
        }
        for diff in diffs {
            diff.update(cx, |diff, cx| diff.detect_languages_again(cx));
        }
    }

    /// Applies reloaded settings to the open editors and to editors yet to
    /// open, and `status_bar.visible` to the status bar and
    /// `workbench.rail` to the rail and sidebar if the reload changed them,
    /// so reloading for any other key keeps a toggle or a move.
    pub fn reload_settings(&mut self, event: &SettingsEvent, cx: &mut Context<Self>) {
        self.preview_tabs = event.settings.workbench.preview_tabs;
        self.set_start_page_on(event.settings.workbench.start_page, cx);
        let level = event.settings.workbench.notifications;
        self.toasts
            .update(cx, |toasts, cx| toasts.set_level(level, cx));
        // Drawn again, it follows vim turned on or off.
        self.start_page.update(cx, |_, cx| cx.notify());
        self.follow_zen_settings(&event.settings.workbench.zen_mode);
        // Not `workbench.icon_theme`: the app's settings choose the icon
        // set (`install_icon_theme`), not a window's.
        let visible = event.settings.status_bar.visible;
        if visible != event.previous.status_bar.visible {
            self.set_status_bar_visible(visible, cx);
        }
        if event.settings.terminal != event.previous.terminal {
            self.set_terminal_settings(&event.settings.terminal, cx);
        }
        let rail = event.settings.workbench.rail;
        if rail != event.previous.workbench.rail {
            // The sidebar opens on the rail's side: they move together, as
            // `dock.move_sidebar_left` and `_right` move them.
            self.move_sidebar(crate::dock::rail_position(rail), cx);
        }
        if event.settings.tree != event.previous.tree {
            let show_ignored = event.settings.tree.show_ignored;
            (self.panel).update(cx, |panel, cx| panel.set_show_ignored(show_ignored, cx));
        }
        for pane in self.panes(cx) {
            pane.update(cx, |pane, cx| pane.reload_settings(event, cx));
        }
        self.publish_associations(Some(&event.settings.files), cx);
        // Editors restored from a saved layout open as the panes' do.
        let theme = self.active.read(cx).theme().clone();
        self.terminal
            .update(cx, |terminal, cx| terminal.set_theme(&theme, cx));
        fe_gpui::set_restore_style(cx, theme, event.settings.editor.clone());
        self.lsp
            .format_on_save
            .set(event.settings.editor.format_on_save);
        self.lsp
            .code_actions_on_save
            .borrow_mut()
            .clone_from(&event.settings.editor.code_actions_on_save);
        self.lsp
            .inlay_hints_on
            .set(event.settings.editor.inlay_hints);
        self.lsp.closing_hints.set((
            event.settings.editor.closing_brace_hints,
            event.settings.editor.closing_brace_hints_min_rows,
        ));
        self.lsp.code_lens_on.set(event.settings.editor.code_lens);
        self.lsp.links_on.set(event.settings.editor.links);
        (self.lsp.color_decorators_on).set(event.settings.editor.color_decorators);
        self.set_default_color_decorators(event.settings.editor.default_color_decorators, cx);
        self.lsp.lightbulb_on.set(event.settings.editor.lightbulb);
        self.lsp
            .linked_editing_on
            .set(event.settings.editor.linked_editing);
        (self.lsp.format_on_type_on).set(event.settings.editor.format_on_type);
        (self.lsp.format_on_paste_on).set(event.settings.editor.format_on_paste);
        self.lsp
            .semantic_highlighting_on
            .set(event.settings.editor.semantic_highlighting);
        self.sync_save_hooks(cx);
        self.reload_lsp_settings(&event.settings.lsp, cx);
        // For the sessions started from now on.
        self.reload_debug_settings(&event.settings.debug, cx);
        self.reload_auto_save(&event.settings.editor, cx);
        self.reload_spell_squiggles(&event.settings.editor, cx);
        self.reload_todo_comments(&event.settings.editor, cx);
        let inline_blame = event.settings.editor.inline_blame;
        if inline_blame != event.previous.editor.inline_blame {
            git_blame::set_line_blame(self, inline_blame, cx);
        }
        // The settings changed: the Settings tab reads its files again.
        self.reload_settings_editors(cx);
    }
}

/// Each workbench's `files.associations`, with its root: from the
/// settings it was last given (the app's, until its own load gives them),
/// dropped with it. What editors detect a file's language by
/// ([`associated_language`]).
#[derive(Default)]
struct RootAssociations {
    workbenches: HashMap<EntityId, WindowAssociations>,
    /// Bumped by each change, so the newest settings win a tie.
    serial: u64,
    /// Whether [`watch_pending_languages`] is waiting.
    watching: bool,
}

impl gpui::Global for RootAssociations {}

/// One workbench's [`RootAssociations`].
struct WindowAssociations {
    root: Option<PathBuf>,
    files: FilesSettings,
    /// The project's patterns, then the user's.
    compiled: Arc<FileAssociations>,
    /// The user's alone.
    user: Arc<FileAssociations>,
    serial: u64,
}

/// The language `files.associations` give the file at `path`: those of the
/// deepest root a workbench has it under, the newest of a root's, with
/// patterns with a `/` matching from there; else the user's, by its name,
/// as last given. So a project's associations apply to its own files in
/// any window, and to no others.
fn associated_language(path: &Path, cx: &App) -> Option<&'static str> {
    let associations = cx.try_global::<RootAssociations>()?;
    let windows = || associations.workbenches.values();
    let under = windows()
        .filter_map(|entry| Some((entry.root.as_deref()?, entry)))
        .filter(|(root, _)| path.starts_with(root))
        .max_by_key(|(root, entry)| (root.components().count(), entry.serial));
    match under {
        Some((root, entry)) => entry.compiled.language_for(path, Some(root)),
        None => {
            let newest = windows().max_by_key(|entry| entry.serial)?;
            newest.user.language_for(path, None)
        }
    }
}

/// Runs `f` on every window's workbench, but one being updated now.
fn each_workbench(cx: &mut App, f: impl Fn(&mut Workbench, &mut Window, &mut Context<Workbench>)) {
    for window in cx.windows() {
        if let Some(window) = window.downcast::<Workbench>() {
            window
                .update(cx, |workbench, window, cx| f(workbench, window, cx))
                .ok();
        }
    }
}

/// How often [`watch_pending_languages`] looks, at first, then after
/// [`PENDING_SLOW_AFTER`].
const PENDING_POLL: Duration = Duration::from_millis(250);
const PENDING_SLOW_POLL: Duration = Duration::from_secs(5);
const PENDING_SLOW_AFTER: Duration = Duration::from_secs(30);

/// The languages the associations name that aren't registered yet but
/// are on their way ([`crate::grammar_prompt::language_on_its_way`]): an
/// installed extension's grammar the loader is still registering.
fn pending_languages(cx: &App) -> BTreeSet<String> {
    let Some(associations) = cx.try_global::<RootAssociations>() else {
        return BTreeSet::new();
    };
    let known: BTreeSet<&str> = fe_syntax::language_names().collect();
    (associations.workbenches.values())
        .flat_map(|entry| {
            let files = &entry.files;
            files
                .project_associations()
                .chain(files.user_associations())
        })
        .map(|(_, language)| language)
        .filter(|language| !known.contains(language))
        .filter(|language| crate::grammar_prompt::language_on_its_way(language, cx))
        .map(str::to_owned)
        .collect()
}

/// Waits, once per app, for the languages the associations name that are
/// on their way ([`pending_languages`]): as each registers, every
/// window's editors are detected again and its config problems listed
/// again. Stops when none is on its way.
fn watch_pending_languages(cx: &mut App) {
    let watching = &mut cx.default_global::<RootAssociations>().watching;
    if *watching || pending_languages(cx).is_empty() {
        return;
    }
    cx.default_global::<RootAssociations>().watching = true;
    cx.spawn(async move |cx| {
        let executor = cx.background_executor().clone();
        let slow_from = executor.now() + PENDING_SLOW_AFTER;
        let mut pending = cx.update(|cx| pending_languages(cx));
        while !pending.is_empty() {
            let poll = match executor.now() < slow_from {
                true => PENDING_POLL,
                false => PENDING_SLOW_POLL,
            };
            executor.timer(poll).await;
            let now = cx.update(|cx| pending_languages(cx));
            if now.len() < pending.len() || !now.is_subset(&pending) {
                cx.update(|cx| {
                    each_workbench(cx, |workbench, window, cx| {
                        workbench.detect_languages_again(cx);
                        workbench.sync_config_problems(window, cx);
                    })
                });
            }
            pending = now;
        }
        cx.update(|cx| cx.default_global::<RootAssociations>().watching = false);
    })
    .detach();
}

/// The tree's own New File or Delete of `paths`, as the watcher would
/// report them.
fn tree_changes(paths: &[(std::path::PathBuf, bool)]) -> Vec<fe_project::FsChange> {
    (paths.iter())
        .map(|(path, _)| fe_project::FsChange::Path(path.clone()))
        .collect()
}

/// The settings layers [`refresh_undo_policy`] keeps for each root.
#[derive(Default)]
struct UndoSettings(std::collections::HashMap<Option<PathBuf>, fe_config::SettingsStore>);

impl gpui::Global for UndoSettings {}

/// Marks that [`follow_undo_policy`] observes the config service.
struct FollowsUndoPolicy;

impl gpui::Global for FollowsUndoPolicy {}

/// Has each window's `editor.persistent_undo` follow the settings files
/// that apply to it, once per app: its root's project file and the user's,
/// whichever the launch root's layer masks (`GlobalSettings` misses both).
fn follow_undo_policy(cx: &mut App) {
    if cx.has_global::<FollowsUndoPolicy>() {
        return;
    }
    cx.set_global(FollowsUndoPolicy);
    cx.observe_global::<kiln::platform::config::ChangedFiles>(|cx| {
        for window in cx.windows() {
            if let Some(window) = window.downcast::<Workbench>() {
                refresh_undo_policy(window, cx);
            }
        }
    })
    .detach();
}

/// Gives `window` the `editor.persistent_undo` its root's settings say now,
/// if the files just changed are its own. Read at once, so no later read is
/// older than the policy it sets; a window still waiting for its root's
/// settings is left to that load, which reads the files as they are.
fn refresh_undo_policy(window: gpui::WindowHandle<Workbench>, cx: &mut App) {
    let id = window.window_id();
    if fe_gpui::pane_item::window_waits_for_persistent_undo(id, cx) {
        return;
    }
    let Ok(root) = window.read_with(cx, |workbench, _| workbench.root().map(Path::to_owned)) else {
        return;
    };
    let user = crate::open::settings_path(cx);
    let project = root.as_deref().map(fe_config::project_settings_path);
    let changed = kiln::platform::config::changed_files(cx);
    if !(user.iter().chain(project.iter())).any(|file| changed.contains(file)) {
        return;
    }
    // The layers kept per root, so a file that no longer parses keeps its
    // last accepted values.
    let files: Vec<PathBuf> = user.iter().chain(project.iter()).cloned().collect();
    let stores = cx.default_global::<UndoSettings>();
    let store = (stores.0.entry(root.clone()))
        .or_insert_with(|| fe_config::SettingsStore::load(user, project));
    store.reload(&files);
    let on = store.settings().editor.persistent_undo;
    fe_gpui::pane_item::set_window_persistent_undo(id, Some(on), cx);
}
