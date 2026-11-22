---
task_uid: "01m8015vqvvt1ce8vnz3hh0a0b"
task: "T2089"
rev: 2
type: "feature"
complexity: "m"
criteria:
  - id: "C1"
    text: "start's cwd authority is the WASI preopens themselves: a root whose preopen failed is not a root for start, and a root retargeted after its preopen keeps the preopened directory; a deterministic hook between the preopen and the store setup retargets a root link, and start with a cwd in it runs in the preopened directory (test in kiln/crates/kiln-plugin/tests/process_stream_handles.rs)"
    required: true
    manual: false
  - id: "C2"
    text: "On Unix the program starts in the directory handle resolved at start: a hook between the final resolution and the launch replaces a cwd ancestor with a link out of the root, and the program still reports the original directory (pwd -P)"
    required: true
    manual: false
  - id: "C3"
    text: "On Windows the path launch and the recheck remain, with the window recorded in wit/process.wit and process_stream's module docs; the Unix test's Windows twin asserts the documented behaviour"
    required: true
    manual: false
  - id: "C4"
    text: "kiln's no_gpui stage passes: every OS branch lives in kiln-core/src/platform"
    required: true
    manual: false
---

# Handle-based cwd for process.start: share the WASI preopen and launch into an open directory on Unix, Windows a documented limit (T1703 follow-up)

# Handle-based cwd for process.start: share the WASI preopen and launch into an open directory on Unix, Windows a documented limit (T1703 follow-up)

Follow-up to T1703 (B4, process.start). T1703 rev 3 accepts two windows as a known limit: `start`'s `cwd` is a convenience checked at start, not a security boundary. This card closes both on Unix and documents what stays open on Windows.

## Windows to close

1. **Root open vs WASI preopen.** `process_stream::Streams::set_roots` pins each root (`Root::pin`: open_dir, identity, canonical path) apart from `fs::configure`, which calls `WasiCtxBuilder::preopened_dir(path, …)` and opens each root by path again. A root retargeted between the two opens names a different directory for `start` than WASI holds, or stays a root for `start` when its preopen failed.
2. **Check then launch.** `process_stream::start` rechecks the resolved cwd (`Streams::recheck`), then calls `Command::current_dir(path)` and `ProcessTree::spawn`. A directory replaced between the recheck and the child's chdir starts the program elsewhere.

## Proposed approach

- **kiln-plugin/src/fs.rs:** open each workspace root once, as a directory handle, and preopen WASI from that handle rather than the path. wasmtime-wasi 48's public `preopened_dir` takes a path, so check first whether it offers a handle-taking entry point (cap-std `Dir`). If not, open by handle and verify the preopen's identity afterwards, and say which. Hand the same opened roots, or only those whose preopen succeeded, to `process_stream` (the runner sets them via `ProcessState::with_roots`), so `start`'s authority is exactly the WASI preopens.
- **kiln-core/src/platform/process_tree.rs:** add a spawn that starts the program in an open directory. On Unix that is a `pre_exec` `fchdir(fd)`, where the fd is the cwd resolved relative to the root handle (openat with O_DIRECTORY|O_NOFOLLOW per component, or the resolved dir's fd). On Windows `CreateProcess` takes the directory as a path, so keep the path launch and the recheck, and document the window as a Windows-only known limit.
- **kiln-plugin/src/process_stream.rs:** resolve `cwd` relative to the root handle rather than by canonical path string, and pass the resulting directory handle to the new spawn. Keep the path recheck on Windows only, through the kiln-core platform API, since no_gpui forbids OS cfg in kiln-plugin.

## Known limits

- **A→B→A retarget of a root before the preopen check (F1).** wasmtime-wasi 48 can't preopen from an open directory handle: `WasiCtxBuilder::preopened_dir` takes a path, and the builder's preopens (and the `WasiCtx`'s) are private, so a `Dir` built from a handle can't be installed and the identity of what a preopen holds can't be read back. `fs::configure_roots` therefore opens the root as a handle, preopens by path, and checks the path's identity against the handle's right after. A root link retargeted to another directory and put back to the original between the handle's open and that check passes it, so `start` holds A while WASI holds B. This is the spec's own fallback ("open by handle and verify the preopen's identity afterwards"), and the window is narrower than before (two path opens and a check, not the store's lifetime). A handle-backed preopen follows when wasmtime-wasi allows one. (`/dev/fd/N` was tried: macOS refuses it as a directory.)
- **`fe-workbench` start roots.** `crates/fe-workbench` (outside this card's scope) still sets `start`'s roots with `process::State::with_roots(Vec<PathBuf>)` from `process_stream::preopened`, which opens each root by path, so there the roots are not the WASI preopens. The runner in kiln-plugin uses `with_preopened`. Switching `fe-workbench` to it is a follow-up.
- **Windows launch window** (C3): as documented in `wit/process.wit`.
