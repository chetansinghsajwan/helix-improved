//! A read-only rendered view of the current file (Markdown, with its markup drawn as styles)
//! as a popup, using the shared `pv` renderer. `j`/`k`, the arrows and the page keys scroll;
//! `q` or Esc closes.

use super::pick::{copy, to_crossterm};
use crate::compositor::{Component, Context, Event, EventResult};
use helix_view::graphics::{CursorKind, Rect};
use ratatui::buffer::Buffer as RBuffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect as RRect;
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, Widget};
use tui::buffer::Buffer as Surface;

pub struct MarkdownPreview {
    source: String,
    title: String,
    theme: tui_design::Theme,
    /// The rendered lines for the width they were made for.
    lines: Vec<Line<'static>>,
    width: u16,
    scroll: usize,
    height: usize,
}

impl MarkdownPreview {
    pub fn new(title: &str, source: String) -> Self {
        MarkdownPreview { source, title: title.to_string(), theme: tui_design::Theme::load(), lines: Vec::new(), width: 0, scroll: 0, height: 10 }
    }
}

impl Component for MarkdownPreview {
    fn handle_event(&mut self, event: &Event, _cx: &mut Context) -> EventResult {
        let Event::Key(key) = event else {
            return EventResult::Ignored(None);
        };
        let Some(ev) = to_crossterm(key) else {
            return EventResult::Consumed(None);
        };
        let last = self.lines.len().saturating_sub(1);
        let page = self.height.max(2) - 1;
        match ev.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                return EventResult::Consumed(Some(Box::new(|compositor, _| {
                    compositor.pop();
                })));
            }
            KeyCode::Down | KeyCode::Char('j') => self.scroll = (self.scroll + 1).min(last),
            KeyCode::Up | KeyCode::Char('k') => self.scroll = self.scroll.saturating_sub(1),
            KeyCode::PageDown | KeyCode::Char(' ') => self.scroll = (self.scroll + page).min(last),
            KeyCode::PageUp => self.scroll = self.scroll.saturating_sub(page),
            KeyCode::Home | KeyCode::Char('g') => self.scroll = 0,
            KeyCode::End | KeyCode::Char('G') => self.scroll = last,
            _ => {}
        }
        EventResult::Consumed(None)
    }

    fn render(&mut self, area: Rect, surface: &mut Surface, _cx: &mut Context) {
        let full = RRect::new(area.x, area.y, area.width, area.height);
        let p = tui_design::layout::popup(&self.theme, full);
        let popup = Rect::new(p.x, p.y, p.width, p.height);
        let r = RRect::new(0, 0, popup.width, popup.height);
        let mut buf = RBuffer::empty(r);
        let pane = tui_design::Pane::new(&self.theme).title(&self.title).focused(true);
        let inner = pane.inner(r);
        pane.render(r, &mut buf);
        if self.width != inner.width {
            self.lines = pv::markdown::render(&self.source, &self.theme, inner.width);
            self.width = inner.width;
        }
        self.height = usize::from(inner.height);
        self.scroll = self.scroll.min(self.lines.len().saturating_sub(1));
        Paragraph::new(self.lines.clone()).scroll((self.scroll as u16, 0)).render(inner, &mut buf);
        copy(&buf, popup, surface);
    }

    fn cursor(&self, _area: Rect, _editor: &helix_view::Editor) -> (Option<helix_core::Position>, CursorKind) {
        (None, CursorKind::Hidden)
    }
}

/// The keys of the current mode, searchable (`Space h`): the shared help overlay over the
/// editor's keymap.
pub struct KeysHelp {
    help: tui_design::keys::Help,
    rows: Vec<tui_design::keys::Row>,
    title: String,
    theme: tui_design::Theme,
}

impl KeysHelp {
    pub fn new(title: &str, rows: Vec<tui_design::keys::Row>) -> Self {
        KeysHelp { help: Default::default(), rows, title: title.to_string(), theme: tui_design::Theme::load() }
    }
}

impl Component for KeysHelp {
    fn handle_event(&mut self, event: &Event, _cx: &mut Context) -> EventResult {
        let Event::Key(key) = event else {
            return EventResult::Ignored(None);
        };
        let Some(ev) = to_crossterm(key) else {
            return EventResult::Consumed(None);
        };
        if self.help.key(&ev) {
            return EventResult::Consumed(None);
        }
        EventResult::Consumed(Some(Box::new(|compositor, _| {
            compositor.pop();
        })))
    }

    fn render(&mut self, area: Rect, surface: &mut Surface, _cx: &mut Context) {
        let full = RRect::new(area.x, area.y, area.width, area.height);
        let p = tui_design::layout::popup(&self.theme, full);
        let popup = Rect::new(p.x, p.y, p.width, p.height);
        let r = RRect::new(0, 0, popup.width, popup.height);
        let mut buf = RBuffer::empty(r);
        self.help.render(r, &mut buf, &self.theme, &self.title, &self.rows);
        copy(&buf, popup, surface);
    }

    fn cursor(&self, _area: Rect, _editor: &helix_view::Editor) -> (Option<helix_core::Position>, CursorKind) {
        (None, CursorKind::Hidden)
    }
}
