// SPDX-License-Identifier: Apache-2.0

//! Key bindings a host sets on behalf of someone else: one keymap layer
//! per owner, at the lowest precedence of any layer (T2106).
//!
//! A host - [`crate::extensions`]' runner, or an app's plugin host - runs
//! code it did not write, which asks for keys it does not own. Such a
//! binding must never take keys away from the app or the user, so it is
//! not simply another layer: ordinary layers are ranked by context depth
//! first and layer order only as a tie-break, so a deeper binding in an
//! earlier layer beats a shallower one in a later layer. A host's binding
//! instead loses to any other layer's binding for the same keys - or for
//! a chord either of them starts - whatever the contexts' depths. That
//! rule lives in [`kiln_core::Keymap`] and is carried by
//! [`kiln_core::Binding::host`], which is set for every binding read from
//! a layer whose path is [`KeymapLayer::host_path`].
//!
//! The user still outranks a host both ways: their binding wins, and
//! `command = ""` in their keymap unbinds a host's keys, because these
//! layers go in with the defaults and the user's file is read over all of
//! them.
//!
//! ```ignore
//! host_keys::set(cx, "acme.tool", bindings, commands);
//! host_keys::remove(cx, "acme.tool");  // unloaded, disabled, revoked
//! ```
//!
//! Each call re-resolves the keymap once the current update is over, as
//! any other contribution does.

use std::{collections::BTreeMap, collections::BTreeSet, rc::Rc};

use gpui::{App, Global};
use kiln_command::{CommandId, ContextPredicate, Value};
use kiln_core::{CatalogError, CommandCatalog, Entry, KeySequence, KeymapLayer, Location};

/// One binding a host asks for, already parsed: a host hands over typed
/// keys and context rather than text, so there is nothing left to
/// diagnose by the time it gets here. Whoever read them from a file or a
/// manifest reports what it refused - see `PluginManifest::bindings` for
/// the plugin rules.
#[derive(Clone, Debug, PartialEq)]
pub struct HostBinding {
    pub keys: KeySequence,
    pub command: CommandId,
    /// `None` binds them everywhere.
    pub context: Option<ContextPredicate>,
}

/// What one owner has asked for.
struct Owner {
    bindings: Vec<HostBinding>,
    /// The commands the owner adds, which the app does not have: a
    /// binding to one of these, in the owner's layer or in the user's
    /// file, is to a known command and not a mistake.
    commands: BTreeSet<CommandId>,
}

/// Every owner's bindings, by owner. Sorted, so the layers go in the same
/// order every time and two owners' bindings tie-break predictably.
#[derive(Default)]
struct HostKeys(BTreeMap<String, Owner>);

impl Global for HostKeys {}

/// Sets `owner`'s bindings and the commands it adds, replacing whatever
/// it had before, and re-resolves the keymap.
pub fn set(cx: &mut App, owner: &str, bindings: Vec<HostBinding>, commands: Vec<CommandId>) {
    let owned = Owner {
        bindings,
        commands: commands.into_iter().collect(),
    };
    cx.default_global::<HostKeys>()
        .0
        .insert(owner.to_owned(), owned);
    crate::contributions::resolve_again(cx);
}

/// Takes `owner`'s bindings and commands away: it was unloaded, disabled,
/// stopped or had the capability revoked. Does nothing, and re-resolves
/// nothing, for an owner that has none.
pub fn remove(cx: &mut App, owner: &str) {
    if cx.try_global::<HostKeys>().is_none() {
        return;
    }
    if cx.global_mut::<HostKeys>().0.remove(owner).is_some() {
        crate::contributions::resolve_again(cx);
    }
}

/// Each owner's bindings as its own layer, for
/// [`crate::contributions::shipped_layers`] to put in with the defaults.
///
/// These go in AFTER the other default layers, not before. Which of them
/// wins is not a question of order - [`kiln_core::Binding::host`] settles
/// that whatever the order - and going in last leaves every other layer's
/// index alone, which a reader of [`kiln_core::Binding::layer`] such as
/// the Keyboard Shortcuts list depends on.
pub(crate) fn layers(cx: &App) -> Vec<KeymapLayer> {
    let Some(this) = cx.try_global::<HostKeys>() else {
        return Vec::new();
    };
    (this.0.iter())
        .filter(|(_, owner)| !owner.bindings.is_empty())
        .map(|(name, owner)| layer(name, &owner.bindings))
        .collect()
}

/// `owner`'s bindings as a layer.
///
/// The entries are built rather than parsed from generated TOML, as the
/// contributed layer is ([`crate::contributions::keymap_layer`]): there
/// is no file and nothing to escape, and a [`HostBinding`] arrives with
/// its keys and context already parsed, so a round trip through text
/// could only lose. Each entry is located on its own 1-based line of the
/// layer's synthetic path, so a diagnostic or the Shortcuts list can say
/// which binding it means.
fn layer(owner: &str, bindings: &[HostBinding]) -> KeymapLayer {
    let path = KeymapLayer::host_path(owner);
    let entries = (bindings.iter().enumerate())
        .map(|(ix, binding)| Entry {
            keys: binding.keys.clone(),
            context: binding.context.clone(),
            command: Some(binding.command.clone()),
            args: Value::Null,
            location: Location {
                file: path.clone(),
                line: ix + 1,
                column: 1,
            },
        })
        .collect();
    KeymapLayer { path, entries }
}

/// The commands every owner adds, as a catalog, or `None` when no owner
/// adds any.
///
/// [`crate::contributions::shipped_catalog`] puts this LAST, so an owner
/// cannot shadow a command kiln or the app already has: a host is lowest
/// here too.
pub(crate) fn catalog(cx: &App) -> Option<Rc<dyn CommandCatalog>> {
    let this = cx.try_global::<HostKeys>()?;
    let commands: BTreeSet<CommandId> = (this.0.values())
        .flat_map(|owner| owner.commands.iter().cloned())
        .collect();
    match commands.is_empty() {
        true => None,
        false => Some(Rc::new(HostCommands(commands))),
    }
}

/// Every key sequence the app and kiln themselves bind, whatever the
/// context, with no duplicates.
///
/// A host checks the bindings it is asked to set against these: keys that
/// equal one of them, start one, or are started by one would take the
/// app's own shortcut away, or make it unreachable, and a host refuses
/// that rather than setting it and letting it silently never run. The
/// extensions' keys, other owners' and the user's are deliberately not
/// here - losing to those at runtime is ordinary and needs no refusal.
///
/// Unbinds (`command = ""`) are left out: they bind nothing, so they are
/// no reason to refuse anything.
pub fn builtin_keys(cx: &App) -> Vec<KeySequence> {
    let mut keys: Vec<KeySequence> = crate::contributions::builtin_layers(cx)
        .iter()
        .flat_map(|layer| layer.entries.iter())
        .filter(|entry| entry.command.is_some())
        .map(|entry| entry.keys.clone())
        .collect();
    keys.sort_by_cached_key(|sequence| sequence.to_string());
    keys.dedup();
    keys
}

/// Accepts the owners' commands with any args. A host checks a call's
/// args itself, against whatever it declared them to be, when the
/// command runs; there is no schema for them here.
struct HostCommands(BTreeSet<CommandId>);

impl CommandCatalog for HostCommands {
    fn check(&self, command: &CommandId, _args: &Value) -> Result<(), CatalogError> {
        match self.0.contains(command) {
            true => Ok(()),
            // Not this catalog's: `Catalogs` tries the next one.
            false => Err(CatalogError::UnknownCommand),
        }
    }
}
