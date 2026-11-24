//! A plugin's settings problems as ordinary config problems, end to end
//! through the real config service: the plugin host hands the installed
//! plugins' schemas to fe-config, the service re-checks the settings files,
//! and the positioned diagnostics reach kiln's `ConfigDiagnostics` - which
//! is what the workbench's config-problems list and status item show
//! (T2094, follow-up of T1706).
//!
//! The third test C2 names, `plugin_settings_problem_pick_opens_file_at_key`,
//! is in `src/config_problems_tests.rs` instead: picking a row needs
//! `Workbench::config_problems()`, which is `#[cfg(test)] pub(crate)` and so
//! cannot be reached from an integration test. Widening it would mean editing
//! `src/workbench.rs`, outside this card's --scope.

use std::collections::BTreeMap;
use std::fs;
use std::sync::Arc;

use fe_config::{PluginSettingsSchema, SettingsStore, Severity};
use gpui::TestAppContext;
use kiln_platform::config::{self, ConfigService};

const SECRET: &str = "\"token\" is a secret and can't be set in a settings file; ignored";

/// The `acme` plugin's contributed schema, as the plugin host hands it over:
/// the warning text is already localized by the host, so fe-config only
/// positions it.
fn acme() -> PluginSettingsSchema {
    PluginSettingsSchema {
        id: "acme".into(),
        schema: Arc::new(serde_json::json!({
            "type": "object",
            "properties": { "token": { "type": "string" } },
        })),
        secret_keys: BTreeMap::from([("token".to_owned(), SECRET.to_owned())]),
        user_only_keys: BTreeMap::new(),
    }
}

#[gpui::test]
fn plugin_settings_problem_in_config_problems_list(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let user = dir.path().join("settings.toml");
    // A secret key set in the user's file: a problem only the plugin's
    // schema knows about.
    fs::write(&user, "[plugins.\"acme\"]\ntoken = \"s3cret\"\n").unwrap();

    // No plugin loaded yet: nothing knows `acme`, so nothing is reported.
    fe_config::settings::set_plugin_schemas(Vec::new());
    let (sender, batches) = config::batch_channel();
    let store = SettingsStore::load(Some(user.clone()), None);
    let _service = cx.update(|cx| {
        ConfigService::new()
            .settings(store.into_inner())
            .start_with(batches, cx)
    });
    cx.run_until_parked();
    assert_eq!(
        cx.update(|cx| config::diagnostics(cx).to_vec()),
        [],
        "a table for a plugin that isn't loaded is not a problem"
    );

    // The plugin loads and hands its schema over; the service re-checks.
    fe_config::settings::set_plugin_schemas(vec![acme()]);
    sender.send(vec![user.clone()]);
    cx.run_until_parked();

    let problems = cx.update(|cx| config::diagnostics(cx).to_vec());
    assert_eq!(problems.len(), 1, "{problems:?}");
    let problem = &problems[0];
    assert_eq!(problem.severity, Severity::Warning);
    assert_eq!(problem.key.as_deref(), Some("plugins.acme.token"));
    assert_eq!(problem.file, user);
    // Positioned at the key: line 2, column 1 - not an unpositioned warning
    // in the plugin's log, which is what T1706 left behind.
    assert_eq!((problem.line, problem.column), (2, 1));
    assert_eq!(problem.message, SECRET);
}

#[gpui::test]
fn plugin_settings_problem_cleared_when_plugin_unloaded(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let user = dir.path().join("settings.toml");
    fs::write(&user, "[plugins.\"acme\"]\ntoken = \"s3cret\"\n").unwrap();

    fe_config::settings::set_plugin_schemas(vec![acme()]);
    let (sender, batches) = config::batch_channel();
    let store = SettingsStore::load(Some(user.clone()), None);
    let _service = cx.update(|cx| {
        ConfigService::new()
            .settings(store.into_inner())
            .start_with(batches, cx)
    });
    cx.run_until_parked();
    assert_eq!(
        cx.update(|cx| config::diagnostics(cx).len()),
        1,
        "the loaded plugin's problem is there to begin with"
    );

    // The plugin unloads: its schema goes, and so does its problem. The
    // settings file has not changed - only what is installed has.
    fe_config::settings::set_plugin_schemas(Vec::new());
    sender.send(vec![user.clone()]);
    cx.run_until_parked();
    assert_eq!(
        cx.update(|cx| config::diagnostics(cx).to_vec()),
        [],
        "unloading the plugin clears the problem it owned"
    );
}
