---
task_uid: "01m8cfa29d2nehnsqhpaqshdsx"
task: "T749"
rev: 1
type: "feature"
complexity: "m"
criteria:
  - id: "C1"
    text: "scripts/build-grammar-wasm.sh rst passes, including wasm_parity every_extension_highlights_its_sample for rst"
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
    text: "THIRD_PARTY.md has one appended WebAssembly row for rst crediting the grammar and every adapted query source with commit and licence"
    required: true
    manual: false
---

# reStructuredText syntax as a first-party WebAssembly grammar (extensions/grammars/rst) (Mac T2129)

# reStructuredText syntax as a first-party WebAssembly grammar

Filler card: one new directory, `extensions/grammars/rst/`, following
`extensions/grammars/README.md` and the existing grammars
(`extensions/grammars/latex/` is the closest: a markup language injecting
other languages). reStructuredText is Python's documentation format (Sphinx,
docutils, PyPI READMEs) and is plain text here; the built-in markdown covers
only Markdown.

## This language

- Grammar: https://github.com/stsewd/tree-sitter-rst, directory `src`.
  Pin: master (HEAD a60f1070b824cb8bb8409b4b6d7da0d07997c30e, 2026-06-09; also
  the revision nvim-treesitter pins; the repo has a "Fix build with MSVC"
  commit on top of 2025 work).
- Licence: MIT (`LICENSE`). External scanner: `src/scanner.c`, which
  `#include`s `src/tree_sitter_rst/scanner.c` and `tokens.h` (plain C; the
  fetch takes the whole commit, as OCaml's scanner needs). Parser size:
  0.32 MB (expect a wasm well under 1 MB). ABI 14. Export `rst`.
- File extensions: rst, rest. File names: -. Shebangs: -. Sample:
  `test/sample.rst`.
- Comments: explicit-markup comments (`.. comment text`) are block-ish; set
  `line_comment = ".."` only if the toggle-comment feature takes it, else
  omit (say which in the finish note).
- `parity_needs = ["latex"]` (the `math` directive and `:math:` role inject
  it; `python`, `csv` and `bash`/`json`/`yaml` named by a `code-block`
  argument are built in).
- Queries: the grammar repo has NO highlights. Use nvim-treesitter's
  `runtime/queries/rst/highlights.scm` and `injections.scm` (Apache-2.0,
  3 KB and 2 KB, nvim-treesitter main
  52773f0d13a0c11d0c3ef33c14ab6ecc1c17148b, 2026-11-21): add
  `LICENSE-APACHE` and credit it. Highlights use `#eq?`, `#any-of?`,
  `#not-any-of?` (all supported); rename `@spell`, drop `@none` (the
  `doctest_block` reset) and map `@keyword.import`/`@markup.raw.block`
  through the vocabulary. Injections use `#set! injection.language`,
  `@injection.language` (a capture whose text names the language: supported by
  fe-syntax's injection.rs) and the `comment` language (drop that pattern).
  The nested `rst` -> `rst` injections for admonition bodies are fine
  (depth <= `MAX_DEPTH` = 3) but check they do not recurse into errors.
- Notes: the sample should touch section titles with over/underlines, a
  bullet and an enumerated list, a field list, `*emph*`/`**strong**`/
  ``literal``, a hyperlink target and a reference `` `text <url>`_ ``,
  `.. note::` and `.. code-block:: python` directives with an indented body,
  a `.. math::` directive, a `:ref:` role, a footnote `[1]_`, a literal block
  after `::`, a doctest block `>>>`, a substitution `|name|` and a `..`
  comment.

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
- `extension.toml`: `id = "tenmoku.grammar_rst"`, `name = "reStructuredText syntax"`,
  `license` = the grammar's SPDX id, one `[[contributes.grammars]]` with
  `language = "rst"`, the extensions, file names, `line_comment` /
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
- `test/sample.rst`: 40-120 lines of idiomatic code touching titles, lists, inline markup, links, directives with injected bodies, roles, a literal block, a doctest block and a comment; the tree must parse with no ERROR or
  MISSING node (an injected layer may have errors). Generate
  `test/sample.rst.golden` with
  `UPDATE_GOLDEN=1 scripts/build-grammar-wasm.sh rst`, then read it: every
  token class above is coloured and nothing is obviously wrong.
- Attribution: append one row for `rst` to `THIRD_PARTY.md`'s "Grammars
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
- Change nothing outside `extensions/grammars/rst/` and that appended
  row. If the module won't load (ABI, the table validator, a libc import the
  wasm stdlib lacks, over `MAX_GRAMMAR_BYTES` = 16 MiB, the load deadline),
  or a built-in already claims an extension, block the card with the exact
  error: fixing fe-syntax, the vendored tree-sitter or the build script is
  out of scope.

## Implementation notes

- `line_comment` is omitted. fe-editor's toggle (`EditorModel::toggle_comment`)
  takes any token, but it treats a row starting with the token as commented,
  and in reStructuredText every explicit-markup block starts with `..`:
  toggling over a directive, a target or a footnote would strip its markup
  as if it were a comment, and commenting a row starting `[1]`, `_name:` or
  `|name|` would make a footnote, target or substitution of it.
- The rst -> rst body injections are rewritten (the header of
  `injections.scm` says how). Upstream injected the `content` node, but the
  grammar's scanner counts indentation from the layer's own text, so a
  two-line admonition body read as a definition list; and upstream's
  admonition pattern injected the `body` less its children, which is only
  whitespace. The layer is now the whole body with its children, when it
  doesn't start with arguments; it reads as one block quote, so
  `highlights.scm` colours a block quote as `@markup.quote` only after a
  sibling. A body after arguments (`.. topic:: Title`) stays plain.
- No `packs`, as the spec says.
