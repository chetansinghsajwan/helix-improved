//! The inline diff: the lines git says were removed, shown in place as virtual lines (off by default,
//! `inline-diff` in the config). The annotation here reserves the rows; the editor's decoration draws
//! the old text into them.

use helix_core::text_annotations::LineAnnotation;
use helix_core::Position;
use std::collections::HashMap;

use crate::Document;

/// Lines removed at one place of the document.
#[derive(Debug, Clone)]
pub struct Removed {
    /// The document line the removed lines show after (the one before the change).
    pub anchor: usize,
    /// The char index of the newline that ends `anchor`, where the rows are reserved.
    pub newline: usize,
    /// The removed text, one entry per line, without line endings.
    pub lines: Vec<String>,
}

/// The removed lines of the hunks that touch the document lines `from..to`, in document order.
pub fn removed_lines(doc: &Document, from: usize, to: usize) -> Vec<Removed> {
    let Some(handle) = doc.diff_handle() else { return Vec::new() };
    let diff = handle.load();
    let base = diff.diff_base();
    let text = doc.text();
    let mut out = Vec::new();
    for hunk in diff.hunks_intersecting_line_ranges(std::iter::once((from, to))) {
        if hunk.before.is_empty() {
            continue;
        }
        // A change at the very top shows its old lines after the first line.
        let anchor = (hunk.after.start as usize).saturating_sub(1).min(text.len_lines().saturating_sub(1));
        let next_line = (anchor + 1).min(text.len_lines());
        let newline = text.line_to_char(next_line).saturating_sub(1);
        let lines = (hunk.before.start..hunk.before.end)
            .filter(|&l| (l as usize) < base.len_lines())
            .map(|l| {
                let line = base.line(l as usize).to_string();
                line.trim_end_matches(['\n', '\r']).to_string()
            })
            .collect();
        out.push(Removed { anchor, newline, lines });
    }
    out
}

/// Reserves a virtual row for each removed line, after the line it follows.
pub struct InlineDiff {
    /// Anchor line -> (the char index right after its newline, rows to reserve).
    rows: HashMap<usize, (usize, usize)>,
}

impl InlineDiff {
    pub fn new(removed: &[Removed]) -> InlineDiff {
        let mut rows: HashMap<usize, (usize, usize)> = HashMap::new();
        for r in removed {
            rows.entry(r.anchor).or_insert((r.newline + 1, 0)).1 += r.lines.len();
        }
        InlineDiff { rows }
    }
}

impl LineAnnotation for InlineDiff {
    fn insert_virtual_lines(
        &mut self,
        line_end_char_idx: usize,
        _line_end_visual_pos: Position,
        doc_line: usize,
    ) -> Position {
        // Only after the newline that ends the line (not after a soft-wrapped part of it).
        match self.rows.get(&doc_line) {
            Some(&(after_newline, rows)) if line_end_char_idx == after_newline => Position::new(rows, 0),
            _ => Position::new(0, 0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_are_reserved_after_the_newline_of_the_line_before_the_change() {
        let removed = vec![
            Removed { anchor: 2, newline: 19, lines: vec!["a".into(), "b".into()] },
            Removed { anchor: 2, newline: 19, lines: vec!["c".into()] },
            Removed { anchor: 5, newline: 40, lines: vec!["d".into()] },
        ];
        let mut diff = InlineDiff::new(&removed);
        let at = |d: &mut InlineDiff, idx, line| d.insert_virtual_lines(idx, Position::new(0, 0), line).row;
        assert_eq!(at(&mut diff, 20, 2), 3, "both hunks after line 2");
        assert_eq!(at(&mut diff, 12, 2), 0, "not after a soft-wrapped part of the line");
        assert_eq!(at(&mut diff, 41, 5), 1);
        assert_eq!(at(&mut diff, 30, 4), 0);
    }
}
