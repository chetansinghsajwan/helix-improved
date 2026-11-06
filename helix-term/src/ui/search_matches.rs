//! Matches of the last search, shared by the view highlight and the statusline count.
//! Computed once per (document, version, query) and cached.

use std::cell::RefCell;
use std::ops::Range;
use std::rc::Rc;

use helix_stdx::rope::{self, RopeSliceExt};
use helix_view::{Document, DocumentId, Editor};

/// Past this many matches the list is cut (the count shows "N+").
const MAX_MATCHES: usize = 20_000;

pub struct Matches {
    /// Char ranges, sorted and non-overlapping.
    pub ranges: Vec<Range<usize>>,
    pub truncated: bool,
}

struct Cache {
    doc: DocumentId,
    version: i32,
    query: String,
    smart_case: bool,
    matches: Option<Rc<Matches>>,
}

thread_local! {
    static CACHE: RefCell<Option<Cache>> = const { RefCell::new(None) };
}

pub fn build_regex(query: &str, smart_case: bool) -> Option<rope::Regex> {
    let case_insensitive = smart_case && !query.chars().any(char::is_uppercase);
    rope::RegexBuilder::new()
        .syntax(
            rope::Config::new()
                .case_insensitive(case_insensitive)
                .multi_line(true),
        )
        .build(query)
        .ok()
}

/// The matches of the last search in `doc`, or `None` when highlighting is off,
/// nothing was searched, or the query is not a valid regex.
pub fn matches(editor: &Editor, doc: &Document) -> Option<Rc<Matches>> {
    if !editor.search_highlight || !editor.config().search.highlight {
        return None;
    }
    all(editor, doc)
}

/// Like `matches`, but whether or not the matches are highlighted.
pub fn all(editor: &Editor, doc: &Document) -> Option<Rc<Matches>> {
    let config = editor.config();
    let query = editor
        .registers
        .first(editor.registers.last_search_register, editor)?
        .to_string();
    if query.is_empty() {
        return None;
    }
    let smart_case = config.search.smart_case;

    CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(c) = cache.as_ref() {
            if c.doc == doc.id()
                && c.version == doc.version()
                && c.smart_case == smart_case
                && c.query == query
            {
                return c.matches.clone();
            }
        }
        let computed = build_regex(&query, smart_case).map(|regex| {
            let text = doc.text().slice(..);
            let mut ranges = Vec::new();
            let mut truncated = false;
            for m in regex.find_iter(text.regex_input()) {
                if m.is_empty() {
                    continue;
                }
                if ranges.len() == MAX_MATCHES {
                    truncated = true;
                    break;
                }
                ranges.push(text.byte_to_char(m.start())..text.byte_to_char(m.end()));
            }
            Rc::new(Matches { ranges, truncated })
        });
        *cache = Some(Cache {
            doc: doc.id(),
            version: doc.version(),
            query,
            smart_case,
            matches: computed.clone(),
        });
        computed
    })
}

impl Matches {
    /// Matches that overlap the char range `view`.
    pub fn in_range(&self, view: Range<usize>) -> &[Range<usize>] {
        let start = self.ranges.partition_point(|r| r.end <= view.start);
        let end = self.ranges.partition_point(|r| r.start < view.end);
        &self.ranges[start..end.max(start)]
    }

    /// 1-based index of the match that starts at `pos`, if there is one.
    pub fn index_at(&self, pos: usize) -> Option<usize> {
        let i = self.ranges.partition_point(|r| r.start < pos);
        (self.ranges.get(i)?.start == pos).then_some(i + 1)
    }
}
