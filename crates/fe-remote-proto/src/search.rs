//! Channel 3's bodies: `SearchHost` (§ 5), over fe-project's search and
//! the finder's file list.

use crate::PathBytes;
use serde::{Deserialize, Serialize};
use std::ops::Range;

/// A `Request` body on channel 3: fe-host's `SearchHost` calls, each a
/// stream the host feeds as its local impl finds results. `Cancel` stops
/// it, and it ends with `Status::Cancelled`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SearchOp {
    /// Searches the files under `root` that `filter` lets through. Replies
    /// [`SearchData::Matches`] batches, then one [`SearchData::Summary`].
    /// `excluded`, relative to `root`, is a file searched another way (from
    /// a buffer): it isn't opened, as in fe-project's `search_excluding`.
    Search {
        root: PathBytes,
        query: SearchQuery,
        options: SearchOptions,
        filter: PathFilter,
        excluded: Option<PathBytes>,
    },
    /// Lists the files under `root` that `filter` lets through, less
    /// `excluded`, for Quick Open. Replies [`SearchData::Files`] batches of
    /// [`FILES_BATCH`] paths, the last fewer, and fewer where that many
    /// long paths wouldn't fit in a frame.
    Files {
        root: PathBytes,
        filter: PathFilter,
        excluded: Option<PathBytes>,
    },
}

/// The most paths one [`SearchData::Files`] batch holds (D § 12: the walk
/// streams in batches of 512).
pub const FILES_BATCH: usize = 512;

/// A `Data` body on channel 3. Each is at most about [`crate::CHUNK`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SearchData {
    /// Files' matches, in the order the host found them. A file with more
    /// matches than fit in one frame comes as consecutive entries of the
    /// same path, its lines in order; a line with more ranges than fit, as
    /// consecutive entries of the same line, its ranges in order.
    Matches(Vec<FileMatches>),
    /// The search's end. Its `errors` come before it, in `Errors` batches,
    /// and are empty here.
    Summary(SearchSummary),
    /// Files the search couldn't search, with why: [`SearchSummary`]'s
    /// errors, in order. Each message is at most 4 KiB.
    Errors(Vec<(PathBytes, String)>),
    /// Paths relative to the root.
    Files(Vec<PathBytes>),
    /// Why the search or file list didn't run, before its `End` with the
    /// error: what fe-host's `SearchFailed` names beyond an I/O error.
    Failed(SearchFailure),
}

/// fe-host's `SearchFailed`, less its I/O error, which is the `End`'s. Each
/// text is at most 4 KiB, cut on a char boundary.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SearchFailure {
    /// The pattern isn't a valid regex, or names a line break.
    InvalidPattern(String),
    /// A glob of the filter doesn't parse.
    InvalidGlob { glob: String, message: String },
}

/// fe-project's `SearchQuery`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchQuery {
    pub pattern: String,
    pub regex: bool,
    pub case_sensitive: bool,
    pub whole_word: bool,
}

/// fe-project's `SearchOptions`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchOptions {
    pub max_matches: u64,
    /// Walker threads on the host; 0 is the walker's default.
    pub threads: u32,
}

/// fe-project's `PathFilter`, as the globs it is built from.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PathFilter {
    pub include: Vec<String>,
    pub exclude: Vec<String>,
}

/// fe-project's `FileMatches`: a file's matching lines, in line order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileMatches {
    pub path: PathBytes,
    /// Relative to the root, `/`-separated, lossy where a name isn't UTF-8.
    pub rel: String,
    pub lines: Vec<LineMatch>,
}

/// fe-project's `LineMatch`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineMatch {
    /// 1-based.
    pub line_number: u64,
    pub line_offset: u64,
    /// Byte ranges in the line.
    pub ranges: Vec<Range<u64>>,
    pub preview: String,
    pub preview_start: u64,
}

/// fe-project's `SearchSummary`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchSummary {
    pub files_searched: u64,
    pub files_matched: u64,
    pub matches: u64,
    pub binary_skipped: u64,
    pub errors: Vec<(PathBytes, String)>,
    pub truncated: bool,
    pub cancelled: bool,
}
