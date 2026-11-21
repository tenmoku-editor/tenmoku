---
task_uid: "01m7qpg3zg7e2vgskg1gmjce2n"
task: "T427"
rev: 1
type: "docs"
complexity: "m"
criteria:
  - id: "C1"
    text: "docs/decisions/\u003clane\u003e-plugin-viewers.md exists with the ten sections above, each ending in a decision or a named open question; section 1 states that PDF rendering ships only as a plugin, never in core, for size, and that images stay native."
    required: true
    manual: false
  - id: "C2"
    text: "Section 7's survey table has a size, licence (Apache-2.0 compatibility stated), measured or flagged-unmeasured speed and feature coverage per renderer, and names a recommendation and a spike card."
    required: true
    manual: false
  - id: "C3"
    text: "The breakdown lists each implementation card with size, dependencies by handle, Linux-OK or Mac-only and scope paths; names Card A (the native PDF notice) as the fallback and the first card; and states whether the design needs T1705 to land first."
    required: true
    manual: false
  - id: "C4"
    text: "The status line says 'proposed' and names the user as the approver; no implementation card is filed by this card."
    required: true
    manual: false
---

# Design custom viewers from plugins: file-type claims, pixel surface, install-on-open, signed-index distribution, first-party PDF plugin (docs/decisions/<lane>-plugin-viewers.md) (Mac T1956)

## Ask

The user asked: "Can we package a pdf viewer as a plugin that we prompt them to download when opening a pdf?" and, after research, approved a design card. Their steer: "the PDF viewer is big enough and niche enough that it makes more sense to package as a plugin rather than as part of the core and make it more bloated." Images stay native in core (they open natively today and keep doing so).

## Deliverable

`docs/decisions/<lane>-plugin-viewers.md`, a proposal extending the approved plugins stage-3 design (docs/decisions/dqsjfernkt-plugins-stage-3.md: tiers §3.1-3.2, panels §4.2, signed index §4.6, core vs plugin §5). The user rules on it. No code, no Cargo changes, no card filed by this card.

Sections, each ending in a decision or a named open question:

1. **What was read and surveyed.** Plus the non-goals, stated first: **PDF rendering ships only as a plugin and never in core**, because it is big and niche and core must stay small; images stay native in core and are not moved to plugins; no webviews.
2. **Claiming file types.** A manifest contribution `[[contributes.plugin.viewers]]` (id, title, icon, extensions, optional magic bytes with offset, optional priority) for an editor-area tab view; how the host resolves a file to a viewer (core viewers first, then installed plugins, then the install prompt); how Open As / the "Open as Hex" and "Open in Default App" escape hatches coexist; session restore of a plugin viewer tab.
3. **The pixel surface.** The panels model (§4.2) is a declarative tree with no pixels, so a page view needs a new node or a new surface. Decide: a host-owned scroll/zoom surface the plugin feeds tiles or pages into (`viewer.set-document` metadata, `render-tile(page, rect, scale)` returning RGBA or an encoded image), with scrolling, zoom, HiDPI scale, visible-tile requests, host cache and eviction, cancellation of stale requests, memory and quota limits (cache ceiling, tile size, result size against the 16 MiB result and 64 MiB / 256 MiB store limits of §3). Which tier renders: the interactive tier's 100 ms call deadline and 64 MiB cap versus a job's worker store, and in particular that §3.2 jobs are one-shot instances, so re-parsing the document per tile is a cost; propose a persistent render session or a document handle and measure it against the survey in section 8. Accessibility (page text as a labelled node tree) and theming (page background, dark-mode invert as a host option).
4. **Input, selection and text copy.** How pointer, wheel, pinch and keys reach the plugin (`viewer-event`), what the host owns (zoom, scroll, focus, find bar), text selection and copy from a PDF (the plugin returns glyph boxes and text per page; the host draws the selection and puts text on the clipboard under a `clipboard` grant), links (open external through the host's checked path, internal jumps), Find in document.
5. **Install on open.** Generalise `crates/fe-workbench/src/grammar_prompt.rs` (docs/decisions/pp118n3cn5-grammar-prompt.md, mmjrn0hj4f-grammar-prompt-packs.md): a file-type to plugin map from the signed index, a toast or tab prompt on open with Install, Not now and Don't ask again (a per-file-type never list in the state directory, as the grammar Never file), what the user sees while it downloads and after it installs (the tab re-opens in the viewer), offline and failed installs, and no network without the prompt answered.
6. **Distribution.** Plugin entries in the signed index (T1709, §4.6): the viewer claim fields in the index entry so the prompt needs no download first, size shown in the prompt, hash-pinned artifacts, verification, updates, uninstall (and what a removed plugin does to open tabs).
7. **The first user: a first-party PDF plugin.** Survey wasm-capable renderers with a table of: artifact size (the binary-size budget: the plugin's .wasm and installed size, and the claim that core grows by zero bytes of renderer), licence against Apache-2.0 (MuPDF is AGPL or commercial, which rules it out unless that changes; PDFium is BSD-3 and Apache-2.0; pdf.js is Apache-2.0 but JS; pure-Rust candidates such as hayro, with their licences read from the repos), speed per page on a text page and an image page measured under wasmtime with the plugin limits, feature coverage (fonts, encryption, forms, annotations), build target (wasm32-wasip2 component, SIMD, threads). Say which spike to run and which to recommend; mark what was measured and what was not.
8. **Fallback.** With no plugin installed, or Don't ask chosen, or the plugin disabled or failed, a PDF opens in the native "This is a PDF" view with Open in Default App and Open as Hex (Card A, `Opened::Pdf`, `PdfNotice`), which is the slot the prompt and the plugin view replace. A plugin viewer that crashes or overruns falls back to it with a message.
9. **Security and limits.** Untrusted file parsed in a sandbox with its limits; hostile PDFs; the capability list for a viewer plugin (`viewers`, `clipboard`, reads of the opened file only).
10. **Proposed card breakdown** with size (xs..xl), dependencies (the stage-3 cards by handle: T1705 panels model, T1708 panel rendering, T1709 plugins in the signed index, T1702 plugin data store, T1704 jobs in the app, T1700 jobs in kiln-plugin), Linux-OK or Mac-only, and scope globs, in order. Say whether each card must wait for T1705.

## Criteria

- C1 docs/decisions/<lane>-plugin-viewers.md exists with the ten sections above, each ending in a decision or a named open question; section 1 states that PDF rendering ships only as a plugin, never in core, for size, and that images stay native.
- C2 Section 7's survey table has a size, licence (Apache-2.0 compatibility stated), measured or flagged-unmeasured speed and feature coverage per renderer, and names a recommendation and a spike card.
- C3 The breakdown lists each implementation card with size, dependencies by handle, Linux-OK or Mac-only and scope paths; names Card A (the native PDF notice) as the fallback and the first card; and states whether the design needs T1705 to land first.
- C4 The status line says 'proposed' and names the user as the approver; no implementation card is filed by this card.

## Out of scope

Any Rust code, Cargo or WIT changes; moving images or any existing viewer into a plugin; filing the breakdown cards.

## Conflicts

Docs only: none with running tasks. It extends T1705 (panels model), T1708 and T1709, which are ready and unaffected.
