---
task_uid: "01m8cfa17kzxeffyq126qheq40"
task: "T747"
rev: 1
type: "feature"
complexity: "m"
criteria:
  - id: "C1"
    text: "scripts/build-grammar-wasm.sh cuda passes, including wasm_parity every_extension_highlights_its_sample for cuda"
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
    text: "THIRD_PARTY.md has one appended WebAssembly row for cuda crediting the grammar and every adapted query source with commit and licence"
    required: true
    manual: false
---

# CUDA C++ syntax as a first-party WebAssembly grammar (extensions/grammars/cuda) (Mac T2126)

# CUDA C++ syntax as a first-party WebAssembly grammar

Filler card: one new directory, `extensions/grammars/cuda/`, following
`extensions/grammars/README.md` and the existing grammars. CUDA is the one
grammar here that extends another first-party language: tree-sitter-cuda
extends tree-sitter-cpp, so it keeps C++'s node names and `extensions/grammars/cpp/`
is the source for its query. `.cu` and `.cuh` are everywhere in ML code, and
nothing built in or installable highlights them today.

## This language

- Grammar: https://github.com/tree-sitter-grammars/tree-sitter-cuda, directory
  `src`. Pin: master (HEAD d58080a327756e4d1d16ec329ba7cb2048f6c6cd,
  2026-09-15; also the revision nvim-treesitter pins).
- Licence: MIT (`LICENSE`). External scanner: scanner.c (plain C, inherited
  from the C++ scanner). Parser size: 31 MB (HLSL's 20 MB built to 4.2 MB, so
  expect about 6-7 MB: under `MAX_GRAMMAR_BYTES` but say the size). ABI 15.
  Export `cuda`.
- File extensions: cu, cuh. File names: -. Shebangs: -. Sample:
  `test/sample.cu`. (Do not claim `h`/`cpp`: they belong to the built-ins.)
- Comments: line `//`, block `/*` `*/`.
- `parity_needs`: none (the printf-c injection below is a built-in).
- Queries: the grammar's `queries/highlights.scm` is 23 lines and starts
  `; inherits: cpp`: inline `extensions/grammars/cpp/highlights.scm` (this
  repo's, already self-contained and in fe-syntax's vocabulary) ahead of
  CUDA's own patterns (`<<<`/`>>>` brackets; `__host__`, `__device__`,
  `__global__`, `__managed__`, `__forceinline__`, `__noinline__`,
  `__launch_bounds__` as keywords, which `@keyword.modifier` already
  resolves to). Credit both sources in the header and the THIRD_PARTY row;
  both MIT. Verify every node name in the cpp query exists in the CUDA grammar
  (it is a fork of tree-sitter-cpp 0.23.x; cpp here pins 0.23.4) and drop
  patterns that don't. Injections: inline `extensions/grammars/cpp/injections.scm`
  (printf-c into printf-style calls, a built-in language) the same way.
- Notes: the sample should touch an `#include`, a `__global__` kernel with
  `threadIdx`/`blockIdx`, `__shared__` memory, `__device__` and
  `__host__ __device__` helpers, a `template`, a kernel launch
  `kernel<<<grid, block>>>(args)` and one with shared-memory and stream
  arguments, `cudaMalloc`/`cudaMemcpy` with a `cudaMemcpyHostToDevice`
  constant, `__syncthreads()`, a `printf`, a raw string, numbers with
  `f` suffix, and both comment forms.

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
- `extension.toml`: `id = "tenmoku.grammar_cuda"`, `name = "CUDA syntax"`,
  `license` = the grammar's SPDX id, one `[[contributes.grammars]]` with
  `language = "cuda"`, the extensions, file names, `line_comment` /
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
- `test/sample.cu`: 40-120 lines of idiomatic code touching kernels and launch brackets, memory-space qualifiers, templates, runtime API calls, a printf, numbers and both comment forms; the tree must parse with no ERROR or
  MISSING node. Generate
  `test/sample.cu.golden` with
  `UPDATE_GOLDEN=1 scripts/build-grammar-wasm.sh cuda`, then read it: every
  token class above is coloured and nothing is obviously wrong.
- Attribution: append one row for `cuda` to `THIRD_PARTY.md`'s "Grammars
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
- Change nothing outside `extensions/grammars/cuda/` and that appended
  row. If the module won't load (ABI, the table validator, a libc import the
  wasm stdlib lacks, over `MAX_GRAMMAR_BYTES` = 16 MiB, the load deadline),
  or a built-in already claims an extension, block the card with the exact
  error: fixing fe-syntax, the vendored tree-sitter or the build script is
  out of scope.

## Implementation notes

- Pinned d58080a (still master's HEAD); version 0.21.2 from tree-sitter.json.
  The module builds to 7.1 MB (7098691 bytes), under `MAX_GRAMMAR_BYTES`.
- Every node and field name in `extensions/grammars/cpp/highlights.scm`
  exists in tree-sitter-cuda's node-types.json, so it is inlined verbatim.
- The C++ query is tree-sitter-c's and tree-sitter-cpp's (another holder's
  MIT notice than tree-sitter-cuda's two-holder LICENSE), so its text ships
  as `LICENSE-TREE-SITTER-CPP`, as objc's `LICENSE-TREE-SITTER-C` does.
- Added beyond upstream's CUDA patterns, marked "Ours": `__shared__`,
  `__constant__`, `__local__`, `__tile__`, `__tile_global__` and
  `__grid_constant__` as `@keyword.modifier` (upstream leaves the sample's
  `__shared__` plain), `threadIdx`/`blockIdx`/`blockDim`/`gridDim`/`warpSize`
  as `@variable.builtin`, and the `cudaMemcpy*` kinds and `cudaSuccess` as
  `@constant`.
- `dim3 block(BLOCK_SIZE);` colours `block` as a function: tree-sitter-cpp's
  most-vexing-parse reading, the same as the C++ grammar.
- `earlier_patterns_win = false`: the inlined C++ query is last-wins
  (generic `(identifier) @variable` first), as `extensions/grammars/cpp/`
  ships it; review round 1's F1 asked for true and is disputed on that ground.
- Commits for the inlined sources: tree-sitter-c 0.24.2 =
  b780e47fc780ddc8da13afa35a3f4ed5c157823d, tree-sitter-cpp 0.23.4 =
  f41e1a044c8a84ea9fa8577fdd2eab92ec96de02 (the crates' `.cargo_vcs_info.json`);
  the cpp directory's queries as of this repository's 9c054a3e9.
