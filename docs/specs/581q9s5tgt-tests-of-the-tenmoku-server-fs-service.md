---
task_uid: "01m813xq8dm4r9z8581q9s5tgt"
task: "T409"
rev: 1
type: "chore"
complexity: "s"
criteria:
  - id: "C1"
    text: "tenmoku-server fs_service tests pass with a temp root reached through a symlinked parent"
    required: true
    manual: false
  - id: "C2"
    text: "fe-remote-client bootstrap and bootstrap_tools tests pass with TMPDIR set to a path longer than 60 characters, and leave no lock files in /tmp/tenmoku-server-*"
    required: true
    manual: false
---

# Tests of the tenmoku-server fs service and the bootstrap pruning assume a short, symlink-free temp dir (macOS gate of batch 75, Mac T2046 and T2047 follow-up)

Mac batch 75 (T405 + T397) failed the macOS gate on test-only assumptions. Full details and fixes: ~/coordination/from-mac-lead-batch-75-macos.md. Test code only, no product change.

1. crates/tenmoku-server/tests/fs_service.rs: Fixture's root is a raw tempfile::tempdir() (/var/folders/... on macOS, /var is a symlink) and fe-project opens roots without following symlinks. Canonicalize the root once in Fixture and derive every at()/wire()/list_tree path from it (also the `elsewhere` tempdir at ~:174).
2. crates/fe-remote-client/tests/bootstrap.rs and bootstrap_tools.rs: with a long TMPDIR tenmoku-server Paths::new (src/paths.rs) overflows SOCKET_PATH_MAX and falls back to the shared /tmp/tenmoku-server-<uid>/<build>.lock, so tests leak and collide on old-1.lock. Keep the tests off that fallback: a short base for the Host dir (tempfile::Builder::new().tempdir_in("/tmp")) or an explicit short XDG_RUNTIME_DIR passed to both hold_lock's Paths::new and the script env, and unique build names per test.

Reproduce on Linux: TMPDIR set to a path longer than 60 characters (group 2), and a root reached through a symlinked parent (group 1). Check both groups pass that way.
