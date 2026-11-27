//! A WebAssembly grammar's memory is capped, grammar by grammar: each gets
//! a store of its own on each thread, so one that fills its store, with
//! static data or by growing its memory, leaves the others parsing; a static
//! size past the cap, or one that would wrap the address space, is a load
//! error naming the module; and a thread keeps a bounded number of stores.
//!
//! These use only APIs older than the per-grammar stores. The helpers are
//! this file's own, like `runtime_grammar.rs`'s but not shared with it.

#![cfg(feature = "json")]

use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard, Once, OnceLock, PoisonError},
    time::Duration,
};

use fe_buffer::Buffer;
use fe_syntax::{
    GrammarDeadlines, GrammarError, GrammarSpec, HighlightSpan, Language, Syntax, language_by_name,
    parse_str, register_language, set_grammar_deadlines, wasm_stats,
};

/// Where the language's data segment lands in a fresh store's memory, as
/// `wasm_table_validation.rs` has it: the stdlib's two pages and the lexer.
const MEMORY_BASE: u32 = 131_100;
/// The bytes a store keeps between a module's static data and the heap:
/// tree-sitter's `TREE_SITTER_SERIALIZATION_BUFFER_SIZE`.
const SERIALIZATION_BUFFER: u32 = 1024;
/// A static size that, with what the store puts before and after it, fills
/// a store of `cap` bytes but for 64, aligned down to 16.
const fn fills_a_store(cap: u32) -> u32 {
    (cap - MEMORY_BASE - SERIALIZATION_BUFFER - 64) & !15
}
/// `WASM_PARSERS_PER_THREAD` in `src/runtime.rs`, which isn't public.
const WASM_PARSERS_PER_THREAD: usize = 8;

fn fixture() -> PathBuf {
    fixture_file("tree-sitter-json.wasm")
}

fn fixture_file(name: &str) -> PathBuf {
    // Read at run time: a path compiled in can name a deleted snapshot.
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR").expect("run by cargo");
    Path::new(&manifest).join("tests/fixtures").join(name)
}

/// Deadlines long enough for the JSON fixture's load on a loaded machine.
fn short_deadlines() {
    static SET: Once = Once::new();
    SET.call_once(|| {
        set_grammar_deadlines(GrammarDeadlines {
            load: Duration::from_secs(10),
            parse: Duration::from_millis(1500),
        });
    });
}

/// Held by every test here: [`wasm_stats`] counts stores across threads,
/// so another test's parses would move it. A file-level lock holds under
/// `cargo test`, which runs a file's tests in one process, as under nextest.
fn serial() -> MutexGuard<'static, ()> {
    static SERIAL: Mutex<()> = Mutex::new(());
    SERIAL.lock().unwrap_or_else(PoisonError::into_inner)
}

/// This process's directory `name` for patched modules and queries, made
/// empty.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join(format!("fe-syntax-wasm-memory-cap-{}", std::process::id()))
        .join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn highlights_query() -> PathBuf {
    static QUERY: OnceLock<PathBuf> = OnceLock::new();
    QUERY
        .get_or_init(|| {
            let path = scratch("query").join("highlights.scm");
            fs::write(&path, tree_sitter_json::HIGHLIGHTS_QUERY).unwrap();
            path
        })
        .clone()
}

/// The JSON module at `wasm`, registered as `name` for files ending `.name`.
fn json_spec(name: &str, wasm: PathBuf) -> GrammarSpec {
    GrammarSpec {
        language: name.into(),
        wasm,
        extensions: vec![name.into()],
        highlights: Some(highlights_query()),
        earlier_patterns_win: true,
        ..GrammarSpec::default()
    }
}

fn json_wasm() -> &'static Language {
    static LANGUAGE: OnceLock<&'static Language> = OnceLock::new();
    short_deadlines();
    LANGUAGE.get_or_init(|| register_language(&json_spec("json_wasm", fixture())).unwrap())
}

fn json() -> &'static Language {
    language_by_name("json").unwrap()
}

const TEXT: &str = r#"{
  "name": "fastedit",
  "numbers": [1, -2.5e3, 0],
  "flags": { "on": true, "off": false, "none": null },
  "escaped": "a\"b\n"
}
"#;

/// Whether `TEXT` parsed in `language` on this thread, and its highlights.
fn parsed_here(language: &'static Language) -> (bool, Vec<HighlightSpan>) {
    let buffer = Buffer::new(TEXT);
    let mut syntax = Syntax::new(language, &buffer.snapshot());
    let parsed = syntax.parse_within(Duration::from_secs(10));
    (parsed, syntax.highlights(0..TEXT.len()))
}

/// `f`'s result on a thread of its own, so with no parsers kept yet; the
/// thread has ended, its parsers gone with it, when this returns.
fn on_a_fresh_thread<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::spawn(f).join().unwrap()
}

fn leb(bytes: &[u8], at: &mut usize) -> u32 {
    let (mut value, mut shift) = (0u32, 0);
    loop {
        let byte = bytes[*at];
        *at += 1;
        value |= u32::from(byte & 0x7f) << shift;
        shift += 7;
        if byte & 0x80 == 0 {
            return value;
        }
    }
}

fn put_leb(out: &mut Vec<u8>, mut value: u32) {
    loop {
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}

/// `module` with its `dylink.0` section's memory size, the static data the
/// store makes room for, set to `memory_size`: the section, the first after
/// the 8-byte header, re-encoded, the rest as it was.
fn with_memory_size(module: &[u8], memory_size: u32) -> Vec<u8> {
    let mut at = 8;
    assert_eq!(module[at], 0, "a custom section first");
    at += 1;
    let size = leb(module, &mut at) as usize;
    let end = at + size;
    let name_len = leb(module, &mut at) as usize;
    assert_eq!(&module[at..at + name_len], b"dylink.0");
    at += name_len;

    let mut body = Vec::new();
    put_leb(&mut body, name_len as u32);
    body.extend_from_slice(b"dylink.0");
    let mut patched = false;
    while at < end {
        let kind = module[at];
        at += 1;
        let len = leb(module, &mut at) as usize;
        let mut payload = module[at..at + len].to_vec();
        at += len;
        // WASM_DYLINK_MEM_INFO: memory size, memory alignment, table size,
        // table alignment.
        if kind == 1 {
            let mut p = 0;
            leb(&payload, &mut p);
            let mut info = Vec::new();
            put_leb(&mut info, memory_size);
            info.extend_from_slice(&payload[p..]);
            payload = info;
            patched = true;
        }
        body.push(kind);
        put_leb(&mut body, payload.len() as u32);
        body.extend_from_slice(&payload);
    }
    assert!(patched, "the module has memory info");

    let mut out = module[..8].to_vec();
    out.push(0);
    put_leb(&mut out, body.len() as u32);
    out.extend_from_slice(&body);
    out.extend_from_slice(&module[end..]);
    out
}

/// A copy of the JSON fixture with a static size of `memory_size`, in
/// scratch directory `dir`, under the fixture's own file name.
fn json_with_memory_size(dir: &str, memory_size: u32) -> PathBuf {
    let path = scratch(dir).join("tree-sitter-json.wasm");
    let module = fs::read(fixture()).unwrap();
    fs::write(&path, with_memory_size(&module, memory_size)).unwrap();
    path
}

/// Grammars whose static data fills a store, of the 128 MiB cap the store
/// had and of the 64 MiB it has, are parsed first on a thread; the JSON
/// grammar still highlights after them there, and so do they. With one store
/// a thread, the first left no room for the others to instantiate in. A
/// filler past the cap doesn't load, and isn't parsed.
#[test]
fn a_grammar_that_fills_its_store_leaves_other_grammars_parsing() {
    let _serial = serial();
    let wasm = json_wasm();
    let fillers: Vec<&'static Language> = [("json_full", 128), ("json_full64", 64)]
        .into_iter()
        .filter_map(|(name, mib)| {
            let module = json_with_memory_size(name, fills_a_store(mib << 20));
            register_language(&json_spec(name, module)).ok()
        })
        .collect();
    let count = fillers.len();
    let (filled, expected, after) = on_a_fresh_thread(move || {
        let filled: Vec<_> = fillers.into_iter().map(parsed_here).collect();
        (filled, parsed_here(json()), parsed_here(wasm))
    });
    assert!(expected.0 && expected.1.len() > 10, "{expected:?}");
    assert_eq!(after, expected);
    assert!(count > 0, "a filler loads");
    assert_eq!(filled, vec![expected; count]);
}

/// `grow`'s scanner grows its memory a page at a time until refused, then
/// finds `capped` if it stayed within the cap; the JSON grammar still
/// highlights on that thread after.
#[test]
fn a_grammar_growing_its_memory_stops_at_the_cap() {
    let _serial = serial();
    let wasm = json_wasm();
    let grow = register_language(&GrammarSpec {
        language: "grow".into(),
        wasm: fixture_file("tree-sitter-grow.wasm"),
        extensions: vec!["grow".into()],
        ..GrammarSpec::default()
    })
    .unwrap();
    let (tree, expected, after) = on_a_fresh_thread(move || {
        let tree = parse_str(grow, "abc ! def", || false).map(|tree| tree.root_node().to_sexp());
        (tree, parsed_here(json()), parsed_here(wasm))
    });
    let tree = tree.expect("parsed");
    assert!(
        tree.contains("(capped)") && !tree.contains("uncapped"),
        "{tree}"
    );
    assert!(expected.0 && expected.1.len() > 10, "{expected:?}");
    assert_eq!(after, expected);
}

/// A static size near 4 GiB would wrap the store's 32-bit offsets.
#[test]
fn an_overflowing_static_size_is_a_load_error() {
    let _serial = serial();
    short_deadlines();
    let path = json_with_memory_size("overflow", 0xFFFF_FF00);
    let result = register_language(&json_spec("json_overflow", path.clone()));
    let error = result.as_ref().err();
    assert_eq!(error.and_then(GrammarError::file), Some(path.as_path()));
    let message = error.map(ToString::to_string).unwrap_or_default();
    assert!(
        message.starts_with(&path.display().to_string()),
        "{message}"
    );
}

/// N+1 grammars parsed in turn on one thread keep at most N stores there,
/// each parsing as the compiled-in JSON does; they go with the thread.
#[test]
fn a_thread_keeps_at_most_n_wasm_parsers() {
    let _serial = serial();
    json_wasm();
    let languages: Vec<&'static Language> = (0..=WASM_PARSERS_PER_THREAD)
        .map(|i| register_language(&json_spec(&format!("json_copy{i}"), fixture())).unwrap())
        .collect();
    let before = wasm_stats().stores;
    let (expected, parsed, during) = on_a_fresh_thread(move || {
        let expected = parsed_here(json());
        // Natively parsed in between, as a file's host language is.
        let parsed: Vec<_> = (languages.iter())
            .map(|&language| (parsed_here(language), parsed_here(json())))
            .collect();
        (expected, parsed, wasm_stats().stores)
    });
    assert!(expected.0 && expected.1.len() > 10, "{expected:?}");
    for (wasm, native) in &parsed {
        assert_eq!(wasm, &expected);
        assert_eq!(native, &expected);
    }
    assert!(
        during > before && during <= before + WASM_PARSERS_PER_THREAD,
        "{before} stores, then {during}"
    );
    assert_eq!(wasm_stats().stores, before);
}
