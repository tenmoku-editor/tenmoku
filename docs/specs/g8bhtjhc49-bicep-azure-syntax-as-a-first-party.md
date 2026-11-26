---
task_uid: "01m8cexxs4b01hccg8bhtjhc49"
task: "T2128"
rev: 1
type: "feature"
complexity: "s"
criteria:
  - id: "C1"
    text: "scripts/build-grammar-wasm.sh bicep passes, including wasm_parity every_extension_highlights_its_sample for bicep"
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
    text: "THIRD_PARTY.md has one appended WebAssembly row for bicep crediting the grammar and every adapted query source with commit and licence"
    required: true
    manual: false
---

# Bicep (Azure) syntax as a first-party WebAssembly grammar (extensions/grammars/bicep)

# Bicep (Azure) syntax as a first-party WebAssembly grammar

Filler card: one new directory, `extensions/grammars/bicep/`, following
`extensions/grammars/README.md` and the existing grammars
(`extensions/grammars/hcl/` and `terraform/` are the closest: infrastructure
as code, `packs = ["infra"]` territory). Bicep is Azure's Terraform
alternative and is otherwise plain text here.

## This language

- Grammar: https://github.com/tree-sitter-grammars/tree-sitter-bicep,
  directory `src`. Pin: master (HEAD bff59884307c0ab009bd5e81afd9324b46a6c0f9,
  2024-12-22; also the revision nvim-treesitter pins). The commit is
  stale-looking but the repo is a maintained tree-sitter-grammars org member
  and Bicep's syntax is stable.
- Licence: MIT (`LICENSE`). External scanner: scanner.c (plain C). Parser
  size: 1.1 MB (expect a wasm well under 1 MB). ABI 14. Export `bicep`.
- File extensions: bicep, bicepparam. File names: -. Shebangs: -. Sample:
  `test/sample.bicep`.
- Comments: line `//`, block `/*` `*/`.
- `parity_needs`: none.
- Queries: the grammar's own `queries/highlights.scm` (MIT, 3 KB, one `#eq?`,
  identical to nvim-treesitter's): use it (set `earlier_patterns_win` by the file's pattern ordering; check). Its injection pair is a
  comment injection only: make `injections.scm` comment-only.
- Notes: the sample should touch `targetScope`, `param` with a decorator
  (`@description`, `@secure()`, `@allowed`), `var`, `resource` with an
  API-versioned type string and `parent:`, a `module`, `output`, an
  interpolated string `'${x}-suffix'`, a multiline `'''` string, a ternary,
  `for` loop and `if` conditional resource, a user-defined `type`, a function
  call (`resourceGroup().location`), numbers, `null`/`true`, and both comment
  forms.

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
- `extension.toml`: `id = "tenmoku.grammar_bicep"`, `name = "Bicep syntax"`,
  `license` = the grammar's SPDX id, one `[[contributes.grammars]]` with
  `language = "bicep"`, the extensions, file names, `line_comment` /
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
- `test/sample.bicep`: 40-120 lines of idiomatic code touching decorators, params, vars, resources and modules, outputs, interpolated and multiline strings, loops and conditions, and both comment forms; the tree must parse with no ERROR or
  MISSING node. Generate
  `test/sample.bicep.golden` with
  `UPDATE_GOLDEN=1 scripts/build-grammar-wasm.sh bicep`, then read it: every
  token class above is coloured and nothing is obviously wrong.
- Attribution: append one row for `bicep` to `THIRD_PARTY.md`'s "Grammars
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
- Change nothing outside `extensions/grammars/bicep/` and that appended
  row. If the module won't load (ABI, the table validator, a libc import the
  wasm stdlib lacks, over `MAX_GRAMMAR_BYTES` = 16 MiB, the load deadline),
  or a built-in already claims an extension, block the card with the exact
  error: fixing fe-syntax, the vendored tree-sitter or the build script is
  out of scope.
