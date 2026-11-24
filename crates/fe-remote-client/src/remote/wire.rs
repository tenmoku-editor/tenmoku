//! Between fe-host's types, which the client's callers use, and the wire's
//! bodies (fe-remote-proto): tenmoku-server's `convert.rs` the other way
//! round. Both are foreign here, so these are functions rather than `From`
//! impls.

use fe_remote_proto as wire;
use fe_remote_proto::PathBytes;
use std::ffi::OsString;
use std::io;
use std::os::unix::ffi::OsStringExt as _;
use std::path::{Path, PathBuf};

pub(super) fn bytes(path: &Path) -> PathBytes {
    PathBytes::from(path)
}

pub(super) fn path(path: &PathBytes) -> PathBuf {
    path.to_path_buf()
}

pub(super) fn name(name: &PathBytes) -> OsString {
    OsString::from_vec(name.0.clone())
}

/// A reply this side can't take: the host sent something no call of this
/// build answers with.
pub(super) fn invalid(what: impl std::fmt::Display) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("the host's reply: {what}"),
    )
}

pub(super) fn stamp(stamp: &fe_host::DiskStamp) -> wire::DiskStamp {
    wire::DiskStamp {
        len: stamp.len,
        modified: stamp.modified.into(),
        dev: stamp.dev,
        ino: stamp.ino,
    }
}

/// The host's stamp as this machine holds it; `InvalidData` for a time it
/// can't.
pub(super) fn host_stamp(stamp: &wire::DiskStamp) -> io::Result<fe_host::DiskStamp> {
    let modified = (stamp.modified.to_system_time())
        .ok_or_else(|| invalid(format_args!("a stamp's time {:?}", stamp.modified)))?;
    Ok(fe_host::DiskStamp {
        len: stamp.len,
        modified,
        dev: stamp.dev,
        ino: stamp.ino,
    })
}

pub(super) fn expect(expect: fe_host::WriteExpect) -> wire::WriteExpect {
    match expect {
        fe_host::WriteExpect::Overwrite => wire::WriteExpect::Overwrite,
        fe_host::WriteExpect::AsFound => wire::WriteExpect::AsFound,
        fe_host::WriteExpect::Absent => wire::WriteExpect::Absent,
        fe_host::WriteExpect::Matches(s) => wire::WriteExpect::Matches(stamp(&s)),
    }
}

pub(super) fn file_kind(kind: wire::FileKind) -> fe_host::FileKind {
    match kind {
        wire::FileKind::File => fe_host::FileKind::File,
        wire::FileKind::Dir => fe_host::FileKind::Dir,
        wire::FileKind::Symlink => fe_host::FileKind::Symlink,
        wire::FileKind::Other => fe_host::FileKind::Other,
    }
}

pub(super) fn stat(stat: &wire::Stat) -> io::Result<fe_host::Stat> {
    Ok(fe_host::Stat {
        kind: file_kind(stat.kind),
        stamp: host_stamp(&stat.stamp)?,
    })
}

pub(super) fn dir_entry(entry: &wire::DirEntry) -> fe_host::DirEntry {
    fe_host::DirEntry {
        name: name(&entry.name),
        kind: file_kind(entry.kind),
    }
}

/// The request a [`fe_host::ListRequest`] makes, less its global excludes,
/// which are this machine's: the host lists with its own.
pub(super) fn list_tree(request: &fe_host::ListRequest) -> wire::ListTree {
    wire::ListTree {
        root: bytes(&request.root),
        rel: bytes(&request.rel),
        show_ignored: request.show_ignored,
        parent_ignored: request.parent_ignored,
        ids: request.ids.clone(),
    }
}

pub(super) fn entry(entry: &wire::Entry) -> fe_host::Entry {
    fe_host::Entry {
        name: name(&entry.name),
        kind: match entry.kind {
            wire::EntryKind::File => fe_host::EntryKind::File,
            wire::EntryKind::Dir => fe_host::EntryKind::Dir,
            wire::EntryKind::Symlink => fe_host::EntryKind::Symlink,
        },
        ignored: entry.ignored,
        id: entry.id,
    }
}

pub(super) fn listing_lost(lost: wire::ListingLost) -> fe_host::ListingLost {
    match lost {
        wire::ListingLost::Itself => fe_host::ListingLost::Itself,
        wire::ListingLost::Above => fe_host::ListingLost::Above,
        wire::ListingLost::Root => fe_host::ListingLost::Root,
    }
}

fn entry_id(id: &fe_host::EntryId) -> wire::EntryId {
    wire::EntryId {
        dev: id.dev,
        ino: id.ino,
        born: id.born,
    }
}

fn host_entry_id(id: &wire::EntryId) -> fe_host::EntryId {
    fe_host::EntryId {
        dev: id.dev,
        ino: id.ino,
        born: id.born,
    }
}

pub(super) fn placed(placed: &wire::Placed) -> fe_host::Placed {
    fe_host::Placed {
        path: path(&placed.path),
        id: host_entry_id(&placed.id),
    }
}

pub(super) fn created(created: &wire::Created) -> fe_host::Created {
    fe_host::Created {
        path: path(&created.path),
        first: placed(&created.first),
    }
}

pub(super) fn trash_record(record: &fe_host::TrashRecord) -> wire::TrashRecord {
    wire::TrashRecord {
        path: bytes(&record.path),
        at: record.at.as_deref().map(bytes),
        id: entry_id(&record.id),
    }
}

pub(super) fn host_trash_record(record: &wire::TrashRecord) -> fe_host::TrashRecord {
    fe_host::TrashRecord {
        path: path(&record.path),
        at: record.at.as_ref().map(path),
        id: host_entry_id(&record.id),
    }
}

pub(super) fn fs_change(change: &wire::FsChange) -> fe_host::FsChange {
    match change {
        wire::FsChange::Path(p) => fe_host::FsChange::Path(path(p)),
        wire::FsChange::Rescan => fe_host::FsChange::Rescan,
    }
}

pub(super) fn query(query: &fe_host::SearchQuery) -> wire::SearchQuery {
    wire::SearchQuery {
        pattern: query.pattern.clone(),
        regex: query.regex,
        case_sensitive: query.case_sensitive,
        whole_word: query.whole_word,
    }
}

pub(super) fn options(options: &fe_host::SearchOptions) -> wire::SearchOptions {
    wire::SearchOptions {
        max_matches: u64::try_from(options.max_matches).unwrap_or(u64::MAX),
        threads: u32::try_from(options.threads).unwrap_or(u32::MAX),
    }
}

/// The filter's globs, and its excluded file apart, as the ops carry them.
pub(super) fn filter(filter: &fe_host::SearchFilter) -> (wire::PathFilter, Option<PathBytes>) {
    let globs = wire::PathFilter {
        include: filter.include.clone(),
        exclude: filter.exclude.clone(),
    };
    (globs, filter.excluded.as_deref().map(bytes))
}

fn narrow(n: u64) -> usize {
    usize::try_from(n).unwrap_or(usize::MAX)
}

fn line_match(line: &wire::LineMatch) -> fe_host::LineMatch {
    fe_host::LineMatch {
        line_number: line.line_number,
        line_offset: line.line_offset,
        ranges: (line.ranges.iter())
            .map(|r| narrow(r.start)..narrow(r.end))
            .collect(),
        preview: line.preview.clone(),
        preview_start: narrow(line.preview_start),
    }
}

pub(super) fn file_matches(file: &wire::FileMatches) -> fe_host::FileMatches {
    fe_host::FileMatches {
        path: path(&file.path),
        rel: file.rel.clone(),
        lines: file.lines.iter().map(line_match).collect(),
    }
}

pub(super) fn summary(summary: &wire::SearchSummary) -> fe_host::SearchSummary {
    fe_host::SearchSummary {
        files_searched: narrow(summary.files_searched),
        files_matched: narrow(summary.files_matched),
        matches: narrow(summary.matches),
        binary_skipped: narrow(summary.binary_skipped),
        errors: (summary.errors.iter())
            .map(|(p, message)| (path(p), message.clone()))
            .collect(),
        truncated: summary.truncated,
        cancelled: summary.cancelled,
    }
}

pub(super) fn search_failed(failure: wire::SearchFailure) -> fe_host::SearchFailed {
    match failure {
        wire::SearchFailure::InvalidPattern(message) => {
            fe_host::SearchFailed::InvalidPattern(message)
        }
        wire::SearchFailure::InvalidGlob { glob, message } => {
            fe_host::SearchFailed::InvalidGlob { glob, message }
        }
    }
}
