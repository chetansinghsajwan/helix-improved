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
    /// The folder relative image paths start from, and where the headings are (`]` and `[` jump).
    base: Option<std::path::PathBuf>,
    headings: Vec<usize>,
}

impl MarkdownPreview {
    pub fn new(title: &str, source: String, base: Option<std::path::PathBuf>) -> Self {
        MarkdownPreview { source, title: title.to_string(), theme: tui_design::Theme::load(), lines: Vec::new(), width: 0, scroll: 0, height: 10, base, headings: Vec::new() }
    }
}

impl Component for MarkdownPreview {
    fn dims_background(&self) -> bool {
        true
    }

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
            KeyCode::Char('j') if ev.modifiers.contains(ratatui::crossterm::event::KeyModifiers::CONTROL) => self.scroll = (self.scroll + 5).min(last),
            KeyCode::Char('k') if ev.modifiers.contains(ratatui::crossterm::event::KeyModifiers::CONTROL) => self.scroll = self.scroll.saturating_sub(5),
            KeyCode::Down | KeyCode::Char('j') => self.scroll = (self.scroll + 1).min(last),
            KeyCode::Up | KeyCode::Char('k') => self.scroll = self.scroll.saturating_sub(1),
            KeyCode::PageDown | KeyCode::Char(' ') => self.scroll = (self.scroll + page).min(last),
            KeyCode::PageUp => self.scroll = self.scroll.saturating_sub(page),
            KeyCode::Char(']') => {
                if let Some(h) = self.headings.iter().find(|h| **h > self.scroll) {
                    self.scroll = *h;
                }
            }
            KeyCode::Char('[') => {
                if let Some(h) = self.headings.iter().rev().find(|h| **h < self.scroll) {
                    self.scroll = *h;
                }
            }
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
            let r = pv::markdown::render_in(&self.source, &self.theme, inner.width, self.base.as_deref());
            self.lines = r.lines;
            self.headings = r.headings;
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
    fn dims_background(&self) -> bool {
        true
    }

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

/// The settings of a PDF export, chosen in a popup: paper, fonts, size, margin and layout. What is
/// preselected is what `mdexport` would use (its config file, which Nix fills with the stylix fonts).
pub struct ExportOptions {
    multi: tui_design::multi::Multi,
    theme: tui_design::Theme,
}

impl ExportOptions {
    /// The popup, or why it cannot be made (the `mdexport` tool is missing).
    pub fn new() -> Result<Self, String> {
        use tui_design::multi::{Kind, Multi, Section};
        let run = |arg: &str| -> Result<String, String> {
            let out = std::process::Command::new("mdexport").arg(arg).output().map_err(|e| format!("mdexport: {e}"))?;
            Ok(String::from_utf8_lossy(&out.stdout).into_owned())
        };
        let defaults = run("--defaults")?;
        let get = |key: &str| defaults.lines().find_map(|l| l.strip_prefix(&format!("{key}="))).unwrap_or("").to_string();
        let mut fonts: Vec<String> = run("--fonts")?.lines().map(str::to_string).collect();
        for want in [get("font"), get("mono")] {
            if !want.is_empty() && !fonts.contains(&want) {
                fonts.insert(0, want);
            }
        }
        let one = |title: &str, items: Vec<String>, chosen: String| {
            let mut items = items;
            if !items.contains(&chosen) {
                items.insert(0, chosen.clone());
            }
            Section::new(title, Kind::One, items, vec![chosen])
        };
        let list = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let mut layout = Vec::new();
        if get("justify") == "true" {
            layout.push("justify text".to_string());
        }
        if get("numbers") == "true" {
            layout.push("page numbers".to_string());
        }
        let sections = vec![
            one("Paper", list(&["a4", "letter"]), get("paper")),
            one("Font", fonts.clone(), get("font")),
            one("Code font", fonts, get("mono")),
            one("Size", list(&["9", "10", "11", "12", "14"]), get("size")),
            one("Margin", list(&["1.5", "2", "2.2", "2.5", "3"]), get("margin")),
            Section::new("Layout", Kind::Many, list(&["justify text", "page numbers"]), layout),
        ];
        Ok(ExportOptions { multi: Multi::new("Export PDF", sections), theme: tui_design::Theme::load() })
    }

    /// The `mdexport` arguments for what is chosen.
    fn args(&self) -> Vec<String> {
        let one = |title: &str| self.multi.chosen(title).first().cloned();
        let layout = self.multi.chosen("Layout");
        let mut args = Vec::new();
        for (flag, title) in [("--paper", "Paper"), ("--font", "Font"), ("--mono", "Code font"), ("--size", "Size"), ("--margin", "Margin")] {
            if let Some(v) = one(title) {
                args.push(flag.to_string());
                args.push(v);
            }
        }
        if !layout.iter().any(|l| l == "justify text") {
            args.push("--no-justify".into());
        }
        if !layout.iter().any(|l| l == "page numbers") {
            args.push("--no-numbers".into());
        }
        args
    }
}

impl Component for ExportOptions {
    fn dims_background(&self) -> bool {
        true
    }

    fn handle_event(&mut self, event: &Event, _cx: &mut Context) -> EventResult {
        let Event::Key(key) = event else {
            return EventResult::Ignored(None);
        };
        let Some(ev) = to_crossterm(key) else {
            return EventResult::Consumed(None);
        };
        match self.multi.key(&ev) {
            tui_design::multi::Outcome::Open => EventResult::Consumed(None),
            tui_design::multi::Outcome::Cancel => EventResult::Consumed(Some(Box::new(|compositor, _| {
                compositor.pop();
            }))),
            tui_design::multi::Outcome::Apply => {
                let args = self.args();
                EventResult::Consumed(Some(Box::new(move |compositor, cx| {
                    compositor.pop();
                    if let Err(e) = crate::commands::typed::start_markdown_export(cx.editor, cx.jobs, None, args) {
                        cx.editor.set_error(e.to_string());
                    }
                })))
            }
        }
    }

    fn render(&mut self, area: Rect, surface: &mut Surface, _cx: &mut Context) {
        let full = RRect::new(area.x, area.y, area.width, area.height);
        let mut buf = RBuffer::empty(full);
        self.multi.render(full, &mut buf, &self.theme);
        copy(&buf, area, surface);
    }

    fn cursor(&self, _area: Rect, _editor: &helix_view::Editor) -> (Option<helix_core::Position>, CursorKind) {
        (None, CursorKind::Hidden)
    }
}
