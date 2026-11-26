---
task_uid: "01m8cfa2ttftnd8959ej3jc4mn"
task: "T750"
rev: 1
type: "feature"
complexity: "s"
criteria:
  - id: "C1"
    text: "scripts/build-grammar-wasm.sh mermaid passes, including wasm_parity every_extension_highlights_its_sample for mermaid"
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
    text: "THIRD_PARTY.md has one appended WebAssembly row for mermaid crediting the grammar and every adapted query source with commit and licence"
    required: true
    manual: false
---

# Mermaid diagrams syntax as a first-party WebAssembly grammar (extensions/grammars/mermaid) (Mac T2133)

# Mermaid diagrams syntax as a first-party WebAssembly grammar

Filler card: one new directory, `extensions/grammars/mermaid/`, following
`extensions/grammars/README.md` and the existing grammars
(`extensions/grammars/kdl/` is a close small one). Mermaid lives mostly in
Markdown fences (` ```mermaid `) and READMEs; fe-syntax's markdown already
injects a fence's language by name, so installing this colours those fences as
well as `.mmd` files.

## This language

- Grammar: https://github.com/monaqa/tree-sitter-mermaid, directory `src`.
  Pin: master (HEAD 90ae195b31933ceb9d079abfa8a3ad0a36fee4cc, 2024-04-22; also
  the revision nvim-treesitter pins). Note the age: it covers the 2024
  diagram set (flowchart, sequence, class, state, ER, gantt, pie, journey,
  gitGraph, requirement, mindmap), not newer ones such as `architecture-beta`
  or `kanban`; say so in the card.
- Licence: MIT (`LICENSE`). External scanner: none. Parser size: 0.94 MB
  (expect a wasm well under 1 MB). ABI 14. Export `mermaid`.
- File extensions: mmd, mermaid. File names: -. Shebangs: -. Sample:
  `test/sample.mmd`.
- Comments: line `%%`.
- `parity_needs`: none.
- Queries: the grammar's own `queries/highlights.scm` (MIT, 3 KB, no
  predicates): use it. nvim-treesitter's copy (Apache-2.0, "adapted from"
  this one) is the same plus a comment injection: stay with the repo's file.
  Injections: comment-only.
- Notes: the sample should touch one block per diagram type the grammar
  covers: a `flowchart TD` with node shapes, labelled edges and a subgraph, a
  `sequenceDiagram` with participants, arrows, a `loop` and a `Note`, a
  `classDiagram`, a `stateDiagram-v2`, an `erDiagram`, a `gantt`, a `pie`,
  a `%%` comment and a `%%{init: ...}%%` directive. If one diagram type
  gives ERROR, drop it from the sample and list it in the finish note.

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
- `extension.toml`: `id = "tenmoku.grammar_mermaid"`, `name = "Mermaid syntax"`,
  `license` = the grammar's SPDX id, one `[[contributes.grammars]]` with
  `language = "mermaid"`, the extensions, file names, `line_comment` /
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
- `test/sample.mmd`: 40-120 lines of idiomatic code touching each supported diagram type (flowcharts with shapes and subgraphs, sequence, class, state, ER, gantt and pie diagrams) and the comment and directive forms; the tree must parse with no ERROR or
  MISSING node. Generate
  `test/sample.mmd.golden` with
  `UPDATE_GOLDEN=1 scripts/build-grammar-wasm.sh mermaid`, then read it: every
  token class above is coloured and nothing is obviously wrong.
- Attribution: append one row for `mermaid` to `THIRD_PARTY.md`'s "Grammars
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
- Change nothing outside `extensions/grammars/mermaid/` and that appended
  row. If the module won't load (ABI, the table validator, a libc import the
  wasm stdlib lacks, over `MAX_GRAMMAR_BYTES` = 16 MiB, the load deadline),
  or a built-in already claims an extension, block the card with the exact
  error: fixing fe-syntax, the vendored tree-sitter or the build script is
  out of scope.

## Implementation notes

- Pinned monaqa/tree-sitter-mermaid at `90ae195b31933ceb9d079abfa8a3ad0a36fee4cc`
  (2024-04-22, ABI 14, no scanner).
- The grammar allows one diagram per file (after any `%%` comments and
  `%%{…}%%` directives), so one sample can't touch every type without ERROR
  nodes. `test/sample.mmd` holds the flowchart and the directive, and each other
  type has its own `test/sample.<type>.mmd` with its golden file (sequence,
  class, state, er, gantt, pie, mindmap). Every one parses without errors; no
  diagram type was dropped.
- Coverage is flowchart, sequence, class, state, ER, gantt, pie and mindmap. At
  this commit the grammar has no journey, gitGraph or requirement diagram, though
  the card lists them, nor newer ones such as `architecture-beta` or `kanban`.
- Forms the grammar rejects, kept out of the samples: the `graph` spelling of
  `flowchart` (such a diagram doesn't parse), text after `else` in an `alt`,
  `inclusiveEndDates` with no value, and a `;` before the final newline of a
  flowchart.
- The upstream query is followed by patterns of ours for what it leaves plain
  (labels and free text, `actor`, `showData`, directions, class members, mind
  maps). The `mindmap` word itself stays plain: its token has no name a query
  can match.
