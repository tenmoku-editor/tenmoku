---
task_uid: "01m8cgd85yszcmft0yger830y2"
task: "T2146"
rev: 1
type: "feature"
complexity: "s"
criteria:
  - id: "C1"
    text: "scripts/build-grammar-wasm.sh lean passes, including wasm_parity every_extension_highlights_its_sample for lean"
    required: true
    manual: false
  - id: "C2"
    text: "cargo nextest run -p fe-syntax --test wasm_parity passes (every_extension_is_well_formed)"
    required: true
    manual: false
  - id: "C3"
    text: "The sample parses with no ERROR or MISSING node in the host tree, and its golden was generated and read: every token class in the card is coloured"
    required: true
    manual: false
  - id: "C4"
    text: "THIRD_PARTY.md has one appended WebAssembly row for lean crediting the grammar and every adapted query source with commit and licence"
    required: true
    manual: false
---

# Lean 4 syntax as a first-party WebAssembly grammar (extensions/grammars/lean)

# Lean 4 syntax as a first-party WebAssembly grammar

Filler card: one new directory, `extensions/grammars/lean/`, following
`extensions/grammars/README.md` and the existing grammars
(`extensions/grammars/haskell/` and `extensions/grammars/ocaml/` (functional languages with scanners) are the closest). Lean 4 is the proof assistant and programming language behind mathlib, with a fast-growing community; `.lean` files open as plain text here today. RISK: its parser.c is 49 MB, the largest in this batch (est. ~10 MB wasm against the 16 MiB `MAX_GRAMMAR_BYTES` cap): the first thing the worker does is build it and check the size and the load time; block the card with the exact numbers if the module is over the cap or fails the load deadline.

## This language

- Grammar: https://github.com/Julian/tree-sitter-lean,
  directory `src`. Pin: main (very active; re-check HEAD before pinning) (HEAD f35fa9b1ad3edd22c2b07b61f569a6d9c8e07d6e, 2026-11-24).
- Licence: MIT (`LICENSE`). External scanner: scanner.c (plain C, 16 KB). Parser size: 49 MB parser.c (est. ~10 MB wasm: build first). ABI 15. Export `lean`: confirm both from `parser.c` (`LANGUAGE_VERSION` and the `tree_sitter_<x>` export) before pinning.
- File extensions: lean. File names: -. Shebangs: -. Sample:
  `test/sample.lean`.
- Comments: line `--`, block `/-` `-/`.
- `parity_needs`: none.
- Queries: the grammar's own `queries/highlights.scm` (MIT, 5.2 KB, no predicates, no `; inherits:`). Captures to check: `@string.special.symbol`, `@type.parameter`, `@keyword.modifier` (alias or drop each unknown one). Its `queries/injections.scm` injects `markdown` into doc comments: keep it if `markdown` is a built-in here and the host tree stays clean, else make `injections.scm` comment-only.
- Notes: the sample should touch `import`, `namespace`/`end`, `def` with typed binders and pattern matching, a `structure` and an `inductive` type, `theorem` with a `by` tactic block (`simp`, `intro`, `exact`), `instance`, `open`, a `fun x => x`, `match ... with`, `do` notation with `let` and `←`, a `#eval`, universe and implicit binders (`{α : Type u}`), a string, a number, `--` comments, a `/- -/` block and a `/-- -/` doc comment. Keep the sample under 100 lines: a 49 MB grammar parses slowly.

## Requirements

- `grammar-source.toml` in the git form (`git`, `rev` = a full 40-hex commit,
  `src`, `grammar`) unless the notes say otherwise. Pin the newest commit that
  builds and passes; take `grammar` from the `tree_sitter_<x>` export in
  `parser.c`, which also fixes `wasm = "tree-sitter-<x>.wasm"`. Add
  `parity_needs` exactly as "This language" says.
- `LICENSE` (and any `LICENSE-*`/`NOTICE`) copied verbatim from the grammar at
  that commit; a `LICENSE.md` upstream is copied to `LICENSE` (the well-formed
  check admits only `LICENSE`, `LICENSE-*` and `NOTICE`). Queries from
  nvim-treesitter are Apache-2.0: add `LICENSE-APACHE` and say so as `kotlin/`
  does. Don't copy Helix or Zed queries (MPL-2.0 / mixed).
- `extension.toml`: `id = "tenmoku.grammar_lean"`, `name = "Lean 4 syntax"`,
  `license` = the grammar's SPDX id, one `[[contributes.grammars]]` with
  `language = "lean"`, the extensions, file names, `line_comment` /
  `block_comment` and `shebangs` above, and `earlier_patterns_win` matching
  the query's convention (tree-sitter style: true; nvim style: false). No
  `packs` or other new keys.
- `highlights.scm`: self-contained (inline anything a `; inherits:` line
  pulls in, adapted to this grammar's node names); captures in fe-syntax's
  vocabulary (`crates/fe-syntax/src/highlight.rs` NAMES/ALIASES). Drop
  patterns using predicates or directives fe-syntax doesn't support
  (`#offset!`, `#gsub!`, `#has-ancestor?`, `#set! priority`, `#lua-match?`
  unless rewritten as `#match?`) rather than leaving them inert. Each query
  file's header says where it came from, at which commit, under what
  licence, and what changed (`extensions/grammars/astro/` is the model).
- `injections.scm`: only injections into languages that exist (built-ins,
  or first-party directories listed in `parity_needs`); a comment-only file
  if none. No locals, folds, indents or outline queries.
- `test/sample.lean`: 40-120 lines of idiomatic code touching import and namespace, def with pattern matching, structure and inductive, a theorem with a tactic block, instance, lambdas, do notation, eval, implicit binders, strings, numbers and all comment forms; the tree must parse with no ERROR or
  MISSING node. Generate
  `test/sample.lean.golden` with
  `UPDATE_GOLDEN=1 scripts/build-grammar-wasm.sh lean`, then read it: every
  token class above is coloured and nothing is obviously wrong.
- Attribution: append one row for `lean` to `THIRD_PARTY.md`'s "Grammars
  shipped as WebAssembly" table, giving the source as `grammar-source.toml`
  does and the licence, and naming in that licence cell every adapted query
  source with its commit and licence (as the astro, svelte and hlsl rows do).
  That row is where adapted queries are credited: add nothing to "Code copied
  or adapted into this repository" (that table and its checks,
  `crates/fastedit/tests/third_party.rs` and `tests/notice.rs`, cover
  `crates/` and `kiln/` only and want NOTICE entries; settled on T1463
  Astro), and don't touch NOTICE. Append only; don't reorder or edit other
  rows. `THIRD_PARTY.md` is in `regatta.toml`'s `shared_append_only`, which
  is why it isn't in this card's scope.
- Change nothing outside `extensions/grammars/lean/` and that appended
  row. If the module won't load (ABI, the table validator, a libc import the
  wasm stdlib lacks, over `MAX_GRAMMAR_BYTES` = 16 MiB, the load deadline),
  or a built-in already claims an extension, block the card with the exact
  error: fixing fe-syntax, the vendored tree-sitter or the build script is
  out of scope.

## Implementation notes

- Pinned at HEAD `f35fa9b1ad3edd22c2b07b61f569a6d9c8e07d6e` (ABI 15, export
  `tree_sitter_lean`).
- The module is 8,237,745 bytes, under `MAX_GRAMMAR_BYTES` (20 MiB in
  `crates/fe-syntax/src/runtime.rs`, so under the spec's 16 MiB too). A
  temporary, uncommitted test registered it under the default deadlines in a
  debug build in 1.06 s, against the 10 s load deadline.
- Upstream ships no `test/sample.lean`; the sample is written for this card.
- The grammar takes `/-!` for a module doc only at the top of a file, so both
  queries also match a `/-!` block comment with `#match?`.
- Grammar limits seen while probing, each leaving an `ERROR` node, none of them
  in the sample: `simp at *`, and an `if` with no `else` in a `do` block.
- `for`, `in` and `return` have no rules in a `do` block and parse as
  identifiers; the highlight query colours them as keywords by name.
