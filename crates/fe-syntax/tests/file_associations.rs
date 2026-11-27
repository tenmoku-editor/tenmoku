//! `files.associations`: glob patterns giving files a language, a project's
//! before the user's, the most specific winning within each; a pattern
//! that isn't a glob, or a language there isn't, is passed over. They come
//! after a modeline and before the built-in path mapping.

#![cfg(all(
    feature = "json",
    feature = "yaml",
    feature = "toml",
    feature = "html",
    feature = "xml",
    feature = "ini",
    feature = "python",
    feature = "bash"
))]

use std::path::{Path, PathBuf};

use fe_syntax::{
    AssociationProblem, FileAssociations, GlobError, GrammarSpec, association_problem,
    detect_language_associated, language_names, register_language, unregister_language,
};

fn user(entries: &[(&'static str, &'static str)]) -> FileAssociations {
    FileAssociations::new([], entries.iter().copied())
}

fn root() -> &'static Path {
    Path::new("/work/site")
}

/// The language `associations` give `path`, under [`root`].
fn language(associations: &FileAssociations, path: &str) -> Option<&'static str> {
    associations.language_for(Path::new(path), Some(root()))
}

#[test]
fn a_slash_free_pattern_matches_the_file_name_anywhere() {
    let associations = user(&[("*.conf", "ini"), ("Jenkinsfile*", "bash")]);
    assert_eq!(
        language(&associations, "/work/site/nginx.conf"),
        Some("ini")
    );
    assert_eq!(
        language(&associations, "/work/site/a/b/x.CONF"),
        Some("ini")
    );
    assert_eq!(language(&associations, "/etc/nginx.conf"), Some("ini"));
    assert_eq!(
        associations.language_for(Path::new("/etc/nginx.conf"), None),
        Some("ini")
    );
    assert_eq!(
        language(&associations, "/work/site/Jenkinsfile.prod"),
        Some("bash")
    );
    assert_eq!(language(&associations, "/work/site/MyJenkinsfile"), None);
    assert_eq!(language(&associations, "/work/site/nginx.cfg"), None);
}

#[test]
fn a_pattern_with_a_slash_matches_the_path_under_the_root() {
    let associations = user(&[("**/templates/*.html", "xml"), ("config/*.txt", "toml")]);
    for (path, want) in [
        ("/work/site/templates/a.html", Some("xml")),
        ("/work/site/app/templates/a.html", Some("xml")),
        ("/work/site/templates/deep/a.html", None),
        ("/work/site/config/a.txt", Some("toml")),
        ("/work/site/x/config/a.txt", None),
        // Outside the root, only slash-free patterns match.
        ("/elsewhere/templates/a.html", None),
        ("/elsewhere/config/a.txt", None),
    ] {
        assert_eq!(language(&associations, path), want, "{path}");
    }
    let rootless = Path::new("/work/site/templates/a.html");
    assert_eq!(associations.language_for(rootless, None), None);
}

#[test]
fn sets_braces_and_single_characters() {
    let associations = user(&[("*.{yml,tmpl}", "yaml"), ("[ab]?.dat", "json")]);
    assert_eq!(language(&associations, "/work/site/ci.tmpl"), Some("yaml"));
    assert_eq!(language(&associations, "/work/site/ci.yml"), Some("yaml"));
    assert_eq!(language(&associations, "/work/site/a1.dat"), Some("json"));
    assert_eq!(language(&associations, "/work/site/c1.dat"), None);
    assert_eq!(language(&associations, "/work/site/a12.dat"), None);
}

#[test]
fn the_most_literal_characters_win() {
    // `*.html` has 5 literal characters, `index.*` 6, `index.html` 10.
    let associations = user(&[
        ("*.html", "xml"),
        ("index.*", "json"),
        ("index.html", "toml"),
    ]);
    assert_eq!(
        language(&associations, "/work/site/index.html"),
        Some("toml")
    );
    assert_eq!(
        language(&associations, "/work/site/index.json"),
        Some("json")
    );
    assert_eq!(
        language(&associations, "/work/site/other.html"),
        Some("xml")
    );
}

#[test]
fn a_slash_wins_at_equal_counts_and_then_the_later_pattern() {
    // Both have 5 literal characters: `a/*.xy` has the slash.
    let associations = user(&[("a/*.xy", "json"), ("ab*.xy", "toml")]);
    assert_eq!(language(&associations, "/work/site/a/ab.xy"), Some("json"));
    // Neither has one, and both have 1: the one given later wins.
    let associations = user(&[("a*", "json"), ("*z", "toml")]);
    assert_eq!(language(&associations, "/work/site/az"), Some("toml"));
    let associations = user(&[("*z", "toml"), ("a*", "json")]);
    assert_eq!(language(&associations, "/work/site/az"), Some("json"));
    // The `/` of `**/` is a literal: `**/ab*` has 3, as `abc*` has, and its
    // slash wins.
    for entries in [
        [("**/ab*", "json"), ("abc*", "toml")],
        [("abc*", "toml"), ("**/ab*", "json")],
    ] {
        let associations = user(&entries);
        assert_eq!(language(&associations, "/work/site/abc"), Some("json"));
    }
}

#[test]
fn a_projects_patterns_come_before_the_users() {
    // The user's pattern is more specific, but the project's comes first.
    let associations = FileAssociations::new(
        [("*.html", "xml")],
        [("**/templates/*.html", "yaml"), ("*.conf", "ini")],
    );
    assert_eq!(
        language(&associations, "/work/site/templates/a.html"),
        Some("xml")
    );
    // A user's pattern the project has none like still applies.
    assert_eq!(language(&associations, "/work/site/x.conf"), Some("ini"));
}

#[test]
fn a_bad_pattern_or_an_unknown_language_is_passed_over() {
    let associations = user(&[
        ("*.{conf", "yaml"),
        ("nginx.conf", "no-such-language"),
        ("*.conf", "ini"),
    ]);
    assert_eq!(
        language(&associations, "/work/site/nginx.conf"),
        Some("ini")
    );
    assert_eq!(
        association_problem("*.{conf", "yaml"),
        Some(AssociationProblem::BadPattern(GlobError::UnclosedBrace))
    );
    assert_eq!(
        association_problem("[]", "yaml"),
        Some(AssociationProblem::BadPattern(GlobError::EmptySet))
    );
    assert_eq!(
        association_problem("", "yaml"),
        Some(AssociationProblem::BadPattern(GlobError::Empty))
    );
    assert_eq!(
        association_problem("nginx.conf", "no-such-language"),
        Some(AssociationProblem::UnknownLanguage)
    );
    assert_eq!(association_problem("*.conf", "ini"), None);
    assert!(user(&[("[x", "ini")]).is_empty());
}

/// A language registered after the associations were read (an installed
/// grammar's) applies from then on, and isn't unknown any more: the JSON
/// fixture stands in for it.
#[test]
fn a_language_registered_later_applies_once_it_is() {
    let name = "assoc-later";
    assert!(!language_names().any(|known| known == name));
    let associations = user(&[("a.later", name), ("*.later", "yaml")]);
    let path = "/work/site/a.later";
    assert_eq!(language(&associations, path), Some("yaml"));
    assert_eq!(
        association_problem("a.later", name),
        Some(AssociationProblem::UnknownLanguage)
    );
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR").expect("run by cargo");
    let wasm = PathBuf::from(manifest).join("tests/fixtures/tree-sitter-json.wasm");
    register_language(&GrammarSpec {
        language: name.into(),
        wasm,
        ..GrammarSpec::default()
    })
    .unwrap();
    let registered = language(&associations, path);
    let problem = association_problem("a.later", name);
    assert!(unregister_language(name));
    assert_eq!(registered, Some(name));
    assert_eq!(problem, None);
}

#[test]
fn associations_come_after_a_modeline_and_before_the_path() {
    let detect = |path: &str, associated, start: &str| {
        detect_language_associated(Some(Path::new(path)), associated, start, "")
    };
    // Over the built-in mapping and a shebang.
    assert_eq!(detect("a.json", Some("yaml"), "{}"), Some("yaml"));
    assert_eq!(
        detect("run", Some("python"), "#!/bin/sh\necho\n"),
        Some("python")
    );
    // Under a modeline.
    assert_eq!(
        detect("a.json", Some("yaml"), "# vim: set ft=toml:\n"),
        Some("toml")
    );
    // None leaves it to the path, then the content.
    assert_eq!(detect("a.json", None, "a: 1\n"), Some("json"));
    assert_eq!(detect("run", None, "#!/bin/sh\n"), Some("bash"));
}

/// A line opening as HTML (or PHP) is that, though it has an XML marker
/// further on (T373 F4).
#[test]
fn an_opening_marker_wins_over_an_xml_marker_later_on_the_line() {
    let detect = |start: &str| detect_language_associated(Some(Path::new("a.zz")), None, start, "");
    assert_eq!(
        detect("<html><body><svg></svg></body></html>\n"),
        Some("html")
    );
    assert_eq!(detect("<!DOCTYPE html><?xml?>\n"), Some("html"));
    assert_eq!(detect("<svg><html>\n"), Some("xml"));
    let php = language_names().any(|name| name == "php");
    let want = if php { Some("php") } else { Some("xml") };
    assert_eq!(detect("<?php echo '<?xml version=\"1.0\"?>'; ?>\n"), want);
}
