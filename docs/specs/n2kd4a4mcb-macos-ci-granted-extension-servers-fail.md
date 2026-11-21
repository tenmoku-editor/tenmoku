---
task_uid: "01m7zk5sgah1w7r1n2kd4a4mcb"
task: "T2083"
rev: 1
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
