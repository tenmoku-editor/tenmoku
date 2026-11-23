//! A plugin panel's `grid` node (T2006's cells, drawn here, T2007): a
//! fixed board of monospace cells, each one grapheme in one of the six
//! tones, maybe reversed.
//!
//! - **One element a row.** Each row is one element painting its cells'
//!   text in the editor's font (its family, features and size,
//!   [`EditorTheme`]), so a 64 x 64 grid is 64 elements, not 4096. The
//!   cell is the font's: as wide as its advance of `m` (what the editor
//!   lays columns out by) and as tall as the editor's line, so the board is
//!   `cols` advances by `rows` lines. Each cell's grapheme is shaped alone
//!   and painted at its column, clipped to its cell, so no grapheme moves
//!   the next: one wider than the cell (CJK, an emoji) is shaped smaller to
//!   fit it, a narrower one (a lone combining mark) is centred in it, and
//!   one holding a control character (a newline) is drawn as a space. A
//!   reversed cell's ground fills the cell.
//! - **Tones.** Each tone is its theme token ([`tone_key`]) drawn on the
//!   panel, raised the least way toward white or black where it falls short
//!   of [`tone_floor`] against the panel: WCAG's AA for text, and its AAA
//!   under Increase Contrast. A reversed cell swaps the two, the panel's
//!   colour on the tone's, so it holds the same ratio.
//! - **Accessibility.** The grid is one image region named by its label:
//!   a screen reader reads the label, never 4096 cells, and no row or cell
//!   has a node of its own.

use fe_gpui::EditorTheme;
use fe_plugin::panels::{Cell, GridNode, Tone};
use fe_ui::{ActiveTheme as _, Role, Theme, hsla, node as a11y};
use gpui::{
    AnyElement, App, Bounds, ContentMask, Font, Hsla, InteractiveElement as _, IntoElement,
    ParentElement as _, Pixels, ShapedLine, SharedString, Size, Styled as _, TextAlign, TextRun,
    Window, canvas, div, fill, point, size,
};
use kiln::core::theme::{
    Color, Contrast, HIGH_CONTRAST_TEXT, at_least, contrast_ratio, over, surface_grounds,
};

use crate::Workbench;

/// A grid tone's least contrast with the panel it's drawn on: WCAG's AA for
/// body text.
pub(crate) const GRID_TONE_CONTRAST: f32 = 4.5;

/// The least contrast a grid tone keeps with the panel at `contrast`:
/// [`GRID_TONE_CONTRAST`], or under Increase Contrast the high-contrast
/// pass's floor for text ([`HIGH_CONTRAST_TEXT`]).
pub(crate) fn tone_floor(contrast: Contrast) -> f32 {
    match contrast {
        Contrast::Normal => GRID_TONE_CONTRAST,
        Contrast::High => HIGH_CONTRAST_TEXT,
    }
}

/// The theme token `tone` names: the same as [`super::tone_color`]'s.
pub(crate) fn tone_key(tone: Tone) -> &'static str {
    match tone {
        Tone::Normal => "text.primary",
        Tone::Muted => "text.muted",
        Tone::Accent => "text.accent",
        Tone::Success => "status.success",
        Tone::Warning => "status.warning",
        Tone::Error => "status.error",
    }
}

/// Every tone, in the WIT's order.
pub(crate) const TONES: [Tone; 6] = [
    Tone::Normal,
    Tone::Muted,
    Tone::Accent,
    Tone::Success,
    Tone::Warning,
    Tone::Error,
];

/// The colours a grid is drawn in under one theme: the panel's ground, and
/// each tone on it, at least [`tone_floor`] from it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct GridTones {
    /// The panel, opaque over the window's background.
    pub ground: Hsla,
    /// Each of [`TONES`], in order.
    pub tones: [Hsla; 6],
}

impl GridTones {
    /// The grid's colours in `theme`.
    pub(crate) fn of(theme: &Theme) -> Self {
        let data = theme.data();
        let tokens = &data.tokens;
        // [background, panel, ..]: the panel over the window's background.
        let ground: Color = surface_grounds(tokens)[1];
        let floor = tone_floor(data.contrast);
        let tones = TONES.map(|tone| {
            let token = *tokens.get(tone_key(tone)).expect("tones are tokens");
            let token = over(token, ground);
            match contrast_ratio(token, ground) >= floor {
                true => hsla(token),
                false => hsla(at_least(token, &[ground], floor)),
            }
        });
        Self {
            ground: hsla(ground),
            tones,
        }
    }

    /// `tone`'s colour.
    pub(crate) fn tone(&self, tone: Tone) -> Hsla {
        let at = TONES.iter().position(|each| *each == tone).expect("a tone");
        self.tones[at]
    }

    /// A cell's foreground and background: the tone on the panel, or
    /// reversed, the panel's colour on the tone.
    pub(crate) fn cell(&self, cell: &Cell) -> (Hsla, Option<Hsla>) {
        let tone = self.tone(cell.tone);
        match cell.reverse {
            false => (tone, None),
            true => (self.ground, Some(tone)),
        }
    }
}

/// The editor's font and line: its theme's, from the workbench the panel is
/// in, else the default dark theme's (a panel drawn alone, in a test).
pub(crate) fn editor_font(window: &Window, cx: &App) -> (Font, Pixels, Pixels) {
    let root = window.root::<Workbench>().flatten();
    let theme = match root {
        Some(workbench) => workbench.read(cx).pane().read(cx).theme().clone(),
        None => EditorTheme::default_dark(cx),
    };
    let line = (theme.font_size * theme.line_height).round();
    (theme.font(), theme.font_size, line)
}

/// One cell as last drawn: its text as shaped, where its glyphs start from
/// the row's left edge, how wide they are and at what size, and its colours.
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DrawnCell {
    pub text: SharedString,
    pub x: Pixels,
    pub width: Pixels,
    pub font_size: Pixels,
    pub fore: Hsla,
    pub back: Option<Hsla>,
}

/// A grid as last drawn, for the tests: its font, its cell, each row's text
/// and each row's cells.
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DrawnGrid {
    pub family: SharedString,
    pub font_size: Pixels,
    pub cell: Size<Pixels>,
    pub rows: Vec<String>,
    pub cells: Vec<Vec<DrawnCell>>,
}

/// What a cell's grapheme is drawn as: itself, or a space for one holding a
/// control character (a newline would break the row's line).
fn shown(ch: &str) -> SharedString {
    match ch.chars().any(char::is_control) {
        true => " ".into(),
        false => SharedString::from(ch.to_owned()),
    }
}

/// One cell shaped to fit its place: at the font's size if it fits the cell's
/// advance, else smaller, to the advance (a wide grapheme: CJK, an emoji);
/// placed at its column's left edge plus half what it leaves of the
/// advance, so a narrow one (a lone combining mark) is centred.
fn shape_cell(
    text: SharedString,
    col: usize,
    font: &Font,
    font_size: Pixels,
    advance: Pixels,
    fore: Hsla,
    window: &Window,
) -> (ShapedLine, Pixels, Pixels) {
    let text_system = window.text_system();
    let run = |size| {
        let runs = [TextRun {
            len: text.len(),
            font: font.clone(),
            color: fore,
            background_color: None,
            underline: None,
            strikethrough: None,
        }];
        text_system.shape_line(text.clone(), size, &runs, None)
    };
    let mut size = font_size;
    let mut line = run(size);
    if line.width() > advance {
        size = font_size * (advance / line.width());
        line = run(size);
    }
    let left = advance * col as f32 + (advance - line.width()).max(Pixels::ZERO) / 2.;
    (line, left, size)
}

/// Grid `grid`, node `node_id`'s, drawn: one image region named by its
/// label, holding one element a row that paints each cell's grapheme in its
/// own place, `advance` from the last, clipped to it. And what was drawn.
pub(crate) fn render_grid(
    node_id: &str,
    grid: &GridNode,
    window: &mut Window,
    cx: &mut App,
) -> (AnyElement, DrawnGrid) {
    let tones = GridTones::of(cx.theme());
    let (font, font_size, line) = editor_font(window, cx);
    let text_system = window.text_system();
    let font_id = text_system.resolve_font(&font);
    let advance = (text_system.advance(font_id, font_size, 'm'))
        .map(|advance| advance.width)
        .or_else(|_| text_system.em_advance(font_id, font_size))
        .unwrap_or(font_size * 0.6);
    let cell = size(advance, line);
    let cols = usize::from(grid.cols);
    // Whole pixels, rounded up, so layout's rounding never clips the last
    // column.
    let width = (advance * cols as f32).ceil();
    let mut drawn = DrawnGrid {
        family: font.family.clone(),
        font_size,
        cell,
        rows: Vec::new(),
        cells: Vec::new(),
    };
    let rows: Vec<AnyElement> = (grid.cells.chunks(cols.max(1)))
        .take(usize::from(grid.rows))
        .map(|cells| {
            let mut text = String::new();
            let mut row = Vec::with_capacity(cells.len());
            let mut painted = Vec::with_capacity(cells.len());
            for (col, each) in cells.iter().enumerate() {
                let (fore, back) = tones.cell(each);
                let shown = shown(&each.ch);
                text.push_str(&shown);
                let (shaped, x, size) =
                    shape_cell(shown.clone(), col, &font, font_size, advance, fore, window);
                row.push(DrawnCell {
                    text: shown,
                    x,
                    width: shaped.width(),
                    font_size: size,
                    fore,
                    back,
                });
                painted.push((col, shaped, x, back));
            }
            drawn.rows.push(text);
            drawn.cells.push(row);
            canvas(
                |_, _, _| {},
                move |bounds, (), window, cx| {
                    for (col, shaped, x, back) in painted {
                        let place = Bounds::new(
                            point(bounds.origin.x + advance * col as f32, bounds.origin.y),
                            size(advance, line),
                        );
                        window.with_content_mask(Some(ContentMask { bounds: place }), |window| {
                            if let Some(back) = back {
                                window.paint_quad(fill(place, back));
                            }
                            let origin = point(bounds.origin.x + x, bounds.origin.y);
                            if let Err(error) =
                                shaped.paint(origin, line, TextAlign::Left, None, window, cx)
                            {
                                log::debug!("a grid cell didn't paint: {error:#}");
                            }
                        });
                    }
                },
            )
            .flex_none()
            .h(line)
            .w(width)
            .into_any_element()
        })
        .collect();
    let at = node_id.to_owned();
    let element = a11y(
        div().id(super::element_id(node_id)),
        window,
        cx,
        Role::Image,
        grid.label.clone(),
    )
    .debug_selector(move || super::selector(&at))
    .flex_none()
    .flex()
    .flex_col()
    .w(width)
    .h(line * f32::from(grid.rows))
    .bg(tones.ground)
    .children(rows)
    .into_any_element();
    (element, drawn)
}
