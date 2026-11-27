---
task_uid: "01m877m7af9fz876f80c20zemn"
task: "T441"
rev: 1
type: "feature"
complexity: "m"
criteria:
  - id: "C1"
    text: "`tests::lsp_add_ons::hints_lenses_and_actions_come_from_both_servers` and `tests::lsp_add_ons::folds_and_tokens_come_from_the_provider` pass"
    required: true
    manual: false
  - id: "C2"
    text: "`tests::lsp_add_ons::primary_that_cannot_start_keeps_path_owner_with_served_add_on` and `tests::lsp_add_ons::css_and_scss_aliases_with_different_add_on_sets_keep_owners_match_manager` pass"
    required: true
    manual: false
  - id: "C3"
    text: "`tests::lsp_add_ons::hint_changed_by_add_on_index_shows_at_next_poll` passes"
    required: true
    manual: false
  - id: "C4"
    text: "`tests::lsp_rename_add_ons::only_add_on_accepts_prepare_rename_and_primary_gets_no_rename` and `tests::lsp_rename_add_ons::owner_restarted_before_confirm_renames_nothing_and_toasts` pass"
    required: true
    manual: false
---

# Move the fe-workbench LSP call sites other than colours to multi-server routing, with the rename owner carried through confirmation (LSP add-ons W1, T1645)

# Move the fe-workbench LSP call sites other than colours to multi-server routing, with the rename owner carried through confirmation (LSP add-ons W1, T1645)

Spec: §6.1, §6.2 (each call site; every row except lsp_colors.rs), §5 (polling, `Notified`), §7 (Lead unavailable, Routing, UI), §8 row W1.

Row: fe-workbench call sites except colours: `lsp.rs` (`Document.route`, `Lsp::route`/`lead`, gates), `lsp_inlay_hints.rs`, `lsp_semantic_tokens.rs`, `lsp_links.rs`, `lsp_lightbulb.rs`, `lsp_lenses.rs`, `lsp_folds.rs`, `lsp_symbols.rs`, `lsp_linked_editing.rs`, `hierarchy_panel.rs`, and `lsp_rename.rs`, whose `Request` carries the prepared rename's `owner` through confirmation. `Lsp::route` and `Lsp::lead` select by configured identity. Hint, lens and link slots of shown buffers are re-asked on `Notified` and polled every `repoll_ms`. Do NOT touch lsp_colors.rs (card W2); keep `server_of` as the deprecated alias of `lead_of` until W2.

Test files: crates/fe-workbench/src/tests/lsp_add_ons.rs (two fakes; hints, lenses and actions from both; folds and tokens from the provider; owners kept; a primary that can't start with a served add-on keeps the path's owner; CSS and SCSS aliases with different add-on sets keep `owners_match_manager`; a hint changed by the add-on's index shows at the next poll) and lsp_rename_add_ons.rs (only the add-on accepts `prepareRename`: add-on alone renames; owner restarted between preparation and confirmation: nothing renamed, no other server asked, toast shows).

Read the design sections named above before starting. Do not widen --scope; if a file outside it is needed, stop and say so.

## Implementation notes

- **`Document.route`** replaces `Document.server`: `route_servers(buffer)`
  at open and after each frame, each entry keeping the instance it last
  went to while it has none now (`refresh_route`), so a stopped server's id
  still matches statuses and diagnostics. `Lsp::route` / `Lsp::lead` read
  it; `Lsp::lead` is `route[0]` by configured identity. `Lsp::server` (the
  status item, `force_lsp_status`, `run_server_op`) now goes by the lead's
  kept id; the `status_at(name, root)` fallback for a lead with no id, the
  hover list, restart of every configured server and the log picker are
  W3's (`workbench_commands.rs` is outside this card's scope).
- **Gates.** `owners_match_manager`, the owners pass of the sync,
  `lsp_prepare_rename`, `lsp_rename`, `lsp_code_actioner` and the
  hierarchy panel go by `is_served`. Semantic tokens, folds, document
  symbols and linked editing go by `provider_of(buffer, Feature::…)`.
  `provider_of` is `None` until the servers' capabilities are known, so
  those slots are made once a server has initialized (a frame follows
  its status change), not before.
- **Lightbulb.** `Wanted.servers`: the buffer's servers with a code-action
  provider, in server order.
- **Hints, lenses, links.** Each slot keeps `asked_of`, the
  `served_identities` it asked of. A server leaving (or, for lenses and
  links, a new opening of one) retires what it showed; one joining only
  asks again. `no_provider` is the identity set that answered "none".
  `LspEvent::Notified { server }` re-asks the slots asked of that server
  after 250 ms (`NOTIFIED_REST`), keeping what they show. Polls: the
  workbench keeps the manager's clock (`TestClock` in tests,
  `SystemClock` otherwise), and each frame, plus a 250 ms tick that runs
  only while some server of the manager has a `repoll` (probed as
  `repoll_due(server, now + 24 h).is_some()`), compares
  `repoll_due(server, now)` with the last poll each slot saw; a newer one
  re-asks the slot at once unless its own request is in flight
  (`InFlight`, a count held by the asking task). An answer equal to the
  one shown changes nothing and repaints nothing. fe-lsp has no
  per-server variant of the merged requests, so a poll or `Notified`
  re-asks the merged request (every server of the buffer), not only that
  server's part (§5 "only for that server's part"); a per-server ask in
  fe-lsp would be a follow-up.
- **Rename.** `Request` keeps the prepared `RenameTarget` (with its
  `owner`) and the owner's opening as preparation answered
  (`active_identity`, as `Owner`'s identity is crate-private). Confirming
  calls `rename_prepared`, which sends only to the owner while its
  session has the document open as it did. If the answer is an error and
  the owner's opening has changed, the picker closes with an info toast
  whose text is fe-lsp's `Unavailable` reason ("the language server
  restarted"), and nothing is renamed or asked of another server. The
  design's wording "…: nothing renamed" would need a new key in
  `crates/fe-workbench/l10n/en-US/lsp.ftl`, outside this card's scope.
  The find bar's Rename symbol (`rename_symbol_to`) prepares again and
  renames through that preparation's owner, the same way.
- `lsp_colors.rs` is untouched and `server_of` stays (W2).
