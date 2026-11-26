---
task_uid: "01m8cdp071t0kenw6keracstv5"
task: "T2115"
rev: 1
type: "chore"
complexity: "m"
criteria: []
---

# Follow-ups from review of T1560

Findings of the review of T1560 that don't block the merge (below merge.block_severity):

- F1 medium crates/fe-workbench/src/user_snippets.rs:100: Several persistent surfaces cache resolved translations without locale invalidation, contrary to design §4. User-snippet completion details remain in the previous language until snippet files change; an open Extensions picker retains translated contribution counts, server descriptions and plugin details because it observes only extensions, consents and plugins; plugin-log tab titles remain in their original language. Refreshing windows does not recompute these stored strings.
  fix: Resolve persistent labels when displayed, or invalidate/rebuild them on ActiveLocale changes or catalogue-generation changes. Cover snippet completions, extension/plugin rows and existing log tabs with a locale-switch regression test that leaves their underlying data unchanged.
  instances: crates/fe-workbench/src/user_snippets.rs:100, crates/fe-workbench/src/extensions_picker.rs:187, crates/fe-workbench/src/extensions_picker.rs:192, crates/fe-workbench/src/extensions_picker.rs:210, crates/fe-workbench/src/extensions_picker.rs:215, crates/fe-workbench/src/extensions_picker.rs:409, crates/fe-workbench/src/extensions_picker.rs:412, crates/fe-workbench/src/extensions_picker.rs:415, crates/fe-workbench/src/extensions_picker.rs:418, crates/fe-workbench/src/extensions_picker.rs:421, crates/fe-workbench/src/extensions_picker.rs:424, crates/fe-workbench/src/extensions_picker.rs:432, crates/fe-workbench/src/plugin_manage.rs:101, crates/fe-workbench/src/plugin_manage.rs:195, crates/fe-workbench/src/plugin_manage.rs:209, crates/fe-workbench/src/plugin_log.rs:217

## Implementation notes

- A user snippet's completion detail is resolved as its item is made; its
  cached offer keeps none.
- The open Extensions picker also observes `ActiveLocale` and reads its rows
  again, which covers the server lines, contribution counts and `PluginRow`'s
  capability lines and reasons. `set_locale` switches the catalogue before it
  sets that global, so the observer reads the new language.
- A plugin log observes `ActiveLocale` for as long as it follows its log, and
  retitles each tab showing the buffer, splits included.
- Not covered, as the finding doesn't list them: a stopped plugin's diagnostic
  and a skipped extension's reason are shown as they were stored, and Insert
  Snippet's rows keep the language they were built in while it's open.
- The regression test is inline in `extensions_picker.rs`, which keeps it in
  the task's scope.
