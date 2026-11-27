---
task_uid: "01m8d3qn10m79t58xw08bdvenc"
task: "T2170"
rev: 1
type: "feature"
complexity: "s"
criteria:
  - id: "C1"
    text: "grammars.yml's build job runs scripts/build-plugin-registry.sh on target/registry after build-grammar-registry.sh and before the artifact upload, with the wasm32-wasip2 target installed; push.paths includes extensions/plugins/** and scripts/build-plugin-registry.sh; the publish job's single sign step covers the combined index"
    required: true
    manual: false
  - id: "C2"
    text: "scripts/publish-grammar-registry.sh uploads each plugin version's archive where its index entry points, never replaces a published version, checks plugin files in its verify-before-publish pass, and still uploads index.json last"
    required: true
    manual: false
  - id: "C3"
    text: "scripts/test-publish-grammar-registry.sh covers an index with a plugin entry (dry-run gh commands for the plugin upload, and refusal when a plugin file is missing or an existing release differs) and passes"
    required: true
    manual: false
  - id: "C4"
    text: "docs/release/plugin-registry.md no longer says CI isn't wired and describes how a first-party plugin reaches users"
    required: true
    manual: false
---

# CI publishes first-party plugins to the live registry: grammars.yml runs build-plugin-registry.sh before signing, and publish-grammar-registry.sh uploads the plugin archives

# Publish first-party plugins to the live registry from CI

The user (2026-11-26) wants first-party plugins such as Blocks (T2009) to show up in the Extensions view for every install. T2008 (Mac) / T728 (neon) landed `scripts/build-plugin-registry.sh`. That script adds each plugin under `extensions/plugins/*` that has a `registry.toml` listing marker to the grammar registry's `index.json`, as a `plugins` entry with a reproducible archive under `<out>/plugins/<id>/<version>/`. `docs/release/plugin-registry.md`, "Not wired into CI yet", records the two gaps this card closes:

1. `.github/workflows/grammars.yml` doesn't run the script. Its build job runs `scripts/build-grammar-registry.sh` and uploads `target/registry` as an artifact. Its publish job signs the index (`grammar-registry sign`) and runs `scripts/publish-grammar-registry.sh`. Add a step to the build job, after the grammar registry is built and before the artifact upload: install the `wasm32-wasip2` target, then run `scripts/build-plugin-registry.sh --plugins-dir extensions/plugins --out target/registry`. Signing stays where it is, so the one signature covers grammars and plugins together. Add `extensions/plugins/**` and `scripts/build-plugin-registry.sh` to the workflow's `push.paths`.
2. `scripts/publish-grammar-registry.sh` doesn't upload the plugin archives. Publish each plugin version at the URL its index entry names (read `fe_registry::plugins::PluginEntry` and the index's `url_template` for the exact release/tag shape). Follow the grammar rules: a published version's release is never replaced or deleted, an existing one must match the entry's files by name and size, and the check-everything-before-publishing-anything pass also covers plugin files. `index.json` still goes up last.

Extend `scripts/test-publish-grammar-registry.sh` (its `--dry-run` / fake gh pattern) with a plugin entry, and `scripts/test-build-plugin-registry.sh` only if the build step needs a new flag. Update `docs/release/plugin-registry.md`: replace "Not wired into CI yet" with how a plugin reaches users, and mention it in `docs/release/grammar-registry.md` where the workflow is described.

This edits a workflow, so merging needs the user's risk approval. That is expected; don't avoid it. Don't run the workflow or publish anything from the lane.

## Implementation notes

- **Reused:** `scripts/build-plugin-registry.sh` as it is (no new flag, so its test is unchanged); the publish
  script's `check_file`, `release_view`, `draft`, `sha256` and `gh_run`; the publish test's fake `gh`,
  `fake_release` and `build_dist`, whose fixture plugin goes through the real build script (`--dry-run`).
  `grammar-registry` has no command that prints plugin entries and `crates/fe-registry` is out of scope, so the
  publish script reads them with `python3 -I`, after `verify` has parsed the index.
- **Release tag: matches T2076/helium T729** (the lead checked with helium, 2026-11-27). `PluginEntry` has no
  tag or URL, so both sides derive `<id>-<first 16 hex of the archive's SHA-256>`, no prefix: a content address
  like a language's, so a release never changes and a republish can't overwrite what a cached index points at.
  It's `plugin_tag` in `scripts/publish-grammar-registry.sh`.
- **Older releases (user-accepted 2026-11-27).** v0.2.2 to v0.2.5 parse the index with `deny_unknown_fields` and
  no `plugins` key, so the first publish after this merges stops their grammar index updates. They get grammars
  again after updating to 0.3.x.
- **Stricter than name and size.** The tag holds only 16 hex digits, so an existing plugin release's archive is
  downloaded and its whole SHA-256 compared.
- **Same version, new bytes.** A rebuild (toolchain, WIT, runner Python or zlib) publishes a new release under
  the same version rather than failing (`docs/release/plugin-registry.md`, "Releasing a new version").
- **Checked locally:** the real wasm build of Blocks, signed with a throwaway key, then `--dry-run` publish. The
  workflow itself was not run and nothing was published.

