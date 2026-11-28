---
task_uid: "01m8da1tbc06trxammtvcy52fr"
task: "T2173"
rev: 1
type: "chore"
complexity: "m"
criteria:
  - id: "C1"
    text: "scripts/build-grammar-wasm.sh with GRAMMAR_ROOT=crates/fe-syntax/benches/native-as-wasm builds a module for each of the spec's 16 grammars into target/grammar-wasm-native/ from the Cargo.lock or vendor/ source the app links (a module that won't build or load is recorded in the decision file with its error); without GRAMMAR_ROOT the script behaves as before and scripts/build-grammar-wasm.sh kotlin passes"
    required: true
    manual: false
  - id: "C2"
    text: "cargo bench -p fe-syntax --bench wasm runs each listed language (WASM_BENCH_LANGUAGES filters) with Kotlin's cases, checks the wasm highlights equal the native ones on the golden sample before timing, and leaves Kotlin's cases unchanged"
    required: true
    manual: false
  - id: "C3"
    text: "PERF.md's WebAssembly grammars section has a per-language table from two quiet-Mac runs (1-minute load below 4 at start; start and end loads stated): golden and 5k-line parse ratios, load in ms, and the verdict under the existing Thresholds; the Kotlin tables and the Thresholds text are unchanged"
    required: true
    manual: false
  - id: "C4"
    text: "The decision file gives each of the 16 grammars migrate or stay native with a reason (scanner size, allocation, past fuzz findings, ratios, load), says how a migrated day-one language would ship (bundled or installed, and whether bundling exists), what its migration must plan for, and has a 'Grammar security: layers and gaps' section naming the file behind each layer"
    required: true
    manual: false
  - id: "C5"
    text: "The finish note lists the migrate set and the stay-native set; no migration card is filed, and no grammar, query or threshold changes"
    required: true
    manual: false
  - id: "C6"
    text: "cargo clippy -p fe-syntax --benches -- -D warnings passes; no file outside scope changes"
    required: true
    manual: false
---

# Bench every built-in grammar with a C external scanner as WebAssembly against PERF.md's thresholds, and decide which move to WebAssembly and which stay native

# Bench every built-in grammar with a C external scanner as WebAssembly against PERF.md's thresholds, and decide which move to WebAssembly and which stay native

User request (2026-11-26): our compiled-in grammars run their C code with the
app's rights. A bug in a hand-written external scanner, hit by a hostile
file, is native memory corruption. The same grammar as WebAssembly is
sandboxed. Bench the risky native grammars as WebAssembly against the
existing thresholds. Move the ones that pass, and fuzz the ones that stay
native (the follow-up card depends on this one).

## Which grammars

These are the built-ins in fe-syntax's `default` features (the shipped app)
that have a C external scanner, with the sources `Cargo.lock` and `vendor/`
pin. There are 16 grammars from 14 crates:

| Language(s) | Source | Scanner |
|---|---|---|
| bash (also behind `dotenv`) | tree-sitter-bash 0.25.1 | `src/scanner.c` |
| css | tree-sitter-css 0.25.0 | `src/scanner.c` |
| dockerfile | tree-sitter-containerfile 0.9.2 | `src/scanner.c` |
| html | tree-sitter-html 0.23.2 | `src/scanner.c` |
| javascript | tree-sitter-javascript 0.25.0 | `src/scanner.c` |
| just | `vendor/tree-sitter-just` | `src/scanner.c` |
| lua | tree-sitter-lua 0.5.0 | `src/scanner.c` |
| markdown, markdown_inline (also behind `rustdoc`) | tree-sitter-md 0.5.3 | `tree-sitter-markdown/src/scanner.c`, `tree-sitter-markdown-inline/src/scanner.c` |
| python | tree-sitter-python 0.25.0 | `src/scanner.c` |
| rust | tree-sitter-rust 0.24.2 | `src/scanner.c` |
| toml | tree-sitter-toml-ng 0.7.0 | `src/scanner.c` |
| typescript, tsx | tree-sitter-typescript 0.23.2 | `typescript/src/scanner.c`, `tsx/src/scanner.c` |
| xml | tree-sitter-xml 0.7.0 | `xml/src/scanner.c` (its `dtd/` scanner is compiled, but no language uses it) |
| yaml | `vendor/tree-sitter-yaml` (patched) | `src/scanner.c` |

Out of this card: built-ins without a scanner (c, go, java, json, make, ini,
diff, printf, regex, the git formats, json5, csv/tsv), since only their
generated table-driven `parser.c` runs. Also out: the eight non-default
features (cpp, ruby, php, csharp, scala, swift, kotlin, sql), which T1322
already ships as WebAssembly.

## Do

1. **Build each one as WebAssembly, the same source the app links.** Extend
   `scripts/build-grammar-wasm.sh` in two ways:
   - a `GRAMMAR_ROOT` override (default `extensions/grammars`) with its own
     output directory (`target/grammar-wasm-native/`). With the override set,
     it skips the wasm_parity run.
   - a `path = "vendor/..."` source form, for the vendored yaml and just.

   Add bench-only `grammar-source.toml` files under
   `crates/fe-syntax/benches/native-as-wasm/<language>/`. Use crate + version
   equal to `Cargo.lock` (the script checks this), and `src` pointing at the
   subdirectory for tree-sitter-md, typescript and xml. Without the override,
   the script must behave exactly as before, because
   `.github/workflows/grammars.yml` runs it.
2. **Extend `crates/fe-syntax/benches/wasm.rs`** so it runs over each language
   above (with a `WASM_BENCH_LANGUAGES=a,b` filter; the default is all). Keep Kotlin's cases as they are, so
   runs C and D stay comparable. For each language:
   - the same cases as Kotlin's: parse and highlight on the golden sample
     (`crates/fe-syntax/tests/golden/<language>.*`) and on that sample
     repeated to about 5,000 lines, the one-character reparse, and the wasm
     load.
   - the module registered as `<language>_wasm`, with no extensions, so it
     doesn't clash with the built-in. Its queries are the highlight and
     injection text the built-in uses, as `DEFS` in
     `crates/fe-syntax/src/language.rs` composes it, written to a temp dir.
   - before timing, one check that the wasm highlights equal the native ones
     on the golden sample. If they don't, report the language as not
     comparable rather than timing it.
   - markdown_inline parses inline text, so bench it on the sample's
     paragraphs, or say in PERF.md how it was measured.
   - a module that fails to load (for example the T1005 table validator
     refusing it, as it did C#'s) is a result: record the error and move on.
3. **Measure on a quiet Mac**, the same M3 Max as runs C and D. Before the
   run, ask the lead to hold the Mac's other lanes, and start only when
   `uptime`'s 1-minute load is below 4. Record the load at the start and end
   of each run, and do two runs. Ask the lead before any cargo build or
   clang build.
4. **PERF.md**: in "WebAssembly grammars", add a per-language table for the
   two runs: parse ratio on the golden sample and on 5k lines, load in ms, and
   the verdict under "Thresholds" (≤2× acceptable, 2–3× warning, >3×
   regression; load under 500 ms). Leave the Kotlin tables and the
   thresholds as they are.
5. **Decision file** (this card's `docs/decisions/<lane>-*.md`):
   - one row per grammar: the scanner's size (lines of `scanner.c`),
     whether it allocates, earlier fuzz findings
     (`docs/decisions/khgdxezget-fuzzing.md`; bash heredoc and markdown
     inline have open ones), both ratios, load, and a verdict of **migrate**
     or **stay native**, with the reason. A grammar that passes may still
     stay native for a stated reason, such as markdown being injected into
     fences and rustdoc, or a language needed on first launch with no
     network.
   - how a migrated day-one language would ship. Today, first-party
     WebAssembly grammars install on demand (T1322, T1647). Check whether
     the app can carry a `.wasm` grammar in its bundle. If it can't, say that
     migrating a day-one language needs that first, and that it is a user
     decision.
   - per migrated language, what T1322 hit and the migration card must plan
     for: outline, definitions and indent queries, tests that name the
     language, and the injection hosts.
   - a short "Grammar security: layers and gaps" section, which replaces a
     separate docs page. State each layer with the file that enforces it:
     - pinned grammar sources (`extensions/grammars/*/grammar-source.toml`,
       `Cargo.lock`)
     - our CI builds (`.github/workflows/grammars.yml`)
     - the signed index (`crates/fastedit/src/grammar_registry.rs`)
     - the wasmtime sandbox and the epoch deadlines
       (`crates/fe-syntax/src/runtime.rs`, `vendor/tree-sitter/README.md`)
     - load-time table validation (`crates/fe-syntax/tests/wasm_table_validation.rs`)
     - the memory cap (the memory-cap card filed with this one)
     - folder trust, for project-supplied grammars, if any. Check whether
       there are.

     Then the gaps, including the native grammars that stay native.
6. **Finish note**: list the migrate set and the stay-native set. Don't file
   the per-grammar migration cards: the lead files them from this decision
   (in T1322's shape, `docs/specs/yd7vp95v91-*`) after the user approves the
   set. The stay-native set is the input to the fuzzing card that depends on
   this one.

## Scope and requirements

- Don't change any grammar, query or threshold to make a number pass.
- No new CI check: the thresholds stay a documented policy, as T1338 set them.
- No file outside scope changes. The bench needs no new dependency; the
  grammar crates are already fe-syntax dependencies.
- Inbox cards T409 and T410 also edit PERF.md, in other sections.

## Implementation notes

- Reused: the Kotlin bench in `crates/fe-syntax/benches/wasm.rs`, which was
  extended in place. Kotlin's cases, ids, settings and 5k corpus are byte for
  byte the same (139,474 B, 4,966 lines). `scripts/build-grammar-wasm.sh`'s
  existing source resolution and build steps are shared by both roots.
- Kotlin's native built-in now needs `--features kotlin` (T427 moved it out of
  `default`). Without the feature the bench says so and leaves Kotlin out
  instead of panicking. It also reports Kotlin's highlight check but times it
  either way, since its wasm queries are its extension's.
- Copies the lead may want shared (each home is outside this card's scope):
  - `built_in_queries` (`crates/fe-syntax/benches/wasm.rs:67`) restates the
    16 languages' query composition from `DEFS`
    (`crates/fe-syntax/src/language.rs:479` onward), as the spec asks. A
    `pub` accessor for a built-in's composed queries in fe-syntax would
    remove the copy. Until then, the highlight check before timing catches
    a drift.
  - `highlights` (`benches/wasm.rs:425`) is the missing-language loop of
    `render` (`crates/fe-syntax/tests/wasm_parity.rs:261`), and `field`
    (`benches/wasm.rs:243`) is a looser `field` (`tests/wasm_parity.rs:61`).
    Benches and integration tests can't share a module without a file
    outside scope (`benches/support/`).
- The 5k corpus for languages other than Kotlin leaves a leading `#!` line
  out of later copies (lua's grammar allows it only first). XML repeats the
  root element's content inside the one root. None of the 16 corpora parses
  with errors.
