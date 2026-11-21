//! Putting back what `fs_ops::trash_recorded` moved to the Trash
//! (`fs_ops::restore`), as the project tree's Undo does: always against the
//! test trash of the process's own (`fs_ops::testing`), never the user's.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use fe_project::fs_ops::{self, testing};

/// A temporary project, by its canonical path.
///
/// Projects, and the test trash with them, live under the target folder,
/// not the temporary one: tmpfs (`/tmp` on many Linux machines) numbers
/// inodes incrementally and never reuses one, so a test of a re-created
/// file with a reused inode can't happen there. The trash is per process,
/// so every test of this binary is rooted there, or one in a tmpfs folder
/// would cross devices.
fn project() -> (tempfile::TempDir, PathBuf) {
    static UNDER: std::sync::Once = std::sync::Once::new();
    UNDER.call_once(|| {
        // SAFETY: this runs before anything is trashed, and every test of
        // this binary goes through here first (each in a process of its own
        // under nextest; under `cargo test` the others wait on the `Once`).
        unsafe { std::env::set_var("FE_TEST_TRASH_ROOT", env!("CARGO_TARGET_TMPDIR")) };
    });
    let dir = tempfile::Builder::new()
        .prefix("fe-project-undo-")
        .tempdir_in(env!("CARGO_TARGET_TMPDIR"))
        .unwrap();
    let root = dunce::canonicalize(dir.path()).unwrap();
    (dir, root)
}

/// The one entry trashed from `path`, which must have gone to the test
/// trash.
fn trash_one(root: &Path, path: &Path) -> fs_ops::Trashed {
    let mut trashed = fs_ops::trash_recorded(root, &[path]).unwrap();
    assert_eq!(trashed.len(), 1, "{trashed:?}");
    let trashed = trashed.pop().unwrap();
    assert_eq!(trashed.path(), path);
    let at = trashed.location().expect("the test trash says where");
    assert!(at.starts_with(testing::trash_dir()), "{at:?}");
    assert!(!path.exists());
    trashed
}

#[test]
fn a_trashed_file_and_folder_are_put_back() {
    let (_dir, root) = project();
    let file = root.join("notes.txt");
    fs::write(&file, "kept\n").unwrap();
    let folder = root.join("src");
    fs::create_dir_all(folder.join("deep")).unwrap();
    fs::write(folder.join("deep/main.rs"), "fn main() {}\n").unwrap();

    let mut trashed = trash_one(&root, &file);
    assert_eq!(fs_ops::restore(&root, &mut trashed).unwrap(), file);
    assert_eq!(fs::read_to_string(&file).unwrap(), "kept\n");

    let mut trashed = trash_one(&root, &folder);
    assert_eq!(fs_ops::restore(&root, &mut trashed).unwrap(), folder);
    assert_eq!(
        fs::read_to_string(folder.join("deep/main.rs")).unwrap(),
        "fn main() {}\n"
    );

    // Put back already: it isn't in the Trash.
    let err = fs_ops::restore(&root, &mut trashed).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::NotFound);
}

#[test]
fn an_entry_inside_another_goes_with_it() {
    let (_dir, root) = project();
    fs::create_dir_all(root.join("app")).unwrap();
    fs::write(root.join("app/lib.rs"), "").unwrap();
    fs::write(root.join("b.txt"), "").unwrap();

    let mut trashed = fs_ops::trash_recorded(
        &root,
        &[
            root.join("b.txt"),
            root.join("app/lib.rs"),
            root.join("app"),
        ],
    )
    .unwrap();
    let paths: Vec<&Path> = trashed.iter().map(fs_ops::Trashed::path).collect();
    assert_eq!(paths, [root.join("app"), root.join("b.txt")]);
    for trashed in &mut trashed {
        fs_ops::restore(&root, trashed).unwrap();
    }
    assert!(root.join("app/lib.rs").is_file());
    assert!(root.join("b.txt").is_file());
}

#[test]
fn a_taken_path_is_never_replaced() {
    let (_dir, root) = project();
    let file = root.join("a.txt");
    fs::write(&file, "old\n").unwrap();
    let mut trashed = trash_one(&root, &file);
    fs::write(&file, "new\n").unwrap();

    let err = fs_ops::restore(&root, &mut trashed).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);
    assert_eq!(fs::read_to_string(&file).unwrap(), "new\n");
    let at = trashed.location().unwrap();
    assert_eq!(
        fs::read_to_string(at).unwrap(),
        "old\n",
        "still in the Trash"
    );

    // Once it's free again, it goes back.
    fs::remove_file(&file).unwrap();
    fs_ops::restore(&root, &mut trashed).unwrap();
    assert_eq!(fs::read_to_string(&file).unwrap(), "old\n");
}

#[test]
fn another_entry_in_its_place_in_the_trash_is_not_taken() {
    let (_dir, root) = project();
    let file = root.join("a.txt");
    fs::write(&file, "trashed\n").unwrap();
    let mut trashed = trash_one(&root, &file);
    let at = trashed.location().unwrap().to_owned();
    fs::remove_file(&at).unwrap();
    fs::write(&at, "another\n").unwrap();
    #[cfg(unix)]
    assert_ne!(
        identity(&at),
        trashed.id(),
        "the replacement is another entry, whatever inode it got"
    );

    let result = fs_ops::restore(&root, &mut trashed);
    assert!(result.is_err(), "restored over another entry: {result:?}");
    let err = result.unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::NotFound);
    assert!(!file.exists());
    assert_eq!(fs::read_to_string(&at).unwrap(), "another\n");
}

/// `trashed` as if its entry had been born at another time than it was, with
/// the same device and inode: what a file re-created with a reused inode
/// looks like to the checks, on any file system.
fn born_at_another_time(trashed: &fs_ops::Trashed) -> fs_ops::Trashed {
    let mut record = fe_host::TrashRecord::from(trashed.clone());
    record.id.born = Some(match record.id.born {
        Some((secs, nanos)) => (secs + 1, nanos),
        None => (1, 0),
    });
    assert_ne!(record.id, trashed.id());
    fs_ops::Trashed::from(record)
}

#[test]
fn an_entry_with_the_same_inode_born_at_another_time_is_not_restored() {
    let (_dir, root) = project();
    let file = root.join("a.txt");
    fs::write(&file, "trashed\n").unwrap();
    let trashed = trash_one(&root, &file);
    let at = trashed.location().unwrap().to_owned();
    let mut other = born_at_another_time(&trashed);

    let result = fs_ops::restore(&root, &mut other);
    assert!(result.is_err(), "restored the wrong entry: {result:?}");
    assert_eq!(result.unwrap_err().kind(), io::ErrorKind::NotFound);
    assert!(!file.exists());
    assert_eq!(fs::read_to_string(&at).unwrap(), "trashed\n");
    assert_eq!(other.location(), Some(at.as_path()), "not moved");
}

#[test]
fn an_entry_with_the_same_inode_born_at_another_time_is_not_in_the_trash() {
    let (_dir, root) = project();
    let file = root.join("a.txt");
    fs::write(&file, "trashed\n").unwrap();
    let trashed = trash_one(&root, &file);
    assert!(trashed.in_trash().unwrap());
    let other = born_at_another_time(&trashed);
    assert!(
        !other.in_trash().unwrap(),
        "the entry there isn't the one recorded"
    );
}

#[test]
fn a_folder_gone_since_fails() {
    let (_dir, root) = project();
    fs::create_dir(root.join("app")).unwrap();
    let file = root.join("app/a.txt");
    fs::write(&file, "").unwrap();
    let mut trashed = trash_one(&root, &file);
    fs::remove_dir(root.join("app")).unwrap();

    let err = fs_ops::restore(&root, &mut trashed).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::NotFound);
    assert!(trashed.location().unwrap().exists(), "still in the Trash");
    assert!(
        trashed.in_trash().unwrap(),
        "and can go back once it's there"
    );
    fs::create_dir(root.join("app")).unwrap();
    fs_ops::restore(&root, &mut trashed).unwrap();
    assert!(file.is_file());
    assert!(!trashed.in_trash().unwrap());
}

/// What the entry at `path` is, as the project tree looks it up (its
/// device and inode, and its birth time where there is one).
#[cfg(unix)]
fn identity(path: &Path) -> fs_ops::EntryId {
    fs_ops::entry_identity(path).unwrap()
}

#[test]
#[cfg(unix)]
fn creates_and_copies_say_what_they_made() {
    let (_dir, root) = project();
    fs::create_dir(root.join("there")).unwrap();

    // Only the folders it made count: `there` was there.
    let created = fs_ops::create_recorded(&root, &root, "there/new/a.rs", false).unwrap();
    assert_eq!(created.path, root.join("there/new/a.rs"));
    assert!(created.path.is_file());
    assert_eq!(created.first.path, root.join("there/new"));
    assert_eq!(created.first.id, identity(&root.join("there/new")));
    let created = fs_ops::create_recorded(&root, &root, "there/b", true).unwrap();
    assert!(created.path.is_dir());
    assert_eq!(created.first.path, root.join("there/b"));
    assert_eq!(created.first.id, identity(&created.path));

    let copy = fs_ops::duplicate_recorded(&root, &root.join("there/new/a.rs")).unwrap();
    assert_eq!(copy.path, root.join("there/new/a copy.rs"));
    assert_eq!(copy.id, identity(&copy.path));
    let copy = fs_ops::copy_into_recorded(&root, &root.join("there/new"), &root).unwrap();
    assert_eq!(copy.path, root.join("new"));
    assert_eq!(copy.id, identity(&copy.path));

    // What restore puts back is what was trashed.
    let mut trashed = trash_one(&root, &copy.path);
    assert_eq!(trashed.id(), copy.id);
    fs_ops::restore(&root, &mut trashed).unwrap();
    assert_eq!(identity(&copy.path), copy.id);
}

#[test]
fn only_a_trash_that_says_where_restores() {
    assert_eq!(fs_ops::SYSTEM_TRASH_RESTORES, cfg!(target_os = "macos"));
    // The test trash always says.
    assert!(fs_ops::restores_from_trash());
}
