//! Dim what is behind a popup (see `tui_design::dim`): the same idea on helix's own surface, with
//! the same colour mixing (`tui_design::mix`).

use helix_view::graphics::{Color, Modifier, Rect};
use ratatui::style::Color as RColor;
use std::sync::OnceLock;
use tui::buffer::Buffer as Surface;

/// The design background colour, read once (the theme file is not parsed again for every frame).
fn background() -> RColor {
    static BG: OnceLock<RColor> = OnceLock::new();
    *BG.get_or_init(|| tui_design::Theme::load().colors.bg.into())
}

/// Mix `c` toward the background like `tui_design::dim` does; colours that are not RGB stay as they are.
fn mixed(c: Color, base: RColor) -> Color {
    let Color::Rgb(r, g, b) = c else { return c };
    match tui_design::mix(RColor::Rgb(r, g, b), base, tui_design::dim::KEEP) {
        RColor::Rgb(r, g, b) => Color::Rgb(r, g, b),
        _ => c,
    }
}

/// Dim every cell of `area`: colours move toward the design background, default colours get the
/// terminal's dim attribute.
pub fn dim(surface: &mut Surface, area: Rect) {
    let base = background();
    let area = area.intersection(surface.area);
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            let Some(cell) = surface.get_mut(x, y) else { continue };
            if matches!(cell.fg, Color::Rgb(..)) {
                cell.fg = mixed(cell.fg, base);
            } else {
                cell.modifier.insert(Modifier::DIM);
            }
            cell.bg = mixed(cell.bg, base);
        }
    }
}
