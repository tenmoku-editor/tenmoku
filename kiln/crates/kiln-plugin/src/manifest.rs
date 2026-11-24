// SPDX-License-Identifier: Apache-2.0

//! Parsing and validating a plugin's manifest.
//!
//! A plugin is an extension whose `extension.toml` has a
//! `[contributes.plugin]` table. The loader checks that table against
//! [`plugin_kind`]'s schema; [`PluginManifest::from_extension`] reads it
//! from the checked [`Manifest`].

use std::{
    path::{Component, Path, PathBuf},
    time::Duration,
};

use kiln_command::{Capability, CapabilityError, ContextPredicate, Grants};
use kiln_core::{AppKind, Key, KeySequence, Os, extension::Manifest};
use kiln_l10n::tr;
use serde_json::json;
use toml::Value;

use crate::limits::{
    BACKGROUND_MEMORY_FLOOR, BACKGROUND_WALL_FLOOR, BackgroundLimit, BackgroundLimits, Ceilings,
    DATA_QUOTA_CEILING,
};
use crate::settings::{SchemaError, SettingsSchema};

/// The `contributes` key that makes an extension a plugin.
pub const PLUGIN_KIND: &str = "plugin";

/// The `[contributes.plugin]` kind: `wasm`, `capabilities`, an optional
/// `signature` and an optional `[contributes.plugin.background]` table. Register it for the user's extensions layer only.
pub fn plugin_kind() -> AppKind {
    AppKind {
        name: PLUGIN_KIND,
        schema: json!({
            "type": "object",
            "properties": {
                "wasm": { "type": "string" },
                "capabilities": { "type": "array", "items": { "type": "string" } },
                "signature": { "type": "string" },
                "settings": { "type": "string" },
                "panels": { "type": "array", "items": { "type": "object" } },
                "bindings": { "type": "array", "items": { "type": "object" } },
                "background": {
                    "type": "object",
                    "properties": {
                        "memory": { "type": "string" },
                        "wall": { "type": "string" },
                        "data": { "type": "string" },
                    },
                    "additionalProperties": false,
                },
            },
            "required": ["wasm"],
            "additionalProperties": false,
        }),
    }
}

/// Where an extension was found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExtensionOrigin {
    /// The user's own extensions directory: the only place plugins install from.
    User,
    /// A workspace's extensions, which can never install or grant a plugin.
    Workspace,
}

#[derive(Debug, thiserror::Error)]
pub enum PluginManifestError {
    WorkspaceCannotInstallPlugins {
        id: String,
    },
    MoreThanOnePlugin {
        id: String,
    },
    MissingWasm {
        id: String,
    },
    CapabilitiesNotStrings {
        id: String,
    },
    SignatureNotString {
        id: String,
    },
    UnknownBackgroundKey {
        id: String,
        key: String,
    },
    Background {
        id: String,
        key: &'static str,
        reason: BackgroundError,
    },
    Capability {
        id: String,
        #[source]
        source: CapabilityError,
    },
    WasmPath {
        id: String,
        path: String,
        reason: WasmPathError,
    },
    Panel {
        id: String,
        reason: PanelError,
    },
    Binding {
        id: String,
        reason: BindingError,
    },
    SettingsNotString {
        id: String,
    },
    Settings {
        id: String,
        path: String,
        reason: String,
    },
}

/// In the active locale.
impl std::fmt::Display for PluginManifestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&match self {
            Self::WorkspaceCannotInstallPlugins { id } => {
                tr!("kiln-plugin-manifest-workspace", id = id)
            }
            Self::MoreThanOnePlugin { id } => tr!("kiln-plugin-manifest-more-than-one", id = id),
            Self::MissingWasm { id } => tr!("kiln-plugin-manifest-missing-wasm", id = id),
            Self::CapabilitiesNotStrings { id } => {
                tr!("kiln-plugin-manifest-capabilities-not-strings", id = id)
            }
            Self::SignatureNotString { id } => {
                tr!("kiln-plugin-manifest-signature-not-string", id = id)
            }
            Self::UnknownBackgroundKey { id, key } => {
                tr!(
                    "kiln-plugin-manifest-background-unknown-key",
                    id = id,
                    key = key
                )
            }
            Self::Background { id, key, reason } => tr!(
                "kiln-plugin-manifest-background",
                id = id,
                key = *key,
                reason = reason.to_string()
            ),
            Self::Capability { id, source } => tr!(
                "kiln-plugin-manifest-capability",
                id = id,
                reason = source.to_string()
            ),
            Self::Panel { id, reason } => tr!(
                "kiln-plugin-manifest-panel",
                id = id,
                reason = reason.to_string()
            ),
            Self::Binding { id, reason } => tr!(
                "kiln-plugin-manifest-binding",
                id = id,
                reason = reason.to_string()
            ),
            Self::SettingsNotString { id } => {
                tr!("kiln-plugin-settings-manifest-not-string", id = id)
            }
            Self::Settings { id, path, reason } => tr!(
                "kiln-plugin-settings-manifest",
                id = id,
                path = format!("{path:?}"),
                reason = reason
            ),
            Self::WasmPath { id, path, reason } => tr!(
                "kiln-plugin-manifest-wasm-path",
                id = id,
                path = format!("{path:?}"),
                reason = reason.to_string()
            ),
        })
    }
}

/// Why a `[contributes.plugin.background]` value is refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackgroundError {
    NotAString,
    Malformed,
    TooSmall,
    TooLarge,
}

/// In the active locale.
impl std::fmt::Display for BackgroundError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&tr!(match self {
            Self::NotAString => "kiln-plugin-background-not-a-string",
            Self::Malformed => "kiln-plugin-background-malformed",
            Self::TooSmall => "kiln-plugin-background-too-small",
            Self::TooLarge => "kiln-plugin-background-too-large",
        }))
    }
}

/// `text` as a whole number of `units`, each a suffix and its size: `512MiB`, `30m`.
fn with_unit(text: &str, units: &[(&str, u64)]) -> Result<u64, BackgroundError> {
    let text = text.trim();
    let (suffix, size) = units
        .iter()
        .filter(|(suffix, _)| text.ends_with(suffix))
        // `MiB` before `B`, were there one.
        .max_by_key(|(suffix, _)| suffix.len())
        .ok_or(BackgroundError::Malformed)?;
    let digits = text[..text.len() - suffix.len()].trim_end();
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(BackgroundError::Malformed);
    }
    let count: u64 = digits.parse().map_err(|_| BackgroundError::TooLarge)?;
    count.checked_mul(*size).ok_or(BackgroundError::TooLarge)
}

const SIZE_UNITS: &[(&str, u64)] = &[("KiB", 1 << 10), ("MiB", 1 << 20), ("GiB", 1 << 30)];
const TIME_UNITS: &[(&str, u64)] = &[("s", 1), ("m", 60)];

/// A size such as `256MiB`, in bytes, within `min..=max`. A value past `max` isn't an error
/// here for `memory` and `wall`: the host's ceilings clamp it ([`Ceilings::clamp`]).
fn size_value(
    id: &str,
    key: &'static str,
    value: &Value,
    min: u64,
    max: u64,
) -> Result<u64, PluginManifestError> {
    let fail = |reason| PluginManifestError::Background {
        id: id.to_owned(),
        key,
        reason,
    };
    let Value::String(text) = value else {
        return Err(fail(BackgroundError::NotAString));
    };
    let bytes = with_unit(text, SIZE_UNITS).map_err(fail)?;
    if bytes < min {
        return Err(fail(BackgroundError::TooSmall));
    }
    if bytes > max {
        return Err(fail(BackgroundError::TooLarge));
    }
    Ok(bytes)
}

/// Why a plugin's `wasm` path isn't a file inside its extension's directory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WasmPathError {
    Empty,
    Backslash,
    Absolute,
    Outside,
    DirUnreadable,
    Missing,
    NotAFile,
}

/// In the active locale.
impl std::fmt::Display for WasmPathError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&tr!(match self {
            Self::Empty => "kiln-plugin-wasm-path-empty",
            Self::Backslash => "kiln-plugin-wasm-path-backslash",
            Self::Absolute => "kiln-plugin-wasm-path-absolute",
            Self::Outside => "kiln-plugin-wasm-path-outside",
            Self::DirUnreadable => "kiln-plugin-wasm-path-dir-unreadable",
            Self::Missing => "kiln-plugin-wasm-path-missing",
            Self::NotAFile => "kiln-plugin-wasm-path-not-a-file",
        }))
    }
}

/// Panels one plugin may declare.
pub const MAX_PANELS: usize = 8;

/// The most `[[contributes.plugin.bindings]]` entries a plugin may declare.
/// A key binding is a claim on the user's keyboard, so the cap is a limit on
/// how much of it one extension can take.
pub const MAX_BINDINGS: usize = 32;

/// What `tick_ms` may be, in milliseconds. The floor keeps a plugin from
/// asking for a tick faster than the panel update rate the stage-3 record
/// allows; the ceiling keeps `tick_ms` meaning "a timer" rather than "a
/// once-a-minute job", which is what `jobs` is for.
pub const TICK_MS_RANGE: std::ops::RangeInclusive<i64> = 50..=1000;

/// Where a panel's dock is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanelLocation {
    Left,
    Right,
    Bottom,
}

/// A panel a plugin declares in `[[contributes.plugin.panels]]`: the panel is
/// `<plugin id>.<id>`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PanelDecl {
    /// Unique among the plugin's panels: ASCII letters, digits, `-` and `_`.
    pub id: String,
    /// The title the dock shows.
    pub title: String,
    /// A name in the app's icon set.
    pub icon: String,
    /// The `dock` key: `left`, `right` or `bottom`.
    pub location: PanelLocation,
    /// `tick_ms`: how often the panel asks to be ticked, in milliseconds,
    /// within [`TICK_MS_RANGE`]. Absent means the panel is never ticked.
    /// Needs `panels.realtime`.
    pub tick_ms: Option<u32>,
    /// `keys`: whether key events reach the panel while it is focused.
    /// Needs `panels.realtime`.
    pub keys: bool,
}

/// Why a `[[contributes.plugin.panels]]` entry is refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PanelError {
    NotATable,
    Missing(&'static str),
    NotAString(&'static str),
    Empty(&'static str),
    UnknownKey(String),
    BadId(String),
    BadDock(String),
    Duplicate(String),
    TooMany,
    /// `tick_ms` was not an integer.
    NotAnInteger(&'static str),
    /// `keys` was not a boolean.
    NotABool(&'static str),
    /// `tick_ms` was an integer outside [`TICK_MS_RANGE`]. Carries the value
    /// as declared, signed, so a negative tick is reported as written.
    TickOutOfRange(i64),
    /// The panel declared `tick_ms` or `keys` without `panels.realtime`.
    /// Carries the panel's id.
    RealtimeWithoutCapability(String),
}

/// In the active locale.
impl std::fmt::Display for PanelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&match self {
            Self::NotATable => tr!("kiln-plugin-panel-not-a-table"),
            Self::Missing(key) => tr!("kiln-plugin-panel-missing", key = *key),
            Self::NotAString(key) => tr!("kiln-plugin-panel-not-a-string", key = *key),
            Self::Empty(key) => tr!("kiln-plugin-panel-empty", key = *key),
            Self::UnknownKey(key) => tr!("kiln-plugin-panel-unknown-key", key = key.as_str()),
            Self::BadId(id) => tr!("kiln-plugin-panel-bad-id", panel = format!("{id:?}")),
            Self::BadDock(dock) => tr!("kiln-plugin-panel-bad-dock", dock = format!("{dock:?}")),
            Self::Duplicate(id) => tr!("kiln-plugin-panel-duplicate", panel = format!("{id:?}")),
            Self::TooMany => tr!("kiln-plugin-panel-too-many", max = MAX_PANELS),
            Self::NotAnInteger(key) => tr!("kiln-plugin-panel-not-an-integer", key = *key),
            Self::NotABool(key) => tr!("kiln-plugin-panel-not-a-bool", key = *key),
            Self::TickOutOfRange(value) => tr!(
                "kiln-plugin-panel-tick-out-of-range",
                value = *value,
                min = *TICK_MS_RANGE.start(),
                max = *TICK_MS_RANGE.end()
            ),
            Self::RealtimeWithoutCapability(id) => tr!(
                "kiln-plugin-panel-realtime-without-capability",
                panel = format!("{id:?}"),
                capability = Capability::PanelsRealtime.to_string()
            ),
        })
    }
}

/// A key binding a plugin declares in `[[contributes.plugin.bindings]]`:
/// keys bound to one of the plugin's own commands, at the lowest precedence
/// of any layer (T2106).
///
/// `keys` and `context` are parsed here rather than kept as text, so that
/// what reaches the host is already known to be a keymap entry it can use.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BindingDecl {
    pub keys: KeySequence,
    /// A full command id starting with the plugin's own id and a dot.
    pub command: String,
    /// A predicate, as a keymap entry's `context`.
    pub context: Option<ContextPredicate>,
}

/// Why a `[[contributes.plugin.bindings]]` entry is refused. Every one of
/// these is decided while the manifest is read, before the plugin loads:
/// a binding commits the host to taking the user's keystrokes, which is the
/// same reasoning `panel_decl` applies to `tick_ms` and `keys`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BindingError {
    NotATable,
    Missing(&'static str),
    NotAString(&'static str),
    Empty(&'static str),
    UnknownKey(String),
    /// `keys` did not parse as a keymap `keys` value. Carries it as written.
    BadKeys(String),
    /// `context` did not parse as a keymap `context` predicate.
    BadContext(String),
    /// The command is not this plugin's. Carries the command as written.
    ForeignCommand(String),
    /// Bindings were declared without the `commands` capability.
    WithoutCapability,
    /// Two entries share keys and context, so one could never run.
    Duplicate(String),
    TooMany,
    /// The first keystroke is a bare typing key, or carries only alt or
    /// shift. Carries the keys as written.
    BadFirstKeystroke(String),
}

/// In the active locale.
impl std::fmt::Display for BindingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&match self {
            Self::NotATable => tr!("kiln-plugin-binding-not-a-table"),
            Self::Missing(key) => tr!("kiln-plugin-binding-missing", key = *key),
            Self::NotAString(key) => tr!("kiln-plugin-binding-not-a-string", key = *key),
            Self::Empty(key) => tr!("kiln-plugin-binding-empty", key = *key),
            Self::UnknownKey(key) => tr!("kiln-plugin-binding-unknown-key", key = key.as_str()),
            Self::BadKeys(keys) => tr!("kiln-plugin-binding-bad-keys", keys = format!("{keys:?}")),
            Self::BadContext(context) => tr!(
                "kiln-plugin-binding-bad-context",
                context = format!("{context:?}")
            ),
            Self::ForeignCommand(command) => tr!(
                "kiln-plugin-binding-foreign-command",
                command = format!("{command:?}")
            ),
            Self::WithoutCapability => tr!(
                "kiln-plugin-binding-without-capability",
                capability = Capability::Commands.to_string()
            ),
            Self::Duplicate(keys) => {
                tr!("kiln-plugin-binding-duplicate", keys = format!("{keys:?}"))
            }
            Self::TooMany => tr!("kiln-plugin-binding-too-many", max = MAX_BINDINGS),
            Self::BadFirstKeystroke(keys) => tr!(
                "kiln-plugin-binding-bad-first-keystroke",
                keys = format!("{keys:?}")
            ),
        })
    }
}

/// Whether a plugin may take `keys` as its first keystroke.
///
/// It must be a function key, or carry ctrl or this platform's secondary
/// modifier (cmd on macOS, where ctrl is also allowed). Everything else is
/// typing: a bare or shift-only key, escape, tab and enter all belong to the
/// editor, and so does alt alone - on macOS Option composes characters, so a
/// plugin holding alt-x would eat what the user types. The rule is the same
/// on every OS, which also makes it one rule for a plugin author to read.
///
/// Only the FIRST keystroke is restricted. A chord may continue with a bare
/// key, as the app's own `secondary-k z` does.
fn first_keystroke_is_allowed(keys: &KeySequence) -> bool {
    keys.keystrokes().first().is_some_and(|first| {
        matches!(first.key, Key::F(_)) || first.modifiers.ctrl || first.modifiers.cmd
    })
}

/// The `keys` of one entry: a string, or a table by platform with an
/// optional `default`. A table naming neither this platform nor a default
/// binds nothing here, which is how a macOS-only binding is written.
fn binding_keys(table: &toml::map::Map<String, Value>) -> Result<Option<String>, BindingError> {
    let this = Os::current().name();
    match table.get("keys") {
        None => Err(BindingError::Missing("keys")),
        Some(Value::String(text)) if text.trim().is_empty() => Err(BindingError::Empty("keys")),
        Some(Value::String(text)) => Ok(Some(text.clone())),
        Some(Value::Table(by_platform)) => {
            if let Some(key) = by_platform
                .keys()
                .find(|key| !matches!(key.as_str(), "macos" | "linux" | "windows" | "default"))
            {
                return Err(BindingError::UnknownKey(key.clone()));
            }
            match by_platform.get(this).or_else(|| by_platform.get("default")) {
                None => Ok(None),
                Some(Value::String(text)) if text.trim().is_empty() => {
                    Err(BindingError::Empty("keys"))
                }
                Some(Value::String(text)) => Ok(Some(text.clone())),
                Some(_) => Err(BindingError::NotAString("keys")),
            }
        }
        Some(_) => Err(BindingError::NotAString("keys")),
    }
}

/// One `[[contributes.plugin.bindings]]` entry, checked against `id`, the
/// plugin's own extension id. `Ok(None)` is an entry whose platform table
/// names nothing for this OS.
fn binding_decl(entry: &Value, id: &str) -> Result<Option<BindingDecl>, BindingError> {
    let Value::Table(table) = entry else {
        return Err(BindingError::NotATable);
    };
    if let Some(key) = table
        .keys()
        .find(|key| !matches!(key.as_str(), "keys" | "command" | "context"))
    {
        return Err(BindingError::UnknownKey(key.clone()));
    }
    let Some(source) = binding_keys(table)? else {
        return Ok(None);
    };
    let keys = KeySequence::parse(&source).map_err(|_| BindingError::BadKeys(source.clone()))?;
    if !first_keystroke_is_allowed(&keys) {
        return Err(BindingError::BadFirstKeystroke(source));
    }
    let command = match table.get("command") {
        Some(Value::String(text)) if text.trim().is_empty() => {
            return Err(BindingError::Empty("command"));
        }
        Some(Value::String(text)) => text.clone(),
        Some(_) => return Err(BindingError::NotAString("command")),
        None => return Err(BindingError::Missing("command")),
    };
    // Its own namespace only: the id plus a dot, with something after it.
    // A plugin binding `editor.delete_all_right` would reach the whole app,
    // and `acme.toolbar.run` belongs to a different extension than
    // `acme.tool` even though it shares a textual prefix.
    if command
        .strip_prefix(id)
        .and_then(|rest| rest.strip_prefix('.'))
        .is_none_or(str::is_empty)
    {
        return Err(BindingError::ForeignCommand(command));
    }
    let context = match table.get("context") {
        None => None,
        Some(Value::String(text)) if text.trim().is_empty() => {
            return Err(BindingError::Empty("context"));
        }
        Some(Value::String(text)) => Some(
            ContextPredicate::parse(text).map_err(|_| BindingError::BadContext(text.clone()))?,
        ),
        Some(_) => return Err(BindingError::NotAString("context")),
    };
    Ok(Some(BindingDecl {
        keys,
        command,
        context,
    }))
}

fn panel_decl(entry: &Value) -> Result<PanelDecl, PanelError> {
    let Value::Table(table) = entry else {
        return Err(PanelError::NotATable);
    };
    if let Some(key) = table.keys().find(|key| {
        !matches!(
            key.as_str(),
            "id" | "title" | "icon" | "dock" | "tick_ms" | "keys"
        )
    }) {
        return Err(PanelError::UnknownKey(key.clone()));
    }
    let string = |key: &'static str, default: Option<&str>| match table.get(key) {
        Some(Value::String(text)) if text.trim().is_empty() => Err(PanelError::Empty(key)),
        Some(Value::String(text)) => Ok(text.clone()),
        Some(_) => Err(PanelError::NotAString(key)),
        None => default.map(str::to_owned).ok_or(PanelError::Missing(key)),
    };
    let id = string("id", None)?;
    if !id
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        return Err(PanelError::BadId(id));
    }
    let title = string("title", None)?;
    let icon = string("icon", None)?;
    let location = match string("dock", Some("left"))?.as_str() {
        "left" => PanelLocation::Left,
        "right" => PanelLocation::Right,
        "bottom" => PanelLocation::Bottom,
        other => return Err(PanelError::BadDock(other.to_owned())),
    };
    let tick_ms = match table.get("tick_ms") {
        None => None,
        Some(Value::Integer(ms)) if TICK_MS_RANGE.contains(ms) => Some(*ms as u32),
        Some(Value::Integer(ms)) => return Err(PanelError::TickOutOfRange(*ms)),
        Some(_) => return Err(PanelError::NotAnInteger("tick_ms")),
    };
    let keys = match table.get("keys") {
        None => false,
        Some(Value::Boolean(keys)) => *keys,
        Some(_) => return Err(PanelError::NotABool("keys")),
    };
    Ok(PanelDecl {
        id,
        title,
        icon,
        location,
        tick_ms,
        keys,
    })
}

/// A plugin's checked manifest entry.
#[derive(Clone, Debug, PartialEq)]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub version: (u32, u32, u32),
    /// The component, an absolute path to a file inside the extension's
    /// directory.
    pub wasm: PathBuf,
    /// What the manifest asks for; granting is decided elsewhere.
    pub capabilities: Grants,
    /// A reserved slot: accepted and kept, never verified.
    pub signature: Option<String>,
    /// What `[contributes.plugin.background]` declares, over the defaults. As declared: the
    /// host's [`Ceilings`] clamp it where it is used.
    pub background: BackgroundLimits,
    /// The panels `[[contributes.plugin.panels]]` declares, in order.
    pub panels: Vec<PanelDecl>,
    /// The key bindings `[[contributes.plugin.bindings]]` declares, in
    /// order. Those for another platform are already left out.
    pub bindings: Vec<BindingDecl>,
    /// The settings schema `settings` names, checked.
    pub settings: Option<std::sync::Arc<SettingsSchema>>,
    /// Notes on capabilities that were accepted but are better written
    /// another way, such as `process.spawn:rg.exe`.
    pub warnings: Vec<String>,
}

impl PluginManifest {
    /// The plugin entry of `manifest`, or `None` for an extension that
    /// contributes none. A `Workspace` extension with one is an error.
    pub fn from_extension(
        manifest: &Manifest,
        origin: ExtensionOrigin,
    ) -> Result<Option<Self>, PluginManifestError> {
        let id = &manifest.id;
        let Some(entry) = manifest.app.get(PLUGIN_KIND) else {
            return Ok(None);
        };
        if origin == ExtensionOrigin::Workspace {
            return Err(PluginManifestError::WorkspaceCannotInstallPlugins { id: id.clone() });
        }
        let Value::Table(entry) = entry else {
            return Err(PluginManifestError::MoreThanOnePlugin { id: id.clone() });
        };
        let Some(Value::String(wasm)) = entry.get("wasm") else {
            return Err(PluginManifestError::MissingWasm { id: id.clone() });
        };
        let wasm = wasm_file(&manifest.dir, wasm).map_err(|reason| {
            let path = wasm.clone();
            PluginManifestError::WasmPath {
                id: id.clone(),
                path,
                reason,
            }
        })?;
        let mut capabilities = Grants::new();
        let mut warnings = Vec::new();
        match entry.get("capabilities") {
            None => {}
            Some(Value::Array(items)) => {
                for item in items {
                    let Value::String(text) = item else {
                        return Err(PluginManifestError::CapabilitiesNotStrings { id: id.clone() });
                    };
                    let capability = Capability::parse(text).map_err(|source| {
                        PluginManifestError::Capability {
                            id: id.clone(),
                            source,
                        }
                    })?;
                    warnings.extend(capability.warning());
                    capabilities.insert(capability);
                }
            }
            Some(_) => {
                return Err(PluginManifestError::CapabilitiesNotStrings { id: id.clone() });
            }
        }
        let signature = match entry.get("signature") {
            None => None,
            Some(Value::String(signature)) => Some(signature.clone()),
            Some(_) => return Err(PluginManifestError::SignatureNotString { id: id.clone() }),
        };
        let mut background = BackgroundLimits::default();
        match entry.get("background") {
            None => {}
            Some(Value::Table(table)) => {
                if let Some(key) = table
                    .keys()
                    .find(|key| !matches!(key.as_str(), "memory" | "wall" | "data"))
                {
                    return Err(PluginManifestError::UnknownBackgroundKey {
                        id: id.clone(),
                        key: key.clone(),
                    });
                }
                // A value past the ceiling isn't refused: the host clamps it.
                if let Some(value) = table.get("memory") {
                    let bytes = size_value(id, "memory", value, BACKGROUND_MEMORY_FLOOR, u64::MAX)?;
                    background.memory_bytes = bytes;
                }
                if let Some(value) = table.get("wall") {
                    let fail = |reason| PluginManifestError::Background {
                        id: id.clone(),
                        key: "wall",
                        reason,
                    };
                    let Value::String(text) = value else {
                        return Err(fail(BackgroundError::NotAString));
                    };
                    let secs = with_unit(text, TIME_UNITS).map_err(fail)?;
                    if secs < BACKGROUND_WALL_FLOOR.as_secs() {
                        return Err(fail(BackgroundError::TooSmall));
                    }
                    background.wall = Duration::from_secs(secs);
                }
                if let Some(value) = table.get("data") {
                    background.data_bytes =
                        Some(size_value(id, "data", value, 0, DATA_QUOTA_CEILING)?);
                }
            }
            Some(_) => {
                return Err(PluginManifestError::Background {
                    id: id.clone(),
                    key: "background",
                    reason: BackgroundError::NotAString,
                });
            }
        }
        let mut panels: Vec<PanelDecl> = Vec::new();
        match entry.get("panels") {
            None => {}
            Some(Value::Array(items)) => {
                let fail = |reason| PluginManifestError::Panel {
                    id: id.clone(),
                    reason,
                };
                for item in items {
                    let panel = panel_decl(item).map_err(fail)?;
                    // `panels` itself is checked at the call, not here, because
                    // showing a panel is an operation. `tick_ms` and `keys` are
                    // declarations that commit the host to a timer and to the
                    // user's keystrokes before the plugin runs at all, so they
                    // are refused while the manifest is read.
                    if (panel.tick_ms.is_some() || panel.keys)
                        && !capabilities.allows(&Capability::PanelsRealtime)
                    {
                        return Err(fail(PanelError::RealtimeWithoutCapability(panel.id)));
                    }
                    if panels.iter().any(|other| other.id == panel.id) {
                        return Err(fail(PanelError::Duplicate(panel.id)));
                    }
                    panels.push(panel);
                }
                if panels.len() > MAX_PANELS {
                    return Err(fail(PanelError::TooMany));
                }
            }
            Some(_) => {
                return Err(PluginManifestError::Panel {
                    id: id.clone(),
                    reason: PanelError::NotATable,
                });
            }
        }
        let mut bindings: Vec<BindingDecl> = Vec::new();
        match entry.get("bindings") {
            None => {}
            Some(Value::Array(items)) => {
                let fail = |reason| PluginManifestError::Binding {
                    id: id.clone(),
                    reason,
                };
                // Checked here and not at the call, for the reason the
                // panels above give: a binding commits the host to taking
                // the user's keystrokes before the plugin runs at all.
                if !items.is_empty() && !capabilities.allows(&Capability::Commands) {
                    return Err(fail(BindingError::WithoutCapability));
                }
                for item in items {
                    // `None` is an entry whose platform table names nothing
                    // for this OS: a macOS-only binding is legitimate and
                    // simply binds nothing here.
                    let Some(binding) = binding_decl(item, id).map_err(fail)? else {
                        continue;
                    };
                    // The cap goes before the duplicate scan and the push,
                    // so a long list is refused at the 33rd binding rather
                    // than compared pairwise in full first.
                    if bindings.len() == MAX_BINDINGS {
                        return Err(fail(BindingError::TooMany));
                    }
                    if bindings
                        .iter()
                        .any(|other| other.keys == binding.keys && other.context == binding.context)
                    {
                        return Err(fail(BindingError::Duplicate(binding.keys.to_string())));
                    }
                    bindings.push(binding);
                }
            }
            Some(_) => {
                return Err(PluginManifestError::Binding {
                    id: id.clone(),
                    reason: BindingError::NotATable,
                });
            }
        }
        let settings = match entry.get("settings") {
            None => None,
            Some(Value::String(path)) => {
                let fail = |reason: String| PluginManifestError::Settings {
                    id: id.clone(),
                    path: path.clone(),
                    reason,
                };
                let file = wasm_file(&manifest.dir, path).map_err(|e| fail(e.to_string()))?;
                if std::fs::metadata(&file)
                    .is_ok_and(|meta| meta.len() > crate::settings::MAX_SCHEMA_BYTES as u64)
                {
                    return Err(fail(SchemaError::TooLarge.to_string()));
                }
                let text = std::fs::read_to_string(&file).map_err(|e| fail(e.to_string()))?;
                let schema = SettingsSchema::parse(&text).map_err(|e| fail(e.to_string()))?;
                Some(std::sync::Arc::new(schema))
            }
            Some(_) => return Err(PluginManifestError::SettingsNotString { id: id.clone() }),
        };
        Ok(Some(Self {
            id: manifest.id.clone(),
            name: manifest.name.clone(),
            version: manifest.version,
            wasm,
            capabilities,
            signature,
            background,
            panels,
            bindings,
            settings,
            warnings,
        }))
    }

    /// The background limits jobs run with on a computer with `ceilings`, and a warning for each
    /// declared value clamped to them. Not an error: the plugin still loads.
    pub fn background_within(&self, ceilings: &Ceilings) -> (BackgroundLimits, Vec<String>) {
        let (limits, clamped) = ceilings.clamp(self.background);
        let warnings = clamped
            .iter()
            .filter_map(|clamped| match clamped.limit {
                BackgroundLimit::Memory => Some(
                    tr!(
                        "kiln-plugin-background-memory-clamped",
                        id = self.id.as_str(),
                        declared = clamped.declared / (1 << 20),
                        ceiling = clamped.ceiling / (1 << 20)
                    )
                    .to_string(),
                ),
                BackgroundLimit::Wall => Some(
                    tr!(
                        "kiln-plugin-background-wall-clamped",
                        id = self.id.as_str(),
                        declared = clamped.declared / 60,
                        ceiling = clamped.ceiling / 60
                    )
                    .to_string(),
                ),
                // A data quota past its ceiling is refused when the manifest is parsed, so
                // `Ceilings::clamp` never clamps one.
                BackgroundLimit::Data => None,
            })
            .collect();
        (limits, warnings)
    }

    /// Whether the signature was checked. Always false: the slot is
    /// reserved and nothing verifies it yet.
    pub fn signature_verified(&self) -> bool {
        false
    }
}

/// `entry` as a file inside `dir`, symlinks followed.
fn wasm_file(dir: &Path, entry: &str) -> Result<PathBuf, WasmPathError> {
    let relative = Path::new(entry);
    if entry.is_empty() {
        return Err(WasmPathError::Empty);
    }
    if entry.contains('\\') {
        return Err(WasmPathError::Backslash);
    }
    if relative.has_root() || relative.is_absolute() {
        return Err(WasmPathError::Absolute);
    }
    // Written to climb out, whatever the links inside lead to.
    let mut depth = 0usize;
    for component in relative.components() {
        match component {
            Component::Normal(_) => depth += 1,
            Component::CurDir => {}
            Component::ParentDir if depth > 0 => depth -= 1,
            _ => return Err(WasmPathError::Outside),
        }
    }
    // Without Windows' `\\?\`, as paths are shown and compared elsewhere.
    let canonical_dir =
        kiln_core::platform::canonicalize(dir).map_err(|_| WasmPathError::DirUnreadable)?;
    let canonical = kiln_core::platform::canonicalize(&dir.join(relative))
        .map_err(|_| WasmPathError::Missing)?;
    if !canonical.starts_with(&canonical_dir) {
        return Err(WasmPathError::Outside);
    }
    if !canonical.is_file() {
        return Err(WasmPathError::NotAFile);
    }
    Ok(canonical)
}

#[cfg(test)]
mod panel_tests {
    use super::*;

    fn parse(text: &str) -> Result<PanelDecl, PanelError> {
        panel_decl(&Value::Table(text.parse::<toml::Table>().unwrap()))
    }

    #[test]
    fn a_panel_declares_id_title_icon_and_location() {
        let panel =
            parse("id = \"todos\"\ntitle = \"TODOs\"\nicon = \"list-checks\"\ndock = \"bottom\"")
                .unwrap();
        assert_eq!(
            panel,
            PanelDecl {
                id: "todos".into(),
                title: "TODOs".into(),
                icon: "list-checks".into(),
                location: PanelLocation::Bottom,
                // The realtime half is off unless the panel asks for it.
                tick_ms: None,
                keys: false,
            }
        );
        // The dock defaults to the left.
        let panel = parse("id = \"a_b-1\"\ntitle = \"A\"\nicon = \"x\"").unwrap();
        assert_eq!(panel.location, PanelLocation::Left);
    }

    #[test]
    fn a_malformed_panel_is_refused() {
        assert_eq!(
            parse("title = \"T\"\nicon = \"x\""),
            Err(PanelError::Missing("id"))
        );
        assert_eq!(
            parse("id = \"a b\"\ntitle = \"T\"\nicon = \"x\""),
            Err(PanelError::BadId("a b".into()))
        );
        assert_eq!(
            parse("id = \"a\"\ntitle = \"T\"\nicon = \"x\"\ndock = \"top\""),
            Err(PanelError::BadDock("top".into()))
        );
        assert_eq!(
            parse("id = \"a\"\ntitle = \"T\"\nicon = \"x\"\nsize = 3"),
            Err(PanelError::UnknownKey("size".into()))
        );
        assert_eq!(
            parse("id = \"a\"\ntitle = \" \"\nicon = \"x\""),
            Err(PanelError::Empty("title"))
        );
    }
}
