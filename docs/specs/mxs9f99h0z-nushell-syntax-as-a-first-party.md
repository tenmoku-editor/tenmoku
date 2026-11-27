---
task_uid: "01m8dfkvnvxyzc78mxs9f99h0z"
task: "T756"
rev: 1
type: "feature"
complexity: "s"
criteria:
  - id: "C1"
    text: "scripts/build-grammar-wasm.sh nu passes, including wasm_parity every_extension_highlights_its_sample for nu"
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
    text: "THIRD_PARTY.md has one appended WebAssembly row for nu crediting the grammar and every adapted query source with commit and licence"
    required: true
    manual: false
---

# Nushell syntax as a first-party WebAssembly grammar (extensions/grammars/nu) (Mac T2139)

# Nushell syntax as a first-party WebAssembly grammar

Filler card: one new directory, `extensions/grammars/nu/`, following
`extensions/grammars/README.md` and the existing grammars
(`extensions/grammars/fish/` and `extensions/grammars/powershell/` are the closest: shell languages (`packs = ["shells"]` territory)). Nushell is the structured-data shell (Rust, popular among the same developers Tenmoku is for); `.nu` scripts and its `config.nu`/`env.nu` open as plain text here today.

## This language

- Grammar: https://github.com/nushell/tree-sitter-nu,
  directory `src`. Pin: main (the shell team's own repo, very active) (HEAD 5a5f9c25f9c356f703a637bb3658248a29713684, 2026-11-25).
- Licence: MIT (`LICENSE`). External scanner: scanner.c (plain C). Parser size: 8.8 MB parser.c (est. ~2 MB wasm). ABI 15. Export `nu`: confirm both from `parser.c` (`LANGUAGE_VERSION` and the `tree_sitter_<x>` export) before pinning.
- File extensions: nu. File names: -. Shebangs: nu. Sample:
  `test/sample.nu`.
- Comments: line `#`, no block comment.
- `parity_needs`: none.
- Queries: `queries/nu/highlights.scm` in the grammar repo (MIT, 7 KB, 7 `#any-of?`, 3 `#eq?`, 1 `#match?`, no `; inherits:`; note the `nu/` subdirectory). Captures to rename or drop: `@spell`, `@none`, `@special`, `@cmd` and the `@_cmd`/`@_flag` helper captures that fe-syntax doesn't know (drop the capture, keep the pattern only if it still adds colour). Its `queries/nu/injections.scm` injects `comment`, `regex` and `nu` (itself, for `nu -c` strings): keep a `nu` self-injection only if the host accepts it, else make `injections.scm` comment-only.
- Notes: the sample should touch `def` with typed parameters and flags (`--flag(-f)`), a pipeline with `|`, a record `{a: 1}` and a list `[1 2 3]`, a closure `{|x| $x + 1}`, `let`/`mut`, an `if`/`else`, `for`, `match`, string interpolation `$"a ($x)"`, a raw string, a `use`/`module`, a `$env.PATH` access, a number with a unit (`10kb`), a range and a `#` comment.

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
- `extension.toml`: `id = "tenmoku.grammar_nu"`, `name = "Nushell syntax"`,
  `license` = the grammar's SPDX id, one `[[contributes.grammars]]` with
  `language = "nu"`, the extensions, file names, `line_comment` /
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
- `test/sample.nu`: 40-120 lines of idiomatic code touching def with typed parameters, pipelines, records and lists, closures, let and mut, control flow, interpolated strings, a use statement, a number with a unit and a comment; the tree must parse with no ERROR or
  MISSING node. Generate
  `test/sample.nu.golden` with
  `UPDATE_GOLDEN=1 scripts/build-grammar-wasm.sh nu`, then read it: every
  token class above is coloured and nothing is obviously wrong.
- Attribution: append one row for `nu` to `THIRD_PARTY.md`'s "Grammars
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
- Change nothing outside `extensions/grammars/nu/` and that appended
  row. If the module won't load (ABI, the table validator, a libc import the
  wasm stdlib lacks, over `MAX_GRAMMAR_BYTES` = 16 MiB, the load deadline),
  or a built-in already claims an extension, block the card with the exact
  error: fixing fe-syntax, the vendored tree-sitter or the build script is
  out of scope.

## Implementation notes

- Pinned `5a5f9c25f9c356f703a637bb3658248a29713684`, upstream's main HEAD.
  `parser.c` says ABI 15 and exports `tree_sitter_nu`; the module is 2.3 MB.
- No `packs`: the requirements allow no `packs` or other new keys.
- `injections.scm` keeps the `nu` self-injection, which the host accepts (the
  golden colours the string of `nu -c`), and the injections into `regex`, a
  built-in language. Only the `comment` injection is dropped.
- `highlights.scm` adds patterns for the names `mut`, `const` and `for` bind,
  which upstream leaves plain though it colours `let`'s.
- A return type needs its input type too (`]: nothing -> string`); a bare
  `-> string` doesn't parse.
