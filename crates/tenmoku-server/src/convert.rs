//! Between the wire's bodies (fe-remote-proto) and fe-host's types, which
//! the local impls take and return. Both are foreign here, so these are
//! functions rather than `From` impls.

use fe_remote_proto as wire;
use fe_remote_proto::PathBytes;
use std::ffi::OsString;
use std::io;
use std::os::unix::ffi::OsStringExt as _;
use std::path::{Path, PathBuf};

pub(crate) fn path(path: &PathBytes) -> PathBuf {
    path.to_path_buf()
}

pub(crate) fn name(name: &PathBytes) -> OsString {
    OsString::from_vec(name.0.clone())
}

pub(crate) fn bytes(path: &Path) -> PathBytes {
    PathBytes::from(path)
}

pub(crate) fn stamp(stamp: &fe_host::DiskStamp) -> wire::DiskStamp {
    wire::DiskStamp {
        len: stamp.len,
        modified: stamp.modified.into(),
        dev: stamp.dev,
        ino: stamp.ino,
    }
}

/// The host's stamp for a wire one; `InvalidInput` for a time this machine
/// can't hold, which no file here has.
pub(crate) fn host_stamp(stamp: &wire::DiskStamp) -> io::Result<fe_host::DiskStamp> {
    let modified = stamp.modified.to_system_time().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "a stamp's time {:?} is out of this host's range",
                stamp.modified
            ),
        )
    })?;
    Ok(fe_host::DiskStamp {
        len: stamp.len,
        modified,
        dev: stamp.dev,
        ino: stamp.ino,
    })
}

pub(crate) fn expect(expect: &wire::WriteExpect) -> io::Result<fe_host::WriteExpect> {
    Ok(match expect {
        wire::WriteExpect::Overwrite => fe_host::WriteExpect::Overwrite,
        wire::WriteExpect::AsFound => fe_host::WriteExpect::AsFound,
        wire::WriteExpect::Absent => fe_host::WriteExpect::Absent,
        wire::WriteExpect::Matches(s) => fe_host::WriteExpect::Matches(host_stamp(s)?),
    })
}

pub(crate) fn file_kind(kind: fe_host::FileKind) -> wire::FileKind {
    match kind {
        fe_host::FileKind::File => wire::FileKind::File,
        fe_host::FileKind::Dir => wire::FileKind::Dir,
        fe_host::FileKind::Symlink => wire::FileKind::Symlink,
        fe_host::FileKind::Other => wire::FileKind::Other,
    }
}

pub(crate) fn stat(stat: &fe_host::Stat) -> wire::Stat {
    wire::Stat {
        kind: file_kind(stat.kind),
        stamp: stamp(&stat.stamp),
    }
}

pub(crate) fn dir_entry(entry: &fe_host::DirEntry) -> wire::DirEntry {
    wire::DirEntry {
        name: PathBytes::from(entry.name.as_os_str()),
        kind: file_kind(entry.kind),
    }
}

/// The request a `ListTree` makes, with no global excludes: the host lists
/// with its own.
pub(crate) fn list_request(list: &wire::ListTree) -> fe_host::ListRequest {
    fe_host::ListRequest {
        root: path(&list.root),
        rel: path(&list.rel),
        show_ignored: list.show_ignored,
        parent_ignored: list.parent_ignored,
        ids: list.ids.clone(),
        global_excludes: None,
    }
}

pub(crate) fn entry(entry: &fe_host::Entry) -> wire::Entry {
    wire::Entry {
        name: PathBytes::from(entry.name.as_os_str()),
        kind: match entry.kind {
            fe_host::EntryKind::File => wire::EntryKind::File,
            fe_host::EntryKind::Dir => wire::EntryKind::Dir,
            fe_host::EntryKind::Symlink => wire::EntryKind::Symlink,
        },
        ignored: entry.ignored,
        id: entry.id,
    }
}

pub(crate) fn listing_lost(lost: fe_host::ListingLost) -> wire::ListingLost {
    match lost {
        fe_host::ListingLost::Itself => wire::ListingLost::Itself,
        fe_host::ListingLost::Above => wire::ListingLost::Above,
        fe_host::ListingLost::Root => wire::ListingLost::Root,
    }
}

pub(crate) fn entry_id(id: &fe_host::EntryId) -> wire::EntryId {
    wire::EntryId {
        dev: id.dev,
        ino: id.ino,
        born: id.born,
    }
}

pub(crate) fn host_entry_id(id: &wire::EntryId) -> fe_host::EntryId {
    fe_host::EntryId {
        dev: id.dev,
        ino: id.ino,
        born: id.born,
    }
}

pub(crate) fn placed(placed: &fe_host::Placed) -> wire::Placed {
    wire::Placed {
        path: bytes(&placed.path),
        id: entry_id(&placed.id),
    }
}

pub(crate) fn created(created: &fe_host::Created) -> wire::Created {
    wire::Created {
        path: bytes(&created.path),
        first: placed(&created.first),
    }
}

pub(crate) fn trash_record(record: &fe_host::TrashRecord) -> wire::TrashRecord {
    wire::TrashRecord {
        path: bytes(&record.path),
        at: record.at.as_deref().map(bytes),
        id: entry_id(&record.id),
    }
}

pub(crate) fn host_trash_record(record: &wire::TrashRecord) -> fe_host::TrashRecord {
    fe_host::TrashRecord {
        path: path(&record.path),
        at: record.at.as_ref().map(path),
        id: host_entry_id(&record.id),
    }
}

pub(crate) fn fs_change(change: &fe_host::FsChange) -> wire::FsChange {
    match change {
        fe_host::FsChange::Path(p) => wire::FsChange::Path(bytes(p)),
        fe_host::FsChange::Rescan => wire::FsChange::Rescan,
    }
}

pub(crate) fn query(query: &wire::SearchQuery) -> fe_host::SearchQuery {
    fe_host::SearchQuery {
        pattern: query.pattern.clone(),
        regex: query.regex,
        case_sensitive: query.case_sensitive,
        whole_word: query.whole_word,
    }
}

pub(crate) fn options(options: &wire::SearchOptions) -> fe_host::SearchOptions {
    fe_host::SearchOptions {
        max_matches: usize::try_from(options.max_matches).unwrap_or(usize::MAX),
        threads: usize::try_from(options.threads).unwrap_or(usize::MAX),
    }
}

pub(crate) fn filter(
    filter: &wire::PathFilter,
    excluded: Option<&PathBytes>,
) -> fe_host::SearchFilter {
    fe_host::SearchFilter {
        include: filter.include.clone(),
        exclude: filter.exclude.clone(),
        excluded: excluded.map(path),
    }
}

fn wide(n: usize) -> u64 {
    u64::try_from(n).unwrap_or(u64::MAX)
}

pub(crate) fn line_match(line: &fe_host::LineMatch) -> wire::LineMatch {
    wire::LineMatch {
        line_number: line.line_number,
        line_offset: line.line_offset,
        ranges: (line.ranges.iter())
            .map(|r| wide(r.start)..wide(r.end))
            .collect(),
        preview: line.preview.clone(),
        preview_start: wide(line.preview_start),
    }
}

pub(crate) fn file_matches(file: &fe_host::FileMatches) -> wire::FileMatches {
    wire::FileMatches {
        path: bytes(&file.path),
        rel: file.rel.clone(),
        lines: file.lines.iter().map(line_match).collect(),
    }
}

pub(crate) fn summary(summary: &fe_host::SearchSummary) -> wire::SearchSummary {
    wire::SearchSummary {
        files_searched: wide(summary.files_searched),
        files_matched: wide(summary.files_matched),
        matches: wide(summary.matches),
        binary_skipped: wide(summary.binary_skipped),
        errors: (summary.errors.iter())
            .map(|(p, message)| (bytes(p), message.clone()))
            .collect(),
        truncated: summary.truncated,
        cancelled: summary.cancelled,
    }
}
