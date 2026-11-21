---
task_uid: "01m7zkcz4dcqtvhnmgheyx9dvf"
task: "T740"
rev: 5
type: "bugfix"
complexity: "s"
criteria:
  - id: "C1"
    text: "restore() compares the trashed entry's full id including birth time (Trashed::id with EntryId.born) at all three checks: before staging, after staging, and once put back"
    required: true
    manual: false
  - id: "C2"
    text: "fs_ops_undo::another_entry_in_its_place_in_the_trash_is_not_taken passes on Linux (ext4 and tmpfs) with no hard-link workaround, and stays green on macOS"
    required: true
    manual: false
---

# Trash restore ignores the birth-time EntryId: a re-created file with a reused inode is restored over as the trashed one on Linux (T1953 follow-up) (Mac T2084)

## Implementation notes

- C2: tmpfs numbers inodes incrementally and can't reuse one, so the test can't reproduce the bug there. fs_ops_undo pins its projects, and the per-process test trash with them, to CARGO_TARGET_TMPDIR (ext4 here and on GitHub) through the new test-support `testing::put_trash_under`; other crates keep `temp_dir()`.
