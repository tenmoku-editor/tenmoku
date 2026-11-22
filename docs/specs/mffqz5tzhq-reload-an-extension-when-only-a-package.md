---
task_uid: "01m7w5r7x9j0xg2dmffqz5tzhq"
task: "T432"
rev: 3
type: "bugfix"
complexity: "s"
criteria:
  - id: "C1"
    text: "On Unix, a change to only the execute bits of a regular file in an installed extension package fires the extension reload, including a change that leaves it executable (0o755 to 0o744); a change to only its write bits does not"
    required: true
    manual: false
  - id: "C2"
    text: "Entries that are not regular files, and all fingerprints off Unix, are unchanged (off Unix the new platform helper reports no execute bits and never consults PATHEXT); existing extensions_fingerprint*, extensions_reload and extension_* tests still pass"
    required: true
    manual: false
---

# Reload an extension when only a package file's execute bits change, so consent is checked again (review follow-up of T2024 F10) (Mac T2033)

{"mode": "inline", "body": "Review follow-up of T2024 F10 (codex review jvbefp, candidate 4y2b8q, medium, marked follow-up; outside T2024's scope).\n\nT2024 puts a file's execute bits into the package hash a grant is bound to (review F9). The extension reload watcher in kiln/crates/kiln/src/extensions.rs (`entry_of`, `EntryKind::File`, about line 164) fingerprints package entries by relative path, type, size/mtime and link target, but not by permissions. So a chmod-only update to an installed extension package (for example `chmod +x` on its server entrypoint) fires no reload: consent is not re-evaluated, the package hash the grant was made on no longer matches the live package, and the user keeps running the old snapshot with no new prompt until something else changes.\n\nFix: include the execute bits (user, group, other) of regular files in the watcher's entry fingerprint on Unix, matching what T2024 hashes; write bits stay out, as in T2024. Off Unix nothing changes.\n\nRegression test (new file kiln/crates/kiln/tests/extensions_watch_exec_bits.rs, Unix only): install an extension package, let the watcher settle, change only the execute bits of one file, and assert a reload fires. A second case changes only the write bit and asserts no reload. The test must fail at base.", "ref": "", "ref_blob": ""}


## Implementation notes

The no_gpui lint keeps cfg(unix) out of extensions.rs, so the raw execute bits come from a new kiln_core::platform::execute_bits (0 off Unix), in kiln-core/src/platform/fs.rs and mod.rs: a scope expansion the review findings (F1: any single bit; F2: no PATHEXT off Unix) required.
