---
task_uid: "01m71sze1zaaarqstn63y2234t"
task: "T1705"
rev: 3
type: "feature"
complexity: "s"
criteria:
  - id: "C1"
    text: "`panels.wit` declares the declarative view tree and events; `[[contributes.plugin.panels]]` parses (id, title key, icon, location)."
    required: true
    manual: false
  - id: "C2"
    text: "A headless store validates the tree against limits (node count, depth, label length, list size, update rate) BEFORE accepting an update; an oversize update is rejected whole and the previous tree stays."
    required: true
    manual: false
  - id: "C3"
    text: "New tests in crates/fe-plugin/tests/panels.rs: `panels_valid_tree_is_stored`, `panels_oversize_tree_rejected_keeps_previous`, `panels_depth_limit`, `panels_label_length_limit`, `panels_update_rate_is_capped`, `panels_requires_panels_capability`."
    required: true
    manual: false
  - id: "C4"
    text: "No GPUI code in this card; it is Linux-testable."
    required: true
    manual: false
---

# Panels model: panels.wit, [[contributes.plugin.panels]] and a headless validating store (A3, 4.2)

# Panels model: panels.wit, [[contributes.plugin.panels]] and a headless validating store (A3, 4.2)

Draft id A3 of the plugins stage 3 plan (docs/decisions/dqsjfernkt-plugins-stage-3.md, section 6). The design is approved by the user (2026-11-09); questions Q1 to Q7 follow the design's proposals.

Section 4.2. Caps on host queues: events from the host to the plugin are queued with a cap.

Read the design sections named above before starting. Do not widen --scope; if a file outside it is needed, stop and say so.
