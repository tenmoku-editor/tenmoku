//! `[files.associations]`: glob patterns to language names, in the user's
//! settings and a project's. The project's replace the user's of the same
//! pattern; each file's are kept apart too, in its order
//! (`files.user_entries`, `files.project_entries`), so the host can put the
//! project's first, break ties by the order and keep the user's whole. An entry that isn't a language's name is
//! dropped with a warning; whether a pattern is a glob and its language is
//! there, the host checks, as a grammar may be installed later.

use std::{collections::BTreeMap, path::PathBuf};

use fe_config::{
    Settings, SettingsModel, SettingsStore, Severity, settings::FilesSettings, settings_schema,
};
use serde_json::json;

fn load(user: &str, project: Option<&str>) -> (SettingsStore, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let user_path = dir.path().join("settings.toml");
    std::fs::write(&user_path, user).unwrap();
    let project_path = project.map(|text| {
        let path = dir.path().join("project.toml");
        std::fs::write(&path, text).unwrap();
        path
    });
    (SettingsStore::load(Some(user_path), project_path), dir)
}

/// `entries` as `(pattern, language)` pairs.
fn pairs<'a>(entries: impl Iterator<Item = (&'a str, &'a str)>) -> Vec<(&'a str, &'a str)> {
    entries.collect()
}

fn map(entries: &[(&str, &str)]) -> BTreeMap<String, String> {
    (entries.iter())
        .map(|&(pattern, language)| (pattern.to_owned(), language.to_owned()))
        .collect()
}

#[test]
fn none_by_default() {
    assert_eq!(Settings::default().files, FilesSettings::default());
    let (store, _dir) = load("", None);
    assert!(store.settings().files.associations.is_empty());
    assert!(store.settings().files.user_entries.is_empty());
    assert!(store.settings().files.project_entries.is_empty());
}

#[test]
fn the_users_associations_load() {
    let user = "[files.associations]\n\"*.conf\" = \"ini\"\n\"**/templates/*.html\" = \"xml\"\n";
    let (store, _dir) = load(user, None);
    assert_eq!(store.diagnostics(), []);
    let files = &store.settings().files;
    let want = map(&[("*.conf", "ini"), ("**/templates/*.html", "xml")]);
    assert_eq!(files.associations, want);
    assert!(files.project_entries.is_empty());
    // In the file's order, not the table's sorted one.
    let user = pairs(files.user_associations());
    assert_eq!(user, [("*.conf", "ini"), ("**/templates/*.html", "xml")]);
}

/// The order ties are broken by is the file's, either way round, and a
/// reload that only reorders the entries changes it.
#[test]
fn the_entries_keep_their_files_order_through_reloads() {
    let ab = "[files.associations]\n\"a*\" = \"json\"\n\"*z\" = \"toml\"\n";
    let ba = "[files.associations]\n\"*z\" = \"toml\"\n\"a*\" = \"json\"\n";
    let (mut store, dir) = load(ab, Some(ba));
    let files = &store.settings().files;
    assert_eq!(
        pairs(files.user_associations()),
        [("a*", "json"), ("*z", "toml")]
    );
    assert_eq!(
        pairs(files.project_associations()),
        [("*z", "toml"), ("a*", "json")]
    );

    let user = dir.path().join("settings.toml");
    std::fs::write(&user, ba).unwrap();
    let event = store.reload(&[user]).expect("a reorder is a change");
    assert_eq!(
        pairs(event.settings.files.user_associations()),
        [("*z", "toml"), ("a*", "json")]
    );
    let project: PathBuf = dir.path().join("project.toml");
    std::fs::write(&project, ab).unwrap();
    let event = store.reload(&[project]).expect("a reorder is a change");
    assert_eq!(
        pairs(event.settings.files.project_associations()),
        [("a*", "json"), ("*z", "toml")]
    );
    // An inline table and dotted keys keep theirs too.
    let inline = "[files]\nassociations = { \"b*\" = \"ini\", \"a*\" = \"json\" }\n";
    let (store, _dir) = load(inline, None);
    assert_eq!(store.diagnostics(), []);
    let files = &store.settings().files;
    assert_eq!(
        pairs(files.user_associations()),
        [("b*", "ini"), ("a*", "json")]
    );
    let dotted = "[files]\nassociations.\"b*\" = \"ini\"\nassociations.\"a*\" = \"json\"\n";
    let (store, _dir) = load(dotted, None);
    assert_eq!(store.diagnostics(), []);
    let files = &store.settings().files;
    assert_eq!(
        pairs(files.user_associations()),
        [("b*", "ini"), ("a*", "json")]
    );
}

#[test]
fn a_projects_replace_the_users_of_the_same_pattern_and_combine_with_the_rest() {
    let user = "[files.associations]\n\"*.conf\" = \"ini\"\n\"*.html\" = \"xml\"\n";
    let project = "[files.associations]\n\"*.html\" = \"html\"\n\"Jenkinsfile*\" = \"bash\"\n";
    let (store, _dir) = load(user, Some(project));
    assert_eq!(store.diagnostics(), []);
    let files = &store.settings().files;
    let merged = map(&[
        ("*.conf", "ini"),
        ("*.html", "html"),
        ("Jenkinsfile*", "bash"),
    ]);
    assert_eq!(files.associations, merged);
    // Each file's own, the user's whole though the project replaces one.
    let user = pairs(files.user_associations());
    assert_eq!(user, [("*.conf", "ini"), ("*.html", "xml")]);
    let project = pairs(files.project_associations());
    assert_eq!(project, [("*.html", "html"), ("Jenkinsfile*", "bash")]);
}

#[test]
fn an_entry_that_isnt_a_language_name_is_dropped_with_a_warning() {
    let user = "[files.associations]\n\"*.a\" = 3\n\"*.b\" = \"\"\n\"*.c\" = \"ini\"\n";
    let (store, _dir) = load(user, None);
    let diagnostics = store.diagnostics();
    assert_eq!(diagnostics.len(), 2, "{diagnostics:?}");
    for (diagnostic, pattern, line) in [(&diagnostics[0], "*.a", 2), (&diagnostics[1], "*.b", 3)] {
        assert_eq!(diagnostic.severity, Severity::Warning);
        assert_eq!(
            diagnostic.key.as_deref(),
            Some(format!("files.associations.{pattern}").as_str())
        );
        assert_eq!(diagnostic.line, line);
        assert!(diagnostic.message.contains(pattern), "{diagnostic}");
    }
    // The rest still applies.
    assert_eq!(store.settings().files.associations, map(&[("*.c", "ini")]));

    let (store, _dir) = load("[files]\nassociations = [\"*.c\"]\n", None);
    assert_eq!(store.diagnostics().len(), 1, "{:?}", store.diagnostics());
    assert_eq!(store.diagnostics()[0].severity, Severity::Warning);
    assert_eq!(
        store.diagnostics()[0].key.as_deref(),
        Some("files.associations")
    );
    assert!(store.settings().files.associations.is_empty());
}

#[test]
fn a_bad_entry_in_a_project_file_is_dropped_too() {
    let project = "[files.associations]\n\"*.a\" = true\n\"*.c\" = \"ini\"\n";
    let (store, _dir) = load("", Some(project));
    assert_eq!(store.diagnostics().len(), 1, "{:?}", store.diagnostics());
    assert!(store.diagnostics()[0].file.ends_with("project.toml"));
    let files = &store.settings().files;
    assert_eq!(files.associations, map(&[("*.c", "ini")]));
    assert_eq!(pairs(files.project_associations()), [("*.c", "ini")]);
    assert!(files.user_entries.is_empty());
}

#[test]
fn no_file_sets_the_entries() {
    for key in ["user_entries", "project_entries"] {
        let text = format!("[files]\n{key} = [[\"*.x\", \"ini\"]]\n");
        for (user, project) in [(text.as_str(), None), ("", Some(text.as_str()))] {
            let (store, _dir) = load(user, project);
            let diagnostics = store.diagnostics();
            assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
            assert_eq!(diagnostics[0].severity, Severity::Warning);
            let want = format!("files.{key}");
            assert_eq!(diagnostics[0].key.as_deref(), Some(want.as_str()));
            let files = &store.settings().files;
            assert!(files.user_entries.is_empty() && files.project_entries.is_empty());
        }
    }
}

#[test]
fn the_schema_takes_a_table_of_strings() {
    let schema = settings_schema();
    let files = &schema["$defs"]["FilesSettings"]["properties"];
    assert_eq!(files["associations"]["type"], json!("object"));
    assert_eq!(
        files["associations"]["additionalProperties"]["type"],
        json!("string")
    );
    let description = files["associations"]["description"].as_str().unwrap();
    assert!(description.contains("most specific"), "{description}");
    // A file using it has no unknown keys.
    let (store, _dir) = load("[files.associations]\n\"*.x\" = \"ini\"\n", None);
    assert_eq!(store.diagnostics(), []);
}

#[test]
fn the_settings_editor_merges_them_and_lets_a_project_set_them() {
    let user = "[files.associations]\n\"*.conf\" = \"ini\"\n\"*.html\" = \"xml\"\n";
    let project = "[files.associations]\n\"*.html\" = \"html\"\n";
    let model = SettingsModel::new(Some(user), Some(project));
    let entry = |key: &str| {
        (model.entries().iter())
            .find(|entry| entry.key_path() == key)
            .unwrap_or_else(|| panic!("no {key} row"))
    };
    let associations = entry("files.associations");
    assert!(model.workspace_allowed(associations));
    assert_eq!(
        model.value(associations).value,
        json!({ "*.conf": "ini", "*.html": "html" })
    );
    for key in ["files.user_entries", "files.project_entries"] {
        assert!(!model.workspace_allowed(entry(key)), "{key}");
    }
}
