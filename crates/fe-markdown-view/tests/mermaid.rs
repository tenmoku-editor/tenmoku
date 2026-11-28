//! ```` ```mermaid ```` fences: drawn as diagrams in the view, as code with
//! a note when they can't be, and as code everywhere else.

use std::{path::Path, sync::Arc};

use fe_gpui::{EditorPalette, EditorSettings, EditorTheme};
use fe_markdown_view::{
    ElementOptions, MarkdownStyle, MarkdownView, document_element, markdown_element, mermaid_svg,
};
use gpui::{
    Bounds, Context, Entity, IntoElement, ParentElement as _, Pixels, Render, Rgba, TestAppContext,
    VisualTestContext, Window, div, px,
};
use kiln_ui::{Theme, UiTheme};

const FLOWCHART: &str =
    "```mermaid\nflowchart TD\n  A[Start] --> B{Ok?}\n  B -->|yes| C[Done]\n```\n";
const SEQUENCE: &str =
    "```Mermaid\nsequenceDiagram\n  Alice->>Bob: Hello\n  Bob-->>Alice: Hi\n```\n";
const RUST: &str = "```rust\nfn main() {}\n```\n";

fn style(cx: &mut TestAppContext, dark: bool) -> MarkdownStyle {
    cx.update(|cx| {
        let (palette, ui) = if dark {
            (EditorPalette::fastedit_dark(), UiTheme::default_dark())
        } else {
            (EditorPalette::fastedit_light(), Theme::light().ui_theme())
        };
        let editor = EditorTheme::from_palette(palette, &EditorSettings::default(), cx);
        MarkdownStyle::new(&editor, &ui)
    })
}

/// A view of `markdown` in a test window, drawn once whatever it loads has
/// landed.
fn open<'a>(
    cx: &'a mut TestAppContext,
    markdown: &str,
) -> (Entity<MarkdownView>, &'a mut VisualTestContext) {
    let style = style(cx, true);
    let document = Arc::new(fe_markdown::parse(markdown));
    let doc = Path::new("/doc.md").to_owned();
    let (view, cx) =
        cx.add_window_view(move |_, cx| MarkdownView::new(document, doc, None, style, cx));
    settle(cx);
    (view, cx)
}

fn settle(cx: &mut VisualTestContext) {
    for _ in 0..3 {
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));
    }
}

fn bounds(cx: &mut VisualTestContext, selector: String) -> Option<Bounds<Pixels>> {
    cx.debug_bounds(selector.leak())
}

#[gpui::test]
fn flowchart_and_sequence_fences_draw_diagrams(cx: &mut TestAppContext) {
    let (_view, cx) = open(cx, &format!("{FLOWCHART}\n{SEQUENCE}\n{RUST}"));
    for n in [0, 1] {
        let diagram = bounds(cx, format!("md-diagram-{n}"))
            .unwrap_or_else(|| panic!("fence {n} is drawn as a diagram"));
        assert!(
            diagram.size.width > px(0.) && diagram.size.height > px(0.),
            "diagram {n} has a size: {diagram:?}"
        );
        assert_eq!(bounds(cx, format!("md-code-{n}-text")), None, "fence {n}");
        assert_eq!(bounds(cx, format!("md-diagram-{n}-error")), None);
    }
    assert!(
        bounds(cx, "md-code-2-text".into()).is_some(),
        "Rust stays code"
    );
    assert_eq!(bounds(cx, "md-diagram-2".into()), None);
}

#[gpui::test]
fn broken_unsupported_and_oversized_mermaid_show_the_code_and_a_note(cx: &mut TestAppContext) {
    let broken = "```mermaid\nflowchart TD\n  A --> (((\n```\n";
    let zenuml = "```mermaid\nzenuml\n  Alice->Bob: Hello\n```\n";
    let nodes: String = (0..5000)
        .map(|n| format!("  N{n} --> N{}\n", n + 1))
        .collect();
    let oversized = format!("```mermaid\nflowchart TD\n{nodes}```\n");
    assert!(oversized.len() > 64 * 1024);
    let markdown = format!("{broken}\n{zenuml}\n{oversized}\nAfter the diagrams.\n");
    let (view, cx) = open(cx, &markdown);
    for n in 0..3 {
        assert!(
            bounds(cx, format!("md-code-{n}-text")).is_some(),
            "fence {n} shows its code"
        );
        let note = bounds(cx, format!("md-diagram-{n}-error"))
            .unwrap_or_else(|| panic!("fence {n} has a note"));
        assert!(note.size.height > px(0.));
        assert_eq!(bounds(cx, format!("md-diagram-{n}")), None);
    }
    let last = view.read_with(cx, |view, _| view.document().blocks.len() - 1);
    let paragraph = bounds(cx, format!("md-block-{last}")).expect("the paragraph is drawn");
    assert!(paragraph.size.height > px(0.));
}

#[gpui::test]
fn mermaid_svg_follows_the_style(cx: &mut TestAppContext) {
    let source = "flowchart LR\n  A[One] --> B[Two]\n";
    let hex = |style: &MarkdownStyle| {
        let rgba = Rgba::from(style.foreground);
        let byte = |channel: f32| (channel * 255.).round() as u8;
        format!(
            "#{:02x}{:02x}{:02x}",
            byte(rgba.r),
            byte(rgba.g),
            byte(rgba.b)
        )
    };
    let mut colours = Vec::new();
    for dark in [true, false] {
        let style = style(cx, dark);
        let svg = mermaid_svg(source, &style).expect("a flowchart draws");
        assert!(!svg.contains("<foreignObject"), "plain text only");
        let foreground = hex(&style);
        assert!(
            svg.to_ascii_lowercase().contains(&foreground),
            "the SVG has the foreground {foreground}"
        );
        colours.push(foreground);
    }
    assert_ne!(colours[0], colours[1], "dark and light differ");
}

/// A compact element and a document element of one document, side by side.
struct Hosts {
    document: Arc<fe_markdown::Document>,
    style: MarkdownStyle,
    compact: bool,
}

impl Render for Hosts {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let element = if self.compact {
            markdown_element(&self.document, &self.style, px(400.), None)
        } else {
            document_element(&self.document, &self.style, ElementOptions::default())
        };
        div().child(element)
    }
}

#[gpui::test]
fn hovers_and_document_elements_keep_mermaid_as_code(cx: &mut TestAppContext) {
    let style = style(cx, true);
    let document = Arc::new(fe_markdown::parse(FLOWCHART));
    for compact in [true, false] {
        let (document, style) = (document.clone(), style.clone());
        let (_host, cx) = cx.add_window_view(move |_, _| Hosts {
            document,
            style,
            compact,
        });
        settle(cx);
        assert!(
            bounds(cx, "md-code-0-text".into()).is_some(),
            "compact {compact}: the fence is code"
        );
        assert_eq!(bounds(cx, "md-diagram-0".into()), None, "compact {compact}");
    }
}
