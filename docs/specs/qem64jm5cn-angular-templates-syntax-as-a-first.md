---
task_uid: "01m8cfa1rgm5deqtqem64jm5cn"
task: "T748"
rev: 1
type: "feature"
complexity: "m"
criteria:
  - id: "C1"
    text: "scripts/build-grammar-wasm.sh angular passes, including wasm_parity every_extension_highlights_its_sample for angular"
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
    text: "THIRD_PARTY.md has one appended WebAssembly row for angular crediting the grammar and every adapted query source with commit and licence"
    required: true
    manual: false
---

# Angular templates syntax as a first-party WebAssembly grammar (extensions/grammars/angular) (Mac T2127)

# Angular templates syntax as a first-party WebAssembly grammar

Filler card: one new directory, `extensions/grammars/angular/`, following
`extensions/grammars/README.md` and the existing grammars
(`extensions/grammars/blade/` and `astro/` are the model: grammars that extend
tree-sitter-html and inject code). Angular is the most-used framework with no
template highlighting today: `app.component.html` is plain HTML here.
Depends on the compound-extension support T1632 shipped (so
`component.html` wins over the built-in `html` for `x.component.html`).

## This language

- Grammar: https://github.com/dlvandenberg/tree-sitter-angular, directory
  `src`. Pin: main (HEAD 38a8014ed5452cd6b7cf1399c00177a1f5374256,
  2026-05-15; also the revision nvim-treesitter pins). Supports Angular 21
  syntax (control flow `@if`/`@for`/`@switch`/`@defer`, `@let`, ICU).
- Licence: MIT (`LICENSE`). External scanner: scanner.c. Parser size: 0.68 MB
  (expect a wasm well under 1 MB). ABI 15. Export `angular`.
- File extensions: component.html (the compound form, as `blade.php`). Say in
  the finish note that Angular's other template names (`*.ng.html`,
  `*.container.html`) are not claimed. File names: -. Shebangs: -. Sample:
  `test/sample.component.html`.
- Comments: block `<!--` `-->`.
- `parity_needs`: none (`html`, `css` and `javascript` are built in).
- Queries: the grammar's own `queries/highlights.scm` (MIT, 2.6 KB) begins
  `; inherits: html`: inline tree-sitter-html 0.23.2's
  `queries/highlights.scm` and `queries/injections.scm` (MIT; `<script>` ->
  javascript, `<style>` -> css) ahead of Angular's own, as `astro/` and
  `blade/` do, and credit both sources in the headers and the THIRD_PARTY
  row. Angular's one `#lua-match?` (`"#.*"` on a template reference
  attribute name) is rewritten `#match?`; `#any-of?`/`#eq?` are supported.
  `@variable.member` etc. resolve through the more-specific rule. Do not use
  nvim-treesitter's copy (it inherits `html_tags`, Apache-2.0): the grammar's
  own file needs no `LICENSE-APACHE`.
- Notes: the sample should touch an element with a `{{ expr | pipe:arg }}`
  interpolation, `[prop]="x"`, `(event)="handler($event)"`, `[(ngModel)]`,
  `[class.active]`, a template reference `#ref`, `*ngIf="cond; else tpl"`,
  the control-flow blocks `@if (a) {} @else {}`, `@for (x of xs; track x.id)`
  with `@empty`, `@switch`/`@case`, `@defer (on viewport) {}` with
  `@placeholder`, `@let x = a + b;`, an `<ng-container>`/`<ng-template>`,
  a comment and a `<script>`/`<style>`. Template expressions are parsed by the
  Angular grammar itself, so there is no JavaScript injection to exercise.

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
- `extension.toml`: `id = "tenmoku.grammar_angular"`, `name = "Angular template syntax"`,
  `license` = the grammar's SPDX id, one `[[contributes.grammars]]` with
  `language = "angular"`, the extensions, file names, `line_comment` /
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
- `test/sample.component.html`: 40-120 lines of idiomatic code touching interpolation and pipes, property and event bindings, template references, structural directives, the control-flow blocks, `@let`, a comment and a `<script>`; the tree must parse with no ERROR or
  MISSING node (an injected layer may have errors). Generate
  `test/sample.component.html.golden` with
  `UPDATE_GOLDEN=1 scripts/build-grammar-wasm.sh angular`, then read it: every
  token class above is coloured and nothing is obviously wrong.
- Attribution: append one row for `angular` to `THIRD_PARTY.md`'s "Grammars
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
- Change nothing outside `extensions/grammars/angular/` and that appended
  row. If the module won't load (ABI, the table validator, a libc import the
  wasm stdlib lacks, over `MAX_GRAMMAR_BYTES` = 16 MiB, the load deadline),
  or a built-in already claims an extension, block the card with the exact
  error: fixing fe-syntax, the vendored tree-sitter or the build script is
  out of scope.

## Implementation notes

- Pinned main's head `38a8014ed5452cd6b7cf1399c00177a1f5374256` (0.9.2); it
  builds to a 137 KiB wasm (ABI 15, export `angular`) and the sample parses
  with no ERROR or MISSING node.
- Claims only `component.html`. Angular's other template names
  (`*.ng.html`, `*.container.html`) are not claimed.
- The queries inline tree-sitter-html 0.23.2's (tag `v0.23.2`, commit
  `5a5ca8551a179998360b4a4ca2c0f366a35acc03`); as razor/ does for another
  holder's MIT query, its licence ships verbatim as
  `LICENSE-TREE-SITTER-HTML`.
- Angular's query is Neovim-style (general `(identifier) @variable` first,
  overrides after), so `earlier_patterns_win = false`; that also lets the
  `#ref` pattern beat HTML's `(attribute_name) @attribute`. Its
  `#lua-match? "#.*"` is `#match? "^#"`.
