# Custom viewers from plugins: file-type claims, a pixel surface, install on open, and a first-party PDF plugin

T427 (Mac T1956). A design card: this document is the only change. It answers
the user's question:

> "Can we package a pdf viewer as a plugin that we prompt them to download when
> opening a pdf?"

and their steer after the research: "the PDF viewer is big enough and niche
enough that it makes more sense to package as a plugin rather than as part of
the core and make it more bloated."

**Status: proposed.** Approver: **the user (Daryl Robbins)**. Nothing here is
built, and **no implementation card is filed by this card**. The cards in §10
are drafted for the user to approve, and are filed only after that.

It extends the approved stage-3 record
[dqsjfernkt-plugins-stage-3.md](dqsjfernkt-plugins-stage-3.md) (T1652): its
tiers (§3.1–3.2), panels (§4.2), signing and the registry (§4.6) and the
core-vs-plugin line (§5). That file isn't edited. Where this one adds to it,
the section says so.

Short answers, argued below:

- **Yes.** A plugin claims file types in its manifest (§2). A file it claims
  opens in a host-owned page surface that the plugin fills with rendered tiles
  (§3). The host owns scrolling, zoom, selection, copy, links and Find (§4).
- **Install on open** generalises the grammar prompt. A PDF opens in a native
  notice tab, and the notice offers the plugin from the signed index, with its
  size, before anything is downloaded (§5, §6, §8).
- **The PDF plugin** is first-party, built on **hayro** (pure Rust, Apache-2.0
  or MIT), pending a measuring spike. PDFium is the fallback candidate. MuPDF is
  ruled out by its licence and pdf.js by being JavaScript (§7).
- **Core grows by zero bytes of renderer.** Card A, the native "This is a PDF"
  notice, is first. Everything else builds on it (§8, §10).
- **T1705 (the panels model) doesn't need to land first.** The page surface is
  a new surface, not a panels node. Two cards share files with T1705 and queue
  behind it on scope, not on design (§10).

## 1. What was read and surveyed, and the non-goals

**Non-goals, stated first:**

- **PDF rendering ships only as a plugin, and never in core.** A renderer is
  big: PDFium's published browser builds are 4.4–4.5 MB of wasm, and the
  pure-Rust candidate's size is what the spike measures (§7). Most users open a PDF rarely, and core must stay small.
  So no renderer crate enters the app's dependency graph, behind a feature
  flag or otherwise. The PDF plugin card (V9) adds `deny.toml` bans to keep it
  that way.
- **Images stay native in core.** The image viewer (`fe-gpui/src/image_viewer.rs`,
  `loader::is_image`: png, jpg, jpeg, gif, webp, bmp, ico, svg, svgz) is
  unchanged. It isn't moved to a plugin, and a plugin can't claim those
  extensions (§2). No existing viewer moves out: not the hex viewer, the large
  file viewer or the Markdown preview.
- **No webviews**, and no HTML or script surface. The stage-3 record's rule
  stands (its §7). The page surface takes pixels and geometry only.
- **View only.** Filling forms, editing annotations and saving a PDF are out
  of scope (Q3).

**Read in this repository, at the base (620e533):**

- The stage-3 record, all of it, and the accepted record
  [vmtnmkay62-extensions-and-plugins.md](vmtnmkay62-extensions-and-plugins.md)
  §6 (licence classes) and §7 (capabilities and prompts).
- The grammar prompt: `crates/fe-workbench/src/grammar_prompt.rs`,
  [pp118n3cn5-grammar-prompt.md](pp118n3cn5-grammar-prompt.md) (T426) and
  [mmjrn0hj4f-grammar-prompt-packs.md](mmjrn0hj4f-grammar-prompt-packs.md)
  (T1485).
- The loader, `crates/fe-workbench/src/loader.rs`: `Opened` has `Text`,
  `Large`, `Image` and `Binary`. There is no PDF case. Today a PDF opens in the
  hex viewer if its first 8 KiB hold a NUL (`is_binary`), and in a text editor
  if they don't. Which one depends on how the file was compressed.
- The hex viewer (`fe-gpui/src/hex_viewer.rs`, its `pane_item.rs`,
  [0cp67sc6m3-hex-viewer.md](0cp67sc6m3-hex-viewer.md)) and its
  `hex.open_as_hex` command (`workbench_commands.rs:1981`, context
  `editor || ImageViewer`). It is the model for in-place swaps and for tab
  restore by `PaneItem::KIND` (`fastedit.hex_viewer`, `fastedit.image_viewer`,
  `kiln_shell::layout::register_kind`).
- The image viewer's zoom: `STEPS`, `PinchEvent`, and `SETTLE` (150 ms before
  re-rendering an SVG at a new scale).
- "Open in Default App" doesn't exist as a command. GPUI has
  `App::open_with_system(path)` (gpui-pre 0.3.9, `app.rs:1918`), beside the
  `reveal_path` the project panel's Reveal uses.
- Links: `crates/fe-workbench/src/lsp_links.rs` sends `http`, `https` and
  `mailto` to the system's opener and refuses any other scheme with a toast.
- The plugin host as it stands: `kiln-plugin/src/limits.rs` (the interactive
  constants, and B1's background ones), `kiln-command/src/capability.rs` (K1's
  `jobs`, `language`, `panels`, `tasks` and `lsp:<server>` have landed;
  `FsScope` has only `Workspace`), `kiln-plugin/src/compat.rs` (`HOST_API` is
  1.1, so C1 has landed), `fe-workbench/src/extension_install.rs` (staged,
  link-free unpacking), `plugin_prompt.rs` (the grant prompt).
- The registry: `crates/fe-registry/src/grammars.rs` and
  `extensions/grammars/registry.toml`. One fact here matters for §6:
  `Index` is `#[serde(deny_unknown_fields)]`, and `Index::parse` refuses any
  `schema` other than 1 (`grammars.rs:89`, `:177`). So **an app shipped today
  refuses an `index.json` that gains a `plugins` key.**
- The test guests (`kiln/crates/kiln-plugin/tests/guests/fs`,
  `crates/fe-plugin/tests/guests/editor`): each is its own Cargo project with
  a lock file, a `build.zsh` and a prebuilt `.wasm`, outside the workspace
  (`members = ["crates/*", "kiln/crates/*"]`).

**Read outside it, for §7 only** (licences and stated features; no code was
copied or read for design):

- hayro's repository README and its crates' READMEs (`hayro`, `hayro-interpret`,
  `hayro-syntax`), and its docs.rs page (0.9.0, 2026-11-02) for the version,
  licence and dependency list. docs.rs's source listing of
  `hayro_syntax::crypto` for which ciphers exist.
- PDFium's `LICENSE` file at googlesource (BSD-3-Clause, with the Apache-2.0
  text).
- pdf.js's repository README (Apache-2.0, JavaScript).
- MuPDF's licensing, from its documentation and Wikipedia (AGPL, or a
  commercial licence from Artifex).
- pdf-rs's repository README (MIT; `pdf_render` and `pdf_text`).
- Third-party pages giving PDFium's emscripten `.wasm` size (4.4–4.5 MB on
  disk, 1.5–2.7 MB compressed). These are other people's builds, not a WASI
  component, and are cited as an order of magnitude only.

**Measured:** nothing. No toolchain for `wasm32-wasip2` is installed on this
host, and building and timing a renderer is the spike's job (V0, §7). Every
speed and size of a plugin build in this document is marked **not measured**.
Where a number is the stage-3 record's (measured on its M3 Max), it says so.

**Decision:** the non-goals above. PDF rendering is a plugin only, images stay
native, there are no webviews, and the plugin is view-only.

## 2. Claiming file types

**The contribution.** A plugin declares the views it provides:

```toml
[contributes.plugin]
wasm = "plugin.wasm"
capabilities = ["viewers", "clipboard"]

[contributes.plugin.background]   # the session's memory (§3); optional
memory = "512MiB"

[[contributes.plugin.viewers]]
id = "pdf"                 # the viewer is "<plugin id>.pdf"
title = "PDF"              # shown in Open With… and in the tab's tooltip
icon = "file-text"         # a name in the app's icon set, as panels use
extensions = ["pdf"]       # lower case, no dot; matched in any case
magic = "%PDF-"            # optional: bytes that identify the type, as text or "0x25504446"
within = 1024              # optional: magic may start anywhere in the first 1 to 1024 bytes
# offset = 0               # optional, instead of `within`: magic starts exactly here (the default is offset 0)
priority = 0               # optional: -100 to 100; higher wins a tie between plugins
```

- **Validation.** It happens at manifest load, as for panels. An `id` is 1 to
  32 characters of `a-z`, `0-9` and `-`. There are at most 8 viewers per plugin
  and 32 extensions per viewer, and a claim needs at least one extension or a
  `magic`. `magic` is at most 16 bytes. A claim gives `within` or `offset`, not
  both, and the magic must end within the first 1024 bytes. An invalid entry
  is dropped with a manifest warning, and the rest of the manifest loads.
- **One matching contract.** A file **matches** a claim if its extension is
  one of the claim's `extensions` (in any case), **or** the claim has a `magic`
  and the file's first bytes hold it where the claim says. Either is enough.
  So a PDF named `paper.bin`, `PAPER.PDF` or `scan` (no extension) matches
  `tenmoku.pdf`, and so does a `.pdf` whose header is damaged, which the
  viewer then reports as malformed (§8). This one function, `claim_matches`,
  is used everywhere a claim is matched:
  - installed viewers (step 3 below);
  - the signed index's offers (step 4, §5);
  - the built-in PDF claim Card A ships (`extensions = ["pdf"]`,
    `magic = "%PDF-"`, `within = 1024`);
  - Open With….

  It runs on the 8 KiB probe `is_binary` already reads, so it costs no second
  read.
- **Claims core keeps.** An extension `loader::is_image` claims (png, jpg,
  jpeg, gif, webp, bmp, ico, svg, svgz) is refused with a warning: images stay
  native. A file of one of those extensions never reaches a plugin claim, even
  by `magic`. A claim never takes a file core would open in an editor either.
  A file the loader reads as text (no NUL in its first 8 KiB, or a BOM)
  matches a claim **only by its `magic`**, never by extension alone. Without
  the rule, a plugin claiming `md` could take every Markdown file. `magic` is
  what makes a claim on a text-like file deliberate.
- **Core's document extensions bypass the text rule.** Core keeps one fixed
  list of extensions that name a non-text document format whatever their
  bytes look like: **`pdf`** only, in Card A. A file whose extension is on
  that list matches by extension alone **even when it reads as text**. A PDF
  can be all ASCII, and a `.pdf` with a damaged or missing `%PDF-` header has
  neither a NUL nor the magic, so without this it would fall to the editor.
  With it, that file matches the installed viewer, the built-in PDF claim and
  the index's offers alike, because all three use `claim_matches`. Plugins
  can't add to the list; widening it is a core change. A text-reading file
  with any other extension still needs the magic.
- **Capability.** `viewers` (new, card V1). Without it the contribution is
  ignored with a warning.

**How the host resolves a file to a view.** In `loader::open`, in this order:

1. **An explicit choice.** A tab reopened through Open With… uses the view
   picked. So does a type the user set a default for (`files.viewers`, below).
2. **Core viewers that claim by type.** The image viewer, by extension, as
   today.
3. **Installed, enabled plugins' claims** that `claim_matches`. A magic match
   ranks above an extension-only match, since the bytes say more than the
   name. Remaining ties go to the higher `priority`, then to the plugin id in
   byte order, so the result is stable.
4. **The built-in PDF claim, with no plugin.** A file matching it opens as
   `Opened::Pdf`: Card A's native notice (§8), with the install offer of §5 if
   the signed index has a claim that matches it. This step is always there,
   with or without an index.
5. **What core does today.** Binary goes to the hex viewer, a large file to the
   read-only viewer, and anything else to the editor. If the cached signed
   index has a claim matching the file (by the same contract, the text and
   image rules included), the tab opens as it does today and §5's offer shows
   as a toast over it. A format the index learns about never changes where its
   files open until the user installs a viewer.

The resolver runs on the background executor, as `open` does now. Matching is
a hash of the lower-cased extension plus a byte compare per claim with a
`magic`, over the probe already read.

**Open With… and the escape hatches.** One new command, `workbench.open_with`
("Open With…"), lists the current file's choices:

- every installed viewer whose claim `claim_matches` the file, best first in
  step 3's order;
- then **Other Viewers…**, every other installed viewer, for a file whose name
  and bytes both fail to say what it is;
- then Text, Hex and Default App.

It is in the tab's context menu and the palette, for an editor, the image
viewer, the hex viewer, the PDF notice and a plugin view. Picking one swaps the
tab in place, as Open as Hex does today, and like it refuses an editor with
unsaved changes ("Save or revert first").

Its footer, "Always open .pdf with this", writes `files.viewers.pdf =
"<choice>"` to user settings. It shows only for a file with an extension. The
choice is a viewer id (`tenmoku.pdf.pdf`), `"text"`, `"hex"` or
`"default-app"`.

- `hex.open_as_hex` keeps its id and gains the notice's and the plugin view's
  key contexts.
- `workbench.open_in_default_app` ("Open in Default App", new in Card A) calls
  `cx.open_with_system(path)`. It is in the same contexts, and is a button on
  the notice.
- Neither escape hatch ever needs the plugin. A plugin view that fails still
  has both (§8).

**Session restore.** A plugin view is a `PaneItem` of kind
`fastedit.plugin_viewer`. Its saved state is
`{path, viewer: "<plugin>.<id>", page, zoom, offset}`.

- If the viewer is installed and enabled, the tab restores into it at that
  page and zoom.
- If it isn't (uninstalled, disabled, failed to load, or a different machine),
  the tab restores as the notice, keeping the saved state. The notice says why,
  and the tab swaps back once the viewer is present.
- **A restore never prompts.** Restored tabs don't trigger the install offer
  until the user shows them. This follows the existing rule that launch offers
  wait for `landed && !restoring` (T420 F1).

**Decision:** `[[contributes.plugin.viewers]]` as above, matched by one
contract (extension **or** magic) for installed viewers, index offers, the
built-in PDF claim and Open With…. It is resolved after an explicit choice
and the core image claims, and before core's binary/large/text fallbacks.
Text files are taken only by a `magic` match, except an extension on core's
document list (`pdf`), so an ASCII `.pdf` with a damaged header still
reaches the PDF claims. A type only the index knows
keeps opening as today, with a toast offer. There is one Open With… command
and a `files.viewers` default per extension. Restores fall back to the
notice, and restores don't prompt.

## 3. The pixel surface

**Why panels don't fit.** The panels model (stage 3 §4.2) is a declarative tree
of text, controls and lists, with no colours and no pixels. A page is pixels at
an arbitrary scale. A `bitmap` node in that tree would mean the plugin sending
whole-page images through `set-view` (1 MiB of text per view, 20 updates a
second) and the host knowing nothing about zoom, scroll or which part shows.
So the page view is **a new surface, owned by the host**, which asks the
plugin for exactly the pixels it needs.

**The shape.** The host lays out pages, scrolls, zooms and caches. The plugin
answers three questions: what the document is, the pixels of a rectangle of a
page at a scale, and a page's text and links.

```wit
// fastedit:editor@1.1.0 (preview): the host side, imported by a viewer session
interface viewer-document {
    use kiln:extension/types@1.1.0.{error};
    /// The opened file, and only it. Ranged reads, so a 2 GB PDF is never copied whole.
    resource document {
        len: func() -> u64;
        /// At most 16 MiB from `offset`; fewer at the end.
        read: func(offset: u64, len: u32) -> result<list<u8>, error>;
        /// The file's name, such as `paper.pdf`: never its path.
        name: func() -> string;
        /// True once the host has dropped the request this call serves (§3, cancellation).
        cancelled: func() -> bool;
    }
}

// Exported by the plugin; runs in the viewer session's store.
interface viewer-session {
    use viewer-document.{document};
    /// Page geometry in points (1/72 in), after the page's rotation.
    record page-size { width: f32, height: f32 }
    record document-info { pages: list<page-size>, title: option<string> }
    variant open-error { needs-password, wrong-password, unsupported(string), malformed(string) }
    /// Device pixels within a page rendered at `scale` (pixels per point).
    record rect { x: f32, y: f32, width: f32, height: f32 }
    /// Premultiplied RGBA8, row-major, `width * height * 4` bytes.
    record tile { width: u32, height: u32, rgba: list<u8> }
    variant link-target { uri(string), page(tuple<u32, f32, f32>) }  // page, x, y in points
    record link { area: rect, target: link-target }                  // in points
    /// `text` in reading order; `boxes[i]` is char `i`'s box in points (empty for a space it invented).
    record page-layout { text: string, boxes: list<rect>, links: list<link> }
    record outline-item { title: string, page: u32, depth: u32 }

    open: func(doc: document, password: option<string>) -> result<document-info, open-error>;
    /// One page, several tiles at one scale: a plugin may render their union once.
    render: func(page: u32, scale: f32, tiles: list<rect>) -> result<list<tile>, error>;
    layout: func(page: u32) -> result<page-layout, error>;
    outline: func() -> result<list<outline-item>, error>;
}
```

The spec sketched a push shape (`viewer.set-document` metadata, then
`render-tile(page, rect, scale)`). This record makes it pull-only. `open`
returns the metadata, so the plugin has nothing to push, and every byte that
crosses is one the host asked for and can account for. `render` takes a list
of tiles because a PDF page's content stream is interpreted whole. Rendering
twelve tiles of one page as twelve calls would interpret it twelve times. With
one call, the plugin can render the union once and cut it.

**Which tier renders: a third shape, the viewer session.** Neither existing
tier fits:

- **The interactive tier is too tight.** It gives 100 ms per call and 64 MiB
  per plugin. A Letter page at 200% on a 2× display is a 2,448 × 3,168 pixel
  target: 7.8 million pixels, 31 MiB of RGBA. A dense vector page that size
  is likely past 100 ms in any renderer (not measured: V0 measures it). And
  64 MiB holds about two such pages, with nothing left for the parsed
  document. An overrun unloads the whole plugin (§3.1), which would kill
  every open PDF over one page.
- **A §3.2 job is one-shot.** Each job runs in a fresh worker instance. A job
  per render would re-read the file and re-parse its cross-reference table,
  page tree and fonts for every request. Three costs from the stage-3
  measurements apply: instantiating from the compiled component (65 KiB
  resident, sub-millisecond), copying the file in (about 0.3 ms per MiB through
  the boundary, so ~6 ms for a 20 MB PDF before any parsing), and rebuilding
  font and image caches. The last is **not measured** and is likely the
  biggest. A page shown at 2× is about twelve 512-pixel tiles, so one-shot
  jobs pay this many times a second while scrolling.

So a **viewer session** is a worker store that lives as long as the document's
tab, built on T1700's worker stores:

| Limit | Value | Why |
|---|---|---|
| Lifetime | From the tab's open to its close. A tab hidden for 60 s drops its session and reopens it when shown. The memory pool (below) may also drop one sooner to admit another. | Holds the parsed document and the plugin's caches across requests. Hidden tabs don't pin memory. |
| Memory | The plugin's `background.memory` (default **256 MiB**), clamped to the session pool | Parsed fonts, decoded images and a page pixmap. A session is admitted only if its declared memory fits in the pool (below). |
| Session pool | **min(1 GiB, ⅛ of RAM)** for all viewer sessions together, **apart from** §3.2's all-jobs total | Sessions and jobs never wait on each other: a PDF can't starve a background index, and an index can't stop a PDF opening. |
| Per request | An **absolute deadline from dispatch**: **10 s** for `render`, `layout` and `outline`, and **30 s** for `open`. It isn't reset by host imports. | §3.2's job slice resets on every import, so a guest calling `cancelled()` in a loop would never hit it. A session sets its epoch deadline once at dispatch and doesn't install the resetting call hook. The deadline bounds every call, whatever the guest does. |
| Cancellation grace | **1 s** after `cancelled()` turns true. Past it, the host interrupts the store. | Bounds how long a stale request can hold the session. An interrupted guest's state can't be trusted (§3.1), so the session is dropped and reopened at once. A grace kill isn't counted as a crash (§8), unless three come in a row. |
| Sessions | **4** live per plugin, **8** in all; the least recently shown is dropped first | Four PDFs side by side is generous. More just reopen when shown. |
| Thread priority | Normal, not the jobs' lowered priority | The user is looking at it. |
| Calls in flight | **1** per session; the host queues the rest | Wasm can't be interrupted without killing the store. So staleness is bounded by the grace, and the queue is where cancellation happens. |
| Result | ≤ **16 MiB** per call (§3.1's read bound, §3.2's result bound) | 16 tiles of 512 × 512 RGBA. `layout` is capped at 4 MiB and `outline` at 1 MiB. Larger is `invalid-argument`, shown as a failed page. |
| No `init`, no registration | as §3.2's worker stores | The session can only answer. |

The interactive instance (with `init`) still exists for the plugin's commands
and `viewer-handler.on-event` (§4).

**Admission to the pool, with a progress guarantee.** A session asks for its
declared memory when it opens or reopens.

- **When it fits**, it opens.
- **When it doesn't**, the host drops **idle** sessions until it fits. Idle
  means no call in flight and none queued. It drops hidden tabs' sessions
  first, then visible ones by least recent request.
- **A dropped visible session** keeps its tab's cached tiles on screen, and
  reopens (and is admitted by this same rule) only when that tab needs new
  pixels.
- **When dropping idle sessions isn't enough, a waiter takes priority at the
  next request boundary.** Queued work doesn't keep a session resident: a session with
  calls queued but none in flight is evictable too. The host picks
  **victims** from the resident sessions, in the same order (hidden first,
  then least recent request), until their memory covers the waiter. A victim
  is **marked**: the host dispatches none of its queued calls, and drops
  them. Its call in flight, if any, runs to its end (or its deadline plus
  the grace), its result is accepted as usual, and then the session is
  dropped. A victim with nothing in flight is dropped at once.
- **The dropped work is safe to lose.** Queued calls are only tile, layout,
  outline or Find-text requests, all of which the host re-derives from what
  the tab still needs. The victim's tab keeps its cached tiles on screen. When
  it next needs pixels it reopens through this same admission, at the back of
  the queue, and re-requests what is then in view. Nothing the user did is
  lost: selection, Find query, page and zoom are the host's.
- **No ping-pong.** A session just admitted can't be a victim until its first
  call ends, so two tabs can't evict each other before either draws. That
  call is bounded by the same deadlines, so a protected session becomes a
  candidate within 31 s and the bound below still holds.
- **The bound.** Marked victims start no new call, and every call ends within
  its absolute deadline plus the grace. So a waiter at the head of the queue
  is admitted within **11 s**, or **31 s** when a victim is in an `open`,
  whatever the resident sessions have queued and however continuously they
  queue more. That holds because victims are chosen by memory, not by
  idleness: it never waits for a session's queue to empty, or for a tab to
  close or hide.
- **Fairness.** Waiting sessions are admitted in arrival order, and only the
  head of the queue marks victims. A session dropped to make room rejoins at
  the back, so it isn't re-admitted ahead of one already waiting. With at most
  8 sessions, the eighth waiter is admitted within 8 × 31 s in the worst
  case, or 8 × 11 s when no victim is in an `open`.
- **Too big for the pool.** A declaration larger than the whole pool is
  clamped to it at install, with a manifest warning, so one session alone
  always fits.

While a session waits, its tab shows its cached tiles, or the low-resolution
pages, with "Waiting for memory" in the status bar.

**What the host does:**

- **Layout.** Pages go in one vertical column, with an 8 px gap and the
  editor's background around them. Fit-to-width is the first zoom. Zoom uses
  the image viewer's `STEPS` (10% to 2,000%) and pinch handling, and steps
  through them on cmd-+ and cmd--.
- **Tiles.** Tiles are 512 × 512 device pixels (1 MiB of RGBA), so the cache
  is in whole MiB and a tile is never larger than one GPU upload. Scale is
  zoom × the window's scale factor. A move to a new display, or a change of
  its scale, re-requests at the new scale.
- **Request order.**
  1. Tiles in view at the current scale, nearest the centre first.
  2. A low-resolution whole page (scale 0.25, about 150 × 200 px for Letter)
     for every page that comes into view, so fast scrolling shows a blurred
     page instead of a gap.
  3. One viewport of prefetch in the scroll's direction.
- **Zoom gestures.** While pinching or wheel-zooming, the host scales the
  tiles it has. It requests the new scale only after a 150 ms settle (the
  image viewer's `SETTLE`).
- **Identity.** Everything the host keeps from a session is keyed by a
  **source**: (file, document generation, renderer). The document generation
  is bumped when the file changes (§8). The renderer is the viewer id plus
  the plugin's version and its installed component's SHA-256. So an update, a
  reinstall or a different viewer is always a different renderer, even over
  unchanged bytes. Each session also has a **session generation**, bumped on
  every open or reopen.
- **Cache and eviction.** One app-wide cache of **256 MiB** of tiles, keyed by
  (source, page, scale, tile, invert). It evicts least recently drawn first,
  never a tile drawn this frame. Low-resolution pages are kept apart, under
  32 MiB, and evicted last. Page layouts and the Find text cache (§4) are
  keyed by source too.
- **Replacing the source is a transaction.** When a tab's source changes
  (Open With another viewer, a plugin update, reinstall, enable after
  disable, or a changed file), the host doesn't throw the old display away
  first:
  1. The tab's current source becomes its **last-good display**: its tiles,
     layouts and page text stay in the cache under the old source, still
     drawn, and are no longer requested or refreshed.
  2. The new source is **pending**. The host drops the tab's queued requests
     for the old source and opens the pending one (a new document generation
     or renderer, in a new session generation).
  3. Results are accepted only for the pending source and its session
     generation once it exists; anything from the old source or an older
     pending one is dropped. A newer change while one is pending supersedes
     it: the newest pending source wins, and the last-good display is
     unchanged.
  4. **On success** (the pending `open` returns), the pending source becomes
     current. The old one's tiles stay drawn under the new layout only until
     the new tiles for the view arrive, then the host evicts the old source's
     tiles, layouts and page text, and clears selection and Find matches,
     keeping the Find query and searching again.
  5. **On failure** (the pending `open` errors, traps or passes its
     deadline), the pending source is discarded, and the last-good display
     stays current, with §8's "Couldn't reload" banner. Selection and Find
     stay as they were. The next change tries again.

  Its page and zoom are kept throughout. Last-good tiles count against the
  256 MiB cache like any other, and are evicted last among that tab's.
- **Cancellation.** Before each dispatch, the host drops queued requests whose
  tiles are no longer wanted: scrolled out of view plus the prefetch margin, at
  a scale no longer current, or from a superseded source. `document.cancelled()`
  turns true for a dispatched request that went stale, and the 1 s grace above
  bounds it. The plugin may stop and return an error, which the host discards.
- **Accepting a result.** Every request carries its source and session
  generation. A result is accepted only if both are still the tab's current
  ones. A result from a superseded renderer, document or session is dropped
  whatever its scale. A stale result with a current source and session goes
  into the cache only if its scale is still current.
- **Checks on what comes back.** Each tile's dimensions must match its request
  and its byte length must equal `width × height × 4`. Coordinates must be
  finite and inside the page. Anything else fails that page with "The PDF
  Viewer returned a bad page", shows a hatched placeholder, and is logged. The
  session isn't trusted to be right, only to be bounded.

**Accessibility.** Each page is an AccessKit node, role document, labelled
"Page 3 of 12". Its child is a static text node holding the page's `layout`
text. That text is asked for lazily, for pages in view and one either side. A
page with no text, such as a scan, is labelled "Page 3 of 12, no text". The
surface is focusable, and its keys are §4's. Labels go in fe-gpui's `.ftl`, as
kiln-ui's lint requires of every literal.

**Theming.**

- The page is paper: it keeps the document's own colours, usually white.
- The surround, the page border and the selection and match colours are theme
  tokens (`editor.background`, `border`, `selection`, `find_match`). So Yohen,
  Hakuji and every other theme style the chrome, and the plugin never sees a
  theme.
- A host option, `viewers.invert_in_dark` (off by default), inverts lightness
  with hue kept, on the CPU, when a tile is uploaded under a dark theme. It is
  part of the cache key, so toggling it re-uploads. Plugins don't implement
  it.

**Decision:** a host-owned, pull-driven tile surface. There are
`viewer-session` exports, a `document` handle limited to the opened file,
512-pixel RGBA tiles in batched per-page calls, and a 256 MiB app-wide tile
cache. Rendering runs in a **viewer session**: a long-lived worker store per
document. Sessions have their own memory pool. A waiting session marks
resident victims at the next request boundary (their queued work dropped and
re-derived later), so it is admitted within 11 s (31 s past an `open`)
however busy the residents are. Each call has an absolute deadline (10 s,
30 s for `open`) that host imports don't reset, plus a 1 s cancellation grace
before a forced interrupt. One call is in flight at a time, and a plugin has
at most 4 sessions. Every cached tile, layout and result is keyed by its
source (file, document generation, renderer) and checked against the
session's generation. Accessibility and theming are the
host's. The one-shot-job alternative is rejected on re-parse cost. That cost
is not measured, so V0 measures it against hayro to confirm (§7).

## 4. Input, selection and text copy

**The host owns input.** Pointer, wheel, pinch and keys don't reach the plugin.
The host interprets them against the geometry the plugin returned: page sizes,
glyph boxes and link areas. This keeps the plugin off the per-frame and
per-event path, which stage 3 §5.2 rules out for plugins (a hop and a copy per
event).

- **Scrolling:** wheel, trackpad, arrows, Page Up/Down, Space and Shift-Space,
  Home and End.
- **Zoom:** cmd-+, cmd--, cmd-0 (fit width), ctrl-wheel and pinch.
- **Focus:** the surface takes focus like any tab.
- **Go to Page:** the editor's Go to Line binding, in the viewer's context,
  opens a page input in a header strip, as the hex viewer's Go to Offset does.
- **Find:** the find binding, below.

New bindings are scoped to the viewer's key context, `PluginViewer`. Whether
vim motions (`j`, `k`, `gg`, `G`, `ctrl-d`) apply follows the hex viewer's
choice and is V6's to settle.

**What the plugin does get**, through `viewer-handler` in its interactive
instance:

```wit
// fastedit:editor@1.1.0 (preview), exported; interactive tier
interface viewer-handler {
    variant viewer-event { shown, hidden, page-changed(u32) }
    on-event: func(viewer: string, path-token: u64, event: viewer-event);
}
```

That is enough to keep state of its own (a recent-pages list, say) and to
enable its contributed commands. A plugin command shown in the viewer's
context menu runs with `Actor::User` as any other command. There is no
pointer event. A plugin that needs one (an annotation tool) is beyond this
design (Q3).

**Selection.**

- **The model.** A selection is a range over the document's text in reading
  order, made of per-page `layout` text joined with a page break. A drag from
  one glyph box to another selects the chars between them in that order.
  Double-click selects a word, by the text's word boundaries. Triple-click
  selects a line: a run of boxes sharing a baseline band. cmd-A selects
  everything, after asking for every page's layout (as Find does).
- **Drawing.** The host draws the selection as the union of the selected
  chars' boxes, in the `selection` token, over the tiles.
- **Text-less areas.** A drag over an area with no text (a figure) selects
  nothing. There is no rectangle-snapshot copy in v1.

**Copy** puts the selected text on the clipboard. Across pages it is joined
with a blank line. It needs the plugin's **`clipboard`** grant: the text comes
from the plugin, and it might not be what's drawn (a PDF can hold invisible
text), so the user agrees to that at install ("Copy text you select in its
views"). Without the grant, Copy is disabled and says so in the status bar.
The host writes the clipboard only on the user's copy. The plugin never
writes it.

**Links.**

- **Hover.** A link's `area` shows the pointing-hand cursor and the target
  in the status bar.
- **External.** A click on a `uri` follows the same rule as
  `lsp_links.rs`: `http`, `https` and `mailto` go to `cx.open_url`, and any
  other scheme (`file:`, `javascript:`, a launch action the plugin turned
  into a URI) is refused with a toast. A target over 2 KiB is refused. There
  is no confirmation, as for any link the editor opens.
- **Internal.** A `page` target scrolls there and puts the point at the top
  of the view. The jump is recorded, so the editor's Go Back (`nav.back`)
  returns to where the link was.

**Find in document.** cmd-F opens a find input in the viewer's header strip,
reusing the `find_bar` key context, as the hex viewer does, so Enter,
Shift-Enter and Escape need no new bindings.

- **Searching.** The host asks the session for `layout` of each page in order,
  after the visible renders, as low-priority requests. It searches each page as
  its text arrives, case-insensitively by default with the editor's case
  toggle, and shows "Searched 12 of 40 pages" until it is done.
- **Results.** Matches are highlighted with `find_match` over their boxes. The
  current one is outlined and scrolled into view.
- **Memory.** Text is cached for the document, up to 32 MiB. Past that, pages
  far from the view are dropped and asked for again.
- **Scans.** A document with no text says "No text in this document" rather
  than "Not found".

**Decision:** the host owns every input, the selection, Find and link
handling, computed from `layout`'s glyph boxes. The plugin sees `shown`,
`hidden` and `page-changed` only. Copy is the host's, on a user act, and needs
the `clipboard` grant. External links follow `lsp_links.rs`'s scheme rule.

## 5. Install on open

Generalising `grammar_prompt.rs`. The grammar prompt's mechanics carry over
whole: catalog-driven, one prompt per key app-wide, owned by the window that
showed it until its download ends, a Never file in the state directory, and
no network until it is answered. V7 moves that shared core into
`offer_prompt.rs`, with the grammar prompt and a new `viewer_prompt.rs` on top
of it. The grammar prompt's behaviour and tests stay as they are.

**The map.** The workbench is handed `ViewerOffers`, as it is handed
`GrammarOffers`. It holds the claims of the index's plugin entries (§6), and
offers for a file the entries whose claim `claim_matches` it (§2's one
contract, with its text and image rules). Several matching entries offer the
best one in §2's step-3 order. It comes from the cached, verified plugin index, through
the same `publish` callback T1485 uses for grammars. With no cached index (the
first seconds of a first launch, offline, or automatic checks off), nothing is
offered. There is **no index fetch on open**, as T1485 decided for grammars.

**Where it asks.** It depends on whether core has a notice for the type:

- **A type with a built-in claim (PDF): in the notice tab.** A PDF without a
  viewer already opens in Card A's notice (§8), which is the tab's own
  content, so the offer is a row there, shown below.
- **Any other type the index claims: a toast over the tab core opened.** The
  file opens as it does today, in the hex viewer for a binary format or the
  editor for one matched by magic. The offer is the grammar prompt's toast:
  "*Title* files can open in a viewer: *Plugin* (*size*)", with Install,
  Not now and Don't ask for .*ext* (or for *Title* files, below). After an
  install, that tab swaps to the viewer only under the revalidation rule of
  "After it installs", so an editor with unsaved edits is never replaced. So
  a new format in the index gets an offer
  without core gaining a notice for it, and without changing where its files
  open first.

The PDF row in the notice:

> **View PDFs in Tenmoku** with the PDF Viewer plugin (2.1 MB download).
> [Install PDF Viewer] [Not now] [Don't ask for .pdf]

For PDF, a toast would repeat what the tab already says, and would disappear
while the tab stays.

**The Never key** is the file's extension, lower-cased. For a file with none,
matched by magic, it is the matching claim's first extension (`pdf`). A
**magic-only claim** (one with no extensions at all) is keyed by its claim
identity instead, `"<plugin id>.<viewer id>"` from the index entry (for
example `"acme.sqlite.db"`), and its toast says "Don't ask for *Title*
files". Every offer therefore has a Don't ask with a stable key. The identity
is the index entry's, so it survives a plugin update; a renamed plugin or
viewer id is a new type and asks again.

**Answers:**

- **Not now** hides the row in every notice of that extension until the next
  launch. Closing the tab counts as nothing: the next PDF asks again.
- **Don't ask for .pdf** is per extension. It is kept in
  `<state dir>/viewer_prompt.json` as `{"never": ["pdf"]}`, with the grammar
  Never file's rules: read off the main thread before each offer, written
  under a lock through a rename, and kept in memory too in case the write
  fails. Claim-identity keys go in the same file, as
  `{"never": ["pdf"], "never_claims": ["acme.sqlite.db"]}`, under the same
  rules, so they persist across restarts like extensions do. The notice keeps a quiet "Get a viewer…" link that opens the plugin's
  page in the extensions list. So the answer is reversible without a settings
  edit, and installing from there clears the Never entry.
- **Install** opens the **standard plugin grant prompt** (`plugin_prompt.rs`)
  before any download. It is filled from the verified index entry: the
  plugin's name, publisher, licence, "signed by the Tenmoku registry", its
  capabilities in plain words ("Show views of files you open", "Copy text you
  select in its views") and its background memory line. That is the one trust
  gate users know. **Allow** starts the download, and **Deny** counts as Not
  now. The grant is recorded only after the downloaded manifest's capabilities
  are checked equal to the entry's (§6).

**While it downloads.** The row becomes "Downloading PDF Viewer… 1.2 of 2.1
MB" with a Cancel button, in every notice of a claimed type, in every window.
Cancel, or closing the owning window, stops it, as for grammars. From the
moment the download completes, the install belongs to no window, so a closing
window can't orphan a half-written extension (the grammar prompt's rule).

**After it installs.**

1. `extension_install.rs` places it with its staged, link-free unpacking.
2. The extensions service's hot reload loads it.
3. The first session compiles the component. A cold compile is about
   0.3–0.5 s per MB of wasm on the stage-3 machine (§5.1, measured), so the row
   says "Preparing PDF Viewer…".
4. **Each candidate tab is revalidated, then swapped.** The candidates are
   every tab, in every window, hidden tabs included, that showed the offer
   or is a notice for a file the new plugin claims. At the moment of the
   swap, not when the offer was shown, the host checks that the tab:
   - still shows the same path, in the same kind of item it had when offered
     (a notice, the hex viewer, or an editor);
   - still `claim_matches` the new plugin (the file may have changed);
   - wasn't set by hand through Open With… since;
   - and, for an editor, **is not dirty**.

   A tab passing every check **swaps in place to the plugin view** at
   page 1. A notice and the hex viewer are never dirty, so they always
   swap. **A dirty editor is left exactly as it is**, edits and undo
   history intact. Its toast changes to "*Plugin* is installed. Save or
   revert to open this file in it", with **Open in *Plugin***, which runs
   Open With…'s swap and, like Open as Hex today, refuses while the editor
   is dirty. Once the editor is saved or reverted, the button works; the
   host never swaps it on its own after the install moment. A tab failing
   any other check is left alone, with no toast.

If the plugin hasn't loaded 60 s after it was placed, the row says "PDF Viewer
was installed but hasn't loaded", pointing at the log, and keeps looking every
2 s, as the grammar install does.

**Offline and failed installs.** One line in the notice row, with Retry:

- a lost connection, a timeout or a broken transfer: "Couldn't download PDF
  Viewer: no connection to the download";
- a hash or capability mismatch: "Couldn't install PDF Viewer: the download
  didn't match the registry" (no hashes shown);
- any other failure: "Couldn't install PDF Viewer: *reason*".

Nothing partial stays on disk (the staging directory is removed). The tab
stays on the notice, whose Open in Default App and Open as Hex still work.

**No network without the answer.** Nothing is fetched for an offer until
Install is clicked *and* the grant prompt's Allow is clicked. Showing the offer
uses only the cached index. A test asserts that no request leaves before both
clicks, using the grammar tests' fetch seam.

**Decision:** a shared `offer_prompt` core under the grammar and viewer
prompts, matching files to index claims by §2's one contract. For PDF the
offer is a row in the notice tab, and for any other claimed type it is a toast
over the tab core opened. Each has Install (size), Not now and a Don't ask
with a stable key (the extension, or the claim identity for a magic-only
claim), persisted in `viewer_prompt.json`. Install goes through the standard
grant prompt before downloading. Progress and errors show in the notice or
the toast. Once the plugin loads, each candidate tab is revalidated (same
path and kind, still matching, not chosen by hand, not dirty) and only then
swapped in place. A dirty editor is never replaced: it gets an explicit
Open in *Plugin* that works after a save or revert.

## 6. Distribution

**Plugins in the signed index (T1709, stage 3 §4.6), with viewer claims.**
T1709 adds plugin entries. This record adds the claims to them, so the offer
needs no download first:

```json
{
  "schema": 1,
  "serial": 1763420000,
  "url_template": "https://github.com/tenmoku-editor/registry/releases/download/{tag}/{file}",
  "plugins": {
    "tenmoku.pdf": {
      "name": "PDF Viewer",
      "version": "1.0.0",
      "tag": "plugin-tenmoku.pdf-<sha256 prefix>",
      "archive": { "name": "tenmoku.pdf-1.0.0.tar.gz", "size": 2200000, "sha256": "<hex>" },
      "installed_size": 6100000,
      "license": "Apache-2.0",
      "engine": { "tenmoku": ">=0.9", "api": "1.1" },
      "capabilities": ["viewers", "clipboard"],
      "background": { "memory": "512MiB" },
      "viewers": [
        { "id": "pdf", "title": "PDF", "extensions": ["pdf"], "magic": "%PDF-", "within": 1024 }
      ]
    }
  }
}
```

The sizes above are placeholders, not measurements. V0 measures the real
ones. Every field but `viewers` and `installed_size` is T1709's to define, and
is shown only to place the claim and the sizes.

**Where the entries live: a sibling file, not `index.json`.** The grammar
`Index` refuses unknown keys and any schema but 1 (§1). So putting `plugins`
into `index.json`, or bumping it to schema 2, would make every app already
shipped refuse the whole index and lose its grammar offers. The plugin entries
go in a **sibling `plugins.json` and `plugins.json.sig`**, published beside
`index.json`:

- signed with T1482's registry key, in T1483's scheme;
- with its own `schema` (1) and its own `serial`;
- verified, cached and refused when unsigned, tampered or rolled back, by
  T1484's client code, applied to a second file.

Old apps never fetch it. This is a recommendation to T1709, which hasn't
started. If T1709 settles another shape first, V8 adds the `viewers` field to
whatever entry T1709 defines (Q1).

**The claim fields in the entry** mirror §2's manifest: `id`, `title`,
`extensions`, `magic`, `within`/`offset` and `priority`. The publisher (V10)
generates them from the plugin's `extension.toml`, so they can't drift. The
app checks them again against the downloaded manifest, and a mismatch fails
the install like a hash mismatch.

**Size in the prompt.** "2.1 MB download" is `archive.size`. The grant prompt
adds "6.1 MB installed" (`installed_size`).

**Hash-pinned artifacts and verification.**

- **Download.** The archive URL is the compiled-in `url_template`, as grammars
  use (T1485), never a URL from the index. It goes through fe-net's granted
  hosts.
- **Checks.** The archive's SHA-256 and size must equal the entry's, then
  `extension_install.rs`'s unpacking limits apply (`MAX_ENTRIES`, the unpacked
  byte cap, no links). Then the manifest's id, version, capabilities,
  `background` and `viewers` must equal the entry's.
- **The listing.** The extensions list shows the install as "signed (Tenmoku
  registry)". A local install of the same plugin stays allowed, and is marked
  "unsigned (installed from a file)", as §4.6 says.

**Updates** come from the plugin index, under §4.6's rule: an update that adds
a capability, raises `background.memory`, or claims a new extension stops and
asks again (decision 3, escalation). A new claim counts as an escalation,
because it changes which files the plugin sees. While a viewer tab is open,
its session finishes its request, the new version loads, and the tab reopens
on it at the same page and zoom.

**Uninstall and disable.**

- **Open tabs.** Each tab's session is dropped and the tab swaps to the
  notice: "PDF Viewer was uninstalled", or "PDF Viewer is disabled" with an
  Enable button. The tab's saved state stays a `fastedit.plugin_viewer`, so a
  reinstall or re-enable swaps it back.
- **Data.** Uninstall removes the extension directory, as today. A viewer
  plugin holds no plugin data (it needs no T1702 store), so there is nothing
  more to delete.
- **The Never list.** Uninstall doesn't touch it. The user can be offered the
  plugin again on the next PDF, which is the point of uninstalling a viewer
  you might want back.

**Decision:** viewer claims in each plugin entry of a **sibling signed
`plugins.json`**, so shipped apps keep reading `index.json`. Sizes come from
the entry, the archive is hash-pinned at the compiled-in URL, and manifest and
claims are checked against the entry. A new claim counts as an escalation. On
removal, open tabs fall back to the notice and keep their state.

## 7. The first user: a first-party PDF plugin

**`tenmoku.pdf`**, under `extensions/plugins/pdf/`, follows stage 3 §5.3's
pattern for first-party plugins:

- its own Cargo project with a lock file, outside the workspace;
- built for `wasm32-wasip2` by a `build.zsh`;
- its prebuilt component checked in;
- CI never builds guests.

It ships through the registry and is **not bundled in the app**.

**Binary-size budget.** The app grows by **zero bytes of renderer**. The
plugin's crates are never in `crates/*` or `kiln/crates/*`, and V9 adds
`deny.toml` bans on `hayro*`, `pdfium-render`, `mupdf` and `pdf_render`, so no
app crate can pull one in by accident. What core does grow by is the host
side: the notice (Card A), the resolver, the session runner, the tile surface
and the prompt. That is ordinary Rust in existing crates, with no new
dependencies. V5 records the release binary's size before and after it lands,
as `perf.rs`-style evidence. The plugin's budget is **at most 8 MB of `.wasm`**
and **at most 16 MB installed**. Past that, the cold compile on a slow machine
passes a few seconds (§5.1's 2–3 MB/s, measured on an M3 Max), which is a
bad first open.

**The survey.** No row's speed is measured: every speed cell is a measurement
V0 must take. Sizes are measured only where another project published them,
and none of those is a WASI component.

| Renderer | Artifact size (.wasm / installed) | Licence, and Apache-2.0 compatibility | Speed per page under wasmtime with the plugin limits (text page / image page) | Feature coverage | Build target |
|---|---|---|---|---|---|
| **hayro** 0.9.0 (LaurenzV; crates `hayro`, `hayro-interpret`, `hayro-syntax`, `hayro-font`, `hayro-jpeg2000`, `hayro-jbig2`, `hayro-ccitt`, `hayro-cmap`) | **Not measured.** The `hayro` crate is 129 kB of source. Its dependencies include `vello_cpu`, `kurbo`, `image`, `pic-scale` and `fearless_simd`. `embed-fonts` adds about 240 KB for the 14 standard-font substitutes (its README). | **Apache-2.0 OR MIT: compatible.** Its crypto is described as ported from pdf.js (Apache-2.0), also compatible. | **Not measured** (V0) / **not measured** (V0) | **Fonts:** Type1 and CFF (`hayro-font`), the standard 14 through `embed-fonts`. TrueType isn't named in the READMEs (V0 checks). **Encryption:** RC4, AES-128 and AES-256 decryption (`hayro_syntax::crypto`). User-password support is unverified. **Forms:** no filling (we need none). Whether widget appearances draw is unverified. **Annotations:** appearance streams unverified. **Images:** JPEG, JPEG 2000, JBIG2 and CCITT in its own crates. **Transparency:** isolated and knockout groups unsupported (README). **Text:** no extraction API. Glyphs pass through the `Device` trait; whether a char's Unicode does is unverified (V0). **Robustness:** its README says there are "still many ways to trigger timeouts and OOM errors". The sandbox's limits are what absorb them (§9). | Pure Rust, `forbid(unsafe_code)` in `hayro`, CPU only, no threads needed. `wasm32-wasip2` has no known blocker, but isn't built here. SIMD through `fearless_simd`, whose wasm `simd128` path is unverified. `hayro-syntax`'s optional `unsafe` feature (zlib-rs) is off for the plugin unless V0 shows a need. |
| **PDFium** (Google; via the `pdfium-render` crate or a C API shim) | **Not measured for WASI.** Third-party emscripten builds are 4.4–4.5 MB of `.wasm` (1.5–2.7 MB compressed). No `wasm32-wasip2` build is known. | **BSD-3-Clause, with Apache-2.0 text in its LICENSE: compatible.** Its third-party code (FreeType, libjpeg-turbo, OpenJPEG, lcms2, zlib) needs a per-library check at port time. | **Not measured** / **not measured** | The most complete: every font type, all standard encryption, AcroForms (XFA optional), annotations, and a text API with char boxes (`FPDFText_*`), which `layout` maps to directly. | C++ built with wasi-sdk: a port, with no threads and no exceptions. Its build system expects a browser or a native target. A much bigger spike than hayro's, and C++ parsing hostile input (contained by the sandbox, §9). |
| **pdf.js** (Mozilla) | **Not measured.** It needs a JavaScript engine compiled to wasm (such as StarlingMonkey through componentize-js) plus pdf.js itself. That is likely several MB before pdf.js, and wasn't checked. | **Apache-2.0: compatible.** | **Not measured** | Complete, and the browser default. | **Ruled out.** It is JavaScript, and it renders through the browser's Canvas 2D API, which a WASI component doesn't have. So it would need a canvas implementation as well as a JS engine. No webviews (§1) closes the remaining route. |
| **MuPDF** (Artifex) | **Not measured.** Artifex publishes a wasm build (mupdf.js), for the browser. | **AGPL-3.0, or a commercial licence: not compatible** with shipping as an Apache-2.0 first-party plugin. The AGPL would bind the plugin's source and, arguably, its host's distribution terms. Out unless Artifex's terms change or a commercial licence is bought (the user's call, not proposed). | **Not measured** | Complete: fonts, encryption, forms, annotations, text with boxes. | C. wasi-sdk is plausible, but moot given the licence. |
| **pdf-rs** (`pdf` with `pdf_render` and `pdf_text`) | **Not measured** | **MIT: compatible.** | **Not measured** | Parsing, image and font extraction. `pdf_text` rebuilds words and lines. `pdf_render` draws through Pathfinder, a GPU-oriented rasterizer, and hayro's README calls hayro the most complete pure-Rust rasterizer. | Pure Rust. `pdf_render`'s Pathfinder dependency is the risk for a CPU-only wasm guest. **Second choice to hayro among pure-Rust options, not a fallback**: it would be a spike of its own. |

`lopdf` (MIT) parses and edits but doesn't render, so it isn't a candidate.

**Recommendation: hayro.** It is pure Rust under our own licence pair, CPU
only, needs no threads and forbids unsafe code. Its crates cover the image
codecs PDFs use, and it should build for `wasm32-wasip2` without a C
toolchain. That keeps the plugin's build like the other guests'. Its known
gaps (transparency groups, robustness on hostile input, no text API) are
either rare in documents people open in an editor, or are covered by the
sandbox (§9) and by V0's check of the `Device` trait for text. **PDFium is the
fallback candidate** if hayro fails V0. It has the best fidelity and the text
API we want, but costs a C++ port to WASI.

**The spike card: V0, "Measure hayro as a WASI viewer component".** It writes
a temporary guest exporting §3's `viewer-session` over a WIT stub (not
committed, like the stage-3 harness in its Appendix A), and a decision record
of what it finds:

- **Sizes:** `.wasm` in release with `opt-level = "s"` and with `"z"`, before
  and after `wasm-opt -Oz`, with and without `embed-fonts`. The installed size
  and the compressed archive size.
- **Compile:** cold compile and cached load under kiln-plugin's engine
  (wasmtime 48.0.8, as the workspace pins), on Linux, and on the Mac if
  argon is free.
- **Speed** under the plugin limits (the session's 256 MiB store and the epoch
  deadline), medians and minimums of 20:
  - a text page (a one-column paper page) and an image page (a full-page
    photo, JPEG, and a scanned JBIG2 page);
  - each at scale 1 and 2, as one whole page and as twelve 512-pixel tiles in
    one call;
  - natively, beside it, for the wasm-vs-native ratio stage 3 didn't measure.
- **Re-parse cost:** `open` and first-page time from a cold instance, to put a
  number on §3's one-shot-job rejection.
- **Memory:** peak store size on each fixture, and on a 500-page document.
- **Text:** whether glyphs reach the `Device` with their Unicode and boxes, so
  `layout` is possible, or whether the plugin needs its own text extraction.
- **Hostile input:** a fuzz minute and three crafted files (deep nesting, a
  huge image, a compression bomb), with how each ends under the limits.
- **Pass:** `.wasm` ≤ 8 MB; median text page at scale 2 ≤ 150 ms and image
  page ≤ 500 ms; peak store ≤ 256 MiB on the fixtures; text with boxes
  available; every hostile file ends in an error or a limit, never a host
  failure.

V0 is Linux-OK, needs no other card, and is the **one** card to run before the
plugin card. If hayro fails a threshold, V0's record says which, and a PDFium
spike is drafted for the user. V9 waits for whichever passes.

**Decision:** hayro is recommended, subject to V0's thresholds, with PDFium as
the fallback candidate. MuPDF is ruled out on licence and pdf.js on being
JavaScript with a Canvas dependency. No speed or plugin size is measured yet.
V0 measures them first.

## 8. Fallback: the native PDF notice (Card A)

**Card A** adds `Opened::Pdf` to the loader and a `PdfNotice` view. It is the
first card. It ships useful on its own: today a PDF lands in the hex viewer or
a text editor depending on its bytes, and after Card A it always lands
somewhere that says what it is.

- **Detection.** `.pdf` in any case, or `%PDF-` in the first 1024 bytes, read
  in the same probe `is_binary` uses. It comes after the image check and
  before the binary check, and §2's resolver later puts plugin claims ahead of
  it.
- **The view.** It is a centred card in the editor's background:
  - a document icon and the file's name;
  - "PDF document · 2.4 MB" (its size only, because core doesn't parse PDFs);
  - two buttons, **Open in Default App** (`cx.open_with_system`) and **Open
    as Hex** (the existing `Pane::open_as_hex`, swapping in place);
  - **the slot**: a row below the buttons that §5's offer, §6's
    uninstalled/disabled messages and this section's crash messages fill. In
    Card A the slot is empty.
- **The pane item.** Kind `fastedit.pdf_notice`, restored by path. It is never
  dirty, read-only in the status bar as the hex viewer is, and has
  `shares_document` for splits. Card A also adds `workbench.open_in_default_app`
  for the notice, editors, the image viewer and the hex viewer.

**The notice is the slot every later card fills or replaces:**

| Situation | The notice shows |
|---|---|
| No plugin installed, the index has one, never answered | §5's offer row |
| No plugin, and Not now or Don't ask for .pdf | Its buttons only, with "Get a viewer…" after Don't ask |
| No plugin in the index, or no cached index | Its buttons only |
| The plugin is installed but disabled | "PDF Viewer is disabled" and **Enable** |
| The plugin failed to load (a bad component, or a compile error) | "PDF Viewer couldn't load: *reason*" and **Show Log** |
| A restored tab whose viewer is gone | Why, with the saved page kept for when it returns |

**A plugin view that crashes or overruns falls back to the notice, with a
message.**

- **A request failure.** A session that traps, passes its call's absolute
  deadline, or passes its memory cap is killed (a worker store, not the
  plugin, §3.2). So is one that ignores a cancellation past the 1 s grace
  three times in a row. The host reopens it once, at the same page. A single
  grace kill, or a drop to make room in the pool (§3), isn't a failure: the
  session just reopens.
- **A second failure** on the same document within a minute swaps the tab to
  the notice with "PDF Viewer stopped on this file: *reason*", plus **Try
  Again** (a fresh session, page kept) beside Open in Default App and Open as
  Hex.
- **One bad page.** An `open` error (`malformed`, `unsupported`) on a tab's
  first open goes straight to the notice with the plugin's reason, truncated to 200 chars. A
  failed `render` of one page shows a hatched placeholder on that page alone,
  so one bad page never costs the document.
- **Passwords.** `needs-password` shows a password input in the notice's
  slot. The password goes to `open` once and is never stored or logged.
  `wrong-password` says so and asks again.
- **The file changes on disk.** The session reopens on the new bytes after a
  300 ms debounce, keeping page and zoom. That is what a LaTeX build loop
  wants. The reload is §3's source transaction: the old display stays drawn
  while the new bytes open, and is retired only once they have. If the new
  bytes don't open (a half-written file), the old display stays current with
  a "Couldn't reload" banner, and the next change tries again. An `open`
  error on a reload keeps the last-good display rather than going to the
  notice, and counts toward the second-failure rule above only if the
  session trapped or passed its deadline.

**Decision:** Card A's `Opened::Pdf` and `PdfNotice` are the first card and
the permanent fallback. Every missing, disabled or failed viewer state is a
message in its slot, beside Open in Default App and Open as Hex. Crashes
restart once, then fall back to the notice with Try Again.

## 9. Security and limits

**The file is untrusted, and it is parsed in the sandbox.** A PDF from an email
attachment or a download is exactly the input parsers fail on. hayro's README
says so of itself (§7). The session is where that is contained:

- **No ambient reach.** The session's WASI context has **no preopens, no
  environment, no arguments and no clocks beyond the monotonic one**. Its only
  input is the `document` handle: ranged reads of the one opened file, plus its
  bare name. It can't list a directory, open a sibling, or learn the path.
- **No output channel but its answers.** A viewer plugin without `net` can't
  send a byte anywhere. Its answers are pixels, text and boxes, all checked
  (§3). Link URIs are checked against `lsp_links.rs`'s scheme rule and opened
  only when the user clicks.
- **Bounded.** A hostile file can, at worst:
  - exhaust its session's memory (256 MiB by default, never more than the
    session pool);
  - spin until its call's absolute deadline (10 s, or 30 s for `open`),
    however often it calls back into the host;
  - return garbage, which is refused and shown as a failed page.

  Each of these ends in §8's fallback, never in a host failure. A compression
  bomb is a memory overrun. Deep nesting is the guest's own stack overflow, a
  trap. A billion-pixel image is the guest's allocation failing.
- **Isolated.** One session per document, so a crash in one PDF doesn't touch
  another, the plugin's interactive instance, or other plugins.
- **No secrets cross.** A password reaches only `open`, for that session. The
  host keeps it only as long as the tab is open, to reopen after a crash or a
  drop from the pool, and never writes it.

**The capability list for a viewer plugin:**

| Capability | New? | What it grants | Prompt wording |
|---|---|---|---|
| `viewers` | **New (V1)** | Its `[[contributes.plugin.viewers]]` claims, and sessions with a `document` handle to **the file the user opened, only**. No `fs` grant is needed or implied. A PDF from outside the workspace opens without one. | "Show views of the file types it claims: .pdf" (the extensions listed) |
| `clipboard` | **New (V1)** | Text it returned may be put on the clipboard when **the user** copies a selection in its view. The plugin itself can never write the clipboard. | "Copy text you select in its views" |
| `commands` | existing | Optional: its own commands in the viewer's context menu | as today |

The first-party PDF plugin asks for `viewers` and `clipboard` only. A viewer
plugin that also asks for `net:*` or `process.spawn:*` gets the existing
extreme warnings. The grant prompt adds "and it can read the files it opens",
because the combination of reading files and network access is how a viewer
could leak a document. `kiln_plugin::warnings` gains that combination (V1).

**Limits, all in one place:** these are §3's session table, plus the 256 MiB
tile cache, the 32 MiB text cache per document, and the validation of §2
(claims) and §3 (results). The user's per-plugin overrides of stage 3 §3.2
(`[extensions.plugin_limits."<id>"]`) apply to the session's memory, as
`background_memory`.

**Open question Q2:** should a viewer plugin be allowed `net` at all? It could
be useful for a viewer that fetches remote fonts. This record allows it behind
the combined warning, and the first-party plugin doesn't ask for it. The
alternative is to refuse `net` to any plugin with `viewers`, which is simpler
to explain and loses little.

## 10. Proposed card breakdown

**Handles of the stage-3 cards** used below (Mac numbers, as the spec names
them; the ids are stage 3 §6's):

| Handle | Stage-3 id | What |
|---|---|---|
| T1700 | B3 | Jobs in kiln-plugin: worker stores without `init`, the slice through the call hook |
| T1702 | B7 | The plugin data store |
| T1704 | B6 | Jobs in the app: fe-workbench runs worker stores through the cache |
| T1705 | A3 | The panels model |
| T1708 | A4 | Rendering panels in docks |
| T1709 | A7 | Plugins in the signed index |

C1 (the 1.1 minor), K1 (the stage-3 capabilities) and B1 (the tier model) have
landed (§1), so no card below waits for them.

**Does this design need T1705 to land first? No.** The page surface is a new
surface beside panels, not a panels node, and no viewer card uses the panels
WIT. Two cards (V2 and V11) touch files T1705 also names
(`crates/fe-plugin/wit/world.wit`, `crates/fe-plugin/src/lib.rs`,
`kiln/crates/kiln-plugin/src/manifest.rs`). They queue behind it, or it behind
them, as ordinary scope conflicts: whichever lands second rebases. Only V11,
the optional outline panel, uses panels, so only V11 waits for T1705, and for
T1708 too.

Host: **Linux-OK** means the work and its gate need no Mac. **Mac-only** means
a gate step needs the Mac (a Metal screenshot golden).

| # | Id | Size | What | Scope (globs) | Depends on | Host | Must wait for T1705? |
|---|---|---|---|---|---|---|---|
| 1 | **A** | s | **The native PDF notice, the fallback** (§8): `Opened::Pdf` through `claim_matches` and the built-in PDF claim (`.pdf` in any case, **or** `%PDF-` within 1024 bytes, so `paper.bin` and an extensionless PDF count; `pdf` on core's document-extension list, so an all-ASCII `.pdf` with a damaged header reaches the notice, tested), `PdfNotice` with Open in Default App and Open as Hex and an empty slot, kind `fastedit.pdf_notice` restored, and `workbench.open_in_default_app` in the editor, image, hex and notice contexts. | `crates/fe-workbench/src/loader.rs`, `crates/fe-workbench/src/pane.rs`, `crates/fe-workbench/src/workbench.rs`, `crates/fe-workbench/src/workbench_commands.rs`, `crates/fe-workbench/src/command_catalog_tests.rs`, `crates/fe-workbench/src/tests/mod.rs`, `crates/fe-workbench/src/tests/pdf_notice.rs`, `crates/fe-workbench/l10n/en-US/commands.ftl`, `crates/fe-gpui/src/pdf_notice.rs`, `crates/fe-gpui/src/pdf_notice/**`, `crates/fe-gpui/src/lib.rs`, `crates/fe-gpui/l10n/en-US/pdf.ftl`, `crates/fastedit/src/main.rs` | — | Linux-OK | No |
| 2 | **V0** | s | **The spike**: hayro as a WASI viewer component, measured as §7 lists, in a decision record. The harness is not committed. | `docs/decisions/*-pdf-renderer-spike.md` | — | Linux-OK | No |
| 3 | V1 | s | Capabilities `viewers` and `clipboard` in kiln-command's `Capability` (parse, display, serde, `Grants::allows`, escalation), their prompt wording, and the read-plus-net warning. | `kiln/crates/kiln-command/src/capability.rs`, `kiln/crates/kiln-command/tests/capability_viewers.rs`, `kiln/crates/kiln-plugin/src/warnings.rs`, `kiln/crates/kiln-plugin/src/grants.rs`, `crates/fe-workbench/src/plugin_prompt.rs`, `crates/fe-workbench/src/plugin_prompt_tests.rs`, `crates/fe-workbench/l10n/en-US/extensions.ftl` | — | Linux-OK | No |
| 4 | V2 | m | The viewer model: `viewer-document`, `viewer-session` and `viewer-handler` WIT (§3, §4), `[[contributes.plugin.viewers]]` parsed and validated (§2, image extensions refused), and a headless checker for results (tile sizes, boxes, links, caps). | `crates/fe-plugin/wit/viewers.wit`, `crates/fe-plugin/wit/world.wit`, `crates/fe-plugin/src/viewers.rs`, `crates/fe-plugin/src/lib.rs`, `crates/fe-plugin/tests/viewers.rs`, `kiln/crates/kiln-plugin/src/manifest.rs`, `kiln/crates/kiln-plugin/tests/manifest.rs` | V1 | Linux-OK | No (shares `world.wit`, `lib.rs` and `manifest.rs` with T1705: scope order only) |
| 5 | V3 | m | Viewer sessions in kiln-plugin (§3): a long-lived worker store per document with no preopens, the `document` resource over ranged reads, the absolute per-call deadline (10 s, 30 s for `open`) set once at dispatch with no resetting call hook, the 1 s cancellation grace and forced interrupt, one call in flight with a host queue, `cancelled()` and the source and session generations on every request, 4 per plugin and 8 in all, and the session pool (min(1 GiB, ⅛ of RAM), apart from the jobs total) with admission by evicting idle sessions in arrival order. Hostile-guest tests: a guest looping on `cancelled()` is interrupted at the deadline, one ignoring a cancel is interrupted after the grace, a third session on a full pool is admitted once the others' calls end, and a waiter is admitted within the 11 s bound while the resident sessions keep their queues continuously full (victims marked at the request boundary, their queued calls dropped, a just-admitted session not a victim before its first call ends). | `kiln/crates/kiln-plugin/src/session.rs`, `kiln/crates/kiln-plugin/src/lib.rs`, `kiln/crates/kiln-plugin/src/runner.rs`, `kiln/crates/kiln-plugin/src/limits.rs`, `kiln/crates/kiln-plugin/tests/session.rs`, `kiln/crates/kiln-plugin/tests/guests/viewer/**` | V2, T1700 | Linux-OK | No |
| 6 | V4 | m | Resolution and the tab (§2): the resolver order with installed claims matched by Card A's `claim_matches` (tests: an extensionless PDF, `paper.bin`, `PAPER.PDF` and an all-ASCII `.pdf` with a damaged header open in an installed viewer; a text file is taken by magic only; image extensions never), Open With's matching and Other Viewers… lists, `files.viewers` in settings, `workbench.open_with`, kind `fastedit.plugin_viewer` restored (falling back to the notice), sessions wired through fe-workbench's worker-store path, and uninstall, disable and crash swapping to the notice (§6, §8). | `crates/fe-workbench/src/loader.rs`, `crates/fe-workbench/src/viewer_resolve.rs`, `crates/fe-workbench/src/viewer_resolve_tests.rs`, `crates/fe-workbench/src/plugin_viewer.rs`, `crates/fe-workbench/src/plugin_viewer_tests.rs`, `crates/fe-workbench/src/pane.rs`, `crates/fe-workbench/src/workbench_commands.rs`, `crates/fe-workbench/src/command_catalog_tests.rs`, `crates/fe-workbench/src/plugin_manage.rs`, `crates/fe-config/src/settings.rs`, `crates/fe-config/settings.schema.json`, `crates/fe-config/settings-template.toml`, `crates/fe-config/tests/viewers_setting.rs` | A, V3, T1704 | Linux-OK | No |
| 7 | V5 | l | The page surface in fe-gpui (§3): page layout, 512-pixel tiles, request order with low-resolution pages and prefetch, the 256 MiB cache and eviction keyed by source (file, document generation, renderer = viewer, version, component SHA-256), the source transaction (last-good display kept while a pending source opens, superseded results rejected, the old source retired only after the new `open` succeeds; tests: a failed reload of a half-written file keeps the last-good tiles, selection and Find, and a second change while one is pending wins), results accepted only for the current or pending source and session generation, settle-then-rerender zoom and pinch, HiDPI and display moves, result checks, AccessKit pages, `viewers.invert_in_dark`, a screenshot golden, and the release binary's size before and after. | `crates/fe-gpui/src/plugin_viewer.rs`, `crates/fe-gpui/src/plugin_viewer/**`, `crates/fe-gpui/src/lib.rs`, `crates/fe-gpui/l10n/en-US/viewer.ftl`, `crates/fe-gpui/tests/plugin_viewer.rs`, `crates/fe-workbench/tests/screenshots/plugin_viewer*` | V4 | **Mac-only** (Metal screenshot golden; the rest is tested on Linux) | No |
| 8 | V6 | m | Input, selection, copy, links and Find (§4): host keys and Go to Page, selection over `layout` boxes, Copy under `clipboard`, external links by `lsp_links.rs`'s rule, internal jumps on the jump list, and Find over lazily fetched page text with its 32 MiB cache. | `crates/fe-gpui/src/plugin_viewer/**`, `crates/fe-gpui/tests/plugin_viewer_select.rs`, `crates/fe-workbench/src/plugin_viewer.rs`, `crates/fe-workbench/src/plugin_viewer_tests.rs`, `crates/fe-keymap/keymaps/fastedit.toml` | V5, V1 | Linux-OK | No |
| 9 | V8 | s | Viewer claims in the plugin index (§6): the sibling signed `plugins.json` (or T1709's entry if it settled otherwise, Q1), the claim fields, the publisher generating them from the manifest, and install checking them against the downloaded manifest. | `crates/fe-registry/src/plugins.rs`, `crates/fe-registry/src/lib.rs`, `crates/fe-registry/tests/plugin_viewers.rs`, `crates/fastedit/src/grammar_registry.rs`, `crates/fe-workbench/src/extension_install.rs` | T1709 | Linux-OK | No |
| 10 | V7 | m | Install on open (§5): `offer_prompt.rs` factored out of `grammar_prompt.rs` with its tests unchanged, `viewer_prompt.rs` with the notice's offer row for the built-in PDF claim and a toast offer over the hex or editor tab for any other signed-index claim that `claim_matches` (which keeps opening as today), the grant prompt before any download, `viewer_prompt.json`, progress and Cancel, failures with Retry, Don't ask keyed by extension or, for a magic-only claim, by claim identity (`never_claims`, with a restart test), the revalidated in-place swap (same path and kind, still matching, not chosen by hand, not dirty), and tests: editing a magic-matched text file while the plugin downloads leaves the dirty editor and its edits intact with an Open in *Plugin* that refuses until saved, and the no-network-before-both-answers test. | `crates/fe-workbench/src/offer_prompt.rs`, `crates/fe-workbench/src/grammar_prompt.rs`, `crates/fe-workbench/src/viewer_prompt.rs`, `crates/fe-workbench/src/viewer_prompt_tests.rs`, `crates/fe-workbench/src/lib.rs`, `crates/fe-gpui/src/pdf_notice/**`, `crates/fastedit/src/main.rs`, `crates/fastedit/src/plugin_install.rs` | A, V4, V8 | Linux-OK | No |
| 11 | V9 | l | **`tenmoku.pdf`** (§7): hayro (or V0's pick) behind `viewer-session`, `layout` from the renderer's glyphs, outline, passwords, its prebuilt component and `build.zsh`, `deny.toml` bans on the renderers, and an end-to-end test over fixtures (text page, image page, encrypted, links, hostile files) checking pixel hashes, text, copy and fallbacks. | `extensions/plugins/pdf/**`, `crates/fe-workbench/tests/first_party_pdf.rs`, `crates/fe-workbench/tests/fixtures/pdf/**`, `deny.toml` | V0, V2, V3, V6 | Linux-OK | No |
| 12 | V10 | s | Publish `tenmoku.pdf` to the registry: the release step that builds the archive, its entry and claims in `plugins.json`, and the docs page. Filed only once stage 3's Q4 (registry review) is answered and the counsel review of the accepted record §6 is done. | `.github/workflows/plugins.yml`, `scripts/build-plugin-index.sh`, `scripts/publish-plugin-index.sh`, `docs/release/plugin-registry.md` | V8, V9 | Linux-OK | No |
| 13 | V11 | s | *(Optional, later)* The document outline in a side panel through the panels model, with `navigation`-like page jumps. | `extensions/plugins/pdf/**`, `crates/fe-workbench/src/plugin_viewer.rs` | V9, T1705, T1708 | Linux-OK | **Yes** (and T1708) |

**Not depended on:** T1702 (the data store). A viewer keeps nothing on disk.
Its caches are the host's, in memory. If a later card wants persistent
thumbnails, it would depend on T1702 then.

**Order.**

1. **Card A first**, alone: it is useful today and every later card builds on
   its slot.
2. **V0 and V1** can run beside A, or right after it.
3. **V2**, then **V3** once T1700 lands.
4. **V4** once V3, A and T1704 are in.
5. **V5**, then **V6**.
6. **V8** after T1709.
7. **V7** after A, V4 and V8.
8. **V9** after V0 passes and V6 is in.
9. **V10** after V9 and the registry policy.
10. **V11** whenever T1705 and T1708 are in, if the user wants it.

**Decision:** thirteen cards: six s, five m and two l (V5 and V9). Card A,
the native PDF notice, is the first card and the permanent fallback. Only V5
is Mac-only. None needs T1705 first except the optional V11. Nothing is filed
until the user approves.

## Open questions for the user

1. **Q1, the index shape** (§6): plugin entries in a sibling signed
   `plugins.json`, so apps already shipped keep reading `index.json`, as
   recommended to T1709? Or a schema-2 `index.json` with a transition release
   that reads both?
2. **Q2, `net` for viewers** (§9): allowed behind the combined warning, as
   proposed, or refused for any plugin claiming file types?
3. **Q3, beyond viewing** (§1, §4): is view-only the permanent scope, or
   should form filling and annotations be a later design (pointer events to
   the plugin, writes back to the file under `fs.write`)?
4. **Q4, after V0** (§7): if hayro misses a threshold, do we spike PDFium for
   WASI (a C++ port), or relax the threshold it missed?

**Decision:** this record is **proposed**. The user rules on it and on Q1–Q4.
Card A and V0 can be filed first on approval. The rest follow the order of §10.
