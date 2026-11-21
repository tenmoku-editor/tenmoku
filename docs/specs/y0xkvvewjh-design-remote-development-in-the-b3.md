---
task_uid: "01m7zn92g5esfxmry0xkvvewjh"
task: "T433"
rev: 1
type: "docs"
complexity: "m"
criteria:
  - id: "C1"
    text: "docs/design/design-system.md gains a Remote development section under Panels and screens; 'Welcome and empty window' stays in Still to design only if untouched; the header's revision number goes up by one"
    required: true
    manual: false
  - id: "C2"
    text: "The section settles the remote pill: a 20 px pill left of project in the title bar with all 7 states of design section 4 (glyph, tone, progress underline, tooltip, click menu, accessible name), the pill with a forward count, and its place at both rail sides"
    required: true
    manual: false
  - id: "C3"
    text: "It settles the Open Remote sheet at 520 px: Host (completion list rows, 'Not an ssh host'), Connecting (all 7 states of design 2.2), Folder, the 9 error causes of 2.4 with their actions, and the sign-in modal's 3 kinds titled with the host"
    required: true
    manual: false
  - id: "C4"
    text: "It settles the pending states: loading read-only tab, saving ring, save pending, 'Not loaded - reconnecting', tree row spinner and pending row, Quick Open Indexing... footer, terminal Reconnecting... overlay, the operation-in-progress use"
    required: true
    manual: false
  - id: "C5"
    text: "It settles the Ports tab (columns, dimmed detected rows, row actions, Forward a Port field, empty state; rail/bottom-panel place), the five toasts, the close prompt with pending saves, and Recents and Welcome remote rows with the host glyph"
    required: true
    manual: false
  - id: "C6"
    text: "Workbench anatomy's Title bar and Bottom panel rows name the remote parts, and the Panels and screens table lists each remote artboard to draw on the canvas, in Yohen and Hakuji"
    required: true
    manual: false
  - id: "C7"
    text: "Each new colour role or reuse (pill tones, amber/red states) has Yohen and Hakuji values or names the existing token; the decision file docs/decisions/\u003clane\u003e-remote-design.md includes the contrast and OKLab-distance check script and its output, with the doc's floors (6:1 Yohen, 4.5:1 Hakuji)"
    required: true
    manual: false
  - id: "C8"
    text: "For every control the section gives its accessible name and announcement text, and every size as an existing kiln-ui roles.rs constant or a new role to add"
    required: true
    manual: false
---

# Design remote development in the B3 design system: the remote pill and its seven states, the Open Remote connect sheet, remote pending states, the Ports tab, toasts and the remote colour roles (remote D0)

# Design remote development in the B3 design system (remote D0)

The remote UI cards (T2055 pill, T2056 connect sheet, T2057 pending states, T2067 Ports tab) say the
design system has no drawing for their surfaces (design § 10, "For later design cards"; T2056 even
says "the lead may prefer a design card first"). This card is that card. It follows the debugger's
D0 (Mac T1839, commit 52fa2bb5b): it writes the design system's entries and the list of artboards
to draw, and settles the calls and colours the UI cards then build to. It writes no code.

## Read first
- `docs/designs/remote-development.md` (design file; § 10 lists what needs a drawing) and the
  decision `docs/decisions/j8wvgszah0-remote-development.md` (§ 1, § 4, § 11 for texts and states).
- `docs/design/design-system.md`: Workbench anatomy, "Panels and screens" (every example is an
  artboard on the canvas, listed in the table), "Still to design".
- The D0 pattern: `docs/decisions/skgc7fe5z9-debugger-design.md` (calls, parting from the record,
  the colour check script and its output).

## Rules
Edit only the files in scope. The canvas itself is not in the repo: as D0 did, the doc lists each
artboard (name, what it shows, both themes) in the Panels and screens table; do not invent a canvas
file. Reuse existing roles and kiln-ui `roles.rs` sizes where they fit; new roles get Yohen and
Hakuji values. Decision notes go in `docs/decisions/<lane>-remote-design.md`.

## Surfaces to settle (design § 10)
Remote pill (7 states, menu, place in the title bar at both rail sides); connect sheet (Host,
Connecting 7 states, Folder, Error 9 causes, sign-in modal 3 kinds); tab states (loading read-only
tab, saving ring, save pending, "Not loaded - reconnecting"); tree pending rows and spinners; Quick
Open Indexing... footer; terminal overlay "Reconnecting..."; Ports tab (rows, dimmed detected,
Forward a Port field, empty); toasts (lost, reconnected, disconnected, port detected, AI unavailable);
close prompt with pending saves; Recents and Welcome remote rows with the host glyph.
