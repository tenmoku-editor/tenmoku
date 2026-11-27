---
task_uid: "01m8cdp08sdra6zz1rsb52jr1k"
task: "T2117"
rev: 2
type: "chore"
complexity: "s"
criteria: []
---

# Follow-ups from review of T1660

Findings of the review of T1660 that don't block the merge (below merge.block_severity):

- F1 medium crates/fe-workbench/src/task_defs.rs:569: Gradle wrapper selection checks only file existence. DiskFiles::exists uses is_file(), so a non-executable gradlew still replaces gradle for all five tasks, including default build/test, and execution fails with permission denied on Linux. This misses the spec's executable-looking wrapper requirement; the new fake cannot represent this case.
  fix: Expose executable-file detection through ProjectFiles and prefer ./gradlew only when it qualifies; otherwise use gradle. Add tests distinguishing executable and non-executable wrappers and update the detection documentation.
  instances: crates/fe-workbench/src/task_defs.rs:569

## Implementation notes

- `ProjectFiles::is_executable` defaults to `exists`, so a source that can't
  tell keeps preferring a wrapper that is there. `DiskFiles` answers with
  `test_runner::runnable`.
- `mvnw` had the same fault as `gradlew` and is fixed with it, at the lead's
  request.
