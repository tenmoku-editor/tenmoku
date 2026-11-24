# Plugin registry publishing

First-party plugins are listed in the same signed index as the grammars
(`docs/release/grammar-registry.md`), under its `plugins` key. The entry format
is `fe_registry::plugins::PluginEntry` (`crates/fe-registry/src/plugins.rs`;
design `docs/decisions/dqsjfernkt-plugins-stage-3.md`, section 4.6).
`scripts/build-plugin-registry.sh` builds the archives and the entries.

## Listed means optional, not bundled

A plugin in the index is **offered as an optional install**. It isn't bundled
in the app, installed by default or turned on by default: a user installs it
from the index, sees its capabilities in the prompt, and accepts them. This is
a rule of the registry, not a field of the entry (`PluginEntry` has no such
field, and denies unknown ones). Whether any first-party plugin should instead
ship installed and on by default is design question Q5, still open; settling
it won't change the index.

## How a plugin gets listed

Put the plugin in a directory under `extensions/plugins/<name>/` with:

- `extension.toml`, the manifest (`docs/plugins/authoring.md`), which must
  have `id`, `name`, `version` and `license`, `[engine] tenmoku = "1.<minor>"`
  (the minor is the entry's `api_minor`) and `[contributes.plugin]` with `wasm`
  and `capabilities` (the entry's `capabilities`, as written);
- `Cargo.toml`, a crate that builds for `wasm32-wasip2`;
- `LICENSE*` and `README*` files, which go in the archive;
- `registry.toml`, which marks the directory for listing. It can hold
  `built = "<file>"`, the module's file name under
  `target/wasm32-wasip2/release/` when it isn't the manifest's `wasm` file
  name. A directory without a `registry.toml` is never listed.

```sh
scripts/build-grammar-registry.sh
scripts/build-plugin-registry.sh --plugins-dir extensions/plugins --out target/registry
```

For each listed directory the script builds the module
(`cargo build --release --target wasm32-wasip2`), then writes
`<out>/plugins/<id>/<version>/<id>-<version>.tar.gz` holding
`extension.toml`, the module at the manifest's `wasm` path, and the `LICENSE*`
and `README*` files. The archive is reproducible: sorted members, zero
timestamps and owners. Its entry (`id`, `name`, `version`, `api_minor`,
`license`, `file`, `sha256`, `size`, `entries`, `capabilities`) is merged into
`<out>/index.json`, replacing the plugins already there. The grammars, the
serial and the `url_template` are untouched. A manifest without id, name,
version or license, a duplicate id, or a missing module fails the run before
the index is written. With no plugins directory, or none listed, nothing is
added and the run succeeds.

`--dry-run` skips the cargo build and takes the module already at the
manifest's `wasm` path in the plugin directory; `scripts/test-build-plugin-registry.sh`
uses it with a tiny fixture module.

## Signing

The index has one signature, `index.json.sig`, over the exact bytes of
`index.json` (Ed25519, the update feed's scheme, with the registry's key;
`docs/release/grammar-registry.md`, "Signature and the committed key"). Adding
plugins changes those bytes, so the script deletes a signature the grammar
build left, and the index has to be signed after this script runs. `--sign`
does it, running `grammar-registry sign` with the private key read from stdin
(`TENMOKU_REGISTRY_PRIVATE_KEY`, never an argument), which refuses a key that
isn't `registry.toml`'s `public_key`. In CI the publish job signs the index as
it does for the grammars.

The signature vouches for the bytes the registry reviewed: there are no
per-plugin keys. An install checks the signature, then the archive's size and
SHA-256, then that the manifest asks for no capability the entry doesn't list.

## Not wired into CI yet

`.github/workflows/grammars.yml` doesn't run this script, and publishing the
archives under `plugins/` isn't in `scripts/publish-grammar-registry.sh`.
Both are a follow-up needing a human workflow approval.

## Known limits

- A failure in the optional `--sign` step doesn't restore the previous signed
  index and artifacts: the new, unsigned output stays, and the index is
  signed again by running the script's `--sign` or `grammar-registry sign`.
- When the plugin set is unchanged the script may rewrite `index.json` with
  different bytes (whitespace, escaping) and keep the old signature, which
  then no longer verifies. Sign after every run.

Both are left to a follow-up card.
