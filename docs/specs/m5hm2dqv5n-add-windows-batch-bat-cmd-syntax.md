---
task_uid: "01m79daa2hs9tf3am5hm2dqv5n"
task: "T340"
rev: 3
type: "feature"
complexity: "s"
criteria:
  - id: "C1"
    text: "extensions/grammars/batch/ holds the file set every_extension_is_well_formed requires, grammar-source.toml pinned to wharflab/tree-sitter-batch at a 40-hex commit, the grammar's LICENSE verbatim; nothing else changes except appended THIRD_PARTY.md rows (the grammars table; the WebAssembly row for extension queries), the wharflab/tree-sitter-batch entry in NOTICE, and one NOT_RELEASED entry in crates/fastedit/tests/grammar_pack_recorded.rs"
    required: true
    manual: false
  - id: "C2"
    text: "scripts/build-grammar-wasm.sh batch passes on the lane's host: the wasm builds and loads within MAX_GRAMMAR_BYTES, and wasm_parity's ignored every_extension_highlights_its_sample passes for batch, with test/sample.bat parsing with no ERROR or MISSING node"
    required: true
    manual: false
  - id: "C3"
    text: "wasm_parity's every_extension_is_well_formed and fastedit's grammar_install_tests::every_first_party_manifest_parses pass with batch present, its extension.toml giving extensions bat and cmd, line_comment REM, no shebangs, and no clash with a built-in"
    required: true
    manual: false
  - id: "C4"
    text: "test/sample.bat.golden colours keywords, labels, variables, strings, comments and operators"
    required: true
    manual: false
  - id: "C5"
    text: "cargo nextest run -p fe-syntax -p fastedit passes, including grammar_pack_recorded"
    required: true
    manual: false
---

# Add Windows batch (.bat/.cmd) syntax highlighting as a first-party WebAssembly grammar (extensions/grammars/batch) (Mac T1772)

# Windows batch (.bat/.cmd) syntax as a first-party WebAssembly grammar

Filler card: one new directory, `extensions/grammars/batch/`, following
`extensions/grammars/README.md` and the existing grammars
(`extensions/grammars/powershell/` is the model; T1442's spec is the template).
Linux-OK: the wasm build and parity tests run on any host.

## This language

- Grammar: https://github.com/wharflab/tree-sitter-batch, directory `src`.
  Pin: main HEAD at drafting, `2aa22f00d7f5bbe58556b8eef0b1fc1629508603`
  (2026-07-22; repo pushed 2026-11-11). Re-check the newest commit that builds.
- Licence: MIT (`LICENSE` at the repo root). ABI 14, no external scanner
  (`src/` is grammar.json, node-types.json, parser.c: about 600 KB). The
  `tree_sitter_batch` export gives `grammar = "batch"`, `wasm = "tree-sitter-batch.wasm"`.
- Alternatives rejected: davidevofficial/tree-sitter-batch (last push 2025-02),
  imDMG/tree-sitter-bat (2026-03, 43 KB repo, thin), SklorGalore (no licence).
- File extensions: bat, cmd. File names: -. Shebangs: none.
- Comments: line `REM` (the grammar also parses `::` labels-as-comments; the
  manifest takes one token, so `line_comment = "REM"` and the notes say `::`
  is also accepted on read). No block comments.
- Queries: upstream `queries/highlights.scm` (MIT, same repo; adapt capture
  names to fe-syntax's vocabulary; it uses `@label`, `@keyword`, `@constant`
  and similar: map any not in NAMES/ALIASES, drop unsupported predicates).
- Notes: `earlier_patterns_win` per the query's convention (nvim style: false).
  `injections.scm`: comment-only.

## Requirements

Everything in T1442's Requirements section, for `batch`:
`extension.toml` id `tenmoku.grammar_batch`, name "Batch syntax"; `LICENSE`
verbatim; `test/sample.bat` (40-120 lines: `@echo off`, `REM` and `::`
comments, labels and `goto`, `set`/`set /a`, `%VAR%` and `%~dp0`, `if`/`else`,
`for /f`, `call`, redirects and pipes, `setlocal`/`endlocal`, `exit /b`) parsing
with no ERROR or MISSING node, plus its `.golden`; rows appended to
`THIRD_PARTY.md` (WebAssembly grammars table and adapted-queries table).

Release plumbing (so `grammar_pack_recorded` stays green): the wasm is not in a
release yet, so append `("batch", "added after grammars-2; its wasm is recorded
with the next grammar release")` to `NOT_RELEASED` in
`crates/fastedit/tests/grammar_pack_recorded.rs` (the only edit outside the
directory besides THIRD_PARTY.md). Do not touch `pack.toml`.

File icons: nothing to add. `bat` and `cmd` already map to `FileTerminal` in
`file_icons.rs` and to the `SCRIPT` tint in `file_icons_tinted.rs`; the worker
only confirms `gradlew.bat` still resolves to its Package entry (name beats
extension) and records that in the decision file. If the worker finds the
coloured set (`file_icons_coloured.rs`) lacks a `bat` extension row, that is a
separate card, not this one.

If the wasm won't load (ABI, validator, size over `MAX_GRAMMAR_BYTES`) or a
built-in claims `bat`/`cmd`, block with the exact error; fixing fe-syntax or
the build script is out of scope.

## Implementation notes

- Pinned at main HEAD `2aa22f00…` (still the newest commit); wasm builds, loads, and parity passes with a clean sample.
- Upstream's statement keywords (`if`, `for`, `goto`, …) are anonymous pattern tokens that queries can't name, so `if_stmt` etc. are captured whole as `@keyword` and later patterns recolour operands. Residue: blanks, `else`, `do`, `in`, and goto/call targets read as keyword. The dynamic-variable pattern moved after `@variable` (later wins).
- `::` comments parse as comments; `line_comment = "REM"` is the manifest's one token.
- Icons: `bat`/`cmd` map to `FileTerminal`/`SCRIPT`; `gradlew.bat` still resolves to `Package` (name beats extension); the coloured set has a `bat` row.
- `crates/fastedit/tests/grammar_pack_recorded.rs` and its `NOT_RELEASED` list were retired on main by T1486 (997666b7), before this lane's base; there is nothing to append, so that part of C1 and the `grammar_pack_recorded` part of C5 are void. `pack.toml` untouched.
