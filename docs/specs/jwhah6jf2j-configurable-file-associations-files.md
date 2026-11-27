---
task_uid: "01m7kpwytfx2pk2njwhah6jf2j"
task: "T374"
rev: 1
type: "feature"
complexity: "m"
criteria: []
---

# Configurable file associations (files.associations globs to languages) at user and project level, re-detecting open editors on change (Mac T1917)

User said: "Yes, we should also support configurable file name patterns at the user and project level" (after asking about detecting file type by content or comments despite the extension).

Depends on card 1 (shares crates/fe-syntax/src/detect.rs, the editor-open call sites and the resolver hook it adds). Do not start before it lands.

## Do
A settings table like VS Code's `files.associations`:

```toml
[files.associations]
"*.conf" = "nginx"
"**/templates/*.html" = "jinja"
"Jenkinsfile*" = "groovy"
```
Glob key -> language name (the ids `fe_syntax::language_names()` lists), in user settings and project settings (`.tenmoku` project file via `fe_config::project_settings_path`).

- Matching: a key with no `/` matches the file's base name; with a `/` it matches the path relative to the project root (`**` crosses directories). Files outside the root, or with no root, match only slash-free keys.
- Layering: crates/fe-config/src/settings.rs builds the store with `options()` (kiln_core `SettingsOptions`, `layer_filter` / `project_filter` as ProjectFilter). Add `files.associations` to the Settings struct, settings.schema.json, settings-template.toml (+ its l10n .ftl if the template lint requires it) and the settings model (settings_model.rs `workspace_allowed`; project files MAY set associations, so no drop filter, but check how the table merge works for tables: a project key replaces a user entry with the same key; different keys combine).
- Specificity rule (pick and document in detect.rs and the schema description): project entries before user entries; within a layer, the most specific key wins = most literal (non-wildcard) characters, a key with a slash beats one without at equal count, remaining ties go to the later key in file order. Test it.
- Precedence (document at the top of detect.rs): per-file remembered choice (card 1) > modeline > configured associations > built-in path mapping > shebang > first-line > sniff.
- Unknown language name in a value: a settings diagnostic (see how other bad values reach the problems list, crates/fe-config/src/settings.rs `drop_bad_formatters` / `drop_bad_spell_scopes` layer_filters and crates/fe-workbench/src/config_problems.rs), entry ignored, no panic. A bad glob likewise. A language not compiled in but registered later (grammar install) must not be diagnosed as unknown at load: validate against registered + built-in names at match time as well, or re-validate after install.
- Live: changing associations (reload through `Workbench::reload_settings`, crates/fe-workbench/src/workbench_root.rs:270) re-detects open editors that have no explicit choice (`is_language_chosen()` false) by calling `EditorView::detect_language`.
- Also treat `Change Language Mode` text/docs if any mention detection (docs/), one line.

## Notes
- Suggested scope: crates/fe-syntax/src/detect.rs (+ new crates/fe-syntax/src/associations.rs), crates/fe-config/src/{settings.rs,settings_model.rs}, settings.schema.json, settings-template.toml and its l10n, crates/fe-workbench/src/{workbench_root.rs,config_problems.rs,tests/mod.rs} and card 1's hook sites, tests below.
- Tests in NEW files: crates/fe-syntax/tests/file_associations.rs (glob forms, specificity, unknown names), crates/fe-config/tests/files_associations_setting.rs (user + project layering, project override of the same key, bad name diagnostic, schema accepts), crates/fe-workbench/src/tests/file_associations_redetect.rs declared from src/tests/mod.rs (shared append-only): settings change re-detects an unchosen editor, leaves a chosen one.
- Overlap: T1485 (grammar_prompt/grammar_install) only if you change `detect_language` signature; keep card 1's additive API.
- Linux-ok.

## Implementation notes

- **Layers and order.** kiln merges a project's table over the user's key by key, and a TOML table reaches the settings with its keys sorted, which loses both a pattern's file and its place in it. So a layer filter lists each file's `files.associations`, in the file's order (by key offset), as `files.user_entries` (pairs of pattern and language), and a project filter moves a project file's list to `files.project_entries`; a layer filter first drops either key from any file, with a warning (as `agents.project_permissions` is handled). `files.associations` stays the merged table (project replaces the user's same pattern, other patterns combine). The user's list stays whole, so a user pattern a project replaces still applies outside that project. A reload that only reorders entries changes the settings.
- **Ties** go to the pattern written later in its file, documented in `associations.rs`, `detect.rs` and the schema description.
- **Glob syntax** (`fe_syntax::associations`, no new dependency): `*`, `?`, `**` / `**/`, `[..]` / `[!..]`, `{a,b}`, matched ignoring ASCII case like the built-in file names. Matching is dynamic programming, so the time is the pattern's tokens times the path's characters, whatever the stars. A pattern may be at most 1024 characters, nest braces at most 8 deep and expand to at most 64 alternatives; past that it is a bad glob. Literal characters are counted from the pattern text, so the `/` of `**/` counts.
- **Diagnostics.** fe-config drops an entry whose value isn't a non-empty string, with a positioned warning (load time). Bad globs and unknown language names are checked in the workbench when its problems are listed (`config_problems.rs`, `association_problems`), against `fe_syntax::language_names()` and the grammar offers' `installed` check (`grammar_prompt::language_on_its_way`). An installed extension grammar still registering at startup is therefore not reported. Matching skips an entry whose language isn't registered. While any association names a language on its way, a poll (`watch_pending_languages`, 250 ms, then 5 s after 30 s) re-detects every window's editors and lists their problems again as each one registers. A file is read only to place a problem. The open problems list is refreshed, or dismissed when empty, whenever the associations change.
- **Per window.** `RootAssociations` (workbench_root.rs) keeps one entry per workbench (its root and the settings it was last given; the app's until its own load), registered from `Workbench::new` (`follow_file_associations`, one line in workbench.rs) and dropped when the workbench is released. The editors' hook (`EditorView::set_language_associations`, pane_item.rs) uses the deepest root any workbench has that holds the file (the newest entry of a shared root); a file under no root gets the user's patterns by name. So a project's associations apply to its own files, in any window, and to no others.
- **Re-detect.** Every change to an entry (settings reload, re-root, a new window) re-detects the unchosen editors of every window, since the lookup is shared. Diff sides with no file go through `DiffView::detect_languages_again`, which now detects by the file compared (`with_compared_path`) when there is one, through the associations, else by the file name. A new diff detects once more right after construction, once its compared path is set. Launch editors detect again once the workbench registers its associations.
- **Carried T373 F4** fixed in `language_name_for_first_line` (PHP and HTML openers before XML substrings, still subject to grammar availability), with regression cases in `tests/file_associations.rs`. **T373 F1** (view.rs) is untouched, since view.rs isn't changed here.
- **Not done:** no docs/ page mentions language detection (languages-and-lsp.md is a stub), so there was nothing to amend.
