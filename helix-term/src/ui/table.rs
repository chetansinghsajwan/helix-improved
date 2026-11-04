//! Plain-text tables (csv, tsv, psv) in live mode: every column is padded to the width of its widest
//! cell. Nothing is written to the file; the padding is virtual text drawn before each delimiter.

use helix_core::text_annotations::{InlineAnnotation, Overlay};
use helix_core::RopeSlice;

/// The widest a column is made; a longer cell only pushes its own row.
const MAX_COL: usize = 48;
/// How many lines are aligned (bigger files get the top).
const MAX_LINES: usize = 20_000;

/// The delimiter of a file by its extension.
pub fn delimiter(path: Option<&std::path::Path>) -> Option<char> {
    match path?.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "csv" => Some(','),
        "tsv" => Some('\t'),
        "psv" => Some('|'),
        _ => None,
    }
}

/// Where the delimiters of a line are (char offsets in the line) and the width of every cell. Quotes
/// protect a delimiter; a quote does not carry over to the next line.
fn cells(line: RopeSlice, delim: char) -> (Vec<usize>, Vec<usize>) {
    use helix_core::unicode::width::UnicodeWidthChar;
    let (mut at, mut widths, mut w, mut quoted) = (Vec::new(), Vec::new(), 0usize, false);
    for (i, ch) in line.chars().enumerate() {
        match ch {
            '\n' | '\r' => break,
            '"' => {
                quoted = !quoted;
                w += 1;
            }
            c if c == delim && !quoted => {
                at.push(i);
                widths.push(w);
                w = 0;
            }
            c => w += c.width().unwrap_or(0),
        }
    }
    widths.push(w);
    (at, widths)
}

/// The padding before every delimiter, and (for tabs) the tab drawn as a space so the padding is exact.
pub fn align(text: RopeSlice, delim: char) -> (Vec<InlineAnnotation>, Vec<Overlay>) {
    let lines = text.len_lines().min(MAX_LINES);
    let rows: Vec<(usize, Vec<usize>, Vec<usize>)> = (0..lines)
        .map(|l| {
            let (at, w) = cells(text.line(l), delim);
            (text.line_to_char(l), at, w)
        })
        .collect();
    let mut col = Vec::<usize>::new();
    for (_, _, w) in &rows {
        for (i, &n) in w.iter().enumerate() {
            if col.len() <= i {
                col.push(0);
            }
            col[i] = col[i].max(n.min(MAX_COL));
        }
    }
    let (mut pads, mut overlays) = (Vec::new(), Vec::new());
    for (start, at, w) in &rows {
        for (i, &d) in at.iter().enumerate() {
            // A tab is shown as one space, so the padding is exact.
            let pad = col[i].saturating_sub(w[i]);
            if delim == '\t' {
                overlays.push(Overlay::new(start + d, " "));
            }
            if pad > 0 {
                pads.push(InlineAnnotation::new(start + d, " ".repeat(pad)));
            }
        }
    }
    (pads, overlays)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_are_padded_to_the_widest_cell_and_quotes_protect_the_delimiter() {
        let rope = helix_core::Rope::from("id,name,note\n1,\"a,b\",x\n22,c,yy\n");
        let (pads, overlays) = align(rope.slice(..), ',');
        assert!(overlays.is_empty());
        // Row 2 ("1,"): the first column is 2 wide, so one space before its comma.
        let after_first_comma_row2 = "id,name,note\n".len() + 1;
        assert_eq!(pads.iter().find(|p| p.char_idx == after_first_comma_row2).map(|p| p.text.len()), Some(1));
        // The quoted cell is one cell: "\"a,b\"" is 5 wide, "name" 4 and "c" 1, so row 3 pads 4.
        let row3_second = "id,name,note\n1,\"a,b\",x\n".len() + "22,c".len();
        assert_eq!(pads.iter().find(|p| p.char_idx == row3_second).map(|p| p.text.len()), Some(4));
    }

    #[test]
    fn tabs_are_drawn_as_spaces_and_the_delimiter_comes_from_the_extension() {
        let rope = helix_core::Rope::from("a\tb\nlonger\tc\n");
        let (pads, overlays) = align(rope.slice(..), '\t');
        assert_eq!(overlays.len(), 2);
        assert_eq!(pads.len(), 1, "only the short first cell needs padding");
        assert_eq!(delimiter(Some(std::path::Path::new("x.TSV"))), Some('\t'));
        assert_eq!(delimiter(Some(std::path::Path::new("x.txt"))), None);
    }
}
