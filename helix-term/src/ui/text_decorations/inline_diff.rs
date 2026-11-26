//! Draws the inline diff: the removed lines (see `helix_view::annotations::inline_diff`) into the
//! virtual rows the annotation reserved, in the removed color on a faded tint.

use helix_core::doc_formatter::FormattedGrapheme;
use helix_core::Position;
use helix_view::annotations::inline_diff::Removed;
use helix_view::graphics::{Rect, Style};

use crate::ui::document::{LinePos, TextRenderer};
use crate::ui::text_decorations::Decoration;

/// The removed lines of the view and the style they are drawn in.
pub struct InlineDiff {
    removed: Vec<Removed>,
    style: Style,
    /// The removed lines whose newline was just drawn: they go below this visual line.
    pending: Option<usize>,
    /// Which entry of `removed` the next newline anchor is.
    next: usize,
}

impl InlineDiff {
    pub fn new(removed: Vec<Removed>, style: Style) -> InlineDiff {
        InlineDiff { removed, style, pending: None, next: 0 }
    }
}

impl Decoration for InlineDiff {
    fn reset_pos(&mut self, pos: usize) -> usize {
        self.next = self.removed.partition_point(|r| r.newline < pos);
        self.removed.get(self.next).map_or(usize::MAX, |r| r.newline)
    }

    fn decorate_grapheme(&mut self, _renderer: &mut TextRenderer, _grapheme: &FormattedGrapheme) -> usize {
        // The anchor is the newline that ends a line with removed lines after it; every hunk that
        // follows that line is drawn below it, so the next anchor is past them all.
        let Some(here) = self.removed.get(self.next).map(|r| r.newline) else { return usize::MAX };
        self.pending = Some(self.next);
        while self.removed.get(self.next).is_some_and(|r| r.newline == here) {
            self.next += 1;
        }
        self.removed.get(self.next).map_or(usize::MAX, |r| r.newline)
    }

    fn render_virt_lines(&mut self, renderer: &mut TextRenderer, pos: LinePos, virt_off: Position) -> Position {
        let Some(i) = self.pending.take() else { return Position::new(0, 0) };
        let Some(removed) = self.removed.get(i) else { return Position::new(0, 0) };
        // Several hunks can follow one line: draw each that anchors here.
        let lines: Vec<&String> = self.removed[i..]
            .iter()
            .take_while(|r| r.anchor == removed.anchor)
            .flat_map(|r| r.lines.iter())
            .collect();
        let viewport = renderer.viewport;
        let skip = renderer.offset.col;
        for (k, line) in lines.iter().enumerate() {
            let y = pos.visual_line + virt_off.row as u16 + k as u16;
            renderer.set_style(Rect::new(viewport.x, y, viewport.width, 1), self.style);
            let text: String = line.replace('\t', "    ").chars().skip(skip).collect();
            renderer.set_stringn(viewport.x, y, &text, viewport.width as usize, self.style);
        }
        Position::new(lines.len(), 0)
    }
}
