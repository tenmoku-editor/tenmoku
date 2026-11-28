//! Built-in grammars against the same grammars as WebAssembly.
//!
//! For each language, built in and as wasm, on the golden sample
//! (`tests/golden/<language>.*`) and on a corpus of it repeated to about 5,000
//! lines (generated here, not committed): a full parse, a full highlight, and
//! one incremental reparse after a single-character edit in the middle of the
//! corpus. The WebAssembly grammar's load cost (wasmtime compile and
//! instantiate, plus its queries) is timed once more, apart.
//!
//! - Kotlin, the first, is the WebAssembly extension
//!   `target/grammar-wasm/tree-sitter-kotlin.wasm`, which
//!   `scripts/build-grammar-wasm.sh kotlin` builds, loaded as a user grammar
//!   is, with the queries of `extensions/grammars/kotlin/`. Its built-in is
//!   compiled only with `--features kotlin`; without it, Kotlin is left out.
//! - The others are the built-ins with a C external scanner, built from the
//!   source the app links into `target/grammar-wasm-native/` by
//!   `GRAMMAR_ROOT=crates/fe-syntax/benches/native-as-wasm
//!   scripts/build-grammar-wasm.sh`. Each module is registered as
//!   `<language>_wasm`, with no extensions, and given the highlight and
//!   injection queries the built-in has (as `DEFS` in `src/language.rs`
//!   composes them), written to a temporary directory.
//!
//! Before timing a language, its wasm highlights of the golden sample must
//! equal the built-in's; if they don't, it's reported as not comparable and
//! not timed (Kotlin, whose queries are its extension's, is only reported). A module that's missing or won't load is reported, and the bench
//! moves on. `markdown_inline` parses the inline text the block grammar hands
//! it, so its "golden sample" is the text of each `inline` node of
//! `tests/golden/markdown.md`, joined by blank lines.
//!
//! Run with `cargo bench -p fe-syntax --bench wasm` (add `--features kotlin`
//! for Kotlin); `WASM_BENCH_LANGUAGES=rust,yaml` runs only those. The settings
//! are criterion's quick ones: a small sample, a short measurement.

use std::{
    env, fs,
    hint::black_box,
    path::{Path, PathBuf},
    process::exit,
    time::{Duration, Instant},
};

use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};
use fe_buffer::{Buffer, Origin};
use fe_syntax::{
    GrammarSpec, Language, Syntax, language_by_name, register_language, unregister_language,
};

const SCRIPT: &str = "scripts/build-grammar-wasm.sh";
const NATIVE_ROOT: &str = "crates/fe-syntax/benches/native-as-wasm";
const LARGE_LINES: usize = 5_000;

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A built-in's queries as `DEFS` composes them: the highlight query's parts,
/// in order, the injection query, and whether earlier patterns win.
struct Queries {
    highlights: &'static [&'static str],
    injections: Option<&'static str>,
    earlier_patterns_win: bool,
}

/// The built-ins with a C external scanner, in `DEFS`'s terms. Kept in step
/// with `src/language.rs` by hand; the highlight check before timing catches
/// a drift.
fn built_in_queries(language: &str) -> Option<Queries> {
    // Each arm is behind its feature, so with none on only `_` is left.
    let found: Option<(&'static [&'static str], Option<&'static str>)> = match language {
        #[cfg(feature = "bash")]
        "bash" => Some((
            &[
                tree_sitter_bash::HIGHLIGHT_QUERY,
                "[\"||\" \"&\"] @operator\n\
                 ((concatenation) @variable\n\
                 (#match? @variable \"^[{][A-Za-z_][A-Za-z0-9_]*([:]?[-=+?][^{}]*)?[}]$\"))",
            ],
            None,
        )),
        #[cfg(feature = "css")]
        "css" => Some((&[tree_sitter_css::HIGHLIGHTS_QUERY], None)),
        #[cfg(feature = "dockerfile")]
        "dockerfile" => Some((
            &[tree_sitter_containerfile::HIGHLIGHTS_QUERY],
            Some(tree_sitter_containerfile::INJECTIONS_QUERY),
        )),
        #[cfg(feature = "html")]
        "html" => Some((
            &[tree_sitter_html::HIGHLIGHTS_QUERY],
            Some(tree_sitter_html::INJECTIONS_QUERY),
        )),
        #[cfg(feature = "javascript")]
        "javascript" => Some((
            &[
                tree_sitter_javascript::HIGHLIGHT_QUERY,
                tree_sitter_javascript::JSX_HIGHLIGHT_QUERY,
                include_str!("../queries/javascript.scm"),
            ],
            Some(tree_sitter_javascript::INJECTIONS_QUERY),
        )),
        #[cfg(feature = "just")]
        "just" => Some((
            &[tree_sitter_just::HIGHLIGHTS_QUERY],
            Some(tree_sitter_just::INJECTIONS_QUERY),
        )),
        #[cfg(feature = "lua")]
        "lua" => Some((
            &[
                tree_sitter_lua::HIGHLIGHTS_QUERY,
                include_str!("../queries/lua.scm"),
            ],
            Some(tree_sitter_lua::INJECTIONS_QUERY),
        )),
        #[cfg(feature = "markdown")]
        "markdown" => Some((
            &[
                tree_sitter_md::HIGHLIGHT_QUERY_BLOCK,
                include_str!("../queries/markdown.scm"),
            ],
            Some(tree_sitter_md::INJECTION_QUERY_BLOCK),
        )),
        #[cfg(feature = "markdown")]
        "markdown_inline" => Some((
            &[
                tree_sitter_md::HIGHLIGHT_QUERY_INLINE,
                include_str!("../queries/markdown-inline.scm"),
            ],
            Some(tree_sitter_md::INJECTION_QUERY_INLINE),
        )),
        #[cfg(feature = "python")]
        "python" => Some((
            &[
                tree_sitter_python::HIGHLIGHTS_QUERY,
                include_str!("../queries/python.scm"),
            ],
            Some(include_str!("../queries/python-injections.scm")),
        )),
        #[cfg(feature = "rust")]
        "rust" => Some((
            &[
                tree_sitter_rust::HIGHLIGHTS_QUERY,
                include_str!("../queries/rust.scm"),
            ],
            Some(include_str!("../queries/rust-injections.scm")),
        )),
        #[cfg(feature = "toml")]
        "toml" => Some((&[include_str!("../queries/toml.scm")], None)),
        #[cfg(feature = "typescript")]
        "typescript" => Some((
            &[
                tree_sitter_javascript::HIGHLIGHT_QUERY,
                tree_sitter_typescript::HIGHLIGHTS_QUERY,
                include_str!("../queries/javascript.scm"),
            ],
            Some(tree_sitter_javascript::INJECTIONS_QUERY),
        )),
        #[cfg(feature = "typescript")]
        "tsx" => Some((
            &[
                tree_sitter_javascript::HIGHLIGHT_QUERY,
                tree_sitter_javascript::JSX_HIGHLIGHT_QUERY,
                tree_sitter_typescript::HIGHLIGHTS_QUERY,
                include_str!("../queries/javascript.scm"),
            ],
            Some(tree_sitter_javascript::INJECTIONS_QUERY),
        )),
        #[cfg(feature = "xml")]
        "xml" => Some((
            &[
                tree_sitter_xml::XML_HIGHLIGHT_QUERY,
                include_str!("../queries/xml.scm"),
            ],
            None,
        )),
        #[cfg(feature = "yaml")]
        "yaml" => Some((
            &[
                tree_sitter_yaml::HIGHLIGHTS_QUERY,
                include_str!("../queries/yaml.scm"),
            ],
            None,
        )),
        _ => None,
    };
    found.map(|(highlights, injections)| Queries {
        highlights,
        injections,
        // CSS's query is one of the older ones written the other way round.
        earlier_patterns_win: language == "css",
    })
}

/// Every language the bench knows, Kotlin first.
const LANGUAGES: &[&str] = &[
    "kotlin",
    "bash",
    "css",
    "dockerfile",
    "html",
    "javascript",
    "just",
    "lua",
    "markdown",
    "markdown_inline",
    "python",
    "rust",
    "toml",
    "typescript",
    "tsx",
    "xml",
    "yaml",
];

/// The languages `WASM_BENCH_LANGUAGES` names, or all of them.
fn selected() -> Vec<&'static str> {
    let Some(list) = env::var("WASM_BENCH_LANGUAGES")
        .ok()
        .filter(|list| !list.trim().is_empty())
    else {
        return LANGUAGES.to_vec();
    };
    list.split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(|name| {
            LANGUAGES
                .iter()
                .find(|known| **known == name)
                .copied()
                .unwrap_or_else(|| {
                    eprintln!(
                        "WASM_BENCH_LANGUAGES names {name}, which the bench doesn't know; it knows {}",
                        LANGUAGES.join(", ")
                    );
                    exit(1);
                })
        })
        .collect()
}

/// The string `key = "..."` of one of our one-key-a-line TOML files.
fn field(file: &Path, key: &str) -> Option<String> {
    let text = fs::read_to_string(file).ok()?;
    text.lines().find_map(|line| {
        let (name, value) = line.split_once('=')?;
        (name.trim() == key).then(|| value.trim().trim_matches('"').to_owned())
    })
}

/// Kotlin's spec, as the bench has always loaded it.
fn kotlin_spec() -> Result<GrammarSpec, String> {
    let wasm = repository().join("target/grammar-wasm/tree-sitter-kotlin.wasm");
    if !wasm.is_file() {
        return Err(format!(
            "{} doesn't exist: build it with {SCRIPT} kotlin",
            wasm.display()
        ));
    }
    let dir = repository().join("extensions/grammars/kotlin");
    Ok(GrammarSpec {
        language: "kotlin_wasm".to_owned(),
        wasm,
        highlights: Some(dir.join("highlights.scm")),
        injections: Some(dir.join("injections.scm")),
        earlier_patterns_win: false,
        ..GrammarSpec::default()
    })
}

/// A built-in's module from `target/grammar-wasm-native/`, with its queries
/// written into `queries`.
fn native_as_wasm_spec(language: &str, queries: &Path) -> Result<GrammarSpec, String> {
    let source = repository()
        .join(NATIVE_ROOT)
        .join(language)
        .join("grammar-source.toml");
    let grammar =
        field(&source, "grammar").ok_or_else(|| format!("{} has no grammar", source.display()))?;
    let wasm = repository().join(format!(
        "target/grammar-wasm-native/tree-sitter-{}.wasm",
        grammar.replace('_', "-")
    ));
    if !wasm.is_file() {
        return Err(format!(
            "{} doesn't exist: build it with GRAMMAR_ROOT={NATIVE_ROOT} {SCRIPT} {language}",
            wasm.display()
        ));
    }
    let def = built_in_queries(language)
        .ok_or_else(|| format!("{language} isn't compiled into this build"))?;
    let dir = queries.join(language);
    fs::create_dir(&dir).map_err(|error| format!("{}: {error}", dir.display()))?;
    let highlights = dir.join("highlights.scm");
    fs::write(&highlights, def.highlights.concat())
        .map_err(|error| format!("{}: {error}", highlights.display()))?;
    let injections = def
        .injections
        .map(|text| {
            let file = dir.join("injections.scm");
            fs::write(&file, text).map_err(|error| format!("{}: {error}", file.display()))?;
            Ok::<_, String>(file)
        })
        .transpose()?;
    Ok(GrammarSpec {
        language: format!("{language}_wasm"),
        wasm,
        highlights: Some(highlights),
        injections,
        earlier_patterns_win: def.earlier_patterns_win,
        ..GrammarSpec::default()
    })
}

/// `tests/golden/<language>.*`, the sample its golden file is of.
fn golden_sample(language: &str) -> Result<String, String> {
    let dir = repository().join("crates/fe-syntax/tests/golden");
    let entries = fs::read_dir(&dir).map_err(|error| format!("{}: {error}", dir.display()))?;
    let path = entries
        .filter_map(|entry| Some(entry.ok()?.path()))
        .find(|path| {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("");
            !name.ends_with(".golden")
                && (name == language
                    || name.split_once('.').map(|(stem, _)| stem) == Some(language))
        })
        .ok_or_else(|| format!("no golden sample for {language} in {}", dir.display()))?;
    fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))
}

/// The text of each `inline` node of the Markdown sample, joined by blank
/// lines: what `markdown_inline` is given in the editor, one node at a time.
fn inline_text(markdown: &'static Language) -> Result<String, String> {
    let sample = golden_sample("markdown")?;
    let buffer = Buffer::new(&sample);
    let mut syntax = Syntax::new(markdown, buffer.current());
    syntax.finish(syntax.parse_job().run());
    let tree = syntax.tree().ok_or("the Markdown sample doesn't parse")?;
    let mut inlines = Vec::new();
    let mut cursor = tree.walk();
    'walk: loop {
        let node = cursor.node();
        if node.kind() == "inline" {
            inlines.push(&sample[node.byte_range()]);
        } else if cursor.goto_first_child() {
            continue;
        }
        while !cursor.goto_next_sibling() {
            if !cursor.goto_parent() {
                break 'walk;
            }
        }
    }
    Ok(inlines.join("\n\n") + "\n")
}

/// Kotlin's sample repeated whole to `LARGE_LINES` lines. Later copies leave
/// out the package and imports, which may only open a file.
fn large_kotlin(sample: &str) -> String {
    let body: String = sample
        .lines()
        .filter(|line| !line.starts_with("package ") && !line.starts_with("import "))
        .map(|line| format!("{line}\n"))
        .collect();
    repeated(sample, &body, sample.len())
}

/// Another language's sample repeated to `LARGE_LINES` lines. Later copies
/// leave out a first `#!` line, which only a file's first line may be. XML
/// may have one root element, so its later copies are the root's content,
/// put before the root's end tag.
fn large(language: &str, sample: &str) -> String {
    if language == "xml"
        && let Some((start, end)) = xml_root_content(sample)
    {
        return repeated(sample, &sample[start..end], end);
    }
    let body = match sample.split_once('\n') {
        Some((first, rest)) if first.starts_with("#!") => rest,
        _ => sample,
    };
    repeated(sample, body, sample.len())
}

/// Where the content of the sample's root element starts and ends: after the
/// first start tag at the start of a line, and before the last end tag.
fn xml_root_content(sample: &str) -> Option<(usize, usize)> {
    let root = sample
        .match_indices("\n<")
        .map(|(at, _)| at + 1)
        .find(|&at| sample[at + 1..].starts_with(|c: char| c.is_ascii_alphabetic()))?;
    let start = root + sample[root..].find('>')? + 1;
    let end = sample.rfind("</")?;
    (start < end).then_some((start, end))
}

/// `sample` with copies of `body` inserted at `at` until it has more than
/// `LARGE_LINES` lines.
fn repeated(sample: &str, body: &str, at: usize) -> String {
    let lines = |text: &str| text.matches('\n').count();
    let mut copies = 0;
    while lines(sample) + (copies + 1) * lines(body) <= LARGE_LINES {
        copies += 1;
    }
    let mut text = sample[..at].to_owned();
    for _ in 0..copies {
        text.push_str(body);
    }
    text.push_str(&sample[at..]);
    text
}

/// The offset of a letter in the middle of the text, where a character goes.
fn middle(text: &str) -> usize {
    let half = text.len() / 2;
    half + text[half..]
        .find(|c: char| c.is_ascii_lowercase())
        .expect("a letter follows the middle")
}

/// The language's highlights of `text`, the injected languages compiled.
fn highlights(language: &'static Language, text: &str) -> fe_syntax::Highlights {
    let buffer = Buffer::new(text);
    let mut syntax = Syntax::new(language, buffer.current());
    syntax.finish(syntax.parse_job().run());
    loop {
        let highlights = syntax.checked_highlights(0..text.len());
        let missing = syntax.take_missing_languages();
        if missing.is_empty() {
            return highlights;
        }
        for name in missing {
            language_by_name(name);
        }
    }
}

/// Why the wasm language's highlights of `text` aren't the built-in's, if
/// they aren't.
fn difference(native: &'static Language, wasm: &'static Language, text: &str) -> Option<String> {
    let (expected, actual) = (highlights(native, text), highlights(wasm, text));
    if !expected.complete || !actual.complete {
        return Some(format!(
            "highlights incomplete (native {}, wasm {})",
            expected.complete, actual.complete
        ));
    }
    let describe = |span: Option<&fe_syntax::HighlightSpan>| match span {
        Some(span) => format!(
            "{:?} {} {:?}",
            span.range,
            span.highlight.name(),
            &text[span.range.clone()]
        ),
        None => "nothing".to_owned(),
    };
    for (what, expected, actual) in [
        ("span", &expected.spans, &actual.spans),
        ("markup span", &expected.markup, &actual.markup),
    ] {
        if expected != actual {
            let at = expected
                .iter()
                .zip(actual.iter())
                .position(|(a, b)| a != b)
                .unwrap_or(expected.len().min(actual.len()));
            return Some(format!(
                "{what} {at} differs: native has {}, wasm {}",
                describe(expected.get(at)),
                describe(actual.get(at))
            ));
        }
    }
    None
}

/// What became of a language that wasn't timed.
fn skip(language: &str, why: &str) {
    eprintln!("wasm bench: {language} not timed: {why}");
}

/// Times one language: the load, then each case on both engines.
fn bench_language(c: &mut Criterion, language: &'static str, queries: &Path) {
    let Some(native) = language_by_name(language) else {
        let hint = if language == "kotlin" {
            " (add --features kotlin)"
        } else {
            ""
        };
        return skip(language, &format!("not compiled into this build{hint}"));
    };
    let spec = if language == "kotlin" {
        kotlin_spec()
    } else {
        native_as_wasm_spec(language, queries)
    };
    let spec = match spec {
        Ok(spec) => spec,
        Err(error) => return skip(language, &format!("no module: {error}")),
    };
    unregister_language(&spec.language);
    let wasm: &'static Language = match register_language(&spec) {
        Ok(wasm) => wasm,
        Err(error) => return skip(language, &format!("the module doesn't load: {error}")),
    };

    let sample = if language == "markdown_inline" {
        inline_text(language_by_name("markdown").expect("markdown is compiled in"))
    } else {
        golden_sample(language)
    };
    let sample = match sample {
        Ok(sample) => sample,
        Err(error) => return skip(language, &error),
    };
    match difference(native, wasm, &sample) {
        // Kotlin's wasm queries are its extension's, not the built-in's, so
        // a difference there is reported and its cases run as they always
        // have.
        Some(why) if language == "kotlin" => {
            eprintln!("wasm bench: kotlin: on the golden sample, {why}")
        }
        Some(why) => {
            return skip(
                language,
                &format!("not comparable: on the golden sample, {why}"),
            );
        }
        None => {
            eprintln!("wasm bench: {language}: wasm highlights equal native on the golden sample")
        }
    }

    // Compile and instantiate again each time: unregistering drops the cached
    // grammar, so the next registration loads the module.
    let mut group = c.benchmark_group("wasm load");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(3));
    group.warm_up_time(Duration::from_secs(1));
    group.bench_function(language, |b| {
        b.iter(|| {
            unregister_language(&spec.language);
            black_box(register_language(&spec).expect("the module loads"))
        })
    });
    group.finish();
    unregister_language(&spec.language);
    let wasm: &'static Language = register_language(&spec).expect("the module loads");

    let large = if language == "kotlin" {
        large_kotlin(&sample)
    } else {
        large(language, &sample)
    };
    let corpora = [("golden sample", sample), ("5k lines", large)];
    let mut group = c.benchmark_group(language);
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(2));
    group.warm_up_time(Duration::from_secs(1));

    for (size, text) in &corpora {
        eprintln!(
            "{language} {size}: {} bytes, {} lines",
            text.len(),
            text.matches('\n').count()
        );
        for (engine, language_of) in [("native", native), ("wasm", wasm)] {
            let id = |what: &str| BenchmarkId::new(format!("{what} {engine}"), size);
            let mut buffer = Buffer::new(text);
            let mut syntax = Syntax::new(language_of, &buffer.snapshot());

            group.bench_function(id("parse"), |b| b.iter(|| syntax.parse_job().run()));
            syntax.finish(syntax.parse_job().run());
            let tree = syntax.tree().expect("the text parses");
            if language == "kotlin" {
                assert!(
                    !tree.root_node().has_error(),
                    "kotlin {size} parses without errors"
                );
            } else if tree.root_node().has_error() {
                eprintln!("wasm bench: {language} {size} parses with errors ({engine})");
            }

            // Highlights are cached by range, so each run gets a parsed
            // `Syntax` of its own, made untimed.
            group.bench_function(id("highlight"), |b| {
                b.iter_batched_ref(
                    || {
                        let mut fresh = Syntax::new(language_of, &buffer.snapshot());
                        fresh.finish(fresh.parse_job().run());
                        fresh
                    },
                    |fresh| black_box(fresh.highlights(0..text.len())),
                    BatchSize::PerIteration,
                )
            });

            if *size == "5k lines" {
                // The edit is undone, untimed, so errors don't pile up.
                let offset = middle(text);
                group.bench_function(id("reparse one character"), |b| {
                    let reparse = |buffer: &Buffer, syntax: &mut Syntax| {
                        syntax.edit(&buffer.snapshot());
                        let started = Instant::now();
                        assert!(syntax.parse_within(Duration::from_secs(60)));
                        started.elapsed()
                    };
                    b.iter_custom(|iters| {
                        let mut total = Duration::ZERO;
                        for _ in 0..iters {
                            buffer.edit([(offset..offset, "x")], Origin::User);
                            total += reparse(&buffer, &mut syntax);
                            buffer.edit([(offset..offset + 1, "")], Origin::User);
                            reparse(&buffer, &mut syntax);
                        }
                        total
                    })
                });
            }
        }
    }
    group.finish();
}

fn benches(c: &mut Criterion) {
    let languages = selected();
    // The queries the built-ins' modules are given; `create_dir` fails rather
    // than reuse a directory that's there.
    let queries = env::temp_dir().join(format!("fe-syntax-wasm-bench-{}", std::process::id()));
    if let Err(error) = fs::create_dir(&queries) {
        eprintln!("{}: {error}", queries.display());
        exit(1);
    }
    for language in languages {
        bench_language(c, language, &queries);
    }
    let _ = fs::remove_dir_all(&queries);
}

criterion_group!(wasm, benches);
criterion_main!(wasm);
