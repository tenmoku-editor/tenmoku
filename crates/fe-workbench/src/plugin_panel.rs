//! A plugin's panel in a dock (plugins stage 3, §4.2 of
//! `docs/decisions/dqsjfernkt-plugins-stage-3.md`): the view a plugin sets
//! through `fastedit:editor/panels`, kept and validated by fe-plugin's
//! [`PanelViews`], drawn here with kiln-ui. Its manifest's `dock` puts it in
//! the sidebar (`left` or `right`: the one sidebar, on the side
//! `workbench.rail` gives it) with a button on the rail's top group, or in
//! the bottom panel (`bottom`) with a button at the rail's foot, as the
//! bottom tool windows have ([`crate::Workbench::add_plugin_panel`]).
//!
//! - **Drawing.** Containers (`column`, `row`, `scroll`) lay out their
//!   children; content and controls are kiln-ui's (a [`Button`], a
//!   [`Checkbox`] for a toggle, a [`TextField`] for an input, a [`TreeRow`]
//!   for a tree item). A `list` is virtualized over GPUI's `uniform_list`:
//!   only the rows in view are built, whatever its length. Its rows are its
//!   children, each tree item followed by its children while it is
//!   expanded, so a tree is a list of tree items.
//! - **Theming.** A view names no colour or font: a [`Tone`] is a theme
//!   token ([`tone_color`]), `mono` the code font, and an icon a name in
//!   kiln-ui's set ([`icon_named`]; an unknown one draws nothing and is
//!   logged once).
//! - **Accessibility.** Every node kind has its AccessKit role ([`role_of`])
//!   and every node that says something its name: a button, toggle, input,
//!   list or tree item its `label`, a heading, text or badge its text, an
//!   icon its name, a progress bar "Progress" with its percent as value.
//!   Containers and separators have a role and no name, as a screen reader
//!   reads no name for them. A list is a list box of options, or, holding
//!   tree items, a tree of tree items, each with its level, place in its
//!   set, and expanded and selected states.
//! - **Keys.** The host owns focus: Tab and Shift-Tab move through the
//!   window's tab stops, so a panel can't keep focus in itself. In a list,
//!   Up, Down, Home, End, Page Up and Page Down move the selection, Enter
//!   activates the selected row, Space toggles it in a multi-select list,
//!   and Right and Left expand and collapse a tree item, or move to its
//!   first child and its parent. Enter or Space presses a focused button
//!   or toggle, as GPUI clicks a focused control.
//! - **Events.** What the user does goes to the plugin as `panel-event`s
//!   ([`PanelEvent`]) through the host's [`PanelEvents`] queue, which holds
//!   at most [`MAX_QUEUED_EVENTS`] a plugin has not taken: one more drops
//!   the oldest, and counts it. An input's text goes 150 ms after the last
//!   keystroke ([`INPUT_DEBOUNCE`]). A toggle's or a tree item's new state
//!   shows at once, until the plugin's next view says otherwise.
//! - **Updates.** The panel reads its view from [`PanelViews`] every
//!   [`MIN_UPDATE_INTERVAL`] while it shows, showing a held update once it
//!   is due ([`PanelViews::flush`]); hidden, it only notes whether it shows
//!   ([`PanelEvent::Shown`], [`PanelEvent::Hidden`]).
//! - **Grids.** A `grid` is a row of text in the editor's font a row of
//!   cells, one image region named by its label (`plugin_panel_grid`).
//! - **Realtime panels** (T2007), ones whose manifest asks for `keys` or
//!   `tick_ms`. One with `keys` takes keyboard focus when clicked or shown
//!   by its command, and shows a focus ring while it has it. Its key
//!   context holds its plain keys ([`HOLDS_PLAIN_KEYS`]), so no binding,
//!   vim's, the editor's or the workbench's, takes them first: each goes to the plugin
//!   ([`PanelEvent::Key`]) except the host's (fe-plugin's
//!   [`KeyEvent::is_reserved`]). Escape returns focus to the editor
//!   (`dock.focus_editor`) from anywhere in a realtime panel, a control
//!   in it that leaves Escape be included; Tab moves focus on; and the
//!   keymap still has keys with the secondary modifier and the chords they
//!   start, a plain key continuing one included, and Tab with any
//!   modifiers (`ctrl-tab`'s tab switcher). One with
//!   `tick_ms` has one timer while it is shown and focused in the active
//!   window, started when it takes focus and dropped when it loses it (the
//!   window's too), is hidden or its plugin unloads. Each tick goes to the
//!   plugin ([`PanelEvent::Tick`]) through fe-plugin's [`PanelRuntime`]
//!   for the load, which lets no tick overlap the last: one falling due
//!   while the plugin still has the last is dropped. A tick is an ordinary
//!   call, so a plugin that runs past its deadline is unloaded, as for any
//!   other. Each timer is a tick session of that panel instance (one per
//!   window): when it stops, the panel is released (its window closed) or
//!   the timer can no longer reach it, the session ends, and a tick of it
//!   queued, or taken by the host, is dropped before it reaches the plugin
//!   ([`PanelEvents::close_ticks`]); another window's session is untouched.

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::ops::Range;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};
use std::time::{Duration, Instant};

use fe_command::Invocation;
use fe_gpui::commands::RunCommand;
use fe_plugin::fastedit::editor::panels::{Node, NodeKind};
pub(crate) use fe_plugin::panels::{
    Delivery, KeyEvent, MIN_UPDATE_INTERVAL, PanelRuntime, PanelState, PanelViews, Skipped, Tone,
    View,
};
use fe_ui::{
    ActiveTheme as _, Button, CODE, Checkbox, FOCUS_RING, FamilySlot, FontFamilies, Icon,
    ROW_HEIGHT, Role, States, TextField, TextFieldEvent, Tokens, TreeRow, UI_BODY, UI_HEADING,
    UiTheme, node as a11y, node_with as a11y_with, space,
};
use gpui::{
    AnyElement, App, AppContext as _, ClickEvent, Context, ElementId, Entity, FocusHandle,
    Focusable, Hsla, InteractiveElement as _, IntoElement, KeyContext, KeyDownEvent, ListAlignment,
    ListOffset, ListState as MeasuredList, Modifiers, ParentElement as _, Pixels, Render,
    ScrollStrategy, SharedString, StatefulInteractiveElement as _, Styled as _, Subscription, Task,
    UniformListScrollHandle, Window, div, list as measured_list, prelude::FluentBuilder as _, px,
    relative, svg, uniform_list,
};
use kiln_l10n::{Text, tr, verbatim};
use kiln_platform::keys::{self, HOLDS_PLAIN_KEYS};
use kiln_plugin::manifest::{PanelDecl, PanelLocation};
use kiln_shell::{DockItem, PanelId};

use crate::{Workbench, plugin_host};

/// How many unknown icon names a panel logs, once each; it logs no more.
pub(crate) const MAX_LOGGED_ICONS: usize = 64;

/// How much of an unknown icon name a panel keeps: more than any icon's.
const MAX_ICON_NAME_BYTES: usize = 64;

/// `text` cut to at most `max` bytes, at a character boundary.
fn truncated(text: &str, max: usize) -> &str {
    let mut end = text.len().min(max);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

/// The events a plugin hasn't taken that the host keeps for it, over all
/// its panels; one more drops the oldest.
pub(crate) const MAX_QUEUED_EVENTS: usize = 256;

/// How long after the last keystroke an input's text goes to the plugin.
pub(crate) const INPUT_DEBOUNCE: Duration = Duration::from_millis(150);

/// Rows a Page Up or Page Down moves a list's selection by.
const PAGE_ROWS: usize = 10;

/// The key context of a panel with `keys`, which no binding names; it
/// holds its plain keys ([`HOLDS_PLAIN_KEYS`]) for the panel's own key
/// listener.
pub(crate) const KEYS_CONTEXT: &str = verbatim!("plugin_panel_keys");

/// What the user did in a panel, as `fastedit:editor/panel-handler`'s
/// `panel-event` tells the plugin. Node ids are the plugin's own.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PanelEvent {
    /// A button pressed, or a tree item or list row opened: Enter, or a
    /// double click.
    Activated(String),
    /// A toggle switched, to the value given.
    Toggled(String, bool),
    /// An input's text, [`INPUT_DEBOUNCE`] after the last change.
    Input(String, String),
    /// Enter in an input, with its text.
    Submitted(String, String),
    /// A list's selected rows, by id.
    Selected(String, Vec<String>),
    /// A tree item expanded (true) or collapsed.
    Expanded(String, bool),
    /// The panel came into view.
    Shown,
    /// It went out of view.
    Hidden,
    /// A key pressed while the panel has focus, of a panel with `keys`.
    Key(KeyEvent),
    /// A tick of a panel with `tick_ms`: the milliseconds since its last.
    Tick(u32),
}

/// One event for a plugin, and the panel it's of.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueuedEvent {
    pub panel: String,
    pub event: PanelEvent,
}

/// The events the panels have for each plugin, until the host delivers
/// them: at most [`MAX_QUEUED_EVENTS`] per plugin, the oldest dropped and
/// counted past that. A plugin's waker ([`PanelEvents::wake_with`]) is
/// called after each event queued for it. Clones share it.
#[derive(Clone, Default)]
pub struct PanelEvents {
    shared: Arc<Mutex<BTreeMap<String, Queue>>>,
}

/// Asks a dock whether it shows a panel.
type Shows = Box<dyn Fn(&App) -> bool>;

/// What wakes the host to deliver a plugin's events.
pub(crate) type Waker = Arc<dyn Fn() + Send + Sync>;

#[derive(Default)]
struct Queue {
    events: VecDeque<QueuedEvent>,
    dropped: u64,
    waker: Option<Waker>,
    /// The load's realtime panels: their ticks in the plugin.
    runtime: Arc<PanelRuntime>,
    /// The open tick session of each realtime panel, by its id: the one
    /// panel instance, in one window, ticking it ([`PanelEvents::open_ticks`],
    /// [`PanelEvents::close_ticks`]). None while none ticks.
    sessions: HashMap<String, TickSession>,
    /// The session of each panel's tick queued or with the host, which the
    /// runtime lets be only one ([`PanelRuntime::deliver_tick`]).
    in_flight: HashMap<String, TickSession>,
}

impl Queue {
    /// `panel`'s tick is done with, delivered or not: the next may go.
    fn release_tick(&mut self, panel: &str) {
        self.in_flight.remove(panel);
        self.runtime.tick_finished(panel);
    }

    /// Queues `event`, dropping the oldest if full; whether it did.
    fn enqueue(&mut self, panel: &str, event: PanelEvent) -> bool {
        let dropped = self.events.len() >= MAX_QUEUED_EVENTS;
        if dropped {
            // A tick dropped never reaches the plugin: the next may go.
            if let Some(QueuedEvent {
                panel,
                event: PanelEvent::Tick(_),
            }) = self.events.pop_front()
            {
                self.release_tick(&panel);
            }
            self.dropped += 1;
        }
        self.events.push_back(QueuedEvent {
            panel: panel.to_owned(),
            event,
        });
        dropped
    }
}

/// One stretch of one panel instance's ticking, from its timer's start to
/// its stop: unique in the process, so one window's panel ending its
/// session never touches another's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TickSession(u64);

impl TickSession {
    fn next() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        Self(NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
    }
}

/// What [`PanelEvents::tick`] did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Ticked {
    /// The tick is queued for the plugin.
    Queued,
    /// The runtime kept it ([`Skipped`]).
    Skipped(Skipped),
    /// The load the panel ticked for is gone.
    Gone,
}

impl PanelEvents {
    pub fn new() -> Self {
        Self::default()
    }

    /// Queues `event` of plugin `plugin`'s panel `panel`. Whether that
    /// dropped the oldest queued event, the queue being full.
    pub fn push(&self, plugin: &str, panel: &str, event: PanelEvent) -> bool {
        let (dropped, waker) = {
            let mut shared = self.lock();
            let queue = shared.entry(plugin.to_owned()).or_default();
            (queue.enqueue(panel, event), queue.waker.clone())
        };
        // Outside the lock: a waker may take the events at once.
        if let Some(waker) = waker {
            waker();
        }
        dropped
    }

    /// Calls `waker` after each event queued for plugin `plugin` from now
    /// on, until it's unloaded or [`unwake`](Self::unwake)d: for the host to
    /// deliver them. Returns it, for `unwake`.
    pub(crate) fn wake_with(
        &self,
        plugin: &str,
        waker: impl Fn() + Send + Sync + 'static,
    ) -> Waker {
        let waker: Waker = Arc::new(waker);
        let mut shared = self.lock();
        shared.entry(plugin.to_owned()).or_default().waker = Some(waker.clone());
        waker
    }

    /// Forgets plugin `plugin`'s waker if it's still `waker`, a later load's
    /// left alone: a waker holds its load's thread's channel open, so its
    /// thread can't stop while it's kept.
    pub(crate) fn unwake(&self, plugin: &str, waker: &Waker) {
        let mut shared = self.lock();
        if let Some(queue) = shared.get_mut(plugin)
            && queue
                .waker
                .as_ref()
                .is_some_and(|kept| Arc::ptr_eq(kept, waker))
        {
            queue.waker = None;
        }
    }

    /// Takes up to `max` of plugin `plugin`'s events, oldest first, for the
    /// host to deliver.
    pub fn take(&self, plugin: &str, max: usize) -> Vec<QueuedEvent> {
        let mut shared = self.lock();
        let Some(queue) = shared.get_mut(plugin) else {
            return Vec::new();
        };
        let count = max.min(queue.events.len());
        queue.events.drain(..count).collect()
    }

    /// How many of plugin `plugin`'s events wait.
    pub fn queued(&self, plugin: &str) -> usize {
        self.lock()
            .get(plugin)
            .map_or(0, |queue| queue.events.len())
    }

    /// How many of plugin `plugin`'s events were dropped for a full queue
    /// since it was loaded.
    pub fn dropped(&self, plugin: &str) -> u64 {
        self.lock().get(plugin).map_or(0, |queue| queue.dropped)
    }

    /// Forgets plugin `plugin`'s events, its count, its waker and its
    /// realtime panels' runtime: it was unloaded.
    pub fn unload(&self, plugin: &str) {
        self.lock().remove(plugin);
    }

    /// Plugin `plugin`'s load's realtime runtime, which its panels tick
    /// through ([`PanelEvents::tick`]); a later load has another.
    pub(crate) fn runtime(&self, plugin: &str) -> Arc<PanelRuntime> {
        (self.lock().entry(plugin.to_owned()).or_default().runtime).clone()
    }

    /// Queues a tick, `ms` since the last, for plugin `plugin`'s panel
    /// `panel`, in `state`, if `runtime` is still the plugin's load's,
    /// `session` is the panel's open one and the runtime lets it go
    /// ([`PanelRuntime::deliver_tick`]).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn tick(
        &self,
        plugin: &str,
        panel: &str,
        session: TickSession,
        runtime: &Arc<PanelRuntime>,
        state: PanelState,
        now: Instant,
        ms: u32,
    ) -> Ticked {
        let waker = {
            let mut shared = self.lock();
            let Some(queue) = shared.get_mut(plugin) else {
                return Ticked::Gone;
            };
            if !Arc::ptr_eq(&queue.runtime, runtime) {
                return Ticked::Gone;
            }
            // Another instance (another window's) ticks it now.
            if queue.sessions.get(panel) != Some(&session) {
                return Ticked::Skipped(Skipped::Unfocused);
            }
            if let Delivery::Skipped(skipped) = runtime.deliver_tick(panel, state, now) {
                return Ticked::Skipped(skipped);
            }
            // Noted before the host can take it.
            queue.in_flight.insert(panel.to_owned(), session);
            queue.enqueue(panel, PanelEvent::Tick(ms));
            queue.waker.clone()
        };
        if let Some(waker) = waker {
            waker();
        }
        Ticked::Queued
    }

    /// Plugin `plugin`'s panel `panel` starts ticking, in one instance: a
    /// new session, the panel's open one, which [`PanelEvents::close_ticks`]
    /// ends.
    pub(crate) fn open_ticks(&self, plugin: &str, panel: &str) -> TickSession {
        let session = TickSession::next();
        if let Some(queue) = self.lock().get_mut(plugin) {
            queue.sessions.insert(panel.to_owned(), session);
        }
        session
    }

    /// The instance that opened `session` stops ticking plugin `plugin`'s
    /// panel `panel` (it lost focus, was hidden, or its window closed): the
    /// session ends, so a tick of it not yet in the plugin never goes. One
    /// still queued is taken out at once; one the host has taken already
    /// is dropped when the host asks ([`PanelEvents::tick_is_current`]).
    /// Another instance's session, one opened since, is left alone.
    pub(crate) fn close_ticks(&self, plugin: &str, panel: &str, session: TickSession) {
        let mut shared = self.lock();
        let Some(queue) = shared.get_mut(plugin) else {
            return;
        };
        if queue.sessions.get(panel) == Some(&session) {
            queue.sessions.remove(panel);
        }
        if queue.in_flight.get(panel) != Some(&session) {
            return;
        }
        let before = queue.events.len();
        (queue.events).retain(|queued| {
            !(queued.panel == panel && matches!(queued.event, PanelEvent::Tick(_)))
        });
        if queue.events.len() != before {
            queue.release_tick(panel);
        }
    }

    /// Whether the host, about to hand plugin `plugin` a tick of its panel
    /// `panel`, still should: the session it was queued in hasn't ended.
    /// When it has, the tick is dropped here and the next may go.
    pub fn tick_is_current(&self, plugin: &str, panel: &str) -> bool {
        let mut shared = self.lock();
        let Some(queue) = shared.get_mut(plugin) else {
            return false;
        };
        let open = queue.sessions.get(panel);
        if open.is_some() && queue.in_flight.get(panel) == open {
            return true;
        }
        queue.release_tick(panel);
        false
    }

    /// The host has handed the plugin `plugin` `event` of its panel
    /// `panel`, and the call has come back, however it ended: after a
    /// tick, the next may go.
    pub fn delivered(&self, plugin: &str, panel: &str, event: &PanelEvent) {
        if !matches!(event, PanelEvent::Tick(_)) {
            return;
        }
        if let Some(queue) = self.lock().get_mut(plugin) {
            queue.release_tick(panel);
        }
    }

    fn lock(&self) -> MutexGuard<'_, BTreeMap<String, Queue>> {
        self.shared.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// The colour `tone` names in the theme's tokens `c`.
pub(crate) fn tone_color(tone: Tone, c: &Tokens<Hsla>) -> Hsla {
    match tone {
        Tone::Normal => c.text.primary,
        Tone::Muted => c.text.muted,
        Tone::Accent => c.text.accent,
        Tone::Success => c.status.success,
        Tone::Warning => c.status.warning,
        Tone::Error => c.status.error,
    }
}

/// A badge's ground for `tone`: the signal's soft ground, or a neutral one.
fn tone_ground(tone: Tone, c: &Tokens<Hsla>) -> Hsla {
    match tone {
        Tone::Normal | Tone::Muted => c.state.hover,
        Tone::Accent => c.accent.subtle,
        Tone::Success => c.status.success_soft,
        Tone::Warning => c.status.warning_soft,
        Tone::Error => c.status.error_soft,
    }
}

/// The icon of kiln-ui's set named `name`, as Lucide or Tabler name it.
pub(crate) fn icon_named(name: &str) -> Option<Icon> {
    Icon::ALL.iter().copied().find(|icon| icon.name() == name)
}

/// The AccessKit role a node of `kind` has. A list holding tree items is a
/// tree ([`is_tree`]); the caller says so with `tree`. A list's row that
/// isn't a tree item is drawn in a list box option, its content keeping its
/// own role inside it.
pub(crate) fn role_of(kind: &NodeKind, tree: bool) -> Role {
    match kind {
        NodeKind::Column | NodeKind::Row => Role::Group,
        NodeKind::Scroll => Role::ScrollView,
        NodeKind::Heading(_) => Role::Heading,
        NodeKind::Text(_) | NodeKind::Badge(_) => Role::Label,
        NodeKind::Icon(_) => Role::Image,
        NodeKind::Button(_) => Role::Button,
        NodeKind::Toggle(_) => Role::CheckBox,
        NodeKind::Input(_) => Role::TextInput,
        NodeKind::List(_) if tree => Role::Tree,
        NodeKind::List(_) => Role::ListBox,
        NodeKind::TreeItem(_) => Role::TreeItem,
        NodeKind::Separator => Role::Splitter,
        NodeKind::Progress(_) => Role::ProgressIndicator,
        // One region, read by its label, not cell by cell (T2006).
        NodeKind::Grid(_) => Role::Image,
    }
}

/// Whether the list at `index` of `view` is a tree: it holds a tree item.
pub(crate) fn is_tree(view: &View, index: usize) -> bool {
    view.nodes[index]
        .children
        .iter()
        .any(|&child| matches!(view.nodes[child as usize].kind, NodeKind::TreeItem(_)))
}

/// The accessible name of `node`, if it has one ([`role_of`]'s module docs):
/// a label, a text, an icon's name, or "Progress".
pub(crate) fn name_of(node: &Node) -> Option<SharedString> {
    let name: SharedString = match &node.kind {
        NodeKind::Column | NodeKind::Row | NodeKind::Scroll | NodeKind::Separator => return None,
        NodeKind::Heading(text) | NodeKind::Icon(text) => text.clone().into(),
        NodeKind::Text(text) | NodeKind::Badge(text) => text.text.clone().into(),
        NodeKind::Button(button) => button.label.clone().into(),
        NodeKind::Toggle(toggle) => toggle.label.clone().into(),
        NodeKind::Input(input) => input.label.clone().into(),
        NodeKind::List(list) => list.label.clone().into(),
        NodeKind::TreeItem(item) => item.label.clone().into(),
        NodeKind::Progress(_) => tr!("workbench-extensions-panel-progress"),
        NodeKind::Grid(grid) => grid.label.clone().into(),
    };
    Some(name)
}

/// A row's name in a list: its node's, or else its first descendant's.
fn row_name(view: &View, index: usize) -> SharedString {
    let mut stack = vec![index];
    while let Some(at) = stack.pop() {
        let node = &view.nodes[at];
        if let Some(name) = name_of(node) {
            return name;
        }
        stack.extend(node.children.iter().rev().map(|&child| child as usize));
    }
    SharedString::default()
}

/// The element id of node `id` of a plugin panel's view, which its
/// accessibility node goes by.
/// What the tests find node `id`'s row or field by
/// (`VisualTestContext::debug_bounds`).
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn selector(id: &str) -> String {
    format!(verbatim!("plugin-node:{}"), id)
}

pub(crate) fn element_id(id: &str) -> ElementId {
    ElementId::Name(format!(verbatim!("plugin-node:{}"), id).into())
}

/// The dock's id of plugin `plugin`'s panel `panel`: `<plugin>.<panel>`,
/// interned, as a [`PanelId`] holds a `&'static str`. A plugin declares at
/// most 8 panels, so the table stays small.
pub(crate) fn panel_id(plugin: &str, panel: &str) -> PanelId {
    static IDS: OnceLock<Mutex<HashSet<&'static str>>> = OnceLock::new();
    let id = format!("{plugin}.{panel}");
    let mut ids = IDS
        .get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    if let Some(interned) = ids.get(id.as_str()) {
        return PanelId(interned);
    }
    let interned: &'static str = Box::leak(id.into_boxed_str());
    ids.insert(interned);
    PanelId(interned)
}

/// One row a list draws: node `node` of the view at `depth` (0 for the
/// list's own children), `position` (from 0) among `siblings`, under the row
/// `parent`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Row {
    node: usize,
    depth: usize,
    position: usize,
    siblings: usize,
    parent: Option<usize>,
}

/// A list's state, kept by its node id across views: its rows, as last
/// flattened, the selected rows' ids, the node of the row the keys act on
/// and of the row a range selection starts at, focus and scroll. A run of
/// tree items outside a list is a list too, by its first item's id.
///
/// The cursor and the anchor are node ids, not row numbers: a new view can
/// reorder, add or take out rows, and a row number of the last one would
/// name another row, or none.
struct ListState {
    rows: Vec<Row>,
    selected: Vec<String>,
    cursor: Option<String>,
    anchor: Option<String>,
    focus: FocusHandle,
    /// Whether every row is a tree item's, one row high: drawn by
    /// `uniform_list` and `scroll`. Else rows are measured, drawn by GPUI's
    /// `list` and `measured`, as a row may hold wrapped text, a column or
    /// another list.
    uniform: bool,
    scroll: UniformListScrollHandle,
    measured: MeasuredList,
    /// The rows' node ids, as last flattened: a new view's are compared
    /// with them.
    ids: Vec<String>,
    /// A measured row's own focus, by its node id: GPUI keeps drawing a
    /// row out of view while it holds focus (a control in it has it), so
    /// keys still reach that control. Focus given to the row itself goes to
    /// the list.
    row_focus: HashMap<String, RowFocus>,
    /// The measured list's top row (node id, pixel offset) while the list is
    /// drawn uniform: emptying the measured list resets its scroll, so the
    /// top is restored by node id when measured rows come back.
    parked_top: Option<(String, Pixels)>,
}

/// A measured row's focus, and what hands focus given to it to its list.
struct RowFocus {
    handle: FocusHandle,
    to_list: Option<Subscription>,
}

impl ListState {
    /// The row of node `id`, in `view`, which the rows were made from.
    fn row_of(&self, view: &View, id: &str) -> Option<usize> {
        (self.rows.iter()).position(|row| view.nodes[row.node].id == id)
    }

    /// The cursor's row, in `view`.
    fn cursor_row(&self, view: &View) -> Option<usize> {
        self.row_of(view, self.cursor.as_deref()?)
    }

    /// Scrolls row `ix` into view, if it isn't.
    fn reveal(&self, ix: usize) {
        match self.uniform {
            true => self.scroll.scroll_to_item(ix, ScrollStrategy::Nearest),
            // A row not drawn yet has no height to scroll by: it goes to the
            // top. One drawn is scrolled just into view.
            false => match self.measured.bounds_for_item(ix) {
                None if ix > self.measured.logical_scroll_top().item_ix => {
                    self.measured.scroll_to(ListOffset {
                        item_ix: ix,
                        offset_in_item: px(0.),
                    })
                }
                _ => self.measured.scroll_to_reveal_item(ix),
            },
        }
    }
}

/// An input's field, and the view's text, label and placeholder it last
/// took. A field with focus takes none of the view's changes, which wait
/// until it loses it.
struct InputState {
    field: Entity<TextField>,
    applied: String,
    label: String,
    placeholder: Option<String>,
    _events: Subscription,
}

/// A plugin's panel: what [`PanelViews`] holds for it, drawn, in a dock.
pub struct PluginPanel {
    plugin: String,
    panel: String,
    id: PanelId,
    title: SharedString,
    icon: Icon,
    views: PanelViews,
    events: PanelEvents,
    view: Option<View>,
    /// The revision of `view` ([`PanelViews::revision`]).
    revision: u64,
    focus: FocusHandle,
    lists: HashMap<String, ListState>,
    inputs: HashMap<String, InputState>,
    /// An input's text not sent yet, and the timer that sends it.
    typing: HashMap<String, Task<()>>,
    /// What the user set that the plugin's view hasn't caught up with: a
    /// toggle's value, a tree item's expansion. A new view clears them.
    toggled: HashMap<String, bool>,
    expanded: HashMap<String, bool>,
    /// Icon names already logged as unknown.
    unknown_icons: HashSet<String>,
    /// The dock its manifest names: `left` and `right` the sidebar, which
    /// `workbench.rail` puts on one side, and `bottom` the bottom panel.
    location: PanelLocation,
    /// Asks the dock holding it whether it shows; none in a test that
    /// draws it alone, where [`PluginPanel::set_shown`] says.
    shows: Option<Shows>,
    shown: bool,
    /// Its manifest's `tick_ms`: how often it asks to be ticked.
    tick_ms: Option<u32>,
    /// Whether its manifest asks for `keys`.
    keys: bool,
    /// Its plugin's load's realtime runtime ([`PanelEvents::runtime`]), for
    /// a realtime panel.
    runtime: Option<Arc<PanelRuntime>>,
    /// The timer ticking it, while it's shown and focused, and its session.
    ticks: Option<(Task<()>, TickSession)>,
    /// When it last ticked, or its timer started.
    last_tick: Option<Instant>,
    /// Its plugin's load went while it ticked: it never ticks again.
    load_gone: bool,
    /// What starts and stops its timer as focus comes and goes, set when
    /// it's first drawn, with a window.
    focus_watch: Vec<Subscription>,
    /// Each grid as drawn, by node id.
    #[cfg(test)]
    pub(crate) drawn_grids: HashMap<String, grid::DrawnGrid>,
    /// The colour each text-bearing node was drawn in, by node id.
    #[cfg(test)]
    pub(crate) drawn_colors: HashMap<String, Hsla>,
    /// The rows each list built when last drawn, by node id.
    #[cfg(test)]
    pub(crate) drawn_rows: HashMap<String, Range<usize>>,
    _poll: Task<()>,
}

impl PluginPanel {
    /// Plugin `plugin`'s panel declared as `decl`, drawing what `views` holds
    /// for it and queueing the user's acts on `events`.
    pub fn new(
        plugin: &str,
        decl: &PanelDecl,
        views: PanelViews,
        events: PanelEvents,
        cx: &mut Context<Self>,
    ) -> Self {
        let icon = icon_named(&decl.icon).unwrap_or_else(|| {
            log::warn!(
                "plugin {plugin}: panel {}: no icon {:?} in the app's set",
                decl.id,
                decl.icon
            );
            Icon::Puzzle
        });
        let runtime = (decl.keys || decl.tick_ms.is_some()).then(|| events.runtime(plugin));
        // Released (its plugin went), it ends its tick session: a tick it
        // queued never reaches the plugin. Its window closing ends it sooner
        // (`watch_focus`).
        cx.on_release(|panel: &mut Self, _| panel.stop_ticks())
            .detach();
        let poll = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(MIN_UPDATE_INTERVAL).await;
                if this.update(cx, |panel, cx| panel.tick(cx)).is_err() {
                    break;
                }
            }
        });
        let mut panel = Self {
            plugin: plugin.to_owned(),
            panel: decl.id.clone(),
            id: panel_id(plugin, &decl.id),
            title: decl.title.clone().into(),
            icon,
            views,
            events,
            view: None,
            revision: 0,
            focus: cx.focus_handle(),
            lists: HashMap::new(),
            inputs: HashMap::new(),
            typing: HashMap::new(),
            toggled: HashMap::new(),
            expanded: HashMap::new(),
            unknown_icons: HashSet::new(),
            location: decl.location,
            shows: None,
            shown: false,
            tick_ms: decl.tick_ms,
            keys: decl.keys,
            runtime,
            ticks: None,
            last_tick: None,
            load_gone: false,
            focus_watch: Vec::new(),
            #[cfg(test)]
            drawn_grids: HashMap::new(),
            #[cfg(test)]
            drawn_colors: HashMap::new(),
            #[cfg(test)]
            drawn_rows: HashMap::new(),
            _poll: poll,
        };
        panel.refresh(cx);
        panel
    }

    pub fn plugin(&self) -> &str {
        &self.plugin
    }

    pub fn panel(&self) -> &str {
        &self.panel
    }

    /// Its id in the dock ([`panel_id`]).
    pub fn dock_id(&self) -> PanelId {
        self.id
    }

    /// The view it draws, as last read.
    pub fn view(&self) -> Option<&View> {
        self.view.as_ref()
    }

    /// The dock its manifest names.
    pub fn location(&self) -> PanelLocation {
        self.location
    }

    /// Whether it goes in the bottom panel rather than the sidebar.
    pub fn in_bottom_panel(&self) -> bool {
        self.location == PanelLocation::Bottom
    }

    /// Notes how to ask the dock it is in whether it shows.
    pub(crate) fn set_shown_by(&mut self, shows: impl Fn(&App) -> bool + 'static) {
        self.shows = Some(Box::new(shows));
    }

    /// Whether the dock shows it, as last noted.
    pub fn is_shown(&self) -> bool {
        self.shown
    }

    /// Notes whether it shows, telling the plugin when that changes.
    /// Hidden, it stops ticking.
    pub fn set_shown(&mut self, shown: bool, cx: &mut Context<Self>) {
        if shown == self.shown {
            return;
        }
        self.shown = shown;
        if !shown {
            self.stop_ticks();
        }
        self.send(if shown {
            PanelEvent::Shown
        } else {
            PanelEvent::Hidden
        });
        if shown {
            self.refresh(cx);
        }
    }

    /// Whether its manifest asks for keys or ticks.
    pub fn is_realtime(&self) -> bool {
        self.runtime.is_some()
    }

    /// Whether its tick timer runs.
    pub fn is_ticking(&self) -> bool {
        self.ticks.is_some()
    }

    /// Whether the dock shows it now, not as last noted: focus can come
    /// before the poll notes it shows.
    fn shows_now(&self, cx: &App) -> bool {
        self.shows.as_ref().map_or(self.shown, |shows| shows(cx))
    }

    /// Whether it may tick: shown, with focus in it, in the active window.
    fn may_tick(&self, window: &Window, cx: &App) -> bool {
        window.is_window_active() && self.focus.contains_focused(window, cx) && self.shows_now(cx)
    }

    /// Watches its focus and its window, once, for a realtime panel: taking
    /// focus starts the tick timer, losing it (the window's included) stops
    /// it, in the frame it moves, and so does its window closing, at once,
    /// whatever still holds the panel or its workbench.
    fn watch_focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.focus_watch.is_empty() || self.tick_ms.is_none() {
            return;
        }
        let focus = self.focus.clone();
        let mine = window.window_handle().window_id();
        let this = cx.weak_entity();
        self.focus_watch = vec![
            cx.on_focus_in(&focus, window, |panel, window, cx| {
                panel.start_ticks(window, cx)
            }),
            cx.on_focus_out(&focus, window, |panel, _, _, _| panel.stop_ticks()),
            // GPUI sends no focus-out as a window goes: its session ends here.
            cx.on_window_closed(move |cx, closed| {
                if closed == mine {
                    let _ = this.update(cx, |panel, _| panel.stop_ticks());
                }
            }),
        ];
    }

    /// Starts its one tick timer, if it ticks, may and has none.
    fn start_ticks(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ms) = self.tick_ms else {
            return;
        };
        if self.ticks.is_some() || self.load_gone || !self.may_tick(window, cx) {
            return;
        }
        let every = Duration::from_millis(u64::from(ms)).max(fe_plugin::panels::MIN_TICK_INTERVAL);
        self.last_tick = Some(cx.background_executor().now());
        let session = self.events.open_ticks(&self.plugin, &self.panel);
        let (events, plugin, panel) =
            (self.events.clone(), self.plugin.clone(), self.panel.clone());
        let timer = cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor().timer(every).await;
                let ticking = this.update_in(cx, |panel, window, cx| panel.tick_due(window, cx));
                match ticking {
                    Ok(true) => {}
                    Ok(false) => break,
                    // Its window closed, or the panel went: its session
                    // ends with the timer.
                    Err(_) => {
                        events.close_ticks(&plugin, &panel, session);
                        break;
                    }
                }
            }
        });
        self.ticks = Some((timer, session));
    }

    /// Drops its tick timer, ending its session: any tick of it not yet in
    /// the plugin goes too.
    fn stop_ticks(&mut self) {
        self.last_tick = None;
        if let Some((_, session)) = self.ticks.take() {
            self.events.close_ticks(&self.plugin, &self.panel, session);
        }
    }

    /// Its timer fired: a tick for the plugin, unless it may no longer tick
    /// or its load is gone, which stop the timer. False once stopped.
    fn tick_due(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let runtime = match &self.runtime {
            Some(runtime) if self.may_tick(window, cx) => runtime.clone(),
            _ => {
                self.stop_ticks();
                return false;
            }
        };
        let now = cx.background_executor().now();
        let since =
            (self.last_tick).map_or(Duration::ZERO, |last| now.saturating_duration_since(last));
        let ms = u32::try_from(since.as_millis()).unwrap_or(u32::MAX);
        let state = PanelState {
            shown: true,
            focused: true,
        };
        let Some(&(_, session)) = self.ticks.as_ref() else {
            return false;
        };
        match (self.events).tick(&self.plugin, &self.panel, session, &runtime, state, now, ms) {
            Ticked::Queued => {
                self.last_tick = Some(now);
                true
            }
            Ticked::Skipped(_) => true,
            Ticked::Gone => {
                self.load_gone = true;
                self.stop_ticks();
                false
            }
        }
    }

    /// A key in the panel that nothing in it took: the host's focus order
    /// for Tab; in a realtime panel, Escape to the editor; and at a panel
    /// with `keys` itself, not a control in it, any key without the
    /// secondary modifier to the plugin (the keymap has those with it).
    /// True if it took the key.
    fn key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let keystroke = &event.keystroke;
        let modifiers = keystroke.modifiers;
        // The host's focus order: a plugin can't keep focus.
        if keystroke.key == "tab" && !(modifiers.control || modifiers.alt || modifiers.platform) {
            match modifiers.shift {
                true => window.focus_prev(cx),
                false => window.focus_next(cx),
            }
            return true;
        }
        // Escape leaves a realtime panel for the editor from anywhere in
        // it, a control in it too, if the control left it.
        if self.runtime.is_some()
            && keystroke.key.eq_ignore_ascii_case("escape")
            && self.focus.contains_focused(window, cx)
        {
            let stack = keys::context_stack(&window.context_stack(), cx);
            let leave = Invocation::of(verbatim!("dock.focus_editor")).expect("a command id");
            run(leave.context(stack), window, cx);
            return true;
        }
        if !self.keys || !self.focus.is_focused(window) {
            return false;
        }
        let key = KeyEvent {
            key: keystroke.key.clone(),
            shift: modifiers.shift,
            ctrl: modifiers.control,
            alt: modifiers.alt,
            secondary: modifiers.secondary(),
        };
        // The host's: the keymap had it first ([`HOLDS_PLAIN_KEYS`]).
        if key.secondary {
            return false;
        }
        let Some(runtime) = &self.runtime else {
            return false;
        };
        let state = PanelState {
            shown: self.shown,
            focused: true,
        };
        match runtime.deliver_key(&self.panel, &key, state) {
            Delivery::Delivered => {
                self.send(PanelEvent::Key(key));
                true
            }
            Delivery::Skipped(_) => false,
        }
    }

    /// Every [`MIN_UPDATE_INTERVAL`]: notes whether the dock shows it, and
    /// while it does, reads its view.
    fn tick(&mut self, cx: &mut Context<Self>) {
        if let Some(shows) = &self.shows {
            let shown = shows(cx);
            self.set_shown(shown, cx);
        }
        if self.shown {
            self.refresh(cx);
        }
    }

    /// Shows the plugin's latest view: a held update once due, then
    /// whatever [`PanelViews`] holds, if the plugin showed a view since,
    /// even one equal to this: its answer to the user's toggle or
    /// expansion, which drops what the user set ([`PluginPanel::take_view`]).
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        self.views.flush(&self.plugin, &self.panel, Instant::now());
        let (latest, revision) = self.views.view_with_revision(&self.plugin, &self.panel);
        if revision != self.revision {
            self.revision = revision;
            self.take_view(latest, cx);
        }
    }

    /// Draws `view` from now on: the user's unconfirmed toggles and
    /// expansions are dropped, lists keep their selection where its rows
    /// remain, and inputs take their new text unless they have focus.
    fn take_view(&mut self, view: Option<View>, cx: &mut Context<Self>) {
        self.view = view;
        self.toggled.clear();
        self.expanded.clear();
        let Some(view) = &self.view else {
            self.lists.clear();
            self.inputs.clear();
            self.typing.clear();
            cx.notify();
            return;
        };
        let mut lists = HashSet::new();
        let mut inputs = HashSet::new();
        for (index, node) in view.nodes.iter().enumerate() {
            match &node.kind {
                NodeKind::List(_) => {
                    lists.insert(node.id.clone());
                }
                NodeKind::Input(_) => {
                    inputs.insert(node.id.clone());
                }
                _ => {}
            }
            // The tree item runs under it, a container's.
            if !matches!(node.kind, NodeKind::List(_) | NodeKind::TreeItem(_)) {
                for start in run_starts(view, &node.children) {
                    lists.insert(view.nodes[start].id.clone());
                }
            }
            if index == 0 && matches!(node.kind, NodeKind::TreeItem(_)) {
                lists.insert(node.id.clone());
            }
        }
        self.lists.retain(|id, _| lists.contains(id));
        self.inputs.retain(|id, _| inputs.contains(id));
        self.typing.retain(|id, _| inputs.contains(id));
        let ids: Vec<String> = lists.into_iter().collect();
        for id in ids {
            self.flatten(&id, cx);
        }
        cx.notify();
    }

    /// Queues `event` for the plugin.
    fn send(&self, event: PanelEvent) {
        if self.events.push(&self.plugin, &self.panel, event) {
            log::debug!(
                "plugin {}: panel events dropped, {} so far",
                self.plugin,
                self.events.dropped(&self.plugin)
            );
        }
    }

    /// The index of node `id` in the view.
    fn index_of(&self, id: &str) -> Option<usize> {
        self.view
            .as_ref()?
            .nodes
            .iter()
            .position(|node| node.id == id)
    }

    /// Whether tree item `index` shows its children: as the user last set
    /// it, else as the view says.
    fn is_expanded(&self, index: usize) -> bool {
        let Some(view) = &self.view else {
            return false;
        };
        let node = &view.nodes[index];
        match &node.kind {
            NodeKind::TreeItem(item) => (self.expanded.get(&node.id).copied())
                .unwrap_or(item.expanded && (item.expandable || !node.children.is_empty())),
            _ => false,
        }
    }

    /// Whether tree item `index` can be expanded: it says so, or has
    /// children.
    fn is_expandable(&self, index: usize) -> bool {
        let Some(view) = &self.view else {
            return false;
        };
        let node = &view.nodes[index];
        match &node.kind {
            NodeKind::TreeItem(item) => item.expandable || !node.children.is_empty(),
            _ => false,
        }
    }

    /// Rebuilds list `id`'s rows: its children (a run of tree items, its
    /// items), each expanded tree item's children after it. The cursor and
    /// the selection stay on their nodes while those are still shown.
    fn flatten(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(index) = self.index_of(id) else {
            return;
        };
        let mut rows = Vec::new();
        {
            let view = self.view.as_ref().expect("index_of found it");
            let Some(children) = list_roots(view, index) else {
                return;
            };
            let children = &children;
            let mut stack: Vec<(usize, usize, usize, usize, Option<usize>)> = children
                .iter()
                .enumerate()
                .rev()
                .map(|(position, &child)| (child as usize, 0, position, children.len(), None))
                .collect();
            while let Some((node, depth, position, siblings, parent)) = stack.pop() {
                let me = rows.len();
                rows.push(Row {
                    node,
                    depth,
                    position,
                    siblings,
                    parent,
                });
                if self.is_expanded(node) {
                    let children = &view.nodes[node].children;
                    stack.extend(children.iter().enumerate().rev().map(|(position, &child)| {
                        (
                            child as usize,
                            depth + 1,
                            position,
                            children.len(),
                            Some(me),
                        )
                    }));
                }
            }
        }
        let multi = self.is_multi(id);
        let view = self.view.as_ref().expect("index_of found it");
        let focus = cx.focus_handle().tab_stop(true);
        let state = self
            .lists
            .entry(id.to_owned())
            .or_insert_with(|| ListState {
                rows: Vec::new(),
                selected: Vec::new(),
                cursor: None,
                anchor: None,
                focus,
                uniform: true,
                scroll: UniformListScrollHandle::new(),
                measured: MeasuredList::new(0, ListAlignment::Top, px(200.)),
                ids: Vec::new(),
                row_focus: HashMap::new(),
                parked_top: None,
            });
        let ids: HashSet<&str> = rows
            .iter()
            .map(|row| view.nodes[row.node].id.as_str())
            .collect();
        state.selected.retain(|id| ids.contains(id.as_str()));
        let shown =
            |id: &Option<String>| id.as_ref().filter(|id| ids.contains(id.as_str())).cloned();
        state.cursor = shown(&state.cursor);
        state.anchor = shown(&state.anchor);
        if !multi && state.selected.len() > 1 {
            // A list made single-select keeps one of its selected rows: the
            // cursor's if it's one, the first if not.
            let kept = (state.cursor.clone())
                .filter(|id| state.selected.contains(id))
                .unwrap_or_else(|| state.selected[0].clone());
            state.selected = vec![kept.clone()];
            state.cursor = Some(kept.clone());
            state.anchor = Some(kept);
        }
        state.uniform =
            (rows.iter()).all(|row| matches!(view.nodes[row.node].kind, NodeKind::TreeItem(_)));
        let new_ids: Vec<String> = (rows.iter())
            .map(|row| view.nodes[row.node].id.clone())
            .collect();
        if state.uniform {
            // Drawn by `uniform_list` now: the measured list is emptied, so
            // measured rows coming back are registered again, with focus.
            state.row_focus.clear();
            if state.measured.item_count() > 0 {
                let top = state.measured.logical_scroll_top();
                state.parked_top =
                    (state.ids.get(top.item_ix)).map(|id| (id.clone(), top.offset_in_item));
            }
            state.measured.splice(0..state.measured.item_count(), 0);
        } else if new_ids == state.ids && state.measured.item_count() == rows.len() {
            // The same rows: measured again where they are, as a new view
            // may change what's in them, the top row's offset kept as it was
            // (not scaled with the row's new height).
            state.measured.remeasure_items(0..rows.len());
        } else {
            // Other rows: the one at the top stays at the top, by its node.
            let mut top = state.measured.logical_scroll_top();
            let mut top_id = state.ids.get(top.item_ix).cloned();
            if let Some((id, offset)) = state.parked_top.take() {
                // Back from uniform: the top the measured list had before.
                top_id = Some(id);
                top.offset_in_item = offset;
            }
            let kept = top_id.and_then(|id| new_ids.iter().position(|new| *new == id));
            state.row_focus.retain(|id, _| ids.contains(id.as_str()));
            for id in &new_ids {
                (state.row_focus.entry(id.clone())).or_insert_with(|| RowFocus {
                    handle: cx.focus_handle(),
                    to_list: None,
                });
            }
            let handles = (new_ids.iter())
                .map(|id| Some(state.row_focus[id].handle.clone()))
                .collect::<Vec<_>>();
            state
                .measured
                .splice_focusable(0..state.measured.item_count(), handles);
            state.measured.scroll_to(match kept {
                Some(item_ix) => ListOffset {
                    item_ix,
                    offset_in_item: top.offset_in_item,
                },
                None => ListOffset {
                    item_ix: top.item_ix.min(rows.len()),
                    offset_in_item: px(0.),
                },
            });
        }
        state.ids = new_ids;
        state.rows = rows;
    }

    /// Whether list `list` is a list node that lets the user select more
    /// than one row.
    fn is_multi(&self, list: &str) -> bool {
        self.index_of(list).is_some_and(|index| {
            matches!(&self.view.as_ref().expect("found").nodes[index].kind,
                NodeKind::List(list) if list.multi_select)
        })
    }

    /// The node ids of list `list`'s rows `rows`.
    fn row_ids(&self, list: &str, rows: impl IntoIterator<Item = usize>) -> Vec<String> {
        let (Some(view), Some(state)) = (&self.view, self.lists.get(list)) else {
            return Vec::new();
        };
        rows.into_iter()
            .filter_map(|ix| state.rows.get(ix))
            .map(|row| view.nodes[row.node].id.clone())
            .collect()
    }

    /// Moves list `list`'s cursor to row `ix`, selecting that row alone, as
    /// a click or an arrow key does.
    fn select_row(&mut self, list: &str, ix: usize, cx: &mut Context<Self>) {
        let Some(id) = self.row_ids(list, [ix]).pop() else {
            return;
        };
        let Some(state) = self.lists.get_mut(list) else {
            return;
        };
        state.cursor = Some(id.clone());
        state.anchor = Some(id.clone());
        state.reveal(ix);
        if state.selected != [id.clone()] {
            state.selected = vec![id];
            let selected = state.selected.clone();
            self.send(PanelEvent::Selected(list.to_owned(), selected));
        }
        cx.notify();
    }

    /// Adds row `ix` to multi-select list `list`'s selection, or takes it
    /// out, moving the cursor to it.
    fn toggle_row(&mut self, list: &str, ix: usize, cx: &mut Context<Self>) {
        let Some(id) = self.row_ids(list, [ix]).pop() else {
            return;
        };
        let Some(state) = self.lists.get_mut(list) else {
            return;
        };
        state.cursor = Some(id.clone());
        state.anchor = Some(id.clone());
        state.reveal(ix);
        match state.selected.iter().position(|selected| *selected == id) {
            Some(at) => {
                state.selected.remove(at);
            }
            None => state.selected.push(id),
        }
        let selected = state.selected.clone();
        self.send(PanelEvent::Selected(list.to_owned(), selected));
        cx.notify();
    }

    /// Activates list `list`'s row `ix`: Enter, or a double click.
    fn activate_row(&mut self, list: &str, ix: usize, cx: &mut Context<Self>) {
        if let Some(id) = self.row_ids(list, [ix]).pop() {
            self.send(PanelEvent::Activated(id));
            cx.notify();
        }
    }

    /// Expands or collapses the tree item at list `list`'s row `ix`,
    /// telling the plugin, and shows it so until the plugin's next view.
    fn expand_row(&mut self, list: &str, ix: usize, expand: bool, cx: &mut Context<Self>) {
        let Some(row) = self
            .lists
            .get(list)
            .and_then(|state| state.rows.get(ix).copied())
        else {
            return;
        };
        if !self.is_expandable(row.node) || self.is_expanded(row.node) == expand {
            return;
        }
        let Some(id) = self.row_ids(list, [ix]).pop() else {
            return;
        };
        self.expanded.insert(id.clone(), expand);
        self.send(PanelEvent::Expanded(id, expand));
        self.flatten(list, cx);
        cx.notify();
    }

    /// A key in list `list`, which has focus. In a multi-select list,
    /// Shift with a move extends the selection from its anchor to where the
    /// cursor goes, the platform's command key with a move moves the cursor
    /// alone, and Space (with or without that key) adds the cursor's row or
    /// takes it out.
    fn list_key(&mut self, list: &str, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        let modifiers = keystroke.modifiers;
        let multi = self.is_multi(list);
        let (extend, cursor_only) = match modifiers {
            modifiers if !modifiers.modified() => (false, false),
            modifiers if multi && modifiers == Modifiers::shift() => (true, false),
            modifiers if multi && modifiers == Modifiers::secondary_key() => (false, true),
            _ => return,
        };
        let (Some(state), Some(view)) = (self.lists.get(list), self.view.as_ref()) else {
            return;
        };
        let count = state.rows.len();
        if count == 0 {
            return;
        }
        let last = count - 1;
        let cursor = state.cursor_row(view);
        let row = cursor.and_then(|ix| state.rows.get(ix).copied());
        let modified = extend || cursor_only;
        let to = |ix: usize| Some(ix.min(last));
        let target = match keystroke.key.as_str() {
            "up" => cursor.map_or(to(last), |ix| to(ix.saturating_sub(1))),
            "down" => cursor.map_or(to(0), |ix| to(ix + 1)),
            "home" => to(0),
            "end" => to(last),
            "pageup" => cursor.map_or(to(0), |ix| to(ix.saturating_sub(PAGE_ROWS))),
            "pagedown" => cursor.map_or(to(0), |ix| to(ix + PAGE_ROWS)),
            "enter" if !modified => {
                let Some(ix) = cursor else { return };
                self.activate_row(list, ix, cx);
                cx.stop_propagation();
                return;
            }
            "space" if multi && !extend => {
                let Some(ix) = cursor else { return };
                self.toggle_row(list, ix, cx);
                cx.stop_propagation();
                return;
            }
            "right" if !modified => {
                let (Some(ix), Some(row)) = (cursor, row) else {
                    return;
                };
                if !self.is_expandable(row.node) {
                    return;
                }
                if !self.is_expanded(row.node) {
                    self.expand_row(list, ix, true, cx);
                    cx.stop_propagation();
                    return;
                }
                let state = &self.lists[list];
                let child = state
                    .rows
                    .get(ix + 1)
                    .filter(|next| next.parent == Some(ix));
                match child {
                    Some(_) => Some(ix + 1),
                    None => {
                        cx.stop_propagation();
                        return;
                    }
                }
            }
            "left" if !modified => {
                let (Some(ix), Some(row)) = (cursor, row) else {
                    return;
                };
                if self.is_expandable(row.node) && self.is_expanded(row.node) {
                    self.expand_row(list, ix, false, cx);
                    cx.stop_propagation();
                    return;
                }
                match row.parent {
                    Some(parent) => Some(parent),
                    None => return,
                }
            }
            _ => return,
        };
        if let Some(ix) = target {
            match (extend, cursor_only) {
                (true, _) => self.select_range(list, ix, cx),
                (_, true) => self.move_cursor(list, ix, cx),
                _ => self.select_row(list, ix, cx),
            }
        }
        cx.stop_propagation();
    }

    /// Moves list `list`'s cursor to row `ix`, leaving the selection.
    fn move_cursor(&mut self, list: &str, ix: usize, cx: &mut Context<Self>) {
        let Some(id) = self.row_ids(list, [ix]).pop() else {
            return;
        };
        let Some(state) = self.lists.get_mut(list) else {
            return;
        };
        state.cursor = Some(id);
        state.reveal(ix);
        cx.notify();
    }

    /// Moves list `list`'s cursor to row `ix`, selecting the rows from the
    /// anchor (the cursor's row if there's none) to it.
    fn select_range(&mut self, list: &str, ix: usize, cx: &mut Context<Self>) {
        let (Some(state), Some(view)) = (self.lists.get(list), self.view.as_ref()) else {
            return;
        };
        let from = (state.anchor.as_deref())
            .and_then(|anchor| state.row_of(view, anchor))
            .or_else(|| state.cursor_row(view))
            .unwrap_or(ix);
        let anchor = self.row_ids(list, [from]).pop();
        let range = from.min(ix)..=from.max(ix);
        let selected = self.row_ids(list, range);
        let Some(id) = self.row_ids(list, [ix]).pop() else {
            return;
        };
        let Some(state) = self.lists.get_mut(list) else {
            return;
        };
        state.cursor = Some(id);
        state.anchor = anchor;
        state.reveal(ix);
        if state.selected != selected {
            state.selected = selected.clone();
            self.send(PanelEvent::Selected(list.to_owned(), selected));
        }
        cx.notify();
    }

    /// A click on list `list`'s row `ix`: selects it (adds or takes it out,
    /// with the platform key, in a multi-select list), and a double click
    /// activates it.
    fn row_clicked(&mut self, list: &str, ix: usize, event: &ClickEvent, cx: &mut Context<Self>) {
        let multi = event.modifiers().platform && self.is_multi(list);
        if multi {
            self.toggle_row(list, ix, cx);
        } else {
            self.select_row(list, ix, cx);
        }
        if event.click_count() >= 2 {
            self.activate_row(list, ix, cx);
        }
    }

    /// The button `id` was pressed: tells the plugin, and runs its command
    /// if it is one of the plugin's own, as the palette would.
    fn press(&mut self, id: &str, command: Option<String>, cx: &mut Context<Self>) {
        self.send(PanelEvent::Activated(id.to_owned()));
        let Some(command) = command else {
            return;
        };
        let Ok(invocation) = Invocation::of(&command) else {
            log::warn!(
                "plugin {}: button {id:?}: bad command {command:?}",
                self.plugin
            );
            return;
        };
        let own = plugin_host::command_host(cx).is_some_and(|commands| {
            commands
                .registered(&self.plugin)
                .contains(&invocation.command)
        });
        if !own {
            log::warn!(
                "plugin {}: button {id:?} runs {command}, which isn't one of its commands",
                self.plugin
            );
            return;
        }
        plugin_host::dispatch(&RunCommand(invocation), cx);
    }

    /// The toggle `id` was switched to `on`.
    fn toggle(&mut self, id: &str, on: bool, cx: &mut Context<Self>) {
        self.toggled.insert(id.to_owned(), on);
        self.send(PanelEvent::Toggled(id.to_owned(), on));
        cx.notify();
    }

    /// Input `id`'s field changed: its text goes to the plugin once
    /// [`INPUT_DEBOUNCE`] passes without another change.
    fn typed(&mut self, id: &str, cx: &mut Context<Self>) {
        let id = id.to_owned();
        let timer = cx.spawn({
            let id = id.clone();
            async move |this, cx| {
                cx.background_executor().timer(INPUT_DEBOUNCE).await;
                let _ = this.update(cx, |panel, cx| {
                    panel.typing.remove(&id);
                    if let Some(text) = panel.input_text(&id, cx) {
                        panel.send(PanelEvent::Input(id, text));
                    }
                });
            }
        });
        self.typing.insert(id, timer);
    }

    /// Enter in input `id`: what's typed goes now, as submitted.
    fn submitted(&mut self, id: &str, cx: &mut Context<Self>) {
        self.typing.remove(id);
        if let Some(text) = self.input_text(id, cx) {
            self.send(PanelEvent::Submitted(id.to_owned(), text));
        }
    }

    fn input_text(&self, id: &str, cx: &App) -> Option<String> {
        let input = self.inputs.get(id)?;
        Some(input.field.read(cx).value().to_owned())
    }

    /// Input node `id`'s field, made on first draw. While the user isn't
    /// in it, it takes the view's `value` if that changed since it last
    /// took one, and is made again for a new `label` or `placeholder`
    /// (keeping its text), as a field can't change those.
    fn field(
        &mut self,
        id: &str,
        label: &str,
        value: &str,
        placeholder: Option<&str>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<TextField> {
        if let Some(input) = self.inputs.get_mut(id) {
            let field = input.field.clone();
            if field.read(cx).focus_handle(cx).is_focused(window) {
                return field;
            }
            // Left with typing not sent yet: it goes before the view's
            // changes replace it (a field made again here never tells of
            // its blur).
            if self.typing.remove(id).is_some() {
                let text = field.read(cx).value().to_owned();
                self.send(PanelEvent::Input(id.to_owned(), text));
            }
            let Some(input) = self.inputs.get_mut(id) else {
                return field;
            };
            if input.applied != value {
                input.applied = value.to_owned();
                if field.read(cx).value() != value {
                    field.update(cx, |field, cx| field.set_value(value, cx));
                }
            }
            if input.label == label && input.placeholder.as_deref() == placeholder {
                return field;
            }
        }
        let text = match self.inputs.get(id) {
            Some(input) => input.field.read(cx).value().to_owned(),
            None => value.to_owned(),
        };
        let (name, hint) = (label.to_owned(), placeholder.map(str::to_owned));
        let field = cx.new(|cx| {
            let field = (TextField::new(window, cx).name(name).text(&text)).tab_stop(true);
            match hint {
                Some(hint) => field.placeholder(Text::verbatim(hint)),
                None => field,
            }
        });
        let node = id.to_owned();
        let events = cx.subscribe(
            &field,
            move |panel, _, event: &TextFieldEvent, cx| match event {
                TextFieldEvent::Changed => panel.typed(&node, cx),
                TextFieldEvent::Submitted => panel.submitted(&node, cx),
                TextFieldEvent::Blurred => panel.left_field(&node, cx),
                TextFieldEvent::Focused => {}
            },
        );
        self.inputs.insert(
            id.to_owned(),
            InputState {
                field: field.clone(),
                applied: value.to_owned(),
                label: label.to_owned(),
                placeholder: placeholder.map(str::to_owned),
                _events: events,
            },
        );
        field
    }

    /// The user left input `id`: what they typed goes now if it hadn't,
    /// and the view's changes held while they were in it apply.
    fn left_field(&mut self, id: &str, cx: &mut Context<Self>) {
        if self.typing.remove(id).is_some()
            && let Some(text) = self.input_text(id, cx)
        {
            self.send(PanelEvent::Input(id.to_owned(), text));
        }
        cx.notify();
    }

    /// The icon named `name`, logging an unknown one once.
    fn icon(&mut self, name: &str) -> Option<Icon> {
        let icon = icon_named(name);
        // At most MAX_LOGGED_ICONS names are kept, each cut to what an
        // icon name can be: a plugin publishing fresh names can't grow it.
        if icon.is_none()
            && self.unknown_icons.len() < MAX_LOGGED_ICONS
            && self
                .unknown_icons
                .insert(truncated(name, MAX_ICON_NAME_BYTES).to_owned())
        {
            log::warn!(
                "plugin {}: panel {}: no icon {name:?} in the app's set",
                self.plugin,
                self.panel
            );
        }
        icon
    }

    /// Notes that node `id` was drawn in `color`, for the tests.
    #[allow(unused_variables)]
    fn drew(&mut self, id: &str, color: Hsla) {
        #[cfg(test)]
        self.drawn_colors.insert(id.to_owned(), color);
    }

    /// Node `index` of the view, drawn, and what's under it.
    fn render_node(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(view) = &self.view else {
            return div().into_any_element();
        };
        let tree = matches!(view.nodes[index].kind, NodeKind::List(_)) && is_tree(view, index);
        let node = view.nodes[index].clone();
        let role = role_of(&node.kind, tree);
        let c = cx.theme().colors;
        let id = element_id(&node.id);
        // A container's children: each run of tree items one virtualized
        // tree.
        let children = |panel: &mut Self, window: &mut Window, cx: &mut Context<Self>| {
            let view = panel.view.as_ref().expect("drawn");
            let starts = run_starts(view, &node.children);
            let shown: Vec<usize> = (node.children.iter())
                .map(|&child| child as usize)
                .filter(|child| {
                    !matches!(view.nodes[*child].kind, NodeKind::TreeItem(_))
                        || starts.contains(child)
                })
                .collect();
            (shown.into_iter())
                .map(|child| panel.render_node(child, window, cx))
                .collect::<Vec<_>>()
        };
        match &node.kind {
            NodeKind::Column => {
                // The root, and a column holding a list or a scroll, fill
                // the height they're given, which the list or scroll takes
                // what's left of.
                let grows = index == 0
                    || (node.children.iter()).any(|&child| {
                        let kind = &self.view.as_ref().expect("drawn").nodes[child as usize].kind;
                        matches!(
                            kind,
                            NodeKind::List(_) | NodeKind::Scroll | NodeKind::TreeItem(_)
                        )
                    });
                let children = children(self, window, cx);
                node_container(div().id(id), window, cx, role)
                    .flex()
                    .flex_col()
                    .gap(space::S6)
                    .when(grows, |column| column.flex_1())
                    .min_h_0()
                    .children(children)
                    .into_any_element()
            }
            NodeKind::Row => {
                let children = children(self, window, cx);
                node_container(div().id(id), window, cx, role)
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(space::S6)
                    .children(children)
                    .into_any_element()
            }
            NodeKind::Scroll => {
                let children = children(self, window, cx);
                node_container(div().id(id), window, cx, role)
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .gap(space::S6)
                    .overflow_y_scroll()
                    .children(children)
                    .into_any_element()
            }
            NodeKind::Heading(text) => {
                let color = c.text.primary;
                self.drew(&node.id, color);
                a11y(div().id(id), window, cx, role, text.clone())
                    .text_size(UI_HEADING.size)
                    .line_height(UI_HEADING.line_height)
                    .font_weight(gpui::FontWeight(UI_HEADING.weight as f32))
                    .text_color(color)
                    .truncate()
                    .child(SharedString::from(text.clone()))
                    .into_any_element()
            }
            NodeKind::Text(text) => {
                let color = tone_color(text.tone, &c);
                self.drew(&node.id, color);
                let mono = text.mono.then(|| code_family(cx));
                a11y(div().id(id), window, cx, role, text.text.clone())
                    .text_size(if text.mono { CODE.size } else { UI_BODY.size })
                    .line_height(if text.mono {
                        CODE.line_height
                    } else {
                        UI_BODY.line_height
                    })
                    .when_some(mono, |div, family| div.font_family(family))
                    .text_color(color)
                    .map(|div| match text.wrap {
                        true => div.whitespace_normal(),
                        false => div.truncate(),
                    })
                    .child(SharedString::from(text.text.clone()))
                    .into_any_element()
            }
            NodeKind::Badge(text) => {
                let color = tone_color(text.tone, &c);
                self.drew(&node.id, color);
                let mono = text.mono.then(|| code_family(cx));
                a11y(div().id(id), window, cx, role, text.text.clone())
                    .flex_none()
                    .px(space::S6)
                    .rounded(px(8.))
                    .bg(tone_ground(text.tone, &c))
                    .text_size(fe_ui::UI_SMALL.size)
                    .line_height(px(16.))
                    .when_some(mono, |div, family| div.font_family(family))
                    .text_color(color)
                    .child(SharedString::from(text.text.clone()))
                    .into_any_element()
            }
            NodeKind::Icon(name) => {
                let icon = self.icon(name);
                let color = c.list.icon;
                self.drew(&node.id, color);
                a11y(div().id(id), window, cx, role, name.clone())
                    .flex_none()
                    .size(fe_ui::ICON_SIZE)
                    .children(icon.map(|icon| {
                        svg()
                            .path(icon.path())
                            .size(fe_ui::ICON_SIZE)
                            .text_color(color)
                    }))
                    .into_any_element()
            }
            NodeKind::Button(button) => {
                let icon = button.icon.as_deref().and_then(|name| self.icon(name));
                let (key, command) = (node.id.clone(), button.command.clone());
                Button::new(id, Text::verbatim(button.label.clone()))
                    .small()
                    .when_some(icon, Button::icon)
                    .disabled(!button.enabled)
                    .on_click(cx.listener(move |panel, _: &ClickEvent, _, cx| {
                        panel.press(&key, command.clone(), cx)
                    }))
                    .into_any_element()
            }
            NodeKind::Toggle(toggle) => {
                let on = self.toggled.get(&node.id).copied().unwrap_or(toggle.on);
                let key = node.id.clone();
                Checkbox::new(id)
                    .checked(on)
                    .label(toggle.label.clone())
                    .disabled(!toggle.enabled)
                    .on_change(
                        cx.listener(move |panel, on: &bool, _, cx| panel.toggle(&key, *on, cx)),
                    )
                    .into_any_element()
            }
            NodeKind::Input(input) => {
                let field = self.field(
                    &node.id,
                    &input.label,
                    &input.value,
                    input.placeholder.as_deref(),
                    window,
                    cx,
                );
                let at = node.id.clone();
                div()
                    .id(id)
                    .w_full()
                    .debug_selector(move || selector(&at))
                    .child(field)
                    .into_any_element()
            }
            // A tree item outside a list starts a run of them (the
            // containers draw only a run's first): a tree, as a list is.
            NodeKind::List(_) | NodeKind::TreeItem(_) => self.render_list(index, window, cx),
            NodeKind::Grid(grid_node) => {
                let (element, drawn) = grid::render_grid(&node.id, grid_node, window, cx);
                #[cfg(test)]
                self.drawn_grids.insert(node.id.clone(), drawn);
                #[cfg(not(test))]
                drop(drawn);
                element
            }
            NodeKind::Separator => a11y(div().id(id), window, cx, role, "")
                .flex_none()
                .h(px(1.))
                .w_full()
                .bg(c.border.subtle)
                .into_any_element(),
            NodeKind::Progress(percent) => {
                let name = tr!("workbench-extensions-panel-progress");
                let value = percent
                    .map(|percent| tr!("workbench-extensions-panel-percent", percent = percent));
                let states = States {
                    value,
                    ..States::default()
                };
                let fraction = percent.map_or(0.3, |percent| f32::from(percent) / 100.);
                let fill = match percent {
                    Some(_) => c.accent.base,
                    None => c.text.faint,
                };
                self.drew(&node.id, fill);
                a11y_with(div().id(id), window, cx, role, name, states)
                    .flex_none()
                    .w_full()
                    .h(px(4.))
                    .rounded(px(2.))
                    .bg(c.border.subtle)
                    .child(
                        div()
                            .h_full()
                            .rounded(px(2.))
                            .w(relative(fraction))
                            .bg(fill),
                    )
                    .into_any_element()
            }
        }
    }

    /// A tree item's row.
    fn tree_row(
        &mut self,
        row: Row,
        selected: bool,
        focused: bool,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> TreeRow {
        let c = cx.theme().colors;
        let node = self.view.as_ref().expect("drawn").nodes[row.node].clone();
        let NodeKind::TreeItem(item) = &node.kind else {
            unreachable!("a tree row is a tree item's");
        };
        let icon = item.icon.as_deref().and_then(|name| self.icon(name));
        let color = tone_color(item.tone, &c);
        self.drew(&node.id, color);
        let expandable = self.is_expandable(row.node);
        let expanded = self.is_expanded(row.node);
        TreeRow::new(
            element_id(&node.id),
            item.label.clone(),
            &UiTheme::default(),
        )
        .depth(row.depth)
        .when(expandable, |tree_row| tree_row.directory(expanded))
        .when_some(icon, TreeRow::icon)
        .label_color(color)
        .when_some(item.detail.clone(), |tree_row, detail| {
            tree_row.detail(detail, c.text.muted)
        })
        .selected(selected)
        .focused(focused)
        .in_set(row.position + 1, row.siblings)
    }

    /// List `index`, or the run of tree items `index` starts, virtualized:
    /// only the rows in view are built. A click in it focuses it, unless a
    /// control in a row took focus first (`track_focus` yields to one).
    fn render_list(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let view = self.view.as_ref().expect("drawn");
        let node = &view.nodes[index];
        let (id, role, label) = match &node.kind {
            NodeKind::List(list) => (
                element_id(&node.id),
                role_of(&node.kind, is_tree(view, index)),
                SharedString::from(list.label.clone()),
            ),
            // Its first item's id names that item's row.
            NodeKind::TreeItem(_) => (
                ElementId::Name(format!(verbatim!("plugin-tree:{}"), node.id).into()),
                Role::Tree,
                tr!("workbench-extensions-panel-tree"),
            ),
            _ => unreachable!("a list's, or a tree item's"),
        };
        let key = node.id.clone();
        if !self.lists.contains_key(&key) {
            self.flatten(&key, cx);
        }
        let Some(state) = self.lists.get(&key) else {
            return div().into_any_element();
        };
        let count = state.rows.len();
        let focus = state.focus.clone();
        let rows_key = key.clone();
        let rows = match state.uniform {
            true => uniform_list(
                ElementId::Name(format!(verbatim!("plugin-list-rows:{}"), key).into()),
                count,
                cx.processor(move |panel, range: Range<usize>, window, cx| {
                    panel.render_rows(&rows_key, range, window, cx)
                }),
            )
            .track_scroll(&state.scroll)
            .size_full()
            .into_any_element(),
            false => {
                #[cfg(test)]
                self.drawn_rows.remove(&key);
                let panel = cx.weak_entity();
                measured_list(state.measured.clone(), move |ix, window, cx| {
                    let row = panel.update(cx, |panel, cx| {
                        #[cfg(test)]
                        {
                            let drawn =
                                (panel.drawn_rows.entry(rows_key.clone())).or_insert(ix..ix + 1);
                            *drawn = drawn.start.min(ix)..drawn.end.max(ix + 1);
                        }
                        panel.render_row(&rows_key, ix, window, cx)
                    });
                    row.ok()
                        .flatten()
                        .unwrap_or_else(|| div().into_any_element())
                })
                .size_full()
                .into_any_element()
            }
        };
        let on_key = key.clone();
        let list_focus = focus.clone();
        a11y(div().id(id), window, cx, role, label)
            .track_focus(&focus)
            .on_key_down(cx.listener(move |panel, event: &KeyDownEvent, window, cx| {
                // Keys of the list itself, not those bubbling up from a
                // control in one of its rows (or from a nested list).
                if list_focus.is_focused(window) {
                    panel.list_key(&on_key, event, cx)
                }
            }))
            .relative()
            .flex_1()
            .min_h(ROW_HEIGHT * 3.)
            .w_full()
            .child(rows)
            .into_any_element()
    }

    /// List `list`'s rows `range`, as the list scrolls them into view.
    fn render_rows(
        &mut self,
        list: &str,
        range: Range<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        #[cfg(test)]
        self.drawn_rows.insert(list.to_owned(), range.clone());
        (range.filter_map(|ix| self.render_row(list, ix, window, cx))).collect()
    }

    /// List `list`'s row `ix`: a tree item's one row high, another node's
    /// as high as it is, at least a row.
    fn render_row(
        &mut self,
        list: &str,
        ix: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let state = self.lists.get_mut(list)?;
        let focused = state.focus.is_focused(window);
        let row = *state.rows.get(ix)?;
        let id = &self.view.as_ref()?.nodes.get(row.node)?.id;
        let selected = state.selected.contains(id);
        let cursor = state.cursor.as_ref() == Some(id);
        let row_focus = state.row_focus.get_mut(id).map(|focus| {
            if focus.to_list.is_none() {
                let list_focus = state.focus.clone();
                focus.to_list = Some(cx.on_focus(&focus.handle, window, move |_, window, cx| {
                    window.focus(&list_focus, cx)
                }));
            }
            focus.handle.clone()
        });
        {
            let key = list.to_owned();
            let is_item = matches!(
                self.view.as_ref().expect("drawn").nodes[row.node].kind,
                NodeKind::TreeItem(_)
            );
            let element = if is_item {
                let tree_row = self.tree_row(row, selected, focused, window, cx);
                let expandable = self.is_expandable(row.node);
                let expanded = self.is_expanded(row.node);
                let tree_row = tree_row.active(cursor && focused).on_click(cx.listener(
                    move |panel, event: &ClickEvent, _, cx| panel.row_clicked(&key, ix, event, cx),
                ));
                // The chevron's own target, so a click on it expands or
                // collapses the item rather than selecting it.
                let key = list.to_owned();
                let chevron = expandable.then(|| {
                    div()
                        .id(("plugin-chevron", ix))
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .left(px(6.) + fe_ui::TREE_INDENT * row.depth as f32)
                        .w(fe_ui::CHEVRON_WIDTH)
                        .occlude()
                        .on_click(cx.listener(move |panel, _: &ClickEvent, _, cx| {
                            panel.expand_row(&key, ix, !expanded, cx)
                        }))
                });
                let at = self.view.as_ref().expect("drawn").nodes[row.node]
                    .id
                    .clone();
                div()
                    .relative()
                    .w_full()
                    .h(ROW_HEIGHT)
                    .debug_selector(move || selector(&at))
                    .child(tree_row)
                    .children(chevron)
                    .into_any_element()
            } else {
                let content = self.render_node(row.node, window, cx);
                let name = row_name(self.view.as_ref().expect("drawn"), row.node);
                let c = cx.theme().colors;
                let background = match (selected, focused) {
                    (true, true) => Some(c.list.row_selected),
                    (true, false) => Some(c.list.row_selected_inactive),
                    (false, _) => None,
                };
                let states = States {
                    selected: Some(selected),
                    position_in_set: Some(row.position + 1),
                    size_of_set: Some(row.siblings),
                    ..States::default()
                };
                let option =
                    ElementId::Name(format!(verbatim!("plugin-row:{}:{}"), list, ix).into());
                a11y_with(
                    div().id(option),
                    window,
                    cx,
                    Role::ListBoxOption,
                    name,
                    states,
                )
                .when(cursor && focused, |row| row.aria_active_descendant())
                .when_some(row_focus, |row, focus| row.track_focus(&focus))
                .debug_selector({
                    let at = self.view.as_ref().expect("drawn").nodes[row.node]
                        .id
                        .clone();
                    move || selector(&at)
                })
                .min_h(ROW_HEIGHT)
                .w_full()
                .flex()
                .items_center()
                .px(space::S8)
                .py(px(2.))
                .rounded(px(4.))
                .when_some(background, |row, background| row.bg(background))
                .hover(move |style| style.bg(c.list.row_hover))
                .on_click(cx.listener(move |panel, event: &ClickEvent, _, cx| {
                    panel.row_clicked(&key, ix, event, cx)
                }))
                .child(div().flex_1().min_w_0().child(content))
                .into_any_element()
            };
            Some(element)
        }
    }
}

/// What list node `index` lists: its children; or, for a tree item
/// outside a list, the run of tree items it starts among its siblings
/// (itself alone, as the root). None for another node.
fn list_roots(view: &View, index: usize) -> Option<Vec<u32>> {
    let node = &view.nodes[index];
    match node.kind {
        NodeKind::List(_) => Some(node.children.clone()),
        NodeKind::TreeItem(_) => {
            let siblings = (view.nodes.iter())
                .find(|parent| parent.children.contains(&(index as u32)))
                .map_or(&[][..], |parent| &parent.children[..]);
            let Some(at) = siblings.iter().position(|&child| child as usize == index) else {
                return Some(vec![index as u32]);
            };
            Some(
                (siblings[at..].iter())
                    .take_while(|&&child| {
                        matches!(view.nodes[child as usize].kind, NodeKind::TreeItem(_))
                    })
                    .copied()
                    .collect(),
            )
        }
        _ => None,
    }
}

/// Which of a container's `children` start a run of tree items: a tree
/// item after anything else, or first.
fn run_starts(view: &View, children: &[u32]) -> Vec<usize> {
    let item = |child: u32| matches!(view.nodes[child as usize].kind, NodeKind::TreeItem(_));
    (children.iter().enumerate())
        .filter(|&(at, &child)| item(child) && (at == 0 || !item(children[at - 1])))
        .map(|(_, &child)| child as usize)
        .collect()
}

/// `element` with `role` and no name: a container, which a screen reader
/// doesn't name.
fn node_container<E: gpui::StatefulInteractiveElement + gpui::Element>(
    element: E,
    window: &Window,
    cx: &mut App,
    role: Role,
) -> E {
    a11y(element, window, cx, role, "")
}

/// The code font's family: the editor's.
fn code_family(cx: &App) -> SharedString {
    (cx.try_global::<FontFamilies>().cloned().unwrap_or_default()).family(FamilySlot::Code, cx)
}

impl Focusable for PluginPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for PluginPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.theme().colors;
        // A realtime panel's timer follows its focus; one that took focus
        // before it was first drawn starts here.
        self.watch_focus(window, cx);
        if self.ticks.is_none() {
            self.start_ticks(window, cx);
        }
        let body = match &self.view {
            Some(_) => self.render_node(0, window, cx),
            None => div().into_any_element(),
        };
        let ring = self.keys && self.focus.is_focused(window);
        div()
            .id(ElementId::Name(
                format!(verbatim!("plugin-panel:{}"), self.id.0).into(),
            ))
            .track_focus(&self.focus)
            .when(self.keys, |root| {
                let mut context = KeyContext::default();
                context.add(KEYS_CONTEXT);
                context.add(HOLDS_PLAIN_KEYS);
                root.key_context(context)
            })
            .on_key_down(cx.listener(|panel, event: &KeyDownEvent, window, cx| {
                if panel.key_down(event, window, cx) {
                    cx.stop_propagation();
                }
            }))
            .relative()
            .when(ring, |root| {
                // Inside its edge: a dock clips what's drawn outside it.
                root.child(
                    div()
                        .absolute()
                        .inset_0()
                        .border(FOCUS_RING.width)
                        .border_color(c.state.focus_ring)
                        .debug_selector(|| verbatim!("plugin-panel-focus-ring").into()),
                )
            })
            .size_full()
            .flex()
            .flex_col()
            .p(space::S8)
            .bg(c.surface.panel)
            .text_color(c.text.primary)
            .text_size(UI_BODY.size)
            .child(body)
    }
}

impl DockItem for PluginPanel {
    fn id(&self) -> PanelId {
        self.id
    }

    fn title(&self, _: &App) -> SharedString {
        self.title.clone()
    }

    fn icon(&self, _: &App) -> Icon {
        self.icon
    }

    fn focus_on_show(&self) -> bool {
        true
    }
}

/// Every plugin's panel views, which the plugin host's panels slices set
/// and the panels read: one for the process, as the plugins' threads reach
/// it without an `App`.
pub(crate) fn views() -> &'static PanelViews {
    static VIEWS: OnceLock<PanelViews> = OnceLock::new();
    VIEWS.get_or_init(PanelViews::new)
}

/// Every plugin's panel events, which the panels queue and the plugin
/// host's threads deliver.
pub(crate) fn events() -> &'static PanelEvents {
    static EVENTS: OnceLock<PanelEvents> = OnceLock::new();
    EVENTS.get_or_init(PanelEvents::new)
}

/// The panels each running plugin declares, which every workbench window
/// shows, one opened later too.
#[derive(Default)]
struct Declared(BTreeMap<String, Vec<PanelDecl>>);

impl gpui::Global for Declared {}

/// Gives each workbench window made from now on the running plugins'
/// panels.
pub(crate) fn init(cx: &mut App) {
    cx.observe_new(|workbench: &mut Workbench, _, cx| {
        let declared = (cx.try_global::<Declared>()).map(|declared| declared.0.clone());
        for (plugin, decls) in declared.unwrap_or_default() {
            add_panels(workbench, &plugin, &decls, cx);
        }
    })
    .detach();
}

/// Plugin `plugin` is running, its manifest declaring `decls`: each window
/// shows them, in the docks they name.
pub(crate) fn attach(plugin: &str, decls: &[PanelDecl], cx: &mut App) {
    if decls.is_empty() {
        return;
    }
    let declared = &mut cx.default_global::<Declared>().0;
    declared.insert(plugin.to_owned(), decls.to_vec());
    for window in workbenches(cx) {
        let _ = window.update(cx, |workbench, _, cx| {
            add_panels(workbench, plugin, decls, cx)
        });
    }
}

/// Plugin `plugin`'s load is going: its views, its events and its panels
/// go with it, from every window.
pub(crate) fn unload(plugin: &str, cx: &mut App) {
    views().unload(plugin);
    events().unload(plugin);
    let had = (cx.try_global::<Declared>()).is_some_and(|declared| declared.0.contains_key(plugin));
    if !had {
        return;
    }
    cx.global_mut::<Declared>().0.remove(plugin);
    for window in workbenches(cx) {
        let _ = window.update(cx, |workbench, _, cx| {
            workbench.remove_plugin_panels(plugin, cx)
        });
    }
}

fn workbenches(cx: &App) -> Vec<gpui::WindowHandle<Workbench>> {
    (cx.windows().into_iter())
        .filter_map(|window| window.downcast::<Workbench>())
        .collect()
}

/// Puts plugin `plugin`'s panels `decls` in `workbench`'s docks.
fn add_panels(
    workbench: &mut Workbench,
    plugin: &str,
    decls: &[PanelDecl],
    cx: &mut Context<Workbench>,
) {
    for decl in decls {
        let panel =
            cx.new(|cx| PluginPanel::new(plugin, decl, views().clone(), events().clone(), cx));
        workbench.add_plugin_panel(panel, cx);
    }
}

/// Runs `invocation` from the panel, as a key bound to it would.
fn run(invocation: Invocation, window: &mut Window, cx: &mut App) {
    if let Err(error) = kiln_platform::command_actions::dispatch_invocation(invocation, window, cx)
    {
        log::warn!("{error}");
    }
}

#[path = "plugin_panel_grid.rs"]
mod grid;

#[cfg(test)]
#[path = "plugin_panel_tests.rs"]
mod plugin_panel_tests;

#[cfg(test)]
#[path = "plugin_panel_realtime_tests.rs"]
mod plugin_panel_realtime_tests;
