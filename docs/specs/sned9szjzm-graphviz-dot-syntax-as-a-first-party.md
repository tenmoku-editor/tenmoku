---
task_uid: "01m8dfktzjb0j90ysned9szjzm"
task: "T755"
rev: 1
type: "feature"
complexity: "s"
criteria:
  - id: "C1"
    text: "scripts/build-grammar-wasm.sh dot passes, including wasm_parity every_extension_highlights_its_sample for dot"
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
    text: "THIRD_PARTY.md has one appended WebAssembly row for dot crediting the grammar and every adapted query source with commit and licence"
    required: true
    manual: false
---

# Graphviz DOT syntax as a first-party WebAssembly grammar (extensions/grammars/dot) (Mac T2138)

# Graphviz DOT syntax as a first-party WebAssembly grammar

Filler card: one new directory, `extensions/grammars/dot/`, following
`extensions/grammars/README.md` and the existing grammars
(`extensions/grammars/wat/` and `extensions/grammars/kdl/` (small grammars) are the closest). Graphviz DOT is the diagram format behind `dot`, `doxygen` call graphs and countless generated `.gv` files; both extensions open as plain text here today.

## This language

- Grammar: https://github.com/rydesun/tree-sitter-dot,
  directory `src`. Pin: main (HEAD 80327abbba6f47530edeb0df9f11bd5d5c93c14d, 2025-10-21).
- Licence: MIT (`LICENSE`). External scanner: none. Parser size: 0.15 MB parser.c (expect a wasm well under 1 MB). ABI 13 (older than the others but accepted by the loader; check). Export `dot`: confirm both from `parser.c` (`LANGUAGE_VERSION` and the `tree_sitter_<x>` export) before pinning.
- File extensions: dot, gv. File names: -. Shebangs: -. Sample:
  `test/sample.dot`.
- Comments: line `//` (and `#` for preprocessor-style lines), block `/*` `*/`.
- `parity_needs`: none.
- Queries: the grammar's own `queries/highlights.scm` (MIT, 0.5 KB, no predicates). Its `queries/injections.scm` injects `html` into the HTML-like label strings (`<...>`): keep that one injection, `html` being a built-in, and add no `parity_needs`; if the injection leaves an ERROR in the host tree or a bad colour, make `injections.scm` comment-only and say so.
- Notes: the sample should touch `digraph` and `graph` (and `strict`), node and edge statements with `->` and `--`, attribute lists (`[label="x", shape=box]`), a `subgraph cluster_0 { }`, a `node [..]` default, an HTML-like label, ports (`a:p1`), numbers, quoted IDs and both comment forms.

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
- `extension.toml`: `id = "tenmoku.grammar_dot"`, `name = "Graphviz DOT syntax"`,
  `license` = the grammar's SPDX id, one `[[contributes.grammars]]` with
  `language = "dot"`, the extensions, file names, `line_comment` /
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
- `test/sample.dot`: 40-120 lines of idiomatic code touching graph and digraph, nodes and edges, attribute lists, a subgraph, an HTML-like label, ports, numbers, quoted IDs and both comment forms; the tree must parse with no ERROR or
  MISSING node. Generate
  `test/sample.dot.golden` with
  `UPDATE_GOLDEN=1 scripts/build-grammar-wasm.sh dot`, then read it: every
  token class above is coloured and nothing is obviously wrong.
- Attribution: append one row for `dot` to `THIRD_PARTY.md`'s "Grammars
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
- Change nothing outside `extensions/grammars/dot/` and that appended
  row. If the module won't load (ABI, the table validator, a libc import the
  wasm stdlib lacks, over `MAX_GRAMMAR_BYTES` = 16 MiB, the load deadline),
  or a built-in already claims an extension, block the card with the exact
  error: fixing fe-syntax, the vendored tree-sitter or the build script is
  out of scope.

## Implementation notes

- Pinned at main's `80327abbba6f47530edeb0df9f11bd5d5c93c14d`; `parser.c` is
  ABI 13 and exports `tree_sitter_dot`.
- The grammar reads one graph a file, so there are two samples:
  `test/sample.dot` (`strict digraph`, `->`, the `graph [..]` default) and
  `test/sample.gv` (`graph`, `--`).
- Known limit, the grammar's: comments are extras inside HTML-like labels too,
  so label text that starts with `//` (`<<B>//x</B>>`) doesn't parse.
- The HTML injection is kept; both samples parse cleanly with it.
