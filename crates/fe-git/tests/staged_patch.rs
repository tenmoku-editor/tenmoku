// Its executable bit is set by mode.
#![cfg(unix)]

//! A staged file's patch, as `git diff --cached` writes it, against temp
//! repositories: mode changes, the final line break, binary files, and
//! the user's config left out of the format.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

use fe_git::Repo;

fn git(dir: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(["-c", "init.defaultBranch=main"])
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn a_staged_patch_keeps_modes_line_breaks_and_binary_files() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    git(root, &["init", "-q"]);
    for (key, value) in [
        ("user.name", "t"),
        ("user.email", "t@t"),
        ("commit.gpgsign", "false"),
        ("diff.noprefix", "true"),
        ("color.diff", "always"),
    ] {
        git(root, &["config", key, value]);
    }
    fs::write(root.join("run.sh"), "echo hi\n").unwrap();
    fs::write(root.join("gains.txt"), "foo").unwrap();
    fs::write(root.join("loses.txt"), "bar\n").unwrap();
    fs::write(root.join("logo.bin"), b"\x00\x01").unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-q", "-m", "init"]);
    let run = root.join("run.sh");
    fs::set_permissions(&run, fs::Permissions::from_mode(0o755)).unwrap();
    fs::write(root.join("gains.txt"), "foo\n").unwrap();
    fs::write(root.join("loses.txt"), "bar").unwrap();
    fs::write(root.join("logo.bin"), b"\x00\x02").unwrap();
    git(root, &["add", "."]);

    let repo = Repo::discover(root).unwrap().unwrap();
    let patch = |name: &str| repo.staged_patch(&repo.workdir().join(name)).unwrap();
    assert_eq!(
        patch("run.sh"),
        "diff --git a/run.sh b/run.sh\nold mode 100644\nnew mode 100755\n"
    );
    let gains = patch("gains.txt");
    assert!(
        gains.ends_with("@@ -1 +1 @@\n-foo\n\\ No newline at end of file\n+foo\n"),
        "{gains}"
    );
    let loses = patch("loses.txt");
    assert!(
        loses.ends_with("@@ -1 +1 @@\n-bar\n+bar\n\\ No newline at end of file\n"),
        "{loses}"
    );
    let binary = patch("logo.bin");
    assert!(
        binary.contains("\nBinary files a/logo.bin and b/logo.bin differ\n"),
        "{binary}"
    );
    fs::write(root.join("unstaged.txt"), "x\n").unwrap();
    assert_eq!(patch("unstaged.txt"), "");
}

#[test]
fn a_staged_patch_is_of_the_one_path_named() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    git(root, &["init", "-q"]);
    for (key, value) in [
        ("user.name", "t"),
        ("user.email", "t@t"),
        ("commit.gpgsign", "false"),
    ] {
        git(root, &["config", key, value]);
    }
    fs::write(root.join("config"), "plain\n").unwrap();
    fs::create_dir(root.join("old")).unwrap();
    fs::write(root.join("old/.env"), "OLD=hush\n").unwrap();
    fs::write(root.join("*"), "star\n").unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-q", "-m", "init"]);
    // A file become a directory, and a directory become a file, each
    // with a secret under the directory.
    git(root, &["rm", "-q", "--cached", "config", "-r", "old"]);
    fs::remove_file(root.join("config")).unwrap();
    fs::remove_dir_all(root.join("old")).unwrap();
    fs::create_dir(root.join("config")).unwrap();
    fs::write(root.join("config/.env"), "NEW=hush\n").unwrap();
    fs::write(root.join("old"), "now a file\n").unwrap();
    fs::write(root.join("*"), "star\nstaged\n").unwrap();
    fs::write(root.join("x.rs"), "fn x() {}\n").unwrap();
    git(root, &["add", "."]);

    let repo = Repo::discover(root).unwrap().unwrap();
    let patch = |name: &str| repo.staged_patch(&repo.workdir().join(name)).unwrap();
    let config = patch("config");
    assert!(config.starts_with("diff --git a/config b/config\ndeleted file mode"));
    let old = patch("old");
    assert!(old.starts_with("diff --git a/old b/old\nnew file mode"));
    for (name, patch) in [("config", &config), ("old", &old)] {
        assert_eq!(patch.matches("diff --git").count(), 1, "{name}: {patch}");
        assert!(!patch.contains("hush"), "{name}: {patch}");
    }
    // `*` is a name, not a glob.
    let star = patch("*");
    assert_eq!(star.matches("diff --git").count(), 1, "{star}");
    assert!(star.ends_with(" star\n+staged\n"), "{star}");
    assert!(patch("config/.env").contains("+NEW=hush\n"));
}
