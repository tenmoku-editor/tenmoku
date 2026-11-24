//! `[plugins."<id>"]` checked against each installed plugin's contributed
//! schema, through kiln's `Layer::check_schema` and the layer filters, so a
//! problem in a plugin's settings is an ordinary positioned config problem:
//! file, line and column of the key, not an unpositioned warning in the
//! plugin's log (T2094, follow-up of T1706).
//!
//! The plugin host hands the schemas over with the warning text already
//! localized ([`PluginSettingsSchema`]), so fe-config positions messages it
//! is given and owns no new catalogue keys of its own.

use std::collections::BTreeMap;
use std::fs;
use std::sync::Arc;

use fe_config::{PluginSettingsSchema, SettingsStore, Severity};

const SECRET: &str = "\"token\" is a secret and can't be set in a settings file; ignored";
const NOT_PROJECT: &str = "\"check_command\" can't be set per project; ignored";

/// The `acme` plugin's schema: a string, a list and a secret string.
///
/// The id has no dot in it on purpose. `check_schema` builds a diagnostic's
/// key by joining the raw table keys with `.`, so a real dotted id
/// (`acme.cargo`) would give the ambiguous key `plugins.acme.cargo.<key>`.
/// That ambiguity is the pipeline's, not this card's; these tests assert on
/// an id that can't be misread.
fn acme() -> PluginSettingsSchema {
    PluginSettingsSchema {
        id: "acme".into(),
        schema: Arc::new(serde_json::json!({
            "type": "object",
            "properties": {
                "check_command": { "type": "string" },
                "features": { "type": "array" },
                "token": { "type": "string" },
            },
        })),
        secret_keys: BTreeMap::from([("token".to_owned(), SECRET.to_owned())]),
        user_only_keys: BTreeMap::from([("check_command".to_owned(), NOT_PROJECT.to_owned())]),
    }
}

fn load(user: Option<&str>, project: Option<&str>) -> (tempfile::TempDir, SettingsStore) {
    let dir = tempfile::tempdir().unwrap();
    let write = |name: &str, text: &str| {
        let path = dir.path().join(name);
        fs::write(&path, text).unwrap();
        path
    };
    let user = user.map(|text| write("settings.toml", text));
    let project = project.map(|text| write("project.toml", text));
    let store = SettingsStore::load_with_plugins(user, project, vec![acme()]);
    (dir, store)
}

/// The one diagnostic whose key is `key`, or a panic naming them all.
fn at<'a>(store: &'a SettingsStore, key: &str) -> &'a kiln_core::Diagnostic {
    let found: Vec<_> = store
        .diagnostics()
        .iter()
        .filter(|d| d.key.as_deref() == Some(key))
        .collect();
    assert_eq!(
        found.len(),
        1,
        "one diagnostic at {key}, got {:?}",
        store.diagnostics()
    );
    found[0]
}

#[test]
fn plugin_settings_user_layer_type_mismatch_is_positioned() {
    // `check_command` is a string in the schema; 3 isn't one. Line 2, and
    // column 17 is the `3`: a type mismatch is reported at the value.
    let (_dir, store) = load(Some("[plugins.\"acme\"]\ncheck_command = 3\n"), None);
    let problem = at(&store, "plugins.acme.check_command");
    assert_eq!(problem.severity, Severity::Error);
    assert_eq!((problem.line, problem.column), (2, 17));
    assert!(
        problem.file.ends_with("settings.toml"),
        "{:?}",
        problem.file
    );

    // The rest of the file still applies: a layer with an error keeps its
    // last good table, and nothing else here is wrong.
    let (_dir, store) = load(
        Some("[plugins.\"acme\"]\ncheck_command = \"clippy\"\nfeatures = [\"serde\"]\n"),
        None,
    );
    assert_eq!(store.diagnostics(), [], "a good plugin table is silent");
}

#[test]
fn plugin_settings_unknown_key_is_positioned() {
    // A key the plugin's schema doesn't declare: a warning at the key
    // itself, line 2 column 1, not an error, so the file still applies.
    let (_dir, store) = load(Some("[plugins.\"acme\"]\nnope = 1\n"), None);
    let problem = at(&store, "plugins.acme.nope");
    assert_eq!(problem.severity, Severity::Warning);
    assert_eq!((problem.line, problem.column), (2, 1));

    // A plugin that isn't installed keeps its table, with no diagnostic:
    // settings for a plugin not loaded yet are not a problem to report.
    let (_dir, store) = load(Some("[plugins.\"no.such\"]\nanything = 1\n"), None);
    assert_eq!(store.diagnostics(), [], "{:?}", store.diagnostics());
}

#[test]
fn plugin_settings_secret_key_in_file_is_error_at_key() {
    // A secret key is never read from a file. The problem is positioned at
    // the KEY (line 2, column 1), not at the value, and the key is dropped,
    // so it is not also reported as an unknown key.
    //
    // Severity is Warning, not Error, although this test's name says error:
    // decision 4.4 ("a secret key found in a settings file is a warning, and
    // is ignored") and T2094's own "messages equivalent to today's" both say
    // warning, and today's is one. The name's "at_key" is the claim that
    // distinguishes it from a value-positioned diagnostic. Raised with the
    // Mac lead rather than silently resolved either way.
    let (_dir, store) = load(Some("[plugins.\"acme\"]\ntoken = \"s3cret\"\n"), None);
    let problem = at(&store, "plugins.acme.token");
    assert_eq!(problem.severity, Severity::Warning);
    assert_eq!((problem.line, problem.column), (2, 1));
    assert_eq!(problem.message, SECRET);
    assert_eq!(
        store.diagnostics().len(),
        1,
        "dropped, so not unknown too: {:?}",
        store.diagnostics()
    );
}

#[test]
fn plugin_settings_project_non_project_scope_key_is_positioned() {
    // `check_command` is a user-scope key: a project file may not set it.
    // Positioned at the key in the PROJECT file, and the user's own value
    // for the same key is untouched.
    let (_dir, store) = load(
        Some("[plugins.\"acme\"]\ncheck_command = \"clippy\"\n"),
        Some("[plugins.\"acme\"]\ncheck_command = \"check\"\n"),
    );
    let problem = at(&store, "plugins.acme.check_command");
    assert_eq!(problem.severity, Severity::Warning);
    assert_eq!((problem.line, problem.column), (2, 1));
    assert_eq!(problem.message, NOT_PROJECT);
    assert!(
        problem.file.ends_with("project.toml"),
        "the project file's, not the user's: {:?}",
        problem.file
    );

    // The same key in the user's file alone is fine.
    let (_dir, store) = load(
        Some("[plugins.\"acme\"]\ncheck_command = \"clippy\"\n"),
        None,
    );
    assert_eq!(store.diagnostics(), [], "{:?}", store.diagnostics());
}
