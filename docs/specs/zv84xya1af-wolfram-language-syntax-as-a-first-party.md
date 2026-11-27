---
task_uid: "01m8cgmbvevtche3zv84xya1af"
task: "T2147"
rev: 1
type: "feature"
complexity: "s"
criteria:
  - id: "C1"
    text: "scripts/build-grammar-wasm.sh wolfram passes, including wasm_parity every_extension_highlights_its_sample for wolfram"
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
    text: "THIRD_PARTY.md has one appended WebAssembly row for wolfram crediting the grammar and every adapted query source with commit and licence"
    required: true
    manual: false
---

# Wolfram Language syntax as a first-party WebAssembly grammar (extensions/grammars/wolfram)

# Wolfram Language syntax as a first-party WebAssembly grammar

Filler card: one new directory, `extensions/grammars/wolfram/`, following
`extensions/grammars/README.md` and the existing grammars
(`extensions/grammars/julia/` (a scientific-computing language with a scanner) is the closest). The Wolfram Language (Mathematica, Wolfram Engine, Wolfram Cloud) has `.wl`, `.wls`, `.wlt`, `.mt` files that open as plain text here today.

## This language

- Grammar: https://github.com/WolframResearch/tree-sitter-wolfram (the vendor's own repo, based on bostick/tree-sitter-wolfram),
  directory `src`. Pin: master (HEAD f7e561abf9ca78c511e629290c7245dbfaf7a8f8, 2026-11-25).
- Licence: MIT (`LICENSE`). External scanner: scanner.c (2.5 KB, plain C: nested `(* *)` comments and statement separators). Parser size: 3.3 MB parser.c (est. ~0.7 MB wasm). ABI 15. Export `wolfram`: confirm both from `parser.c` (`LANGUAGE_VERSION` and the `tree_sitter_<x>` export) before pinning.
- File extensions: wl, wls, wlt, mt. File names: -. Shebangs: `wolframscript` (`#!/usr/bin/env wolframscript` heads `.wls` scripts). Do NOT claim `m` (objc's; see T2136) and do NOT claim `nb` (see Notes). Sample: `test/sample.wl`.
- Comments: block only, `(*` `*)`, nesting allowed; no line comment. Set `block_comment = ["(*", "*)"]` and omit `line_comment` if the manifest allows (else see the well-formed check and block with the exact error).
- `parity_needs`: none.
- Queries: the grammar repo ships NO queries. Write `highlights.scm` yourself against `src/node-types.json` (named nodes: `comment`, `string`, `integer`, `real`, `symbol`, `call` with `head:`/`arguments:` fields, `pattern`, `blank`/`blank_sequence`/`blank_null_sequence`/`blank_default`, `slot`/`slot_sequence`, `message_name`/`message_tag`, `named_character`, `file_name`, `out`, `infix`/`prefix`/`postfix`/`span`/`tilde`/`part`/`group`/`implicit_times`, `type_specifier`, `freeform_evaluate`). Operators are anonymous tokens. Do NOT copy the Zed extension's queries (WolframResearch/zed-wolfram-highlighter, `languages/wolfram/highlights.scm`, 107 KB): they are generated from vscode-wolfram, whose licence GitHub reports as NOASSERTION. Instead: `(comment) @comment`, `(string) @string`, `(integer)`/`(real)` `@number`, `(call head: (symbol) @function)`, `(pattern ...)`/blank nodes `@variable.parameter` or `@operator`, `(slot)` `@variable.parameter`, `(message_name)` `@string.special`, `(named_character)` `@string.escape`, a short hand-written `#match?` list of control and scoping heads (`If|Which|Switch|Do|For|While|Module|Block|With|Table|Map|Function|Print|Return|Throw|Catch|Check|Begin|End|BeginPackage|EndPackage|Needs|Get|Package`) as `@keyword`, `$`-prefixed symbols as `@constant.builtin`, `True|False|Null|None|All|Automatic|Infinity|Pi|E|I|Missing` as `@constant.builtin`, and operators as `@operator`. Check every capture against fe-syntax's NAMES/ALIASES. Keep it a few KB; a full System`-symbol table is out of scope. `injections.scm`: comment-only file.
- Notes: the sample should touch a `BeginPackage`/`Begin`/`End`/`EndPackage` block, a `(* nested (* comment *) *)`, `f[x_, y_Integer] := x^2 + y`, `f[x_] /; x > 0 := ...`, `SetDelayed`/`Set`/`Rule`/`RuleDelayed` (`->`, `:>`), `/@`, `@@`, `//`-postfix and `@` prefix application, `#`/`#1`/`##` slots with `&`, `x_`/`__`/`___`/`x_.` patterns, `Part` (`a[[1]]`), `Span` (`;;`), `::usage` messages, a string with an escape and a `\[Alpha]` named character, `1.5*^10` and `2^^101` numbers, `%`/`%%` out, a `<<file` get, `Module`/`Block`/`With`, `Association` `<| |>`, and `?Name` info. Constructs that give ERROR: drop that line and say so in the finish note. Known upstream state: the grammar is under active repair (open PRs on operator associativity and symbol tokens, 2026-11); 7 corpus files, no highlight tests, a handful of stars, so expect a few ERROR nodes on exotic operators; the card passes when the idiomatic sample is clean.
- `.nb` notebooks are deliberately NOT claimed: they are machine-generated `Notebook[{Cell[...]}]` expressions, often megabytes (embedded images as base64 in `Cell[BoxData[...]]`), hand-edited never, and the parser's cost on them would dwarf the benefit. Leave them plain text.
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
- `extension.toml`: `id = "tenmoku.grammar_wolfram"`, `name = "Wolfram syntax"`,
  `license` = the grammar's SPDX id, one `[[contributes.grammars]]` with
  `language = "wolfram"`, the extensions, file names, `line_comment` /
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
- `test/sample.wl`: 40-120 lines of idiomatic code touching every construct the Notes list; the tree must parse with no ERROR or
  MISSING node. Generate
  `test/sample.wl.golden` with
  `UPDATE_GOLDEN=1 scripts/build-grammar-wasm.sh wolfram`, then read it: every
  token class above is coloured and nothing is obviously wrong.
- Attribution: append one row for `wolfram` to `THIRD_PARTY.md`'s "Grammars
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
- Change nothing outside `extensions/grammars/wolfram/` and that appended
  row. If the module won't load (ABI, the table validator, a libc import the
  wasm stdlib lacks, over `MAX_GRAMMAR_BYTES` = 16 MiB, the load deadline),
  or a built-in already claims an extension, block the card with the exact
  error: fixing fe-syntax, the vendored tree-sitter or the build script is
  out of scope.

## Implementation notes

- Pinned f7e561abf9ca78c511e629290c7245dbfaf7a8f8, still `master`'s head when
  built. ABI 15, export `tree_sitter_wolfram`; the module is 871 KB.
- Dropped from the sample: `?Name` info. The grammar has no prefix `?`, so
  `?Table`, `??Table` and `? Table` each parse as `ERROR`.
- `\[Alpha]` and other letter-like long names lex as `symbol`, not
  `named_character`; a `#match?` pattern colours a symbol that is exactly one.
- A string is one token, so its escapes and long names have no colour of their
  own.
- The grammar has no `#!` node: a script's first line parses as slot, `!` and
  divisions. `highlights.scm` colours `#!/...` as a comment. A flag with `-`
  in it (`#!/usr/bin/env -S ...`) would still show its `-` as an operator.
- A symbol no pattern names has no capture.
- No helpers or other code were added; only the grammar's directory and its
  `THIRD_PARTY.md` row.
