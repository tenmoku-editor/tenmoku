use super::*;

fn open_fd(path: &str) -> i32 {
    let path = std::ffi::CString::new(path).unwrap();
    // SAFETY: a valid C string; the descriptor is closed by the caller.
    let fd = unsafe { libc::open(path.as_ptr(), libc::O_WRONLY) };
    assert!(fd >= 0);
    fd
}

#[test]
fn redirects_only_a_null_stderr() {
    assert!(redirects(Stderr::Null));
    assert!(!redirects(Stderr::Elsewhere));
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("log/tenmoku.log");

    // A pipe and a regular file are left alone, and no log is made.
    let mut pipe = [0; 2];
    // SAFETY: `pipe` has room for two descriptors.
    assert_eq!(unsafe { libc::pipe(pipe.as_mut_ptr()) }, 0);
    assert_eq!(stderr_kind(pipe[1]), Stderr::Elsewhere);
    assert!(!redirect(pipe[1], &log, LIMIT, "x\n"));
    let file_path = dir.path().join("plain");
    fs::write(&file_path, "").unwrap();
    let plain = open_fd(file_path.to_str().unwrap());
    assert_eq!(stderr_kind(plain), Stderr::Elsewhere);
    assert!(!redirect(plain, &log, LIMIT, "x\n"));
    assert!(!log.exists());

    // A null descriptor is pointed at the log, and writes to it land there.
    let null = open_fd("/dev/null");
    assert_eq!(stderr_kind(null), Stderr::Null);
    assert!(redirect(null, &log, LIMIT, "first\n"));
    assert_eq!(stderr_kind(null), Stderr::Elsewhere);
    let mut out = ManuallyFile::new(null);
    writeln!(out.0, "second").unwrap();
    assert_eq!(fs::read_to_string(&log).unwrap(), "first\nsecond\n");
    // SAFETY: closing descriptors this test opened.
    unsafe {
        libc::close(pipe[0]);
        libc::close(pipe[1]);
        libc::close(plain);
    }
}

/// A descriptor as a `File` that closes it.
struct ManuallyFile(File);

impl ManuallyFile {
    fn new(fd: i32) -> Self {
        use std::os::unix::io::FromRawFd as _;
        // SAFETY: `fd` is open and owned by the caller, who gives it up.
        Self(unsafe { File::from_raw_fd(fd) })
    }
}

#[test]
fn rotates_past_the_size_limit() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("tenmoku.log");
    let old = dir.path().join("tenmoku.log.1");
    fs::write(&log, "0123456789").unwrap();
    rotate(&log, 5).unwrap();
    assert!(!log.exists());
    assert_eq!(fs::read_to_string(&old).unwrap(), "0123456789");
    fs::write(&log, "abcdefghij").unwrap();
    rotate(&log, 5).unwrap();
    assert_eq!(fs::read_to_string(&old).unwrap(), "abcdefghij");
    // Under the limit it is appended to.
    fs::write(&log, "ab").unwrap();
    open(&log, 5, "x\n").unwrap();
    assert_eq!(fs::read_to_string(&log).unwrap(), "abx\n");
    assert_eq!(fs::read_to_string(&old).unwrap(), "abcdefghij");
}

#[test]
fn launch_line_is_first() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("sub/tenmoku.log");
    let line = launch_line(LaunchKind::Detached, 42, Some(2));
    let mut file = open(&log, LIMIT, &line).unwrap();
    writeln!(file, "[WARN x] record").unwrap();
    let text = fs::read_to_string(&log).unwrap();
    let first = text.lines().next().unwrap();
    assert!(first.contains("pid 42") && first.contains("detached") && first.contains("displays 2"));
    assert!(first.contains(env!("CARGO_PKG_VERSION")));
    assert!(text.lines().nth(1).unwrap().contains("record"));
}

/// The child half of [`install_follows_the_real_stderr`]: does nothing unless
/// the parent started it with `TENMOKU_LOG_CHILD`.
#[test]
fn install_child() {
    if std::env::var_os("TENMOKU_LOG_CHILD").is_none() {
        return;
    }
    install(LaunchKind::Detached, Some(1));
    eprintln!("[WARN child] record");
}

#[test]
fn install_follows_the_real_stderr() {
    use std::os::unix::io::FromRawFd as _;
    use std::process::{Command, Stdio};
    // Runs the child with the given stderr and a log optionally prepared
    // first; returns the child's output, its log path and its home.
    let run = |stderr: Stdio, existing: Option<&str>| {
        let home = tempfile::tempdir().unwrap();
        let log = log_path(|name| match name {
            "HOME" => Some(home.path().to_owned()),
            "XDG_STATE_HOME" => Some(home.path().join("state")),
            _ => None,
        })
        .unwrap();
        if let Some(text) = existing {
            fs::create_dir_all(log.parent().unwrap()).unwrap();
            fs::write(&log, text).unwrap();
        }
        let output = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "log_file::tests::install_child", "--nocapture"])
            .env("TENMOKU_LOG_CHILD", "1")
            .env("HOME", home.path())
            .env("XDG_STATE_HOME", home.path().join("state"))
            .stdout(Stdio::null())
            .stderr(stderr)
            .output()
            .unwrap();
        assert!(output.status.success(), "child failed: {output:?}");
        (output, log, home)
    };

    // Null stderr: the launch line and the record land in the log.
    let (_, log, _home) = run(Stdio::null(), None);
    let text = fs::read_to_string(&log).unwrap();
    let mut lines = text.lines();
    assert!(lines.next().unwrap().contains("launch: version"));
    assert!(lines.next().unwrap().contains("record"));

    // Null stderr with a log already there: it is appended to, not replaced.
    let (_, log, _home) = run(Stdio::null(), Some("earlier line\n"));
    let text = fs::read_to_string(&log).unwrap();
    let mut lines = text.lines();
    assert_eq!(lines.next(), Some("earlier line"));
    assert!(lines.next().unwrap().contains("launch: version"));
    assert!(lines.next().unwrap().contains("record"));
    assert_eq!(lines.next(), None);

    // A pipe stays the pipe: the record comes through it, and no log is made.
    let (output, log, _home) = run(Stdio::piped(), None);
    assert!(String::from_utf8_lossy(&output.stderr).contains("record"));
    assert!(!log.exists());

    // A regular file the user redirected to stays: the record is in it.
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("mine.log");
    let file = File::create(&target).unwrap();
    let (_, log, _home) = run(Stdio::from(file), None);
    assert!(fs::read_to_string(&target).unwrap().contains("record"));
    assert!(!log.exists());

    // A terminal stays the terminal: the record is read from the pty master.
    let (mut master, mut slave) = (0, 0);
    // SAFETY: valid out-pointers; the null name/termios/winsize are allowed.
    let made = unsafe {
        libc::openpty(
            &mut master,
            &mut slave,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    assert_eq!(made, 0);
    // SAFETY: both descriptors are open and owned here.
    let (master, slave) = unsafe {
        (
            File::from_raw_fd(master),
            Stdio::from(File::from_raw_fd(slave)),
        )
    };
    // Read concurrently: a pty may drop unread output when the slave closes.
    let reader = std::thread::spawn(move || {
        let mut master = master;
        let mut seen = Vec::new();
        let mut buf = [0u8; 512];
        while let Ok(n) = std::io::Read::read(&mut master, &mut buf) {
            if n == 0 {
                break;
            }
            seen.extend_from_slice(&buf[..n]);
        }
        seen
    });
    let (_, log, _home) = run(slave, None);
    assert!(!log.exists());
    // The slave is closed once the child exits, which ends the read.
    let seen = reader.join().unwrap();
    assert!(String::from_utf8_lossy(&seen).contains("record"));
}
