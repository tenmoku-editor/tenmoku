---
task_uid: "01m8dfkwdvphdyjyckyeqd985w"
task: "T757"
rev: 1
type: "feature"
complexity: "s"
criteria:
  - id: "C1"
    text: "scripts/build-grammar-wasm.sh luau passes, including wasm_parity every_extension_highlights_its_sample for luau"
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
    text: "THIRD_PARTY.md has one appended WebAssembly row for luau crediting the grammar and every adapted query source with commit and licence"
    required: true
    manual: false
---

# Luau (Roblox) syntax as a first-party WebAssembly grammar (extensions/grammars/luau) (Mac T2140)

# Luau (Roblox) syntax as a first-party WebAssembly grammar

Filler card: one new directory, `extensions/grammars/luau/`, following
`extensions/grammars/README.md` and the existing grammars
(`extensions/grammars/julia/` (a scripting language with a scanner); Lua itself is a built-in language, which Luau must not claim anything of). Luau is Roblox's typed Lua dialect, with a very large hobbyist and professional community; `.luau` files (and Roblox project files named `.luau`) open as plain text here today. The built-in `lua` claims `.lua`: leave it alone.

## This language

- Grammar: https://github.com/tree-sitter-grammars/tree-sitter-luau,
  directory `src`. Pin: master (a tree-sitter-grammars org member; stale-looking but Luau's syntax is stable). A more active alternative exists, polychromatist/tree-sitter-luau (MIT, 2025-12, ABI 14) but it ships queries only for helix and an archived nvim: use it only if the org grammar fails to parse the sample (HEAD a8914d6c1fc5131f8e1c13f769fa704c9f5eb02f, 2024-12-22).
- Licence: MIT (`LICENSE`). External scanner: scanner.c (plain C). Parser size: 0.67 MB parser.c (expect a wasm well under 1 MB). ABI 14. Export `luau`: confirm both from `parser.c` (`LANGUAGE_VERSION` and the `tree_sitter_<x>` export) before pinning.
- File extensions: luau. File names: -. Shebangs: luau. Sample:
  `test/sample.luau`.
- Comments: line `--`, block `--[[` `]]`.
- `parity_needs`: none.
- Queries: the grammar's own `queries/highlights.scm` (MIT, 4.7 KB; 4 `#lua-match?` to rewrite as `#match?`, 3 `#eq?`, 4 `#any-of?`, no `; inherits:`). Drop `@spell`; `@keyword.coroutine`, `@module.builtin` and `@function.method.call` need checking against fe-syntax's NAMES/ALIASES (alias or drop). Its `queries/injections.scm` injects `comment`, `luadoc`, `luap` and `c` (FFI): none but `c` exists here, so keep only the `c` injection if it parses, else make `injections.scm` comment-only.
- Notes: the sample should touch a `type` alias with generics and a union, a typed function (`local function f(x: number): string`), `export type`, `local`/`const`-style declarations, a table constructor with fields and methods, `:` method calls, `if`/`elseif`/`else`, `for in`/`while`/`repeat until`, `continue`, a string interpolation (backtick with `{x}`), `--` comment and `--[[ ]]` block, `@native`-style attribute, numbers and `nil`/`true`.

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
- `extension.toml`: `id = "tenmoku.grammar_luau"`, `name = "Luau syntax"`,
  `license` = the grammar's SPDX id, one `[[contributes.grammars]]` with
  `language = "luau"`, the extensions, file names, `line_comment` /
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
- `test/sample.luau`: 40-120 lines of idiomatic code touching type aliases and generics, typed functions, tables and method calls, control flow, interpolated strings, an attribute, numbers, and both comment forms; the tree must parse with no ERROR or
  MISSING node. Generate
  `test/sample.luau.golden` with
  `UPDATE_GOLDEN=1 scripts/build-grammar-wasm.sh luau`, then read it: every
  token class above is coloured and nothing is obviously wrong.
- Attribution: append one row for `luau` to `THIRD_PARTY.md`'s "Grammars
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
- Change nothing outside `extensions/grammars/luau/` and that appended
  row. If the module won't load (ABI, the table validator, a libc import the
  wasm stdlib lacks, over `MAX_GRAMMAR_BYTES` = 16 MiB, the load deadline),
  or a built-in already claims an extension, block the card with the exact
  error: fixing fe-syntax, the vendored tree-sitter or the build script is
  out of scope.

## Implementation notes

- **Grammar**: the fallback the spec names, polychromatist/tree-sitter-luau at
  `71b03e66b2c8dd04e0133c9b998a54a58f239ca4` (main, 0.3.0, ABI 14, export
  `luau`, MIT). The tree-sitter-grammars grammar at `a8914d6c` has no
  attribute rule: `@native` before a `local function` is an ERROR node. The
  module is 0.47 MB.
- **Queries**: `highlights.scm` is adapted from the grammar's
  `_archive/nvim-queries/highlights.scm`, not the org grammar's query, whose
  node names don't fit. No Helix query was copied. `earlier_patterns_win` is
  false (nvim style). `injections.scm` is comment-only: a string's content is
  no node of its own, so a `cdef` injection into `c` would include the quotes.
- **Known limits of the grammar**, found by parsing probes:
  - a `#!` first line is an ERROR node (so `shebangs = ["luau"]` still picks
    the language, and the lines after it parsed cleanly);
  - a generic type pack is rejected as a vararg's type (`...: U...`) and after
    another type in a return list (`(T, U...)`);
  - its scanner ignores `realloc`'s result when a name passes 255 bytes. A
    probe of names up to 20,000 characters (2.5 MB) parsed and highlighted
    without error.
