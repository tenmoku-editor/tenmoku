//! Telling XML and JSON by how a file starts, for files nothing else names,
//! and Objective-C from MATLAB in a `.m` file.

use crate::language::built_in_name;

/// How much of the text the sniff reads.
const SNIFF_BYTES: usize = 512;

/// The language a buffer's content says it is, for one whose path names no
/// language ([`detect_language`](crate::detect_language) asks only then): XML
/// for an element, after any comments, whose closing tag or a `/>` is in the
/// first [`SNIFF_BYTES`] of `text_start`, but not `<html`; JSON for `{` then
/// `"` or `}`, or `[` then `{`, `"`, a digit, `-` or `]`, past whitespace.
/// Only languages compiled in count. When unsure, `None`.
pub(crate) fn language_name_by_content(text_start: &str) -> Option<&'static str> {
    let head = head(text_start);
    let text = head.trim_start_matches(|c: char| c == '\u{feff}' || c.is_whitespace());
    let language = if is_json(text) {
        "json"
    } else if is_xml(text) {
        "xml"
    } else {
        return None;
    };
    built_in_name(language)
}

/// Which of the two languages that claim `.m` a file's start says it is:
/// `objc` or `matlab`, by the first [`SNIFF_BYTES`] of `text_start`, or `None`
/// when unsure. A line's start, past its indent and any `/* */` comment
/// closed on it, is what counts, and text inside a block comment doesn't: a
/// C one, opened anywhere on a line outside a string (and, once the text
/// looks like MATLAB, before a `%`), or a MATLAB `%{` `%}` one, nested.
///
/// Objective-C when a line starts with a preprocessor directive (`#import`,
/// `#include`, `#define`) or an `@` keyword ([`OBJC_KEYWORDS`]) anywhere, or
/// with a `//` or `/*` comment before any MATLAB marker. Else MATLAB when one
/// starts with a MATLAB marker: `function` declaring one (`function f`,
/// `function y = f(x)`, `function [a, b] = f`, `function[y] = f`),
/// `classdef`, a `%` comment, `clc`, `clear`, `close all`, `format long` or
/// `short`, an `end` closing an `if`, `for`, `while`, `switch` or `function`
/// opened before it, or `disp(` or `fprintf(` with a `%` format.
pub(crate) fn language_name_for_m_file(text_start: &str) -> Option<&'static str> {
    let head = head(text_start).trim_start_matches('\u{feff}');
    let mut saw_matlab = false;
    let mut opened = false;
    let mut in_c_comment = false;
    // How many `%{` comments the line is in.
    let mut matlab_comments = 0usize;
    for line in head.lines() {
        let mut text = line.trim_start();
        if matlab_comments > 0 {
            match text.trim_end() {
                "%{" => matlab_comments += 1,
                "%}" => matlab_comments -= 1,
                _ => {}
            }
            continue;
        }
        if in_c_comment {
            let Some(end) = text.find("*/") else {
                continue;
            };
            in_c_comment = false;
            text = text[end + 2..].trim_start();
        }
        // Past `/* */` comments closed on this line, to what follows.
        while let Some(comment) = text.strip_prefix("/*") {
            if !saw_matlab {
                return Some("objc");
            }
            match comment.find("*/") {
                Some(end) => text = comment[end + 2..].trim_start(),
                None => {
                    in_c_comment = true;
                    text = "";
                }
            }
        }
        if text.starts_with("//") {
            if !saw_matlab {
                return Some("objc");
            }
            continue;
        }
        if is_objc_line(text) {
            return Some("objc");
        }
        if text.trim_end() == "%{" {
            matlab_comments = 1;
            saw_matlab = true;
            continue;
        }
        let word = leading_word(text);
        let rest = &text[word.len()..];
        if is_matlab_line(text, word, rest, opened) {
            saw_matlab = true;
        }
        if matches!(word, "if" | "for" | "while" | "switch" | "function")
            && rest.starts_with([' ', '\t', '(', '['])
        {
            opened = true;
        }
        // A `%` line is a MATLAB comment, `/*` and all.
        if !in_c_comment && !text.starts_with('%') {
            in_c_comment = c_comment_left_open(text, saw_matlab);
        }
    }
    saw_matlab.then_some("matlab")
}

/// Whether `code`, a line's text, leaves a `/*` comment open at its end: one
/// outside a string, quoted by `"` or `'` with `\` escapes, before any `//`
/// and any MATLAB `%` comment: any `%` when `percent_comments`, else one
/// after a `;` or `,`, where C's `%` can't be, or with no `;` after it before
/// the `/*`, which C's `a % b;` has.
fn c_comment_left_open(code: &str, percent_comments: bool) -> bool {
    let mut chars = code.chars().peekable();
    let mut quote = None;
    // The last character outside a string and whitespace.
    let mut last = None;
    while let Some(c) = chars.next() {
        let before = last;
        if quote.is_none() && !c.is_whitespace() {
            last = Some(c);
        }
        match (quote, c) {
            (Some(_), '\\') => {
                chars.next();
            }
            (Some(open), c) if c == open => quote = None,
            (Some(_), _) => {}
            (None, '"' | '\'') => quote = Some(c),
            // C's `%` is between operands, a `;` after them before a `/*`.
            (None, '%')
                if percent_comments
                    || matches!(before, Some(';' | ','))
                    || !(chars.clone().collect::<String>())
                        .split("/*")
                        .next()
                        .is_some_and(|operands| operands.contains(';')) =>
            {
                return false;
            }
            (None, '/') => match chars.peek() {
                Some('/') => return false,
                Some('*') => {
                    chars.next();
                    // To the comment's `*/`, if it's on this line.
                    let mut star = false;
                    loop {
                        match chars.next() {
                            None => return true,
                            Some('/') if star => break,
                            Some(c) => star = c == '*',
                        }
                    }
                }
                _ => {}
            },
            (None, _) => {}
        }
    }
    false
}

/// The `@` keywords that start an Objective-C line.
const OBJC_KEYWORDS: &[&str] = &[
    "interface",
    "implementation",
    "protocol",
    "class",
    "end",
    "property",
    "synthesize",
    "autoreleasepool",
    "import",
];

/// The preprocessor directives that start an Objective-C line.
const OBJC_DIRECTIVES: &[&str] = &["import", "include", "define"];

/// A line, past its indent, starting with an Objective-C directive or `@`
/// keyword, the word ending there.
fn is_objc_line(text: &str) -> bool {
    let starts = |prefix: char, words: &[&str]| {
        text.strip_prefix(prefix)
            .is_some_and(|rest| words.contains(&leading_word(rest)))
    };
    starts('#', OBJC_DIRECTIVES) || starts('@', OBJC_KEYWORDS)
}

/// A line, past its indent, starting with a MATLAB marker: see
/// [`language_name_for_m_file`]. `word` is its leading word and `rest` what
/// follows it; `opened` is whether a block was opened before it.
fn is_matlab_line(text: &str, word: &str, rest: &str, opened: bool) -> bool {
    let next_word = || leading_word(rest.trim_start());
    let ends_statement = || {
        let rest = rest.trim_start();
        rest.is_empty() || rest.starts_with([';', ',', '%'])
    };
    if text.starts_with('%') {
        return true;
    }
    match word {
        // A name after a space, which may be an output before ` = `, a `[`
        // list of them, or `=`, as the spec lists them.
        "function" => {
            let after = rest.trim_start();
            after.starts_with(['[', '='])
                || (after.len() < rest.len()
                    && after.starts_with(|c: char| c.is_ascii_alphabetic()))
        }
        "classdef" => rest.starts_with([' ', '\t', '(']),
        "clc" => ends_statement(),
        // `clear`, `clear all`, `clear x y`; not a call or an assignment.
        "clear" => ends_statement() || (rest.starts_with([' ', '\t']) && !next_word().is_empty()),
        "close" => rest.starts_with([' ', '\t']) && next_word() == "all",
        "format" => rest.starts_with([' ', '\t']) && matches!(next_word(), "long" | "short"),
        "end" => opened && ends_statement(),
        "disp" => rest
            .strip_prefix('(')
            .is_some_and(|args| has_percent_format(args, false)),
        "fprintf" => rest
            .strip_prefix('(')
            .is_some_and(|args| has_percent_format(args, true)),
        _ => false,
    }
}

/// A MATLAB format in the arguments `args` of `disp(` or `fprintf(`: a
/// double-quoted string first, or first in a `sprintf(` first, or a
/// single-quoted string with a `%` in it anywhere in the call. For
/// `fprintf(`, `c_char_literals` says, one after the first argument that is
/// a C character literal, one character or a `\` and one (`'%'`, `'\%'`),
/// doesn't count: C's first argument is a stream, as in
/// `fprintf(stderr, "%c", '%')`. Only the call counts: the scan ends at its
/// `)`, or at a comment (`%`, `//`, `/*`) outside a string, and skips
/// double-quoted ones.
fn has_percent_format(args: &str, c_char_literals: bool) -> bool {
    let first = args.trim_start();
    let first = first.strip_prefix("sprintf(").unwrap_or(first).trim_start();
    if first
        .strip_prefix('"')
        .is_some_and(|string| double_quoted(string).contains('%'))
    {
        return true;
    }
    let mut depth = 0usize;
    let mut first_argument = true;
    let mut chars = args.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                while let Some(c) = chars.next() {
                    match c {
                        '\\' => {
                            chars.next();
                        }
                        '"' if chars.next_if_eq(&'"').is_none() => break,
                        _ => {}
                    }
                }
            }
            '\'' => {
                let mut string = String::new();
                while let Some(c) = chars.next() {
                    if c == '\'' && chars.next_if_eq(&'\'').is_none() {
                        break;
                    }
                    string.push(c);
                }
                let len = string.chars().count();
                let char_literal = c_char_literals
                    && !first_argument
                    && (len == 1 || (len == 2 && string.starts_with('\\')));
                if string.contains('%') && !char_literal {
                    return true;
                }
            }
            ',' if depth == 0 => first_argument = false,
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => match depth.checked_sub(1) {
                Some(outer) => depth = outer,
                None => return false,
            },
            '%' => return false,
            '/' if matches!(chars.peek(), Some('/' | '*')) => return false,
            _ => {}
        }
    }
    false
}

/// The text of a MATLAB double-quoted string, from just after its opening
/// `"` in `string`, to its end or the end of `string`: `""` is a `"` in it.
fn double_quoted(string: &str) -> String {
    let mut text = String::new();
    let mut chars = string.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '"' && chars.next_if_eq(&'"').is_none() {
            break;
        }
        text.push(c);
    }
    text
}

/// The identifier `text` starts with, if any.
fn leading_word(text: &str) -> &str {
    let end = text
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .unwrap_or(text.len());
    &text[..end]
}

/// The first [`SNIFF_BYTES`] of `text`, cut back to a character boundary.
fn head(text: &str) -> &str {
    let mut end = text.len().min(SNIFF_BYTES);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

fn is_json_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r')
}

fn is_json(text: &str) -> bool {
    let (open, rest) = match text.as_bytes().first() {
        Some(b'{') => (b'{', &text[1..]),
        Some(b'[') => (b'[', &text[1..]),
        _ => return false,
    };
    let Some(next) = rest.trim_start_matches(is_json_space).chars().next() else {
        return false;
    };
    match open {
        b'{' => matches!(next, '"' | '}'),
        _ => matches!(next, '{' | '"' | '-' | ']') || next.is_ascii_digit(),
    }
}

/// An element after any comments, with its closing tag or a `/>` in `head`.
fn is_xml(head: &str) -> bool {
    let mut text = head;
    while let Some(comment) = text.strip_prefix("<!--") {
        let Some(end) = comment.find("-->") else {
            return false;
        };
        text = comment[end + 3..].trim_start();
    }
    let Some(element) = text.strip_prefix('<') else {
        return false;
    };
    let name_len = element
        .find(|c: char| !is_name_char(c))
        .unwrap_or(element.len());
    let name = &element[..name_len];
    if !name.starts_with(|c: char| c.is_alphabetic() || c == '_')
        || name.eq_ignore_ascii_case("html")
        || name_len == element.len()
    {
        return false;
    }
    let after = &element[name_len..];
    after.contains("/>")
        || after.match_indices("</").any(|(at, close)| {
            let rest = &after[at + close.len()..];
            rest.strip_prefix(name)
                .is_some_and(|rest| rest.trim_start().starts_with('>'))
        })
}

/// A character of an XML name after its first.
fn is_name_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | ':')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_head_is_cut_at_a_character_boundary() {
        let text = format!("{}é", "a".repeat(SNIFF_BYTES - 1));
        assert_eq!(head(&text).len(), SNIFF_BYTES - 1);
        assert_eq!(head("ab"), "ab");
    }

    #[test]
    fn xml_needs_a_closing_tag_or_an_empty_element() {
        assert!(is_xml("<a>x</a>"));
        assert!(is_xml("<a><b/></a"));
        assert!(is_xml("<!-- c --> <!--d--><a:b>x</a:b >"));
        assert!(!is_xml("<a>x</ab>"));
        assert!(!is_xml("<a>x"));
        assert!(!is_xml("<!-- c <a/>"));
        assert!(!is_xml("<1a/>"));
        assert!(!is_xml("<HTML><br/></HTML>"));
        assert!(!is_xml("<?xml version=\"1.0\"?><a/>"));
    }

    #[test]
    fn json_is_judged_by_its_first_two_tokens() {
        for text in [
            "{\"a\": 1}",
            "{ }",
            "{\n}",
            "[1]",
            "[ -1]",
            "[]",
            "[{",
            "[\"a\"",
        ] {
            assert!(is_json(text), "{text:?}");
        }
        for text in ["{", "{ a: 1}", "[section]", "[[bin]]", "[ ", "x{}"] {
            assert!(!is_json(text), "{text:?}");
        }
    }
}
