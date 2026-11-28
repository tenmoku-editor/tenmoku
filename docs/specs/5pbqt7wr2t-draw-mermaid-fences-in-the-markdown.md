---
task_uid: "01m8ew2nyxbg7a9s5pbqt7wr2t"
task: "T2181"
rev: 3
type: "feature"
complexity: "m"
criteria:
  - id: "C1"
    text: "`flowchart_and_sequence_fences_draw_diagrams` (crates/fe-markdown-view/tests/mermaid.rs): in a MarkdownView, a ```mermaid flowchart fence and a ```Mermaid sequence fence each draw `md-diagram-\u003cn\u003e` with a non-zero size and no `md-code-\u003cn\u003e-text`; a ```rust fence beside them still draws its code text"
    required: true
    manual: false
  - id: "C2"
    text: "`broken_unsupported_and_oversized_mermaid_show_the_code_and_a_note` (crates/fe-markdown-view/tests/mermaid.rs): a parse error, a `zenuml` diagram and a source over 64 KiB each draw `md-code-\u003cn\u003e-text` plus `md-diagram-\u003cn\u003e-error`, and the paragraph after them still draws"
    required: true
    manual: false
  - id: "C3"
    text: "`mermaid_svg_follows_the_style` (crates/fe-markdown-view/tests/mermaid.rs): `fe_markdown_view::mermaid_svg` for the dark and light MarkdownStyle gives SVG with no `\u003cforeignObject\u003e`, containing each style's foreground colour, different between the two"
    required: true
    manual: false
  - id: "C4"
    text: "`hovers_and_document_elements_keep_mermaid_as_code` (crates/fe-markdown-view/tests/mermaid.rs): a compact view and `document_element` draw a mermaid fence's code text and no `md-diagram-\u003cn\u003e`"
    required: true
    manual: false
  - id: "C5"
    text: "`cargo deny check licenses bans sources` passes with exactly three new MPL-2.0 exceptions in deny.toml (cssparser, cssparser-macros, dtoa-short; the user allowed them as unmodified dependencies, 2026-11-27, docs/decisions/5pbqt7wr2t-merman-mpl.md) and no other licence change; THIRD_PARTY.md appends rows for cssparser and dtoa-short to its shipped MPL-2.0 table and a build-time-only line for cssparser-macros like cbindgen's; `cargo tree -p fe-markdown-view -e features` lists no `layout-elk` or `merman-layout-elk`"
    required: true
    manual: false
  - id: "C6"
    text: "On the Mac: new scenes markdown_mermaid_dark and markdown_mermaid_light have reviewed goldens, and `cargo run -p fe-markdown-view --example screenshots` and `cargo run -p fe-workbench --example screenshots` pass with every existing golden unchanged"
    required: true
    manual: false
  - id: "C7"
    text: "merman is behind an optional `mermaid` feature of fe-markdown-view, off by default, so a crate embedding fe-markdown-view gets no MPL-2.0 code unless it asks (the user, 2026-11-27); fe-workbench turns it on for the app. `cargo tree -p fe-markdown-view -e normal` (default features) lists no merman, cssparser or dtoa-short; with the feature off a ```mermaid fence draws as code; tests/mermaid.rs runs with the feature on (required-features or cfg)"
    required: true
    manual: false
---

# Draw ```mermaid fences in the Markdown preview as diagrams: flowchart and sequence through merman, with the code and a note when one can't be drawn

## Problem

The user asked (2026-11-27) "Does this mean the markdown preview renders mermaid diagrams?" after T2133's Mermaid grammar. It doesn't: T2133 only highlights Mermaid syntax, and a ```mermaid fence in the preview is drawn as code. The user asked for a card to draw them as diagrams.

Today every fence goes through the `BlockKind::Code` arm of `render` in `crates/fe-markdown-view/src/blocks.rs` (about line 330): the code text as `StyledText`, or a host's `CodeBodyFn`. Images already have the seam this needs: `crates/fe-markdown-view/src/image.rs` loads a file through a GPUI `Asset` (`LocalImageLoader`), which runs on the background executor, is cached per source by GPUI, and turns SVG bytes into a `RenderImage` with `cx.svg_renderer().render_single_frame`, drawn by `img(..)` with `max_w_full` and `ObjectFit::ScaleDown`. Nothing in `crates/fe-markdown` or `crates/fe-markdown-view` knows Mermaid (`rg -i mermaid crates/fe-markdown*` finds nothing).

## Change

Render Mermaid source to SVG with the pure-Rust **merman** crate (0.11, MIT OR Apache-2.0, Latias94/merman; parity-targeted at Mermaid 12.3, no browser or JS engine) and show that SVG through the image path that exists.

- **Dependency.** In the root `Cargo.toml` `[workspace.dependencies]`: `merman = { version = "0.11", default-features = false, features = [...] }` with `svg` and only `diagram-flowchart` and `diagram-sequence`. Add a layout feature only if one of those needs it, and never `layout-elk` (the ELK layout is ported EPL-2.0 code, which deny.toml does not allow, whatever its crate metadata says). Use it from `crates/fe-markdown-view/Cargo.toml` as `merman.workspace = true`. deny.toml and THIRD_PARTY.md do not change (`cargo deny list` covers MIT/Apache crates).
- **`crates/fe-markdown-view/src/mermaid.rs` (new).** `pub fn mermaid_svg(code: &str, style: &MarkdownStyle) -> Result<String, DiagramError>`, exported from `lib.rs`: merman's `Renderer` with `SvgRequest.pipeline = SvgPipeline::resvg_safe()` (plain `<text>`, no `<foreignObject>`, which usvg can't draw), the `base` theme with `themeVariables` from the style (background, node fill = `code_background`, text and lines = `foreground`, borders = `border`, accent = `link`) and a font family GPUI's SVG renderer resolves (the bundled UI family), and an `OperationControl` budget (about 2 s and a bounded output size). The call is wrapped in `std::panic::catch_unwind`. `DiagramError` tells apart: unsupported diagram type (merman detects a type not compiled in), parse error (merman's message, first line), over budget, too large (source over 64 KiB, refused before merman sees it), and panicked. A private `Asset` (`DiagramLoader`) beside it, as `LocalImageLoader` is, keyed by the code and the style's diagram colours and font, runs `mermaid_svg` then `render_single_frame` on the background executor and gives `Result<(Arc<RenderImage>, scale), DiagramError>`.
- **`blocks.rs`.** In the `BlockKind::Code` arm, when the info string's first word is `mermaid` (ASCII case-insensitive; not by fe-syntax language, which depends on the grammar being installed), and only for `MarkdownView` (`element()` is `None` and `cx.compact` is false: hovers, chat replies and other `document_element` hosts keep drawing code): while loading, draw the code block as today; once loaded, draw the diagram in the code block's rounded `code_background` box, scaled down to the column, debug selector `md-diagram-<id>` (`id` from `cx.next_code()`, so it matches `md-code-<id>`); on any `DiagramError`, draw the code block as today plus a muted one-line note under it, `md-diagram-<id>-error`. A diagram never stops the rest of the document drawing.
- **`view.rs`.** Remember the diagram sources drawn; when `set_document` or `set_style` replaces them, `App::remove_asset::<DiagramLoader>` the ones no longer drawn, so editing a diagram doesn't grow the cache.
- **l10n.** `crates/fe-markdown-view/l10n/en-US/fe-markdown-view.ftl`: `markdown-view-diagram-unsupported` (with `$kind`) and `markdown-view-diagram-failed` (with `$reason`); merman's parse message is passed as the variable.
- **Tests.** One new file, `crates/fe-markdown-view/tests/mermaid.rs`, with the open/settle helpers as `tests/render.rs` has them; small samples, no sleeps.
- **Screenshots.** Two new scenes in `crates/fe-markdown-view/examples/screenshots/scenes.rs`, `markdown_mermaid_dark` and `markdown_mermaid_light`, with their own fixture (a five-node flowchart, a three-actor sequence, and one broken fence), and their new goldens. The existing `markdown_preview_*` fixture and goldens do not change.

Out of scope: other diagram types (the follow-up card), chat replies, hovers, T1985's Reading mode, zoom, pan, click-to-source and export.

## Rules

Do not widen --scope; if a file outside it is needed, stop and respec (notes go in your lane's `docs/decisions/<lane>-*.md`, which is exempt). New tests go in the new file, one fn per behaviour. Gate tests on fe-markdown-view (and fe-workbench's screenshots, below). Ask before any cargo build. The screenshot examples are not in the gate: run `cargo run -p fe-markdown-view --example screenshots` and `cargo run -p fe-workbench --example screenshots` on the Mac, and restore any golden that `UPDATE_SCREENSHOTS=1` rewrote other than the two new ones. The card is not `linux-ok`. In `## Implementation notes` name the merman features enabled, `cargo tree -p fe-markdown-view | wc -l` before and after, and anything merman drew badly through usvg.

## Implementation notes

- **merman features:** `default-features = false` with `svg`, `diagram-flowchart` and `diagram-sequence`. No layout feature: both diagrams lay out through dugong, which merman always builds. No `layout-elk` anywhere in `cargo tree --workspace -e features`.
- **`cargo tree -p fe-markdown-view | wc -l`:** 1362 before; 1513 after, which counts the dev-dependency that turns `mermaid` on; 1364 for the default normal tree (`-e normal`), which has no merman, cssparser or dtoa-short (C7).
- **Licences (C5):** merman hard-depends on Servo's MPL-2.0 cssparser 0.38, cssparser-macros 0.7.1 and dtoa-short 0.3.5. The user allowed them as unmodified dependencies (docs/decisions/5pbqt7wr2t-merman-mpl.md). deny.toml has one exception each; THIRD_PARTY.md has rows for cssparser and dtoa-short and a build-time-only line for cssparser-macros.
- **The `mermaid` feature (C7):** off by default; fe-workbench turns it on. `mermaid.rs` keeps a small always-built part (`is_mermaid`, `Drawn`, `note`) and puts merman in a `#[cfg(feature = "mermaid")] mod engine`. Without the feature, `render` gives `Drawn::Code` and `DiagramSource` is `Infallible`, so blocks.rs and view.rs need no cfg. A self dev-dependency (`fe-markdown-view = { path = ".", features = ["mermaid"] }`) turns it on for the tests and the screenshots example, so the C6 command needs no `--features`. `tests/mermaid.rs` also has `required-features = ["mermaid"]`. Clippy is clean with the feature off (`--lib`) and on (`--all-targets --features mermaid`).
- **Reused:** `image.rs`'s `LocalImageLoader` pattern: a GPUI `Asset` on the background executor, `svg_renderer().render_single_frame`, `SMOOTH_SVG_SCALE_FACTOR`, drawn by `img` with `max_w_full` and `ObjectFit::ScaleDown`. Also the code block's `code_background` box and padding, `cx.next_code()` ids, and the view's per-frame `Tracked` record, which carries the drawn diagrams so compact.rs is untouched. The note is a muted `div`, as image.rs's placeholder text is; kiln-ui has no muted one-line label to use. `tests/mermaid.rs`'s `open`/`settle`/`bounds` helpers are copies of `tests/render.rs:33-57`, as the spec asks. Their shared home would be a test-support module in fe-markdown-view's tests, which is out of scope.
- **Departures from the spec:**
  - The diagrams' font is `sans-serif`, not the bundled UI family. GPUI's SVG renderer reads system fonts and its asset source's (IBM Plex Sans, Lilex), not the fonts the app hands its text system, so Instrument Sans can't resolve there.
  - Node borders are `muted`, not `border`: `border` hardly shows on `code_background` (in the dark goldens nodes were nearly invisible). Lifelines, clusters and label boxes keep `border`; actors and notes use the accent.
  - Three messages beyond the two the spec names, so every note is translated: `markdown-view-diagram-over-budget`, `-too-large`, `-panicked`. `DiagramError::Failed` carries merman's first message line for parse errors and any other merman or usvg failure.
  - The `md-diagram-*` debug selectors are set in blocks.rs, which l10n-allow.toml allows file-wide, rather than in mermaid.rs, which would need allow entries.
  - Diagrams drawn before a frame and not in it are removed from GPUI's cache when the view renders, which covers `set_document` and `set_style`. If two views draw the same diagram, one dropping it makes the other reload it, briefly showing code.
- **What merman draws badly through usvg** (goldens markdown_mermaid_*):
  - Flowchart edge labels ("yes"/"no") sit at the edge's end, with the line running through the text; in light, the label's background box is offset from it.
  - Sequence actors have merman's drop shadow, which shows as a dark offset in the dark theme.
  - A parse error's note repeats merman's own prefix: "Can't draw this diagram: Diagram parse error (flowchart-v2): …".
