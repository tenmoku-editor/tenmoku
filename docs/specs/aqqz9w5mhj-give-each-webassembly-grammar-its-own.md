---
task_uid: "01m8e8hkq29d7gr0aqqz9w5mhj"
task: "T759"
rev: 3
type: "bugfix"
complexity: "m"
criteria:
  - id: "C1"
    text: "a_grammar_that_fills_its_store_leaves_other_grammars_parsing (crates/fe-syntax/tests/wasm_memory_cap.rs) passes and fails at base: a copy of the JSON fixture whose dylink.0 memory_size fills a 128 MiB store is parsed first on a fresh thread, and the plain json_wasm grammar still highlights as the compiled-in JSON on that thread (assert_eq!)"
    required: true
    manual: false
  - id: "C2"
    text: "a_grammar_growing_its_memory_stops_at_the_cap: the new hostile fixture tree-sitter-grow.wasm (built and hashed as tests/fixtures/hostile-grammars.md documents, its section and SHA-256 added there) parses with a capped node and no uncapped node, and json_wasm still highlights on the same thread afterwards"
    required: true
    manual: false
  - id: "C3"
    text: "an_overflowing_static_size_is_a_load_error: a module declaring a dylink.0 memory_size of 0xFFFF_FF00 makes register_language return an error naming its file; wasm_store.c checks the offset sum (marked fastedit patch)"
    required: true
    manual: false
  - id: "C4"
    text: "a_thread_keeps_at_most_n_wasm_parsers: N+1 WebAssembly grammars parsed in turn on one thread raise wasm_stats().stores by at most WASM_PARSERS_PER_THREAD; native languages still share the one cached parser"
    required: true
    manual: false
  - id: "C5"
    text: "The cap is chosen from measurement: the decision file lists the five largest first-party module needs (dylink.0 memory_size plus stdlib pages, the 4 MiB scanner heap and stack) and the chosen cap; MAX_MEMORY_SIZE in wasm_store.c equals it (at most 128 MiB); scripts/build-grammar-wasm.sh (all languages) passes with it"
    required: true
    manual: false
  - id: "C6"
    text: "vendor/tree-sitter/README.md (What changed, and the memory claim: the cap per grammar per thread and the number of threads that parse WebAssembly) states the memory bound; the finish notes give the one memory sentence for docs/decisions/vmtnmkay62-extensions-and-plugins.md section 6 (T845/T230 paragraph), ready to paste, since helium T662 holds that file"
    required: true
    manual: false
  - id: "C7"
    text: "cargo nextest run -p fe-syntax passes; runtime_grammar.rs and wasm_table_validation.rs are unchanged and pass"
    required: true
    manual: false
---

# Give each WebAssembly grammar its own store per thread, so the store's memory cap applies to each grammar and one grammar can't fill the store the others parse in; prove the cap with a hostile grammar (Mac T2172)

# Give each WebAssembly grammar its own store per thread, so the store's memory cap applies to each grammar and one grammar can't fill the store the others parse in; prove the cap with a hostile grammar

User request (2026-11-26), after asking what our security story is against a
malicious tree-sitter grammar: check, and enforce, a memory cap on
WebAssembly grammars.

## Today (read from main)

- **A cap exists, per store, and nothing tests it.**
  `vendor/tree-sitter/src/wasm_store.c` defines `MAX_MEMORY_SIZE` (128 MiB in
  64 KiB pages, ~L215). This is upstream code, not a fastedit patch. `ts_wasm_store_new` creates the store's one
  linear memory with that maximum (~L943), and every grammar module imports
  that memory. So a module's own `memory.grow` past 128 MiB returns -1 inside
  the module, and static data that doesn't fit is a load error ("invalid
  memory size", ~L1296). The stdlib allocator caps a scanner's `malloc` heap
  at 4 MiB (`vendor/tree-sitter/src/wasm-stdlib/external_scanner_allocator.c`,
  `MAX_HEAP_SIZE`), but a hostile module can call `memory.grow` directly.
  Nothing else limits memory: `crates/fe-syntax/src/runtime.rs` builds the
  engine with only `epoch_interruption` (~L457), and there is no
  `ResourceLimiter` or `StoreLimits`.
- **The gap: one store per thread, shared by every WebAssembly grammar.**
  `runtime.rs` keeps one cached `Parser` per thread (`PARSER`, ~L641), and
  `ThreadParser::set_language` (~L697) gives it one `WasmStore`. Each module
  that store instantiates gets its static data at `current_memory_offset`,
  and that offset only ever grows (`wasm_store.c` ~L1366). Take a grammar that
  declares ~127 MiB of zeroed static data. It costs nothing in the `.wasm`
  file, so `MAX_GRAMMAR_BYTES` doesn't bound it. It loads, and from then on every other WebAssembly grammar parsed on that thread fails to
  instantiate until the app restarts. T1892 (`docs/specs/aawkvdvmb6-*`) hit
  the same limit with about 60 real grammars on one test thread. Its finish note says the
  app's long-lived parse threads can reach it too, which hasn't been measured.
- **Unchecked arithmetic.** `ts_wasm_store__heap_address(self) +
  dylink_info->memory_size` (~L1289) and `current_memory_offset +=
  dylink_info->memory_size` (~L1366) are `uint32_t` sums with no check, so a
  declared size near 4 GiB wraps.

## Do

1. **One store per grammar per thread.** In `runtime.rs`, cache one
   `Parser` with its own `WasmStore` per WebAssembly grammar per thread. Key
   it by the language's identity, for example the registered module file. Keep at most
   `WASM_PARSERS_PER_THREAD` of them and drop the least recently used. Pick N
   (around 8) and say why in its doc comment. Native languages keep the one
   cached parser they use today. Keep the semantics of `take` and drop: when the
   grammar's parser is already taken, make a new one and drop it afterwards.
   `LIVE_STORES` stays exact through `Cached`'s `Drop`.
2. **Choose the cap from measurement.** Build every first-party module
   (`scripts/build-grammar-wasm.sh`, all languages; ask the lead first, since
   it is about 2 minutes of clang plus the parity test). Read each module's
   `dylink.0` `memory_size`. A grammar needs about the stdlib's initial pages
   plus the lexer, plus its `memory_size`, plus the 4 MiB scanner heap, plus
   its stack. Take the smallest power-of-two MiB that is at least twice the
   largest need, and no more than 128 MiB. If that's below 128 MiB, set
   `MAX_MEMORY_SIZE` to it in `wasm_store.c`, marked `fastedit patch`.
   Record the five largest needs and the chosen cap in the decision file.
3. **Checked offsets.** In `wasm_store.c`'s module load, an overflowing
   `heap_address + memory_size`, or a total past the cap, is a load error
   (`fastedit patch`).
4. **Hostile fixture `grow`.** Build it as
   `crates/fe-syntax/tests/fixtures/hostile-grammars.md` documents (the same
   `grammar.js` shape as `loop`, built with `build.sh`). Its external scanner
   grows memory one page at a time until `__builtin_wasm_memory_grow` returns
   -1, writing a byte to each new page so the pages are really resident. It
   then returns token `capped` if `__builtin_wasm_memory_size(0)` is at most
   the cap in pages, else `uncapped`. Add its section and SHA-256 to
   `hostile-grammars.md`. Tools: wasi-sdk 34 is at
   `~/.cache/tree-sitter/wasi-sdk`. The tree-sitter CLI 0.27.0 release binary
   isn't on this Mac's PATH, so ask the lead before fetching it.
5. **Tests** go in a new file, `crates/fe-syntax/tests/wasm_memory_cap.rs`
   (`#![cfg(feature = "json")]`). Copy the helpers it needs from
   `runtime_grammar.rs`; don't share them. Use only APIs that exist at base,
   because the regression test runs there. Serialize the tests that read
   `wasm_stats()` with a file-level mutex, so they hold under `cargo test` as
   well as nextest.
   - `a_grammar_that_fills_its_store_leaves_other_grammars_parsing`
     (regression). Copy `tests/fixtures/tree-sitter-json.wasm` into a
     scratch dir and re-encode its `dylink.0` custom section, the way
     `wasm_table_validation.rs` patches modules. Set `memory_size` to
     128 MiB − `MEMORY_BASE` (131,100, as `wasm_table_validation.rs` has it)
     − 64 bytes, aligned down. Register that copy as `json_full`, with an
     extension of its own, and the unpatched fixture as `json_wasm`. On one
     freshly spawned thread, parse a `json_full` buffer, then a `json_wasm`
     buffer. Then `assert_eq!` `json_wasm`'s highlights against the
     compiled-in JSON's. If `json_full`'s registration fails, which a lower cap
     makes it do after the fix, check `json_wasm` alone. Use `assert_eq!`,
     not `assert!(c, "msg")`: at base, regatta counts only panics whose text
     starts with "assertion" as the test failing.
   - `a_grammar_growing_its_memory_stops_at_the_cap`: a `grow` buffer parses
     with a `capped` node and no `uncapped` node, and `json_wasm` still
     highlights on the same thread afterwards.
   - `an_overflowing_static_size_is_a_load_error`: the JSON copy with
     `memory_size` 0xFFFF_FF00 makes `register_language` return an error
     naming its file.
   - `a_thread_keeps_at_most_n_wasm_parsers`: N+1 copies of the JSON module,
     under distinct names and extensions and parsed in turn on one thread,
     raise `wasm_stats().stores` by at most N.
6. **Docs.** Update `vendor/tree-sitter/README.md`: "What changed", plus the
   memory claim, which is a cap of X per grammar per thread, so a hostile
   grammar holds at most X × the number of threads that parse WebAssembly.
   Count those threads in the app and give the number. Add one memory sentence
   to the "Met by T845 and T230" paragraph of
   `docs/decisions/vmtnmkay62-extensions-and-plugins.md` §6.

## Scope and requirements

- Change only the files in scope. Leave `wasm_parity.rs`'s thread-per-language
  workaround (~L409) alone, and say in the finish whether it is still needed.
- `runtime_grammar.rs` and `wasm_table_validation.rs` stay unchanged and keep
  passing, which also confirms that `MEMORY_BASE` didn't move.
- If the PERF.md memory-report wording about stores ("count only") needs to
  change now that stores are per grammar, say so in the finish rather than
  editing PERF.md: inbox cards T409 and T410 hold it.
- Headless; plain cargo tests, so any host can run them.

## Helium note (lead, 2026-11-27)

Helium task T662 holds `docs/decisions/vmtnmkay62-extensions-and-plugins.md` in an open lane, so that file is out of this card's scope here. Leave it alone. Instead, put the one memory sentence for §6's "Met by T845 and T230" paragraph in your finish notes, as text ready to paste, so a follow-up can add it once T662 lands. C6 now covers only `vendor/tree-sitter/README.md`.

## Implementation notes

- **Cache.** `runtime.rs` keeps, per thread, one native `Parser` and up to
  `WASM_PARSERS_PER_THREAD` (8) parsers each with its own `WasmStore`, least
  recently used first. The key is the `Language`'s address: a registered
  language is leaked, so it's unique for the process's life, and a name
  registered again with new files is a new language with a new store. A
  `ThreadParser` set from one language to another gives its parser back and
  takes the new language's (the injection layers reuse one `ThreadParser`).
  A parser taken twice is made anew, and the second given back for a key the
  cache already holds is dropped. `LIVE_STORES` counts in `take_wasm` and
  `Cached`'s `Drop`.
- **Cap, from measurement** (`scripts/build-grammar-wasm.sh`, all 88 modules,
  on helium with the lead's go-ahead). Need = `dylink.0` `memory_size` +
  131,100 (stdlib's 2 pages with the stack, and the lexer) + 1,024
  (serialization buffer) + 4 MiB (scanner heap). Five largest: verilog
  21.28 MiB (17.16 static), fsharp 14.87 (10.74), razor 14.13 (10.00), nim
  12.09 (7.97), cuda 10.70 (6.57). Twice the largest is 42.6 MiB, so
  `MAX_MEMORY_SIZE` is 64 MiB. The decision file is this task's own,
  `docs/decisions/aqqz9w5mhj-wasm-grammar-memory-cap.md` (scope added after
  review round 1), and `vendor/tree-sitter/README.md` (Memory) repeats the
  table.
- **C1's filler.** 128 MiB − `MEMORY_BASE` − 64 bytes would not load even
  at base: the store puts the 1,024-byte serialization buffer between a
  module's statics and the heap, so the load needs 960 bytes past the cap,
  and the test would pass at base. The filler also leaves out the buffer:
  `(cap − 131,100 − 1,024 − 64) & !15`. A second filler sized for 64 MiB
  loads under the new cap, so after the fix the test still parses a full
  store before `json_wasm`, not `json_wasm` alone.
- **`WASM_PARSERS_PER_THREAD`** is `pub(crate)` (lib.rs is out of scope),
  so `wasm_memory_cap.rs` keeps its own copy of 8. The regression test has
  to compile at base anyway.
- **`grow`** was generated with `tree-sitter generate --js-runtime native`,
  since helium has no `node`, and built by `hostile-grammars.md`'s
  `build.sh` (wasi-sdk 34).

### Finish notes

- **For `docs/decisions/vmtnmkay62-extensions-and-plugins.md` §6** ("Met by
  T845 and T230" paragraph), once T662 lands, ready to paste: "A grammar's
  memory is capped as well: each WebAssembly grammar gets a store of its own
  on each thread that parses it, capped at 64 MiB for its static data and
  its own `memory.grow` alike, so a hostile grammar can't fill the memory
  other grammars parse in, and it holds 64 MiB per thread that parses it, or
  that times its nesting while a parse nests it inside itself (T759;
  `vendor/tree-sitter/README.md`, Memory)."
- **`wasm_parity.rs`'s thread per language** (~L409) is no longer needed for
  memory: one thread parsing every language now caches at most 8 stores
  between parses, one per grammar, each with its own 64 MiB. It still keeps a panicking
  language from failing the others' checks, so it's worth keeping, with its
  comment changed to say so (not in scope here).
- **PERF.md's memory report** ("counts only") is still right. The count
  means more now, though: a store is one grammar on one thread, at most 8 a
  thread cached between parses (plus the parsers in use), each up to 64 MiB. That's worth a clause when T409/T410 next edit
  it.
- **C1's filler differs from the spec (the lead agreed).** The store
  instantiates a module only if `heap_address + memory_size` fits the cap,
  where `heap_address` is the offset (131,100 in a fresh store) plus the
  1,024-byte serialization buffer (`TREE_SITTER_SERIALIZATION_BUFFER_SIZE`).
  The spec's size, 128 MiB − 131,100 − 64 aligned down to 16, is
  134,086,560, and it needs 131,100 + 1,024 + 134,086,560 = 134,218,684
  bytes. That's 956 past the 134,217,728 cap, so `json_full` wouldn't
  register even at base. The test would then check `json_wasm` alone and
  pass at base, which a regression test mustn't do. The filler is
  (cap − 131,100 − 1,024 − 64) & !15 = 134,085,536 instead. It needs
  134,217,660, so 68 bytes are left, fewer than `json_wasm`'s
  1,024 + 2,928, and at base `json_wasm` then gets no tree (verified:
  `left: (false, [])`). A second filler, sized for 64 MiB
  ((64 MiB − 131,100 − 1,024 − 64) & !15 = 66,976,672, leaving 68 bytes),
  loads under the new cap. So after the fix the test still parses a full
  store before `json_wasm`, and doesn't fall back to `json_wasm` alone.
  The test also asserts that one filler loads, and that each filler
  highlights as the compiled-in JSON does.
- **Review round 1 (codex ae6f8t).** F2: the cap's measurement is in a
  decision file of its own, `docs/decisions/aqqz9w5mhj-wasm-grammar-memory-cap.md`,
  added to the scope by respec (the lead agreed). F1: the memory bound now
  reads per store, not per cached parser. `WASM_PARSERS_PER_THREAD` (8)
  limits what a thread keeps between parses, not the stores alive at once.
  A thread holds at most 64 MiB × (8 + the parsers it has in use), and a
  grammar holds at most 64 MiB × the parsers taking it at once on a thread,
  which is more than one only while a parse nests it inside itself. So a
  hostile grammar holds 64 MiB per thread that parses WebAssembly (about
  1.5 × cores + 7 on Linux), times its nesting while a nested parse runs.
