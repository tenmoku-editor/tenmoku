//! The project tree's right-click menu, as VS Code's explorer and
//! JetBrains' project view offer one. Its items are the panel's own
//! `project_panel.*` commands, each run through [`ProjectPanel::run_op`] as
//! its key runs it, titled by the command's title and showing its key
//! right-aligned, read from the keymap as the menu is drawn.
//!
//! A right click on a row selects it (it doesn't open a file or toggle a
//! folder, as a left click does) and offers, on a folder: New File, New
//! Folder | Rename, Duplicate, Move to Trash | Cut, Copy, Paste | Copy Path,
//! Copy Relative Path, Reveal in Finder (the File Manager elsewhere), Open in
//! Terminal, Find in Folder | Collapse All Folders. A file's menu is the same
//! without New File and New Folder, and starts with Open and Open to the
//! Side; its Paste goes beside it, as the key's
//! does. A right click on the empty space below the rows selects nothing
//! and offers New File, New Folder | Paste | Refresh, each acting on the
//! root. Items that would do nothing now are left out, not disabled: Paste
//! with nothing cut or copied, or with a cut entry that would land where it
//! is.
//!
//! The menu is drawn where it was asked for, over the window. A click
//! outside it, Escape, focus leaving the panel, a refresh or its row going
//! closes it; Up and Down move its highlight, wrapping, Home and End go to
//! its first and last item, and Enter chooses it ([`PopupMenu`]). While
//! it's open the panel keeps focus, with [`MENU_KEY_CONTEXT`] for its key
//! context, so the tree's own keys (vim's included) wait. A right click while a name
//! field is open does nothing: the field keeps focus and commits nothing.

use fe_command::{ContextFrame, ContextStack};
use fe_project::{EntryKind, NodeId};
use fe_ui::{MenuEntry, MenuInput, PopupMenu, Role};
use gpui::{
    AnyElement, App, Context, IntoElement, KeyDownEvent, ParentElement as _, Pixels, Point,
    SharedString, Subscription, Window, anchored, deferred,
};
use kiln_l10n::{tr, verbatim};

use super::{KEY_CONTEXT, PanelOp, ProjectPanel};

/// The panel's key context while its menu is open: no binding names it,
/// so Escape, Up, Down and Enter reach the menu.
pub(super) const MENU_KEY_CONTEXT: &str = verbatim!("project_panel_menu");

/// An item of the menu: a panel command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MenuItem {
    Open,
    OpenToSide,
    NewFile,
    NewFolder,
    Rename,
    Duplicate,
    Delete,
    Cut,
    Copy,
    Paste,
    CopyPath,
    CopyRelativePath,
    RevealInOs,
    OpenInTerminal,
    FindInFolder,
    CollapseAll,
    Refresh,
}

use MenuItem::*;

/// A file row's menu starts with these, ahead of [`FOLDER_ITEMS`]' rows
/// from its Rename on; `None` is a separator.
const FILE_HEAD: &[Option<MenuItem>] = &[Some(Open), Some(OpenToSide), None];

/// A folder row's menu; `None` is a separator.
const FOLDER_ITEMS: &[Option<MenuItem>] = &[
    Some(NewFile),
    Some(NewFolder),
    None,
    Some(Rename),
    Some(Duplicate),
    Some(Delete),
    None,
    Some(Cut),
    Some(Copy),
    Some(Paste),
    None,
    Some(CopyPath),
    Some(CopyRelativePath),
    Some(RevealInOs),
    Some(OpenInTerminal),
    Some(FindInFolder),
    None,
    Some(CollapseAll),
];

/// The empty space's menu, below the rows.
const EMPTY_ITEMS: &[Option<MenuItem>] = &[
    Some(NewFile),
    Some(NewFolder),
    None,
    Some(Paste),
    None,
    Some(Refresh),
];

impl MenuItem {
    /// The op it runs.
    fn op(self) -> PanelOp {
        match self {
            Open => PanelOp::Open,
            OpenToSide => PanelOp::OpenToSide,
            NewFile => PanelOp::NewFile,
            NewFolder => PanelOp::NewFolder,
            Rename => PanelOp::Rename,
            Duplicate => PanelOp::Duplicate,
            Delete => PanelOp::Delete,
            Cut => PanelOp::Cut,
            Copy => PanelOp::Copy,
            Paste => PanelOp::Paste,
            CopyPath => PanelOp::CopyPath,
            CopyRelativePath => PanelOp::CopyRelativePath,
            RevealInOs => PanelOp::RevealInOs,
            OpenInTerminal => PanelOp::OpenInTerminal,
            FindInFolder => PanelOp::FindInFolder,
            CollapseAll => PanelOp::CollapseAll,
            Refresh => PanelOp::Refresh,
        }
    }

    /// The command it is, which titles it and finds its key.
    pub(crate) fn command(self) -> &'static str {
        match self {
            Open => "project_panel.open",
            OpenToSide => "project_panel.open_to_side",
            NewFile => "project_panel.new_file",
            NewFolder => "project_panel.new_folder",
            Rename => "project_panel.rename",
            Duplicate => "project_panel.duplicate",
            Delete => "project_panel.delete",
            Cut => "project_panel.cut",
            Copy => "project_panel.copy",
            Paste => "project_panel.paste",
            CopyPath => "project_panel.copy_path",
            CopyRelativePath => "project_panel.copy_relative_path",
            RevealInOs => "project_panel.reveal_in_os",
            OpenInTerminal => "project_panel.open_in_terminal",
            FindInFolder => "project_panel.find_in_folder",
            CollapseAll => "project_panel.collapse_all",
            Refresh => "project_panel.refresh",
        }
    }

    /// Its title: its command's, in the active locale.
    pub(crate) fn title(self) -> SharedString {
        if self == RevealInOs {
            return super::os::reveal_title();
        }
        fe_command::l10n::command_label(self.command(), "")
    }
}

/// The open menu.
pub(super) struct TreeMenu {
    /// The row it's for; `None` for the empty space.
    target: Option<NodeId>,
    /// Its items; `None` is a separator.
    entries: Vec<Option<MenuItem>>,
    /// The menu drawn for them, whose highlight Up and Down move, or the
    /// pointer does, and Enter chooses: none until either does.
    menu: PopupMenu,
    /// Where it was asked for, in the window.
    position: Point<Pixels>,
    /// Closes it when focus leaves the panel.
    _focus_out: Subscription,
}

/// What the menu draws for `entries`: each item's title and the key that
/// runs it where the tree has focus, as the tree has keys while the menu
/// isn't open.
fn menu_entries(entries: &[Option<MenuItem>], cx: &App) -> Vec<MenuEntry> {
    let stack = tree_context(cx);
    (entries.iter())
        .map(|entry| match entry {
            Some(item) => MenuEntry::Item {
                title: item.title(),
                shortcut: (crate::pane_toolbar::menu_shortcut(item.command(), &stack, cx))
                    .map(Into::into),
            },
            None => MenuEntry::Separator,
        })
        .collect()
}

/// The key context of the tree with focus, its menus closed, which their
/// items' keys are looked up in.
pub(super) fn tree_context(cx: &App) -> ContextStack {
    let mut frame = ContextFrame::new(KEY_CONTEXT);
    if fe_gpui::vim_enabled(cx) {
        frame = frame.flag("vim");
    }
    [ContextFrame::new(crate::KEY_CONTEXT), frame]
        .into_iter()
        .collect()
}

/// `items` without those `keep` leaves out, and without a separator
/// that would start or end the menu or follow another.
fn kept(items: &[Option<MenuItem>], keep: impl Fn(MenuItem) -> bool) -> Vec<Option<MenuItem>> {
    let mut entries: Vec<Option<MenuItem>> = Vec::new();
    for entry in items.iter().copied() {
        match entry {
            Some(item) if keep(item) => entries.push(entry),
            Some(_) => {}
            None if entries.last().is_some_and(Option::is_some) => entries.push(None),
            None => {}
        }
    }
    if entries.last() == Some(&None) {
        entries.pop();
    }
    entries
}

impl ProjectPanel {
    /// A right click at `position` on row `id`, or with `None` on the
    /// empty space below the rows: selects it (or nothing) and opens its
    /// menu. Nothing while a name field is open.
    pub(super) fn open_menu(
        &mut self,
        id: Option<NodeId>,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.edit.is_some() {
            return;
        }
        let Some(tree) = self.tree() else {
            return;
        };
        let items: Vec<Option<MenuItem>> = match id {
            Some(id) => match tree.row(id).map(|row| row.kind) {
                Some(EntryKind::Dir) => FOLDER_ITEMS.to_vec(),
                Some(_) => [FILE_HEAD, &FOLDER_ITEMS[3..]].concat(),
                None => return,
            },
            None => EMPTY_ITEMS.to_vec(),
        };
        self.cancel_reveal();
        self.selected = id;
        if let Some(ix) = self.selected_index() {
            self.selected_hint = ix;
        }
        let can_paste = self.can_paste();
        let entries = kept(&items, |item| item != Paste || can_paste);
        self.new_menu = None;
        window.focus(&self.focus, cx);
        let focus_out = cx.on_focus_out(&self.focus, window, |panel, _, _, cx| {
            panel.close_menu(cx);
        });
        let menu = PopupMenu::new(menu_entries(&entries, cx));
        self.menu = Some(TreeMenu {
            target: id,
            entries,
            menu,
            position,
            _focus_out: focus_out,
        });
        cx.notify();
    }

    /// Closes the open menu, if there's one, this one or the New menu
    /// ([`super::new_menu`]), leaving focus where it is.
    pub(super) fn close_menu(&mut self, cx: &mut Context<Self>) {
        let tree = self.menu.take().is_some();
        if self.new_menu.take().is_some() || tree {
            cx.notify();
        }
    }

    /// Closes the open menu if the row it's for is gone from the tree.
    pub(super) fn close_gone_menu(&mut self, cx: &mut Context<Self>) {
        let target = self.menu.as_ref().and_then(|menu| menu.target);
        let gone = target.is_some_and(|id| self.tree().is_none_or(|tree| tree.row(id).is_none()));
        if gone {
            self.close_menu(cx);
        }
    }

    /// The keys the open menu takes ([`PopupMenu::key`]): see the module
    /// docs. Other keys do nothing then.
    pub(super) fn on_menu_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(input) = (self.menu.as_mut()).and_then(|menu| menu.menu.key(event)) else {
            return;
        };
        cx.stop_propagation();
        self.on_menu_input(input, window, cx);
    }

    /// What the open menu's keys and pointer ask: see the module docs.
    fn on_menu_input(&mut self, input: MenuInput, window: &mut Window, cx: &mut Context<Self>) {
        match input {
            MenuInput::Handled => cx.notify(),
            MenuInput::Chosen(ix) => {
                let menu = self.menu.as_ref();
                if let Some(item) = menu.and_then(|menu| *menu.entries.get(ix)?) {
                    self.run_menu_item(item, window, cx);
                }
            }
            MenuInput::Escape | MenuInput::PressedOutside(_) => self.close_menu(cx),
        }
    }

    /// Closes the menu and runs `item` on its row (the root for the empty
    /// space's), as the item's key would.
    pub(super) fn run_menu_item(
        &mut self,
        item: MenuItem,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(menu) = self.menu.take() else {
            return;
        };
        cx.notify();
        self.selected = menu.target;
        self.run_op(item.op(), window, cx);
    }

    /// The open menu, drawn over the window where it was asked for.
    pub(super) fn render_menu(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let menu = self.menu.as_ref()?;
        let wrapper = menu.menu.render(
            "project-tree-menu",
            cx,
            |panel| Some(&mut panel.menu.as_mut()?.menu),
            Self::on_menu_input,
        );
        let wrapper = fe_ui::node(
            wrapper,
            window,
            cx,
            Role::Menu,
            tr!("workbench-project-menu"),
        );
        Some(
            deferred(
                anchored()
                    .position(menu.position)
                    .snap_to_window()
                    .child(wrapper),
            )
            .with_priority(1)
            .into_any_element(),
        )
    }
}

#[cfg(test)]
impl ProjectPanel {
    /// The open menu's entries by command, `None` for a separator.
    pub(crate) fn menu_commands(&self) -> Option<Vec<Option<&'static str>>> {
        let menu = self.menu.as_ref()?;
        Some(
            menu.entries
                .iter()
                .map(|entry| entry.map(MenuItem::command))
                .collect(),
        )
    }

    /// The open menu's items as drawn: each title and its key, if any.
    pub(crate) fn menu_drawn(&self, cx: &App) -> Option<Vec<(String, Option<String>)>> {
        let menu = self.menu.as_ref()?;
        let drawn = menu_entries(&menu.entries, cx)
            .into_iter()
            .filter_map(|entry| match entry {
                MenuEntry::Item { title, shortcut } => {
                    Some((title.to_string(), shortcut.map(|keys| keys.to_string())))
                }
                MenuEntry::Separator => None,
            });
        Some(drawn.collect())
    }

    /// The open menu's highlighted item's command, if one is.
    pub(crate) fn menu_highlighted(&self) -> Option<&'static str> {
        let menu = self.menu.as_ref()?;
        Some((*menu.entries.get(menu.menu.highlighted()?)?)?.command())
    }
}
