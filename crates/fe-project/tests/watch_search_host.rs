//! fe-host's `WatchHost` and `SearchHost` through their local impls
//! (`LocalWatch`, `LocalSearch`): the same changes, matches and file list
//! as calling fe-project's watcher, search and finder directly.

use std::{
    collections::{BTreeSet, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, atomic::AtomicBool, mpsc},
    time::{Duration, Instant},
};

use fe_host::{FilesSummary, SearchFailed, SearchFilter, SearchHost, WatchHost};
use fe_project::{
    FileMatches, FinderIndex, FsChange, LocalSearch, LocalWatch, PathFilter, ProjectWatcher,
    SearchError, SearchOptions, SearchQuery, SearchSummary, WalkControl, search, search_excluding,
    search_filtered, walk_files,
};

/// An upper bound only, for a loaded machine: each wait ends as soon as
/// what it waits for arrives.
const LOADED_TIMEOUT: Duration = Duration::from_secs(60);

fn write(root: &Path, rel: &str, text: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

/// A small project: ignored, hidden and nested files, matches in most.
fn project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(root, ".gitignore", "ignored.txt\nbuild/\n");
    write(root, "a.txt", "needle\nhay\nneedle needle\n");
    write(root, "src/b.rs", "fn needle() {}\n");
    write(root, "src/deep/c.rs", "// needle\n");
    write(root, "src/d.rs", "nothing here\n");
    write(root, "docs/e.md", "a needle\n");
    write(root, "ignored.txt", "needle\n");
    write(root, "build/out.txt", "needle\n");
    dir
}

fn one_thread() -> SearchOptions {
    SearchOptions {
        threads: 1,
        ..SearchOptions::default()
    }
}

/// Every batch a direct search's sink got, and its summary.
fn direct(
    run: impl FnOnce(
        Arc<AtomicBool>,
        &mut dyn FnMut(Vec<FileMatches>),
    ) -> Result<SearchSummary, SearchError>,
) -> (Vec<Vec<FileMatches>>, SearchSummary) {
    let mut batches = Vec::new();
    let summary = run(Arc::default(), &mut |batch| batches.push(batch)).unwrap();
    (batches, summary)
}

/// Every batch a host's search streamed, and its summary.
fn hosted(
    host: &dyn SearchHost,
    root: &Path,
    query: &SearchQuery,
    filter: &SearchFilter,
) -> (Vec<Vec<FileMatches>>, SearchSummary) {
    let mut stream = host.search(root, query, &one_thread(), filter);
    let batches: Vec<_> = stream.by_ref().collect();
    let summary = stream.finish().expect("an end value").unwrap();
    (batches, summary)
}

#[test]
fn a_hosted_search_streams_what_search_hands_its_sink() {
    let dir = project();
    let query = SearchQuery::literal("needle");
    let expected = direct(|cancel, sink| search(dir.path(), &query, &one_thread(), cancel, sink));
    assert!(!expected.0.is_empty());
    let got = hosted(&LocalSearch, dir.path(), &query, &SearchFilter::default());
    assert_eq!(got, expected);
}

#[test]
fn a_hosted_search_with_globs_matches_search_filtered() {
    let dir = project();
    let query = SearchQuery::literal("needle");
    let (include, exclude) = (["*.rs", "*.md"], ["src/deep/"]);
    let filter = PathFilter::new(dir.path(), &include, &exclude).unwrap();
    let expected = direct(|cancel, sink| {
        search_filtered(dir.path(), &query, &one_thread(), &filter, cancel, sink)
    });
    let rels: Vec<_> = (expected.0.iter().flatten())
        .map(|file| file.rel.as_str())
        .collect();
    assert_eq!(rels, ["docs/e.md", "src/b.rs"]);
    let globs = SearchFilter::globs(&include, &exclude);
    let got = hosted(&LocalSearch, dir.path(), &query, &globs);
    assert_eq!(got, expected);
}

#[test]
fn a_hosted_search_excluding_a_file_matches_search_excluding() {
    let dir = project();
    let query = SearchQuery::literal("needle");
    let excluded = Path::new("a.txt");
    let expected = direct(|cancel, sink| {
        search_excluding(dir.path(), &query, &one_thread(), excluded, cancel, sink)
    });
    assert!(expected.0.iter().flatten().all(|file| file.rel != "a.txt"));
    let filter = SearchFilter::excluding(excluded);
    let got = hosted(&LocalSearch, dir.path(), &query, &filter);
    assert_eq!(got, expected);
}

#[test]
fn a_hosted_search_with_an_invalid_pattern_ends_with_searchs_error() {
    let dir = project();
    let query = SearchQuery {
        regex: true,
        ..SearchQuery::literal("(")
    };
    let Err(SearchError::InvalidPattern(expected)) =
        search(dir.path(), &query, &one_thread(), Arc::default(), |_| {})
    else {
        panic!("the pattern parsed");
    };
    let host: &dyn SearchHost = &LocalSearch;
    let stream = host.search(dir.path(), &query, &one_thread(), &SearchFilter::default());
    assert!(stream.recv().is_none());
    match stream.finish() {
        Some(Err(SearchFailed::InvalidPattern(message))) => assert_eq!(message, expected),
        other => panic!("ended with {other:?}"),
    }
}

#[test]
fn a_hosted_search_with_an_invalid_glob_ends_with_path_filters_error() {
    let dir = project();
    let expected = PathFilter::new(dir.path(), &["src/[ab"], &[]).unwrap_err();
    let host: &dyn SearchHost = &LocalSearch;
    let filter = SearchFilter::globs(&["src/[ab"], &[]);
    let query = SearchQuery::literal("needle");
    let stream = host.search(dir.path(), &query, &one_thread(), &filter);
    match stream.finish() {
        Some(Err(SearchFailed::InvalidGlob { glob, message })) => {
            assert_eq!((glob, message), (expected.glob, expected.message));
        }
        other => panic!("ended with {other:?}"),
    }
}

#[test]
fn dropping_a_hosted_search_cancels_it() {
    let dir = tempfile::tempdir().unwrap();
    // Batches of 64 files on one thread: the walk can't get past the few
    // batches the stream holds before the drop.
    let total = 400;
    for index in 0..total {
        write(dir.path(), &format!("f{index:03}.txt"), "needle\n");
    }
    let host: &dyn SearchHost = &LocalSearch;
    let query = SearchQuery::literal("needle");
    let stream = host.search(dir.path(), &query, &one_thread(), &SearchFilter::default());
    let ended = stream.ended();
    assert!(stream.recv().is_some());
    drop(stream);
    let summary = ended.wait().expect("an end value").unwrap();
    assert!(summary.cancelled);
    assert!(summary.files_searched < total, "{summary:?}");
}

/// Every path a direct finder walk sent, in order.
fn finder_walk(root: &Path) -> BTreeSet<PathBuf> {
    let mut paths = BTreeSet::new();
    assert!(FinderIndex::walk(root, Arc::default(), |batch| {
        paths.extend(batch)
    }));
    paths
}

/// Every path a host's file list sent, checking none came twice.
fn hosted_files(root: &Path, filter: &SearchFilter) -> BTreeSet<PathBuf> {
    let host: &dyn SearchHost = &LocalSearch;
    let mut stream = host.files(root, filter);
    let mut paths = BTreeSet::new();
    for batch in stream.by_ref() {
        assert!(!batch.is_empty());
        for path in batch {
            assert!(paths.insert(path));
        }
    }
    let end = stream.finish().expect("an end value").unwrap();
    assert_eq!(end, FilesSummary { complete: true });
    paths
}

#[test]
fn a_hosted_file_list_is_the_finders() {
    let dir = project();
    let expected = finder_walk(dir.path());
    assert!(expected.contains(Path::new("src/deep/c.rs")));
    assert!(!expected.contains(Path::new("ignored.txt")));
    assert_eq!(hosted_files(dir.path(), &SearchFilter::default()), expected);
}

#[test]
fn a_hosted_file_list_with_globs_takes_what_the_search_walk_takes() {
    let dir = project();
    let (include, exclude) = (["*.rs", "*.md"], ["src/deep/"]);
    let filter = PathFilter::new(dir.path(), &include, &exclude).unwrap();
    let expected = search_walked(dir.path(), &filter);
    assert_eq!(
        expected,
        BTreeSet::from(["docs/e.md", "src/b.rs", "src/d.rs"].map(PathBuf::from))
    );
    let globs = SearchFilter::globs(&include, &exclude);
    assert_eq!(hosted_files(dir.path(), &globs), expected);
}

/// The files the search walk takes with `filter`.
fn search_walked(root: &Path, filter: &PathFilter) -> BTreeSet<PathBuf> {
    let walked = Mutex::new(BTreeSet::new());
    walk_files(root, 1, filter, Arc::default(), |file| {
        walked.lock().unwrap().insert(file.rel);
        WalkControl::Continue
    });
    walked.into_inner().unwrap()
}

#[test]
fn a_hosted_file_list_keeps_an_excluded_directorys_negated_files_out() {
    let dir = project();
    write(dir.path(), "src/deep/keep.rs", "needle\n");
    write(dir.path(), "docs/old/notes/keep.md", "needle\n");
    // The walk never enters an excluded directory, so a negated glob for a
    // file in it brings nothing back.
    let exclude = [
        "src/deep/",
        "!src/deep/keep.rs",
        "docs/old/",
        "!docs/old/notes/keep.md",
    ];
    let filter = PathFilter::new(dir.path(), &[], &exclude).unwrap();
    let expected = search_walked(dir.path(), &filter);
    assert!(expected.contains(Path::new("src/b.rs")));
    assert!(!expected.contains(Path::new("src/deep/keep.rs")));
    assert!(!expected.contains(Path::new("docs/old/notes/keep.md")));
    let globs = SearchFilter::globs(&[], &exclude);
    assert_eq!(hosted_files(dir.path(), &globs), expected);
}

#[test]
fn a_hosted_file_list_leaves_the_excluded_file_out() {
    let dir = project();
    let mut expected = finder_walk(dir.path());
    assert!(expected.remove(Path::new("src/b.rs")));
    let filter = SearchFilter::excluding("src/b.rs");
    assert_eq!(hosted_files(dir.path(), &filter), expected);
}

/// How long a watcher stays quiet before `last` is written again.
const QUIET: Duration = Duration::from_secs(3);

/// What one watcher delivered while `changes_through` waited on it.
struct Wait {
    /// Every change delivered, up to and including the batch naming `last`.
    seen: HashSet<FsChange>,
    /// Each batch (when, after how many rewrites, which changes), quiet
    /// period and rewrite, in order.
    timeline: Vec<String>,
    /// Why the wait ended without `last`: the deadline or a disconnect.
    failure: Option<&'static str>,
}

impl Wait {
    /// One line per watcher for a failure message: how its wait ended, and
    /// everything it delivered and when.
    fn report(&self, label: &str) -> String {
        let end = self.failure.unwrap_or("named the last file");
        format!("{label}: {end}; {:#?}", self.timeline)
    }
}

/// Every change `next` delivers up to and including the batch that names
/// `last`: a watcher delivers a change after those made before it, so this
/// is everything before `last` too. It never panics, so the caller can wait
/// on every watcher and report all of their timelines together.
///
/// Under heavy load (a security scanner pinning the CPU) FSEvents has
/// delivered a batch many seconds late or not at all; which, the timeline
/// can't say, because a batch that comes after a rewrite may hold the
/// original event delayed, not lost. So the wait is long, and when nothing
/// arrives for `quiet` `last` is written again, which names it in a fresh
/// event without changing the set of paths: the test tolerates either. A
/// disconnected watcher fails at once, without writing anything.
fn changes_through(
    last: &Path,
    quiet: Duration,
    deadline: Duration,
    mut next: impl FnMut(Duration) -> Result<Vec<FsChange>, mpsc::RecvTimeoutError>,
) -> Wait {
    let start = Instant::now();
    let mut wait = Wait {
        seen: HashSet::new(),
        timeline: Vec::new(),
        failure: None,
    };
    let wanted = FsChange::Path(last.to_path_buf());
    let mut rewrites = 0;
    while !wait.seen.contains(&wanted) {
        let Some(left) = deadline.checked_sub(start.elapsed()) else {
            wait.failure = Some("no last file by the deadline");
            break;
        };
        match next(left.min(quiet)) {
            Ok(batch) => {
                wait.timeline.push(format!(
                    "{:?}: batch after {rewrites} rewrites: {batch:?}",
                    start.elapsed()
                ));
                wait.seen.extend(batch);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                rewrites += 1;
                wait.timeline
                    .push(format!("{:?}: quiet, rewrote the file", start.elapsed()));
                fs::write(last, "").unwrap();
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                wait.failure = Some("the watcher went away");
                break;
            }
        }
    }
    if wait.failure.is_none() && rewrites > 0 {
        eprintln!("{}", wait.report("named late"));
    }
    wait
}

/// `Err` with both watchers' reports unless each named `last` and delivered
/// every path in `changed`. Only those are compared: the writes that built
/// the project before the watchers started can still be in fseventsd's queue
/// when a stream opens, and reach one watcher and not the other (the root,
/// `src`, `main.rs`).
fn compare(direct: &Wait, hosted: &Wait, changed: &HashSet<FsChange>) -> Result<(), String> {
    let reports = format!("{}\n{}", direct.report("watcher"), hosted.report("host"));
    for (label, wait) in [("watcher", direct), ("host", hosted)] {
        if let Some(why) = wait.failure {
            return Err(format!("the {label} failed ({why})\n{reports}"));
        }
        let missing: Vec<_> = changed.difference(&wait.seen).collect();
        if !missing.is_empty() {
            return Err(format!("the {label} missed {missing:?}\n{reports}"));
        }
    }
    Ok(())
}

fn changed_set(root: &Path) -> HashSet<FsChange> {
    [
        "src/new.rs",
        "src/old.rs",
        "src/moved.rs",
        "src/gone.rs",
        "src/last.rs",
    ]
    .iter()
    .map(|rel| FsChange::Path(root.join(rel)))
    .collect()
}

fn batch(paths: &[&Path]) -> Vec<FsChange> {
    paths
        .iter()
        .map(|p| FsChange::Path(p.to_path_buf()))
        .collect()
}

#[test]
fn a_disconnected_watcher_fails_without_rewriting() {
    let dir = tempfile::tempdir().unwrap();
    let last = dir.path().join("last.rs");
    let (tx, rx) = mpsc::channel::<Vec<FsChange>>();
    drop(tx);
    let wait = changes_through(&last, QUIET, LOADED_TIMEOUT, |left| rx.recv_timeout(left));
    assert_eq!(wait.failure, Some("the watcher went away"));
    assert!(!last.exists());
}

#[test]
fn a_quiet_watcher_gets_the_file_rewritten_until_it_names_it() {
    let dir = tempfile::tempdir().unwrap();
    let last = dir.path().join("last.rs");
    let (tx, rx) = mpsc::channel();
    let mut quiet_waits = 0;
    let wait = changes_through(&last, Duration::from_millis(10), LOADED_TIMEOUT, |left| {
        if last.exists() {
            tx.send(batch(&[&last])).unwrap();
        } else {
            quiet_waits += 1;
        }
        rx.recv_timeout(left)
    });
    assert_eq!(wait.failure, None);
    assert!(wait.seen.contains(&FsChange::Path(last.clone())));
    assert_eq!(quiet_waits, 1);
}

#[test]
fn a_watcher_that_never_names_the_file_fails_at_the_deadline_with_what_it_saw() {
    let dir = tempfile::tempdir().unwrap();
    let last = dir.path().join("last.rs");
    let other = dir.path().join("other.rs");
    let (tx, rx) = mpsc::channel();
    tx.send(batch(&[&other])).unwrap();
    let wait = changes_through(
        &last,
        Duration::from_millis(10),
        Duration::from_millis(60),
        |left| rx.recv_timeout(left),
    );
    assert_eq!(wait.failure, Some("no last file by the deadline"));
    let report = wait.report("watcher");
    assert!(report.contains("other.rs"), "{report}");
    assert!(report.contains("rewrote the file"), "{report}");
}

#[test]
fn a_failure_reports_both_watchers_timelines_and_batch_contents() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let changed = changed_set(root);
    let last = root.join("src/last.rs");
    let named = Wait {
        seen: changed.clone(),
        timeline: vec![format!("1s: batch after 0 rewrites: {:?}", batch(&[&last]))],
        failure: None,
    };
    let silent = Wait {
        seen: HashSet::new(),
        timeline: vec!["3s: quiet, rewrote the file".into()],
        failure: Some("no last file by the deadline"),
    };
    // The direct watcher failing still shows what the host delivered.
    let error = compare(&silent, &named, &changed).unwrap_err();
    assert!(error.contains("the watcher failed"), "{error}");
    assert!(error.contains("last.rs"), "{error}");
    assert!(error.contains("rewrote the file"), "{error}");
    // A host that named the last file but missed a path shows both too.
    let partial = Wait {
        seen: batch(&[&last]).into_iter().collect(),
        timeline: vec!["1s: batch after 0 rewrites: [last]".into()],
        failure: None,
    };
    let error = compare(&named, &partial, &changed).unwrap_err();
    assert!(error.contains("the host missed"), "{error}");
    assert!(error.contains("watcher: named the last file"), "{error}");
    assert!(compare(&named, &named, &changed).is_ok());
}

#[test]
fn a_hosted_watch_delivers_the_watchers_changes() {
    let dir = tempfile::tempdir().unwrap();
    let root = dunce::canonicalize(dir.path()).unwrap();
    write(&root, "src/main.rs", "fn main() {}\n");
    write(&root, "src/old.rs", "");
    write(&root, "src/gone.rs", "");
    let (tx, direct) = mpsc::channel();
    let _watcher = ProjectWatcher::new(&root, move |batch| {
        let _ = tx.send(batch);
    })
    .unwrap();
    let host: &dyn WatchHost = &LocalWatch;
    let hosted = host.watch(&root).unwrap();

    // A create, a rename and a removal, then one file written last, which
    // each watcher reports after the rest.
    fs::write(root.join("src/new.rs"), "").unwrap();
    fs::rename(root.join("src/old.rs"), root.join("src/moved.rs")).unwrap();
    fs::remove_file(root.join("src/gone.rs")).unwrap();
    let last = root.join("src/last.rs");
    fs::write(&last, "").unwrap();

    // Each watcher coalesces into batches as its own thread wakes, so the
    // batches can differ; the changes they hold don't. Both are waited on
    // before anything is judged, so a failure shows both timelines.
    let from_watcher = changes_through(&last, QUIET, LOADED_TIMEOUT, |left| {
        direct.recv_timeout(left)
    });
    let from_host = changes_through(&last, QUIET, LOADED_TIMEOUT, |left| {
        hosted.recv_timeout(left)
    });
    if let Err(error) = compare(&from_watcher, &from_host, &changed_set(&root)) {
        panic!("{error}");
    }
}

#[test]
fn a_hosted_watch_that_cant_start_fails_as_the_watcher_does() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("missing");
    let direct = ProjectWatcher::new(&missing, |_| {});
    let host: &dyn WatchHost = &LocalWatch;
    let hosted = host.watch(&missing);
    assert_eq!(direct.is_err(), hosted.is_err());
    // inotify watches each directory itself, from the root down.
    #[cfg(target_os = "linux")]
    assert_eq!(
        hosted.err().map(|error| error.kind()),
        Some(std::io::ErrorKind::NotFound)
    );
}
