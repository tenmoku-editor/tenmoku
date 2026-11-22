---
task_uid: "01m71szfyhv374zq3qf3nhy278"
task: "T1706"
rev: 3
type: "feature"
complexity: "s"
criteria:
  - id: "C1"
    text: "Manifest settings contributions with the schema subset of section 4.4, scopes, and secret keys; `settings.get` takes an optional root. Known limit: problems in a settings layer are reported as unpositioned warnings in the plugin's log, not as positioned diagnostics through `Layer::check_schema` and the fe-config problem pipeline (follow-up below)."
    required: true
    manual: false
  - id: "C2"
    text: "New tests in kiln/crates/kiln-plugin/tests/settings.rs: `settings_defaults_then_user_then_trusted_project`, `settings_untrusted_project_dropped`, `settings_two_open_roots_each_get_own_project_keys`, `settings_root_not_open_refused`, `settings_secret_key_in_file_is_warned_and_ignored`, `settings_value_type_mismatch_dropped_with_diagnostic`."
    required: true
    manual: false
  - id: "C3"
    text: "Root context: `settings.get` with a root resolves project values for that open root only, never the nearest or first root."
    required: true
    manual: false
  - id: "C4"
    text: "Values returned to a plugin are size-capped by the host."
    required: true
    manual: false
---

# Settings contributions: schema subset, scopes, secret keys, settings.get with root (A5, 4.4)

# Settings contributions: schema subset, scopes, secret keys, settings.get with root (A5, 4.4)

Draft id A5 of the plugins stage 3 plan (docs/decisions/dqsjfernkt-plugins-stage-3.md, section 6). The design is approved by the user (2026-11-09); questions Q1 to Q7 follow the design's proposals.

Section 4.4.

Read the design sections named above before starting. Do not widen --scope; if a file outside it is needed, stop and say so.

## Implementation notes

- Project trust in the app: `PluginHost` keeps `open_roots` (carried on `LoadContext`, no static): every window's root and
  its trust, refreshed by `workspace_changed` (already run on trust, root and window-close events).
  `FeSettings::project` answers from it, so an open untrusted root gets its project keys dropped
  with the "trusted folders only" diagnostic, and a root no window has open is `invalid-argument`.
- Known limit: diagnostics are checked by `SettingsSchema::resolve`, not `Layer::check_schema`,
  and reach the plugin's log as warnings without file positions.
- Respec (C1, F4 of review rvde6e): positioned diagnostics for plugin settings are a known
  limit of this card. `SettingsSchema::resolve` validates each layer itself and its diagnostics
  reach the plugin's log as warnings, without file positions, and again at each `settings.get`.
  Follow-up to file: route contributed plugin schemas through `Layer::check_schema` into the
  config-problem pipeline. Files it needs: `crates/fe-config/src/settings.rs` (a layer filter
  that checks `[plugins."<id>"]` against the installed plugins' schemas),
  `crates/fe-workbench/src/config_problems.rs` (show the positioned problems),
  `crates/fe-workbench/src/plugin_host.rs` (hand the schemas to fe-config), and
  `kiln/crates/kiln-core/src/settings.rs` if the schema lookup must be per-table.
- Review rvde6e F1 (guest root paths), F2 (job worker stores), F3 (schema subset) and F5
  (non-finite numbers) are fixed, each with a test. The F2 test covers `settings_state`, the one
  function both the interactive store and `worker_data` call, not a guest job calling
  `settings.get`: a `Launch` can't be built outside kiln-plugin.
