//! Channel 3 (D § 2.2, § 5, § 12): a search streams the `FileMatches`
//! fe-project's `LocalSearch` finds, a files request streams the walk in
//! batches of 512, and a cancel ends either with the data after it within
//! the window.

#![cfg(unix)]

mod common;

use fe_host::{SearchHost, SearchStream, Stream as HostStream};
use fe_project::LocalSearch;
use fe_remote_proto::window::MIN_WINDOW;
use fe_remote_proto::{
    CHUNK, Config as MuxConfig, Event, FILES_BATCH, FileMatches, LineMatch, Link, Mux, PathBytes,
    PathFilter, SearchData, SearchFailure, SearchOp, SearchOptions, SearchQuery, SearchSummary,
    Status, Stream, chan, encoded_len,
};
use std::path::Path;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use tempfile::TempDir;
use tenmoku_server::{Config, Hosts, serve};

fn connect(search: Arc<dyn SearchHost>, client: MuxConfig) -> Mux {
    let config = Config {
        hosts: Arc::new(move || Hosts {
            search: search.clone(),
            ..Hosts::local()
        }),
        ..Config::default()
    };
    let (client_end, server_end) = Link::pipe().unwrap();
    thread::spawn(move || serve(server_end, config));
    common::handshake(client_end, client, None).unwrap().0
}

fn local() -> Mux {
    connect(Arc::new(LocalSearch), common::client_config())
}

/// A client with a small window, so a reply it doesn't read soon runs out
/// of credit.
fn local_small_window() -> Mux {
    connect(
        Arc::new(LocalSearch),
        MuxConfig {
            window: MIN_WINDOW,
            ..common::client_config()
        },
    )
}

/// A project of `files`, each `(rel, contents)`.
fn project(files: impl IntoIterator<Item = (String, String)>) -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    for (rel, contents) in files {
        let path = dir.path().join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }
    dir
}

fn literal(pattern: &str) -> SearchQuery {
    SearchQuery {
        pattern: pattern.into(),
        regex: false,
        case_sensitive: false,
        whole_word: false,
    }
}

/// One walker thread, so the host's results come in path order.
const IN_ORDER: SearchOptions = SearchOptions {
    max_matches: 10_000,
    threads: 1,
};

fn search(root: &Path, query: SearchQuery, filter: PathFilter, excluded: Option<&str>) -> SearchOp {
    SearchOp::Search {
        root: PathBytes::from(root),
        query,
        options: IN_ORDER,
        filter,
        excluded: excluded.map(PathBytes::from),
    }
}

fn reply(mut stream: Stream) -> (Vec<SearchData>, Status) {
    let mut data = Vec::new();
    loop {
        match stream.recv() {
            Some(Event::Data(bytes)) => data.push(stream.decode(&bytes).unwrap()),
            Some(Event::End(status)) => return (data, status),
            None => panic!("the reply ended without an End"),
        }
    }
}

fn next(stream: &mut Stream) -> SearchData {
    match stream.recv() {
        Some(Event::Data(bytes)) => stream.decode(&bytes).unwrap(),
        ev => panic!("{ev:?}"),
    }
}

/// Cancels `stream`, and counts the data bytes that still come before its
/// `End`.
fn cancel(mut stream: Stream) -> (u64, Status) {
    stream.cancel();
    let mut after = 0;
    loop {
        match stream.recv() {
            Some(Event::Data(bytes)) => after += bytes.len() as u64,
            Some(Event::End(status)) => return (after, status),
            None => panic!("the reply ended without an End"),
        }
    }
}

fn wire_line(line: &fe_host::LineMatch) -> LineMatch {
    LineMatch {
        line_number: line.line_number,
        line_offset: line.line_offset,
        ranges: (line.ranges.iter())
            .map(|r| r.start as u64..r.end as u64)
            .collect(),
        preview: line.preview.clone(),
        preview_start: line.preview_start as u64,
    }
}

fn wire_file(file: &fe_host::FileMatches) -> FileMatches {
    FileMatches {
        path: PathBytes::from(file.path.as_path()),
        rel: file.rel.clone(),
        lines: file.lines.iter().map(wire_line).collect(),
    }
}

fn wire_summary(summary: &fe_host::SearchSummary) -> SearchSummary {
    SearchSummary {
        files_searched: summary.files_searched as u64,
        files_matched: summary.files_matched as u64,
        matches: summary.matches as u64,
        binary_skipped: summary.binary_skipped as u64,
        errors: (summary.errors.iter())
            .map(|(p, m)| (PathBytes::from(p.as_path()), m.clone()))
            .collect(),
        truncated: summary.truncated,
        cancelled: summary.cancelled,
    }
}

/// What `LocalSearch` finds here, as the wire has it.
fn local_search(
    root: &Path,
    pattern: &str,
    filter: fe_host::SearchFilter,
) -> (Vec<FileMatches>, SearchSummary) {
    let options = fe_host::SearchOptions {
        max_matches: 10_000,
        threads: 1,
    };
    let mut stream = LocalSearch.search(
        root,
        &fe_host::SearchQuery::literal(pattern),
        &options,
        &filter,
    );
    let files: Vec<FileMatches> = stream.by_ref().flatten().map(|f| wire_file(&f)).collect();
    let summary = stream.finish().unwrap().unwrap();
    (files, wire_summary(&summary))
}

/// The matches and summary a search reply carried, which ended well: the
/// summary with the errors that came before it.
fn found(reply: (Vec<SearchData>, Status)) -> (Vec<FileMatches>, SearchSummary) {
    let (data, status) = reply;
    assert_eq!(status, Status::Ok, "{data:?}");
    let (last, batches) = data.split_last().unwrap();
    let SearchData::Summary(summary) = last else {
        panic!("{last:?}");
    };
    let mut summary = summary.clone();
    assert!(summary.errors.is_empty(), "{summary:?}");
    let mut files = Vec::new();
    for body in batches {
        match body {
            SearchData::Matches(batch) => files.extend(batch.iter().cloned()),
            SearchData::Errors(errors) => summary.errors.extend(errors.iter().cloned()),
            d => panic!("{d:?}"),
        }
    }
    (files, summary)
}

/// Every body of `data` fits in a frame.
fn all_fit(data: &[SearchData]) {
    for body in data {
        assert!(encoded_len(body).unwrap() <= CHUNK);
    }
}

#[test]
fn a_search_streams_the_file_matches_the_local_search_finds() {
    let dir = project([
        ("a.rs".into(), "fn needle() {}\nfn other() {}\n".into()),
        (
            "src/b.rs".into(),
            "// a Needle\nlet needle = 1; needle\n".into(),
        ),
        ("src/c.rs".into(), "nothing here\n".into()),
    ]);
    let client = local();
    let op = search(dir.path(), literal("needle"), PathFilter::default(), None);
    let remote = found(reply(client.request(chan::SEARCH, &op).unwrap()));
    let local = local_search(dir.path(), "needle", fe_host::SearchFilter::default());
    assert_eq!(remote, local);
    assert_eq!(remote.0.len(), 2, "{remote:?}");
}

#[test]
fn a_search_takes_its_filter_and_its_excluded_file_together() {
    let dir = project([
        ("a.rs".into(), "needle\n".into()),
        ("b.rs".into(), "needle\n".into()),
        ("c.txt".into(), "needle\n".into()),
        ("skip/d.rs".into(), "needle\n".into()),
    ]);
    let client = local();
    let filter = PathFilter {
        include: vec!["*.rs".into()],
        exclude: vec!["skip".into()],
    };
    let op = search(dir.path(), literal("needle"), filter, Some("b.rs"));
    let remote = found(reply(client.request(chan::SEARCH, &op).unwrap()));
    let local = local_search(
        dir.path(),
        "needle",
        fe_host::SearchFilter {
            include: vec!["*.rs".into()],
            exclude: vec!["skip".into()],
            excluded: Some("b.rs".into()),
        },
    );
    assert_eq!(remote, local);
    let rels: Vec<&str> = remote.0.iter().map(|f| f.rel.as_str()).collect();
    assert_eq!(rels, ["a.rs"]);
}

#[test]
fn a_file_with_more_matches_than_fit_in_a_frame_comes_as_consecutive_entries() {
    let line = format!("needle {}\n", "x".repeat(400));
    let dir = project([("big.txt".into(), line.repeat(400))]);
    let client = local();
    let op = search(dir.path(), literal("needle"), PathFilter::default(), None);
    let (data, status) = reply(client.request(chan::SEARCH, &op).unwrap());
    all_fit(&data);
    let (remote, _) = found((data, status));
    assert!(remote.len() > 1, "{} entries", remote.len());
    assert!(remote.iter().all(|f| f.rel == "big.txt"));
    let lines: Vec<LineMatch> = remote.into_iter().flat_map(|f| f.lines).collect();
    let (local, _) = local_search(dir.path(), "needle", fe_host::SearchFilter::default());
    let [local] = <[FileMatches; 1]>::try_from(local).unwrap();
    assert_eq!(lines, local.lines);
}

#[test]
fn an_invalid_pattern_is_named_before_its_error() {
    let dir = project([]);
    let client = local();
    let query = SearchQuery {
        regex: true,
        ..literal("(")
    };
    let op = search(dir.path(), query, PathFilter::default(), None);
    let (data, status) = reply(client.request(chan::SEARCH, &op).unwrap());
    assert!(
        matches!(
            &data[..],
            [SearchData::Failed(SearchFailure::InvalidPattern(_))]
        ),
        "{data:?}"
    );
    assert!(matches!(status, Status::Error(_)), "{status:?}");
}

fn files(root: &Path) -> SearchOp {
    SearchOp::Files {
        root: PathBytes::from(root),
        filter: PathFilter::default(),
        excluded: None,
    }
}

/// `count` empty files under `dir/`, with names `pad` long or so.
fn many_files(count: usize, pad: usize) -> TempDir {
    let long = "n".repeat(pad);
    project((0..count).map(|i| (format!("dir{}/{long}{i:05}", i % 7), String::new())))
}

#[test]
fn a_files_request_streams_the_walk_in_batches_of_512() {
    let dir = many_files(1300, 0);
    let client = local();
    let (data, status) = reply(client.request(chan::SEARCH, &files(dir.path())).unwrap());
    assert_eq!(status, Status::Ok);
    let batches: Vec<Vec<PathBytes>> = (data.into_iter())
        .map(|d| match d {
            SearchData::Files(paths) => paths,
            d => panic!("{d:?}"),
        })
        .collect();
    let sizes: Vec<usize> = batches.iter().map(Vec::len).collect();
    assert_eq!(sizes, [FILES_BATCH, FILES_BATCH, 1300 - 2 * FILES_BATCH]);

    let mut remote: Vec<PathBytes> = batches.into_iter().flatten().collect();
    let mut local: Vec<PathBytes> = (LocalSearch
        .files(dir.path(), &fe_host::SearchFilter::default()))
    .flatten()
    .map(|p| PathBytes::from(p.as_path()))
    .collect();
    remote.sort();
    local.sort();
    assert_eq!(remote.len(), 1300);
    assert_eq!(remote, local);
}

#[test]
fn cancelling_a_files_request_after_the_first_batch_ends_it_within_the_window() {
    // About 1 MiB of paths, four windows' worth.
    let dir = many_files(5000, 200);
    let client = local_small_window();
    let mut stream = client.request(chan::SEARCH, &files(dir.path())).unwrap();
    assert!(matches!(next(&mut stream), SearchData::Files(_)));
    let (after, status) = cancel(stream);
    assert_eq!(status, Status::Cancelled);
    assert!(
        after <= u64::from(MIN_WINDOW),
        "{after} bytes after the cancel"
    );
}

#[test]
fn cancelling_a_search_after_the_first_batch_ends_it_within_the_window() {
    let line = format!("needle {}\n", "x".repeat(300));
    let dir = project((0..3000).map(|i| (format!("f{i:04}.txt"), line.clone())));
    let client = local_small_window();
    let op = search(dir.path(), literal("needle"), PathFilter::default(), None);
    let mut stream = client.request(chan::SEARCH, &op).unwrap();
    assert!(matches!(next(&mut stream), SearchData::Matches(_)));
    let (after, status) = cancel(stream);
    assert_eq!(status, Status::Cancelled);
    assert!(
        after <= u64::from(MIN_WINDOW),
        "{after} bytes after the cancel"
    );
}

/// A search host whose search sends one batch, then finds nothing more
/// until it is cancelled, as a search of a big tree with few matches does.
#[derive(Default)]
struct SparseSearch {
    stopped: Mutex<Option<mpsc::Sender<()>>>,
}

impl SearchHost for SparseSearch {
    fn search(
        &self,
        root: &Path,
        _: &fe_host::SearchQuery,
        _: &fe_host::SearchOptions,
        _: &fe_host::SearchFilter,
    ) -> SearchStream {
        let (tx, stream) = HostStream::channel(1);
        let stopped = self.stopped.lock().unwrap().take().unwrap();
        let first = fe_host::FileMatches {
            path: root.join("a.rs"),
            rel: "a.rs".into(),
            lines: Vec::new(),
        };
        thread::spawn(move || {
            tx.send(vec![first]);
            while !tx.is_cancelled() {
                thread::yield_now();
            }
            tx.end(Ok(fe_host::SearchSummary::default()));
            stopped.send(()).unwrap();
        });
        stream
    }

    fn files(&self, _: &Path, _: &fe_host::SearchFilter) -> fe_host::FilesStream {
        HostStream::ended_with(Ok(fe_host::FilesSummary { complete: true }))
    }
}

#[test]
fn a_cancel_stops_the_hosts_search_between_its_results() {
    let (stopped_tx, stopped) = mpsc::channel();
    let host = SparseSearch {
        stopped: Mutex::new(Some(stopped_tx)),
    };
    let client = connect(Arc::new(host), common::client_config());
    let op = search(
        Path::new("/project"),
        literal("x"),
        PathFilter::default(),
        None,
    );
    let mut stream = client.request(chan::SEARCH, &op).unwrap();
    assert!(matches!(next(&mut stream), SearchData::Matches(_)));
    let (_, status) = cancel(stream);
    assert_eq!(status, Status::Cancelled);
    stopped.recv().unwrap();
}

/// A search host whose search finds nothing and ends with `summary`.
struct Summarised(fe_host::SearchSummary);

impl SearchHost for Summarised {
    fn search(
        &self,
        _: &Path,
        _: &fe_host::SearchQuery,
        _: &fe_host::SearchOptions,
        _: &fe_host::SearchFilter,
    ) -> SearchStream {
        HostStream::ended_with(Ok(self.0.clone()))
    }

    fn files(&self, _: &Path, _: &fe_host::SearchFilter) -> fe_host::FilesStream {
        HostStream::ended_with(Ok(fe_host::FilesSummary { complete: true }))
    }
}

#[test]
fn a_summary_with_more_errors_than_fit_in_a_frame_comes_whole() {
    let errors: Vec<(std::path::PathBuf, String)> = (0..3000)
        .map(|i| {
            let path = format!("/project/src/unreadable-{i:04}.rs").into();
            (path, format!("Permission denied (os error 13), file {i}"))
        })
        .collect();
    let summary = fe_host::SearchSummary {
        files_searched: 3000,
        errors: errors.clone(),
        ..fe_host::SearchSummary::default()
    };
    let client = connect(
        Arc::new(Summarised(summary.clone())),
        common::client_config(),
    );
    let op = search(
        Path::new("/project"),
        literal("x"),
        PathFilter::default(),
        None,
    );
    let (data, status) = reply(client.request(chan::SEARCH, &op).unwrap());
    all_fit(&data);
    assert!(data.len() > 2, "{} frames", data.len());
    let (files, remote) = found((data, status));
    assert!(files.is_empty());
    assert_eq!(remote, wire_summary(&summary));
}

#[test]
fn a_long_invalid_pattern_is_named_within_a_frame() {
    let dir = project([]);
    let client = local();
    let query = SearchQuery {
        regex: true,
        ..literal(&format!("({}", "a".repeat(300_000)))
    };
    let op = search(dir.path(), query, PathFilter::default(), None);
    let (data, status) = reply(client.request(chan::SEARCH, &op).unwrap());
    all_fit(&data);
    let [SearchData::Failed(SearchFailure::InvalidPattern(message))] = &data[..] else {
        panic!("{data:?}");
    };
    assert!(message.len() <= 4096, "{} bytes", message.len());
    let Status::Error(e) = status else {
        panic!("{status:?}");
    };
    assert!(e.message.len() <= 4096, "{} bytes", e.message.len());
}

/// A search host whose search sends `files` in one batch and ends.
struct Found(Vec<fe_host::FileMatches>);

impl SearchHost for Found {
    fn search(
        &self,
        _: &Path,
        _: &fe_host::SearchQuery,
        _: &fe_host::SearchOptions,
        _: &fe_host::SearchFilter,
    ) -> SearchStream {
        let (tx, stream) = HostStream::channel(1);
        tx.send(self.0.clone());
        tx.end(Ok(fe_host::SearchSummary::default()));
        stream
    }

    fn files(&self, _: &Path, _: &fe_host::SearchFilter) -> fe_host::FilesStream {
        HostStream::ended_with(Ok(fe_host::FilesSummary { complete: true }))
    }
}

#[test]
#[allow(clippy::single_range_in_vec_init)]
fn matches_under_a_long_path_that_isnt_utf_8_fit_in_frames() {
    use std::os::unix::ffi::OsStrExt as _;
    // A host's own: APFS refuses names that aren't UTF-8, and Darwin's
    // paths stop at 1024 bytes, so a fake host reports the file.
    let name = std::ffi::OsStr::from_bytes(&[0xff; 250]);
    let mut path = Path::new("/project").to_path_buf();
    for _ in 0..12 {
        path.push(name);
    }
    path.push("big.txt");
    let rel = path.strip_prefix("/project").unwrap().to_string_lossy();
    // Each byte 0xff is three bytes of U+FFFD in the lossy rel.
    assert!(rel.len() > 9000, "{} bytes", rel.len());
    let preview = format!("needle {}", "x".repeat(400));
    let lines = (0..400)
        .map(|i| fe_host::LineMatch {
            line_number: i + 1,
            line_offset: i * (preview.len() as u64 + 1),
            ranges: vec![0..6],
            preview: preview.clone(),
            preview_start: 0,
        })
        .collect();
    let file = fe_host::FileMatches {
        path: path.clone(),
        rel: rel.into_owned(),
        lines,
    };
    let client = connect(Arc::new(Found(vec![file.clone()])), common::client_config());
    let op = search(
        Path::new("/project"),
        literal("needle"),
        PathFilter::default(),
        None,
    );
    let (data, status) = reply(client.request(chan::SEARCH, &op).unwrap());
    all_fit(&data);
    let (remote, _) = found((data, status));
    assert!(remote.len() > 1, "{} entries", remote.len());
    let local = wire_file(&file);
    for entry in &remote {
        assert_eq!((&entry.path, &entry.rel), (&local.path, &local.rel));
    }
    let lines: Vec<LineMatch> = remote.into_iter().flat_map(|f| f.lines).collect();
    assert_eq!(lines, local.lines);
}
