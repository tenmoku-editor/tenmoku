# Tenmoku design system

Written 2026-11-07 by Daryl Robbins (revision 20, direction B3). This is the
**should** for Tenmoku's look and feel; `docs/design-language.md` records what
the code does today, and the cards that implement this file bring the two
together.

## Principles

Tenmoku looks like one product, not a host for plugins: every pane, panel and
popup uses the same few parts, colours and keys. It sits in the JetBrains New
UI family (a real IDE, a rail, docked tool windows) and parts ways with it
where agents, Vim and review come in.

- **One meaning per colour.** Each hue carries exactly one job. The rust accent
  is you; silver-blue is agents; green, red, ochre and blue are added, removed,
  warning and modified. Nothing is decoration.
- **Agents are authors, not a feature.** Their lines, commits, terminals, tests
  and changes carry their mark everywhere, and their work reaches your tree
  only through review.
- **Keyboard first, mouse welcome.** Every pane action has a key, and menus
  show it, so the mouse teaches the keyboard. Vim's own ⌃w window commands are
  used where they exist; the ␣ leader fills the gaps.
- **One shape per job.** Builds, tests, scripts and agent tasks are all Runs.
  Every pane header ends the same way. Every setting row looks the same.
- **Calm by default.** Hints, guides and fold chevrons stay quiet until they
  are needed; notifications separate what needs you from what is only news.
- **The file is the truth.** Settings show their key in settings.toml; the
  diff's right side is the live, editable working copy.

All examples below come from the canvas Tenmoku visual directions, row B3.

## Colour

Two themes share one set of roles: **Yohen** (dark, warm iron-brown, the
signature; named for the kiln-transformed tenmoku bowls) and **Hakuji** (light,
white porcelain and cool greys, rust kept as the one brand thread). The closest
pair of roles in Yohen is 5.4 apart in OKLab ΔE, up from 1.2 before the retune;
every text colour clears 4.5:1 on every surface it sits on, and code text
clears 6:1 in the dark theme. The rust accent keeps the name hare's fur, after
the glaze it comes from.

T1512 (rev 2) moved five values by OKLab lightness only, keeping hue and
chroma, so the rules above hold: Yohen faint `#978777` → `#a09080` and comment
`#839c86` → `#88a18b` (4.5:1 on selected rows, 6:1 on the cursor line), Hakuji
comment `#6a737d` → `#69727c` (4.5:1 on the cursor line), and Hakuji shinsha
`#c0362c` → `#a1100e` and oil spot `#3f6587` → `#3a5f81` (0.05 OKLab L from
celadon, kiseto and hare's fur; kiseto can't darken in the sRGB gamut, so
shinsha moved for both of its pairs).

Yohen's comment moved from `#88a18b` to the neutral grey `#9e9b97`: chroma dropped
and lightness stayed at the 6:1 floor on the cursor line.

### Signals

| Role | Yohen | Hakuji | Means | Never |
| --- | --- | --- | --- | --- |
| Hare's fur (accent) | `#e0874c` | `#a84a16` | You: caret, focus ring, NORMAL mode, active tab, selected row, primary button | Decoration, a second accent |
| Oil spot | `#b4c2cc` | `#3a5f81` | Agents: their lines, chips, terminals, toasts, review | Anything a person did |
| Celadon | `#6fd49a` | `#17773f` | Added, passed, INSERT mode | Modified |
| Shinsha | `#f2716f` | `#a1100e` | Removed, failed, errors | Warnings |
| Kiseto | `#e8b440` | `#776a00` | Warnings, flaky tests | Modified |
| Ruri | `#4f95f5` | `#1f68cf` | Modified files and lines | Agent work |
| Jun | `#c7a5e6` | `#7b36a8` | VISUAL mode (shares the keyword hue) | Status |

Each signal has a soft ground for rows and badges (for example celadon-soft
`#18291f` / `#e3f3e8` behind added lines). A colour-blind mode swaps green and
red for blue and orange as its own setting, rather than weakening the default
for everyone.

### Syntax

| Token | Yohen | Hakuji | Glaze | Style |
| --- | --- | --- | --- | --- |
| Keyword | `#c7a5e6` | `#7b36a8` | Jun splash | |
| Function | `#f2c77a` | `#925a00` | Ame | Bold at its definition |
| Type | `#7fc9be` | `#0d6e66` | Seiji | |
| String | `#b5cf86` | `#2f6f1f` | Oribe | |
| Number, constant | `#f0a3c2` | `#a32f6b` | Peach bloom | |
| Macro, attribute | `#8db1f2` | `#2457c5` | Lapis | |
| Property | `#8db1f2` | `#2457c5` | Lapis | |
| Key | `#8db1f2` | `#2457c5` | Lapis | |
| Variable | `#ede4d6` | `#1f2328` | ink | |
| Punctuation | `#a89a8c` | `#596069` | Iron | |
| Comment | `#9e9b97` | `#69727c` | Wood ash | Italic |

A name that labels a value is Property: member access, field declarations,
named arguments and the keys written in object, struct and table literals.
Key (`string.special.key`) is only for the keys of data files. In Yohen and
Hakuji both are Lapis.

Brackets pair by depth in function, keyword and type colours. Mutable bindings
get a quiet solid underline in their own colour; warnings and errors use wavy
underlines in ochre and red.

### Surfaces

| Surface | Yohen | Hakuji | Used for |
| --- | --- | --- | --- |
| Chrome | `#120e0b` | `#f2f2f4` | Title bar, rail, tab strip, status bar |
| Tool window | `#17120f` | `#f7f7f9` | Sidebars, bottom panels |
| Editor | `#1c1613` | `#ffffff` | Code, terminal, diff panes |
| Cursor line | `#231c18` | `#f6f7f9` | Current line band |
| Hover | `#2b221d` | `#ebecef` | Rows and ghost buttons |
| Selected | `#2f2621` | `#e8eaee` | Selected row without focus, pressed segment |
| Focused selection | `#3e2717` | `#fbe7da` | Selected row with focus (rust tint + 2px rust edge) |
| Overlay | `#2a211c` | `#ffffff` + shadow | Popups, hover cards, menus, toasts |
| Line | `#2e251e` | `#e1e3e7` | Hairlines between regions |
| Control border | `#4a3a2e` | `#c6cad0` | Inputs, secondary buttons |

Text is three steps on every surface: ink `#ede4d6` / `#1f2328`, muted
`#b8a99a` / `#4b525b`, faint `#a09080` / `#5f6670`.

## Type, spacing and sizes

Three families: **Instrument Sans** for the interface, **Spline Sans Mono** for
code and anything you could type, **Instrument Serif** only for page titles
(Settings, welcome). Interface text is sentence case, never all caps.

| Style | Family | Size / line | Weight | Used for |
| --- | --- | --- | --- | --- |
| Page title | Serif | 30 / 36 px | 400 | Settings, welcome, empty window |
| Section heading | Sans | 15 / 20 px | 600 | Settings sections |
| UI heading | Sans | 13 / 18 px | 600 | Panel and dialog headings |
| UI body | Sans | 13 / 18 px | 400 | Tree rows, tabs, menus, settings help |
| UI label | Sans | 12 / 16 px | 500 | Buttons, badges, status bar |
| UI small | Sans | 11 / 14 px | 500 | Timestamps, counts, key hints |
| Code | Mono | 13 / 20 px | 400 | Editor; line height fixed so splits and diffs align |
| Code small | Mono | 12 / 18–20 px | 400 | Terminal, diff panes, paths, setting keys |

| Measure | Value | Notes |
| --- | --- | --- |
| Title bar | 38 px | Project and branch side by side, no slash |
| Tab strip | 34 px | Panel tab strips match |
| Row | 24 px | Trees, lists, menus (28 px in menus and settings index) |
| Status bar | 24 px | |
| Rail | 42 px wide | 32 px tall items |
| Sidebar | 232–248 px | Resizable |
| Bottom panel | 250 px | Resizable |
| Controls | 26 px, 22 px small | |
| Spacing | 2, 4, 6, 8, 12, 16, 24, 40 px | 2 px base for dense chrome, 4 px grid for layout |
| Radius | 2 / 4 / 6 / 10 px | Find fills; buttons, tabs, rows; menus, cards; palette, toasts |

Only floating things cast a shadow (overlay: 0 12px 32px at 55% black in dark,
14% in light, plus a 1 px hairline). Panels are separated by hairlines, never
shadows.

## Workbench anatomy

The window is a title bar, a rail, a sidebar, the editor and one bottom panel,
framed by a status bar. Each region has one job and one place for its controls.

| Region | What it holds | Rules |
| --- | --- | --- |
| Title bar | Back/forward, the remote pill (remote windows: host and connection state, see Remote development), project ▾, branch ▾ (side by side, no slash), agent chip, the run widget (a task or debug configuration ▾, run or stop, debug; while debugging, the session's controls: continue or pause, the three steps, restart, stop), search, settings gear | The agent chip is the global "agents working" indicator; without a title bar the session's controls float (see Debugger) and the remote pill is the status bar's first item. The pill doesn't move with the rail |
| Rail | Top: Project, Search, Changes, Agents. Bottom: Problems, Runs, Debug, Git log, Terminal, Ports | Left or right (`workbench.rail`); panels open on the rail's side. Debug joins once the root has a debug configuration or a session starts. A remote window has Ports and no Agents or Chat |
| Rail selection | A tinted tab behind the icon that runs to the rail's panel-facing edge, with a 2 px rust bar on that edge | Flips with the rail side |
| Rail badges | Bottom-right corner, 2 px cut-out ring; colour = meaning (blue changes, silver-blue agents, red dot errors) | Never on top of the glyph |
| Sidebar | Project tree with Structure collapsed at its foot; or Changes; or Review | Structure, collapsed, still shows the symbol under the caret |
| Editor tab strip | Tabs (rust top line on the active tab, unsaved dot, pinned tab as an icon), then tab list ▾, split right, ⋯ | |
| Breadcrumbs | `fe-display › src › minimap.rs › MinimapTile › render` | Symbols in their syntax colours; clicking opens the outline |
| Bottom panel | Problems, Runs, Debug (Variables, Breakpoints, Console), Terminal, Git log, Ports (remote windows: forwarded and detected ports) | Own header, own tabs where it has them. A remote terminal shows the Reconnecting… overlay while the link is down |
| Status bar | Left: the remote pill (remote windows without a title bar), Vim mode pill, the operation in progress (spinner, label, bar, count, +N more), error and warning counts. Right: language server, position, indent, encoding, language, notification bell | No branch here: it is in the title bar |

**Pane controls**, the same on every pane. Right-aligned in this order: the
pane's own actions, then ⋯ (pane actions), then — (hide). The ⋯ menu offers
split right ⌃w v, split down ⌃w s, maximize ⌃w z, move to new window, move to
the other split ⌃w x, pin tab, close other tabs ␣b o, reopen closed tab ␣b u.

**Icons.** 16 px line icons on a 24 grid, 1.7–1.8 stroke, in the text colour of
their state. Each language has its own glyph (Lucide's, with Tabler's marks
where Lucide has none), tinted by family: file-type icons sit in the palette
(Rust in muted kaki, the compiled languages jun, TOML grey, Markdown lapis); folders tint by role (source, tests, docs) and go neutral grey
in Hakuji. Agents use a small robot head; Runs a play triangle with list lines;
Changes a page with +/−.

## Editor

The editor carries most of the information density, so its layers are ranked:
code first, then problems and changes, then language-server annotations, then
guides that only appear on demand.

**Gutter**, left to right (81 px at 13 px code). Line numbers 38 px
right-aligned; problem dot 10 px (ochre warning, red error); fold column 16 px,
always reserved so code never shifts; change bar 3 px (blue modified, green
added, a red notch for removed, a dotted silver-blue lane for agent-written
lines); 10 px to the code.

**Folding.** Chevrons appear on every block opener only while the pointer is in
the gutter. The hovered one is drawn at full strength on a hover tile, its
closing line shows a small up-chevron, and its indent guide brightens. No
traced range lines. Collapsed blocks (to design) show a `{ … 9 lines }` pill
that takes the colour of any error or agent edit inside.

**Indent guides.** 1 px at every 4 columns inside leading whitespace only,
`#2c241e` / `#eceef1`. The guide of the block holding the caret, or of a
hovered fold, is one step brighter.

**Language-server formatting.**

- Inlay type hints as small grey pills: `budget: Budget`, `row: usize`.
- Closing-brace hints in faint italic after a block's last brace: `} fn render`.
- Mutable bindings underlined solid, thin, in their own colour.
- Warnings and errors: wavy underline, gutter dot, and an inline lens at the
  end of the line (ochre at 70%, red at full).
- Code lenses above tests in sans: `▷ Run test · Debug · passed in the last 8 runs`.
- Identifier highlight: the symbol under the caret and its other uses get an
  ochre-soft fill.

**Hover card** (K, or a resting pointer). Module path in faint mono, the
signature syntax-coloured with the name bold, prose docs, then a footer:
definition location, caller count, `gd definition · K pin`. The hovered token
gets a soft fill.

**Find and replace.** A row under the breadcrumbs, not a floating box. The find
field has a rust focus ring; match case, whole word and regex toggle inside it
(on = rust-soft tint, not a solid fill); "2 of 3", previous, next, in
selection. Replace adds Replace, Replace all, and Rename symbol instead when the
match is a symbol. The equivalent Vim command shows faintly on the right
(`/dirty`, `:%s/dirty/needs_paint/gI`): find and Vim search are one feature.
Current match uses find-match `#5a4314`; others ochre-soft.

## Panels and screens

Every example lives on the canvas as an artboard, most in both themes. Each
artboard is one component with switches (panel, sidebar, find, hover, rail
side, Vim mode), so any combination can be shown.

| Example | Artboard on the canvas | Also shows |
| --- | --- | --- |
| Problems | B3 · Problems panel, find and replace, agent toast | Find and replace, fold hover, inlay hints, toast |
| Problems in Hakuji | B3 · Hakuji (light): the same screen | The light theme |
| Runs: tests | B3 · Runs: a test run in progress | Structure expanded under Project |
| Runs: build | B3 · Runs: build timeline, notification inbox open | Inbox, INSERT mode |
| Terminal | B3 · Terminal as tabs, command blocks, LSP hover | Hover card |
| Changes and commit | B3 · Changes sidebar | Pane ⋯ menu open |
| Review diff | B3 · Reviewing an agent's change, and the same diff in Hakuji | Agent comment |
| Settings | B3 · Settings, and Settings after picking Hakuji | Rail on the right |
| Palette | Yohen, retuned | B2 and B3 colours side by side |
| Debugger: gutter | B3 · Debugger gutter, in Yohen and Hakuji | Every breakpoint state, the later conditional and logpoint glyphs, both frame arrows and bands, a breakpoint beside a problem dot, the hover tile |
| Debugger: run widget | B3 · Run widget with a session, in Yohen and Hakuji | The picker with tasks and debug configurations; the stopped session's controls; the floating toolbar without a title bar |
| Debugger: stopped | B3 · Debug tool window while stopped, in Yohen and Hakuji | Call Stack, Watch and Variables; the current line in the editor; the session's row in Runs and in the status bar |
| Debugger: breakpoints | B3 · Debug tool window: Breakpoints, in Yohen and Hakuji | Exception filters, a disabled and an unverified breakpoint, the empty state |
| Debugger: console | B3 · Debug Console, in Yohen and Hakuji | Mixed output, an expanded result, an error |
| Debugger: hover value | B3 · Hover value, in Yohen and Hakuji | A value with children above the language server's hover |
| Remote: pill | B3 · Remote pill in the title bar, in Yohen and Hakuji | All seven states, each at rest and hovered; the forward count; the install underline at 60%; each state's menu; the title bar with the rail on the left and on the right; the pill as the status bar's first item without a title bar |
| Remote: host | B3 · Open Remote: Host, in Yohen and Hakuji | The completion list with recent, ssh config and typed rows; "Not an ssh host"; Connect off on an empty field |
| Remote: connecting | B3 · Open Remote: Connecting, in Yohen and Hakuji | The seven status lines, the bar at Downloading and Uploading, the Reinstalling… line, the first connect's footnote |
| Remote: folder | B3 · Open Remote: Folder, in Yohen and Hakuji | The path field completing, the row spinner, folders over dimmed files, hidden entries after a `.` |
| Remote: errors | B3 · Open Remote: errors, in Yohen and Hakuji | The nine causes with their actions, the changed host key's `shield-alert`, the second refused handshake |
| Remote: sign-in | B3 · Sign-in prompt, in Yohen and Hakuji | A secret, an unknown host with its fingerprint selected, a touch notice; a git credential on the host |
| Remote: pending | B3 · Remote pending states, in Yohen and Hakuji | Tabs loading, saving, save pending and not loaded, with the loading and not-loaded editor areas; tree rows expanding, renaming, moving, deleting and creating; Quick Open's Indexing… footer; the status bar's remote operations |
| Remote: terminal | B3 · Terminal reconnecting, in Yohen and Hakuji | The scrim and Reconnecting… card; the lost-output line after resume |
| Remote: ports | B3 · Ports tab, in Yohen and Hakuji | Forwarded, detected (dimmed) and starting rows; "5173 was in use"; row actions on hover; the Forward a Port input with its error; the empty state; the rail item |
| Remote: toasts | B3 · Remote toasts, in Yohen and Hakuji | Lost, reconnected, disconnected, port detected and AI unavailable |
| Remote: close prompt | B3 · Closing with pending saves, in Yohen and Hakuji | The prompt over a window with two save-pending tabs |
| Remote: recents | B3 · Open Recent and Welcome with remote folders, in Yohen and Hakuji | Local and remote rows, the same folder on two hosts; Welcome's Open Remote… beside Open Folder |

### Problems

Grouped by file, with a scope switch (This file, Project, From agents) and
severity filters that double as counts. The selected problem opens a detail
pane: code and source, a three-line snippet with the suggested fix as a green
line, then Apply fix ⌥↵, Ask an agent and Explain. Files an agent wrote carry a
"written by lane 2" chip.

### Runs: builds, tests and agent tasks in one list

Everything that runs has the same shape: a status icon, a mono title, a
one-line summary, and an age. A running item shows a thin rust progress line.
The detail depends on the kind:

- **Tests:** a split bar (green passed, red failed, track remaining), a test
  tree, and each test's last 8 results as small bars, so flaky tests stand out.
  A failure shows the assertion with left in red and right in green, a "failed
  3 of its last 8 runs, likely flaky" note, and Debug this test or Ask an agent.
- **Build:** a crate waterfall instead of a wall of log: fresh crates as thin
  lines, rebuilt ones as bars, the crate you edited in rust; messages beside
  it; Timeline or Output.

### Terminal

Terminals are tabs in the panel's own strip: active tab with a rust top line; a
spinner for one still running; a red mark for one that exited badly; an agent's
terminal in silver-blue, read-only. Each command and its output is a block with
a left edge in green or red, the exit code and duration on its prompt line, and
Send to an agent on failures. Paths in output are links; the live prompt shows a
faint history suggestion.

### Changes and commit

Your changes have checkboxes; an agent's changes sit in their own silver-blue
group with no checkboxes and a Review link, so unreviewed agent work cannot slip
into your commit. Above the message: live pre-commit checks from Runs (rustfmt,
clippy, tests). The message box has a 72-character counter and Draft from diff;
then Amend, Commit 1 file ⌘↵ and "and push".

### Review diff

Reviewing an agent's change. The left panel lists the task, files with viewed
checkboxes and a progress bar, and Accept all or Request changes. The diff is
side by side: red and green row tints, stronger tints on the exact words that
changed, a clear hatch where a line does not exist, collapsed unchanged runs.
Hunks are named by symbol (`impl MinimapTile › render · 4 changes`) with
Comment, Revert, Accept. The right side is the live working copy with the
language server running, so review catches errors; comments go to the agent as
its next instruction.

### Settings

One page in an editor tab (␣,), not a dialog. A serif title, a You / This
project switch, one search that also takes keys like `editor.font`. A section
index on the left marks sections you changed. Common comes first: theme cards
drawn in each theme's real colours (Yohen, Hakuji, Match system) with the
colour-blind switch under them, then rail position, editor font and size, Vim
mode, agent command permissions and notification level. Every row: label, its
key in settings.toml, one line of help, one control; a rust dot and Reset when
changed from default. Choices apply live: the example shows the rail already
moved to the right.

### Debugger

The debugger (`docs/decisions/r1b9dw3g0g-debugger.md`) fits into the parts that are
already here: its breakpoints sit over the line numbers, its controls are in the
title bar's run widget, its views are one more bottom tool window, and a session
is one more Run. There is no right dock and no debug layout. The calls behind
this section, and the script that checks its colours, are in
`docs/decisions/skgc7fe5z9-debugger-design.md`.

**Breakpoints over the line numbers.** The gutter stays 81 px (Editor, Gutter);
breakpoints get no lane. They draw in the line-number column: x 0–42 at 13 px
code, the 4 px inset and the 38 px of numbers. The column has two slots, and a
row with a glyph in either draws no line number; the glyph's tooltip and the
status bar's position give it, and the numbers above and below bracket it.
The problem dot, the fold column and the change bar don't move, so a breakpoint
and a problem dot sit side by side. Sizes are at 13 px code, on a 20 px row, and
scale with the code as the gutter does.

| State | Glyph | Slot | Colour |
| --- | --- | --- | --- |
| Enabled, verified (or no session) | Filled dot, 12 px | Breakpoint, x 14–26 | Breakpoint |
| Enabled, not yet verified | Ring, 12 px, 2 px stroke; tooltip is the adapter's message | Breakpoint | Breakpoint |
| Disabled | Filled grey dot, 12 px | Breakpoint | Disabled breakpoint |
| Current frame (the top frame, stopped) | Filled arrow pointing right, 12 × 10 px: a 6 px body and a 6 px point | Frame, x 29–41 | Hare's fur |
| Caller frame (selected in Call Stack) | The same arrow as an outline, 2 px stroke | Frame | Hare's fur |
| Conditional or hit count (later) | The state's dot or ring with a 6 × 2 px bar across its middle: cut out of a dot, drawn in the ring | Breakpoint | The state's |
| Logpoint (later) | A diamond 12 px corner to corner, filled, outlined or grey as the dot | Breakpoint | The state's |

Every state differs from the others in shape or in lightness, not only in hue:
filled against outlined, a dot against an arrow, and the disabled grey is at
least 0.13 OKLab L from the breakpoint red in both themes. A breakpoint the
adapter moved draws on the row the adapter reports for the session's lifetime.

- **Click target:** the whole line-number column of a row, 42 × 20 px at 13 px
  code, every display row of a wrapped line. A click toggles a breakpoint on
  that buffer row and doesn't move the caret; a press that moves before release
  is a drag, as in the rest of the gutter. Right-click opens the menu: Add
  Breakpoint, or Disable or Enable Breakpoint and Remove Breakpoint, and Run to
  Line while stopped; later Edit Condition…, Edit Hit Count… and Edit Log
  Message…. The keys (F9, ␣db) do the same at the caret, so the mouse is never
  the only way.
- **Hover:** the pointer over the column draws the fold chevrons' hover tile
  (the hover surface, 38 × 16 px, 4 px radius) behind the number, and after a rest
  the tooltip: "Add breakpoint", or the glyph's own text, with the key.
- **The current line:** while stopped, the top frame's row gets the current-line
  band and the selected caller frame's row the caller-frame band. Each spans the
  code area's width on every display row of the buffer row, and has a 2 px edge
  at its left, in the last 2 px of the gutter's gap to the code: hare's fur on
  the current line, muted text on a caller frame. A band is opaque and draws
  instead of the cursor line on its row; selections, find matches and the
  identifier highlight draw over it. The editor scrolls the row to the middle
  third on each stop and each frame selection. Without a gutter the bands and
  their edges still draw, the edge at the code's left.

**The run widget.** The title bar's run widget (`run_widget.rs`) chooses one
thing to run: a task or a debug configuration.

- **The picker** (Choose Task or Configuration…) has two groups under small
  faint headings: Tasks, as today, then Debug configurations, each row with the
  `bug` icon and its source as a faint suffix (launch.toml, detected,
  launch.json). Typing filters both.
- **Idle,** the widget is the label, ▷ and 🐞: with a task chosen, ▷ Run Task (■
  Stop Task while it runs) and 🐞 Debug…, which opens the picker at the debug
  configurations; with a debug configuration chosen, its label carries the
  `bug` icon and only 🐞 Debug shows, which starts it. With no debug
  configuration 🐞 isn't drawn.
- **While a session runs** the widget becomes the session's controls: the label
  names the configuration with the session's state icon (a spinner while
  running, `pause` while stopped), and its click opens the session menu (Show
  Debugger, Show Console, Show Adapter Log, Choose Task or Configuration…).
  Then, 22 px icon buttons 2 px apart: Continue or Pause in one place (both,
  side by side, while the session is uncertain), Step Over, Step Into, Step
  Out, 6 px of space, Restart, Stop. A task still running keeps its tab in the
  terminal and its row in Runs. Disabled states follow the session: no
  stepping while running, resuming or uncertain, and nothing but Stop while
  resuming.
- **Without a title bar** (Linux, and macOS without the unified title bar) the
  same controls float: a toolbar centred over the editor area, 4 px below the
  tab strip, on the overlay surface with the floating shadow and hairline, 6 px
  radius, 30 px tall (the 22 px buttons and 4 px padding), the configuration's
  label first. It shows while a session exists, and a title bar never has both.

**The Debug tool window.** Variables, Watch, Call Stack, Breakpoints and the
Debug Console are one tool window in the bottom panel, its rail icon (Lucide
`bug`) in the bottom group after Runs. The rail icon carries a hare's fur dot
while a session is stopped: it needs you. Its header is a 34 px panel tab strip:
Variables, Breakpoints and Console, then the session's label ▾, then the pane
controls. A session opens the tool window on Console once; its first stop
switches it to Variables unless you picked a tab since. Ending a session leaves
it where it is.

- **The Console is a tab here, not in the terminal panel.** It evaluates in the
  selected frame, so it sits beside the frames, and the terminal panel keeps to
  real terminals. The debuggee's own terminal stays a terminal tab (it has a
  pty).
- **Variables** is two columns, split by a hairline: Call Stack on the left (280
  px, resizable), Watch and Variables on the right. All rows are 24 px.
  - Call Stack: threads, each with its frames; a thread row reads `main · stopped:
    breakpoint` or `worker 2 · running`. A frame row is the function in small
    code with `minimap.rs:42` faint at its right; the top frame carries the filled
    arrow and the selected frame the focused selection, with the outlined arrow
    if it isn't the top. Frames with no source, or that the adapter calls
    subtle, are faint.
  - Watch: a group heading at the top of the right column, its expressions as
    tree rows, then an Add watch row (a small input, "Add a watch expression").
    A watch that fails shows its error in shinsha with ✕.
  - Variables: one root per scope, the first expanded, as a tree. A row is the
    name in property colour, `=`, the value in ink, both small code, and the
    type faint at the right; a long value ends in an ellipsis and its tooltip
    holds it all. Children page by 100, with a "Show 100 more" row.
- **Breakpoints**: Exception filters first, as checkboxes with the adapter's
  labels and descriptions as tooltips; then breakpoints grouped by file as
  Problems groups (file icon, path, count). A row is a checkbox (enabled), the
  state's glyph, `line 42`, and the line's code in faint small code; later its
  condition in mono after it. Selecting a row reveals the line.
- **Console**: output rows in small code, 18 px (its line height, not a 24 px
  row), by category: program output in
  ink, its error stream in shinsha with a 2 px shinsha left edge (as a failed
  command block), the adapter's own messages faint, important ones kiseto with
  ▲. Under them, an input (26 px, `›` prompt, history on ↑ ↓). An evaluation
  echoes `› expr` in muted, then its result as an expandable value row; an
  error in shinsha with ✕.
- **Empty states**, centred, in muted body text, at most one button:
  - no session: "Not debugging." and Debug ‹configuration›, or "No debug
    configurations." and Add launch.toml;
  - running: "Running. Pause to see variables and the call stack." and Pause;
  - a frame with no scopes: "No variables in this frame.";
  - no breakpoints: "No breakpoints. Click a line number or press F9.", and with
    no session "Exception filters show when a session starts.";
  - Console with no session: "Start a session to evaluate expressions."

**Everywhere else.**

- **Runs:** a session is a run of its own kind, with the `bug` icon, the
  configuration as its mono title and a one-line summary: `stopped at
  minimap.rs:42 · breakpoint`, `running`, `exited 0 in 4.2 s`. Running or
  stopped it has the thin rust progress line, without a fraction; stopped, its
  icon is `pause` in hare's fur. Its detail offers Show Debugger, Restart and
  Stop, and once it has ended, Debug Again and Show Output. A pre-launch task is
  its own run, before it.
- **Operation in progress:** a session is listed from launch to end: spinner and
  "Debugging ‹configuration›" while running, `pause` and "Paused in
  ‹configuration›" while stopped, no bar. Clicking its entry shows the Debug tool
  window.
- **Test lens:** `▷ Run test · Debug` (Editor, language-server formatting): Debug
  is a second item, after a faint `·`, that debugs that test (D13). It shows only
  where the test's framework can be debugged (Cargo and pytest first, Go once
  delve is supported); a module's `▷ Run tests` has no Debug.
- **Debug this test** in a test failure (Runs: tests) debugs the failed test as
  the lens does, with a one-shot breakpoint on the failure's line when it is in
  the project, so the session stops at the assertion.
- **Hover value:** while stopped, the hover card gets a section above the
  language server's: a faint `value · frame render` line, then `name: Type =
  value` in syntax colours, then up to 20 children as `field = value` rows in
  small code and "+N more in Variables" (it opens Variables at that value), a
  hairline, then the language server's hover as today. Values don't expand in
  the card.

**Colour roles.** Four new editor roles (theme keys in `[editor]`, D6). Bands are
checked against code text and glyphs against the gutter and the cursor line;
the check and its output are in the decision file.

| Role | Key | Yohen | Hakuji | Falls back to | Used for |
| --- | --- | --- | --- | --- | --- |
| Current line | `debug_current_line` | `#351501` | `#fff4ed` | `active_line` | The top frame's band, with a 2 px hare's fur edge |
| Caller frame | `debug_frame_line` | `#2a1a11` | `#f6f7f9` (the cursor line's) | `active_line` | A selected caller frame's band, with a 2 px muted edge |
| Breakpoint | `debug_breakpoint` | `#f2716f` | `#a1100e` | `error` | Enabled and unverified breakpoints (shinsha's value, its own role) |
| Disabled breakpoint | `debug_breakpoint_disabled` | `#827468` | `#7c828a` | `gutter_foreground` | Disabled breakpoints |

- Code text on both bands keeps its floor: comment, the lowest, is 6.03:1 and
  6.05:1 in Yohen, 4.52:1 and 4.56:1 in Hakuji. Each band replaces the cursor
  line, so these are the cursor line's numbers too.
- Every glyph is 3:1 or better wherever it sits: the gutter, the cursor line and
  the hover tile, and in the tool window's rows bare, hovered, selected and
  selected with focus. The lowest is the disabled breakpoint, 3.08:1 on a
  focused row in Yohen and 3.22:1 on a selected row in Hakuji. The edges clear
  3:1 on their bands.
- The comment's floor leaves the bands little room: on the cursor line it sits at
  6:1 in Yohen and 4.5:1 in Hakuji, so no band can be lighter in Yohen or darker
  in Hakuji. Hakuji's current line is 3.0 ΔE from the editor and its caller frame
  is the cursor line itself; the edge and the arrow carry them, not the fill.
- A breakpoint is the one shared value: it is red because every debugger's is,
  and it never sits in the problem column, so it is shinsha's colour under its
  own role, which a theme may set apart.

**Accessibility.** Every string is in `debug.ftl`; every control has a name.

| Control | Accessible name |
| --- | --- |
| Line-number column (tooltip and name) | "Add breakpoint, line 42"; "Breakpoint, line 42"; "Breakpoint, line 42, not bound: ‹adapter message›"; "Disabled breakpoint, line 42"; "Stopped here, line 42"; "Caller frame, line 42" |
| Run widget label | "Task: ‹label›", "Debug configuration: ‹name›", "Debug session: ‹name›, running" or ", stopped at line 42" |
| Run widget and toolbar buttons | Run Task, Stop Task, Debug, Debug…, Continue, Pause, Step Over, Step Into, Step Out, Restart, Stop Debugging; tooltips add the key |
| Floating toolbar | A toolbar named "Debug" |
| Rail | "Debug", and "Debug, stopped" while stopped |
| Tool window | Tabs Variables, Breakpoints, Console; trees "Call stack", "Watch", "Variables", "Breakpoints"; "Add a watch expression"; "Evaluate in ‹frame›"; the list "Console output" |
| Rows | Frame: "render, minimap.rs line 42, current frame"; variable: "budget, Budget, ‹value›"; breakpoint checkbox: "Breakpoint, minimap.rs line 42"; exception filter checkbox: the adapter's label |

Announcements, polite, one per change:

- "Debugging ‹configuration›" when a session starts;
- "Stopped at line N in ‹file›: ‹reason›" (breakpoint, step, pause, exception
  ‹text›, entry) on each stop, and "Running" on continue;
- "Frame K of M: ‹function›, line N" on a frame selection;
- "Breakpoint added at line N", "removed", "disabled", "enabled", and
  "Breakpoint at line N moved to line M" when the adapter moves one;
- "‹configuration› ended: exited with code N", or "Debugging stopped";
- a console result, read as its value.

**Sizes.** Every size above is a kiln-ui role (`roles.rs`) or one to add:

| Size | Role |
| --- | --- |
| Rows in every view but the Console's output | `height::ROW` (24) |
| Console output rows | `CODE_SMALL`'s line height (18) |
| Tool window header | `height::TAB_STRIP` (34) |
| Console input, Add watch | `height::CONTROL` (26) |
| Toolbar and run widget buttons | `height::CONTROL_SMALL` (22), 2 px apart (`space::S2`), 6 px between groups (`space::S6`) |
| Floating toolbar | New `height::TOOLBAR` (30): the buttons and `space::S4` padding; `radius::MENU`, `SHADOW_FLOAT`; 4 px (`space::S4`) below the tab strip |
| Run widget label | New `width::TITLE_LABEL_MAX` (200), today `run_widget.rs`'s `LABEL_MAX_WIDTH` |
| Call Stack column | New `width::CALL_STACK_DEFAULT` (280) |
| Icons | New `glyph::ICON` (16), the design's line icons |
| Dot, ring and diamond; the arrow's width; the same glyphs in rows | New `glyph::BREAKPOINT` (12) at 13 px code; the gutter scales the glyphs as its columns |
| The arrow's height | New `glyph::ARROW_HEIGHT` (10) |
| The arrow's point and body; the conditional bar's width | New `glyph::HALF` (6), half of `glyph::BREAKPOINT` |
| Rings, outlined arrows, the conditional bar's height, band and console edges | New `glyph::STROKE` (2) |
| The two slots' left edges in the number column | New fe-gpui gutter constants beside `GUTTER_INSET`: `GUTTER_BREAKPOINT_X` (14) and `GUTTER_FRAME_X` (29), so the frame slot ends 1 px before the problem column |
| Click target | `GUTTER_INSET` + `GUTTER_NUMBERS` (42) wide, `CODE`'s line height (20) tall, per display row |
| Hover tile | `GUTTER_NUMBERS` (38) wide, `CODE`'s line height less 2 × `space::S2` (16) tall, `radius::CONTROL` (4) |
| Type | `CODE_SMALL` for frames, values, console and the hover's children; `UI_BODY` for empty states; `UI_SMALL` for group headings and suffixes; `UI_LABEL` for the session label |

The gutter's columns are fe-gpui's (`GUTTER_INSET`, `GUTTER_NUMBERS` in
`element.rs`, pinned by `gutter_b3.rs`), not kiln-ui roles, since they scale
with the code. New icons from Lucide: `pause`, `redo-dot` (Step Over),
`arrow-down-to-dot` (Step Into), `arrow-up-from-dot` (Step Out); `bug`, `play`,
`square` and `rotate-ccw` (Restart) are in kiln-ui.

**Keys.** F5 start or continue, ⇧F5 stop, ⇧⌘F5 restart, F6 pause, F9 toggle a
breakpoint, F10 and F11 step over and into and ⇧F11 step out while stopped
(only then, so Linux keeps F10 and F11), ⌘F10 run to cursor. With the LazyVim
preset: ␣db breakpoint, ␣dc continue, ␣dO over, ␣di into, ␣do out, ␣dP pause,
␣dt stop, ␣du the Debug tool window, ␣dr the Console, ␣dk ␣dj up and down the
stack, ␣td debug the nearest test.

### Remote development

A remote window (`docs/designs/remote-development.md`, the record
`docs/decisions/j8wvgszah0-remote-development.md`) is a normal window whose
root lives on an ssh host. It adds one title-bar part, one sheet, one tool
window and a handful of pending states to parts that are already here; nothing
else changes shape. The calls behind this section, where it parts from the
design file, and the script that checks its colours are in
`docs/decisions/y0xkvvewjh-remote-design.md`. The examples use the host `neon`.

Every text this section draws is ink or muted, never faint, so it clears the
code floor too: 6:1 in Yohen and 4.5:1 in Hakuji, wherever it sits. The one
exception is a primary button's label, `text.on_accent` on hare's fur, which
clears the same floors (Buttons, below). Every
spinner is the status bar's (Lucide `loader-circle`, a turn a second, 12 px),
and none shows before 150 ms, so a fast reply never flickers one.

**The remote pill.** A 20 px pill in the title bar, first in the left group:
back and forward, 6 px, the pill, 2 px, then project ▾ and branch ▾ as today.
It holds a 12 px glyph and the host's name, 6 px of padding each side and 4 px
between, fully rounded (10 px radius). The name is UI label in ink and ends in
an ellipsis past 200 px; the pill gives way last as the bar narrows (branch,
then project, then the pill's name), and never below its glyph. Local windows
have no pill.

| State | Glyph | Tone | Tooltip | Click menu |
| --- | --- | --- | --- | --- |
| Connecting | Spinner | Neutral | "Connecting to neon…" | Cancel, Show Log |
| Installing | Spinner, and a progress underline | Neutral | "Installing tenmoku-server… 60%" | Cancel, Show Log |
| Connected | `server` | Neutral | "Connected to neon (me@neon.tailnet.ts.net) · 34 ms" | Disconnect, Forwarded Ports (2), Show Log, Close Remote Window |
| Connected with ports | `server`, then `· 2` | Neutral | "Connected to neon (me@neon.tailnet.ts.net) · 34 ms · 2 forwarded ports" | As Connected |
| Reconnecting | Spinner | Warning | "Lost connection to neon. Reconnecting (attempt 3, next in 8 s)." | Reconnect Now, Disconnect, Show Log |
| Disconnected | `unplug` | Error | "Disconnected from neon. Your edits are kept." | Reconnect, Show Log, Close Remote Window |
| Needs sign-in | `key-round` | Warning | "neon is waiting for you to sign in." | Sign In, Disconnect |

- **Tones.** Neutral: no fill, a 1 px control-border edge, the glyph in muted;
  hovered, the hover fill. Warning: the kiseto-soft fill, a 1 px kiseto edge,
  the glyph in kiseto; hovered, the warning pill hover. Error: the same in
  shinsha. The name stays ink in every tone, so the tone is the fill, the edge
  and the glyph, never the text. The edge tells neutral from a signal by
  lightness (control border against kiseto or shinsha, 0.31 OKLab L or more
  apart in both themes); the two signal tones are told apart by their glyphs
  (a spinner or a key against `unplug`) and their words.
- **The progress underline** (Installing): a 2 px line across the pill's
  content, inset 6 px each side and 2 px above its bottom edge, its track in
  the control border and its fill in hare's fur, at the install's fraction.
  It is the Runs progress line at pill size.
- **The forward count:** while ports are forwarded, a `·` and the count follow
  the name, 4 px apart, both UI label in muted (`neon · 2`). The count shows
  only in Connected. Reconnecting and Needs sign-in keep the forwards' local
  listeners (new connections wait or are refused) without counting them, and
  they resume on reconnect. Disconnected has none: a disconnect, or the grace
  running out, removes every forward, and the fresh session of a Reconnect
  brings back only the ports `remote.ports` declares.
- **The click menu** opens under the pill, left-aligned to it: the overlay
  surface, `radius::MENU`, the floating shadow, 28 px rows. Its first row is a
  header that isn't a choice: the host's ssh name and the latency, UI small in
  muted (`me@neon.tailnet.ts.net · 34 ms`), then a hairline, then the state's
  items as in the table. Sign In brings the waiting prompt forward;
  Forwarded Ports (n) shows the Ports tab; Show Log opens the connection log
  (ssh's stderr and the bootstrap's transcript) as a read-only editor tab,
  "Remote log — neon". Every item is also a palette command, so the pill is
  never the only way: Remote: Disconnect, Remote: Reconnect, Remote: Show Log,
  Remote: Show Ports, Remote: Forward a Port…, Remote: Close Remote Window.
- **Both rail sides.** The title bar doesn't flip with the rail
  (`workbench.rail`): the pill stays first in the left group with the rail on
  either side, as project and branch do, and the artboard shows it with both.
- **Without a title bar** (Linux, and macOS without the unified title bar), the
  pill is the status bar's first item, before the Vim mode pill, with the same
  states, menu and name, as the agents chip becomes the `agents` status item.
  A window never shows two pills.

**The Open Remote sheet.** File › Open Remote… (`remote.open`) and the Welcome
action open a modal sheet over the current window: 520 px wide, the overlay
surface, `radius::FLOAT`, the floating shadow, 16 px padding, over the modal
scrim. Its heading (UI heading, ink) names the step; under it the step's body;
then the footnote; then the buttons, 26 px, 8 px apart, right-aligned with the
default last, and Back or Show Log on the left. Esc cancels what is in flight
and closes the sheet; Back returns a step.

- **Buttons.** The default (Connect, Open, Retry, Open Settings when it leads,
  OK) is kiln-ui's primary button: `accent.base` filled and edged, its label
  `text.on_accent`; hovered and pressed, `accent.hover`, which the bundled
  themes now set to an opaque value (Colour roles) instead of kiln's fallback,
  the accent at 85% alpha, which drops the label to 5.24:1 in Yohen and 4.30:1
  in Hakuji. The others (Cancel, Back, Show Log, and every empty-state button
  in this section) are secondary: no fill, the control-border edge, an ink
  label; hovered `state.hover`, pressed `state.pressed`. A disabled button
  (Connect on an empty field) keeps its colours at kiln-ui's disabled opacity,
  and isn't held to a floor, as WCAG exempts inactive controls. The prompts'
  default buttons are the same primary button.

- **Host** ("Open Remote"). One field, labelled Host, 26 px, small code, with
  the placeholder `ssh alias, user@host or user@host:port`. Typing opens a
  completion list under the field: the overlay surface, `radius::MENU`, 24 px
  rows, at most eight before it scrolls. A row is the `server` glyph (16 px,
  muted), the host as typed (small code, ink), its config `HostName` when it
  has one (small code, muted, 12 px after), and at the right its source in UI
  small muted: "recent", "ssh config" or "typed". Recent hosts come first.
  ↑ ↓ move, Tab or Enter takes a row, and Enter again connects. Buttons:
  Cancel, Connect (off until the field has text). Connect with a malformed
  value (a space, a scheme other than `ssh://`) draws the field's edge in
  shinsha and, under it, a 12 px `circle-x` in shinsha and "Not an ssh host"
  in muted UI small, until the field changes. A pasted `ssh://host/path`
  fills Host and keeps the path for Folder.
- **Connecting** ("Connecting to neon"). A 16 px spinner and one status line
  in UI body, ink, at the body's left; under the line, while there is a
  fraction, the 72 × 4 px bar of the operation in progress, track in the
  control border and fill in hare's fur, with the percentage in the line.
  Buttons: Show Log on the left, Cancel. The lines, in order:

  | State | Line | Bar |
  | --- | --- | --- |
  | ssh starting | "Connecting to neon…" | None |
  | Waiting on the user | "Waiting for you to sign in to neon" | None; the sign-in prompt is up |
  | Probing | "Checking neon…" | None |
  | Downloading | "Downloading tenmoku-server 0.3.0 for linux-x86_64… 43%" | Yes |
  | Verifying | "Verifying…" | None |
  | Uploading | "Installing on neon… 60%" | Yes |
  | Starting | "Starting tenmoku-server…" | None |

  A first refused handshake is one more line in the same place, "tenmoku-server
  on neon is a different build (0.2.9+ab12cd3). Reinstalling…", and the steps
  from Uploading run again. On a first connect the footnote reads "Tenmoku
  installs its server in ~/.cache/tenmoku-server on neon. It runs only while
  you're connected." and ends in a Learn more link, both UI small in muted;
  later connects have no footnote.
- **Folder** ("Open Folder on neon"). A path field (26 px, small code)
  prefilled with the remembered path or the remote `$HOME`, which completes
  directory names inline after 150 ms or on Tab; its right end holds the row
  spinner while a listing is out. Under it, a list 10 rows tall (240 px,
  scrolling): folders first (the folder glyph in its role's tint, the name in
  ink), then files (the file glyph and name in muted, dimmed and not
  choosable). → or a double click enters a folder, ← or Backspace on an empty
  segment goes up, and a `.` shows hidden entries. Buttons: Back on the left,
  Cancel, Open. A listing that fails shows its error in the list's place as an
  error line (below).
- **Errors.** An error replaces the status line: a 16 px `circle-x` in
  shinsha (`shield-alert` for a changed host key), then the message in UI body,
  ink, wrapping; ssh's own words in it are small code. The heading stays the
  step's. The first action in each row is the default; Show Log sits on the
  left. Nine causes are errors:

  | Cause | Message | Actions |
  | --- | --- | --- |
  | ssh failed (DNS, refused, timeout) | "Couldn't connect to neon: ‹ssh's last stderr line›" | Retry, Back; Show Log |
  | Authentication failed or cancelled | "neon didn't accept the sign-in." | Retry, Back; Show Log |
  | Host key changed | "The host key for neon has changed. This could mean someone is intercepting the connection. Tenmoku won't connect until the key in ~/.ssh/known_hosts is fixed." | Back; Show Log. No override |
  | Unsupported host | "tenmoku-server isn't built for FreeBSD amd64." | Back |
  | No matching release (dev build) | "This development build has no matching tenmoku-server release. Set remote.dev_server to a server built for linux-x86_64." | Open Settings, Back |
  | Offline, no cache | "Couldn't download tenmoku-server 0.3.0: ‹error›" | Retry, Open Settings, Back |
  | Verification failed | "The downloaded tenmoku-server failed its signature check and was deleted." | Retry, Back; Show Log |
  | Host can't install | "Couldn't install on neon: ~/.cache isn't writable (mkdir: Permission denied)", or "…: no space left on device" | Back; Show Log |
  | Dev server is another build | "remote.dev_server is build 0.3.0+9f8e7d6, this client is 0.3.0+ab12cd3-dirty. Build it from the same checkout." | Open Settings, Back; Show Log |

  A second refused handshake, after the reinstall, is the error "tenmoku-server
  on neon still refused the connection after reinstalling." with Back and Show
  Log.
- **The sign-in prompt.** ssh's prompts use the git credential prompt's frame
  (`git_askpass.rs`: the palette frame at the top of the window, 600 px), one at
  a time across windows, each titled with the host. It gains OK and Cancel at
  the status line's right for the mouse; Enter and Esc stay. Three kinds:
  - **A secret** (passphrase, password, OTP, keyboard-interactive): titled
    "Sign in to neon". ssh's prompt verbatim as the field's label, a masked
    field, and the status "neon asks for this. Enter sends it to ssh, Escape
    cancels; it isn't saved." Buttons OK, Cancel.
  - **An unknown host:** titled "Unknown host: neon". ssh's whole text in small
    code, selectable so the fingerprint can be compared, and no field. Buttons
    Connect (answers `yes`) and Cancel; Connect isn't the default, so Enter
    alone never accepts a key.
  - **A notice** (FIDO touch): titled "Touch your security key for neon".
    ssh's text, a 16 px `key-round` in muted, no field, and Cancel only. It
    closes by itself when ssh goes on.

  A git credential on the host uses the secret kind, titled "Sign in to origin
  on neon".

**Pending states.** Typing never waits; each remote operation shows one of
these until the host answers.

- **Loading tab.** A file opens as a tab at once, read-only and empty. After
  150 ms its tab draws a spinner in the close button's place (the 16 px box)
  and its label in muted; the editor area shows "Loading main.rs from neon…"
  centred in UI body, muted, with no gutter. The content replaces both when it
  lands. A large file streams through the large-file path, with its operation
  in the status bar.
- **Saving ring.** While a save is in flight the unsaved dot becomes a 12 px
  ring (Lucide `circle`) in the unsaved dot's colour (`state.dirty`), in the
  same box, and the tab turns clean when the host confirms. It shows hover's
  close button as the dot does.
- **Save pending.** A save made while reconnecting turns the dot into a 12 px
  `clock` (the ring with hands) in the same colour, with the tooltip "Will save
  when neon is back". On resume it becomes the ring, then clean; a changed
  file asks its usual Overwrite / Cancel.
- **Not loaded — reconnecting.** A file opened while reconnecting opens as a
  tab whose label is muted and whose close box holds the spinner; the editor
  area shows the empty state "Not loaded — reconnecting to neon. It opens when
  neon is back." centred in muted UI body, with one button, Reconnect Now. It
  loads on resume.
- **Tree.** Expanding a folder draws the spinner in the row's chevron place
  after 150 ms, and the children replace it. A create, rename, move or delete
  is never shown as done: the row stays as it was, its glyph and name in muted,
  with the spinner at its right end where the git status letter sits, and a
  muted suffix saying what is pending ("renaming to lib.rs", "moving to lib/",
  "deleting"); a new file or folder is a row of its typed name in the same
  style. The host's reply settles it, and a refusal is a toast. While
  reconnecting a folder keeps its old children, in muted.
- **Quick Open.** Results fill in as the host walks the tree; until the walk
  ends the list ends in a 24 px footer row, not a choice: the spinner and
  "Indexing… 12,480 files" in UI small, muted.
- **Terminal.** While reconnecting a terminal tab draws the modal scrim over
  its content and, centred on it, a card: the overlay surface with the 1 px
  hairline and the floating shadow, `radius::MENU`, 8 px by 12 px of padding,
  the spinner and "Reconnecting…" in UI body, ink. It takes no input. On
  resume the scrim and card go; if output was lost, the terminal shows one
  line, "Output was lost while disconnected", in muted small code, where the
  gap is.
- **Operation in progress.** The status bar's existing slot lists the remote
  work, with nothing new: "Connecting to neon" (spinner), "Installing
  tenmoku-server on neon" (bar and percentage), "Opening big.log from neon"
  (bar, for streamed files), "Indexing neon" (spinner and file count) and, on
  resume, "Saving 2 files to neon" (count). Clicking an entry opens the list as
  today; a connect's entry offers Show Log.

**The Ports tab.** A tool window in the bottom panel, in remote windows only.
Its rail item (Lucide `ethernet-port`) is in the bottom group after Terminal,
on whichever side the rail is, with no badge: the pill carries the count. Its
header is the 34 px panel strip: "Ports", then Forward a Port…, then the pane
controls.

- **Rows**, 24 px, one per forwarded or detected port, in four columns sized
  to their content 24 px apart, Process taking the rest:

  | Column | Content |
  | --- | --- |
  | Port | A 12 px glyph, `arrow-left-right` (forwarded), `circle-dashed` (detected, not forwarded) or the spinner (starting), then `5173` in small code |
  | Local address | `localhost:5174` in small code; "5173 was in use" after it in UI small, muted; "Starting…" while it starts; "Not forwarded" on a detected row |
  | Process | `node (vite)` and `· 48213` (pid) in small code, muted; empty when the host doesn't know |
  | Source | Detected, Task "dev", Settings or Added, in UI label, muted |

  A forwarded row is ink; a detected row that isn't forwarded is dimmed: every
  cell in muted, with the dashed glyph, so it differs in lightness and shape,
  not only in tint. While reconnecting, rows stay as they are.
- **Row actions:** icon buttons (22 px, 2 px apart) at the row's right on hover
  or selection, ghost buttons with muted icons. Which are on follows the row,
  since only a forwarded row has a local address:

  | Row | Open in Browser (`external-link`) | Copy Address (`copy`) | Last button |
  | --- | --- | --- | --- |
  | Forwarded | Opens `http://localhost:5174` | Copies `localhost:5174` | Stop Forwarding (`x`) |
  | Starting | Off until the address exists | Off until the address exists | Stop Forwarding (`x`): cancels the forward |
  | Detected, not forwarded | Forwards the port, then opens the local address it gets; if the forward fails, its error is a toast and nothing opens | Off: there's no address | Forward (`arrow-left-right`) |

  An off button is drawn at kiln-ui's disabled opacity and stays in its place,
  so the buttons don't shift between states. The row's context menu has the
  same items in the same states. Keys follow the table: Enter is Open in
  Browser (on a starting row it does nothing), ⌘C copies only when there's an
  address, Delete stops a forwarded or starting row's forward, and ⌘↵ forwards
  a detected row.
- **Forward a Port…** opens an input row at the top of the list (26 px, small
  code, the placeholder `3000 or 3000:13000 (host:local)`), as Watch's Add
  watch row does. Enter forwards and closes it, Esc closes it. Enter on a
  malformed value keeps the row open and marks the input invalid: its edge in
  shinsha and, under it, a 12 px `circle-x` in shinsha and "Not a port: use
  3000 or 3000:13000" in muted UI small. The message is the input's
  description while it shows, and it's announced once per failed Enter; it
  goes when the value changes. A port the host or the client refuses (in use
  on both, say) is the same kind of message: "Couldn't forward 3000: ‹error›".
- **Empty state**, centred, muted UI body: "No ports yet. Ports your programs
  listen on appear here." and one button, Forward a Port….

**Toasts.** The five are B3 toasts, each at its level's icon and colour
(`toast.rs`); identical ones count up (×3) rather than stack while a link flaps.

| Toast | Level | Title, line | Actions | Goes |
| --- | --- | --- | --- | --- |
| Lost | Warning | "Lost connection to neon", "Your edits are kept, and saves will finish when it's back." | None | After the usual lifetime; the pill stays amber |
| Reconnected | Success | "Reconnected to neon" | None | After the usual lifetime |
| Disconnected | Error | "Disconnected from neon", "Terminals and language servers there were stopped. Your edits are kept." | Reconnect | Stays until dismissed or reconnected |
| Port detected | Info | "Port 5173 is listening on neon" | Open in Browser, Forward | After the usual lifetime; the row stays in Ports |
| AI unavailable | Info | "AI chat and agents aren't available in remote windows yet" | None | After the usual lifetime |

**Closing with pending saves.** The app prompt (the platform's alert, or
kiln-ui's 420 px prompt where there is none), at the warning level: "2 files
have changes that weren't saved to neon.", the detail "Tenmoku keeps them in
its backups and offers them the next time you connect to app on neon.", and
Keep for Next Time (the default), Discard and Cancel.

**Recents and Welcome.** Open Recent lists a remote folder as `app — neon`
with the `server` glyph in the icon slot where a local row has its folder, its
location (`ssh://neon/home/me/src/app`) in muted small code as a local row's
path is, so the same folder on two hosts is two rows. The Welcome page (still
to design) gets Open Remote… beside Open Folder, and lists its recent remote
folders as these same rows.

**Colour roles.** Two new roles, theme keys in a new `[remote]` table, and a
value for the existing `accent.hover` in both bundled themes; every other
colour is an existing token. The check and its output are in the
decision file.

| Role | Key | Yohen | Hakuji | Falls back to | Used for |
| --- | --- | --- | --- | --- | --- |
| Warning pill hover | `remote.pill_warning_hover` | `#3f3216` | `#e5e2ca` | `status.warning_soft` | A hovered Reconnecting or Needs sign-in pill: kiseto-soft a tenth of the way to kiseto |
| Error pill hover | `remote.pill_error_hover` | `#482524` | `#f6d2cd` | `status.error_soft` | A hovered Disconnected pill: shinsha-soft a tenth of the way to shinsha |
| Primary button hovered | `accent.hover` (an existing key, now set in both bundled themes) | `#e39a68` | `#863b12` | kiln's `accent.base` at 85% alpha | A hovered or pressed primary button: hare's fur a fifth of the way to ink (Yohen) or black (Hakuji) |

| Reuse | Token | Yohen | Hakuji |
| --- | --- | --- | --- |
| Neutral pill edge | `border.control` | `#4a3a2e` | `#c6cad0` |
| Neutral pill hover | `state.hover` | `#2b221d` | `#ebecef` |
| Neutral glyph, count, every muted text | `text.muted` | `#b8a99a` | `#4b525b` |
| Warning pill fill; its edge and glyph | `status.warning_soft`; `status.warning` (kiseto) | `#2c2311`; `#e8b440` | `#f1efe0`; `#776a00` |
| Error pill fill; its edge and glyph; error icons and field edges | `status.error_soft`; `status.error` (shinsha) | `#351d1c`; `#f2716f` | `#ffe7e2`; `#a1100e` |
| Install underline and sheet bar | `accent.base` (hare's fur) on `border.control` | `#e0874c` | `#a84a16` |
| Primary button and its label | `accent.base`; `text.on_accent` | `#e0874c`; `#1c1613` | `#a84a16`; `#ffffff` |
| Secondary button: edge; label; hovered; pressed | `border.control`; `text.primary`; `state.hover`; `state.pressed` | `#4a3a2e`; `#ede4d6`; `#2b221d`; `#2f2621` | `#c6cad0`; `#1f2328`; `#ebecef`; `#e8eaee` |
| Saving ring, save-pending clock | `state.dirty` (the unsaved dot's, kiseto in both themes) | `#e8b440` | `#776a00` |
| Terminal scrim | `surface.overlay` | `#00000080` | `#0000004d` |

- Every text clears its floor: the lowest is muted on a focused row, 6.09:1 in
  Yohen, and muted on a selected row, 6.56:1 in Hakuji. The pill's name is
  9.93:1 or better on every pill ground. A primary button's label is 6.59:1
  (Yohen) and 5.74:1 (Hakuji) at rest, 7.74:1 and 7.93:1 hovered or pressed.
- Every glyph and edge is 3:1 or better on its ground: the lowest is the
  install underline on its track, 4.00:1 and 3.49:1, and Hakuji's kiseto on
  the warning pill hover, 4.18:1.
- Hakuji's soft grounds are only 2.6 and 3.2 ΔE from the chrome, so the 1 px
  signal edge, not the fill, marks the pill there; hovered they are 6.3 and
  8.1.

**Accessibility.** Every string is in a new `remote.ftl` in fe-workbench; the
tab states are kiln-ui's (`kiln-ui.ftl`). Every control has a name.

| Control | Accessible name |
| --- | --- |
| Pill (a button with a menu) | "Remote: neon, connecting"; "Remote: neon, installing tenmoku-server, 60 percent"; "Remote: neon, connected, 34 milliseconds"; "…, 2 forwarded ports"; "Remote: neon, reconnecting, attempt 3, next in 8 seconds"; "Remote: neon, disconnected, your edits are kept"; "Remote: neon, waiting for you to sign in" |
| Pill menu | A menu named "neon"; its items by their labels; the header row as its description |
| Sheet | A dialog named by its heading; the field "Host", its list "Hosts" with options "neon, neon.tailnet.ts.net, recent"; the field invalid while "Not an ssh host" shows, with it as the field's description; the status line a status; the bar a progress bar named by its line; the field "Folder on neon"; the list "Folders in /home/me/src", rows "src, folder" and "README.md, file, not selectable"; the error a status; the footnote's link "Learn more about the remote server" |
| Sign-in prompt | A dialog named by its title; the field named by ssh's prompt; the unknown host's text a read-only text named "ssh's message"; the notice a status |
| Tabs | "main.rs, loading"; "main.rs, saving"; "main.rs, save pending, will save when neon is back"; "main.rs, not loaded, reconnecting" |
| Tree rows | "src, loading"; "main.rs, renaming to lib.rs"; "lib/, moving to src/"; "old.rs, deleting"; "new.rs, creating" |
| Quick Open footer | "Indexing, 12,480 files so far" |
| Terminal | Its tab's name gains ", reconnecting"; the card is a status |
| Ports | The rail item "Ports" ("Ports, 2 forwarded"); a table "Ports" with its four column headers; rows "Port 5173, forwarded to localhost:5174, node (vite), process 48213, detected", "Port 8080, starting" and "Port 3000, detected, not forwarded"; on a forwarded row the buttons "Open localhost:5174 in browser", "Copy address localhost:5174", "Stop forwarding port 5173"; on a starting row "Open in browser" and "Copy address", both unavailable, and "Cancel forwarding port 8080"; on a detected row "Forward port 3000 and open in browser", "Copy address" (unavailable) and "Forward port 3000"; the header's "Forward a Port…"; the input "Forward a port, as 3000 or 3000:13000", invalid while its error shows, with the error as its description |
| Close prompt | An alert named by its message |
| Recent row | "app on neon, remote folder, ssh://neon/home/me/src/app" |

Announcements, polite, one per change (progress isn't announced: the bar has
its value):

- the pill's state on each change: "Connecting to neon", "Installing
  tenmoku-server on neon", "Connected to neon", "Lost connection to neon.
  Reconnecting.", "Reconnected to neon", "Disconnected from neon. Your edits
  are kept.", "neon is waiting for you to sign in";
- in the sheet, each step's heading, each status line, and an error's message;
- "Port 5173 forwarded to localhost:5174", "Stopped forwarding port 5173";
- a refused submission's message, once each time: "Not an ssh host" (Connect
  in Host), "Not a port: use 3000 or 3000:13000" and "Couldn't forward 3000:
  ‹error›" (Forward a Port);
- "main.rs will save when neon is back" on a queued save, and "Saved 2 files
  to neon" when the queue is done;
- "Terminal reconnecting" and "Terminal reconnected", or "Output was lost while
  disconnected";
- "Indexing finished: 50,000 files" when Quick Open's walk ends while it is
  open;
- toasts, as every toast is.

**Sizes.** Every size above is a kiln-ui role (`roles.rs`) or one to add:

| Size | Role |
| --- | --- |
| Pill | New `height::PILL` (20); `radius::FLOAT` (10, half its height); `space::S6` padding, `space::S4` between glyph and name and before the count; the name's width `width::TITLE_LABEL_MAX` (200) |
| Pill's place | `space::S6` after forward, `space::S2` before project (`title_bar.rs`'s `NAV_GAP` and `GAP`) |
| Pill edge, card and field hairlines | New `glyph::HAIRLINE` (1) |
| Progress underline | `glyph::STROKE` (2) tall, inset `space::S6`, `space::S2` above the bottom |
| Pill and row glyphs, spinners, ring, clock, field-error icon | New `glyph::ICON_SMALL` (12), today `operations_status.rs`'s `SPINNER` and the tab's close icon |
| Sheet and empty-state icons, the sign-in notice's key | `glyph::ICON` (16) |
| Close box the ring and clock sit in | The tab's 16 px close box (`tab_bar.rs`'s `CLOSE_BOX_SIZE`), moving to new `glyph::CLOSE_BOX` (16) |
| Recents icon slot | The picker's 14 px icon (`recent_picker.rs`), moving to new `glyph::ROW_ICON` (14) |
| Pill menu rows | `height::MENU_ROW` (28); `radius::MENU`, `SHADOW_FLOAT` |
| Sheet | New `width::SHEET` (520); `space::S16` padding, `space::S12` between field, body and buttons; `radius::FLOAT`, `SHADOW_FLOAT` |
| Fields and buttons | `height::CONTROL` (26), buttons `space::S8` apart |
| Completion and folder rows, Ports rows, Quick Open footer | `height::ROW` (24); the completion list at most 8 rows, the folder list 10 (240) |
| Completion rows' HostName and source | `space::S12` after the host; the source right-aligned |
| Connecting bar | New `width::PROGRESS_BAR` (72) and `height::PROGRESS_BAR` (4), today `operations_status.rs`'s `BAR_WIDTH` and `BAR_HEIGHT` |
| Sign-in prompt | The palette frame, new `width::PALETTE` (600), today `palette.rs`'s `PALETTE_WIDTH` |
| Close prompt | New `width::PROMPT` (420), today `prompt.rs`'s `PROMPT_WIDTH` |
| Toasts | New `width::TOAST` (340), today `toast.rs`'s `WIDTH` |
| Terminal card | `space::S8` by `space::S12` padding, `radius::MENU`, `SHADOW_FLOAT` |
| Ports header | `height::TAB_STRIP` (34) |
| Ports columns | Sized to content, `space::S24` apart |
| Row action buttons | `height::CONTROL_SMALL` (22), `space::S2` apart |
| Type | `UI_LABEL` for the pill's name and count, the Source column; `UI_BODY` for status lines, messages, empty states and the terminal card; `UI_HEADING` for the sheet's heading; `UI_SMALL` for the footnote, field errors, the menu header and the Quick Open footer; `CODE_SMALL` for hosts, paths, ports, addresses, processes and ssh's words |

New Lucide icons: `unplug`, `clock`, `circle`, `circle-dashed`,
`shield-alert`, `external-link`, `ethernet-port` and `arrow-left-right`;
`server`, `key-round`, `loader-circle`, `circle-x`, `copy` and `x` are in
kiln-ui.

## Notifications and agents

Notifications come from Runs and agents, and say what they need from you.

- **Toast:** lower right above the panel, 340 px, overlay surface with the
  toast shadow. An icon in the source's colour, a title, one line, at most two
  actions (agent toasts: a silver-blue Review changes and Later), and a
  dismiss. No coloured left borders.
- **Inbox:** opened from the bell at the right end of the status bar (a rust
  dot when something needs you). Two groups: Needs you (tinted rows, each with
  one action) and Earlier. A Focus mode holds non-urgent items until you are
  idle.
- **Operation in progress:** the status bar's left side, right after the Vim
  mode: spinner, label, a 72 px bar, a count, and +N for other background
  tasks. Clicking lists them all.

### Agent provenance

Everywhere it appears:

| Where | How agent work shows |
| --- | --- |
| Title bar | "2 agents working" chip |
| Rail | Robot icon with a silver-blue count |
| Editor | Dotted silver-blue change lane, a faint row tint, an inline "lane 2 · pending" chip |
| Problems, tests | "written by lane 2" and "new, lane 2" chips; a From agents scope |
| Terminal | The agent's own tab, silver-blue, read-only |
| Changes | Its own group, no checkboxes, Review link |
| Diff | "After · lane 2" header; comments become its next instruction |

Silver-blue is never used for anything a person did, and nothing an agent did
is shown in rust.

## Keyboard, accessibility and open questions

**Keys the design shows.** ␣ opens which-key; ␣p problems, ␣r runs, ␣tf rerun
failed, ␣v mark viewed, ]f next file in review, ␣\` new terminal, ␣1–9 switch
terminals, ␣, settings, ␣o quick outline, K hover, gd definition, / find, n N
next and previous.

### Accessibility rules

- Text 4.5:1 or better on every surface it sits on, including selected rows;
  checked per theme.
- Colours that must be told apart also differ in lightness, and every signal
  carries a glyph (+, −, ✓, ✕, ▲) so meaning never rests on hue alone.
- Real buttons, inputs, tabs, trees and switches with labels; icon-only buttons
  have names; the focus ring is 2 px rust, offset 1 px.
- A colour-blind mode for diffs and signals, offered next to the theme picker.

### Still to design

- Collapsed fold pill
- Command palette and the shared popup list
- Completion popup with agent ghost text
- Inline agent proposal in the editor
- Split editors with a VISUAL selection
- Three-way merge
- Welcome and empty window
- Move these tokens into the Tenmoku design system artifact, and render the
  canvas from tokens rather than generating Hakuji by colour mapping
