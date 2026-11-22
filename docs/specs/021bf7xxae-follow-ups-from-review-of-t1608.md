---
task_uid: "01m80egd2qc2f50p021bf7xxae"
task: "T2091"
rev: 1
type: "chore"
complexity: "s"
criteria: []
---

# Follow-ups from review of T1608

Findings of the review of T1608 that don't block the merge (below merge.block_severity):

- F4 medium crates/fastedit/src/log_file/tests.rs:104: The subprocess test now exercises install and fd 2, but the previous coverage gap is only partially resolved: there is still no terminal case, no install test preserving user-redirected file stderr, and no fd-2 append test against a preexisting log. The separate open test does not establish append preservation through install.
  fix: Extend the isolated subprocess coverage with regular-file and pseudo-terminal stderr, and prepopulate the null-stderr child's log before invoking install. Assert preserved destinations, preserved existing log contents, appended header and record, and successful child exit. A pseudo-terminal can be created using the existing libc dependency.
  instances: crates/fastedit/src/log_file/tests.rs:12, crates/fastedit/src/log_file/tests.rs:104
