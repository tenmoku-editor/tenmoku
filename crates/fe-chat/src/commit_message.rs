//! The headless core of Draft from diff: a commit message for the staged
//! changes, asked of the chat's provider in one request.
//!
//! - [`commit_message_request`] lays out the request from the
//!   [`StagedFile`]s: the files and how each changed, then their unified
//!   diff, cut to [`DIFF_BUDGET`] bytes. When the diffs don't fit, the
//!   largest are cut first, down to an even share; when even
//!   [`MIN_SECTION`] each doesn't fit, the largest are left out whole and
//!   counted ("N more files"). A binary file, one too large to read, or one
//!   the chat never reads ([`StagedDiff::Denied`], the chat's deny list) is
//!   left out of the text: only how many there were is said.
//! - [`parse_commit_message`] reads the reply as a subject and a body,
//!   dropping a Markdown fence around them.
//! - [`run_commit_message`] drives a provider to a [`CommitDraft`].

use std::fmt::Write as _;

use futures::StreamExt as _;
use kiln_l10n::tr;
use serde_json::{Map, Value};

use crate::provider::{
    ApiMessage, ChatEvent, ChatProvider, ChatRequest, Effort, ErrorKind, ProviderError, Role,
    SystemBlock,
};
use crate::request::ChatSettings;

/// The most bytes of diff a request carries.
pub const DIFF_BUDGET: usize = 24 * 1024;

/// The least a cut file's diff keeps: below that share, files are left out
/// whole instead, the largest first.
pub const MIN_SECTION: usize = 512;

/// The longest subject a draft asks for, in characters, as the box's
/// counter counts.
pub const SUBJECT_LIMIT: usize = 72;

/// The most files the request names; the rest are counted.
pub const MAX_LISTED: usize = 200;

/// Room kept in a cut diff for the line that says it was cut.
const CUT_NOTE_ROOM: usize = 64;

/// Room kept in the diff for the line saying files were left out.
const NOTE_ROOM: usize = 64;

/// How a staged file changed, HEAD to index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StagedChange {
    Added,
    Modified,
    Deleted,
}

impl StagedChange {
    /// Its letter, as `git diff --name-status` gives it.
    fn letter(self) -> char {
        match self {
            StagedChange::Added => 'A',
            StagedChange::Modified => 'M',
            StagedChange::Deleted => 'D',
        }
    }
}

/// What a staged file's diff is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StagedDiff {
    /// Its patch as `git diff --cached` writes it, from its `diff --git`
    /// line: mode lines and `\ No newline at end of file` markers
    /// included.
    Text(String),
    Binary,
    /// Too large to read.
    TooLarge,
    /// A path the chat never reads: a secret, under `.git/`, or outside
    /// the folder.
    Denied,
}

/// One staged file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StagedFile {
    /// Relative to the repository, `/`-separated.
    pub path: String,
    pub change: StagedChange,
    pub diff: StagedDiff,
}

const SYSTEM: &str = "\
You write git commit messages in Tenmoku, a code editor. The user message \
lists the staged files, each with its status (A added, M modified, D \
deleted), then their unified diff. Long diffs are cut, and some files may \
be left out; write from what is shown.

Answer with only the commit message, nothing before or after it, and no \
Markdown fence or quotes around it:
- a subject line of at most 72 characters, in the imperative mood (\"Add\", \
\"Fix\", \"Move\"), with no period at the end;
- a blank line;
- a short body of a few lines, wrapped at 72 characters, saying what changed \
and why.
";

/// The request asking `settings`' model for a commit message for `files`:
/// see the module's notes.
pub fn commit_message_request(files: &[StagedFile], settings: &ChatSettings) -> ChatRequest {
    ChatRequest {
        model: settings.model.clone(),
        system: vec![SystemBlock {
            text: SYSTEM.to_owned(),
            cache_control: None,
        }],
        tools: Vec::new(),
        messages: vec![ApiMessage {
            role: Role::User,
            content: vec![text_block(staged_text(files))],
        }],
        max_tokens: settings.max_tokens,
        effort: Effort::Low,
        show_thinking: false,
        task_budget: None,
    }
}

/// The user message: the files, then the diff.
fn staged_text(files: &[StagedFile]) -> String {
    let (shown, hidden): (Vec<&StagedFile>, Vec<&StagedFile>) = files
        .iter()
        .partition(|file| matches!(file.diff, StagedDiff::Text(_)));
    let mut text = String::from("<staged-files>\n");
    for file in shown.iter().take(MAX_LISTED) {
        let _ = writeln!(text, "{} {}", file.change.letter(), file.path);
    }
    if shown.len() > MAX_LISTED {
        let _ = writeln!(text, "[{} more files]", shown.len() - MAX_LISTED);
    }
    if !hidden.is_empty() {
        let _ = writeln!(
            text,
            "[{} more files not shown: binary, too large or private]",
            hidden.len()
        );
    }
    text.push_str("</staged-files>\n<diff>\n");
    let sections: Vec<String> = shown.iter().map(|file| section(file)).collect();
    let (cap, kept) = fit(&sections.iter().map(String::len).collect::<Vec<_>>());
    for (section, kept) in sections.iter().zip(&kept) {
        if *kept {
            text.push_str(&cut(section, cap));
        }
    }
    let left_out = kept.iter().filter(|kept| !**kept).count();
    if left_out > 0 {
        let _ = writeln!(text, "[the diffs of {left_out} more files left out]");
    }
    text.push_str("</diff>");
    text
}

/// A file's patch, ending in a line break.
fn section(file: &StagedFile) -> String {
    let StagedDiff::Text(patch) = &file.diff else {
        return String::new();
    };
    let mut section = patch.clone();
    if !section.ends_with('\n') {
        section.push('\n');
    }
    section
}

/// How the sections of `sizes` fit [`DIFF_BUDGET`], the note that some
/// are left out included: see [`fit_under`]. Leaving any out keeps
/// [`NOTE_ROOM`] for the note.
fn fit(sizes: &[usize]) -> (usize, Vec<bool>) {
    let (cap, kept) = fit_under(sizes, DIFF_BUDGET);
    match kept.iter().all(|kept| *kept) {
        true => (cap, kept),
        false => fit_under(sizes, DIFF_BUDGET - NOTE_ROOM),
    }
}

/// How the sections of `sizes` fit `budget`: the size each is cut to
/// (`usize::MAX` when none is), and which are kept. The cap is the largest
/// even share the kept ones fit under; while it'd be under
/// [`MIN_SECTION`], the largest are left out, the later of equal ones
/// first.
fn fit_under(sizes: &[usize], budget: usize) -> (usize, Vec<bool>) {
    let mut order: Vec<usize> = (0..sizes.len()).collect();
    order.sort_by_key(|&ix| (sizes[ix], ix));
    // The most of the smallest that fit at MIN_SECTION each.
    let mut keep = 0;
    let mut floor = 0;
    for &ix in &order {
        floor += sizes[ix].min(MIN_SECTION);
        if floor > budget {
            break;
        }
        keep += 1;
    }
    let mut kept = vec![false; sizes.len()];
    for &ix in &order[..keep] {
        kept[ix] = true;
    }
    // The even share: the smallest sizes whole, the rest cut to it.
    let mut cap = usize::MAX;
    let mut used = 0;
    for (at, &ix) in order[..keep].iter().enumerate() {
        let left = keep - at;
        if used + sizes[ix] * left > budget {
            cap = (budget - used) / left;
            break;
        }
        used += sizes[ix];
    }
    (cap, kept)
}

/// `section` cut to at most `cap` bytes, at a line's end, with a line
/// saying how many lines were cut.
fn cut(section: &str, cap: usize) -> String {
    if section.len() <= cap {
        return section.to_owned();
    }
    let mut end = cap.saturating_sub(CUT_NOTE_ROOM);
    while !section.is_char_boundary(end) {
        end -= 1;
    }
    let end = section[..end].rfind('\n').map_or(0, |newline| newline + 1);
    let lines = section[end..].lines().count();
    format!("{}[{lines} more lines cut]\n", &section[..end])
}

fn text_block(text: String) -> Value {
    let mut block = Map::new();
    block.insert("text".to_owned(), Value::String(text));
    block.insert("type".to_owned(), Value::String("text".to_owned()));
    Value::Object(block)
}

/// A drafted commit message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommitDraft {
    pub subject: String,
    /// Empty when the reply had none.
    pub body: String,
}

impl CommitDraft {
    /// The message as the box takes it: the subject, then a blank line and
    /// the body if there is one.
    pub fn message(&self) -> String {
        match self.body.is_empty() {
            true => self.subject.clone(),
            false => format!("{}\n\n{}", self.subject, self.body),
        }
    }

    /// The subject's length in characters when it's over
    /// [`SUBJECT_LIMIT`].
    pub fn long_subject(&self) -> Option<usize> {
        let chars = self.subject.chars().count();
        (chars > SUBJECT_LIMIT).then_some(chars)
    }
}

/// The message in `reply`: inside its first fence if it has one (closed
/// or not), else the whole of it; its first line with text is the
/// subject, and what follows the blank lines after it the body, each line
/// trimmed at its end. `None` when there's no text.
pub fn parse_commit_message(reply: &str) -> Option<CommitDraft> {
    let lines: Vec<&str> = reply.lines().map(str::trim_end).collect();
    let inside = match lines.iter().position(|line| is_fence(line)) {
        Some(open) => {
            let rest = &lines[open + 1..];
            let close = rest.iter().position(|line| is_fence(line));
            &rest[..close.unwrap_or(rest.len())]
        }
        None => &lines[..],
    };
    let mut lines = inside.iter().skip_while(|line| line.trim().is_empty());
    let subject = lines.next()?.trim().to_owned();
    let body: Vec<&str> = lines.copied().skip_while(|line| line.is_empty()).collect();
    let body = body.join("\n").trim_end().to_owned();
    Some(CommitDraft { subject, body })
}

/// Whether `line` opens or closes a Markdown fence.
fn is_fence(line: &str) -> bool {
    let line = line.trim_start();
    line.starts_with("```") || line.starts_with("~~~")
}

/// Why no draft came.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DraftError {
    Failed(ProviderError),
    /// The reply had no text.
    Empty,
}

/// Asks `provider` for a commit message for `files`, reading the reply to
/// its end. Dropping the future drops the provider's stream, which
/// cancels the request.
pub async fn run_commit_message(
    provider: &dyn ChatProvider,
    files: &[StagedFile],
    settings: &ChatSettings,
) -> Result<CommitDraft, DraftError> {
    let mut events = provider.stream(commit_message_request(files, settings));
    let mut reply = String::new();
    loop {
        match events.next().await {
            Some(ChatEvent::TextDelta { text, .. }) => reply.push_str(&text),
            Some(ChatEvent::Failed(error)) => return Err(DraftError::Failed(error)),
            Some(ChatEvent::Finished { .. }) => break,
            Some(_) => {}
            None => {
                let message = tr!("chat-reply-ended-early").to_string();
                return Err(DraftError::Failed(ProviderError::new(
                    ErrorKind::Stream,
                    message,
                )));
            }
        }
    }
    parse_commit_message(&reply).ok_or(DraftError::Empty)
}
