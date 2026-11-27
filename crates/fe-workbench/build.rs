//! Embeds the curated release notes, `release-notes/<version>.md` at the
//! repository root, for the What's New tab (`src/whats_new.rs`, which has
//! the files' rules): `$OUT_DIR/release_notes.rs` is `RELEASE_NOTES`, each
//! file's version and text, newest first. A file not named `X.Y.Z.md` or
//! `X.Y.Z-preview.N.md` fails the build; hidden files (`.DS_Store`) are
//! skipped.

use std::{env, fs, path::PathBuf};

fn main() {
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let dir = manifest.join("../../release-notes");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={}", dir.display());
    let dir = fs::canonicalize(&dir)
        .unwrap_or_else(|error| panic!("can't read {}: {error}", dir.display()));
    let mut notes = Vec::new();
    for entry in fs::read_dir(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())) {
        let entry = entry.unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') {
            continue;
        }
        let (order, version) = (name.strip_suffix(".md"))
            .and_then(|version| Some((parse_release_version(version)?, version)))
            .unwrap_or_else(|| {
                panic!(
                    "release-notes/{name}: a release's notes are named X.Y.Z.md or \
                     X.Y.Z-preview.N.md, its version as [workspace.package] version \
                     writes it"
                )
            });
        let path = entry.path();
        assert!(path.is_file(), "release-notes/{name} isn't a file");
        notes.push((order, version.to_owned(), path));
    }
    notes.sort_by_key(|note| std::cmp::Reverse(note.0));
    let mut out = String::from(
        "/// Each `release-notes/` file's version and text, newest first.\n\
         pub(crate) static RELEASE_NOTES: &[(&str, &str)] = &[\n",
    );
    for (_, version, path) in notes {
        let path = path.to_str().expect("a UTF-8 path to release-notes");
        out.push_str(&format!("    ({version:?}, include_str!({path:?})),\n"));
    }
    out.push_str("];\n");
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    fs::write(out_dir.join("release_notes.rs"), out).unwrap();
}

/// `X.Y.Z` or `X.Y.Z-preview.N` as a key that sorts as versions do: the
/// numbers, then whether it is the stable release, then the preview's number.
/// fe-config's `whats_new::parse_release_version` and its order, which a
/// build script can't depend on without a build dependency.
fn parse_release_version(text: &str) -> Option<((u64, u64, u64), bool, u64)> {
    let (core, pre) = match text.split_once('-') {
        Some((core, pre)) => (core, Some(pre.strip_prefix("preview.")?)),
        None => (text, None),
    };
    let core = parse_version(core)?;
    // `parse_version` on "N.0.0" holds N to plain digits.
    let pre = match pre {
        Some(n) => Some(parse_version(&format!("{n}.0.0"))?.0),
        None => None,
    };
    Some((core, pre.is_none(), pre.unwrap_or(0)))
}

/// `MAJOR.MINOR.PATCH` as numbers: fe-config's `whats_new::parse_version`,
/// copied for the same reason.
fn parse_version(text: &str) -> Option<(u64, u64, u64)> {
    let mut parts = text.split('.');
    let mut part = || {
        let part = parts.next()?;
        let digits = !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit());
        let plain = part == "0" || !part.starts_with('0');
        (digits && plain).then(|| part.parse().ok()).flatten()
    };
    let version = (part()?, part()?, part()?);
    parts.next().is_none().then_some(version)
}
