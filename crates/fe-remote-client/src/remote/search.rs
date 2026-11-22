//! [`SearchHost`] over channel 3: a `Search` or a `Files` request, whose
//! `Data` frames a thread feeds into the host stream as they come (D § 5,
//! § 11: Quick Open fills progressively, Find in Files streams).
//!
//! What one host batch was can't be seen at this end, so each batch here
//! is what came in one go: a frame, and up to [`JOINED_MOST`] frames
//! already waiting behind it, so a file whose matches the server split
//! across frames is one entry again when its pieces come together.
//!
//! Dropping the stream cancels the request. A host stream says nothing
//! when it's dropped, so the thread looks every [`STOP_POLL`] while it
//! waits for the host. A request whose link drops before it has delivered
//! anything runs once more on the next connection, as a read does; one
//! that has ends with the link's error, as the search panel's
//! "interrupted" (D § 11).

use super::conn::{Attempt, Conn, Failed, STOP_POLL};
use super::fs::{ended, request};
use super::{RemoteHost, Wakeup, wire};
use fe_host::{
    FileMatches, FilesStream, FilesSummary, SearchFailed, SearchFilter, SearchHost, SearchOptions,
    SearchQuery, SearchStream, SearchSummary, Stream as HostStream, StreamSender,
};
use fe_remote_proto::{Event, SearchData, SearchFailure, SearchOp, Status, chan};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::task::{Context, Poll, Waker};

/// Batches the host stream holds before the thread waits for its reader,
/// as fe-project's `LocalSearch` holds.
const STREAMED: usize = 2;

/// The most frames one batch is made of: past this it's delivered though
/// more are waiting, so a walk that never pauses still streams.
const JOINED_MOST: usize = 16;

impl SearchHost for RemoteHost {
    fn search(
        &self,
        root: &Path,
        query: &SearchQuery,
        options: &SearchOptions,
        filter: &SearchFilter,
    ) -> SearchStream {
        let (globs, excluded) = wire::filter(filter);
        let op = SearchOp::Search {
            root: wire::bytes(root),
            query: wire::query(query),
            options: wire::options(options),
            filter: globs,
            excluded,
        };
        stream::<Matches>(&self.conn, "remote-search", op)
    }

    fn files(&self, root: &Path, filter: &SearchFilter) -> FilesStream {
        let (globs, excluded) = wire::filter(filter);
        let op = SearchOp::Files {
            root: wire::bytes(root),
            filter: globs,
            excluded,
        };
        stream::<Files>(&self.conn, "remote-files", op)
    }
}

/// What one request's reply makes of its bodies, and how it ends.
trait Collect: Default + Send + 'static {
    type Item: Send + 'static;
    type End: Send + 'static;

    /// A body of the reply: the batch it carries, if it carries one.
    fn data(&mut self, data: SearchData) -> io::Result<Option<Vec<Self::Item>>>;

    /// Adds `batch` to the batch waiting to be delivered.
    fn merge(waiting: &mut Vec<Self::Item>, batch: Vec<Self::Item>);

    /// The end value for the reply's `End`, other than a lost link.
    fn end(self, status: Status) -> Result<Self::End, SearchFailed>;

    /// The end value once the stream was dropped.
    fn cancelled(self) -> Self::End;
}

type Sender<C> = StreamSender<Vec<<C as Collect>::Item>, Result<<C as Collect>::End, SearchFailed>>;

/// Runs `op` on a thread named `name`, feeding a host stream.
fn stream<C: Collect>(
    conn: &Conn,
    name: &str,
    op: SearchOp,
) -> HostStream<Vec<C::Item>, Result<C::End, SearchFailed>> {
    let (sender, stream) = HostStream::channel(STREAMED);
    let conn = conn.clone();
    let started = std::thread::Builder::new()
        .name(name.into())
        .spawn(move || serve::<C>(&conn, &op, sender));
    match started {
        Ok(_) => stream,
        Err(error) => HostStream::ended_with(Err(SearchFailed::Io(error))),
    }
}

/// How one run of the request went.
enum Ran<C: Collect> {
    Ended(Result<C::End, SearchFailed>),
    /// The link went first.
    Lost,
    /// The stream was dropped.
    Stopped(C),
}

fn serve<C: Collect>(conn: &Conn, op: &SearchOp, sender: Sender<C>) {
    let mut attempt = match conn.live() {
        Ok(attempt) => attempt,
        Err(error) => return sender.end(Err(SearchFailed::Io(error))),
    };
    let mut delivered = false;
    let mut retried = false;
    let end = loop {
        match run::<C>(&attempt, op, &sender, &mut delivered) {
            Ran::Ended(end) => break end,
            Ran::Stopped(collected) => break Ok(collected.cancelled()),
            Ran::Lost if delivered || retried => break Err(SearchFailed::Io(attempt.lost())),
            Ran::Lost => {
                match conn.next_unless(&attempt, || sender.is_cancelled()) {
                    Ok(Some(next)) => attempt = next,
                    Ok(None) => break Ok(C::default().cancelled()),
                    Err(error) => break Err(SearchFailed::Io(error)),
                }
                retried = true;
            }
        }
    };
    sender.end(end);
}

/// One run of `op` on `attempt`'s connection, delivering what comes.
fn run<C: Collect>(
    attempt: &Attempt,
    op: &SearchOp,
    sender: &Sender<C>,
    delivered: &mut bool,
) -> Ran<C> {
    let mut collected = C::default();
    let mut stream = match request(attempt.mux(), chan::SEARCH, op) {
        Ok(stream) => stream,
        Err(Failed::Lost) => return Ran::Lost,
        Err(Failed::Io(error)) => return Ran::Ended(Err(SearchFailed::Io(error))),
    };
    let wake = Arc::new(Wakeup::default());
    let waker = Waker::from(wake.clone());
    let mut cx = Context::from_waker(&waker);
    // A batch taken from the frames that came in one go, and how many.
    let mut waiting: Option<Vec<C::Item>> = None;
    let mut joined = 0;
    loop {
        if sender.is_cancelled() {
            // Dropping the request cancels it.
            return Ran::Stopped(collected);
        }
        let polled = match joined {
            JOINED_MOST.. => Poll::Pending,
            _ => stream.poll_recv(&mut cx),
        };
        match polled {
            Poll::Ready(Some(Event::Data(bytes))) => {
                let batch = (stream.decode::<SearchData>(&bytes))
                    .map_err(wire::invalid)
                    .and_then(|data| collected.data(data));
                match batch {
                    Ok(Some(batch)) => {
                        match &mut waiting {
                            Some(waiting) => C::merge(waiting, batch),
                            None => waiting = Some(batch),
                        }
                        joined += 1;
                    }
                    Ok(None) => {}
                    Err(error) => return Ran::Ended(Err(SearchFailed::Io(error))),
                }
            }
            Poll::Pending => match waiting.take() {
                Some(batch) => {
                    joined = 0;
                    if !sender.send(batch) {
                        return Ran::Stopped(collected);
                    }
                    *delivered = true;
                }
                None => wake.wait(Some(STOP_POLL)),
            },
            Poll::Ready(Some(Event::End(Status::LinkLost)) | None) => {
                // What came on a run that ran again comes again.
                if let Some(batch) = waiting.take().filter(|_| *delivered) {
                    sender.send(batch);
                }
                return Ran::Lost;
            }
            Poll::Ready(Some(Event::End(status))) => {
                if let Some(batch) = waiting.take() {
                    if !sender.send(batch) {
                        return Ran::Stopped(collected);
                    }
                    *delivered = true;
                }
                return Ran::Ended(collected.end(status));
            }
        }
    }
}

/// A failed reply's end: what the host named before it, or its error.
fn failed(failure: Option<SearchFailure>, status: Status) -> SearchFailed {
    match (failure, ended(status)) {
        (Some(failure), _) => wire::search_failed(failure),
        (None, Err(Failed::Io(error))) => SearchFailed::Io(error),
        (None, _) => SearchFailed::Io(wire::invalid("a search ended with no outcome")),
    }
}

/// A `Search`'s reply.
#[derive(Default)]
struct Matches {
    summary: Option<SearchSummary>,
    errors: Vec<(PathBuf, String)>,
    failure: Option<SearchFailure>,
    /// What was delivered, for the summary of a search cut short.
    files_matched: usize,
    matches: usize,
}

impl Collect for Matches {
    type Item = FileMatches;
    type End = SearchSummary;

    fn data(&mut self, data: SearchData) -> io::Result<Option<Vec<FileMatches>>> {
        match data {
            SearchData::Matches(files) if self.summary.is_none() => {
                let files: Vec<_> = files.iter().map(wire::file_matches).collect();
                self.files_matched += files.len();
                self.matches += (files.iter().flat_map(|f| &f.lines))
                    .map(|l| l.ranges.len())
                    .sum::<usize>();
                let mut batch = Vec::new();
                Self::merge(&mut batch, files);
                return Ok(Some(batch));
            }
            SearchData::Errors(errors) if self.summary.is_none() => self
                .errors
                .extend((errors.iter()).map(|(path, message)| (wire::path(path), message.clone()))),
            SearchData::Summary(summary) if self.summary.is_none() => {
                self.summary = Some(wire::summary(&summary));
            }
            SearchData::Failed(failure) => self.failure = Some(failure),
            data => return Err(wire::invalid(format_args!("unexpected {data:?}"))),
        }
        Ok(None)
    }

    /// Adds `batch`'s files, a file that continues the last one waiting
    /// joining it, and a line that continues its last line joining that.
    fn merge(waiting: &mut Vec<FileMatches>, batch: Vec<FileMatches>) {
        for file in batch {
            let Some(last) = waiting.last_mut().filter(|last| last.path == file.path) else {
                waiting.push(file);
                continue;
            };
            for line in file.lines {
                match last.lines.last_mut() {
                    Some(prev) if prev.line_number == line.line_number => {
                        prev.ranges.extend(line.ranges);
                    }
                    _ => last.lines.push(line),
                }
            }
        }
    }

    fn end(self, status: Status) -> Result<SearchSummary, SearchFailed> {
        match (status, self.summary) {
            (Status::Ok, Some(mut summary)) => {
                summary.errors = self.errors;
                Ok(summary)
            }
            (Status::Cancelled, _) => Ok(SearchSummary {
                errors: self.errors,
                ..Self::cut_short(self.files_matched, self.matches)
            }),
            (status, _) => Err(failed(self.failure, status)),
        }
    }

    fn cancelled(self) -> SearchSummary {
        SearchSummary {
            errors: self.errors,
            ..Self::cut_short(self.files_matched, self.matches)
        }
    }
}

impl Matches {
    fn cut_short(files_matched: usize, matches: usize) -> SearchSummary {
        SearchSummary {
            files_matched,
            matches,
            cancelled: true,
            ..SearchSummary::default()
        }
    }
}

/// A `Files`' reply.
#[derive(Default)]
struct Files {
    failure: Option<SearchFailure>,
}

impl Collect for Files {
    type Item = PathBuf;
    type End = FilesSummary;

    fn data(&mut self, data: SearchData) -> io::Result<Option<Vec<PathBuf>>> {
        match data {
            SearchData::Files(paths) => Ok(Some(paths.iter().map(wire::path).collect())),
            SearchData::Failed(failure) => {
                self.failure = Some(failure);
                Ok(None)
            }
            data => Err(wire::invalid(format_args!("unexpected {data:?}"))),
        }
    }

    fn merge(waiting: &mut Vec<PathBuf>, batch: Vec<PathBuf>) {
        waiting.extend(batch);
    }

    fn end(self, status: Status) -> Result<FilesSummary, SearchFailed> {
        match status {
            Status::Ok => Ok(FilesSummary { complete: true }),
            Status::Cancelled => Ok(FilesSummary { complete: false }),
            status => Err(failed(self.failure, status)),
        }
    }

    fn cancelled(self) -> FilesSummary {
        FilesSummary { complete: false }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::single_range_in_vec_init)]

    use super::{Collect, Matches};
    use fe_host::{FileMatches, LineMatch};

    fn line(line_number: u64, ranges: &[std::ops::Range<usize>]) -> LineMatch {
        LineMatch {
            line_number,
            line_offset: 0,
            ranges: ranges.to_vec(),
            preview: String::new(),
            preview_start: 0,
        }
    }

    fn file(name: &str, lines: Vec<LineMatch>) -> FileMatches {
        FileMatches {
            path: format!("/p/{name}").into(),
            rel: name.into(),
            lines,
        }
    }

    #[test]
    fn a_file_split_across_frames_is_one_entry_again() {
        let mut waiting = vec![
            file("a", vec![line(1, &[0..1])]),
            file("b", vec![line(1, &[0..1])]),
        ];
        // The server's next frame continues b: its line 1's ranges, then
        // a line of its own.
        let next = vec![
            file("b", vec![line(1, &[4..5]), line(2, &[0..2])]),
            file("c", vec![line(9, &[0..1])]),
        ];
        Matches::merge(&mut waiting, next);
        assert_eq!(
            waiting,
            [
                file("a", vec![line(1, &[0..1])]),
                file("b", vec![line(1, &[0..1, 4..5]), line(2, &[0..2])]),
                file("c", vec![line(9, &[0..1])]),
            ]
        );
    }

    #[test]
    fn the_same_path_apart_stays_apart() {
        let mut waiting = vec![file("a", vec![line(1, &[0..1])])];
        Matches::merge(
            &mut waiting,
            vec![
                file("b", vec![line(1, &[0..1])]),
                file("a", vec![line(2, &[0..1])]),
            ],
        );
        assert_eq!(waiting.len(), 3);
    }
}
