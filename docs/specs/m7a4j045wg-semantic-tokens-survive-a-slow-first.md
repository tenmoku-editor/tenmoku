---
task_uid: "01m84vnkh2bkg0hym7a4j045wg"
task: "T414"
rev: 2
type: "bugfix"
complexity: "s"
criteria:
  - id: "C1"
    text: "C1 crates/fe-workbench/src/lsp_semantic_tokens.rs semantic_tokens_landed: the fixed RETRIES (8) cap is replaced by a backoff (250 ms doubling, capped at 5 s) that keeps asking while the slot, buffer version and server are unchanged; asks that keep timing out log once per buffer at INFO (a slow server, naming it), and non-Retry errors log at INFO, not debug. Regression test (fails at base, base APIs only) in new crates/fe-workbench/src/tests/lsp_semantic_slow_first_answer.rs, declared with #[path] from src/tests/lsp_semantic_tokens.rs as lsp_semantic_timeout.rs is: the fake server answers semanticTokens/full 10 s late for the first 12 asks, then at once with a mutable token; with no edit and no refresh the underline lands (at base the 10th ask is never sent)."
    required: true
    manual: false
  - id: "C2"
    text: "C2 crates/fe-lsp/src/semantic_tokens.rs: a null or empty full answer while Reply.empty_is_final is false (the server not yet quiescent) is Err(Retry), not final empty tokens; doc comment and the Failures module doc say so. Test in crates/fe-lsp/src/tests/encoding/semantic_tokens.rs: a null reply before the fake serverStatus says quiescent is Err(Retry); the same reply after quiescent stays Ok(Some(empty))."
    required: true
    manual: false
  - id: "C3"
    text: "C3 existing lsp_semantic* tests in fe-workbench and fe-lsp semantic_tokens tests pass; clippy clean for fe-lsp and fe-workbench."
    required: true
    manual: false
---

# Semantic tokens survive a slow first analysis: asks back off instead of stopping after 8 retries, and rust-analyzer's null while loading is not final (no mut underlines or format colours in restored tabs) (Mac T2108)

User report 2026-11-23: in Tenmoku 0.2.5 on this repo, `mut` is not underlined and format placeholders in log strings are not coloured; "it was working before". Cause (lead investigation): semantic tokens are asked at most 9 times (RETRIES 8, since 071bf472c on 10-21) with a 3 s timeout (lsp.timeouts.hover), ~29 s, then never again until an edit or workspace/semanticTokens/refresh. Since last-quit restore (T1666), tabs are restored at launch, so every ask lands while rust-analyzer primes caches for the whole workspace (1731 crates); RA sends refresh on quiescence, before priming ends, and none after, so read-only restored tabs never get tokens. Nothing logs at INFO, so tenmoku.log shows no trace. Second gap: rust-analyzer's null while its files load is taken as final empty tokens (unwrap_or_default; Reply.empty_is_final ignored). Settings ruled out (editor.semantic_highlighting defaults true; no rust-analyzer semanticHighlighting options sent). New tests in new files; do not widen --scope.
