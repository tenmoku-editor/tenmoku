---
task_uid: "01m86a624hsy42d2axcrv8ggt0"
task: "T2113"
rev: 1
type: "chore"
complexity: "m"
criteria: []
---

# Follow-ups from review of T1483

Findings of the review of T1483 that don't block the merge (below merge.block_severity):

- F2 medium scripts/publish-grammar-registry.sh:195: Freshness checking is bypassed when an existing index release lacks index.json. After B publishes serial 8, a failed retry can delete index.json before its replacement uploads: this is documented behavior of [gh --clobber](https://cli.github.com/manual/gh_release_upload). Rerunning A at serial 7 then skips this guard and replaces B's packs and index. Concurrency does not prevent this sequential failure. The fake uploader only copies files and cannot exercise the deletion/failure state.
  fix: Preserve a durable publication watermark independently of the clobbered index, or enforce an equivalent workflow freshness guard; refuse publication when freshness cannot be established while retaining safe bootstrap and same-version recovery. Add a regression covering B published, B retry failing after index deletion, then A attempted.
  instances: scripts/publish-grammar-registry.sh:195, scripts/publish-grammar-registry.sh:273, scripts/test-publish-grammar-registry.sh:65

- F3 medium scripts/publish-grammar-registry.sh:202: Equal serials are unconditionally accepted as retries, but the serial is a commit timestamp with one-second resolution, not a commit identity. Different commits can have the same timestamp. If B publishes changed grammars and older A shares its timestamp, rerunning A passes this comparison and rolls the registry back. The regression tests retry identical bytes and miss this case.
  fix: For equal serials, require the candidate index to match the published index byte-for-byte, or use authenticated commit identity/order to distinguish retries. Add different-content, equal-serial coverage and correct the documentation's claim that equality identifies the same commit.
  instances: scripts/publish-grammar-registry.sh:202, docs/release/grammar-registry.md:200
