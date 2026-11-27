//! Languages loaded at run time from WebAssembly grammars, such as those
//! extensions contribute ([`register_language`]), and the per-thread parsers
//! that parse them ([`ThreadParser`]).
//!
//! A module is compiled once, on a shared [`Engine`], into a language every
//! thread can use. Parsing it needs a [`WasmStore`] in the parser, which
//! instantiates the module, so each thread keeps a parser with a store for
//! each WebAssembly grammar it parsed lately, instead of making one per parse
//! ([`WASM_PARSERS_PER_THREAD`]). A store of its own holds a grammar to the
//! store's memory cap, whatever the others use.
//!
//! A grammar's code runs under a deadline ([`GrammarDeadlines`]): loading
//! it, and each parse. The engine checks it once an epoch, which a ticker
//! thread advances while any deadline is running; code still running when it
//! passes stops (the vendored tree-sitter's patch, `vendor/tree-sitter`).

use kiln_l10n::tr;
use std::{
    cell::{Cell, RefCell},
    collections::BTreeSet,
    fmt, fs,
    hash::{DefaultHasher, Hash as _, Hasher as _},
    io::{self, Read as _},
    ops::{Deref, DerefMut},
    path::{Path, PathBuf},
    sync::{
        Condvar, Mutex, OnceLock, PoisonError, RwLock,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use tree_sitter::{
    LANGUAGE_VERSION, LanguageError, MIN_COMPATIBLE_LANGUAGE_VERSION, Parser, QueryError,
    WasmStore,
    wasmtime::{Config, Engine},
};

use crate::{
    CommentTokens, Language, detect,
    language::{self, QueryKind},
};

/// The most [`register_language`] reads of a module or a query file.
pub const MAX_GRAMMAR_BYTES: u64 = 20 << 20;

/// How long a grammar's code may run before it's stopped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GrammarDeadlines {
    /// Loading it, from compiling the module (for which there's time to
    /// spare) to its initializer's end; also instantiating it in each
    /// parsing thread.
    pub load: Duration,
    /// Each parse, as long as a full parse is given on another thread.
    pub parse: Duration,
}

impl GrammarDeadlines {
    pub const DEFAULT: Self = Self {
        load: Duration::from_secs(10),
        parse: Duration::from_secs(5),
    };
}

impl Default for GrammarDeadlines {
    fn default() -> Self {
        Self::DEFAULT
    }
}

static DEADLINES: RwLock<GrammarDeadlines> = RwLock::new(GrammarDeadlines::DEFAULT);

/// Sets the deadlines for loads and parses started from now on.
pub fn set_grammar_deadlines(deadlines: GrammarDeadlines) {
    *DEADLINES.write().unwrap_or_else(PoisonError::into_inner) = deadlines;
}

fn deadlines() -> GrammarDeadlines {
    *DEADLINES.read().unwrap_or_else(PoisonError::into_inner)
}

/// A grammar to load at run time, and the files it's for.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GrammarSpec {
    /// What [`crate::language_by_name`] and injections call it.
    pub language: String,
    /// The module `tree-sitter build --wasm` makes. Its file name says which
    /// grammar it holds, as the CLI names it: `tree-sitter-<grammar>.wasm`
    /// or `<grammar>.wasm`, a `-` in `<grammar>` standing for `_`.
    pub wasm: PathBuf,
    /// File extensions, without the dot, and whole file names it's chosen
    /// for, ignoring case. An extension may be compound, such as `blade.php`
    /// for `x.blade.php`: it wins over any language's plain `php`, and the
    /// longest compound one wins; see [`Language::file_has_extension`].
    pub extensions: Vec<String>,
    pub file_names: Vec<String>,
    /// The highlight query; without one the language parses (for folding
    /// and selection) but isn't coloured.
    pub highlights: Option<PathBuf>,
    /// The query marking code in other languages, as in Markdown fences.
    pub injections: Option<PathBuf>,
    /// Whether the first of several patterns capturing a node wins, as in
    /// tree-sitter's own highlighter and most grammars' queries. Otherwise the
    /// last does, as in Neovim's.
    pub earlier_patterns_win: bool,
    /// The token starting a line comment, for [`crate::comment_tokens`].
    pub line_comment: Option<String>,
    /// The tokens opening and closing a block comment.
    pub block_comment: Option<(String, String)>,
    /// Interpreters whose `#!` line picks it for a file no path decides,
    /// by base name, as [`crate::language_name_for_shebang`] matches the
    /// built-in ones: `elixir` takes `#!/usr/bin/env elixir` and
    /// `#!/usr/bin/elixir1.17`.
    pub shebangs: Vec<String>,
}

/// Why [`register_language`] refused a grammar.
#[derive(Debug)]
pub enum GrammarError {
    /// A file couldn't be read.
    Read { file: PathBuf, error: io::Error },
    /// A file is over [`MAX_GRAMMAR_BYTES`].
    TooLarge { file: PathBuf, bytes: u64 },
    /// The module isn't a grammar this tree-sitter loads: not WebAssembly,
    /// without the `tree_sitter_<grammar>` function its name says, or of an
    /// ABI version outside those supported.
    Module { file: PathBuf, message: String },
    /// A query doesn't compile against the grammar; `error` has its row and
    /// column, zero-based.
    Query { file: PathBuf, error: QueryError },
    /// The module's code was still running at the load's deadline
    /// ([`GrammarDeadlines::load`]), as an initializer that never returns is.
    Deadline { file: PathBuf, deadline: Duration },
    /// A built-in language has this name, extension or file name; the
    /// built-in keeps it.
    Clash {
        language: String,
        built_in: &'static str,
        clash: Clash,
    },
}

/// What a [`GrammarError::Clash`] is over.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Clash {
    Name,
    Extension(String),
    FileName(String),
    /// A [shebang](GrammarSpec::shebangs) interpreter, some name of which
    /// a built-in's would also match.
    Interpreter(String),
}

impl GrammarError {
    /// The file at fault: all but a clash have one.
    pub fn file(&self) -> Option<&Path> {
        match self {
            Self::Read { file, .. }
            | Self::TooLarge { file, .. }
            | Self::Module { file, .. }
            | Self::Query { file, .. }
            | Self::Deadline { file, .. } => Some(file),
            Self::Clash { .. } => None,
        }
    }
}

impl fmt::Display for GrammarError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { file, error } => f.write_str(&tr!(
                "syntax-grammar-file-error",
                file = file.display().to_string(),
                error = error.to_string()
            )),
            Self::TooLarge { file, bytes } => f.write_str(&tr!(
                "syntax-grammar-too-large",
                file = file.display().to_string(),
                bytes = *bytes,
                max = MAX_GRAMMAR_BYTES
            )),
            Self::Module { file, message } => f.write_str(&tr!(
                "syntax-grammar-file-error",
                file = file.display().to_string(),
                error = message.as_str()
            )),
            Self::Query { file, error } => f.write_str(&tr!(
                "syntax-grammar-file-error",
                file = file.display().to_string(),
                error = error.to_string()
            )),
            Self::Deadline { file, deadline } => f.write_str(&tr!(
                "syntax-grammar-deadline",
                file = file.display().to_string(),
                deadline = format!("{deadline:?}")
            )),
            Self::Clash {
                language,
                built_in,
                clash,
            } => {
                let (kind, value) = match clash {
                    Clash::Name => ("name", String::new()),
                    Clash::Extension(extension) => ("extension", format!("{extension:?}")),
                    Clash::FileName(name) => ("file-name", format!("{name:?}")),
                    Clash::Interpreter(name) => ("interpreter", format!("{name:?}")),
                };
                f.write_str(&tr!(
                    "syntax-grammar-clash",
                    language = language.as_str(),
                    kind = kind,
                    value = value,
                    built_in = *built_in
                ))
            }
        }
    }
}

impl std::error::Error for GrammarError {}

/// Loads `spec`'s grammar and queries and adds the language for its files,
/// after the built-in languages: lookups by name, path and injection find a
/// built-in first, and an earlier registered language before a later one.
///
/// Registering a name again replaces it, unless its files are unchanged, which
/// returns the language already loaded. Registrations run one at a time.
/// Each language loaded stays in memory, for the documents still using it.
///
/// Compiling the module takes a while (see the crate's tests), so call it off
/// the main thread.
pub fn register_language(spec: &GrammarSpec) -> Result<&'static Language, GrammarError> {
    if let Some((built_in, clash)) = language::built_in_clash(spec) {
        return Err(GrammarError::Clash {
            language: spec.language.clone(),
            built_in,
            clash,
        });
    }
    let _loading = loading();
    let wasm = read(&spec.wasm)?;
    let highlights = spec.highlights.as_deref().map(read_text).transpose()?;
    let injections = spec.injections.as_deref().map(read_text).transpose()?;
    let grammar_name = grammar_name(&spec.wasm);

    let mut hasher = DefaultHasher::new();
    (&wasm, &highlights, &injections, &grammar_name).hash(&mut hasher);
    spec.earlier_patterns_win.hash(&mut hasher);
    let version = hasher.finish();
    let (name, loaded) = {
        let registry = registry().read().unwrap_or_else(PoisonError::into_inner);
        let existing = registry.iter().find(|entry| entry.name == spec.language);
        (
            existing.map(|entry| entry.name),
            existing
                .filter(|entry| entry.version == version)
                .map(|entry| entry.language),
        )
    };
    let language = match loaded {
        Some(language) => language,
        None => {
            let grammar = load(&spec.wasm, &grammar_name, &wasm)?;
            let name = name.unwrap_or_else(|| String::leak(spec.language.clone()));
            let mut language = Language::new(
                name,
                grammar,
                highlights.as_deref().unwrap_or_default(),
                spec.earlier_patterns_win,
                injections.as_deref(),
            )
            .map_err(|(kind, error)| {
                let file = match kind {
                    QueryKind::Highlights => spec.highlights.clone(),
                    QueryKind::Injections => spec.injections.clone(),
                };
                GrammarError::Query {
                    file: file.unwrap_or_default(),
                    error,
                }
            })?;
            language.wasm = Some(spec.wasm.clone());
            // Kept for the process's life from here: counted once it is.
            LOADED_GRAMMARS.fetch_add(1, Ordering::Relaxed);
            &*Box::leak(Box::new(language))
        }
    };
    let entry = Registered {
        name: language.name(),
        extensions: spec.extensions.clone(),
        file_names: spec.file_names.clone(),
        comments: comment_tokens_of(spec),
        shebangs: (spec.shebangs.iter())
            .filter(|name| !name.is_empty())
            .cloned()
            .collect(),
        language,
        version,
    };
    let mut registry = registry().write().unwrap_or_else(PoisonError::into_inner);
    match registry.iter_mut().find(|other| other.name == entry.name) {
        Some(other) => *other = entry,
        None => registry.push(entry),
    }
    Ok(language)
}

/// Withdraws the language registered as `name` ([`register_language`]):
/// files opened after, and injections found after, no longer get it,
/// while a [`Syntax`](crate::Syntax) made with it keeps it. Waits for a
/// registration under way, so one that started before can't publish
/// after. Returns whether it was registered.
pub fn unregister_language(name: &str) -> bool {
    let _loading = loading();
    let mut registry = registry().write().unwrap_or_else(PoisonError::into_inner);
    let before = registry.len();
    registry.retain(|entry| entry.name != name);
    registry.len() != before
}

/// Held from reading a grammar's files to publishing it, and while one is
/// withdrawn: two registrations of the same files then load once, and
/// the one called last, which read them last, is the one kept.
fn loading() -> std::sync::MutexGuard<'static, ()> {
    static LOADING: Mutex<()> = Mutex::new(());
    LOADING.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The WebAssembly stores parsers hold, across threads: each thread keeps
/// one per grammar between parses, up to [`WASM_PARSERS_PER_THREAD`], and
/// has one more for each parser in use from outside its cache.
static LIVE_STORES: AtomicUsize = AtomicUsize::new(0);
/// The WebAssembly grammars [`register_language`] made a language of: each
/// is kept for the process's life, withdrawn or not.
static LOADED_GRAMMARS: AtomicUsize = AtomicUsize::new(0);

/// How many WebAssembly stores and grammars the process holds, for the
/// memory report. Their linear memory isn't measured: tree-sitter doesn't
/// expose a store's memory.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WasmStats {
    /// Stores in parsers, each for one grammar on one thread.
    pub stores: usize,
    /// Grammars loaded by [`register_language`].
    pub grammars: usize,
}

pub fn wasm_stats() -> WasmStats {
    WasmStats {
        stores: LIVE_STORES.load(Ordering::Relaxed),
        grammars: LOADED_GRAMMARS.load(Ordering::Relaxed),
    }
}

/// The grammar `wasm` holds, by its file name: see [`GrammarSpec::wasm`].
fn grammar_name(wasm: &Path) -> String {
    let stem = wasm
        .file_stem()
        .map(|stem| stem.to_string_lossy())
        .unwrap_or_default();
    let stem = stem.strip_prefix("tree-sitter-").unwrap_or(&stem);
    stem.replace('-', "_")
}

fn load(
    file: &Path,
    grammar_name: &str,
    wasm: &[u8],
) -> Result<tree_sitter::Language, GrammarError> {
    let module = |message: String| GrammarError::Module {
        file: file.to_owned(),
        message,
    };
    let engine = engine().map_err(|error| module(error.to_owned()))?;
    let timeout = deadlines().load;
    // The store only instantiates the module to read the language out of
    // it; parsers instantiate it again in their own.
    let (loaded, timed_out) = within(timeout, &|| false, || {
        WasmStore::new(engine)?.load_language(grammar_name, wasm)
    });
    let grammar = loaded.map_err(|error| {
        if timed_out {
            GrammarError::Deadline {
                file: file.to_owned(),
                deadline: timeout,
            }
        } else {
            module(error.to_string())
        }
    })?;
    let abi = grammar.abi_version();
    if !(MIN_COMPATIBLE_LANGUAGE_VERSION..=LANGUAGE_VERSION).contains(&abi) {
        return Err(module(
            tr!(
                "syntax-grammar-abi",
                abi = abi.to_string(),
                min = MIN_COMPATIBLE_LANGUAGE_VERSION.to_string(),
                max = LANGUAGE_VERSION.to_string()
            )
            .to_string(),
        ));
    }
    Ok(grammar)
}

fn read(file: &Path) -> Result<Vec<u8>, GrammarError> {
    let error = |error| GrammarError::Read {
        file: file.to_owned(),
        error,
    };
    let opened = fs::File::open(file).map_err(error)?;
    let mut bytes = Vec::new();
    // One byte over the limit tells a file that's too large, whatever its
    // metadata says.
    opened
        .take(MAX_GRAMMAR_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(error)?;
    if bytes.len() as u64 > MAX_GRAMMAR_BYTES {
        let bytes = fs::metadata(file).map_or(MAX_GRAMMAR_BYTES + 1, |meta| meta.len());
        return Err(GrammarError::TooLarge {
            file: file.to_owned(),
            bytes,
        });
    }
    Ok(bytes)
}

fn read_text(file: &Path) -> Result<String, GrammarError> {
    String::from_utf8(read(file)?).map_err(|error| GrammarError::Read {
        file: file.to_owned(),
        error: io::Error::new(io::ErrorKind::InvalidData, error),
    })
}

/// How often the engine's epoch advances while a deadline runs, so how
/// often running code checks it.
const EPOCH: Duration = Duration::from_millis(5);

/// The engine every module is compiled on, with epoch interruption, and the
/// ticker advancing its epoch. Without a ticker, an error: a grammar's code
/// would run with no deadline.
fn engine() -> Result<&'static Engine, &'static str> {
    static ENGINE: OnceLock<Result<Engine, String>> = OnceLock::new();
    let engine = ENGINE.get_or_init(|| {
        new_engine(|ticking| {
            std::thread::Builder::new()
                .name("wasm-epoch".into())
                .spawn(move || tick(&ticking))
                .map(drop)
        })
    });
    engine.as_ref().map_err(String::as_str)
}

/// An engine with epoch interruption, once `start_ticker` has started its
/// ticker with a handle to it.
fn new_engine(start_ticker: impl FnOnce(Engine) -> io::Result<()>) -> Result<Engine, String> {
    let mut config = Config::new();
    config.epoch_interruption(true);
    let engine = Engine::new(&config).map_err(|error| error.to_string())?;
    start_ticker(engine.clone())
        .map_err(|error| tr!("syntax-grammar-no-ticker", error = error.to_string()).to_string())?;
    Ok(engine)
}

/// How many [`within`] calls are running, which the ticker waits for.
static RUNNING: Mutex<usize> = Mutex::new(0);
static STARTED: Condvar = Condvar::new();

/// The ticker: advances `engine`'s epoch every [`EPOCH`] while a [`within`]
/// call is running, and sleeps while none is.
fn tick(engine: &Engine) {
    loop {
        let running = RUNNING.lock().unwrap_or_else(PoisonError::into_inner);
        drop(
            STARTED
                .wait_while(running, |running| *running == 0)
                .unwrap_or_else(PoisonError::into_inner),
        );
        std::thread::sleep(EPOCH);
        engine.increment_epoch();
    }
}

/// Runs `f`, with Wasm code this thread runs inside it stopped once
/// `timeout` from now has passed, or once `interrupt` says so: the caller's
/// own budget. Also returns whether the timeout stopped it.
fn within<R>(timeout: Duration, interrupt: &dyn Fn() -> bool, f: impl FnOnce() -> R) -> (R, bool) {
    struct Running;
    impl Drop for Running {
        fn drop(&mut self) {
            *RUNNING.lock().unwrap_or_else(PoisonError::into_inner) -= 1;
        }
    }
    let deadline = Instant::now().checked_add(timeout);
    let timed_out = Cell::new(false);
    let stop = || {
        if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            timed_out.set(true);
            return true;
        }
        interrupt()
    };
    *RUNNING.lock().unwrap_or_else(PoisonError::into_inner) += 1;
    let running = Running;
    STARTED.notify_all();
    let result = tree_sitter::with_wasm_interrupt(&stop, f);
    drop(running);
    (result, timed_out.get())
}

struct Registered {
    name: &'static str,
    extensions: Vec<String>,
    file_names: Vec<String>,
    /// `None` when its spec gave no tokens, as for a language without
    /// them before they could be given.
    comments: Option<CommentTokens>,
    shebangs: Vec<String>,
    language: &'static Language,
    /// A hash of the files it was loaded from.
    version: u64,
}

fn registry() -> &'static RwLock<Vec<Registered>> {
    static REGISTRY: RwLock<Vec<Registered>> = RwLock::new(Vec::new());
    &REGISTRY
}

fn find<T>(f: impl FnMut(&Registered) -> Option<T>) -> Option<T> {
    registry()
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .iter()
        .find_map(f)
}

pub(crate) fn by_name(name: &str) -> Option<&'static Language> {
    find(|entry| (entry.name == name).then_some(entry.language))
}

/// The comment tokens `spec` gives, interned: `None` if it gives none.
/// An empty token is none.
fn comment_tokens_of(spec: &GrammarSpec) -> Option<CommentTokens> {
    let line = (spec.line_comment.as_deref())
        .filter(|token| !token.is_empty())
        .map(intern);
    let block = (spec.block_comment.as_ref())
        .filter(|(open, close)| !open.is_empty() && !close.is_empty())
        .map(|(open, close)| (intern(open), intern(close)));
    (line.is_some() || block.is_some()).then_some(CommentTokens { line, block })
}

/// `text` as a string that lives for the process, one copy each: a
/// grammar registered again with the same tokens leaks nothing more.
fn intern(text: &str) -> &'static str {
    static INTERNED: Mutex<BTreeSet<&'static str>> = Mutex::new(BTreeSet::new());
    let mut interned = INTERNED.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(&kept) = interned.get(text) {
        return kept;
    }
    let kept = String::leak(text.to_owned());
    interned.insert(kept);
    kept
}

pub(crate) fn comment_tokens(name: &str) -> Option<CommentTokens> {
    find(|entry| (entry.name == name).then_some(entry.comments)).flatten()
}

/// The registered language whose shebangs match the interpreter `program`,
/// a base name; see [`detect::interpreter_matches`].
pub(crate) fn name_for_interpreter(program: &str) -> Option<&'static str> {
    find(|entry| {
        (entry.shebangs.iter())
            .any(|interpreter| detect::interpreter_matches(interpreter, program))
            .then_some(entry.name)
    })
}

pub(crate) fn name_for_file(file_name: &str, extension: Option<&str>) -> Option<&'static str> {
    find(|entry| {
        let named = entry
            .file_names
            .iter()
            .any(|name| name.eq_ignore_ascii_case(file_name));
        let extended = extension.is_some_and(|extension| {
            entry
                .extensions
                .iter()
                .any(|known| known.eq_ignore_ascii_case(extension))
        });
        (named || extended).then_some(entry.name)
    })
}

/// Each registered language claiming a compound extension of a file called
/// `file_name`, with the longest such claim's length, in registration order.
pub(crate) fn compound_owners(file_name: &str) -> Vec<(&'static str, usize)> {
    registry()
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .iter()
        .filter_map(|entry| {
            let extensions = entry.extensions.iter().map(String::as_str);
            Some((
                entry.name,
                language::longest_compound(file_name, extensions)?,
            ))
        })
        .collect()
}

/// The language an injection names, lowercase: by name, else by extension.
pub(crate) fn injected(name: &str) -> Option<&'static Language> {
    find(|entry| {
        entry
            .name
            .eq_ignore_ascii_case(name)
            .then_some(entry.language)
    })
    .or_else(|| {
        find(|entry| {
            entry
                .extensions
                .iter()
                .any(|extension| extension.eq_ignore_ascii_case(name))
                .then_some(entry.language)
        })
    })
}

pub(crate) fn names() -> Vec<&'static str> {
    registry()
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .iter()
        .map(|entry| entry.name)
        .collect()
}

/// The most WebAssembly parsers a thread keeps between parses, each with
/// a store of its own for one grammar; past that the least recently used is
/// dropped, with its store. A store per grammar makes the store's memory cap
/// (`MAX_MEMORY_SIZE`, `vendor/tree-sitter/src/wasm_store.c`) a cap on each
/// grammar: one can't fill the memory the others parse in. Eight hold a
/// file's language and the few its injections bring in (a Markdown file's
/// fences, a web template's script and style) for several files at once,
/// so switching between them doesn't instantiate a module again, which runs
/// its initializer. It bounds the cache, not the stores alive at once:
/// parsers in use are outside it, a grammar new to the thread gets its store
/// before the oldest cached one goes, and a grammar taken twice at once (an
/// injection of itself) has a second store until it's given back.
pub(crate) const WASM_PARSERS_PER_THREAD: usize = 8;

thread_local! {
    static PARSERS: RefCell<Parsers> = const {
        RefCell::new(Parsers {
            native: None,
            wasm: Vec::new(),
        })
    };
}

/// A thread's parsers, kept between parses.
struct Parsers {
    /// The one native languages share: they need no store.
    native: Option<Cached>,
    /// One per WebAssembly language, least recently used first, at most
    /// [`WASM_PARSERS_PER_THREAD`].
    wasm: Vec<Cached>,
}

struct Cached {
    parser: Parser,
    /// The WebAssembly language its [`WasmStore`] is for ([`wasm_key`]),
    /// counted in [`LIVE_STORES`]; `None` for the native parser, which has
    /// no store.
    store_for: Option<usize>,
}

impl Cached {
    fn native() -> Self {
        Self {
            parser: Parser::new(),
            store_for: None,
        }
    }
}

impl Drop for Cached {
    fn drop(&mut self) {
        if self.store_for.is_some() {
            LIVE_STORES.fetch_sub(1, Ordering::Relaxed);
        }
    }
}

/// Which WebAssembly language a parser's store is for: the language's
/// address, which a registered one keeps for the process's life.
fn wasm_key(language: &Language) -> usize {
    std::ptr::from_ref(language) as usize
}

/// This thread's native parser, or a new one when it's taken.
fn take_native() -> Cached {
    (PARSERS.try_with(|parsers| parsers.borrow_mut().native.take()))
        .ok()
        .flatten()
        .unwrap_or_else(Cached::native)
}

/// This thread's parser for the WebAssembly language `key`, or, when it has
/// none or it's taken, a new one with a store of its own.
fn take_wasm(key: usize, name: &str) -> Result<Cached, LanguageError> {
    let kept = PARSERS.try_with(|parsers| {
        let wasm = &mut parsers.borrow_mut().wasm;
        let at = wasm
            .iter()
            .position(|cached| cached.store_for == Some(key))?;
        Some(wasm.remove(at))
    });
    if let Some(kept) = kept.ok().flatten() {
        return Ok(kept);
    }
    let store = engine()
        .map_err(str::to_owned)
        .and_then(|engine| WasmStore::new(engine).map_err(|error| error.to_string()))
        .map_err(|error| {
            log::error!("{name}: no WebAssembly store: {error}");
            LanguageError::Wasm
        })?;
    let mut parser = Parser::new();
    parser.set_wasm_store(store)?;
    LIVE_STORES.fetch_add(1, Ordering::Relaxed);
    Ok(Cached {
        parser,
        store_for: Some(key),
    })
}

/// Puts `cached` back in this thread's cache, reset to parse afresh: as its
/// language's most recently used, dropping the least recently used past
/// [`WASM_PARSERS_PER_THREAD`]. One the cache holds already for its
/// language, as when it was taken twice, is just dropped.
fn give_back(mut cached: Cached) {
    // A parse stopped part way would otherwise resume.
    cached.parser.reset();
    if cached.parser.set_included_ranges(&[]).is_err() {
        return;
    }
    // Gone at thread exit, when the parser is just dropped. What isn't kept
    // is dropped once the cache is no longer borrowed.
    let _dropped = PARSERS.try_with(move |parsers| {
        let parsers = &mut *parsers.borrow_mut();
        let Some(key) = cached.store_for else {
            if parsers.native.is_some() {
                return Some(cached);
            }
            parsers.native = Some(cached);
            return None;
        };
        if (parsers.wasm.iter()).any(|other| other.store_for == Some(key)) {
            return Some(cached);
        }
        parsers.wasm.push(cached);
        (parsers.wasm.len() > WASM_PARSERS_PER_THREAD).then(|| parsers.wasm.remove(0))
    });
}

/// One of this thread's parsers, back in its cache when dropped, reset to
/// parse afresh: the native one, or, once [set](Self::set_language) to a
/// WebAssembly language, that language's own. When the one wanted is taken
/// already, a new one, dropped after.
pub(crate) struct ThreadParser {
    cached: Option<Cached>,
    /// The WebAssembly language set, if it is one.
    wasm: Option<WasmLanguage>,
}

/// A WebAssembly language [`ThreadParser::set_language`] was given. The
/// parser is set to it in [`ThreadParser::parse`], under the parse's budget:
/// a store new to the module instantiates it, running its initializer.
struct WasmLanguage {
    name: &'static str,
    file: PathBuf,
    grammar: tree_sitter::Language,
    set: bool,
}

impl ThreadParser {
    pub(crate) fn take() -> Self {
        Self {
            cached: Some(take_native()),
            wasm: None,
        }
    }

    /// [`Self::take`], set to `language`.
    pub(crate) fn for_language(language: &Language) -> Result<Self, LanguageError> {
        let mut parser = Self::take();
        parser.set_language(language)?;
        Ok(parser)
    }

    /// Sets the parser's language. A WebAssembly one swaps in the parser
    /// with that language's store, making it if the thread has none, and is
    /// set when it [parses](Self::parse); a native one swaps in the native
    /// parser.
    pub(crate) fn set_language(&mut self, language: &Language) -> Result<(), LanguageError> {
        self.wasm = None;
        let key = language.grammar.is_wasm().then(|| wasm_key(language));
        if self.cached().store_for != key {
            let next = match key {
                None => take_native(),
                Some(key) => take_wasm(key, language.name())?,
            };
            if let Some(previous) = self.cached.replace(next) {
                give_back(previous);
            }
        }
        if key.is_none() {
            return self.cached().parser.set_language(&language.grammar);
        }
        self.wasm = Some(WasmLanguage {
            name: language.name(),
            file: language.wasm.clone().unwrap_or_default(),
            grammar: language.grammar.clone(),
            set: false,
        });
        Ok(())
    }

    /// Runs `parse`, one parse with this parser. With a WebAssembly language
    /// it's set first, and its code stops once `interrupt` says so, the
    /// caller's budget, or at the parse deadline, which is logged.
    pub(crate) fn parse<T>(
        &mut self,
        interrupt: &dyn Fn() -> bool,
        parse: impl FnOnce(&mut Parser) -> Option<T>,
    ) -> Option<T> {
        let Self { cached, wasm } = self;
        let parser = &mut cached.as_mut().expect("taken only when dropped").parser;
        let Some(wasm) = wasm else {
            return parse(parser);
        };
        let timeout = deadlines().parse;
        let (parsed, timed_out) = within(timeout, interrupt, || {
            if !wasm.set {
                if let Err(error) = parser.set_language(&wasm.grammar) {
                    log::warn!("{}: {}: {error}", wasm.file.display(), wasm.name);
                    return None;
                }
                wasm.set = true;
            }
            parse(parser)
        });
        if parsed.is_none() && timed_out {
            log::error!(
                "{}: {} was still running at its {timeout:?} parse deadline, so stopped",
                wasm.file.display(),
                wasm.name
            );
        }
        parsed
    }

    fn cached(&mut self) -> &mut Cached {
        self.cached.as_mut().expect("taken only when dropped")
    }
}

impl Deref for ThreadParser {
    type Target = Parser;

    fn deref(&self) -> &Parser {
        &self
            .cached
            .as_ref()
            .expect("taken only when dropped")
            .parser
    }
}

impl DerefMut for ThreadParser {
    fn deref_mut(&mut self) -> &mut Parser {
        &mut self.cached().parser
    }
}

impl Drop for ThreadParser {
    fn drop(&mut self) {
        if let Some(cached) = self.cached.take() {
            give_back(cached);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, io, path::Path};

    use super::{GrammarError, GrammarSpec, new_engine, register_language, wasm_stats};

    #[test]
    fn no_ticker_is_no_engine() {
        let error = new_engine(|_| Err(io::Error::other("refused"))).unwrap_err();
        assert_eq!(error, "no thread to enforce grammars' deadlines: refused");
    }

    /// A grammar whose query is refused isn't counted as held; one that
    /// registers is, once.
    #[test]
    fn only_registered_grammars_are_counted() {
        let manifest = std::env::var_os("CARGO_MANIFEST_DIR").expect("run by cargo");
        let wasm = Path::new(&manifest).join("tests/fixtures/tree-sitter-json.wasm");
        let dir = std::env::temp_dir().join(format!("fe-syntax-wasm-stats-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let bad = dir.join("bad.scm");
        fs::write(&bad, "(no_such_node) @string\n").unwrap();
        let good = dir.join("good.scm");
        fs::write(&good, "(string) @string\n").unwrap();
        let spec = |name: &str, highlights: &Path| GrammarSpec {
            language: name.into(),
            wasm: wasm.clone(),
            extensions: vec![format!("{name}x")],
            highlights: Some(highlights.to_owned()),
            ..GrammarSpec::default()
        };
        let before = wasm_stats().grammars;
        let refused = register_language(&spec("json_bad_query", &bad));
        assert!(
            matches!(refused, Err(GrammarError::Query { .. })),
            "{refused:?}"
        );
        assert_eq!(wasm_stats().grammars, before);
        register_language(&spec("json_counted", &good)).unwrap();
        assert_eq!(wasm_stats().grammars, before + 1);
        fs::remove_dir_all(&dir).unwrap();
    }

    /// A thread keeps one native parser, however many were taken at once,
    /// and one per WebAssembly language, the least recently used dropped
    /// past [`WASM_PARSERS_PER_THREAD`]; a parser set from one kind to the
    /// other swaps the thread's parser for it in.
    #[cfg(feature = "json")]
    #[test]
    fn a_thread_keeps_one_native_parser_and_one_per_wasm_language() {
        use super::{PARSERS, ThreadParser, WASM_PARSERS_PER_THREAD, wasm_key};

        let manifest = std::env::var_os("CARGO_MANIFEST_DIR").expect("run by cargo");
        let wasm = Path::new(&manifest).join("tests/fixtures/tree-sitter-json.wasm");
        let languages: Vec<_> = (0..=WASM_PARSERS_PER_THREAD)
            .map(|i| {
                let name = format!("json_cache{i}");
                register_language(&GrammarSpec {
                    language: name.clone(),
                    wasm: wasm.clone(),
                    extensions: vec![format!("{name}x")],
                    ..GrammarSpec::default()
                })
                .unwrap()
            })
            .collect();
        let json = crate::language_by_name("json").unwrap();
        std::thread::spawn(move || {
            let kept = || {
                PARSERS.with(|parsers| {
                    let parsers = parsers.borrow();
                    let wasm = parsers.wasm.iter().map(|cached| cached.store_for);
                    (parsers.native.is_some(), wasm.collect::<Vec<_>>())
                })
            };
            let key = |i: usize| Some(wasm_key(languages[i]));

            let both = (
                ThreadParser::for_language(json).unwrap(),
                ThreadParser::for_language(json).unwrap(),
            );
            assert_eq!(kept(), (false, vec![]));
            drop(both);
            assert_eq!(kept(), (true, vec![]));

            let mut parser = ThreadParser::for_language(languages[0]).unwrap();
            assert_eq!(parser.cached().store_for, key(0));
            assert_eq!(kept(), (true, vec![]));
            parser.set_language(json).unwrap();
            assert_eq!(parser.cached().store_for, None);
            assert_eq!(kept(), (false, vec![key(0)]));
            parser.set_language(languages[0]).unwrap();
            assert_eq!(kept(), (true, vec![]));
            drop(parser);
            assert_eq!(kept(), (true, vec![key(0)]));

            for language in &languages[1..] {
                drop(ThreadParser::for_language(language).unwrap());
            }
            let newest: Vec<_> = (1..=WASM_PARSERS_PER_THREAD).map(key).collect();
            assert_eq!(kept(), (true, newest));
            // Used again, the oldest is the newest.
            drop(ThreadParser::for_language(languages[1]).unwrap());
            let order: Vec<_> = (2..=WASM_PARSERS_PER_THREAD).chain([1]).map(key).collect();
            assert_eq!(kept(), (true, order));
        })
        .join()
        .unwrap();
    }
}
