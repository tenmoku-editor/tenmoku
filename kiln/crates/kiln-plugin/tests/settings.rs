// SPDX-License-Identifier: Apache-2.0

//! A plugin's settings: the schema subset, the layers, and `settings.get` with a root.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use kiln_plugin::bindings::kiln::extension::settings::Host;
use kiln_plugin::bindings::{Error, HostState};
use kiln_plugin::settings::{
    KeyError, PluginSettings, ProjectSettings, SchemaError, SettingsSchema, SettingsSource, State,
};
use serde_json::{Value as Json, json};
use toml::Table;

const SCHEMA: &str = r#"{
  "type": "object",
  "properties": {
    "check_command": { "type": "string", "default": "check", "description": "x" },
    "features": { "type": "array", "items": { "type": "string" }, "x-scope": "project" },
    "level": { "enum": ["low", "high"], "default": "low", "x-scope": "project" },
    "jobs": { "type": "integer", "default": 2, "x-scope": "project" },
    "token": { "type": "string", "x-secret": true }
  }
}"#;

struct Source {
    user: Table,
    roots: BTreeMap<String, ProjectSettings>,
}

impl SettingsSource for Source {
    fn user(&self) -> Table {
        self.user.clone()
    }
    fn project(&self, root: &str) -> Option<ProjectSettings> {
        self.roots.get(root).cloned()
    }
}

fn table(text: &str) -> Table {
    text.parse().unwrap()
}

fn project(trusted: bool, text: &str) -> ProjectSettings {
    ProjectSettings {
        trusted,
        table: table(text),
    }
}

type Logs = Arc<Mutex<Vec<String>>>;

fn host(user: &str, roots: &[(&str, ProjectSettings)]) -> (HostState, Logs) {
    let source = Source {
        user: table(user),
        roots: roots
            .iter()
            .map(|(r, p)| ((*r).to_owned(), p.clone()))
            .collect(),
    };
    let schema = Arc::new(SettingsSchema::parse(SCHEMA).unwrap());
    let logs: Logs = Arc::default();
    let sink = logs.clone();
    let mut host = HostState::new(Box::new(move |_, message| {
        sink.lock().unwrap().push(message)
    }));
    host.settings = State::new(PluginSettings::new(schema, Arc::new(source)));
    (host, logs)
}

fn get(host: &mut HostState, root: Option<&str>) -> Json {
    serde_json::from_str(&host.get(root.map(str::to_owned)).unwrap()).unwrap()
}

#[test]
fn settings_defaults_then_user_then_trusted_project() {
    let (mut host, _) = host(
        "check_command = \"clippy\"\nfeatures = [\"serde\"]\njobs = 4",
        &[("/p", project(true, "jobs = 8\nlevel = \"high\""))],
    );
    // Without a root: defaults, then the user's table; a project key is the user's own value.
    assert_eq!(
        get(&mut host, None),
        json!({"check_command": "clippy", "features": ["serde"], "level": "low", "jobs": 4})
    );
    assert_eq!(
        get(&mut host, Some("/p")),
        json!({"check_command": "clippy", "features": ["serde"], "level": "high", "jobs": 8})
    );
}

#[test]
fn settings_untrusted_project_dropped() {
    let (mut host, logs) = host("", &[("/p", project(false, "jobs = 8"))]);
    assert_eq!(
        get(&mut host, Some("/p")),
        json!({"check_command": "check", "level": "low", "jobs": 2})
    );
    assert_eq!(logs.lock().unwrap().len(), 1);
    assert!(logs.lock().unwrap()[0].contains("trusted"));
}

#[test]
fn settings_two_open_roots_each_get_own_project_keys() {
    let (mut host, _) = host(
        "",
        &[
            ("/a", project(true, "jobs = 3")),
            ("/b", project(true, "level = \"high\"")),
        ],
    );
    let a = get(&mut host, Some("/a"));
    let b = get(&mut host, Some("/b"));
    assert_eq!((&a["jobs"], &a["level"]), (&json!(3), &json!("low")));
    assert_eq!((&b["jobs"], &b["level"]), (&json!(2), &json!("high")));
}

#[test]
fn settings_root_not_open_refused() {
    let (mut host, _) = host("", &[("/a", project(true, "jobs = 3"))]);
    let error = host.get(Some("/other".into())).unwrap_err();
    assert!(matches!(error, Error::InvalidArgument(_)), "{error:?}");
}

#[test]
fn settings_secret_key_in_file_is_warned_and_ignored() {
    let (mut host, logs) = host("token = \"hunter2\"", &[]);
    let value = get(&mut host, None);
    assert!(value.get("token").is_none());
    assert!(!value.to_string().contains("hunter2"));
    let logs = logs.lock().unwrap();
    assert_eq!(logs.len(), 1);
    assert!(logs[0].contains("token") && !logs[0].contains("hunter2"));
}

#[test]
fn settings_value_type_mismatch_dropped_with_diagnostic() {
    let (mut host, logs) = host("check_command = 5\nlevel = \"medium\"", &[]);
    let value = get(&mut host, None);
    assert_eq!(value["check_command"], "check");
    assert_eq!(value["level"], "low");
    assert_eq!(logs.lock().unwrap().len(), 2);
}

#[test]
fn settings_project_user_scoped_key_is_ignored() {
    let (mut host, logs) = host("", &[("/p", project(true, "check_command = \"x\""))]);
    assert_eq!(get(&mut host, Some("/p"))["check_command"], "check");
    assert_eq!(logs.lock().unwrap().len(), 1);
}

#[test]
fn settings_values_are_size_capped() {
    let big = "x".repeat(300 * 1024);
    let (mut host, _) = host(&format!("check_command = \"{big}\""), &[]);
    let error = host.get(None).unwrap_err();
    assert!(matches!(error, Error::Failed(_)), "{error:?}");
}

#[test]
fn settings_schema_outside_the_subset_is_refused() {
    let parse = |text: &str| SettingsSchema::parse(text);
    let prop = |decl: &str| format!(r#"{{"properties": {{"k": {decl}}}}}"#);
    assert!(matches!(
        parse(&prop(r#"{"type": "object"}"#)),
        Err(SchemaError::Key {
            reason: KeyError::BadType,
            ..
        })
    ));
    assert!(matches!(
        parse(&prop(r#"{"type": "string", "pattern": "x"}"#)),
        Err(SchemaError::Key {
            reason: KeyError::UnknownKeyword(_),
            ..
        })
    ));
    assert!(matches!(
        parse(&prop(r#"{"type": "string", "default": 1}"#)),
        Err(SchemaError::Key {
            reason: KeyError::BadDefault,
            ..
        })
    ));
    assert!(matches!(
        parse(&prop(
            r#"{"type": "string", "x-secret": true, "default": "a"}"#
        )),
        Err(SchemaError::Key {
            reason: KeyError::SecretDefault,
            ..
        })
    ));
    assert!(parse("[]").is_err());
    let many: String = (0..257)
        .map(|i| format!("\"k{i}\":{{\"type\":\"string\"}},"))
        .collect();
    let text = format!(r#"{{"properties": {{{}"z": {{"type":"string"}}}}}}"#, many);
    assert_eq!(parse(&text), Err(SchemaError::TooManyKeys));
    assert_eq!(parse(&" ".repeat(70_000)), Err(SchemaError::TooLarge));
}

#[test]
fn settings_schema_root_items_and_value_shapes_are_checked() {
    let parse = |text: &str| SettingsSchema::parse(text);
    // The root: only the subset's keywords, and an object.
    assert!(matches!(
        parse(r#"{"allOf": [], "properties": {}}"#),
        Err(SchemaError::Root(word)) if word == "allOf"
    ));
    assert!(matches!(
        parse(r#"{"type": "array", "properties": {}}"#),
        Err(SchemaError::Root(_))
    ));
    assert!(parse(r#"{"type": "object", "title": "t", "properties": {}}"#).is_ok());
    // Array items are a bare scalar type.
    let prop = |decl: &str| format!(r#"{{"properties": {{"k": {decl}}}}}"#);
    for items in [
        r#"{"type": "string", "pattern": "x"}"#,
        r#"{"type": "object"}"#,
        r#"{}"#,
        r#""string""#,
    ] {
        assert!(
            matches!(
                parse(&prop(&format!(r#"{{"type": "array", "items": {items}}}"#))),
                Err(SchemaError::Key {
                    reason: KeyError::BadItems,
                    ..
                })
            ),
            "{items}"
        );
    }
    // Keyword values have their shapes.
    assert!(matches!(
        parse(&prop(r#"{"type": "string", "description": 3}"#)),
        Err(SchemaError::Key {
            reason: KeyError::BadDescription,
            ..
        })
    ));
    assert!(matches!(
        parse(&prop(r#"{"enum": ["a"], "type": 3}"#)),
        Err(SchemaError::Key {
            reason: KeyError::BadEnum,
            ..
        })
    ));
    assert!(matches!(
        parse(&prop(r#"{"type": "string", "x-scope": 1}"#)),
        Err(SchemaError::Key {
            reason: KeyError::BadScope,
            ..
        })
    ));
}

#[test]
fn settings_non_finite_numbers_are_dropped_with_a_diagnostic() {
    let schema = SettingsSchema::parse(
        r#"{"properties": {
            "ratio": {"type": "number", "default": 1.5},
            "marks": {"type": "array", "items": {"type": "number"}, "default": [1.0]}
        }}"#,
    )
    .unwrap();
    for bad in ["nan", "inf", "-inf", "+inf"] {
        let user = table(&format!("ratio = {bad}\nmarks = [1.0, {bad}]"));
        let resolved = schema.resolve(&user, None);
        assert_eq!(resolved.values["ratio"], json!(1.5), "{bad}");
        assert_eq!(resolved.values["marks"], json!([1.0]), "{bad}");
        assert_eq!(resolved.diagnostics.len(), 2, "{bad}");
    }
    let finite = schema.resolve(&table("ratio = 2\nmarks = [0.5]"), None);
    assert_eq!(finite.values["ratio"], json!(2));
    assert!(finite.diagnostics.is_empty());
}

#[test]
fn settings_root_metadata_and_items_on_other_kinds_are_refused() {
    let parse = |text: &str| SettingsSchema::parse(text);
    for root in [
        r#"{"description": 3, "properties": {}}"#,
        r#"{"title": [], "properties": {}}"#,
        r#"{"$schema": 1, "properties": {}}"#,
    ] {
        assert!(matches!(parse(root), Err(SchemaError::Root(_))), "{root}");
    }
    assert!(
        parse(r#"{"$schema": "x", "title": "t", "description": "d", "properties": {}}"#).is_ok()
    );
    let prop = |decl: &str| format!(r#"{{"properties": {{"k": {decl}}}}}"#);
    for decl in [
        r#"{"type": "string", "items": 3}"#,
        r#"{"enum": ["a"], "items": {"pattern": "x"}}"#,
        r#"{"type": "integer", "items": {"type": "string"}}"#,
    ] {
        assert!(
            matches!(
                parse(&prop(decl)),
                Err(SchemaError::Key {
                    reason: KeyError::BadItems,
                    ..
                })
            ),
            "{decl}"
        );
    }
}

/// C3 of T2094. The same problems used to reach the plugin's log at every
/// `settings.get`; they are positioned config problems now, so the log keeps
/// one warning of each per load. The values `get` returns do not change.
#[test]
fn settings_problems_are_logged_once_per_load_not_per_call() {
    let (mut host, logs) = host("token = \"hunter2\"\ncheck_command = 5", &[]);
    let first = get(&mut host, None);
    // The guard is taken once and dropped before the assertion: a failing
    // `assert_eq!` formats its arguments while the receiver's temporary guard
    // is still alive, and std's Mutex is not reentrant, so locking twice in
    // one assert expression deadlocks on failure instead of reporting it.
    let seen = logs.lock().unwrap().clone();
    assert_eq!(seen.len(), 2, "{seen:?}");

    // Three more calls: the same two problems, already logged.
    for _ in 0..3 {
        assert_eq!(get(&mut host, None), first, "the values are unchanged");
    }
    let seen = logs.lock().unwrap().clone();
    assert_eq!(
        seen.len(),
        2,
        "logged once each, not once per call: {seen:?}"
    );
}
