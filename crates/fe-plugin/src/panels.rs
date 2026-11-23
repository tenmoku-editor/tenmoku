//! Panels plugins contribute, kept by the host.
//!
//! A plugin with the `panels` capability sets the view of each panel its manifest declares, a
//! tree of nodes ([`View`]), and the [`PanelViews`] keep them for the docks to render; nothing
//! here draws. The store validates a view against the limits below BEFORE it takes it: an
//! invalid view is refused whole with `invalid-argument` and the panel keeps its last good one.
//! Updates are rate-capped per panel; one over the rate is validated, then held, the latest
//! replacing an earlier held one, until [`PanelViews::flush`] finds it due. Unloading a plugin
//! clears its views.
//!
//! [`PanelRuntime`] is the other direction: the ticks and keys the host delivers to a panel that
//! asked for them ([`PanelDecl`]'s `tick_ms` and `keys`, which need the `panels.realtime`
//! capability). The host owns focus, so nothing is delivered to a panel that is not shown and
//! focused, and nothing reserved is delivered at all; the renderer calls it and nothing here
//! draws or talks to a guest.
//!
//! [`PanelDecl`]: kiln_plugin::manifest::PanelDecl

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use kiln_command::CommandId;
use kiln_command::capability::{Capability, Grants};
use kiln_plugin::bindings::{Error, unimplemented};

use crate::HostState;
pub use crate::fastedit::editor::panels::{
    ButtonNode, Cell, GridNode, InputNode, ListNode, TextNode, ToggleNode, Tone, TreeItemNode, View,
};
use crate::fastedit::editor::panels::{Host, Node, NodeKind};
use unicode_segmentation::UnicodeSegmentation;

/// Nodes in one view.
pub const MAX_NODES: usize = 10_000;
/// Depth of a view; the root is depth 1.
pub const MAX_DEPTH: usize = 32;
/// Bytes of text (labels, ids, values, details) in one node.
pub const MAX_NODE_TEXT_BYTES: usize = 16 * 1024;
/// Bytes of text in one view.
pub const MAX_VIEW_TEXT_BYTES: usize = 1024 * 1024;
/// Children of one node: a list's rows are its children, so this is the list size. A view's node
/// cap bounds it too; this bounds one list a plugin could make of nearly all of them.
pub const MAX_CHILDREN: usize = 5_000;
/// Columns of a grid, and rows of one.
pub const MAX_GRID_SIDE: usize = 64;
/// Cells in one grid.
pub const MAX_GRID_CELLS: usize = 4096;
/// Grids in one view.
pub const MAX_GRIDS: usize = 4;
/// Panels one plugin may declare and show.
pub const MAX_PANELS_PER_PLUGIN: usize = 8;
/// Updates a second one panel takes.
pub const MAX_UPDATES_PER_SECOND: u32 = 20;
/// The least time between two updates of a panel.
pub const MIN_UPDATE_INTERVAL: Duration =
    Duration::from_millis(1000 / MAX_UPDATES_PER_SECOND as u64);

/// What a [`PanelViews::set_view_at`] or [`PanelViews::patch_at`] did with a valid update.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Update {
    /// The panel shows it.
    Applied,
    /// The panel updated too recently: it is held until [`PanelViews::flush`], or replaced by a
    /// later update.
    Coalesced,
}

/// Every plugin's panel views, which the app shares with every plugin's panels slice and with
/// its docks. Clones share it.
#[derive(Clone, Default)]
pub struct PanelViews {
    shared: Arc<Mutex<Shared>>,
}

#[derive(Default)]
struct Shared {
    loads: u64,
    plugins: BTreeMap<String, Plugin>,
}

struct Plugin {
    /// The number of the plugin's load.
    load: u64,
    panels: BTreeMap<String, Panel>,
}

#[derive(Default)]
struct Panel {
    shown: Option<View>,
    /// The number of `shown`, from [`next_revision`]; 0 before it has one.
    revision: u64,
    /// A valid view not shown yet, for the rate.
    pending: Option<View>,
    last_applied: Option<Instant>,
}

/// A number no shown view had before, in this process: one counter for every panel of every
/// load, so a reloaded plugin's first view isn't taken for its last load's.
fn next_revision() -> u64 {
    static REVISIONS: AtomicU64 = AtomicU64::new(0);
    REVISIONS.fetch_add(1, Ordering::Relaxed) + 1
}

impl PanelViews {
    pub fn new() -> Self {
        Self::default()
    }

    /// Plugin `plugin`'s panel `panel`'s view, if it has set one.
    pub fn view(&self, plugin: &str, panel: &str) -> Option<View> {
        let shared = self.lock();
        shared.plugins.get(plugin)?.panels.get(panel)?.shown.clone()
    }

    /// The revision of plugin `plugin`'s panel `panel`'s view: a new number each time a view
    /// is shown, even one equal to the last, which tells a renderer the plugin answered. 0 while
    /// it shows none.
    pub fn revision(&self, plugin: &str, panel: &str) -> u64 {
        self.view_with_revision(plugin, panel).1
    }

    /// [`PanelViews::view`] and [`PanelViews::revision`] together, as one read.
    pub fn view_with_revision(&self, plugin: &str, panel: &str) -> (Option<View>, u64) {
        let shared = self.lock();
        match shared
            .plugins
            .get(plugin)
            .and_then(|plugin| plugin.panels.get(panel))
        {
            Some(panel) => (panel.shown.clone(), panel.revision),
            None => (None, 0),
        }
    }

    /// Whether a valid update of the panel waits for [`PanelViews::flush`].
    pub fn has_pending(&self, plugin: &str, panel: &str) -> bool {
        let shared = self.lock();
        shared
            .plugins
            .get(plugin)
            .and_then(|plugin| plugin.panels.get(panel))
            .is_some_and(|panel| panel.pending.is_some())
    }

    /// Shows the panel's held update, if there is one and the panel may update at `now`. Whether
    /// it did.
    pub fn flush(&self, plugin: &str, panel: &str, now: Instant) -> bool {
        let mut shared = self.lock();
        let Some(panel) = shared
            .plugins
            .get_mut(plugin)
            .and_then(|plugin| plugin.panels.get_mut(panel))
        else {
            return false;
        };
        if panel.pending.is_none() || !due(panel, now) {
            return false;
        }
        panel.shown = panel.pending.take();
        panel.revision = next_revision();
        panel.last_applied = Some(now);
        true
    }

    /// Forgets plugin `plugin`: its views go, and its slice sets nothing more.
    pub fn unload(&self, plugin: &str) {
        self.lock().plugins.remove(plugin);
    }

    /// Attaches a new load of `plugin` with `declared` panels, none showing a view.
    fn attach(&self, plugin: &str, declared: &[String]) -> u64 {
        let mut shared = self.lock();
        shared.loads += 1;
        let load = shared.loads;
        let panels = declared
            .iter()
            .take(MAX_PANELS_PER_PLUGIN)
            .map(|id| (id.clone(), Panel::default()))
            .collect();
        shared
            .plugins
            .insert(plugin.to_owned(), Plugin { load, panels });
        load
    }

    /// Replaces `panel`'s view with `view`, at time `now`: validated first, so an invalid view
    /// changes nothing.
    pub fn set_view_at(
        &self,
        plugin: &str,
        load: u64,
        panel: &str,
        view: View,
        now: Instant,
    ) -> Result<Update, Error> {
        validate(&view)?;
        let mut shared = self.lock();
        let panel = panel_of(&mut shared, plugin, load, panel)?;
        Ok(accept(panel, view, now))
    }

    /// Replaces the subtree at node `id` of the panel's latest view (a held update, else the
    /// shown one) with `view`, at time `now`. Both `view` and the result are validated first.
    pub fn patch_at(
        &self,
        plugin: &str,
        load: u64,
        panel: &str,
        id: &str,
        view: View,
        now: Instant,
    ) -> Result<Update, Error> {
        validate(&view)?;
        let mut shared = self.lock();
        let panel = panel_of(&mut shared, plugin, load, panel)?;
        let Some(base) = panel.pending.as_ref().or(panel.shown.as_ref()) else {
            return Err(Error::NotFound(format!(
                "panel has no view to patch at {id:?}"
            )));
        };
        let merged = splice(base, id, &view)?;
        validate(&merged)?;
        Ok(accept(panel, merged, now))
    }

    /// Replaces rows `first_row..` of grid node `id` in the panel's latest view with `rows`, at
    /// time `now`; the other rows are kept. The result is validated first.
    pub fn set_grid_rows_at(
        &self,
        plugin: &str,
        load: u64,
        panel: &str,
        (id, first_row, rows): (&str, u8, Vec<Vec<Cell>>),
        now: Instant,
    ) -> Result<Update, Error> {
        let mut shared = self.lock();
        let panel = panel_of(&mut shared, plugin, load, panel)?;
        let Some(base) = panel.pending.as_ref().or(panel.shown.as_ref()) else {
            return Err(Error::NotFound(format!(
                "panel has no view to update the grid {id:?} of"
            )));
        };
        let mut merged = base.clone();
        let Some(node) = merged.nodes.iter_mut().find(|node| node.id == id) else {
            return Err(Error::NotFound(format!(
                "the panel's view has no node {id:?}"
            )));
        };
        let NodeKind::Grid(grid) = &mut node.kind else {
            return Err(invalid(format!("node {id:?} is not a grid")));
        };
        let (cols, first) = (usize::from(grid.cols), usize::from(first_row));
        if first + rows.len() > usize::from(grid.rows) {
            return Err(invalid(format!(
                "grid {:?}: {} rows from row {first} run past its {} rows",
                abbreviated(id),
                rows.len(),
                grid.rows
            )));
        }
        for (offset, row) in rows.into_iter().enumerate() {
            if row.len() != cols {
                return Err(invalid(format!(
                    "grid {:?}: row {} has {} cells, not {cols}",
                    abbreviated(id),
                    first + offset,
                    row.len()
                )));
            }
            let start = (first + offset) * cols;
            for (column, cell) in row.into_iter().enumerate() {
                grid.cells[start + column] = cell;
            }
        }
        validate(&merged)?;
        Ok(accept(panel, merged, now))
    }

    fn lock(&self) -> MutexGuard<'_, Shared> {
        self.shared.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

fn due(panel: &Panel, now: Instant) -> bool {
    panel
        .last_applied
        .is_none_or(|last| now.saturating_duration_since(last) >= MIN_UPDATE_INTERVAL)
}

fn accept(panel: &mut Panel, view: View, now: Instant) -> Update {
    if due(panel, now) {
        panel.shown = Some(view);
        panel.revision = next_revision();
        panel.pending = None;
        panel.last_applied = Some(now);
        Update::Applied
    } else {
        panel.pending = Some(view);
        Update::Coalesced
    }
}

fn panel_of<'a>(
    shared: &'a mut Shared,
    plugin: &str,
    load: u64,
    panel: &str,
) -> Result<&'a mut Panel, Error> {
    let Some(entry) = shared.plugins.get_mut(plugin).filter(|p| p.load == load) else {
        return Err(Error::Denied(format!("plugin {plugin} was unloaded")));
    };
    entry.panels.get_mut(panel).ok_or_else(|| {
        Error::NotFound(format!(
            "plugin {plugin} declares no panel {panel:?} in [[contributes.plugin.panels]]"
        ))
    })
}

fn invalid(what: impl std::fmt::Display) -> Error {
    Error::InvalidArgument(format!("invalid panel view: {what}"))
}

/// The strings of `node`, which count against its text limit.
fn texts(node: &Node) -> Vec<&str> {
    let mut texts = vec![node.id.as_str()];
    match &node.kind {
        NodeKind::Heading(text) | NodeKind::Icon(text) => texts.push(text),
        NodeKind::Text(text) | NodeKind::Badge(text) => texts.push(&text.text),
        NodeKind::Button(button) => {
            texts.push(&button.label);
            texts.extend(button.icon.as_deref());
            texts.extend(button.command.as_deref());
        }
        NodeKind::Toggle(toggle) => texts.push(&toggle.label),
        NodeKind::Input(input) => {
            texts.push(&input.label);
            texts.push(&input.value);
            texts.extend(input.placeholder.as_deref());
        }
        NodeKind::List(list) => texts.push(&list.label),
        NodeKind::TreeItem(item) => {
            texts.push(&item.label);
            texts.extend(item.icon.as_deref());
            texts.extend(item.detail.as_deref());
        }
        NodeKind::Column | NodeKind::Row | NodeKind::Scroll | NodeKind::Separator => {}
        NodeKind::Progress(_) => {}
        NodeKind::Grid(grid) => {
            texts.push(&grid.label);
            texts.extend(grid.cells.iter().map(|cell| cell.ch.as_str()));
        }
    }
    texts
}

/// The accessible name an interactive node must have.
fn label(node: &Node) -> Option<&str> {
    match &node.kind {
        NodeKind::Button(button) => Some(&button.label),
        NodeKind::Toggle(toggle) => Some(&toggle.label),
        NodeKind::Input(input) => Some(&input.label),
        NodeKind::List(list) => Some(&list.label),
        NodeKind::TreeItem(item) => Some(&item.label),
        NodeKind::Grid(grid) => Some(&grid.label),
        _ => None,
    }
}

fn may_have_children(kind: &NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Column
            | NodeKind::Row
            | NodeKind::Scroll
            | NodeKind::List(_)
            | NodeKind::TreeItem(_)
    )
}

/// Checks `view` against every limit and rule, changing nothing: `Err(invalid-argument)` names
/// the first it breaks.
pub fn validate(view: &View) -> Result<(), Error> {
    let nodes = &view.nodes;
    if nodes.is_empty() {
        return Err(invalid("it has no nodes, not even a root"));
    }
    if nodes.len() > MAX_NODES {
        return Err(invalid(format!(
            "{} nodes, the most a view may have is {MAX_NODES}",
            nodes.len()
        )));
    }
    let mut total = 0usize;
    let mut grids = 0usize;
    let mut ids = std::collections::HashSet::with_capacity(nodes.len());
    for node in nodes {
        let bytes: usize = texts(node).iter().map(|text| text.len()).sum();
        if bytes > MAX_NODE_TEXT_BYTES {
            return Err(invalid(format!(
                "node {:?} has {bytes} bytes of text, the most a node may have is {MAX_NODE_TEXT_BYTES}",
                abbreviated(&node.id)
            )));
        }
        total += bytes;
        if node.id.is_empty() {
            return Err(invalid("a node has an empty id"));
        }
        if !ids.insert(node.id.as_str()) {
            return Err(invalid(format!(
                "two nodes have the id {:?}",
                abbreviated(&node.id)
            )));
        }
        if label(node).is_some_and(|label| label.trim().is_empty()) {
            return Err(invalid(format!(
                "node {:?} is interactive and needs a non-empty label",
                node.id
            )));
        }
        if let NodeKind::Button(ButtonNode {
            command: Some(command),
            ..
        }) = &node.kind
        {
            CommandId::parse(command).map_err(|error| {
                invalid(format!("node {:?}: command {command:?}: {error}", node.id))
            })?;
        }
        if let NodeKind::Progress(Some(percent)) = &node.kind
            && *percent > 100
        {
            return Err(invalid(format!(
                "node {:?}: progress {percent} is over 100",
                node.id
            )));
        }
        if let NodeKind::Grid(grid) = &node.kind {
            validate_grid(&node.id, grid)?;
            grids += 1;
            if grids > MAX_GRIDS {
                return Err(invalid(format!(
                    "more than {MAX_GRIDS} grids, the most a view may have"
                )));
            }
        }
        if !node.children.is_empty() && !may_have_children(&node.kind) {
            return Err(invalid(format!("node {:?} cannot have children", node.id)));
        }
        if node.children.len() > MAX_CHILDREN {
            return Err(invalid(format!(
                "node {:?} has {} children, the most a node may have is {MAX_CHILDREN}",
                node.id,
                node.children.len()
            )));
        }
    }
    if total > MAX_VIEW_TEXT_BYTES {
        return Err(invalid(format!(
            "{total} bytes of text, the most a view may have is {MAX_VIEW_TEXT_BYTES}"
        )));
    }
    // The shape: a tree from node 0 reaching every node once, in at most MAX_DEPTH levels.
    let mut seen = vec![false; nodes.len()];
    seen[0] = true;
    let mut stack = vec![(0usize, 1usize)];
    let mut reached = 1usize;
    while let Some((index, depth)) = stack.pop() {
        if depth > MAX_DEPTH {
            return Err(invalid(format!("deeper than {MAX_DEPTH} levels")));
        }
        for &child in &nodes[index].children {
            let child = child as usize;
            if child >= nodes.len() {
                return Err(invalid(format!(
                    "node {:?} has a child {child} past the {} nodes",
                    nodes[index].id,
                    nodes.len()
                )));
            }
            if std::mem::replace(&mut seen[child], true) {
                return Err(invalid(format!(
                    "node {:?} is the child of more than one node, or of itself or its parent",
                    nodes[child].id
                )));
            }
            reached += 1;
            stack.push((child, depth + 1));
        }
    }
    if reached != nodes.len() {
        return Err(invalid(format!(
            "{} nodes are not under the root",
            nodes.len() - reached
        )));
    }
    Ok(())
}

/// A grid's size, and that each of its cells is one grapheme.
fn validate_grid(id: &str, grid: &GridNode) -> Result<(), Error> {
    let (cols, rows) = (usize::from(grid.cols), usize::from(grid.rows));
    if cols > MAX_GRID_SIDE || rows > MAX_GRID_SIDE || cols * rows > MAX_GRID_CELLS {
        return Err(invalid(format!(
            "grid {:?} is {cols}x{rows}, the most a grid may be is {MAX_GRID_SIDE}x{MAX_GRID_SIDE} and {MAX_GRID_CELLS} cells",
            abbreviated(id)
        )));
    }
    if grid.cells.len() != cols * rows {
        return Err(invalid(format!(
            "grid {:?} is {cols}x{rows} and needs {} cells, not {}",
            abbreviated(id),
            cols * rows,
            grid.cells.len()
        )));
    }
    for (index, cell) in grid.cells.iter().enumerate() {
        check_cell(id, index, cell)?;
    }
    Ok(())
}

fn check_cell(id: &str, index: usize, cell: &Cell) -> Result<(), Error> {
    if cell.ch.graphemes(true).count() != 1 {
        return Err(invalid(format!(
            "grid {:?} cell {index} must be exactly one grapheme",
            abbreviated(id)
        )));
    }
    Ok(())
}

/// A node id cut for a message.
fn abbreviated(id: &str) -> String {
    id.chars().take(64).collect()
}

/// `base` with the subtree at node `id` replaced by `patch`'s tree. Both are valid trees, so the
/// recursion is at most twice [`MAX_DEPTH`] deep.
fn splice(base: &View, id: &str, patch: &View) -> Result<View, Error> {
    if !base.nodes.iter().any(|node| node.id == id) {
        return Err(Error::NotFound(format!(
            "the panel's view has no node {id:?}"
        )));
    }
    fn copy(from: &View, index: usize, id: &str, patch: &View, out: &mut Vec<Node>) -> usize {
        if from.nodes[index].id == id {
            return copy_all(patch, 0, out);
        }
        let slot = out.len();
        let mut node = from.nodes[index].clone();
        out.push(node.clone());
        node.children = node
            .children
            .iter()
            .map(|&child| copy(from, child as usize, id, patch, out) as u32)
            .collect();
        out[slot] = node;
        slot
    }
    fn copy_all(from: &View, index: usize, out: &mut Vec<Node>) -> usize {
        let slot = out.len();
        out.push(from.nodes[index].clone());
        let children = from.nodes[index]
            .children
            .iter()
            .map(|&child| copy_all(from, child as usize, out) as u32)
            .collect();
        out[slot].children = children;
        slot
    }
    let mut nodes = Vec::new();
    copy(base, 0, id, patch, &mut nodes);
    Ok(View { nodes })
}

/// The panels slice of [`HostState`]. The default has no [`PanelViews`] and answers every
/// import with [`unimplemented`].
#[derive(Default)]
pub struct State {
    attached: Option<Attached>,
}

/// One load of a plugin's panels slice, which its interactive store and its worker stores share
/// ([`State::shared`]). Clones share it.
#[derive(Clone)]
pub struct SharedLoad(Attached);

#[derive(Clone)]
struct Attached {
    plugin: String,
    grants: Grants,
    views: PanelViews,
    load: u64,
}

impl State {
    /// Plugin `plugin`'s slice on `views`, as far as `grants` allow, with the panels its manifest
    /// `declared` by id. It is a new load of the plugin: an earlier load's views are dropped and
    /// that load's slice sets nothing more.
    pub fn new(plugin: &str, grants: &Grants, declared: &[String], views: &PanelViews) -> Self {
        Self {
            attached: Some(Attached {
                plugin: plugin.to_owned(),
                grants: grants.clone(),
                load: views.attach(plugin, declared),
                views: views.clone(),
            }),
        }
    }

    /// This load, as its worker stores' slices share it ([`State::worker`]); none for the
    /// default.
    pub fn shared(&self) -> Option<SharedLoad> {
        self.attached.clone().map(SharedLoad)
    }

    /// A worker store's slice of `shared`'s load: it attaches nothing, and sets the load's views
    /// as the load's own slice does.
    pub fn worker(shared: &SharedLoad) -> Self {
        Self {
            attached: Some(shared.0.clone()),
        }
    }

    /// [`Host::set_view`] with the time of the update given: a headless host's clock.
    pub fn set_view_at(&self, panel: &str, view: View, now: Instant) -> Result<Update, Error> {
        let attached = self.granted("fastedit:editor/panels.set-view")?;
        attached
            .views
            .set_view_at(&attached.plugin, attached.load, panel, view, now)
    }

    /// [`Host::patch`] with the time of the update given.
    pub fn patch_at(
        &self,
        panel: &str,
        id: &str,
        view: View,
        now: Instant,
    ) -> Result<Update, Error> {
        let attached = self.granted("fastedit:editor/panels.patch")?;
        attached
            .views
            .patch_at(&attached.plugin, attached.load, panel, id, view, now)
    }

    /// [`Host::set_grid_rows`] with the time of the update given.
    pub fn set_grid_rows_at(
        &self,
        panel: &str,
        id: &str,
        first_row: u8,
        rows: Vec<Vec<Cell>>,
        now: Instant,
    ) -> Result<Update, Error> {
        let attached = self.granted("fastedit:editor/panels.set-grid-rows")?;
        attached.views.set_grid_rows_at(
            &attached.plugin,
            attached.load,
            panel,
            (id, first_row, rows),
            now,
        )
    }

    /// The attachment, if its grants include `panels`, which `import` needs.
    fn granted(&self, import: &str) -> Result<&Attached, Error> {
        let Some(attached) = &self.attached else {
            return unimplemented(import);
        };
        if !attached.grants.allows(&Capability::Panels) {
            return Err(Error::Denied(format!(
                "{import} needs the {} capability",
                Capability::Panels
            )));
        }
        Ok(attached)
    }
}

impl Host for HostState {
    fn set_view(&mut self, panel: String, view: View) -> Result<(), Error> {
        self.panels
            .set_view_at(&panel, view, Instant::now())
            .map(drop)
    }

    fn patch(&mut self, panel: String, id: String, view: View) -> Result<(), Error> {
        self.panels
            .patch_at(&panel, &id, view, Instant::now())
            .map(drop)
    }

    fn set_grid_rows(
        &mut self,
        panel: String,
        id: String,
        first_row: u8,
        rows: Vec<Vec<Cell>>,
    ) -> Result<(), Error> {
        self.panels
            .set_grid_rows_at(&panel, &id, first_row, rows, Instant::now())
            .map(drop)
    }

    fn reveal(&mut self, _panel: String) -> Result<(), Error> {
        // Showing a dock is the app's: A4 lands it.
        self.panels.granted("fastedit:editor/panels.reveal")?;
        unimplemented("fastedit:editor/panels.reveal")
    }
}

/// Ticks one panel takes a second: the update rate of the design's §4.2, which a tick is one
/// source of, so a ticking panel cannot outpace what the docks will draw.
pub const MAX_TICKS_PER_SECOND: u32 = MAX_UPDATES_PER_SECOND;
/// The least time between two ticks of one panel.
pub const MIN_TICK_INTERVAL: Duration = MIN_UPDATE_INTERVAL;

/// The keys the host keeps, whatever a panel asks for, lower-case as the host names them:
/// `escape` returns focus to the editor and `tab` moves it on. A key with the `secondary`
/// modifier is the host's too, which is not a name and so is not in this list.
pub const RESERVED_KEYS: [&str; 2] = ["escape", "tab"];

/// What the renderer knows about a panel when it has an event for it. The host owns both: a
/// plugin cannot show, hide or focus its own panel, and so cannot trap focus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PanelState {
    pub shown: bool,
    pub focused: bool,
}

/// Why the host kept an event rather than delivering it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Skipped {
    /// The panel is not shown, or not focused.
    Unfocused,
    /// A call into the plugin for this panel has not come back. The tick is dropped and counted
    /// ([`PanelRuntime::dropped_ticks`]), never queued: a slow plugin falls behind the clock
    /// instead of building a backlog to be delivered late.
    Busy,
    /// Less than [`MIN_TICK_INTERVAL`] since the panel's last tick.
    TooSoon,
    /// One of [`RESERVED_KEYS`], or a key with the `secondary` modifier.
    Reserved,
}

/// What [`PanelRuntime::deliver_tick`] or [`PanelRuntime::deliver_key`] did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Delivery {
    /// The renderer should call the plugin's `panel-handler.on-event` with it.
    Delivered,
    Skipped(Skipped),
}

/// A key press as the host names it: `fastedit:editor/panel-handler`'s `key-event`, with no text
/// input. `secondary` is the platform's command modifier.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyEvent {
    /// The host's key name, as `escape` or `f1`; compared without case.
    pub key: String,
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub secondary: bool,
}

impl KeyEvent {
    /// Whether the host keeps this key: one of [`RESERVED_KEYS`], or any key with `secondary`.
    pub fn is_reserved(&self) -> bool {
        self.secondary
            || RESERVED_KEYS
                .iter()
                .any(|reserved| self.key.eq_ignore_ascii_case(reserved))
    }
}

/// One plugin load's realtime panels: what it is owed and what it is in the middle of. The
/// renderer holds one and calls it; clones share it. Nothing here calls a guest - a
/// [`Delivery::Delivered`] is the renderer's cue to make that call, under the same
/// [`PanelRuntime::call_deadline`] as any other.
#[derive(Clone, Default)]
pub struct PanelRuntime {
    panels: Arc<Mutex<BTreeMap<String, Realtime>>>,
}

#[derive(Default)]
struct Realtime {
    /// When the panel's last tick was delivered.
    last_tick: Option<Instant>,
    /// Whether a call for this panel is still in the plugin.
    in_call: bool,
    dropped: u64,
}

impl PanelRuntime {
    pub fn new() -> Self {
        Self::default()
    }

    /// How long a tick or a key has in the plugin: kiln-plugin's call deadline, the same one
    /// every other call gets. A panel that asked to be ticked buys no more time.
    pub fn call_deadline() -> Duration {
        kiln_plugin::limits::CALL_DEADLINE
    }

    /// Whether the renderer should deliver a tick to `panel` at `now`, given what it is showing.
    /// A tick goes only to a shown and focused panel, one at a time, and no sooner than
    /// [`MIN_TICK_INTERVAL`] after the last.
    pub fn deliver_tick(&self, panel: &str, state: PanelState, now: Instant) -> Delivery {
        if !state.shown || !state.focused {
            return Delivery::Skipped(Skipped::Unfocused);
        }
        let mut panels = self.lock();
        let realtime = panels.entry(panel.to_owned()).or_default();
        if realtime.in_call {
            realtime.dropped += 1;
            return Delivery::Skipped(Skipped::Busy);
        }
        if realtime
            .last_tick
            .is_some_and(|last| now.saturating_duration_since(last) < MIN_TICK_INTERVAL)
        {
            return Delivery::Skipped(Skipped::TooSoon);
        }
        realtime.in_call = true;
        realtime.last_tick = Some(now);
        Delivery::Delivered
    }

    /// Tells the runtime that the call carrying `panel`'s tick has come back, however it ended:
    /// returned, trapped, or stopped at its deadline. Until it does, ticks are dropped.
    pub fn tick_finished(&self, panel: &str) {
        self.lock().entry(panel.to_owned()).or_default().in_call = false;
    }

    /// Whether the renderer should deliver `key` to `panel`. A key goes only to a focused panel,
    /// and never one the host keeps ([`KeyEvent::is_reserved`]).
    pub fn deliver_key(&self, panel: &str, key: &KeyEvent, state: PanelState) -> Delivery {
        let _ = panel;
        if !state.focused {
            return Delivery::Skipped(Skipped::Unfocused);
        }
        if key.is_reserved() {
            return Delivery::Skipped(Skipped::Reserved);
        }
        Delivery::Delivered
    }

    /// How many of `panel`'s ticks fell due while a call for it was still in the plugin. They are
    /// gone, not queued; the count is what the host knows about a plugin falling behind.
    pub fn dropped_ticks(&self, panel: &str) -> u64 {
        self.lock()
            .get(panel)
            .map_or(0, |realtime| realtime.dropped)
    }

    fn lock(&self) -> MutexGuard<'_, BTreeMap<String, Realtime>> {
        self.panels.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
