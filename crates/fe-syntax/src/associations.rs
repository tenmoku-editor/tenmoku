//! `files.associations`: glob patterns that give the files they match a
//! language, set in the user's settings and a project's.
//!
//! A pattern with no `/` matches a file's name; one with a `/` matches its
//! path relative to the project root, so a file outside the root, or with
//! no root, is matched by the slash-free patterns alone. `*` matches any
//! run of characters but `/`, `?` any one but `/`, `**` any run including
//! `/` (`**/` any number of whole directories, none too), `[abc]`, `[a-z]`
//! and `[!a]` one of a set of characters or not, and `{a,b}` either
//! alternative (nested at most 8 deep, 64 alternatives at most, in a
//! pattern of at most 1024 characters). Matching ignores ASCII case, as
//! the built-in file names do.
//!
//! Of the patterns matching a file, a project's come before the user's.
//! Within each, the most specific wins: the one with the most literal
//! characters (those that aren't `*`, `?`, a set or a brace's commas and
//! braces, of a brace's alternatives the one with fewest), and at equal
//! counts one with a `/` over one without; remaining ties go to the
//! pattern given later, in a settings file the one written later. The
//! `/` of a `**/` is a literal character. A pattern naming a language
//! there isn't now (one whose grammar isn't installed yet, say) is passed over
//! for the next, so one registered later applies from then on.

use std::path::{Component, Path};

use crate::language::language_names;

/// Brace alternatives one pattern may expand to, at most.
const MAX_ALTERNATIVES: usize = 64;

/// Braces one pattern may nest, at most.
const MAX_BRACE_DEPTH: usize = 8;

/// The longest pattern, in characters.
const MAX_PATTERN_CHARS: usize = 1024;

/// The `files.associations` of the user's settings and a project's, ready
/// to match files against: see the [module](self) for the rules.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FileAssociations {
    /// The patterns that compile, the first to match winning.
    entries: Vec<Entry>,
}

#[derive(Clone, Debug, PartialEq)]
struct Entry {
    /// Each brace alternative's tokens.
    alternatives: Vec<Alternative>,
    /// Whether it matches the path relative to the root, not the name.
    slash: bool,
    language: String,
}

/// Why a `files.associations` entry is ignored.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AssociationProblem {
    /// Its pattern isn't a glob.
    BadPattern(GlobError),
    /// It names a language that isn't compiled in or registered now.
    UnknownLanguage,
}

/// What's wrong with a glob pattern.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlobError {
    /// It's empty.
    Empty,
    /// A `[` has no `]`.
    UnclosedSet,
    /// A set has no characters: `[]` or `[!]`.
    EmptySet,
    /// A `{` has no `}`.
    UnclosedBrace,
    /// A `}` has no `{`.
    UnopenedBrace,
    /// Its braces expand to more than 64 alternatives.
    TooManyAlternatives,
    /// It nests braces more than 8 deep.
    TooDeep,
    /// It's longer than 1024 characters.
    TooLong,
}

impl FileAssociations {
    /// The associations of a project's settings, `project`, over the
    /// user's, `user`: each a pattern and a language name, in the order
    /// that breaks ties. A pattern that isn't a glob is left out
    /// ([`association_problem`] says so).
    pub fn new<'a>(
        project: impl IntoIterator<Item = (&'a str, &'a str)>,
        user: impl IntoIterator<Item = (&'a str, &'a str)>,
    ) -> Self {
        // (from the project, literal count, slash, order) for each.
        let mut ranked: Vec<((bool, usize, bool, usize), Entry)> = Vec::new();
        let layers = [
            (true, project.into_iter().collect::<Vec<_>>()),
            (false, user.into_iter().collect()),
        ];
        for (from_project, entries) in layers {
            for (order, (pattern, language)) in entries.into_iter().enumerate() {
                let Ok(alternatives) = compile(pattern) else {
                    continue;
                };
                let literals = (alternatives.iter())
                    .map(|alternative| alternative.literals)
                    .min()
                    .unwrap_or(0);
                let slash = pattern.contains('/');
                let entry = Entry {
                    alternatives,
                    slash,
                    language: language.to_owned(),
                };
                ranked.push(((from_project, literals, slash, order), entry));
            }
        }
        ranked.sort_by(|(a, _), (b, _)| b.cmp(a));
        Self {
            entries: ranked.into_iter().map(|(_, entry)| entry).collect(),
        }
    }

    /// Whether it has no patterns.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The language the first pattern matching `path` names, of those
    /// naming one there is now: a pattern with a `/` matches its path
    /// relative to `root`, if it's under it, and one without its name.
    pub fn language_for(&self, path: &Path, root: Option<&Path>) -> Option<&'static str> {
        if self.entries.is_empty() {
            return None;
        }
        let name: Vec<char> = path.file_name()?.to_str()?.chars().collect();
        let relative: Option<Vec<char>> = root.and_then(|root| relative_path(path, root));
        self.entries
            .iter()
            .filter(|entry| {
                let subject = match entry.slash {
                    true => relative.as_deref(),
                    false => Some(name.as_slice()),
                };
                subject.is_some_and(|subject| {
                    (entry.alternatives.iter())
                        .any(|alternative| matches(&alternative.tokens, subject))
                })
            })
            .find_map(|entry| language_names().find(|&known| known == entry.language))
    }
}

/// Why the `files.associations` entry `pattern = "language"` is ignored, if
/// it is: a pattern that isn't a glob, or a language not compiled in or
/// [registered](crate::register_language) now.
pub fn association_problem(pattern: &str, language: &str) -> Option<AssociationProblem> {
    if let Err(error) = compile(pattern) {
        return Some(AssociationProblem::BadPattern(error));
    }
    match language_names().any(|known| known == language) {
        true => None,
        false => Some(AssociationProblem::UnknownLanguage),
    }
}

/// `path` relative to `root`, its components joined by `/`: `None` if it
/// isn't under `root`, or a component isn't UTF-8.
fn relative_path(path: &Path, root: &Path) -> Option<Vec<char>> {
    let relative = path.strip_prefix(root).ok()?;
    let mut joined = String::new();
    for component in relative.components() {
        let Component::Normal(part) = component else {
            return None;
        };
        if !joined.is_empty() {
            joined.push('/');
        }
        joined.push_str(part.to_str()?);
    }
    (!joined.is_empty()).then(|| joined.chars().collect())
}

/// One brace alternative of a pattern.
#[derive(Clone, Debug, PartialEq)]
struct Alternative {
    tokens: Vec<Token>,
    /// Its literal characters: those that aren't `*`, `?` or in a set,
    /// the `/` of a `**/` too.
    literals: usize,
}

#[derive(Clone, Debug, PartialEq)]
enum Token {
    Char(char),
    /// `?`.
    One,
    /// `*`.
    Star,
    /// `**` not followed by `/`: any run, `/` too.
    GlobStar,
    /// `**/`: nothing, or a run ending in `/`.
    Dirs,
    /// `[...]`: inclusive ranges, matched or (negated) not.
    Set {
        negated: bool,
        ranges: Vec<(char, char)>,
    },
}

/// `pattern`'s brace alternatives, each as tokens.
fn compile(pattern: &str) -> Result<Vec<Alternative>, GlobError> {
    if pattern.is_empty() {
        return Err(GlobError::Empty);
    }
    let chars: Vec<char> = pattern.chars().collect();
    if chars.len() > MAX_PATTERN_CHARS {
        return Err(GlobError::TooLong);
    }
    let mut at = 0;
    let alternatives = sequence(&chars, &mut at, 0)?;
    // `sequence` stops at the top level only at the end.
    debug_assert_eq!(at, chars.len());
    Ok(alternatives
        .into_iter()
        .map(|text| Alternative {
            literals: literals(&text),
            tokens: tokens(text),
        })
        .collect())
}

/// The literal characters of `text`, a pattern with no braces whose sets
/// are checked already: all but `*`, `?` and the sets.
fn literals(text: &str) -> usize {
    let chars: Vec<char> = text.chars().collect();
    let mut count = 0;
    let mut at = 0;
    while let Some(&c) = chars.get(at) {
        match c {
            '*' | '?' => at += 1,
            '[' => at = set_end(&chars, at).map_or(chars.len(), |end| end + 1),
            _ => {
                count += 1;
                at += 1;
            }
        }
    }
    count
}

/// The alternatives of the run of `chars` from `at`: to the end, or, in a
/// brace (`depth` above 0), to the `,` or `}` that ends it. Each is the
/// pattern's text with that alternative's braces resolved.
fn sequence(chars: &[char], at: &mut usize, depth: usize) -> Result<Vec<String>, GlobError> {
    let nested = depth > 0;
    let mut alternatives = vec![String::new()];
    while let Some(&c) = chars.get(*at) {
        match c {
            ',' | '}' if nested => return Ok(alternatives),
            '}' => return Err(GlobError::UnopenedBrace),
            '{' => {
                if depth == MAX_BRACE_DEPTH {
                    return Err(GlobError::TooDeep);
                }
                *at += 1;
                let mut options = Vec::new();
                loop {
                    options.extend(sequence(chars, at, depth + 1)?);
                    if options.len() > MAX_ALTERNATIVES {
                        return Err(GlobError::TooManyAlternatives);
                    }
                    match chars.get(*at) {
                        Some(',') => *at += 1,
                        Some('}') => break,
                        _ => return Err(GlobError::UnclosedBrace),
                    }
                }
                *at += 1;
                if alternatives.len() * options.len() > MAX_ALTERNATIVES {
                    return Err(GlobError::TooManyAlternatives);
                }
                alternatives = (alternatives.iter())
                    .flat_map(|head| options.iter().map(move |option| format!("{head}{option}")))
                    .collect();
            }
            '[' => {
                let end = set_end(chars, *at)?;
                let set: String = chars[*at..=end].iter().collect();
                for alternative in &mut alternatives {
                    alternative.push_str(&set);
                }
                *at = end + 1;
            }
            c => {
                for alternative in &mut alternatives {
                    alternative.push(c);
                }
                *at += 1;
            }
        }
    }
    match nested {
        true => Err(GlobError::UnclosedBrace),
        false => Ok(alternatives),
    }
}

/// Where the set opening at `chars[open]` closes.
fn set_end(chars: &[char], open: usize) -> Result<usize, GlobError> {
    let mut at = open + 1;
    if matches!(chars.get(at), Some('!' | '^')) {
        at += 1;
    }
    match chars[at..].iter().position(|&c| c == ']') {
        None => Err(GlobError::UnclosedSet),
        Some(0) => Err(GlobError::EmptySet),
        Some(len) => Ok(at + len),
    }
}

/// The tokens of a pattern with no braces, its sets checked already.
fn tokens(pattern: String) -> Vec<Token> {
    let chars: Vec<char> = pattern.chars().collect();
    let mut tokens = Vec::new();
    let mut at = 0;
    while let Some(&c) = chars.get(at) {
        at += 1;
        let token = match c {
            '?' => Token::One,
            '*' if chars.get(at) == Some(&'*') => {
                at += 1;
                while chars.get(at) == Some(&'*') {
                    at += 1;
                }
                if chars.get(at) == Some(&'/') {
                    at += 1;
                    Token::Dirs
                } else {
                    Token::GlobStar
                }
            }
            '*' => Token::Star,
            '[' => {
                let negated = matches!(chars.get(at), Some('!' | '^'));
                if negated {
                    at += 1;
                }
                let mut ranges = Vec::new();
                while chars[at] != ']' {
                    let start = chars[at];
                    match (chars.get(at + 1), chars.get(at + 2)) {
                        (Some('-'), Some(&end)) if end != ']' => {
                            ranges.push((start, end));
                            at += 3;
                        }
                        _ => {
                            ranges.push((start, start));
                            at += 1;
                        }
                    }
                }
                at += 1;
                Token::Set { negated, ranges }
            }
            c => Token::Char(c),
        };
        tokens.push(token);
    }
    tokens
}

/// Whether `tokens` match all of `subject`: by dynamic programming over
/// the tokens from the last, each row saying from which characters the
/// rest of the tokens match the rest of `subject`, so it takes time
/// proportional to their product, however many stars.
fn matches(tokens: &[Token], subject: &[char]) -> bool {
    let n = subject.len();
    // `rest[j]`: whether the tokens after this one match `subject[j..]`.
    let mut rest = vec![false; n + 1];
    rest[n] = true;
    for token in tokens.iter().rev() {
        let mut row = vec![false; n + 1];
        match token {
            Token::Star => {
                for j in (0..=n).rev() {
                    row[j] = rest[j] || (j < n && subject[j] != '/' && row[j + 1]);
                }
            }
            Token::GlobStar => {
                for j in (0..=n).rev() {
                    row[j] = rest[j] || (j < n && row[j + 1]);
                }
            }
            Token::Dirs => {
                // Whether some `/` at or after `j` ends a run the rest
                // matches after.
                let mut through_slash = false;
                for j in (0..=n).rev() {
                    through_slash |= j < n && subject[j] == '/' && rest[j + 1];
                    row[j] = rest[j] || through_slash;
                }
            }
            token => {
                for j in 0..n {
                    row[j] = rest[j + 1] && matches_one(token, subject[j]);
                }
            }
        }
        rest = row;
    }
    rest[0]
}

/// Whether `token`, one that matches a single character, matches `c`.
fn matches_one(token: &Token, c: char) -> bool {
    match token {
        Token::Char(want) => c.eq_ignore_ascii_case(want),
        Token::One => c != '/',
        Token::Set { negated, ranges } => {
            let within = |c: char| ranges.iter().any(|&(lo, hi)| lo <= c && c <= hi);
            let found =
                within(c) || within(c.to_ascii_lowercase()) || within(c.to_ascii_uppercase());
            c != '/' && found != *negated
        }
        Token::Star | Token::GlobStar | Token::Dirs => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn glob(pattern: &str, subject: &str) -> bool {
        let subject: Vec<char> = subject.chars().collect();
        (compile(pattern).unwrap().iter()).any(|alternative| matches(&alternative.tokens, &subject))
    }

    #[test]
    fn globs_match_as_documented() {
        for (pattern, subject, want) in [
            ("*.conf", "nginx.conf", true),
            ("*.conf", "a/nginx.conf", false),
            ("*.CONF", "nginx.conf", true),
            ("Jenkinsfile*", "Jenkinsfile.prod", true),
            ("Jenkinsfile*", "MyJenkinsfile", false),
            ("?.c", "a.c", true),
            ("?.c", "ab.c", false),
            ("**/templates/*.html", "templates/a.html", true),
            ("**/templates/*.html", "site/x/templates/a.html", true),
            ("**/templates/*.html", "templates/x/a.html", false),
            ("src/**", "src/a/b.rs", true),
            ("src/**/*.rs", "src/b.rs", true),
            ("a**b", "a/x/b", true),
            ("[abc].txt", "b.txt", true),
            ("[a-c].txt", "B.txt", true),
            ("[!a-c].txt", "d.txt", true),
            ("[!a-c].txt", "a.txt", false),
            ("*.{yml,yaml}", "ci.yaml", true),
            ("*.{yml,yaml}", "ci.toml", false),
            ("{a,b{c,d}}.x", "bd.x", true),
            ("{a{b{c{d{e{f{g{h}}}}}}}}", "abcdefgh", true),
        ] {
            assert_eq!(glob(pattern, subject), want, "{pattern:?} on {subject:?}");
        }
    }

    /// Nested braces, deep or never closed, are refused before they can
    /// recurse far: the stack holds.
    #[test]
    fn deep_braces_are_refused() {
        let deep = format!("{}{}", "{".repeat(10_000), "}".repeat(10_000));
        assert_eq!(compile(&deep), Err(GlobError::TooLong));
        assert_eq!(compile(&"{".repeat(1000)), Err(GlobError::TooDeep));
        assert_eq!(compile(&"{".repeat(9)), Err(GlobError::TooDeep));
        assert_eq!(compile(&"{".repeat(8)), Err(GlobError::UnclosedBrace));
    }

    /// Stars that backtracking would try every split of match, or fail to,
    /// at once.
    #[test]
    fn many_stars_match_in_polynomial_time() {
        let started = std::time::Instant::now();
        for (pattern, subject) in [
            (format!("{}b", "*a".repeat(35)), "a".repeat(35)),
            (format!("{}b", "**a".repeat(35)), "a/".repeat(35)),
            (
                format!("{}a", "**/a".repeat(30)),
                format!("{}b", "a/".repeat(40)),
            ),
        ] {
            let subject: Vec<char> = subject.chars().collect();
            let alternatives = compile(&pattern).unwrap();
            assert!(!matches(&alternatives[0].tokens, &subject), "{pattern}");
        }
        assert!(started.elapsed() < std::time::Duration::from_secs(1));
    }

    #[test]
    fn literals_count_all_but_wildcards_and_sets() {
        for (pattern, count) in [("*.conf", 5), ("**/ab*", 3), ("[abc]x?", 1), ("a/**/b", 4)] {
            let alternatives = compile(pattern).unwrap();
            assert_eq!(alternatives[0].literals, count, "{pattern:?}");
        }
    }

    #[test]
    fn bad_globs_say_why() {
        for (pattern, error) in [
            ("", GlobError::Empty),
            ("[ab", GlobError::UnclosedSet),
            ("[]", GlobError::EmptySet),
            ("[!]x", GlobError::EmptySet),
            ("*.{a,b", GlobError::UnclosedBrace),
            ("a}", GlobError::UnopenedBrace),
            (
                "{a,b}{a,b}{a,b}{a,b}{a,b}{a,b}{a,b}",
                GlobError::TooManyAlternatives,
            ),
            ("{a{b{c{d{e{f{g{h{i}}}}}}}}}", GlobError::TooDeep),
        ] {
            assert_eq!(compile(pattern), Err(error), "{pattern:?}");
        }
    }
}
