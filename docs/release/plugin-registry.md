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

## How a plugin reaches users

`.github/workflows/grammars.yml` publishes the plugins with the grammars
(`docs/release/grammar-registry.md`, "Publishing"). A first-party plugin needs
no app release:

1. **Merge to `main`.** A push touching `extensions/plugins/**` or
   `scripts/build-plugin-registry.sh` runs the workflow, as the grammars' paths
   do, and it can be run by hand.
2. **Build.** The `build` job, which holds no secret, builds the grammar
   registry, then runs
   `scripts/build-plugin-registry.sh --plugins-dir extensions/plugins --out target/registry`
   with the `wasm32-wasip2` target. The module is built from source; a `.wasm`
   checked into the plugin's directory isn't what's published. The archives and
   the combined index go to the `publish` job as one artifact.
3. **Sign.** The `publish` job signs `index.json` once, which covers the
   grammars and the plugins.
4. **Publish.** `scripts/publish-grammar-registry.sh` creates a release for
   each new plugin archive and uploads the index last.
5. **Install.** The index lists the plugin as an optional install (above). An
   app that installs from the registry checks the index's signature, downloads
   the archive from its URL and checks it against the entry ("Signing").

### An archive's release and URL

Each plugin archive is one release of `tenmoku-editor/registry`, holding
exactly that file. Its tag is a content address, as a language's is:
`<id>-<the first 16 hex digits of the archive's SHA-256>`, with no prefix. The
entry has no URL or tag of its own: the archive is at the index's
`url_template` with `{tag}` replaced by that tag and `{file}` by the entry's
`file`. For Blocks 0.1.0:

```text
https://github.com/tenmoku-editor/registry/releases/download/tenmoku.blocks-<16 hex digits>/tenmoku.blocks-0.1.0.tar.gz
```

The in-app download (T2076) builds the tag the same way, from the entry's `id`
and `sha256`. The publish refuses an id, version or file name with anything but
ASCII letters, digits, `.`, `_` and `-` (so no `+` build metadata), since
GitHub may rename an asset and a tag must be a valid git tag.

The release is never replaced or deleted, because an older cached index can
still point at it. Before publishing anything, the script checks that each
archive in the artifact has the entry's size and SHA-256, and that an existing
release isn't a draft and holds one asset with the entry's name and size. The
tag holds only part of the digest, so the script also downloads the existing
archive and checks its whole SHA-256.

### Releasing a new version

Raise `version` in the plugin's `extension.toml` and merge. The old release
stays; the new index lists only the new archive.

The publish doesn't require the raise: any change to the archive's bytes makes
a new tag, and so a new release. Besides the plugin's own files, the bytes can
change with the WIT files it generates bindings from
(`kiln/crates/kiln-plugin/wit`, `crates/fe-plugin/wit`), the Rust toolchain
(`rust-toolchain.toml`) and possibly the build runner's Python or zlib. Those
don't start the workflow, so such a change is published, under the same
version, by the next run that something else starts. Raise the version whenever
users should see a change as one.

Removing a plugin's `registry.toml` takes it out of the next index. Its
releases stay.

### What apps do with it today

- The app in this repository reads and verifies an index with plugins, and can
  verify and unpack a listed archive, but doesn't yet download one: installing
  from the Extensions view is separate work (T2076, T2080).
- **Releases v0.2.2 to v0.2.5 refuse an index that has a `plugins` key**, since
  their reader refuses unknown keys. They keep the last index they cached, so
  they get no newer grammars, and one with no cached index offers none. v0.3.0
  and later read it.

## Known limits

- A failure in the optional `--sign` step doesn't restore the previous signed
  index and artifacts: the new, unsigned output stays, and the index is
  signed again by running the script's `--sign` or `grammar-registry sign`.
- When the plugin set is unchanged the script may rewrite `index.json` with
  different bytes (whitespace, escaping) and keep the old signature, which
  then no longer verifies. Sign after every run.

Both are left to a follow-up card.
