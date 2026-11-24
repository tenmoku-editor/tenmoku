---
task_uid: "01m81e1bme78ngyktmpt5xb9w1"
task: "T2098"
rev: 1
type: "chore"
complexity: "xs"
criteria:
  - id: "C1"
    text: "A GitHub CI run of the change shows the linux job passing the \"Kiln builds and tests on its own\" step and the launch step, with df before and after it in the log."
    required: true
    manual: false
  - id: "C2"
    text: "The workspace Test step and the rust-cache save are unchanged in what they build and cache."
    required: true
    manual: false
---

# Linux CI: kiln's stand-alone build runs out of runner disk after the workspace tests (No space left on device)

# Linux CI: room for kiln's stand-alone build

GitHub CI's `linux` job on main 5f8fabeaf (run 45011030559) passed Clippy and Test (11911/11911). Then the "Kiln builds and tests on its own" step (scripts/check-framework-standalone.sh, target under /tmp) died with `rustc-LLVM ERROR: IO failure ... No space left on device` while compiling kiln's `default_menus_catalog` test. f3ba1971a (run 45007670495) stopped at the same step. The job's "Free runner disk" step already removes the preinstalled SDKs. The workspace's debug test build plus the stand-alone target no longer both fit on the disk.

Make room before that step, in .github/workflows/ci.yml only. For example, delete the workspace's test binaries and incremental dirs (target/debug/deps test executables, target/debug/incremental) once Test has passed. Keep whatever the launch step and the rust-cache save still need. Alternatively, remove more preinstalled tools. Leave df around the step so the margin shows in the log. Say in the finish note how much space each removal frees, read from the df lines of a GitHub run.

This is a workflow edit, so it needs the user's risk approval. Read the files named before starting. Do not widen --scope; if a file outside it is needed, stop and respec.
