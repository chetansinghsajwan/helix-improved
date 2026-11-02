//! Dim what is behind a popup (see `tui_design::dim`): the same idea on helix's own surface.

use helix_view::graphics::{Color, Modifier, Rect};
use tui::buffer::Buffer as Surface;

/// Dim every cell of `area`: colours move toward the design background, default colours get the
/// terminal's dim attribute.
pub fn dim(surface: &mut Surface, area: Rect) {
    let keep = u32::from(tui_design::dim::KEEP);
    let Color::Rgb(br, bg, bb) = super::pick::color(tui_design::Theme::load().colors.bg.into()) else { return };
    let mix = |c: Color| match c {
        Color::Rgb(r, g, b) => {
            let m = |a: u8, base: u8| ((u32::from(a) * keep + u32::from(base) * (100 - keep)) / 100) as u8;
            Color::Rgb(m(r, br), m(g, bg), m(b, bb))
        }
        other => other,
    };
    let area = area.intersection(surface.area);
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            let Some(cell) = surface.get_mut(x, y) else { continue };
            if matches!(cell.fg, Color::Rgb(..)) {
                cell.fg = mix(cell.fg);
            } else {
                cell.modifier.insert(Modifier::DIM);
            }
            cell.bg = mix(cell.bg);
        }
    }
}
