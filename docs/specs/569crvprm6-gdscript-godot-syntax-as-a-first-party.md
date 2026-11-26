---
task_uid: "01m8cey91jkv1qz4569crvprm6"
task: "T2134"
rev: 1
type: "feature"
complexity: "s"
criteria:
  - id: "C1"
    text: "scripts/build-grammar-wasm.sh gdscript passes, including wasm_parity every_extension_highlights_its_sample for gdscript"
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
    text: "THIRD_PARTY.md has one appended WebAssembly row for gdscript crediting the grammar and every adapted query source with commit and licence"
    required: true
    manual: false
---

# GDScript (Godot) syntax as a first-party WebAssembly grammar (extensions/grammars/gdscript)

# GDScript (Godot) syntax as a first-party WebAssembly grammar

Filler card: one new directory, `extensions/grammars/gdscript/`,
following `extensions/grammars/README.md` and the existing grammars
(`extensions/grammars/nim/` is the closest: an indentation-scoped language
with an external scanner). GDScript is on the Stack Overflow 2025 survey's
list and Godot is the large game-engine community this covers.

## This language

- Grammar: https://github.com/PrestonKnopp/tree-sitter-gdscript, directory
  `src`. Pin: try master HEAD 872e2e96905892c954da4101c74b884d0155f4d8
  (2026-10-21); if the nvim-treesitter queries below fail to compile against
  it, pin 9686853b696db07118ad110e440d6de0ca6498b4 (2025-09-18), the
  revision nvim-treesitter pins (HEAD is 93 commits and a grammar.js change
  past it).
- Licence: MIT (`LICENSE`). External scanner: scanner.c (plain C). Parser
  size: 2.5 MB (expect a wasm under 1 MB). ABI 14 at both. Export `gdscript`.
- File extensions: gd. File names: -. Shebangs: -. Sample: `test/sample.gd`.
- Comments: line `#`.
- `parity_needs`: none.
- Queries: the grammar repo has NO `queries/highlights.scm`. Use
  nvim-treesitter's `runtime/queries/gdscript/highlights.scm` (Apache-2.0,
  36 KB, from nvim-treesitter main 52773f0d13a0c11d0c3ef33c14ab6ecc1c17148b,
  2026-11-21): add `LICENSE-APACHE` and credit it in the header and the
  THIRD_PARTY row. It is nvim style (`earlier_patterns_win = false`). Rewrite
  its two `#lua-match?` (`^[A-Z]` and `^[A-Z][A-Z_0-9]*$`) as `#match?`; keep
  `#any-of?`/`#eq?`; drop `@spell`/`@nospell`/`@none`. Its injection file is
  a comment injection only: make `injections.scm` comment-only.
- Notes: the sample should touch `extends`/`class_name`, `signal`, `@export`
  and `@onready` annotations, `var`/`const` with type hints, `func` with
  typed parameters and a return type, `static func`, an `enum`, `match`
  with patterns, `for ... in`, `await`, a lambda, a `$Node/Path` and `%Unique`
  node path, a `&"StringName"`, `preload("res://...")`, a string with escapes,
  numbers (`0xFF`, `1_000`, `1.5e3`) and `#` comments. Godot's `.tscn`,
  `.tres` and `project.godot` are separate (tree-sitter-godot-resource,
  Tier 2).

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
- `extension.toml`: `id = "tenmoku.grammar_gdscript"`, `name = "GDScript syntax"`,
  `license` = the grammar's SPDX id, one `[[contributes.grammars]]` with
  `language = "gdscript"`, the extensions, file names, `line_comment` /
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
- `test/sample.gd`: 40-120 lines of idiomatic code touching class declarations, annotations, signals, typed functions, match, lambdas, node paths, string names, numbers and comments; the tree must parse with no ERROR or
  MISSING node. Generate
  `test/sample.gd.golden` with
  `UPDATE_GOLDEN=1 scripts/build-grammar-wasm.sh gdscript`, then read it: every
  token class above is coloured and nothing is obviously wrong.
- Attribution: append one row for `gdscript` to `THIRD_PARTY.md`'s "Grammars
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
- Change nothing outside `extensions/grammars/gdscript/` and that appended
  row. If the module won't load (ABI, the table validator, a libc import the
  wasm stdlib lacks, over `MAX_GRAMMAR_BYTES` = 16 MiB, the load deadline),
  or a built-in already claims an extension, block the card with the exact
  error: fixing fe-syntax, the vendored tree-sitter or the build script is
  out of scope.

## Implementation notes

- Pin: `9686853b696db07118ad110e440d6de0ca6498b4`, the card's fallback. At
  HEAD `872e2e96905892c954da4101c74b884d0155f4d8` the nvim-treesitter query
  fails to compile (`Query error at 54:2. Invalid node type "tool_statement"`,
  the first failing pattern; others weren't checked). The pinned parser's ABI
  is 15, not 14; it loads. The wasm is about 270 KB.
- Known limits of the pin, each parsed with errors by it and cleanly by HEAD:
  an inferred-type parameter default (`func f(delay := 1.5)`), `class_name A
  extends B` on one line, `@abstract func f()` with no body, variadic
  `...args`, and non-ASCII identifiers. The sample avoids them. A follow-up
  could move the pin forward and adapt the query to the newer node types.
- `#region`/`#endregion` markers are uncoloured (hidden tokens in this
  grammar); a region's label is a comment.
- Upstream's annotation pattern would never match here: fe-syntax requires
  every node of a capture to satisfy `#any-of?`, and upstream captured "@"
  with the name. The query's header lists this and the other changes.
