---
task_uid: "01m8ceybcs9hv46aace8jmar89"
task: "T2135"
rev: 3
type: "feature"
complexity: "s"
criteria:
  - id: "C1"
    text: "scripts/build-grammar-wasm.sh matlab passes, including wasm_parity every_extension_highlights_its_sample for matlab"
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
    text: "THIRD_PARTY.md has one appended WebAssembly row for matlab crediting the grammar and every adapted query source with commit and licence"
    required: true
    manual: false
---

# MATLAB syntax as a first-party WebAssembly grammar (extensions/grammars/matlab)

# MATLAB (blocked on `.m` ownership; see Notes) syntax as a first-party WebAssembly grammar

Filler card: one new directory, `extensions/grammars/matlab/`, following
`extensions/grammars/README.md` and the existing grammars
(`extensions/grammars/julia/` is the closest: a numeric-computing language
with a scanner). MATLAB is on the Stack Overflow 2025 survey's list.

DECIDED: `.m` is shared with Objective-C. Task T2136 picks objc or matlab for a `.m` file by its content and lets both directories claim `m` (wasm_parity exempts exactly that pair). So this card sets `extensions = ["m"]` in matlab's manifest, and its sample (`test/sample.m`) is judged as MATLAB by the extension alone. Don't touch `extensions/grammars/objc/`. Run the sniff's own test crates/fe-syntax/tests/m_file_sniff.rs too, so a MATLAB-looking sample is not mistaken for Objective-C.

## This language

- Grammar: https://github.com/acristoffers/tree-sitter-matlab, directory `src`.
  Pin: main (HEAD 1bdb8cf5e73620892a199f659ceb9c06d02e4b2f, 2026-11-20;
  also the revision nvim-treesitter pins).
- Licence: MIT (`LICENSE`). External scanner: scanner.c (38 KB, plain C).
  Parser size: 3.7 MB (expect a wasm around 1 MB). ABI 15. Export `matlab`.
- File extensions: m (shared with `objc`, sniffed by content). File names: -.
  Shebangs: -. Sample: `test/sample.m`.
- Comments: line `%`, block `%{` `%}` (each on its own line).
- `parity_needs`: none.
- Queries: `queries/neovim/highlights.scm` in the grammar repo (MIT, 2.4 KB;
  `queries/helix/` and `queries/emacs/` are not used). It uses legacy
  names that fe-syntax's ALIASES already map (`@include`, `@conditional`,
  `@repeat`, `@exception`), one `#lua-match?` (rewrite as `#match?`) and
  `#eq?` (supported). `queries/neovim/injections.scm` is only
  `(comment) @comment`, not an injection: make `injections.scm`
  comment-only.
- Notes: the sample should touch a `function` with `end`, a `classdef` with
  `properties`/`methods`, `if/elseif/else`, `for`, `while`, `switch/case`,
  `try/catch`, a matrix literal with `;` rows and a transpose (`'`),
  a string in `"` and `'` forms, a `%` comment and a `%{ %}` block, a command
  syntax call (`hold on`), an anonymous function (`@(x) x.^2`) and `end` inside
  an index. The transpose-vs-quote scanner is the likely source of an ERROR:
  keep such lines minimal.

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
- `extension.toml`: `id = "tenmoku.grammar_matlab"`, `name = "MATLAB syntax"`,
  `license` = the grammar's SPDX id, one `[[contributes.grammars]]` with
  `language = "matlab"`, the extensions, file names, `line_comment` /
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
- `test/sample.m`: 40-120 lines of idiomatic code touching functions, classdef, control flow, matrices and the transpose, both string forms, command syntax, anonymous functions and both comment forms; the tree must parse with no ERROR or
  MISSING node. Generate
  `test/sample.m.golden` with
  `UPDATE_GOLDEN=1 scripts/build-grammar-wasm.sh matlab`, then read it: every
  token class above is coloured and nothing is obviously wrong.
- Attribution: append one row for `matlab` to `THIRD_PARTY.md`'s "Grammars
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
- Change nothing outside `extensions/grammars/matlab/` and that appended
  row. If the module won't load (ABI, the table validator, a libc import the
  wasm stdlib lacks, over `MAX_GRAMMAR_BYTES` = 16 MiB, the load deadline),
  or a built-in already claims an extension, block the card with the exact
  error: fixing fe-syntax, the vendored tree-sitter or the build script is
  out of scope.

## Implementation notes

- One new directory, `extensions/grammars/matlab/`, and one appended
  `THIRD_PARTY.md` row. Reused as they are: `scripts/build-grammar-wasm.sh`,
  the `wasm_parity` and `m_file_sniff` tests, and fe-syntax's highlight names
  and aliases. No code, helper or test fixture was added.
- Pinned `1bdb8cf5e73620892a199f659ceb9c06d02e4b2f` (1.3.4, ABI 15). The
  module is about 0.5 MB.
- The `;` between the rows of a matrix or cell is an unnamed pattern token that
  a query can't capture. `highlights.scm` captures the whole literal as
  `@punctuation.delimiter` and each `row` as `@embedded` (the plain
  foreground), first in the file so that every other capture beats them.
  `@none` would say this better, but `every_extension_is_well_formed` rejects
  it in a directory with no built-in.
- The sample is a tour: one file holds script code, a function and a
  `classdef`, which the grammar parses cleanly but MATLAB itself would want in
  separate files.
