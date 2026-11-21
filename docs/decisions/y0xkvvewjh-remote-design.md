# Remote development in B3: the design calls (remote D0, T433)

The remote workflow (`docs/designs/remote-development.md`, § 10 "For later
design cards") lists the surfaces that need a drawing before the UI cards
start: T2055 (the pill), T2056 (the connect sheet), T2057 (the pending states)
and T2067 (the Ports tab). This card fits them to
`docs/design/design-system.md`, as the debugger's D0 did
(`docs/decisions/skgc7fe5z9-debugger-design.md`). The result is the design
system's Remote development section (revision 20). This file keeps the calls
that section makes, where they part from the design file and the record
(`docs/decisions/j8wvgszah0-remote-development.md`), and the script that checks
its colours. The canvas isn't in the repo: the section's Panels and screens
rows list the twelve artboards to draw, each in Yohen and Hakuji.

## Calls, and where they part from the design file

- **The pill's tone is its fill, edge and glyph; its name stays ink.** The
  design file says the pill's colour "follows the signal tokens". Shinsha on
  shinsha-soft is 5.48:1 in Yohen, under the 6:1 the section holds its text to,
  and kiseto on its hover is 4.18:1 in Hakuji. So no text is drawn in a signal
  colour: the signal is the soft fill, a 1 px edge and the glyph (3:1 or
  better everywhere), and the words are ink.
- **An edge, not just a fill.** In Yohen the soft grounds sit at the hover's
  lightness (OKLab L 0.26), so a filled pill and a hovered neutral one would
  differ in hue only; in Hakuji the soft grounds are 2.6 and 3.2 ΔE from the
  chrome, too close to read. The edge carries the tone: the neutral pill's is
  the control border, the others kiseto or shinsha, at least 0.31 OKLab L from
  it in both themes. That's the accessibility rule that colours told apart
  differ in lightness.
- **Warning and error are told apart by glyph, not lightness.** Kiseto and
  shinsha are 0.096 OKLab L apart in Yohen and 0.069 in Hakuji, under the
  debugger's 0.1. Their states never share a glyph: Reconnecting is the
  spinner, Needs sign-in a key, Disconnected `unplug`. The tooltip and the
  accessible name say the state in words.
- **Two hover roles.** A hovered signal pill needs a ground that keeps its
  tone; `state.hover` would turn it neutral. Each is its soft ground a tenth of
  the way to its signal, 4.3–6.4 ΔE from the resting fill. They fall back to
  the soft grounds, so a theme without them only loses the hover.
- **The underline is the Runs progress line.** Hare's fur on the control
  border, 2 px, inside the pill: B3's "a running item shows a thin rust
  progress line". Installing stays neutral: it's progress, not trouble.
- **The forward count only in Connected.** Reconnecting and Disconnected
  already say more than a count would. The forwards follow § 6: Reconnecting
  (and Needs sign-in, which is a paused reconnect) keeps their listeners and
  resumes them; a disconnect removes them, and a fresh session brings back
  only `remote.ports`' (T433 F2).
- **The pill doesn't follow the rail.** "Its place at both rail sides": the
  title bar is the window's, not a panel's, and nothing in it flips with
  `workbench.rail`. The pill stays first in the left group either way, and the
  artboard shows both so the UI card can test it.
- **Without a title bar the pill is the status bar's first item.** The design
  file places it only in the title bar, which Linux doesn't have, and v1 ships
  a Linux client. The agents chip already becomes a status item there, and
  the status bar is chrome, so the pill's colours hold. It goes before the Vim
  mode pill: it's the window's, the mode is the editor's.
- **Every menu item is a palette command.** Keyboard first: the title bar's
  buttons don't take focus, so Disconnect, Reconnect, Show Log, Show Ports,
  Forward a Port… and Close Remote Window are commands, and the menu teaches
  them.
- **Show Log is a read-only editor tab.** The connection log is long, often
  pasted into a bug report, and searched; a tab gives find and copy for free.
- **Buttons are kiln-ui's, with an opaque accent hover.** The sheet's
  default button is the primary button (`text.on_accent` on hare's fur, 6.59:1
  and 5.74:1), and the rest are secondary (ink, 11.74:1 or better). Kiln's
  fallback for `accent.hover` is the accent at 85% alpha, which over the
  overlay surface drops the label to 5.24:1 in Yohen and 4.30:1 in Hakuji,
  under both floors, so the bundled themes set it: hare's fur a fifth of the
  way to ink in Yohen (`#e39a68`, 7.74:1) and to black in Hakuji (`#863b12`,
  7.93:1). Only kiln-ui's primary button reads `accent.hover`, so the change
  touches nothing else, and it fixes every primary button, not only the
  sheet's. Review round 1 (T433 F1) found the sheet's button unspecified and
  the pairs missing from the script; both are now in it. Disabled buttons
  keep their colours at the disabled opacity and aren't held to a floor.
- **The sheet uses the floating surface's radius.** A sheet floats over the
  window like the palette, so `radius::FLOAT` and the floating shadow, not a
  prompt's. It's 520 px, the design file's width, against the prompt's 420.
- **"Not an ssh host" shows on Connect, not while typing.** A host is often
  half-typed; marking it wrong as you type is noise. Connect stays on once
  there's text, and the error says what's wrong.
- **§ 2.4's table has ten rows; nine are errors.** A refused handshake
  reinstalls without asking, so the first refusal is a Connecting line
  ("Reinstalling…"). Only a second refusal stops, as its own message with Back
  and Show Log. A changed host key gets `shield-alert` rather than `circle-x`:
  it may be an attack, and it's the one error with no way forward.
- **Show Log sits on the left; Back is offered on every error.** § 2.4 leaves
  Back off Verification failed. Every error step keeps a way back to Host, so
  the user can try another host without cancelling.
- **The sign-in prompt keeps the git prompt's frame,** at the top of the
  window, not inside the sheet: it can appear without the sheet (a reconnect,
  a git push on the host), and one prompt at a time across windows is already
  that modal's rule. The three kinds get titles that name the host and what's
  asked: "Sign in to neon", "Unknown host: neon", "Touch your security key for
  neon". The design file's OK and Cancel are added to the frame for the mouse.
- **An unknown host's Connect isn't the default.** Enter in a modal accepts by
  habit; accepting a host key should be a choice, so Enter does nothing until
  a button is focused.
- **The loading tab waits 150 ms before it shows anything.** A file under 1 MiB
  lands in one round trip (50.7 ms at 25 ms one way, record § 11), so most
  opens never draw a spinner. The 150 ms is the tree's and the folder list's.
- **Saving is a ring, pending a clock.** § 5's "ring" and § 7's "ring with a
  small clock" become Lucide `circle` and `clock` at 12 px, in the unsaved
  dot's colour, so the three states differ by shape: a filled dot, a ring, a
  ring with hands. Neither spins: a save in flight is usually a round trip.
- **Pending tree rows don't pretend.** The record says "nothing is
  optimistic", and the design file says "pending style". A renamed row keeps
  its old name with "renaming to lib.rs" after it until the host confirms,
  rather than showing the new name early. Only a new file, which has no old
  row, is drawn under its typed name.
- **The terminal overlay is the modal scrim with a card.** The scrim dims the
  output without hiding it, and the card is the overlay surface. On the scrim
  the card is only 1.25:1 in Yohen; the hairline and floating shadow edge it,
  as every floating surface is edged.
- **The Ports tab joins the rail's bottom group without a badge.** It's a
  bottom tool window like Terminal. The pill already carries the count, and a
  rail badge's colour means changes, agents or errors, none of which a forward
  is.
- **Dimmed is muted text and a dashed glyph.** A detected port that isn't
  forwarded is drawn in muted with `circle-dashed`; a forwarded one in ink with
  `arrow-left-right`. Lightness and shape both differ, so it isn't a tint
  alone.
- **Ports actions follow the row's state** (T433 F3). Only a forwarded row has
  a local address, so Copy Address is off on starting and detected rows, and
  Open in Browser on a detected row forwards first and opens what it gets, as
  the port toast's does (§ 6). A starting row can only be cancelled. Off
  buttons stay in place at the disabled opacity so the row doesn't shift, and
  their names say they're unavailable.
- **Field errors are the field's description and are announced once** (T433
  F4). The Host and Forward a Port inputs both mark themselves invalid while
  their message shows, carry it as their description, and announce it once
  per refused submission, so a screen reader hears why Enter did nothing.
- **Forward a Port… is an input row,** as Watch's Add watch row is in the
  Debug tool window, not a floating field in the header. It reuses a pattern
  and needs no width of its own.
- **Toasts use the existing levels and lifetime.** Lost is a warning,
  Reconnected a success, Disconnected an error (it stays, with Reconnect), the
  port toast and the AI notice are info. The design file's 4 s for Reconnected
  becomes the usual lifetime: one rule for every toast, and an identical toast
  counts up rather than stacking while a link flaps.
- **The close prompt is the app's prompt.** The record's "Keep pending
  changes?" and the design file's "2 files have changes that weren't saved to
  neon." become the design file's text, with a detail line that says where the
  changes go.
- **Welcome stays in Still to design.** The section settles only the remote
  rows and the Open Remote… action, which the Welcome page takes when it's
  drawn; its layout is still undesigned, so that item stays.
- **No faint text.** The section's text is ink or muted, so it clears 6:1 in
  Yohen everywhere it sits, including focused rows (muted, 6.09:1). Faint
  would be 4.51:1 there.
- **Sizes.** New kiln-ui roles: `height::PILL` (20), `width::SHEET` (520),
  `glyph::HAIRLINE` (1) and `glyph::ICON_SMALL` (12). Constants that move into
  `roles.rs` so the remote cards and their present owners share them:
  `width::PROGRESS_BAR` (72) and `height::PROGRESS_BAR` (4) from
  `operations_status.rs`, `width::PALETTE` (600) from `palette.rs`,
  `width::PROMPT` (420) from `prompt.rs`, `width::TOAST` (340) from `toast.rs`,
  `glyph::CLOSE_BOX` (16) from `tab_bar.rs` and `glyph::ROW_ICON` (14) from
  `recent_picker.rs`. New Lucide icons: `unplug`, `clock`, `circle`,
  `circle-dashed`, `shield-alert`, `external-link`, `ethernet-port` and
  `arrow-left-right`.

## For the UI cards

- **The accent hover** (`accent.hover` in `yohen.toml` and `hakuji.toml`)
  goes with T2056, the first card to draw a primary button on the sheet.

- **T2055 (pill)** adds the `[remote]` theme keys with their fallbacks, the
  pill in the title bar and the status bar, its menu and the palette
  commands, and the announcements for state changes.
- **T2056 (sheet)** adds the sheet, the sign-in prompt's three kinds (and the
  OK and Cancel buttons on the shared frame), and the close prompt.
- **T2057 (pending)** adds the tab states (kiln-ui's `Tab` gains loading,
  saving, save pending and not loaded, with their names in `kiln-ui.ftl`), the
  tree rows, the Quick Open footer, the terminal overlay and the operations.
- **T2067 (Ports)** adds the tool window, its rail item and the port toast.

## The colour check

Run from the repo root with `python3 -I`. It reads every existing role from the
bundled theme files (a translucent one over the editor's ground), resolves the
two the bundled themes leave to kiln's fallbacks (`state.dirty` and
`tab.active`), takes `accent.hover` from its new value, and checks each text the section draws at the theme's floor
(6:1 Yohen, 4.5:1 Hakuji) on every ground it sits on, and each glyph and edge
at 3:1. It prints the pill edges' OKLab lightness, the pill grounds' distances
from the chrome and from each other, the terminal card on the scrim, and each
new role's OKLab distance (×100) to its nearest role. Once the UI cards add the
`[remote]` keys and `accent.hover`, the script skips them when it lists the
existing roles.

```python
"""Remote development's colours (docs/design/design-system.md, Remote
development): checks every text, glyph and edge the section draws against the
design's floors, and measures the new roles' OKLab distances. Run from the
repo root: python3 -I remote_roles.py"""

import tomllib

# Text floors: 6:1 in Yohen, 4.5:1 in Hakuji, for every text the section
# draws (it uses ink and muted only, never faint). Glyphs and edges 3:1.
THEMES = {
    "Yohen": ("crates/fe-config/themes/yohen.toml", 6.0),
    "Hakuji": ("crates/fe-config/themes/hakuji.toml", 4.5),
}
GLYPH_FLOOR = 3.0

# The new roles: the warning and error pills' hover grounds, each its soft
# ground a tenth of the way to its signal; and an opaque value for the
# existing `accent.hover` (a primary button hovered or pressed), hare's fur a
# fifth of the way to ink in Yohen and to black in Hakuji, where kiln's
# fallback (the accent at 85% alpha) drops the label under the floor.
NEW = {
    "Yohen": {
        "remote.pill_warning_hover": "#3f3216",
        "remote.pill_error_hover": "#482524",
        "accent.hover": "#e39a68",
    },
    "Hakuji": {
        "remote.pill_warning_hover": "#e5e2ca",
        "remote.pill_error_hover": "#f6d2cd",
        "accent.hover": "#863b12",
    },
}

# Roles the bundled themes leave to kiln's fallbacks (kiln-core tokens.rs).
FALLBACK = {"state.dirty": "status.warning", "tab.active": "surface.background"}

# Where the section's text sits: (what, its roles, the grounds it sits on).
TEXT = [
    ("pill label (ink)", ["text.primary"],
     ["surface.chrome", "state.hover", "status.warning_soft", "remote.pill_warning_hover",
      "status.error_soft", "remote.pill_error_hover"]),
    ("primary buttons' label (on accent): Connect, Open, Retry, OK, the prompts' defaults",
     ["text.on_accent"], ["accent.base", "accent.hover"]),
    ("secondary and ghost buttons' label (ink): Cancel, Back, Show Log, empty-state buttons",
     ["text.primary"], ["surface.raised", "surface.panel", "surface.background", "state.hover",
                         "state.pressed"]),
    ("pill forward count (muted)", ["text.muted"], ["surface.chrome", "state.hover"]),
    ("pill menu, sheet, completion and folder rows, Quick Open footer, overlays, toasts, recents",
     ["text.primary", "text.muted"], ["surface.raised", "state.hover", "state.selected"]),
    ("tabs: loading and not-loaded labels (muted)", ["text.muted"],
     ["tab.inactive", "tab.active", "state.hover"]),
    ("editor-area states, the terminal's lost-output line (muted)", ["text.muted"],
     ["surface.background"]),
    ("tree pending rows, Ports rows and dimmed detected rows", ["text.primary", "text.muted"],
     ["surface.panel", "state.hover", "state.selected_inactive", "state.selected",
      "state.pressed"]),
]

# Glyphs and edges that carry meaning: (what, role, grounds).
GLYPHS = [
    ("neutral pill glyph and spinner (muted)", "text.muted", ["surface.chrome", "state.hover"]),
    ("warning pill glyph and edge (kiseto)", "status.warning",
     ["surface.chrome", "status.warning_soft", "remote.pill_warning_hover"]),
    ("error pill glyph and edge (shinsha)", "status.error",
     ["surface.chrome", "status.error_soft", "remote.pill_error_hover"]),
    ("primary button on the sheet, at rest (hare's fur)", "accent.base",
     ["surface.raised"]),
    ("hovered primary button on the sheet", "accent.hover", ["surface.raised"]),
    ("install underline (hare's fur)", "accent.base",
     ["surface.chrome", "state.hover", "border.control"]),
    ("saving ring and save-pending clock (dirty)", "state.dirty",
     ["tab.inactive", "tab.active", "state.hover"]),
    ("sheet error icon, field error edge (shinsha)", "status.error", ["surface.raised"]),
    ("toast icons: info", "status.info", ["surface.raised"]),
    ("toast icons: warning", "status.warning", ["surface.raised"]),
    ("toast icons: error", "status.error", ["surface.raised"]),
    ("toast icons: success", "status.success", ["surface.raised"]),
]


def rgb(hex_):
    hex_ = hex_.lstrip("#")
    alpha = int(hex_[6:8], 16) / 255 if len(hex_) == 8 else 1.0
    return tuple(int(hex_[i : i + 2], 16) / 255 for i in (0, 2, 4)), alpha


def over(top, ground):
    (t, a), (g, _) = rgb(top), rgb(ground)
    return "#" + "".join(f"{round((a * x + (1 - a) * y) * 255):02x}" for x, y in zip(t, g))


def linear(c):
    return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4


def luminance(hex_):
    r, g, b = (linear(c) for c in rgb(hex_)[0])
    return 0.2126 * r + 0.7152 * g + 0.0722 * b


def contrast(a, b):
    high, low = sorted((luminance(a), luminance(b)), reverse=True)
    return (high + 0.05) / (low + 0.05)


def oklab(hex_):
    r, g, b = (linear(c) for c in rgb(hex_)[0])
    l = 0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b
    m = 0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b
    s = 0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b
    l, m, s = (x ** (1 / 3) for x in (l, m, s))
    return (
        0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s,
        1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s,
        0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s,
    )


def delta_e(a, b):
    """OKLab ΔE ×100, as the design quotes it."""
    return 100 * sum((x - y) ** 2 for x, y in zip(oklab(a), oklab(b))) ** 0.5


def roles(path):
    """Every colour role in a theme file but the remote ones, by `table.key`,
    a translucent one over the editor's ground, as it's seen."""
    with open(path, "rb") as file:
        theme = tomllib.load(file)
    ground = theme["editor"]["background"]
    out = {}
    for table, keys in theme.items():
        for key, value in keys.items() if isinstance(keys, dict) else ():
            if table == "remote" or f"{table}.{key}" in NEW["Yohen"] or not isinstance(value, str):
                continue
            if value.startswith("#") and len(value) in (7, 9):
                out[f"{table}.{key}"] = over(value, ground) if len(value) == 9 else value
    return theme, out


def verdict(ok):
    results.append(ok)
    return "ok" if ok else "FAIL"


results = []
for name, (path, floor) in THEMES.items():
    theme, existing = roles(path)
    colours = dict(existing, **NEW[name])
    for role, fallback in FALLBACK.items():
        colours.setdefault(role, colours[fallback])
    print(f"== {name}: text {floor}:1, glyphs and edges {GLYPH_FLOOR}:1")
    for what, texts, grounds in TEXT:
        pairs = [(text, ground) for text in texts for ground in grounds]
        low = min(pairs, key=lambda pair: contrast(colours[pair[0]], colours[pair[1]]))
        ratio = contrast(colours[low[0]], colours[low[1]])
        print(f"{what}: lowest {low[0]} on {low[1]} {ratio:.2f}:1", verdict(ratio >= floor))
    for what, role, grounds in GLYPHS:
        lit = {ground: contrast(colours[role], colours[ground]) for ground in grounds}
        low = min(lit, key=lit.get)
        print(f"{what} {colours[role]}: " + ", ".join(f"{g} {r:.2f}" for g, r in lit.items()))
        print(f"  lowest {low} {lit[low]:.2f}:1", verdict(lit[low] >= GLYPH_FLOOR))
    # The pill's edge tells neutral from the two signal tones by lightness.
    edges = {role: oklab(colours[role])[0] for role in ("border.control", "status.warning", "status.error")}
    print("pill edges OKLab L: " + ", ".join(f"{role} {l:.3f}" for role, l in edges.items()))
    for tone in ("status.warning", "status.error"):
        apart = abs(edges["border.control"] - edges[tone])
        print(f"  neutral and {tone} {apart:.3f} apart", verdict(apart >= 0.1))
    apart = abs(edges["status.warning"] - edges["status.error"])
    print(f"  warning and error {apart:.3f} apart (their glyphs differ: spinner and key, unplug)")
    # The pill's grounds, at rest and hovered.
    chrome = colours["surface.chrome"]
    for ground in ("state.hover", "status.warning_soft", "remote.pill_warning_hover",
                   "status.error_soft", "remote.pill_error_hover"):
        print(f"{ground} {colours[ground]}: chrome {delta_e(colours[ground], chrome):.1f},"
              f" OKLab L {oklab(colours[ground])[0]:.3f}")
    for rest, hover in (("status.warning_soft", "remote.pill_warning_hover"),
                        ("status.error_soft", "remote.pill_error_hover")):
        print(f"  {hover} from {rest}: ΔE {delta_e(colours[hover], colours[rest]):.1f}")
    # The terminal's overlay card, over the scrimmed terminal.
    scrim = over(theme["surface"]["overlay"], theme["surface"]["background"])
    card = colours["surface.raised"]
    print(f"terminal overlay card {card} on the scrim {scrim}: {contrast(card, scrim):.2f}:1,"
          f" ΔE {delta_e(card, scrim):.1f} (the hairline and shadow edge it)")
    for role, colour in NEW[name].items():
        others = dict(existing)
        others.update({other: value for other, value in NEW[name].items() if other != role})
        near = min(others, key=lambda other: delta_e(colour, others[other]))
        print(f"{role} {colour}: nearest {near} {others[near]}, ΔE {delta_e(colour, others[near]):.1f}")
print("all pass" if all(results) else "SOME FAIL")
```

Its output on the themes at this commit:

```text
== Yohen: text 6.0:1, glyphs and edges 3.0:1
pill label (ink): lowest text.primary on remote.pill_warning_hover 9.93:1 ok
primary buttons' label (on accent): Connect, Open, Retry, OK, the prompts' defaults: lowest text.on_accent on accent.base 6.59:1 ok
secondary and ghost buttons' label (ink): Cancel, Back, Show Log, empty-state buttons: lowest text.primary on state.pressed 11.74:1 ok
pill forward count (muted): lowest text.muted on state.hover 6.80:1 ok
pill menu, sheet, completion and folder rows, Quick Open footer, overlays, toasts, recents: lowest text.muted on state.selected 6.09:1 ok
tabs: loading and not-loaded labels (muted): lowest text.muted on state.hover 6.80:1 ok
editor-area states, the terminal's lost-output line (muted): lowest text.muted on surface.background 7.82:1 ok
tree pending rows, Ports rows and dimmed detected rows: lowest text.muted on state.selected 6.09:1 ok
neutral pill glyph and spinner (muted) #b8a99a: surface.chrome 8.40, state.hover 6.80
  lowest state.hover 6.80:1 ok
warning pill glyph and edge (kiseto) #e8b440: surface.chrome 10.09, status.warning_soft 8.14, remote.pill_warning_hover 6.57
  lowest remote.pill_warning_hover 6.57:1 ok
error pill glyph and edge (shinsha) #f2716f: surface.chrome 6.74, status.error_soft 5.48, remote.pill_error_hover 4.71
  lowest remote.pill_error_hover 4.71:1 ok
primary button on the sheet, at rest (hare's fur) #e0874c: surface.raised 5.81
  lowest surface.raised 5.81:1 ok
hovered primary button on the sheet #e39a68: surface.raised 6.82
  lowest surface.raised 6.82:1 ok
install underline (hare's fur) #e0874c: surface.chrome 7.08, state.hover 5.74, border.control 4.00
  lowest border.control 4.00:1 ok
saving ring and save-pending clock (dirty) #e8b440: tab.inactive 10.09, tab.active 9.40, state.hover 8.18
  lowest state.hover 8.18:1 ok
sheet error icon, field error edge (shinsha) #f2716f: surface.raised 5.53
  lowest surface.raised 5.53:1 ok
toast icons: info #4f95f5: surface.raised 5.21
  lowest surface.raised 5.21:1 ok
toast icons: warning #e8b440: surface.raised 8.28
  lowest surface.raised 8.28:1 ok
toast icons: error #f2716f: surface.raised 5.53
  lowest surface.raised 5.53:1 ok
toast icons: success #6fd49a: surface.raised 8.67
  lowest surface.raised 8.67:1 ok
pill edges OKLab L: border.control 0.362, status.warning 0.797, status.error 0.702
  neutral and status.warning 0.435 apart ok
  neutral and status.error 0.340 apart ok
  warning and error 0.096 apart (their glyphs differ: spinner and key, unplug)
state.hover #2b221d: chrome 9.3, OKLab L 0.261
status.warning_soft #2c2311: chrome 9.7, OKLab L 0.262
remote.pill_warning_hover #3f3216: chrome 16.2, OKLab L 0.325
status.error_soft #351d1c: chrome 10.0, OKLab L 0.263
remote.pill_error_hover #482524: chrome 15.1, OKLab L 0.311
  remote.pill_warning_hover from status.warning_soft: ΔE 6.4
  remote.pill_error_hover from status.error_soft: ΔE 5.1
terminal overlay card #2a211c on the scrim #0e0b09: 1.25:1, ΔE 10.4 (the hairline and shadow edge it)
remote.pill_warning_hover #3f3216: nearest editor.vim_yank_highlight #472f20, ΔE 2.6
remote.pill_error_hover #482524: nearest editor.diff_removed_word #432624, ΔE 1.1
accent.hover #e39a68: nearest file_icon.rust #d0956b, ΔE 3.5
== Hakuji: text 4.5:1, glyphs and edges 3.0:1
pill label (ink): lowest text.primary on remote.pill_error_hover 11.30:1 ok
primary buttons' label (on accent): Connect, Open, Retry, OK, the prompts' defaults: lowest text.on_accent on accent.base 5.74:1 ok
secondary and ghost buttons' label (ink): Cancel, Back, Show Log, empty-state buttons: lowest text.primary on state.pressed 13.12:1 ok
pill forward count (muted): lowest text.muted on state.hover 6.69:1 ok
pill menu, sheet, completion and folder rows, Quick Open footer, overlays, toasts, recents: lowest text.muted on state.selected 6.60:1 ok
tabs: loading and not-loaded labels (muted): lowest text.muted on state.hover 6.69:1 ok
editor-area states, the terminal's lost-output line (muted): lowest text.muted on surface.background 7.90:1 ok
tree pending rows, Ports rows and dimmed detected rows: lowest text.muted on state.selected_inactive 6.56:1 ok
neutral pill glyph and spinner (muted) #4b525b: surface.chrome 7.07, state.hover 6.69
  lowest state.hover 6.69:1 ok
warning pill glyph and edge (kiseto) #776a00: surface.chrome 4.88, status.warning_soft 4.72, remote.pill_warning_hover 4.18
  lowest remote.pill_warning_hover 4.18:1 ok
error pill glyph and edge (shinsha) #a1100e: surface.chrome 7.24, status.error_soft 6.85, remote.pill_error_hover 5.79
  lowest remote.pill_error_hover 5.79:1 ok
primary button on the sheet, at rest (hare's fur) #a84a16: surface.raised 5.74
  lowest surface.raised 5.74:1 ok
hovered primary button on the sheet #863b12: surface.raised 7.93
  lowest surface.raised 7.93:1 ok
install underline (hare's fur) #a84a16: surface.chrome 5.14, state.hover 4.86, border.control 3.49
  lowest border.control 3.49:1 ok
saving ring and save-pending clock (dirty) #776a00: tab.inactive 4.88, tab.active 5.46, state.hover 4.62
  lowest state.hover 4.62:1 ok
sheet error icon, field error edge (shinsha) #a1100e: surface.raised 8.09
  lowest surface.raised 8.09:1 ok
toast icons: info #1f68cf: surface.raised 5.33
  lowest surface.raised 5.33:1 ok
toast icons: warning #776a00: surface.raised 5.46
  lowest surface.raised 5.46:1 ok
toast icons: error #a1100e: surface.raised 8.09
  lowest surface.raised 8.09:1 ok
toast icons: success #17773f: surface.raised 5.60
  lowest surface.raised 5.60:1 ok
pill edges OKLab L: border.control 0.838, status.warning 0.520, status.error 0.452
  neutral and status.warning 0.317 apart ok
  neutral and status.error 0.386 apart ok
  warning and error 0.069 apart (their glyphs differ: spinner and key, unplug)
state.hover #ebecef: chrome 1.9, OKLab L 0.943
status.warning_soft #f1efe0: chrome 2.6, OKLab L 0.950
remote.pill_warning_hover #e5e2ca: chrome 6.3, OKLab L 0.909
status.error_soft #ffe7e2: chrome 3.2, OKLab L 0.946
remote.pill_error_hover #f6d2cd: chrome 8.1, OKLab L 0.893
  remote.pill_warning_hover from status.warning_soft: ΔE 4.3
  remote.pill_error_hover from status.error_soft: ΔE 5.5
terminal overlay card #ffffff on the scrim #b2b2b2: 2.12:1, ΔE 23.6 (the hairline and shadow edge it)
remote.pill_warning_hover #e5e2ca: nearest editor.selection #eedbd0, ΔE 2.5
remote.pill_error_hover #f6d2cd: nearest editor.vim_yank_highlight #ecd7cc, ΔE 1.9
accent.hover #863b12: nearest file_icon.rust #834d32, ΔE 4.4
all pass
```

Reading the distances: the accent hover's nearest role is the Rust file icon's
muted kaki, which is hare's fur moved toward the muted text; a button and a tree
icon never sit together. The two pill hover roles' nearest roles are editor
overlays (the yank flash, a selection, a removed word in a diff), which never sit in the
title bar or the status bar beside a pill; Yohen's error hover is 1.1 from the
removed word because both are shinsha over a dark ground, which is the point.
Hovered, the signal pills move 4.3–6.4 ΔE from their resting fill, more than
the neutral pill's hover moves in Hakuji (1.9 from the chrome). Warning and
error sit under 0.1 OKLab L apart, which the script prints but doesn't fail:
their glyphs and words differ, and no state is told from another by hue alone.
