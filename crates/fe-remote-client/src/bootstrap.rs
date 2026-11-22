//! The bootstrap (D § 4): the POSIX sh script ssh runs as `sh -c
//! '<script>'`, and the lines it prints.
//!
//! In one ssh session, so with one sign-in, the script prints
//! `TENMOKU-BOOT 1 <uname -s> <uname -m> <present|missing>` for
//! `${XDG_CACHE_HOME:-~/.cache}/tenmoku-server/<dir>/tenmoku-server`. When
//! the server is missing (or a reinstall is asked for) it reads
//! `<size> <sha256>` and then exactly `<size>` bytes, a `.tar.gz` with
//! `tenmoku-server` at its root. It checks the SHA-256, unpacks, `chmod
//! 755`s and renames the server into place, and prunes. Then it prints
//! `TENMOKU-BOOT ready` and execs `tenmoku-server --stdio`. A failure is one
//! `TENMOKU-BOOT error <message>` line, and the script exits.
//!
//! After an install the script records the build the server reports
//! (`tenmoku-server --version`) in a `build` file beside it, as that names
//! its daemon's lock. Then it prunes the other installs to the newest three,
//! by the server's modification time (which `tar -m` sets to the install's),
//! a tie going to the later name. It keeps any install whose daemon lock
//! file is there, held or not, and any whose build is unknown. sh has no
//! `flock`, and nothing it can see shows a lock free: a daemon takes its
//! lock before it writes its pid, and a process's open files can change
//! while they're looked at, or not be readable at all. A lock file goes
//! when its runtime folder does (`$XDG_RUNTIME_DIR`, cleared once the user
//! has no session; `/tmp` at a reboot), and then its install can go too.
//!
//! The client sends nothing after the archive until it reads `ready`, so no
//! tool can read past the payload. The script needs `uname`, `mkdir`,
//! `head -c`, `chmod`, `mv`, `tar` and `sha256sum` or `shasum -a 256`, and
//! `rm` to drop a bad upload and prune. GNU tar also runs `gzip` for `-z`.
//!
//! ssh hands the command to the user's login shell, so the script is sent
//! in single quotes. It has none itself, and no `\\`, so sh, bash, zsh and
//! fish all read it the same.

use std::fmt;

/// The script, after its parameters.
const BODY: &str = include_str!("bootstrap.sh");

/// What starts each line the script prints.
pub const PREFIX: &str = "TENMOKU-BOOT";

/// The bootstrap's version, the probe line's first word after the prefix.
pub const VERSION: &str = "1";

/// The script that installs into `dir` (the build, or `dev-<sha256[..12]>`),
/// reinstalling if `reinstall`. `None` if `dir` isn't a safe name: letters,
/// digits and `.+-_`, not starting with `.` or `-`.
pub fn script(dir: &str, reinstall: bool) -> Option<String> {
    if !safe_name(dir) {
        return None;
    }
    let force = if reinstall { 1 } else { 0 };
    Some(format!("B={dir} F={force}\n{BODY}"))
}

/// Whether `name` can go in the script unquoted and name a folder.
pub fn safe_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with(['.', '-'])
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._+-".contains(&b))
}

/// The probe line: the host and whether its server is installed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Probe {
    /// `uname -s`, as `Linux` or `Darwin`.
    pub os: String,
    /// `uname -m`, as `x86_64` or `arm64`.
    pub arch: String,
    pub present: bool,
}

/// A line the script printed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Line {
    Probe(Probe),
    Ready,
    Error(String),
}

impl Line {
    /// The line, if it's the script's (it starts with [`PREFIX`]). Others,
    /// such as a login shell's greeting, are `None`.
    pub fn parse(line: &str) -> Option<Result<Line, String>> {
        let rest = line.strip_prefix(PREFIX)?.strip_prefix(' ')?;
        let bad = || Err(format!("the bootstrap printed {line:?}"));
        if rest == "ready" {
            return Some(Ok(Line::Ready));
        }
        if let Some(message) = rest.strip_prefix("error ") {
            return Some(Ok(Line::Error(message.to_owned())));
        }
        let words: Vec<&str> = rest.split(' ').collect();
        Some(match words[..] {
            [VERSION, os, arch, state @ ("present" | "missing")] => Ok(Line::Probe(Probe {
                os: os.to_owned(),
                arch: arch.to_owned(),
                present: state == "present",
            })),
            _ => bad(),
        })
    }
}

/// A release asset's platform, as its file name has it: `linux-x86_64`,
/// `linux-aarch64` or `macos-arm64` (D § 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Asset {
    LinuxX86_64,
    LinuxAarch64,
    MacosArm64,
}

impl Asset {
    /// The asset for a host's `uname -s` and `uname -m`; `None` for one no
    /// server is built for.
    pub fn for_host(os: &str, arch: &str) -> Option<Asset> {
        match (os, arch) {
            ("Linux", "x86_64" | "amd64") => Some(Asset::LinuxX86_64),
            ("Linux", "aarch64" | "arm64") => Some(Asset::LinuxAarch64),
            ("Darwin", "arm64" | "aarch64") => Some(Asset::MacosArm64),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Asset::LinuxX86_64 => "linux-x86_64",
            Asset::LinuxAarch64 => "linux-aarch64",
            Asset::MacosArm64 => "macos-arm64",
        }
    }
}

impl fmt::Display for Asset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}
