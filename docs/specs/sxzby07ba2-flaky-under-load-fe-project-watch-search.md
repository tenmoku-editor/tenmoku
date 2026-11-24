---
task_uid: "01m83k8t87cdhzw3sxzby07ba2"
task: "T2104"
rev: 2
type: "chore"
complexity: "s"
criteria:
  - id: "C1"
    text: "Known limit: the cause of the two post-merge failures of 9fed701fb (runs 01m83gg09988ppsk97q9h46qk6, 01m83htd3kzhw4wxjqswmag1gj; 'the watcher saw the last file' after a 10.1-10.4 s wait, load 17-21 with XprotectService ~960% CPU) is not determinable: whether FSEvents delivered late or lost the event cannot be told without a reproduction, and it did not reproduce under load or after fseventsd's restart (the lead attributes the failures to a bloated fseventsd, 8 days up, 1.7 GB; unverified). The test tolerates both: a 60 s deadline, and last.rs rewritten after 3 s of quiet. On failure it prints the timeline of what each watcher saw. The finish note states this limit and claims no cause."
    required: true
    manual: false
  - id: "C2"
    text: "The fix is to the test only (crates/fe-project/tests/watch_search_host.rs): a longer deadline for each watcher's wait that still fails fast when nothing arrives, and re-touching the last file until it is seen (only on a receive timeout; a disconnected watcher fails at once); the test still checks that both watchers report the create, rename and removal set. The comparison is narrowed to the five paths the test changes (new, old, moved, gone, last), each of which both watchers must report, because setup events (the root, src, main.rs) can reach one watcher only (30-pass load attempt 1, runs 2 and 19). If the cause is in fe-project production code, stop and respec."
    required: true
    manual: false
  - id: "C3"
    text: "fe-project::watch_search_host a_hosted_watch_delivers_the_watchers_changes passes 30 times in a row under load (a parallel cargo nextest of fe-project and fe-workbench, or stress-ng), shown in the finish note with the command."
    required: true
    manual: false
---

# Flaky under load: fe-project watch_search_host a_hosted_watch_delivers_the_watchers_changes (the watcher saw the last file, 10 s FSEvents wait)
