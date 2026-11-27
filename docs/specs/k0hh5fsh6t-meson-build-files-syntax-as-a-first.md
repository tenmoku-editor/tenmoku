---
task_uid: "01m8cgd6k8rht0vak0hh5fsh6t"
task: "T2145"
rev: 1
type: "feature"
complexity: "s"
criteria:
  - id: "C1"
    text: "scripts/build-grammar-wasm.sh meson passes, including wasm_parity every_extension_highlights_its_sample for meson"
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
    text: "THIRD_PARTY.md has one appended WebAssembly row for meson crediting the grammar and every adapted query source with commit and licence"
    required: true
    manual: false
---

# Meson build files syntax as a first-party WebAssembly grammar (extensions/grammars/meson)

# Meson build files syntax as a first-party WebAssembly grammar

Filler card: one new directory, `extensions/grammars/meson/`, following
`extensions/grammars/README.md` and the existing grammars
(`extensions/grammars/cmake/` (a build-system language claiming file names) is the closest). Meson is the build system of GNOME, Mesa, systemd and many C/C++ projects; `meson.build` files open as plain text here today.

## This language

- Grammar: https://github.com/tree-sitter-grammars/tree-sitter-meson,
  directory `src`. Pin: master (HEAD aa8d472034956f94f51f2ef2cbfec4cc07efbfde, 2026-06-29).
- Licence: MIT (`LICENSE`). External scanner: none. Parser size: 0.79 MB parser.c (expect a wasm well under 1 MB). ABI 15. Export `meson`: confirm both from `parser.c` (`LANGUAGE_VERSION` and the `tree_sitter_<x>` export) before pinning.
- File extensions: -. File names: meson.build, meson.options, meson_options.txt. Shebangs: -. Sample:
  `test/sample.build`.
- Comments: line `#`, no block comment.
- `parity_needs`: none.
- Queries: the grammar's own `queries/highlights.scm` (MIT, 0.5 KB, no predicates, no `; inherits:`; legacy `@conditional` and `@repeat` which ALIASES maps). It has no injections file (only `folds.scm`, left out): make `injections.scm` comment-only. The grammar claims no extension, only file names: `extensions = []` is allowed with `file_names` (as `cmake/`, `gomod/` do), so use that.
- Notes: the sample should touch `project('x', 'c', version: '1.0', default_options: [...])`, `dependency('glib-2.0', required: false)`, `executable('a', sources, dependencies: deps, install: true)`, `if`/`elif`/`else`/`endif`, `foreach`/`endforeach`, `files()`, a method call (`cc.has_header('x.h')`), a dict, a multiline `'''` string, an f-string `f'@x@'`, `get_option('b')`, numbers, `true`/`false` and `#` comments.

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
- `extension.toml`: `id = "tenmoku.grammar_meson"`, `name = "Meson syntax"`,
  `license` = the grammar's SPDX id, one `[[contributes.grammars]]` with
  `language = "meson"`, the extensions, file names, `line_comment` /
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
- `test/sample.build`: 40-120 lines of idiomatic code touching a project call, dependencies and executables, if and foreach, method calls, a dict, a multiline string, an f-string, get_option, numbers and comments; the tree must parse with no ERROR or
  MISSING node. Generate
  `test/sample.build.golden` with
  `UPDATE_GOLDEN=1 scripts/build-grammar-wasm.sh meson`, then read it: every
  token class above is coloured and nothing is obviously wrong.
- Attribution: append one row for `meson` to `THIRD_PARTY.md`'s "Grammars
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
- Change nothing outside `extensions/grammars/meson/` and that appended
  row. If the module won't load (ABI, the table validator, a libc import the
  wasm stdlib lacks, over `MAX_GRAMMAR_BYTES` = 16 MiB, the load deadline),
  or a built-in already claims an extension, block the card with the exact
  error: fixing fe-syntax, the vendored tree-sitter or the build script is
  out of scope.

## Implementation notes

- `extensions/grammars/meson/` pins upstream's HEAD, the newest commit
  (grammar 1.3.0, ABI 15, no external scanner; the wasm is 118 KB).
- `highlights.scm` is upstream's with the additions its header lists. It uses
  Neovim's `@conditional` and `@repeat`, so later patterns win.
- Limits of the pinned parser, found by probing, which leave an `ERROR` node in
  valid Meson and so some wrong colours near it: a bare `#` line; the `<=`,
  binary `-`, `*` and `%` operators (the grammar has no such tokens); a `'`
  inside a `'''` string; a `^` in a string; and a lone `@` in a string
  (`'user@host'`). Fixing them is upstream's grammar, out of this card's scope.
  The sample avoids them.
