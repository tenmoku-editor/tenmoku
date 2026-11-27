---
task_uid: "01m8dfkt6pwpg77109hpqr7gdh"
task: "T754"
rev: 1
type: "feature"
complexity: "s"
criteria:
  - id: "C1"
    text: "scripts/build-grammar-wasm.sh cython passes, including wasm_parity every_extension_highlights_its_sample for cython"
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
    text: "THIRD_PARTY.md has one appended WebAssembly row for cython crediting the grammar and every adapted query source with commit and licence"
    required: true
    manual: false
---

# Cython syntax as a first-party WebAssembly grammar (extensions/grammars/cython) (Mac T2137)

# Cython syntax as a first-party WebAssembly grammar

Filler card: one new directory, `extensions/grammars/cython/`, following
`extensions/grammars/README.md` and the existing grammars
(`extensions/grammars/julia/` (a scientific-computing language with a scanner) is the closest). Cython is how a large part of the scientific Python stack (NumPy, pandas, scikit-learn, lxml) is written, and `.pyx`/`.pxd`/`.pxi` open as plain text here today.

## This language

- Grammar: https://github.com/b0o/tree-sitter-cython,
  directory `src`. Pin: master (HEAD 756a20c4d601b715f7b321eaad144f4cb14031bf, 2026-05-07).
- Licence: MIT (`LICENSE`). External scanner: scanner.c (plain C). Parser size: 11.5 MB parser.c (est. ~2.5 MB wasm). ABI 15. Export `cython`: confirm both from `parser.c` (`LANGUAGE_VERSION` and the `tree_sitter_<x>` export) before pinning.
- File extensions: pyx, pxd, pxi. File names: -. Shebangs: -. Sample:
  `test/sample.pyx`.
- Comments: line `#`, no block comment.
- `parity_needs`: none.
- Queries: the grammar's own `queries/highlights.scm` (MIT, 2.6 KB, 3 `#match?`, no `; inherits:`). Captures to rename or drop: `@embedded`, `@escape` (-> `@string.escape`), `@constructor` is fine; check every capture against fe-syntax's NAMES/ALIASES. Its `queries/injections.scm` injects `comment`, `printf` and `regex`, none of which exists here: make `injections.scm` comment-only.
- Notes: the sample should touch `cdef` variables and a `cdef class`, `cpdef`/`def` functions with typed arguments (`int x`, `double[:] a`), a `cimport`, an `ctypedef`, a `nogil` block, `extern` from a header, a `for i from 0 <= i < n` loop, `with nogil:`, a string, decorators, numbers and a `#` comment. If a Cython-only construct yields ERROR, drop that line and say so in the finish note.

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
- `extension.toml`: `id = "tenmoku.grammar_cython"`, `name = "Cython syntax"`,
  `license` = the grammar's SPDX id, one `[[contributes.grammars]]` with
  `language = "cython"`, the extensions, file names, `line_comment` /
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
- `test/sample.pyx`: 40-120 lines of idiomatic code touching cdef and cpdef declarations, typed arguments, cimport, ctypedef, nogil, a decorated function, strings and numbers, and a comment; the tree must parse with no ERROR or
  MISSING node. Generate
  `test/sample.pyx.golden` with
  `UPDATE_GOLDEN=1 scripts/build-grammar-wasm.sh cython`, then read it: every
  token class above is coloured and nothing is obviously wrong.
- Attribution: append one row for `cython` to `THIRD_PARTY.md`'s "Grammars
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
- Change nothing outside `extensions/grammars/cython/` and that appended
  row. If the module won't load (ABI, the table validator, a libc import the
  wasm stdlib lacks, over `MAX_GRAMMAR_BYTES` = 16 MiB, the load deadline),
  or a built-in already claims an extension, block the card with the exact
  error: fixing fe-syntax, the vendored tree-sitter or the build script is
  out of scope.

## Implementation notes

- Pinned `756a20c4d601b715f7b321eaad144f4cb14031bf` (`parser.c`: ABI 15, export
  `tree_sitter_cython`; the module is about 2.5 MB). Every construct in
  `test/sample.pyx` parses; no line had to be dropped for an `ERROR`.
- `earlier_patterns_win = false`: the grammar's query puts `(identifier)
  @variable` first and the specific patterns after it, so later ones must win.
  `highlights.scm`'s header lists what was changed from upstream's query.
