# Built-in grammars with a C scanner: migrate to WebAssembly or stay native (T2173)

Status: **measured, awaiting the user's approval of the set.** The lead files
one migration card per migrated grammar (in T1322's shape,
`docs/specs/yd7vp95v91-*`) after the user approves. The stay-native set is the
input to the fuzzing card that depends on this one. Nothing here changes a
grammar, a query or a threshold.

## What was measured

The 16 grammars (from 14 crates) that fe-syntax's `default` features compile
in with a hand-written C external scanner. Each was built as WebAssembly from
the source the app links: the `Cargo.lock` crate, or `vendor/` for just and
the patched yaml. The bench-only sources are
`crates/fe-syntax/benches/native-as-wasm/<language>/grammar-source.toml`, and
`GRAMMAR_ROOT=crates/fe-syntax/benches/native-as-wasm
scripts/build-grammar-wasm.sh` builds them. Every module built, with
wasi-sdk 34 and the script's flags, in 12 s for all 16. Every module loaded:
the T1005 table validator refused none. Each one's highlights of its golden
sample equal the built-in's, with the built-in's own queries. So all 16 were
timed; none is "not comparable".

The figures are PERF.md's "Built-in grammars with a C scanner, as
WebAssembly (T2173)" tables:
two quiet-Mac runs, E and F, under the existing Thresholds (parse ≤2×
acceptable, 2–3× warning, >3× regression; load under 500 ms).

What a wasm grammar takes out of the app's address space is the generated
lexer and the external scanner. Those are the only grammar functions that run
in the module. tree-sitter's parser core (`lib.c`), which walks the parse
tables, stays native either way: `ts_wasm_store_load_language` copies the
tables to the host and checks them (`ts_wasm_language__validate`). So
migration removes exactly the hand-written C this card is about, and none of
tree-sitter's own code.

## Per grammar

Scanner size is lines of `scanner.c`, plus any header the scanner includes
from the crate: `common/scanner.h` for typescript, tsx and xml, and `tag.h`
for html. "Allocates" means the scanner holds heap state that grows with the
input. That is the part a malformed file can push past a bounds check.
Earlier fuzz findings are from `docs/decisions/khgdxezget-fuzzing.md`, with
saved inputs in `fuzz/regressions/syntax_edits/`. That run was built under
ASan and found no memory errors in any grammar. The findings are
incremental-versus-fresh tree differences and one hang.

The rule applied: **migrate** a grammar whose scanner keeps heap state that
grows with the input (a stack or strings, rebuilt from the serialized state
on every reparse), since that is where a hostile file overruns a buffer.
**Stay native** where the scanner's state is fixed size, as long as it passes
and no finding points at the scanner. Every grammar passed the load
threshold, and all but HTML passed parse outright, so no verdict rests on a
failed threshold. The parse ratios are E / F, wasm over native.

| Grammar | Scanner (lines) | Allocates | Earlier fuzz findings | Parse, golden | Parse, 5k | Load | Verdict | Reason |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| bash | 1,217 | yes: a heredoc array and delimiter strings, sized from serialized lengths | **open**: 6 heredoc incremental/fresh mismatches (finding 1) | 1.59× / 1.62× | 1.64× / 1.64× | 106 ms | **migrate** | Largest growing state of the set, open scanner findings, any shell script from a clone. `dotenv` shares the grammar and moves with it. |
| css | 100 | no | none | 1.52× / 1.57× | 1.50× / 1.50× | 44 ms | stay native | 100 lines of fixed state, nothing found. Injected by HTML and eight wasm hosts, so offline styles in HTML need it. |
| dockerfile | 907 | yes: `malloc`'d heredoc strings and delimiter copies | none | 1.78× / 1.80× | 1.74× / 1.79× | 34 ms | **migrate** | Heredoc buffers copied by length from the serialized state, the same shape as bash's. |
| html | 362 + 385 (`tag.h`) | yes: a tag stack with custom tag names, sized from a serialized count | none | 2.10× / 2.09× | 2.14× / 2.03× | 14 ms | **migrate** (warning) | Growing state over the most hostile input (pages from anywhere). Parse is a warning, not a regression, under the Thresholds; the card re-measures it. |
| javascript | 364 | no | 1, core: comment placement (finding 5) | 1.80× / 1.85× | 1.81× / 1.83× | 34 ms | stay native | Fixed state. Its finding is tree-sitter's core, which is native under wasm too. Injected by HTML and ten hosts. |
| just | 317 | no: one fixed-size `calloc` | **never fuzzed** (added in T1298, after the runs) | 1.64× / 1.61× | 1.57× / 1.57× | 15 ms | stay native | Fixed state. Fuzz it first: it is the one grammar here with no coverage at all. |
| lua | 195 | no: one fixed `ts_calloc` | none | 1.46× / 1.51× | 1.54× / 1.51× | 15 ms | stay native | Small, fixed state, nothing found. |
| markdown | 1,602 | yes: the open-block stack, `realloc`'d as blocks nest | none in the block grammar | 1.42× / 1.38× | 1.40× / 1.42× | 73 ms | **migrate** | The largest scanner, with growing state, over READMEs from any clone. Moves with `markdown_inline`, and `rustdoc` with it (below). |
| markdown_inline | 397 | no: one fixed `malloc` | **open**: 49 mismatch inputs (finding 2) | 1.33× / 1.28× | 1.32× / 1.31× | 75 ms | **migrate** | The most open findings of any grammar. It's one crate and one injection pair with markdown, so they move together. |
| python | 437 | yes: indent and delimiter stacks | 1, core: error recovery (finding 4) | 1.56× / 1.58× | 1.59× / 1.60× | 32 ms | **migrate** | Stacks rebuilt from serialized counts on every reparse. |
| rust | 403 | no: one fixed `ts_calloc` | none | 1.61× / 1.57× | 1.50× / 1.57× | 51 ms | stay native | Fixed state, nothing found. It also has the most native-only features and tests (154 files) of any language. |
| toml | 82 | no | none | 1.48× / 1.45× | 1.50× / 1.42× | 16 ms | stay native | The smallest scanner here, fixed state. |
| typescript | 13 + 347 (`common/scanner.h`) | no | none | 1.78× / 1.79× | 1.82× / 1.79× | 47 ms | stay native | Fixed state, shared with tsx. |
| tsx | 13 + 347 (`common/scanner.h`) | no | 2, core: the reparse hang (finding 3) | 1.80× / 1.83× | 1.80× / 1.81× | 52 ms | stay native | Fixed state. The hang is in tree-sitter's core, which wasm doesn't sandbox; the background parse cap bounds it (`tests/runaway_parse.rs`). |
| xml | 270 + 155 (`common/scanner.h`) | yes: a tag-name stack | none | 1.49× / 1.47× | 1.53× / 1.40× | 21 ms | **migrate** | Growing state over files from anywhere (feeds, SVG, plists). |
| yaml | 1,417 | yes: indent-type and indent-length stacks | none since the vendored fix. Its scanner had a real overflow (an `int16_t` row count, `vendor/tree-sitter-yaml/README.md`) | 1.44× / 1.38× | 1.44× / 1.40× | 31 ms | **migrate** | A large scanner with growing state, which has already shipped one integer-overflow bug. |

**Migrate (8):** bash (with dotenv), dockerfile, html, markdown, markdown_inline,
python, xml, yaml.

**Stay native (8), the fuzzing card's input:** css, javascript, just, lua,
rust, toml, typescript, tsx. Just goes first, since it has never been fuzzed.

Two calls the migration cards make, beyond the list below:

- **Markdown and rustdoc.** The `rustdoc` and `rustdoc_inline` built-ins
  (`src/language.rs`, `all(markdown, rust)`) use tree-sitter-md with queries
  of their own. Migrating markdown removes that grammar from the binary only
  if rustdoc also loads the wasm modules. That means registering each module
  a second time under the rustdoc names with rustdoc's queries, and Rust
  doc comments highlight plainly until it's installed. The alternative is
  keeping tree-sitter-md native for rustdoc alone: the scanner is then still
  linked, but reachable only through Rust doc comments.
- **HTML's warning.** If the migration card's own measurement crosses 3×,
  HTML stays native and joins the fuzzing set.

Nothing in this set can ship until the bundled path below exists (a user
decision).

## How a migrated day-one language would ship

Every one of these 16 is a day-one language: it highlights on first launch,
with no network, today. First-party WebAssembly grammars install on demand
from the signed registry (T1322, T1647; `crates/fastedit/src/grammar_registry.rs`).

**The app can't carry a `.wasm` grammar in its bundle today.**

- `scripts/bundle-macos.sh` ships only each extension's licence texts into
  `Contents/Resources` (lines 279–286), never a module.
  `scripts/package-linux.sh` (118–123) and `scripts/package-windows.ps1` (84)
  do the same.
- Extensions load from the user state directory
  (`kiln/crates/kiln-core/src/paths.rs:244`) and from a workspace's
  `.tenmoku/extensions` (`paths.rs:259`). Nothing resolves a path inside the
  app bundle or next to `current_exe`.
- The only `include_bytes!` of a `.wasm` is in tests.
- `scripts/package-language-pack.sh` makes an offline tarball, but it unpacks
  into the user's extensions directory, not the bundle.

So migrating any of these languages first needs a bundled-grammar path: the
modules built in CI, shipped in the bundle (or embedded), loaded at startup
before the registry is asked, and covered by the bundle's code signature
rather than the index's. Without that path, migrating a day-one language
means it stops highlighting offline until it's installed. Both the new
loading path and that interim trade-off are **user decisions**. This card
only marks which grammars should move once there's a way to ship them.

## What each migration card must plan for

From T1322 (`docs/specs/yd7vp95v91-make-the-default-features-the-day-one.md`,
"Lead decision") and `docs/grammar-size-audit/README.md` (on-demand path,
result of card 4). This applies to every migrated language:

- **Built in or extension, never both.** `register_language` refuses a
  registered grammar whose name or extensions clash with a built-in, so the
  feature leaves fe-syntax's `default` list but keeps its `Def`.
  `tests/default_built_in_set.rs` pins the list. `scripts/check-syntax-features.sh`
  and `tests/wasm_parity.rs` must keep passing.
- **The extension carries the highlight overrides.** Several built-ins add our
  own `crates/fe-syntax/queries/<language>.scm` after the crate's query, and
  bash adds an inline pattern in `src/language.rs`. The extension's
  `highlights.scm` must be that composition, which is what this bench wrote.
  Otherwise the golden file changes.
- **Outline, definitions, indent, folds and text objects stay native,** keyed
  by language name (`outline.rs`, `indent.rs`, `fold.rs`, `textobject.rs`).
  A registered grammar with the same name picks them up, compiled lazily
  against its grammar (`outline.rs:2428–2434`). Neither `GrammarSpec` nor the
  extension manifest can supply them. That `Query::new` panics on a node name
  the wasm build lacks, so each card tests every such query against the wasm
  module.
- **Tests that name the language.** Gate a test with `cfg(feature)` when the
  language is its subject, and port it to a default language when the
  language is incidental. Never add language features to dev-dependencies:
  feature unification would hide the change. T1322 broke 14 tests outside
  fe-syntax (fe-nav, fe-workbench, fe-gpui).
- **Injection hosts.** A language other grammars inject into must be installed
  before the host's injection resolves. Until then the span stays plain, so a
  host needs `parity_needs` (or a bundled path) for it.
- **Manifest data** the built-in had in code: comment tokens, shebangs, file
  names, and the first-line and content sniffing in `detect.rs` and `sniff.rs`.

Per language, from a search of `crates/fe-syntax` (the test counts are `.rs`
files on test paths that name the language; fe-syntax's share in brackets):

| Language | Injected by | Native-only features to keep working | Tests naming it |
| --- | --- | --- | --- |
| bash | make; elixir, julia, nix extensions; fences | outline, definitions, folds, detection, modelines; `dotenv` shares the grammar | 33 (25) |
| css | html, JS tagged templates; angular, astro, blade, haskell, heex, liquid, svelte, vue | outline, indent, folds | 29 (12) |
| dockerfile | fences | outline, folds | 7 (7) |
| html | markdown, markdown_inline, JS templates; dot, ejs, erb, handlebars, jinja, liquid, razor, twig, haskell | folds, content sniff, detection | 24 (16) |
| javascript | html; angular, blade, ejs, elixir, haskell, heex, http, liquid, svelte, vue | outline, definitions, imports, indent (with JSX), folds, detection | 39 (25) |
| just | fences | folds, detection | 6 (3) |
| lua | vim | outline, definitions, indent, folds, text objects | 20 (15) |
| markdown | elixir, gleam, julia, lean; its grammar also backs `rustdoc` | outline, folds | 46 (27) |
| markdown_inline | markdown; its grammar also backs `rustdoc_inline` | comment tokens | 8 (4) |
| python | latex, rst, elixir, vim | outline, definitions, indent, folds, detection, `queries/python.scm` | 86 (33) |
| rust | fences | outline, definitions, indent, folds, `parse_str`, rust and rustdoc injections | 154 (39) |
| toml | markdown front matter | outline, folds, `queries/toml.scm` (stands in for the crate's) | 38 (22) |
| typescript | astro, haskell, svelte, vue | outline, definitions, imports, indent, folds, detection | 33 (22) |
| tsx | vue | indent, folds, imports | 24 (17) |
| xml | C# doc comments; http | folds, content sniff, detection, `injection.rs`, `queries/xml.scm` | 19 (15) |
| yaml | markdown front matter; liquid | outline, folds, `queries/yaml.scm` | 20 (19) |

Every language is also injected by Markdown fences by info string
(`fence.rs`), and by perl, typst and rst where they name it.

## Grammar security: layers and gaps

This replaces a separate docs page. Each layer and the file that enforces it:

1. **Pinned grammar sources.** Each first-party wasm grammar names its exact
   source in `extensions/grammars/*/grammar-source.toml`: a crate version
   equal to `Cargo.lock`'s, a 40-hex git commit, or a crates.io `.crate` with
   its SHA-256. `scripts/build-grammar-wasm.sh` refuses a mismatch, and
   `crates/fe-syntax/tests/wasm_parity.rs` checks the versions. The native
   built-ins are pinned by `Cargo.lock`, plus `vendor/` for the patched and
   vendored ones.
2. **Our CI builds the modules.** `.github/workflows/grammars.yml` builds every
   module from those sources with a pinned, hash-checked wasi-sdk, runs the
   parity check and publishes the files. No module comes from an upstream
   binary.
3. **The signed index.** `crates/fastedit/src/grammar_registry.rs`
   (`verify_index`) checks the index's Ed25519 signature with the update
   feed's key before trusting any module's hash, and keeps the last good
   index.
4. **The wasmtime sandbox and epoch deadlines.** Grammar code runs in a
   wasmtime store and can't address host memory (`crates/fe-syntax/src/runtime.rs`;
   the vendored patches in `vendor/tree-sitter/README.md`). The epoch ticks
   every 5 ms. A load is stopped at 10 s and a parse at 5 s
   (`GrammarDeadlines::DEFAULT`, `runtime.rs:56–60`). The patches also turn
   the store's asserts on a malformed module into errors, type-check table
   entries, trap lexer callbacks outside a lex, and bounds-check token ids.
5. **Load-time table validation.** Every static table a module hands the
   parser core is checked before it's used (`ts_wasm_language__validate`;
   `crates/fe-syntax/tests/wasm_table_validation.rs`, with a hostile module
   for each table). A module over `MAX_GRAMMAR_BYTES` (20 MiB, `runtime.rs:43`)
   is refused.
6. **A memory cap.** Today there is only upstream's fixed 128 MiB maximum per
   store (`vendor/tree-sitter/src/wasm_store.c`, `MAX_MEMORY_SIZE`). No
   `ResourceLimiter` sits on the grammar path, and memory in use isn't
   measured. The memory-cap card filed with this one adds a configurable cap.
7. **Folder trust for project-supplied grammars.** A workspace can supply
   grammars in `<root>/.tenmoku/extensions`
   (`crates/fastedit/src/workspace_extensions.rs`; kinds in
   `crates/fastedit/src/config.rs:269`). They load only for a root the user
   has trusted (`kiln/crates/kiln/src/extensions.rs:56–61`, through
   `kiln_core::trust::TrustStore`), and are dropped when trust is withdrawn.
   They aren't in the signed index, so trust is their only gate before the
   sandbox.

**Gaps:**

- **Native grammars run with the app's rights.** The stay-native set below,
  and every built-in without a scanner, run generated C (and here,
  hand-written scanners) in the editor's process. A memory bug there is
  native memory corruption. The fuzzing card that follows this one is the
  mitigation for the stay-native set. Tables without a scanner are generated
  code with no heap state of their own, which is the lower risk.
- **tree-sitter's parser core is native even for wasm grammars.** A core bug,
  such as the TSX reparse that never ends (fuzz finding 3), is reached through
  any grammar. That hang is bounded by the background parse cap
  (`crates/fe-syntax/tests/runaway_parse.rs`), not by the sandbox.
- **No per-grammar memory cap below 128 MiB** until the memory-cap card lands
  (layer 6).
- **Workspace grammars skip layers 1–3.** A trusted folder's module is built
  by whoever wrote it. The sandbox, deadlines and validation still apply.
- **Bundled grammars don't exist yet.** If they're added, their integrity
  comes from the app's code signature, not the index. That's equivalent for
  a signed build, but a loader that reads a writable location would not be.
- **Highlight queries run natively** over any grammar's tree. They're ours or
  pinned with the grammar, and the query budget bounds their time.

## Not done here

- No migration card is filed (the lead files them after the user approves).
- No grammar, query or threshold changed. No CI check was added: the
  thresholds stay a documented policy (T1338).
