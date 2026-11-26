---
task_uid: "01m8cey1j1r5nvbzgv2cjwr8sr"
task: "T2130"
rev: 1
type: "feature"
complexity: "s"
criteria:
  - id: "C1"
    text: "scripts/build-grammar-wasm.sh commonlisp passes, including wasm_parity every_extension_highlights_its_sample for commonlisp"
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
    text: "THIRD_PARTY.md has one appended WebAssembly row for commonlisp crediting the grammar and every adapted query source with commit and licence"
    required: true
    manual: false
---

# Common Lisp syntax as a first-party WebAssembly grammar (extensions/grammars/commonlisp)

# Common Lisp syntax as a first-party WebAssembly grammar

Filler card: one new directory, `extensions/grammars/commonlisp/`,
following `extensions/grammars/README.md` and the existing grammars
(`extensions/grammars/scheme/` and `clojure/` are the closest: S-expression
languages, no scanner). "Lisp" is on the Stack Overflow 2025 survey's list;
this is Common Lisp (SBCL, Quicklisp). Emacs Lisp is a separate Tier 2 card.

## This language

- Grammar: https://github.com/tree-sitter-grammars/tree-sitter-commonlisp,
  directory `src`. Pin: master (HEAD 32323509b3d9fe96607d151c2da2c9009eb13a2f,
  2025-03-16; also the revision nvim-treesitter pins).
- Licence: MIT (`LICENSE.md` upstream: copy it to `LICENSE`). External
  scanner: none. Parser size: 5.9 MB (expect a wasm around 1.2 MB). ABI 14.
  Export `commonlisp`.
- File extensions: lisp, lsp, cl, asd, asdf, ros. File names: -. Shebangs:
  `sbcl`, `ros`. Sample: `test/sample.lisp`.
- Comments: line `;`, block `#|` `|#`.
- `parity_needs`: none.
- Queries: the grammar repo has NO highlights. Use nvim-treesitter's
  `runtime/queries/commonlisp/highlights.scm` (Apache-2.0, 15 KB,
  nvim-treesitter main 52773f0d13a0c11d0c3ef33c14ab6ecc1c17148b, 2026-11-21):
  add `LICENSE-APACHE` and credit it. Three `#lua-match?` (earmuffs
  `^[*].+[*]$`, constants `^[+].+[+]$`, lambda-list keywords `^[&]`) become
  `#match?`; two `#cl-standard-function?`/`#cl-standard-macro?` lines are
  already commented out in the source: drop them. Rename `@spell`, map
  `@string.special.symbol`/`@keyword.*` through the vocabulary. The injection
  file is a comment injection only: make `injections.scm` comment-only.
- Notes: the sample should touch `defpackage`/`in-package`, `defun` with
  `&optional`/`&key`, `defmacro` with a backquote template and `,`/`,@`,
  `defvar` with an earmuffed name, `defconstant` with `+name+`, `defclass`
  with slots, `defgeneric`/`defmethod`, `let`/`let*`, `loop` with `for ...
  collect`, `format` with a `~a` directive string, a keyword `:key`, a
  character `#\a`, a quoted list, a vector `#(1 2)`, `#'function`, numbers
  (rational `1/2`, float `1.5d0`), and both comment forms.

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
- `extension.toml`: `id = "tenmoku.grammar_commonlisp"`, `name = "Common Lisp syntax"`,
  `license` = the grammar's SPDX id, one `[[contributes.grammars]]` with
  `language = "commonlisp"`, the extensions, file names, `line_comment` /
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
- `test/sample.lisp`: 40-120 lines of idiomatic code touching packages, functions and macros with lambda-list keywords, classes and generics, loop, strings, characters, keywords, quoting forms and both comment forms; the tree must parse with no ERROR or
  MISSING node. Generate
  `test/sample.lisp.golden` with
  `UPDATE_GOLDEN=1 scripts/build-grammar-wasm.sh commonlisp`, then read it: every
  token class above is coloured and nothing is obviously wrong.
- Attribution: append one row for `commonlisp` to `THIRD_PARTY.md`'s "Grammars
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
- Change nothing outside `extensions/grammars/commonlisp/` and that appended
  row. If the module won't load (ABI, the table validator, a libc import the
  wasm stdlib lacks, over `MAX_GRAMMAR_BYTES` = 16 MiB, the load deadline),
  or a built-in already claims an extension, block the card with the exact
  error: fixing fe-syntax, the vendored tree-sitter or the build script is
  out of scope.
