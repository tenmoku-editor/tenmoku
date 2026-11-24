---
task_uid: "01m7zk5sgah1w7r1n2kd4a4mcb"
task: "T2083"
rev: 2
type: "bugfix"
complexity: "m"
criteria:
  - id: "C1"
    text: "A failing Snapshots::snapshot keeps its cause: the step (stage, copy, publish, rename, lock), the path and the io error reach both the warn log and the error grant_extension_server returns, instead of a bare PACKAGE_INCOMPLETE InvalidData"
    required: true
    manual: false
  - id: "C2"
    text: "The cause of the GitHub macOS runner failure is found from a CI run of a ci/t\u003cn\u003e-macos evidence branch (ask the lead; the user pushes it), named in the spec's implementation notes, and fixed"
    required: true
    manual: false
  - id: "C3"
    text: "On that CI run's macOS check job, lsp_consent::lsp_consent_snapshot_tests::*, lsp_consent_tests, lsp_consent_relaunch_tests, lsp_consent_package_tests, extensions_picker_disable_tests, plugin_lsp_tests::a_plugin_reaches_its_extensions_consented_server_through_a_pending_change and fastedit's extension_consent_tree and extension_disabled::the_setting_reaches_the_app pass (cite the run URL)"
    required: true
    manual: false
  - id: "C4"
    text: "If the cause can hit a user's machine (e.g. a macOS version or volume type), say so in the implementation notes; the release v0.2.6 waits on this card"
    required: true
    manual: false
---

# macOS CI: granted extension servers fail to snapshot on the GitHub macOS runner (grant_extension_server -> InvalidData), ~57 tests red since T2024

## Implementation notes

### C1: a failed snapshot keeps its cause

`Session::snapshot` names the step that failed (`check`, `hash`, `reuse`,
`lock`, `stage`, `copy`, `publish`, `rename`), the path and the io error, as
`<step> <path>: <error>` with the io error's kind kept. Hashing a package
says which entry stopped it and why (`package_hash_or_why`), and a snapshot
found unsound says what it hashes to or which entry is writable. The
evaluation logs that text in its warning and keeps it on the server, and
`grant_extension_server` returns it (`can't snapshot the package of <id>:
<cause>`), still `InvalidData`. Test:
`lsp_consent::lsp_consent_snapshot_tests::a_failed_snapshot_keeps_its_cause`
(runs `a_failed_snapshot_names_its_step_path_and_cause` in a process of its
own, for its logger).

Review round 1 (F2): a lock that can't be taken, as opposed to one that's
only held, is now an error naming the lock file and its cause. Before, a
snapshot's lock read as "damaged and in use" and a staging lock as "no free
staging directory". The walk over a staged copy names the nested path it
failed on, not the copy's root. Tests, in `lsp_consent::lsp_consent_snapshot::cause_and_race_tests`:
`lock_failures_keep_their_cause`, which checks the warning and the grant
error for both locks, in a process of its own;
`a_staging_lock_that_cant_be_taken_is_named_with_its_cause`;
`a_snapshot_lock_that_cant_be_taken_is_named_with_its_cause`; and
`a_walk_names_the_nested_path_it_failed_on`. A test seam (`fail_lock`)
injects the lock error.

### C2: the cause on the GitHub macOS runner

Evidence run: PR 11, branch `ci/t2083-macos` at `33d7dcd03` (the C1 commit
plus an evidence-only `ci.yml` that runs these tests and prints the runner's
environment), run
https://github.com/tenmoku-editor/tenmoku/actions/runs/45041720622, job
`check` (137267417982): 110 run, 54 passed, 56 failed. Each failure with an
error to show reads like this one:

    can't snapshot the package of acme.sh: rename
    /var/folders/…/T/.tmpdG8ZkC/extensions-lsp-packages/.staging-50245-0 ->
    /var/folders/…/T/.tmpdG8ZkC/extensions-lsp-packages/8273efa0…b434:
    Permission denied (os error 13)

That's all 43 errors the run shows. The other 13 failures are assertions
that a hash isn't `incomplete`, or on counts that follow from it.

Cause: `publish` cleared the write bits of the whole staged copy, its top
directory included, and then renamed that directory to its hash. On the
runner (macOS 15.8.2, 24H232, Darwin 24.6.0, APFS data volume, uid 501
`runner` in admin, umask 022), `rename(2)` of a directory its owner can't
write fails with EACCES, even within one parent directory. This Mac
(macOS 27.0) and Linux allow that rename, which is why the local gate and
the Linux job passed.

Fix: the staged copy's contents are made read-only before the rename, as
before, and the directory's own write bits are cleared right after it
(`make_readonly_within`, then `set_readonly`). A directory with a hash's
name is finished once it is read-only. From before the rename to after the
chmod, the publisher holds `<hash>.publishing.lock` exclusively. A check
that finds the snapshot unfinished (reuse, `verified`, `hold`) takes that
lock shared and checks again. So a check in another instance during the
gap waits for the publish rather than recording an incomplete package. A
second publisher of the same hash waits on the lock before its rename, which
then fails, and it uses the first one's snapshot (review round 1, F1). A
crash in the gap releases the lock and leaves a writable snapshot, which is
replaced when nothing holds it, as before
(`a_writable_snapshot_left_by_a_crash_is_replaced_not_reused`). Tests, using
a seam that pauses after the rename:
`lsp_consent::lsp_consent_snapshot::cause_and_race_tests::a_check_while_a_snapshot_is_published_waits_for_it_and_reuses_it`
and
`…::a_publisher_that_loses_the_race_waits_for_the_winner_and_uses_its_snapshot`.

### C3: the macOS check job with the fix

Run https://github.com/tenmoku-editor/tenmoku/actions/runs/45233900230
(PR 11, `ci/t2083-macos` at `66cc970cb`, which is the fix commit plus the
same evidence-only `ci.yml` commit), job `check` 137900378441, on the same
runner image (macOS 15.8.2). Result: success. The same nextest filter as
the first run gave 110 run and 110 passed, against 54 passed before the fix:

- `lsp_consent::lsp_consent_snapshot_tests::*`: 29 passed, including
  `a_failed_snapshot_keeps_its_cause`
- `lsp_consent_tests`: 15
- `lsp_consent_relaunch_tests`: 2
- `lsp_consent_package_tests`: 12
- `extensions_picker_disable_tests`: 7
- `plugin_lsp_tests::a_plugin_reaches_its_extensions_consented_server_through_a_pending_change`
- fastedit's `extension_consent_tree`: 9 (4 of its own and 5 theme tests)
- `extension_disabled::the_setting_reaches_the_app`

`extension_consent` and `extension_servers` passed too.

### C4: users' machines

Yes. The cause is the macOS version, not the runner's volume type: the
runner's data volume is ordinary APFS, the same as a user's Mac. On
macOS 15, and presumably earlier versions (not checked), every attempt to
snapshot a granted extension's language server package fails at `rename`.
No extension language server can be granted there, so none runs, and before
C1 the only sign was a bare `InvalidData`. The app supports macOS 11 and later
(`LSMinimumSystemVersion` 11.0), so those users are in range. No release has
the snapshots yet: T2024 (`811182dba`) came after v0.2.5. So no user has met
this, but v0.2.6 would have shipped it without this fix.
