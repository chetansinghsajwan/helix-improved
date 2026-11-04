//! Docks: a left sidebar, a right sidebar and a bottom panel around the editor,
//! like VS Code and Zed. Each dock holds one or more panels (`files`, `git`) that
//! the user assigns in `editor.docks`; one panel shows at a time and `[` / `]`
//! switch. The shared panels are drawn by ratatui and copied onto the helix
//! surface (see `pick.rs`); sizes and colors come from the shared `ui` tokens.
//! The layout itself is `tui_design::layout::workbench`.

use super::pick::{copy, to_crossterm};
use helix_view::editor::DocksConfig;
use helix_view::graphics::Rect;
use helix_view::input::KeyEvent;
use helix_view::Editor;
use ratatui::buffer::Buffer as RBuffer;
use ratatui::layout::Rect as RRect;
use ratatui::style::{Modifier, Style as RStyle};
use ratatui::crossterm::event as ct;
use ratatui::widgets::Widget;
use std::path::{Path, PathBuf};
use tui::buffer::Buffer as Surface;

pub fn to_ratatui(r: Rect) -> RRect {
    RRect::new(r.x, r.y, r.width, r.height)
}

pub fn from_ratatui(r: RRect) -> Rect {
    Rect::new(r.x, r.y, r.width, r.height)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
    Bottom,
}

/// What a panel asks the editor to do after a key.
#[derive(Debug, PartialEq, Eq)]
pub enum PanelEvent {
    None,
    /// Open this file.
    Open(PathBuf),
    /// Open this file at a 1-based line and column.
    OpenAt { path: PathBuf, line: u32, col: u32 },
    /// Give the keyboard back to the editor.
    Unfocus,
    /// A file or folder was created.
    Created(PathBuf, bool),
    /// Files were renamed or moved; open buffers should follow.
    Moved(Vec<(PathBuf, PathBuf)>),
    /// Files or folders were trashed or deleted; their buffers close.
    Deleted(Vec<PathBuf>),
}

/// A panel that can sit in a dock.
pub trait DockPanel {
    fn id(&self) -> &'static str;
    /// `editor` is there for panels that show editor state (diagnostics).
    fn render(&mut self, buf: &mut RBuffer, area: RRect, theme: &tui_design::Theme, focused: bool, editor: &Editor);
    fn handle_key(&mut self, key: ct::KeyEvent) -> PanelEvent;
    /// Whether typed text is expected (a prompt is open): then the leader key is a character, not the leader.
    fn is_typing(&self) -> bool {
        false
    }
    /// Show this file (the file explorer expands to it).
    fn reveal(&mut self, _path: &Path) {}
    /// Reread what the panel shows (something changed on disk).
    fn refresh(&mut self) {}
    /// The symbols of the file being edited (the symbol explorer).
    fn set_symbols(&mut self, _path: &Path, _symbols: Vec<symbols_panel::Symbol>) {}
    /// The line the editor's cursor is on (0-based), if there is a file.
    fn follow_line(&mut self, _line: Option<u32>) {}
    /// The file or folder under the cursor, for the preview beside the dock.
    fn preview_target(&self) -> Option<PathBuf> {
        None
    }
    /// The width the panel would like while it has the keyboard (a long name that does not fit).
    fn wanted_width(&self, _theme: &tui_design::Theme) -> Option<u16> {
        None
    }
}

struct FilesDock(files::Panel, Option<tui_design::keys::Help>);

impl DockPanel for FilesDock {
    fn id(&self) -> &'static str {
        "files"
    }

    fn render(&mut self, buf: &mut RBuffer, area: RRect, theme: &tui_design::Theme, focused: bool, _editor: &Editor) {
        self.0.render(area, buf, theme, focused);
        if let Some(help) = &mut self.1 {
            help.render(area, buf, theme, "Keys", &self.0.keys_rows());
        }
    }

    fn is_typing(&self) -> bool {
        self.0.is_typing() || self.1.is_some()
    }

    fn preview_target(&self) -> Option<PathBuf> {
        self.0.current().map(Path::to_path_buf)
    }

    fn wanted_width(&self, theme: &tui_design::Theme) -> Option<u16> {
        Some(self.0.wanted_width(theme))
    }

    fn handle_key(&mut self, key: ct::KeyEvent) -> PanelEvent {
        if let Some(help) = &mut self.1 {
            if !help.key(&key) {
                self.1 = None;
            }
            return PanelEvent::None;
        }
        match self.0.handle_event(&ct::Event::Key(key)) {
            files::Action::Open(p) => PanelEvent::Open(p),
            files::Action::Close => PanelEvent::Unfocus,
            files::Action::Created(p, d) => PanelEvent::Created(p, d),
            files::Action::Moved(m) => PanelEvent::Moved(m),
            files::Action::Deleted(d) => PanelEvent::Deleted(d),
            files::Action::Help => {
                self.1 = Some(tui_design::keys::Help::default());
                PanelEvent::None
            }
            _ => PanelEvent::None,
        }
    }

    fn reveal(&mut self, path: &Path) {
        self.0.reveal(path);
    }

    fn refresh(&mut self) {
        self.0.refresh();
    }
}

/// The changed files (no diff pane: use `space g` for the full panel).
struct GitDock(git_panel::Panel, Option<tui_design::keys::Help>);

impl DockPanel for GitDock {
    fn id(&self) -> &'static str {
        "git"
    }

    fn render(&mut self, buf: &mut RBuffer, area: RRect, theme: &tui_design::Theme, focused: bool, _editor: &Editor) {
        self.0.render_compact(area, buf, theme, focused);
        if let Some(help) = &mut self.1 {
            help.render(area, buf, theme, "Keys", &self.0.keys_rows());
        }
    }

    fn is_typing(&self) -> bool {
        self.0.is_typing() || self.1.is_some()
    }

    fn refresh(&mut self) {
        self.0.refresh();
    }

    fn handle_key(&mut self, key: ct::KeyEvent) -> PanelEvent {
        if let Some(help) = &mut self.1 {
            if !help.key(&key) {
                self.1 = None;
            }
            return PanelEvent::None;
        }
        match self.0.handle_event(&ct::Event::Key(key)) {
            git_panel::Action::Open(p) => PanelEvent::Open(p),
            git_panel::Action::Close => PanelEvent::Unfocus,
            // The palette, the help and the host commands are for hosts that draw them (the standalone tools).
            git_panel::Action::Help => {
                self.1 = Some(tui_design::keys::Help::default());
                PanelEvent::None
            }
            git_panel::Action::Changed | git_panel::Action::None | git_panel::Action::Palette | git_panel::Action::Command(_) => PanelEvent::None,
        }
    }
}

/// The problems of the workspace (what the language servers reported), as a list
/// grouped by file.
struct DiagnosticsDock(diagnostics_panel::Panel);

/// What the language servers reported, flattened for the panel.
fn diagnostics_of(editor: &Editor) -> Vec<diagnostics_panel::Item> {
    use diagnostics_panel::{Item, Severity};
    use helix_lsp::lsp::DiagnosticSeverity as Sev;
    let mut out = Vec::new();
    for (uri, list) in &editor.diagnostics {
        let Some(path) = uri.as_path() else { continue };
        for (d, _provider) in list {
            let severity = match d.severity {
                Some(s) if s == Sev::ERROR => Severity::Error,
                Some(s) if s == Sev::WARNING => Severity::Warning,
                Some(s) if s == Sev::HINT => Severity::Hint,
                _ => Severity::Info,
            };
            out.push(Item {
                path: path.to_path_buf(),
                line: d.range.start.line + 1,
                col: d.range.start.character + 1,
                severity,
                message: d.message.clone(),
                source: d.source.clone(),
            });
        }
    }
    out
}

impl DockPanel for DiagnosticsDock {
    fn id(&self) -> &'static str {
        "diagnostics"
    }

    fn render(&mut self, buf: &mut RBuffer, area: RRect, theme: &tui_design::Theme, focused: bool, editor: &Editor) {
        self.0.set_items(diagnostics_of(editor));
        self.0.render(area, buf, theme, focused);
    }

    fn handle_key(&mut self, key: ct::KeyEvent) -> PanelEvent {
        match self.0.handle_event(&ct::Event::Key(key)) {
            diagnostics_panel::Action::Open { path, line, col } => PanelEvent::OpenAt { path, line, col },
            diagnostics_panel::Action::Close => PanelEvent::Unfocus,
            diagnostics_panel::Action::None => PanelEvent::None,
        }
    }
}

fn make_panel(id: &str, root: &Path) -> Option<Box<dyn DockPanel>> {
    match id {
        "symbols" => Some(Box::new(SymbolsDock::default())),
        "diagnostics" => Some(Box::new(DiagnosticsDock(diagnostics_panel::Panel::new(root)))),
        "files" => Some(Box::new(FilesDock(files::Panel::new(root, files::PanelOptions::default()), None))),
        "git" => git_panel::Panel::new(root).map(|p| Box::new(GitDock(p, None)) as Box<dyn DockPanel>),
        _ => None,
    }
}

/// One dock: the panels assigned to it (built when first shown) and its state.
#[derive(Default)]
pub struct Dock {
    ids: Vec<String>,
    panels: Vec<Option<Box<dyn DockPanel>>>,
    active: usize,
    pub open: bool,
    pub focused: bool,
}

impl Dock {
    fn assign(&mut self, ids: &[String]) {
        if self.ids == ids {
            return;
        }
        self.ids = ids.to_vec();
        self.panels = ids.iter().map(|_| None).collect();
        self.active = self.active.min(ids.len().saturating_sub(1));
        if ids.is_empty() {
            self.open = false;
            self.focused = false;
        }
    }

    /// Open docks that hold a panel take room; empty ones never do.
    fn shown(&self) -> bool {
        self.open && !self.ids.is_empty()
    }

    fn active_panel(&mut self, root: &Path) -> Option<&mut Box<dyn DockPanel>> {
        let i = self.active;
        if self.panels.get(i)?.is_none() {
            self.panels[i] = make_panel(&self.ids[i], root);
        }
        self.panels[i].as_mut()
    }

    fn cycle(&mut self, delta: isize) {
        let n = self.ids.len() as isize;
        if n > 1 {
            self.active = (self.active as isize + delta).rem_euclid(n) as usize;
        }
    }

    /// Tab labels over the top border when the dock holds several panels.
    fn draw_tabs(&self, buf: &mut RBuffer, area: RRect, theme: &tui_design::Theme) {
        if self.ids.len() < 2 || area.width < 6 {
            return;
        }
        let c = &theme.colors;
        let mut x = area.x + 2;
        for (i, id) in self.ids.iter().enumerate() {
            let style = if i == self.active { RStyle::new().fg(c.accent.into()).add_modifier(Modifier::BOLD) } else { RStyle::new().fg(c.muted.into()) };
            let label = format!(" {id} ");
            if x + label.chars().count() as u16 >= area.right() {
                break;
            }
            buf.set_string(x, area.y, &label, style);
            x += label.chars().count() as u16;
        }
    }
}

#[derive(Default)]
pub struct Docks {
    pub left: Dock,
    pub right: Dock,
    pub bottom: Dock,
    root: Option<PathBuf>,
    theme: Option<tui_design::Theme>,
    last: Option<DocksConfig>,
    /// The file the panels last followed (the focused editor's file).
    followed: Option<PathBuf>,
    /// The preview beside the files pane: what it was made for, and its lines.
    preview: Option<(PreviewKey, Vec<ratatui::text::Line<'static>>)>,
}

type PreviewKey = (PathBuf, u16, u16, bool);

impl Docks {
    pub fn theme(&mut self) -> &tui_design::Theme {
        self.theme.get_or_insert_with(tui_design::Theme::load)
    }

    fn dock(&mut self, side: Side) -> &mut Dock {
        match side {
            Side::Left => &mut self.left,
            Side::Right => &mut self.right,
            Side::Bottom => &mut self.bottom,
        }
    }

    /// Apply the configuration (which panels go where) and the workspace root.
    /// Open/closed state survives a change; panels are rebuilt when their dock's
    /// assignment or the root changed.
    pub fn configure(&mut self, cfg: &DocksConfig, root: PathBuf) {
        if self.root.as_ref() != Some(&root) {
            for d in [&mut self.left, &mut self.right, &mut self.bottom] {
                d.panels = d.ids.iter().map(|_| None).collect();
            }
            self.root = Some(root);
        }
        if self.last.as_ref() != Some(cfg) {
            self.left.assign(&cfg.left);
            self.right.assign(&cfg.right);
            self.bottom.assign(&cfg.bottom);
            self.last = Some(cfg.clone());
        }
    }

    pub fn focused_side(&self) -> Option<Side> {
        [(Side::Left, &self.left), (Side::Right, &self.right), (Side::Bottom, &self.bottom)].into_iter().find(|(_, d)| d.shown() && d.focused).map(|(s, _)| s)
    }

    pub fn unfocus(&mut self) {
        for d in [&mut self.left, &mut self.right, &mut self.bottom] {
            d.focused = false;
        }
    }

    /// Open and focus the dock; if it is already focused, close it. A dock that
    /// holds no panel does nothing and returns `false`.
    pub fn toggle(&mut self, side: Side, current: Option<&Path>) -> bool {
        if self.dock(side).ids.is_empty() {
            return false;
        }
        if self.dock(side).shown() && self.dock(side).focused {
            let d = self.dock(side);
            d.open = false;
            d.focused = false;
            return true;
        }
        self.focus(side, current);
        true
    }

    fn focus(&mut self, side: Side, current: Option<&Path>) {
        self.unfocus();
        let root = self.root.clone().unwrap_or_default();
        let d = self.dock(side);
        d.open = true;
        d.focused = true;
        if let (Some(path), Some(panel)) = (current, d.active_panel(&root)) {
            panel.reveal(path);
        }
    }

    /// Show a panel by name in the dock it is assigned to: switch to it, open the
    /// dock and focus it; if it is already focused, close the dock. `false` if no
    /// dock holds that panel.
    pub fn show_panel(&mut self, id: &str, current: Option<&Path>) -> bool {
        let Some((side, index)) = [Side::Left, Side::Right, Side::Bottom].into_iter().find_map(|s| self.dock(s).ids.iter().position(|p| p == id).map(|i| (s, i))) else {
            return false;
        };
        let d = self.dock(side);
        let already = d.shown() && d.focused && d.active == index;
        d.active = index;
        if already {
            d.open = false;
            d.focused = false;
        } else {
            self.focus(side, current);
        }
        true
    }

    /// Rows and columns the docks take: `(left, right, bottom)` in cells.
    pub fn sizes(&mut self) -> (u16, u16, u16) {
        let (w, h) = {
            let t = self.theme();
            (t.layout.sidebar_width, t.layout.bottom_height)
        };
        // A side dock that has the keyboard grows to fit a long name (up to twice its width), and goes back when it loses it.
        let root = self.root.clone().unwrap_or_default();
        let theme = self.theme().clone();
        let mut side_width = |dock: &mut Dock| -> u16 {
            if !dock.shown() {
                return 0;
            }
            if dock.focused {
                if let Some(want) = dock.active_panel(&root).and_then(|p| p.wanted_width(&theme)) {
                    return want.clamp(w, (w * 2).max(50));
                }
            }
            w
        };
        let (left, right) = (side_width(&mut self.left), side_width(&mut self.right));
        (left, right, if self.bottom.shown() { h } else { 0 })
    }

    /// Panels show the file of the focused editor even while they do not have the keyboard.
    fn follow(&mut self, editor: &Editor) {
        let current = editor.tree.try_get(editor.tree.focus).and_then(|v| editor.document(v.doc)).and_then(|d| d.path()).cloned();
        if current == self.followed {
            return;
        }
        self.followed = current.clone();
        // A file opened from somewhere else (a picker, a jump) takes the keyboard back from the docks.
        for side in [Side::Left, Side::Right, Side::Bottom] {
            self.dock(side).focused = false;
        }
        let Some(path) = current else { return };
        for side in [Side::Left, Side::Right, Side::Bottom] {
            let d = self.dock(side);
            if d.focused {
                continue;
            }
            for panel in d.panels.iter_mut().flatten() {
                panel.reveal(&path);
            }
        }
    }

    pub fn render(&mut self, wb: &tui_design::layout::Workbench, surface: &mut Surface, editor: &Editor) {
        self.follow(editor);
        let theme = self.theme().clone();
        let root = self.root.clone().unwrap_or_default();
        for (side, rect) in [(Side::Left, wb.left), (Side::Right, wb.right), (Side::Bottom, wb.bottom)] {
            let d = self.dock(side);
            if !d.shown() || rect.width == 0 || rect.height == 0 {
                continue;
            }
            let focused = d.focused;
            let local = RRect::new(0, 0, rect.width, rect.height);
            let mut buf = RBuffer::empty(local);
            match d.active_panel(&root) {
                Some(panel) => panel.render(&mut buf, local, &theme, focused, editor),
                None => buf.set_string(1, 0, "unavailable here", RStyle::new().fg(theme.colors.muted.into())),
            }
            d.draw_tabs(&mut buf, local, &theme);
            copy(&buf, from_ratatui(rect), surface);
        }
    }

    /// Every created panel rereads what it shows.
    pub fn refresh_all(&mut self) {
        for side in [Side::Left, Side::Right, Side::Bottom] {
            for panel in self.dock(side).panels.iter_mut().flatten() {
                panel.refresh();
            }
        }
    }

    /// Route a key to the focused dock. `None` if no dock has the keyboard.
    pub fn handle_key(&mut self, key: &KeyEvent, leader: &KeyEvent) -> Option<PanelEvent> {
        let side = self.focused_side()?;
        let ev = to_crossterm(key)?;
        let root = self.root.clone().unwrap_or_default();
        let d = self.dock(side);
        // The leader key always reaches the editor, unless a prompt is typing.
        if key == leader && !d.active_panel(&root).is_some_and(|p| p.is_typing()) {
            return None;
        }
        // `[` and `]` switch between the panels of a dock.
        match ev.code {
            ct::KeyCode::Char('[') if ev.modifiers.is_empty() => {
                d.cycle(-1);
                return Some(PanelEvent::None);
            }
            ct::KeyCode::Char(']') if ev.modifiers.is_empty() => {
                d.cycle(1);
                return Some(PanelEvent::None);
            }
            _ => {}
        }
        let event = d.active_panel(&root).map(|p| p.handle_key(ev)).unwrap_or(PanelEvent::Unfocus);
        if event == PanelEvent::Unfocus {
            d.focused = false;
        }
        Some(event)
    }
}

impl Docks {
    /// Whether the editor is drawn in a bordered box (the `editor_box` token).
    pub fn editor_box(&mut self) -> bool {
        self.theme().chrome.editor_box
    }
}

/// Draw only the border of `rect` over what is already on the surface (the
/// editor's text inside stays), with the focus color when `focused`.
pub fn draw_border(surface: &mut Surface, rect: Rect, theme: &tui_design::Theme, focused: bool) {
    if rect.width < 2 || rect.height < 2 {
        return;
    }
    let local = RRect::new(0, 0, rect.width, rect.height);
    let mut buf = RBuffer::empty(local);
    tui_design::Pane::new(theme).focused(focused).render(local, &mut buf);
    let (w, h) = (rect.width, rect.height);
    for y in 0..h {
        for x in 0..w {
            if x == 0 || y == 0 || x == w - 1 || y == h - 1 {
                let src = &buf[(x, y)];
                if let Some(dst) = surface.get_mut(rect.x + x, rect.y + y) {
                    dst.set_symbol(src.symbol());
                    dst.fg = super::pick::color(src.fg);
                }
            }
        }
    }
}

/// A horizontal rule across `rect` at row `y`, joined to the box border on both ends,
/// in the border's color (the line under the tabs).
pub fn draw_rule(surface: &mut Surface, rect: Rect, y: u16, theme: &tui_design::Theme, focused: bool) {
    if rect.width < 3 || y < rect.y || y >= rect.bottom() {
        return;
    }
    let local = RRect::new(0, 0, rect.width, rect.height);
    let mut buf = RBuffer::empty(local);
    tui_design::Pane::new(theme).focused(focused).render(local, &mut buf);
    let fg = super::pick::color(buf[(0, 0)].fg);
    for x in rect.x..rect.right() {
        let sym = if x == rect.x { "├" } else if x == rect.right() - 1 { "┤" } else { "─" };
        if let Some(dst) = surface.get_mut(x, y) {
            dst.set_symbol(sym);
            dst.fg = fg;
        }
    }
}

impl Docks {
    /// While the files pane has the keyboard, a preview of the highlighted file fills the part of the
    /// editor next to it (the pane's own width stays small).
    /// Drawn last, over the editor, so nothing dims or covers it. `view` is the whole editor pane (its border included): the preview is a pane of the same height, beside the dock.
    pub fn render_preview(&mut self, wb: &tui_design::layout::Workbench, view: Rect, surface: &mut Surface, editor: &Editor) {
        if !editor.config().dock_preview {
            return;
        }
        let theme = &self.theme().clone();
        let root = self.root.clone().unwrap_or_default();
        let root = root.as_path();
        let mut target = None;
        for (side, rect) in [(Side::Left, wb.left), (Side::Right, wb.right)] {
            let d = self.dock(side);
            if !d.shown() || !d.focused {
                continue;
            }
            if let Some(panel) = d.active_panel(root) {
                if let Some(path) = panel.preview_target() {
                    target = Some((side, rect, path));
                }
            }
        }
        let Some((side, dock, path)) = target else { return };
        let editor = to_ratatui(view);
        let _ = wb.editor;
        // Half of the editor pane, at least 36 columns, from the dock to the right (or from the right edge for a right dock).
        let width = (editor.width / 2).max(36).min(editor.width.saturating_sub(1));
        let _ = dock;
        if editor.width < 50 || editor.height < 6 {
            return;
        }
        let area = match side {
            Side::Left => RRect::new(editor.x, editor.y, width, editor.height),
            _ => RRect::new(editor.right().saturating_sub(width), editor.y, width, editor.height),
        };
        let local = RRect::new(0, 0, area.width, area.height);
        let mut buf = RBuffer::empty(local);
        let pane = tui_design::Pane::new(theme).focused(true);
        let inner = pane.inner(local);
        pane.render(local, &mut buf);
        if inner.width > 0 && inner.height > 0 {
            let key: PreviewKey = (path.clone(), inner.width, inner.height, pv::raw_tables());
            if self.preview.as_ref().map(|p| &p.0) != Some(&key) {
                let opts = pv::Options { width: Some(inner.width), height: Some(inner.height), git: true, focus: None };
                let lines = pv::preview(&path, theme, &opts).unwrap_or_else(|e| vec![ratatui::text::Line::from(ratatui::text::Span::styled(e.to_string(), RStyle::new().fg(theme.colors.muted.into())))]);
                self.preview = Some((key, lines));
            }
            if let Some((_, lines)) = &self.preview {
                ratatui::widgets::Paragraph::new(lines.clone()).render(inner, &mut buf);
            }
        }
        copy(&buf, from_ratatui(area), surface);
    }
}

/// The symbols of the file being edited, as a tree to jump from.
#[derive(Default)]
struct SymbolsDock {
    panel: symbols_panel::Panel,
    /// The file the symbols are of (a jump opens it).
    path: Option<PathBuf>,
    help: Option<tui_design::keys::Help>,
}

impl DockPanel for SymbolsDock {
    fn id(&self) -> &'static str {
        "symbols"
    }

    fn render(&mut self, buf: &mut RBuffer, area: RRect, theme: &tui_design::Theme, focused: bool, _editor: &Editor) {
        self.panel.render(area, buf, theme, focused);
        if let Some(help) = &mut self.help {
            help.render(area, buf, theme, "Keys", &self.panel.keys_rows());
        }
    }

    fn is_typing(&self) -> bool {
        self.panel.is_typing() || self.help.is_some()
    }

    fn handle_key(&mut self, key: ct::KeyEvent) -> PanelEvent {
        if let Some(help) = &mut self.help {
            if !help.key(&key) {
                self.help = None;
            }
            return PanelEvent::None;
        }
        match self.panel.handle_event(&ct::Event::Key(key)) {
            symbols_panel::Action::Goto { line, col } => match &self.path {
                Some(path) => PanelEvent::OpenAt { path: path.clone(), line: line + 1, col: col + 1 },
                None => PanelEvent::None,
            },
            symbols_panel::Action::Close => PanelEvent::Unfocus,
            symbols_panel::Action::Help => {
                self.help = Some(tui_design::keys::Help::default());
                PanelEvent::None
            }
            symbols_panel::Action::Palette | symbols_panel::Action::None => PanelEvent::None,
        }
    }

    fn set_symbols(&mut self, path: &Path, symbols: Vec<symbols_panel::Symbol>) {
        self.path = Some(path.to_path_buf());
        self.panel.set_symbols(symbols);
    }

    fn follow_line(&mut self, line: Option<u32>) {
        self.panel.follow(line);
    }
}

impl Docks {
    /// Whether a symbol explorer is on screen (so the editor should ask the language server for symbols).
    pub fn wants_symbols(&mut self) -> bool {
        [Side::Left, Side::Right, Side::Bottom].into_iter().any(|side| {
            let d = self.dock(side);
            d.shown() && d.ids.get(d.active).is_some_and(|id| id == "symbols")
        })
    }

    /// Give every symbol explorer the symbols of `path`.
    pub fn set_symbols(&mut self, path: &Path, symbols: Vec<symbols_panel::Symbol>) {
        let root = self.root.clone().unwrap_or_default();
        for side in [Side::Left, Side::Right, Side::Bottom] {
            let d = self.dock(side);
            if d.ids.iter().any(|id| id == "symbols") {
                for i in 0..d.ids.len() {
                    if d.ids[i] == "symbols" {
                        if d.panels[i].is_none() {
                            d.panels[i] = make_panel(&d.ids[i], &root);
                        }
                        if let Some(p) = d.panels[i].as_mut() {
                            p.set_symbols(path, symbols.clone());
                        }
                    }
                }
            }
        }
    }

    /// Tell the panels which line the editor's cursor is on.
    pub fn follow_line(&mut self, line: Option<u32>) {
        for side in [Side::Left, Side::Right, Side::Bottom] {
            for p in self.dock(side).panels.iter_mut().flatten() {
                p.follow_line(line);
            }
        }
    }
}
