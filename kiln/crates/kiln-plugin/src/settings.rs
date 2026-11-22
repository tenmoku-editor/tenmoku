// SPDX-License-Identifier: Apache-2.0

//! A plugin's settings contributions (design section 4.4): the JSON Schema subset its manifest
//! names, the layers that make up its effective settings, and the `settings.get` import.
//!
//! The schema is an object whose properties are of type `string`, `number`, `integer`,
//! `boolean`, `array` (of those) or an `enum`, with `default` and `description`, plus
//! `x-scope` (`user` or `project`) and `x-secret`. A secret is never read from a settings file:
//! one found there is a warning and ignored. The effective settings are the defaults, then the
//! user's table, then, for an open and trusted root, that root's `project`-scoped keys. A value
//! that doesn't fit its key is dropped with a diagnostic.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use kiln_l10n::tr;
use serde_json::{Map, Value as Json, json};
use toml::{Table, Value};

use crate::bindings::kiln::extension::log::Level;
use crate::bindings::kiln::extension::settings::Host;
use crate::bindings::{Error, HostState, unimplemented};

/// The largest schema file, in bytes.
pub const MAX_SCHEMA_BYTES: usize = 64 * 1024;
/// The most keys a schema may declare.
pub const MAX_KEYS: usize = 256;
/// The most bytes of effective settings handed to a plugin.
pub const MAX_VALUE_BYTES: usize = 256 * 1024;

/// Where a key may be set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    User,
    Project,
}

/// What a value of a key may be.
#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    String,
    Number,
    Integer,
    Boolean,
    Array(Box<Kind>),
    Enum(Vec<String>),
}

impl Kind {
    fn accepts(&self, value: &Value) -> bool {
        match (self, value) {
            (Self::String, Value::String(_)) | (Self::Number, Value::Integer(_)) => true,
            // `nan` and `inf` have no JSON form: they would reach the plugin as `null`.
            (Self::Number, Value::Float(float)) => float.is_finite(),
            (Self::Integer, Value::Integer(_)) | (Self::Boolean, Value::Boolean(_)) => true,
            (Self::Array(item), Value::Array(items)) => items.iter().all(|i| item.accepts(i)),
            (Self::Enum(names), Value::String(text)) => names.contains(text),
            _ => false,
        }
    }
}

/// One declared key.
#[derive(Clone, Debug, PartialEq)]
pub struct Key {
    pub kind: Kind,
    pub default: Option<Value>,
    pub scope: Scope,
    pub secret: bool,
}

/// Why a schema isn't the subset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SchemaError {
    TooLarge,
    NotJson(String),
    NotAnObject,
    /// A root keyword outside the subset, or a root `type` that isn't `object`.
    Root(String),
    TooManyKeys,
    Key {
        key: String,
        reason: KeyError,
    },
}

/// Why one key's declaration is refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyError {
    NotAnObject,
    UnknownKeyword(String),
    BadType,
    BadScope,
    BadSecret,
    BadEnum,
    BadDescription,
    BadItems,
    BadDefault,
    SecretDefault,
}

impl fmt::Display for KeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&match self {
            Self::NotAnObject => tr!("kiln-plugin-settings-key-not-object"),
            Self::UnknownKeyword(word) => {
                tr!("kiln-plugin-settings-key-keyword", keyword = word.as_str())
            }
            Self::BadType => tr!("kiln-plugin-settings-key-type"),
            Self::BadScope => tr!("kiln-plugin-settings-key-scope"),
            Self::BadSecret => tr!("kiln-plugin-settings-key-secret"),
            Self::BadEnum => tr!("kiln-plugin-settings-key-enum"),
            Self::BadDescription => tr!("kiln-plugin-settings-key-description"),
            Self::BadItems => tr!("kiln-plugin-settings-key-items"),
            Self::BadDefault => tr!("kiln-plugin-settings-key-default"),
            Self::SecretDefault => tr!("kiln-plugin-settings-key-secret-default"),
        })
    }
}

impl fmt::Display for SchemaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&match self {
            Self::TooLarge => tr!(
                "kiln-plugin-settings-schema-too-large",
                max = MAX_SCHEMA_BYTES
            ),
            Self::NotJson(reason) => {
                tr!(
                    "kiln-plugin-settings-schema-not-json",
                    reason = reason.as_str()
                )
            }
            Self::NotAnObject => tr!("kiln-plugin-settings-schema-not-object"),
            Self::Root(word) => tr!("kiln-plugin-settings-schema-root", keyword = word.as_str()),
            Self::TooManyKeys => tr!("kiln-plugin-settings-schema-too-many-keys", max = MAX_KEYS),
            Self::Key { key, reason } => tr!(
                "kiln-plugin-settings-schema-key",
                key = key.as_str(),
                reason = reason.to_string()
            ),
        })
    }
}

fn scalar(name: &str) -> Option<Kind> {
    Some(match name {
        "string" => Kind::String,
        "number" => Kind::Number,
        "integer" => Kind::Integer,
        "boolean" => Kind::Boolean,
        _ => return None,
    })
}

fn to_toml(json: &Json) -> Option<Value> {
    Some(match json {
        Json::String(text) => Value::String(text.clone()),
        Json::Bool(flag) => Value::Boolean(*flag),
        Json::Number(n) => match n.as_i64() {
            Some(i) => Value::Integer(i),
            None => Value::Float(n.as_f64()?),
        },
        Json::Array(items) => Value::Array(items.iter().map(to_toml).collect::<Option<_>>()?),
        _ => return None,
    })
}

fn key_decl(decl: &Json) -> Result<Key, KeyError> {
    let Json::Object(map) = decl else {
        return Err(KeyError::NotAnObject);
    };
    if let Some(word) = map.keys().find(|word| {
        !matches!(
            word.as_str(),
            "type" | "default" | "description" | "enum" | "items" | "x-scope" | "x-secret"
        )
    }) {
        return Err(KeyError::UnknownKeyword(word.clone()));
    }
    if map.get("description").is_some_and(|text| !text.is_string()) {
        return Err(KeyError::BadDescription);
    }
    // `items` belongs to arrays alone.
    if map.contains_key("items")
        && (map.contains_key("enum") || map.get("type") != Some(&json!("array")))
    {
        return Err(KeyError::BadItems);
    }
    let kind = if let Some(names) = map.get("enum") {
        let Json::Array(names) = names else {
            return Err(KeyError::BadEnum);
        };
        let names: Option<Vec<String>> = names
            .iter()
            .map(|n| n.as_str().map(str::to_owned))
            .collect();
        match (names, map.get("type")) {
            (Some(names), None) if !names.is_empty() => Kind::Enum(names),
            (Some(names), Some(Json::String(ty))) if ty == "string" && !names.is_empty() => {
                Kind::Enum(names)
            }
            _ => return Err(KeyError::BadEnum),
        }
    } else {
        match map.get("type").and_then(Json::as_str) {
            Some("array") => {
                // The items are a bare scalar type: no `pattern`, `enum` or the like.
                let items = map
                    .get("items")
                    .and_then(Json::as_object)
                    .ok_or(KeyError::BadItems)?;
                if items.len() != 1 {
                    return Err(KeyError::BadItems);
                }
                let item = (items.get("type").and_then(Json::as_str))
                    .and_then(scalar)
                    .ok_or(KeyError::BadItems)?;
                Kind::Array(Box::new(item))
            }
            Some(name) => scalar(name).ok_or(KeyError::BadType)?,
            None => return Err(KeyError::BadType),
        }
    };
    let scope = match map.get("x-scope").and_then(|s| s.as_str().or(Some("?"))) {
        None | Some("user") => Scope::User,
        Some("project") => Scope::Project,
        _ => return Err(KeyError::BadScope),
    };
    let secret = match map.get("x-secret") {
        None => false,
        Some(Json::Bool(flag)) => *flag,
        Some(_) => return Err(KeyError::BadSecret),
    };
    let default = match map.get("default") {
        None => None,
        Some(json) => {
            let value = to_toml(json).ok_or(KeyError::BadDefault)?;
            if !kind.accepts(&value) {
                return Err(KeyError::BadDefault);
            }
            Some(value)
        }
    };
    if secret && default.is_some() {
        return Err(KeyError::SecretDefault);
    }
    Ok(Key {
        kind,
        default,
        scope,
        secret,
    })
}

/// A plugin's checked schema.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SettingsSchema {
    pub keys: BTreeMap<String, Key>,
}

/// A project's settings for one open root.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProjectSettings {
    /// Whether the project is trusted; an untrusted project's keys are dropped.
    pub trusted: bool,
    /// The project's `[plugins."<id>"]` table.
    pub table: Table,
}

/// The effective settings and what was wrong with the layers.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Resolved {
    pub values: Map<String, Json>,
    pub diagnostics: Vec<String>,
}

fn to_json(value: &Value) -> Json {
    match value {
        Value::String(text) => Json::String(text.clone()),
        Value::Integer(i) => Json::from(*i),
        Value::Float(f) => serde_json::Number::from_f64(*f).map_or(Json::Null, Json::Number),
        Value::Boolean(flag) => Json::Bool(*flag),
        Value::Array(items) => Json::Array(items.iter().map(to_json).collect()),
        Value::Datetime(date) => Json::String(date.to_string()),
        Value::Table(table) => Json::Object(
            table
                .iter()
                .map(|(key, value)| (key.clone(), to_json(value)))
                .collect(),
        ),
    }
}

impl SettingsSchema {
    /// `text` as a schema, or why it isn't the subset.
    pub fn parse(text: &str) -> Result<Self, SchemaError> {
        if text.len() > MAX_SCHEMA_BYTES {
            return Err(SchemaError::TooLarge);
        }
        let json: Json =
            serde_json::from_str(text).map_err(|error| SchemaError::NotJson(error.to_string()))?;
        let Some(root) = json.as_object() else {
            return Err(SchemaError::NotAnObject);
        };
        if let Some(word) = root.keys().find(|word| {
            !matches!(
                word.as_str(),
                "$schema" | "title" | "description" | "type" | "properties"
            )
        }) {
            return Err(SchemaError::Root(word.clone()));
        }
        if let Some(word) = ["$schema", "title", "description"]
            .into_iter()
            .find(|word| root.get(*word).is_some_and(|value| !value.is_string()))
        {
            return Err(SchemaError::Root(word.to_owned()));
        }
        if root.get("type").is_some_and(|ty| ty != "object") {
            return Err(SchemaError::Root("type".to_owned()));
        }
        let Some(properties) = root.get("properties").and_then(Json::as_object) else {
            return Err(SchemaError::NotAnObject);
        };
        if properties.len() > MAX_KEYS {
            return Err(SchemaError::TooManyKeys);
        }
        let mut keys = BTreeMap::new();
        for (name, decl) in properties {
            let key = key_decl(decl).map_err(|reason| SchemaError::Key {
                key: name.clone(),
                reason,
            })?;
            keys.insert(name.clone(), key);
        }
        Ok(Self { keys })
    }

    /// Whether `name` is a secret key, which `secrets.get` serves.
    pub fn is_secret(&self, name: &str) -> bool {
        self.keys.get(name).is_some_and(|key| key.secret)
    }

    /// The defaults, then `user`, then `project` if given: each layer checked on its own.
    pub fn resolve(&self, user: &Table, project: Option<&ProjectSettings>) -> Resolved {
        let mut out = Resolved::default();
        for (name, key) in &self.keys {
            if let Some(default) = &key.default {
                out.values.insert(name.clone(), to_json(default));
            }
        }
        self.apply(&mut out, user, false);
        if let Some(project) = project {
            if !project.trusted {
                if !project.table.is_empty() {
                    out.diagnostics
                        .push(tr!("kiln-plugin-settings-untrusted").to_string());
                }
            } else {
                self.apply(&mut out, &project.table, true);
            }
        }
        out
    }

    fn apply(&self, out: &mut Resolved, layer: &Table, project: bool) {
        for (name, value) in layer {
            let Some(key) = self.keys.get(name) else {
                out.diagnostics
                    .push(tr!("kiln-plugin-settings-unknown-key", key = name.as_str()).to_string());
                continue;
            };
            let problem = if key.secret {
                Some(tr!(
                    "kiln-plugin-settings-secret-in-file",
                    key = name.as_str()
                ))
            } else if project && key.scope != Scope::Project {
                Some(tr!("kiln-plugin-settings-not-project", key = name.as_str()))
            } else if !key.kind.accepts(value) {
                Some(tr!(
                    "kiln-plugin-settings-type-mismatch",
                    key = name.as_str()
                ))
            } else {
                None
            };
            match problem {
                Some(message) => out.diagnostics.push(message.to_string()),
                None => {
                    out.values.insert(name.clone(), to_json(value));
                }
            }
        }
    }
}

/// Where a plugin's settings layers come from; the app implements it over its settings.
pub trait SettingsSource: Send + Sync {
    /// The user's `[plugins."<id>"]` table.
    fn user(&self) -> Table;
    /// The settings of the open root `root` (a guest path), or `None` if it isn't open. Never
    /// answers for a different root.
    fn project(&self, root: &str) -> Option<ProjectSettings>;
}

/// One plugin's schema and where its layers come from.
#[derive(Clone)]
pub struct PluginSettings {
    schema: Arc<SettingsSchema>,
    source: Arc<dyn SettingsSource>,
}

impl fmt::Debug for PluginSettings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PluginSettings").finish_non_exhaustive()
    }
}

impl PluginSettings {
    pub fn new(schema: Arc<SettingsSchema>, source: Arc<dyn SettingsSource>) -> Self {
        Self { schema, source }
    }

    /// The effective settings with `root`'s project layer, if a root is named.
    pub fn get(&self, root: Option<&str>) -> Result<Resolved, Error> {
        let project = match root {
            None => None,
            Some(root) => Some(self.source.project(root).ok_or_else(|| {
                Error::InvalidArgument(tr!("kiln-plugin-settings-root-not-open").to_string())
            })?),
        };
        Ok(self.schema.resolve(&self.source.user(), project.as_ref()))
    }
}

/// The settings slice of [`HostState`]. The default has none, and `get` is unimplemented.
#[derive(Debug, Default)]
pub struct State {
    settings: Option<PluginSettings>,
}

impl State {
    pub fn new(settings: PluginSettings) -> Self {
        Self {
            settings: Some(settings),
        }
    }
}

impl Host for HostState {
    fn get(&mut self, root: Option<String>) -> Result<String, Error> {
        let Some(settings) = &self.settings.settings else {
            return unimplemented("kiln:extension/settings.get");
        };
        let resolved = settings.get(root.as_deref())?;
        for message in resolved.diagnostics {
            (self.log)(Level::Warn, message);
        }
        let text = Json::Object(resolved.values).to_string();
        if text.len() > MAX_VALUE_BYTES {
            return Err(Error::Failed(
                tr!("kiln-plugin-settings-too-large", max = MAX_VALUE_BYTES).to_string(),
            ));
        }
        Ok(text)
    }
}
