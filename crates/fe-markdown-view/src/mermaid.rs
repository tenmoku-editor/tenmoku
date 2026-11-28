//! ```` ```mermaid ```` fences drawn as diagrams: the source to SVG through
//! merman, in the preview's colours, and the SVG to an image through GPUI's
//! SVG renderer, off the main thread, as local images are.
//!
//! merman, and the MPL-2.0 CSS parser it needs, are built only with the
//! `mermaid` feature, which is off by default. Without it a Mermaid fence
//! is drawn as code, as any other.

use gpui::{Div, ParentElement as _, SharedString, Styled as _, div};

use crate::MarkdownStyle;

#[cfg(feature = "mermaid")]
pub use engine::{DiagramError, mermaid_svg};
#[cfg(feature = "mermaid")]
pub(crate) use engine::{DiagramSource, forget, render};

/// A diagram drawn: none without the `mermaid` feature.
#[cfg(not(feature = "mermaid"))]
pub(crate) type DiagramSource = std::convert::Infallible;

/// Drops `gone`'s image from GPUI's cache: there are none without the
/// `mermaid` feature.
#[cfg(not(feature = "mermaid"))]
pub(crate) fn forget(gone: &DiagramSource, _: &mut gpui::App) {
    match *gone {}
}

/// The Mermaid fence `code` as code: without the `mermaid` feature,
/// diagrams aren't drawn.
#[cfg(not(feature = "mermaid"))]
pub(crate) fn render(_code: &str, _cx: &mut crate::view::RenderCx) -> Drawn {
    Drawn::Code
}

/// Whether a fence's info string names Mermaid: its first word, in any
/// case. Not by fe-syntax language, which needs the grammar installed.
pub(crate) fn is_mermaid(info: &str) -> bool {
    (info.split_whitespace().next()).is_some_and(|word| word.eq_ignore_ascii_case("mermaid"))
}

/// How a Mermaid fence is drawn this frame.
#[cfg_attr(
    not(feature = "mermaid"),
    expect(dead_code, reason = "only the feature draws diagrams")
)]
pub(crate) enum Drawn {
    /// As code: while it loads, or always when the `mermaid` feature is
    /// off.
    Code,
    /// The diagram in a code block's box, for the caller to give its
    /// `debug_selector`.
    Diagram(Box<Div>),
    /// As code, with this note under it.
    Failed(SharedString),
}

/// The note under a fence that isn't drawn as a diagram, saying why, one
/// muted line.
pub(crate) fn note(why: SharedString, style: &MarkdownStyle) -> Div {
    div()
        .text_color(style.muted)
        .text_size(style.text_size * 0.875)
        .whitespace_nowrap()
        .overflow_hidden()
        .text_ellipsis()
        .child(why)
}

/// merman: Mermaid source to SVG, and the SVG to an image.
#[cfg(feature = "mermaid")]
mod engine {
    use std::{
        panic::{AssertUnwindSafe, catch_unwind},
        sync::Arc,
        time::Duration,
    };

    use gpui::{
        App, Asset, Hsla, ObjectFit, ParentElement as _, RenderImage, SMOOTH_SVG_SCALE_FACTOR,
        SharedString, Styled as _, StyledImage as _, div, img, px,
    };
    use kiln_l10n::tr;
    use merman::{
        MermaidEngine, OperationControl, RenderError, RenderOutput, RenderRequest, Renderer,
        SourcePresentationConfig, SvgEnvironment, SvgRequest,
        svg::{RenderResourcePolicy, ResourceLimitId, SvgPipeline},
    };

    use super::Drawn;
    use crate::{MarkdownStyle, view::RenderCx};

    /// The longest source drawn; a longer one is refused before merman reads it.
    const MAX_SOURCE_BYTES: usize = 64 * 1024;
    /// How long merman may take over one diagram.
    const DEADLINE: Duration = Duration::from_secs(2);
    /// The largest SVG merman may write for one diagram.
    const MAX_SVG_BYTES: usize = 4 * 1024 * 1024;
    /// The diagrams' text: a family GPUI's SVG renderer always resolves. It
    /// reads system fonts and its asset source's, not the fonts the app hands
    /// its text system, so the bundled UI family isn't among them.
    const FONT_FAMILY: &str = "sans-serif";

    /// Why a Mermaid diagram isn't drawn.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum DiagramError {
        /// A diagram type merman knows but this build doesn't draw, such as
        /// `zenuml`: merman's name for it.
        Unsupported { kind: String },
        /// merman couldn't parse or draw it: the first line of its message.
        Failed(String),
        /// It took longer, or wrote more, than a diagram may.
        OverBudget,
        /// The source is over 64 KiB, so it wasn't read.
        TooLarge,
        /// merman panicked drawing it.
        Panicked,
    }

    impl DiagramError {
        /// The one-line note shown under the code.
        pub fn note(&self) -> SharedString {
            match self {
                DiagramError::Unsupported { kind } => {
                    tr!("markdown-view-diagram-unsupported", kind = kind.as_str())
                }
                DiagramError::Failed(reason) => {
                    tr!("markdown-view-diagram-failed", reason = reason.as_str())
                }
                DiagramError::OverBudget => tr!("markdown-view-diagram-over-budget"),
                DiagramError::TooLarge => tr!("markdown-view-diagram-too-large"),
                DiagramError::Panicked => tr!("markdown-view-diagram-panicked"),
            }
        }
    }

    /// What a diagram is drawn in, from a [`MarkdownStyle`], as `#rrggbb`.
    #[derive(Clone, Debug, PartialEq, Eq, Hash)]
    pub(crate) struct DiagramColours {
        background: String,
        foreground: String,
        border: String,
        /// Around nodes: `muted`, since `border` hardly shows on `fill`.
        outline: String,
        /// Behind nodes, actors and notes.
        fill: String,
        accent: String,
    }

    impl DiagramColours {
        fn new(style: &MarkdownStyle) -> Self {
            let on = |colour: Hsla| hex(colour, style.background);
            DiagramColours {
                background: on(style.background),
                foreground: on(style.foreground),
                border: on(style.border),
                outline: on(style.muted),
                fill: on(style.code_background),
                accent: on(style.link),
            }
        }

        /// merman's site configuration: Mermaid's `base` theme with its
        /// `themeVariables` from these colours.
        fn site_config(&self) -> serde_json::Value {
            let DiagramColours {
                background,
                foreground,
                border,
                outline,
                fill,
                accent,
            } = self;
            serde_json::json!({
                "theme": "base",
                "fontFamily": FONT_FAMILY,
                "themeVariables": {
                    "darkMode": is_dark(background),
                    "fontFamily": FONT_FAMILY,
                    "background": background,
                    "primaryColor": fill,
                    "primaryTextColor": foreground,
                    "primaryBorderColor": outline,
                    "secondaryColor": fill,
                    "secondaryTextColor": foreground,
                    "secondaryBorderColor": outline,
                    "tertiaryColor": background,
                    "tertiaryTextColor": foreground,
                    "tertiaryBorderColor": outline,
                    "mainBkg": fill,
                    "nodeBorder": outline,
                    "nodeTextColor": foreground,
                    "textColor": foreground,
                    "titleColor": foreground,
                    "lineColor": foreground,
                    "edgeLabelBackground": fill,
                    "clusterBkg": background,
                    "clusterBorder": border,
                    "actorBkg": fill,
                    "actorBorder": accent,
                    "actorTextColor": foreground,
                    "actorLineColor": border,
                    "signalColor": foreground,
                    "signalTextColor": foreground,
                    "labelBoxBkgColor": fill,
                    "labelBoxBorderColor": border,
                    "labelTextColor": foreground,
                    "loopTextColor": foreground,
                    "noteBkgColor": background,
                    "noteBorderColor": accent,
                    "noteTextColor": foreground,
                    "activationBkgColor": fill,
                    "activationBorderColor": border,
                    "sequenceNumberColor": background,
                },
            })
        }
    }

    /// `colour` as `#rrggbb`, over `under` if it's translucent.
    fn hex(colour: Hsla, under: Hsla) -> String {
        let alpha = colour.a.clamp(0., 1.);
        let (top, under) = (gpui::Rgba::from(colour), gpui::Rgba::from(under));
        let byte = |top: f32, under: f32| {
            ((top * alpha + under * (1. - alpha)).clamp(0., 1.) * 255.).round() as u8
        };
        format!(
            "#{:02x}{:02x}{:02x}",
            byte(top.r, under.r),
            byte(top.g, under.g),
            byte(top.b, under.b),
        )
    }

    /// Whether `#rrggbb` is a dark colour.
    fn is_dark(hex: &str) -> bool {
        let channel = |at: usize| u8::from_str_radix(&hex[at..at + 2], 16).unwrap_or(0) as f32;
        let luma = 0.299 * channel(1) + 0.587 * channel(3) + 0.114 * channel(5);
        luma < 128.
    }

    /// Mermaid `code` as SVG in `style`'s colours: plain `<text>`, never a
    /// `<foreignObject>`, which GPUI's SVG renderer can't draw. Only
    /// flowcharts and sequence diagrams are drawn; merman gets two seconds and
    /// 4 MiB of SVG, and a source over 64 KiB isn't read.
    pub fn mermaid_svg(code: &str, style: &MarkdownStyle) -> Result<String, DiagramError> {
        render_svg(code, &DiagramColours::new(style))
    }

    fn render_svg(code: &str, colours: &DiagramColours) -> Result<String, DiagramError> {
        if code.len() > MAX_SOURCE_BYTES {
            return Err(DiagramError::TooLarge);
        }
        catch_unwind(AssertUnwindSafe(|| render_unguarded(code, colours)))
            .unwrap_or(Err(DiagramError::Panicked))
    }

    fn render_unguarded(code: &str, colours: &DiagramColours) -> Result<String, DiagramError> {
        let site = SourcePresentationConfig::from_value(colours.site_config());
        let renderer =
            Renderer::new().with_presentation_engine(MermaidEngine::new().with_site_config(site));
        let resources = RenderResourcePolicy::interactive()
            .with_limit(ResourceLimitId::MaxSvgBytes, MAX_SVG_BYTES)
            .map_err(|err| DiagramError::Failed(first_line(&err.to_string())))?;
        let request = SvgRequest {
            environment: SvgEnvironment::deterministic().with_resource_policy(resources),
            pipeline: Some(SvgPipeline::resvg_safe()),
            ..SvgRequest::default()
        };
        let control = OperationControl::new().with_deadline(DEADLINE);
        match renderer.render(RenderRequest::svg(code, control, request)) {
            Ok(RenderOutput::Svg(Some(svg))) => Ok(svg.svg().to_owned()),
            Ok(_) => Err(DiagramError::Failed(first_line(
                &RenderError::NoDiagram.to_string(),
            ))),
            Err(err) => Err(classify(&err)),
        }
    }

    /// What a merman error means for the note.
    fn classify(err: &RenderError) -> DiagramError {
        match err {
            RenderError::Cancelled(_) | RenderError::ResourceLimitExceeded(_) => {
                DiagramError::OverBudget
            }
            RenderError::Parse(diagnostic) if diagnostic.is_resource_limit() => {
                DiagramError::OverBudget
            }
            RenderError::Parse(diagnostic) => {
                let details = diagnostic.terminal_diagnostic_details();
                match details.diagram_type {
                    Some(kind) if details.code == "merman.parse.unsupported_diagram" => {
                        DiagramError::Unsupported { kind }
                    }
                    _ => DiagramError::Failed(first_line(&diagnostic.terminal_safe_message())),
                }
            }
            err => DiagramError::Failed(first_line(&err.to_string())),
        }
    }

    fn first_line(message: &str) -> String {
        message.lines().next().unwrap_or_default().trim().to_owned()
    }

    /// A diagram to draw: its source and what it's drawn in.
    #[derive(Clone, Debug, PartialEq, Eq, Hash)]
    pub(crate) struct DiagramSource {
        code: Arc<str>,
        colours: DiagramColours,
    }

    /// A drawn diagram, and how many of its pixels a logical pixel is.
    #[derive(Clone)]
    pub(crate) struct Diagram {
        image: Arc<RenderImage>,
        scale: f32,
    }

    /// Draws a [`DiagramSource`]: merman, then GPUI's SVG renderer, on the
    /// background executor. GPUI caches the result per source until
    /// [`crate::MarkdownView`] removes the ones it no longer draws.
    pub(crate) enum DiagramLoader {}

    impl Asset for DiagramLoader {
        type Source = DiagramSource;
        type Output = Result<Diagram, DiagramError>;

        fn load(
            source: Self::Source,
            cx: &mut App,
        ) -> impl Future<Output = Self::Output> + Send + 'static {
            let renderer = cx.svg_renderer();
            async move {
                let svg = render_svg(&source.code, &source.colours)?;
                let image = catch_unwind(AssertUnwindSafe(|| {
                    renderer.render_single_frame(svg.as_bytes(), 1.0)
                }))
                .map_err(|_| DiagramError::Panicked)?
                .map_err(|err| DiagramError::Failed(first_line(&err.to_string())))?;
                Ok(Diagram {
                    image,
                    // GPUI draws an SVG at this many pixels a point.
                    scale: SMOOTH_SVG_SCALE_FACTOR,
                })
            }
        }
    }

    /// The Mermaid fence `code` as a diagram in a code block's box, once
    /// drawn. It's recorded as drawn, so the view can drop the diagrams it no
    /// longer shows.
    pub(crate) fn render(code: &str, cx: &mut RenderCx) -> Drawn {
        let style = cx.style;
        let source = DiagramSource {
            code: code.into(),
            colours: DiagramColours::new(style),
        };
        let loaded = cx.window.use_asset::<DiagramLoader>(&source, cx.app);
        cx.tracked.borrow_mut().diagrams.push(source);
        match loaded {
            None => Drawn::Code,
            Some(Err(err)) => Drawn::Failed(err.note()),
            Some(Ok(diagram)) => {
                let size = diagram.image.size(0);
                let width = size.width.0 as f32 / diagram.scale;
                Drawn::Diagram(Box::new(
                    div()
                        .w_full()
                        .flex()
                        .flex_col()
                        .items_center()
                        .rounded(px(4.))
                        .bg(style.code_background)
                        .p(style.code_padding)
                        .child(
                            img(diagram.image)
                                .w(px(width))
                                .max_w_full()
                                .object_fit(ObjectFit::ScaleDown),
                        ),
                ))
            }
        }
    }

    /// Drops `gone`'s image from GPUI's cache.
    pub(crate) fn forget(gone: &DiagramSource, cx: &mut App) {
        cx.remove_asset::<DiagramLoader>(gone);
    }
}
