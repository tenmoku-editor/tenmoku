---
task_uid: "01m7xyp17qygte80d1c1rysz8h"
task: "T728"
rev: 3
type: "chore"
complexity: "s"
criteria:
  - id: "C1"
    text: "`scripts/build-plugin-registry.sh --plugins-dir extensions/plugins --out DIR` builds each plugin directory that has a `registry.toml` (wasm32-wasip2 release build, then the archive: extension.toml, the .wasm, LICENSE, README), writes one hashed artifact per plugin version and the plugin entries in exactly the T1709 `fe_registry::plugins::PluginEntry` format (id, name, version, api_minor, license, file, sha256, size, entries, capabilities; no other fields: the struct is deny_unknown_fields, so there is no `optional` field) into the same index the grammar script produces, and re-signs it with the existing update-feed signer (merging into index.json and re-signing from the script is fine; no new Rust CLI); a missing or empty plugins dir writes no plugin entries and succeeds; it refuses a plugin whose manifest id, version or license is missing. Known limit (user-approved findings_override, 2026-11-24): a failure in the optional --sign step need not restore the previous signed index/artifacts (review F1), and an unchanged plugin set may rewrite index.json bytes while keeping its old signature (review F2); both go to a Mac follow-up card, not this one."
    required: true
    manual: false
  - id: "C2"
    text: "Tests (scripts/test-build-plugin-registry.sh, `--dry-run` plus a tiny prebuilt fixture wasm, no cargo build): the archive's listing, entry fields, hash matches, and refusal cases. It does NOT edit workflows: wiring it into CI (grammars.yml publish) is a separate card needing a human workflow approval; this card names that follow-up in its finish note."
    required: true
    manual: false
  - id: "C3"
    text: "docs/release/plugin-registry.md: how a plugin gets listed, the rule that listed plugins are not bundled but offered as optional installs (stated in the doc, not as an index field; design Q5 on first-party defaults stays open), and how the index is signed."
    required: true
    manual: false
---

# Publish first-party plugins to the signed index: build-plugin-registry.sh (game 4) (Mac T2008)

# Plugin registry publishing

T1709 gives fe-registry plugin entries and verified installs but nothing builds and publishes the plugin side (design section 4.6 says 'published from CI'). T1713..T1715 will also need it. Read scripts/build-grammar-registry.sh and scripts/publish-grammar-registry.sh (a model), docs/release/grammar-registry.md, T1709's finished plugins.rs. Scope is scripts and docs only.


Read the files named before starting. Do not widen --scope; if a file outside it is needed, stop and respec. New tests go in new files.
