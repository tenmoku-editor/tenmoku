---
task_uid: "01m6w0acb4n79pvg616esw84s9"
task: "T1592"
rev: 5
type: "feature"
complexity: "m"
criteria:
  - id: "C1"
    text: "Tests in cargo nextest run -p fe-chat --test commit_message pass (fake provider): the_request_lists_files_and_cuts_the_diff_to_24_kb_largest_first, denied_and_binary_paths_are_left_out_of_the_text, the_reply_parses_to_subject_and_body_and_drops_fences, a_subject_longer_than_72_is_reported"
    required: true
    manual: false
  - id: "C2"
    text: "Tests in tests::draft_commit_message (cargo nextest run -p fe-workbench) pass: fills_an_empty_box_as_one_undoable_edit, asks_before_replacing_a_typed_message, disabled_with_a_reason_when_nothing_is_staged_or_chat_is_unavailable"
    required: true
    manual: false
  - id: "C3"
    text: "cargo nextest run -p fe-workbench --test l10n passes (ui_text_is_in_the_catalogue, keys_used_here_exist) with this card's files in l10n-allow.toml's migrated list"
    required: true
    manual: false
---

# Draft from diff: a commit message from the staged changes through the chat provider

# Draft commit message from the staged diff

Spec: `docs/design/design-system.md#changes-and-commit` ("The message box has a
72-character counter and Draft from diff"). A chat backend exists
(`crates/fe-chat`, `fe-chat-anthropic`); inline edit shows how a one-shot
provider request is driven (`crates/fe-chat/src/inline.rs`).

## Change

- `crates/fe-chat/src/commit_message.rs` (new, headless): builds the request
  from the staged diff: file list with status, then the unified diff, cut to
  24 KB (largest files trimmed first, "N more files" noted), binary and
  denied paths (the chat's secret deny list, `chat_context`'s refusal rules)
  left out of the text; a system prompt asking for a subject of at most 72
  characters, a blank line and a short body; a parser that returns subject and
  body from the reply and drops fences. Uses the same provider trait as
  inline edit; tested with the fake provider.
- `crates/fe-workbench/src/git_commit.rs`: a **Draft from diff** button in the
  commit box (beside Amend) and `git.draft_commit_message`: runs the request in
  the background, fills the empty box or asks before replacing a typed message,
  and is one undoable edit. Disabled with a tooltip reason when nothing is
  staged, chat has no key, or the folder is untrusted.

## Not in this card

Streaming into the box, conventional-commit formats, per-hunk summaries.

Platform: **Linux-OK**.

## i18n

Every user-visible string this card adds goes through the catalogue
(`docs/decisions/xte5e7hqsw-i18n-design.md` §§ 4–5): `tr!` / `text!`, whole
messages, plural selectors for counts, keys `workbench-changes-…` in
`crates/fe-workbench/l10n/en-US/changes.ftl`; a new file is added to `resources!` in
`src/l10n.rs` and to `l10n-allow.toml`'s `migrated` list. Tooltips and
accessible names count; a new command's title is the key `command-<id>` in the
same file. Tests build expected text through the catalogue.

## Implementation notes

- Scope grew by self-respec: `fe-chat/src/lib.rs` (module and exports),
  `fe-chat/l10n-allow.toml` (the system prompt and request framing are
  model-facing, as for `inline.rs`), `git_panel.rs` (the button and the
  drafting state live in the commit box), `workbench_commands.rs` (the
  typed command), `chat_panel.rs` (a non-test `has_key` for the button's
  state) and `tests/mod.rs`. fe-workbench's allow list is `all = true`, so
  C3 needed no list entry.
- Each staged file's patch comes from git itself, `Repo::staged_patch` (new
  in fe-git: `git diff --cached` with external diff, textconv, colour and
  renames off, `a/`/`b/` prefixes forced), so mode changes, symlinks and
  `\ No newline at end of file` reach the model as git writes them (review
  r1 F3, F4; fe-git `commit.rs` and `tests/staged_patch.rs` respecced in).
  The pathspecs are `:(top,literal)<path>` and
  `:(top,literal,exclude)<path>/` (with `--no-literal-pathspecs` after
  fe-git's own `--literal-pathspecs`, so the magic is read), so a file that
  became a directory, or a directory that became a file, brings in none of
  the files under it; a patch with more than one `diff --git` header is
  treated as denied all the same (review r2 F6). Binary is judged from the
  HEAD and index blobs' bytes first (a NUL in the first 8000, via
  `head_text`/`index_text`), so a binary file's mode change alone or a
  `.gitattributes` `diff` forcing a textual patch still leaves it out
  (r2 F7); git's "Binary files … differ" with no hunks still counts too. Blobs over the
  chat's 1 MiB (`chat_context::MAX_FILE`) are `TooLarge` before git runs;
  denial is `chat_context::refusal` at the workbench root, both paths
  canonicalized, so a file outside a subfolder root is left out.
- "Left out of the text" is read literally: binary, too large and denied
  files are not named, only counted ("N more files not shown").
- Cutting: an even share of 24 KB, the smaller diffs whole; when that share
  would be under 512 bytes, the largest files' diffs are left out whole and
  counted, 64 bytes kept for that note so the whole diff stays within 24 KB
  (r1 F5). Each cut diff ends at a line with "[N more lines cut]".
- The reply is read to its end (no streaming, per the card), at effort low.
  A typed message is replaced only after a prompt that shows the draft; the
  fill is `set_box_text`, one undo step. The read, the request and the
  question are one task the box holds, referring to the panel weakly;
  untrusting the folder (`TrustChanged`), changing the root (`RootChanged`)
  or starting a commit drops it, and trust, key and panel are checked again
  after the read, before sending (r1 F1, F2). Drafting is disabled while a
  commit runs. The command shows the panel and focuses the box, as
  `git.commit` does; it checks the index itself in the background, so a
  panel that hasn't read its status yet doesn't refuse.
- Not handled: Amend checked still drafts from the index's diff against
  HEAD, not against HEAD's parent.
