# WebAssembly grammar memory cap (T759)

Spec: `docs/specs/aqqz9w5mhj-give-each-webassembly-grammar-its-own.md`. The
memory claim itself, and how many threads parse WebAssembly, are in
`vendor/tree-sitter/README.md` (Memory).

## Decision

`MAX_MEMORY_SIZE` in `vendor/tree-sitter/src/wasm_store.c` is **64 MiB**
(1,024 pages of 64 KiB). Upstream's is 128 MiB. Each WebAssembly grammar
gets a store of its own on each thread, so this caps one grammar's memory,
its static data and its own `memory.grow` alike.

## Measurement

On helium, 2026-11-27, `scripts/build-grammar-wasm.sh` built every
first-party language, 88 modules, with wasi-sdk 34. Each module's `dylink.0`
custom section says how much static data it has (`memory_size`). A store
needs that, plus:

- 131,100 bytes before it: the stdlib's two initial pages, which hold its
  stack and statics, and then the lexer (`MEMORY_BASE` in
  `crates/fe-syntax/tests/wasm_table_validation.rs`);
- 1,024 bytes after it, the serialization buffer
  (`TREE_SITTER_SERIALIZATION_BUFFER_SIZE`);
- the 4 MiB an external scanner's `malloc` heap may grow to (`MAX_HEAP_SIZE`
  in `vendor/tree-sitter/src/wasm-stdlib/external_scanner_allocator.c`).

The five largest needs:

| Grammar | `memory_size` (bytes) | Static data | Need |
| --- | ---: | ---: | ---: |
| verilog | 17,989,128 | 17.16 MiB | 21.28 MiB |
| fsharp | 11,264,128 | 10.74 MiB | 14.87 MiB |
| razor | 10,485,296 | 10.00 MiB | 14.13 MiB |
| nim | 8,353,808 | 7.97 MiB | 12.09 MiB |
| cuda | 6,889,640 | 6.57 MiB | 10.70 MiB |

## Rule

The cap is the smallest power-of-two number of MiB that is at least twice
the largest need, and no more than 128 MiB. Twice 21.28 MiB is 42.56 MiB, so
the cap is 64 MiB. The margin leaves room for a grammar's next release and
for a scanner that grows its memory past the heap the stdlib's allocator
allows.

With 64 MiB, `scripts/build-grammar-wasm.sh` for every language passes
again: each module loads and highlights its sample as its golden says
(`wasm_parity.rs`, `every_extension_highlights_its_sample`).

## Revisit when

- a first-party or popular grammar needs more than 32 MiB: rerun the
  measurement and apply the rule;
- tree-sitter changes its store's memory layout (the stdlib's pages, the
  serialization buffer or the heap limit).
