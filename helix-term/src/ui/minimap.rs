//! The minimap: a zoomed-out overview of the file along the right edge of a view. Every cell is a
//! braille character, two dots wide and four tall; a dot is lit when the text it stands for has
//! something other than blanks in it. The lines on screen are tinted, the cursor line is marked, and
//! errors, warnings and changed lines show in their colors.

use helix_core::diagnostic::Severity;
use helix_view::graphics::{Color, Rect, Style};
use helix_view::{Document, Editor, View};
use tui::buffer::Buffer as Surface;

/// How many source columns one dot stands for.
const COLS_PER_DOT: usize = 4;
/// How many source lines one dot row stands for at most (taller files show a window).
const MAX_LINES_PER_ROW: usize = 4;

/// Which lines the minimap shows and how they are squeezed: `lines` source lines per dot row, from `start`.
#[derive(Debug, PartialEq, Eq)]
pub struct Layout {
    pub start: usize,
    pub lines: usize,
}

impl Layout {
    /// For a file of `total` lines, with `first..first + visible` on screen, in `rows` cells of height.
    pub fn new(total: usize, first: usize, visible: usize, rows: usize) -> Layout {
        let dot_rows = (rows * 4).max(1);
        let lines = total.div_ceil(dot_rows).clamp(1, MAX_LINES_PER_ROW);
        let cover = dot_rows * lines;
        if total <= cover {
            return Layout { start: 0, lines };
        }
        // A window that moves with the scroll position, like the scroll bar of the file.
        let travel = total.saturating_sub(visible).max(1);
        let start = first.min(travel) * (total - cover) / travel;
        Layout { start, lines }
    }

    /// The source line a click `dot_row` dot rows from the top lands on.
    pub fn line_at(&self, dot_row: usize) -> usize {
        self.start + dot_row * self.lines
    }
}

/// The braille bit of the dot at column `c` (0..2) and row `r` (0..4) of a cell.
fn bit(c: usize, r: usize) -> u32 {
    const BITS: [[u32; 4]; 2] = [[0x01, 0x02, 0x04, 0x40], [0x08, 0x10, 0x20, 0x80]];
    BITS[c][r]
}

/// How many characters with text (not blanks) each dot column of the line has, at most `COLS_PER_DOT`.
fn line_dots(text: helix_core::RopeSlice, dots: usize) -> Vec<u8> {
    let mut counts = vec![0u8; dots];
    let mut col = 0usize;
    for ch in text.chars() {
        match ch {
            '\n' | '\r' => break,
            '\t' => col += 4,
            c if c.is_whitespace() => col += 1,
            _ => {
                let dot = col / COLS_PER_DOT;
                if dot >= dots {
                    break;
                }
                counts[dot] += 1;
                col += 1;
            }
        }
    }
    counts
}

/// The area of `view` the minimap takes (nothing when it is off).
pub fn rect(view: &View) -> Option<Rect> {
    if view.minimap == 0 {
        return None;
    }
    let inner = view.area.clip_bottom(1 + view.bottom_margin);
    (inner.height > 0 && view.minimap > 1).then(|| Rect::new(inner.right().saturating_sub(view.minimap - 1), inner.y, view.minimap - 1, inner.height))
}

pub fn render(editor: &Editor, doc: &Document, view: &View, surface: &mut Surface) {
    let Some(r) = rect(view) else { return };
    let theme = &editor.theme;
    let text = doc.text();
    let total = text.len_lines().max(1);
    let first = text.char_to_line(doc.view_offset(view.id).anchor.min(text.len_chars()));
    let visible = view.inner_height();
    let layout = Layout::new(total, first, visible, usize::from(r.height));
    let dots = usize::from(r.width) * 2;
    let rows = usize::from(r.height) * 4;

    // The dots of every line in the window.
    let end = (layout.start + rows * layout.lines).min(total);
    let counts: Vec<Vec<u8>> = (layout.start..end).map(|l| line_dots(text.line(l), dots)).collect();
    let group = |dr: usize| layout.start + dr * layout.lines..(layout.start + (dr + 1) * layout.lines).min(end);

    // Lines with something to show.
    let mut marks: Vec<(usize, Color)> = Vec::new();
    let style_fg = |name: &str| theme.get(name).fg;
    if let Some(diff) = doc.diff_handle() {
        let diff = diff.load();
        let (plus, delta, minus) = (style_fg("diff.plus"), style_fg("diff.delta"), style_fg("diff.minus"));
        for h in diff.hunks_intersecting_line_ranges(std::iter::once((layout.start, end))) {
            let (range, color) = if h.is_pure_removal() { (h.after.start..h.after.start + 1, minus) } else if h.is_pure_insertion() { (h.after.clone(), plus) } else { (h.after.clone(), delta) };
            if let Some(c) = color {
                marks.extend((range.start as usize..range.end as usize).map(|l| (l, c)));
            }
        }
    }
    let (error, warning, other) = (style_fg("error"), style_fg("warning"), style_fg("info"));
    let mut worst: std::collections::HashMap<usize, u8> = Default::default();
    for d in doc.diagnostics() {
        let rank = match d.severity {
            Some(Severity::Error) | None => 3,
            Some(Severity::Warning) => 2,
            _ => 1,
        };
        let e = worst.entry(d.line).or_insert(0);
        *e = (*e).max(rank);
    }
    for (line, rank) in worst {
        let color = match rank {
            3 => error,
            2 => warning,
            _ => other,
        };
        if let Some(c) = color {
            marks.push((line, c));
        }
    }
    let cursor_lines: Vec<usize> = doc.selection(view.id).ranges().iter().map(|r| text.char_to_line(r.cursor(text.slice(..)))).collect();

    let dim = theme.get("ui.linenr").fg.unwrap_or(Color::Gray);
    let tint = theme.get("ui.selection").bg.or_else(|| theme.get("ui.cursorline.primary").bg);
    let accent = theme.get("ui.cursor.primary").bg.or_else(|| theme.get("ui.text").fg).unwrap_or(Color::White);
    let on_screen = first..first + visible;
    for cy in 0..usize::from(r.height) {
        // The cell stands for these lines.
        let cell_lines = layout.start + cy * 4 * layout.lines..(layout.start + (cy + 1) * 4 * layout.lines).min(total);
        for cx in 0..usize::from(r.width) {
            let mut mask = 0u32;
            for c in 0..2 {
                for rr in 0..4 {
                    // A dot is lit when a quarter of the characters it stands for have text.
                    let lines = group(cy * 4 + rr);
                    let have: usize = lines.clone().filter_map(|l| counts.get(l - layout.start)).map(|m| usize::from(m[cx * 2 + c])).sum();
                    if have > 0 && have * 4 >= lines.count().max(1) * COLS_PER_DOT {
                        mask |= bit(c, rr);
                    }
                }
            }
            let mut style = Style::default().fg(dim);
            if on_screen.start < cell_lines.end && cell_lines.start < on_screen.end {
                if let Some(t) = tint {
                    style = style.bg(t);
                }
            }
            // The edge column carries the marks, so they do not recolor the text dots.
            if cx + 1 == usize::from(r.width) {
                if let Some((_, c)) = marks.iter().rev().find(|(l, _)| cell_lines.contains(l)) {
                    style = style.fg(*c);
                    if mask == 0 {
                        mask = 0xC0;
                    }
                }
                if cursor_lines.iter().any(|l| cell_lines.contains(l)) {
                    style = style.fg(accent);
                    if mask == 0 {
                        mask = 0xC0;
                    }
                }
            }
            let ch = char::from_u32(0x2800 + mask).unwrap_or(' ');
            let cell = surface.get_mut(r.x + cx as u16, r.y + cy as u16);
            if let Some(cell) = cell {
                cell.set_symbol(&ch.to_string());
                cell.set_style(style);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_short_file_shows_four_lines_per_cell_and_a_long_one_squeezes_then_scrolls() {
        // 40 rows of cells = 160 dot rows: 100 lines fit one to one.
        assert_eq!(Layout::new(100, 0, 40, 40), Layout { start: 0, lines: 1 });
        // 500 lines: three to a dot row, all of it fits in the window.
        assert_eq!(Layout::new(500, 0, 40, 40), Layout { start: 0, lines: 4 });
        // A huge file shows a window that follows the scroll position.
        let top = Layout::new(10_000, 0, 40, 40);
        let end = Layout::new(10_000, 9_960, 40, 40);
        assert_eq!((top.start, top.lines), (0, 4));
        assert_eq!(end.start + 160 * 4, 10_000, "the window ends where the file ends");
        assert!(Layout::new(10_000, 5_000, 40, 40).start > 0);
    }

    #[test]
    fn dots_are_lit_where_there_is_text_and_braille_bits_are_the_standard_ones() {
        let rope = helix_core::Rope::from("ab    cd\n");
        let counts = line_dots(rope.slice(..), 8);
        assert_eq!(counts[0], 2, "ab in the first four columns");
        assert_eq!(counts[1], 2, "cd in columns four to seven");
        assert_eq!(counts[2], 0);
        assert_eq!(bit(0, 0) | bit(1, 3), 0x81);
        assert_eq!(char::from_u32(0x2800 + 0xFF), Some('⣿'));
    }
}
