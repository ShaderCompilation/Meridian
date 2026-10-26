//! Command palette state, item building (fuzzy ranked) and activation.

use chrono::{Duration, NaiveDate};
use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};

use crate::app::{Act, App, Sel};
use crate::commands::Cmd;
use crate::dates;
use crate::model::{Id, Priority};
use crate::ui::icons;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelOp {
    Rename,
    Recolor,
    Delete,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
    Commands,
    Tasks,
    Boards,
    Priority(Id),
    Progress(Id),
    Labels(Id),
    MoveTo(Id),
    Due(Id),
    PickLabel(LabelOp),
    RenameLabel(Id),
    LabelColor(Id),
}

impl Mode {
    pub fn parent(&self) -> Option<Mode> {
        match self {
            Mode::RenameLabel(_) => Some(Mode::PickLabel(LabelOp::Rename)),
            Mode::LabelColor(_) => Some(Mode::PickLabel(LabelOp::Recolor)),
            _ => None,
        }
    }

    pub fn placeholder(&self) -> &'static str {
        match self {
            Mode::Commands => "Type a command…",
            Mode::Tasks => "Search tasks on all boards…",
            Mode::Boards => "Switch to board, or type a new name…",
            Mode::Priority(_) => "Set priority  (0–4)",
            Mode::Progress(_) => "Progress: type a percentage, e.g. 40",
            Mode::Labels(_) => "Toggle labels, or type to create one…",
            Mode::MoveTo(_) => "Move to section…",
            Mode::Due(_) => "Due date: today, tom, fri, +3d, nov 3…",
            Mode::PickLabel(LabelOp::Rename) => "Rename which label?",
            Mode::PickLabel(LabelOp::Recolor) => "Recolor which label?",
            Mode::PickLabel(LabelOp::Delete) => "Delete which label?",
            Mode::RenameLabel(_) => "New label name",
            Mode::LabelColor(_) => "Pick a color",
        }
    }

    pub fn title(&self) -> Option<&'static str> {
        match self {
            Mode::Commands => None,
            Mode::Tasks => Some("Go to task"),
            Mode::Boards => Some("Boards"),
            Mode::Priority(_) => Some("Priority"),
            Mode::Progress(_) => Some("Progress"),
            Mode::Labels(_) => Some("Labels"),
            Mode::MoveTo(_) => Some("Move to"),
            Mode::Due(_) => Some("Due date"),
            Mode::PickLabel(_) | Mode::RenameLabel(_) | Mode::LabelColor(_) => Some("Labels"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum PAction {
    Cmd(Cmd),
    Task(Id),
    Board(Id),
    CreateBoard(String),
    Priority(Priority),
    Progress(Option<u8>),
    ToggleLabel(Id),
    CreateLabel(String),
    Section(Id),
    Due(Option<NaiveDate>),
    PickLabel(Id),
    Color(u8),
    Rename(String),
}

#[derive(Debug, Clone)]
pub struct Item {
    pub icon: char,
    pub title: String,
    pub detail: String,
    pub hint: String,
    pub action: PAction,
    pub checked: Option<bool>,
    pub swatch: Option<u8>,
    pub priority: Option<Priority>,
}

impl Item {
    fn new(icon: char, title: impl Into<String>, action: PAction) -> Self {
        Item {
            icon,
            title: title.into(),
            detail: String::new(),
            hint: String::new(),
            action,
            checked: None,
            swatch: None,
            priority: None,
        }
    }

    fn hint(mut self, h: impl Into<String>) -> Self {
        self.hint = h.into();
        self
    }

    fn detail(mut self, d: impl Into<String>) -> Self {
        self.detail = d.into();
        self
    }

    fn checked(mut self, c: bool) -> Self {
        self.checked = Some(c);
        self
    }
}

pub struct Palette {
    pub mode: Mode,
    pub query: String,
    pub items: Vec<Item>,
    pub sel: usize,
    /// Calendar cursor for the due-date picker.
    pub cal: NaiveDate,
    /// The calendar cursor was moved with the keyboard: Enter picks it.
    pub cal_active: bool,
    /// Scroll offset of the item list.
    pub offset: f32,
}

impl Palette {
    pub fn new(mode: Mode, query: String, cal: NaiveDate) -> Self {
        Palette { mode, query, items: vec![], sel: 0, cal, cal_active: false, offset: 0.0 }
    }
}

/// Fuzzy-rank `(haystack, item)` pairs. Empty query keeps the original order.
fn rank(query: &str, items: Vec<(String, Item)>, limit: usize) -> Vec<Item> {
    let q = query.trim();
    if q.is_empty() {
        return items.into_iter().take(limit).map(|(_, i)| i).collect();
    }
    let mut matcher = Matcher::new(Config::DEFAULT);
    let pattern = Pattern::parse(q, CaseMatching::Ignore, Normalization::Smart);
    let mut buf = Vec::new();
    let mut scored: Vec<(u32, usize, Item)> = items
        .into_iter()
        .enumerate()
        .filter_map(|(i, (hay, item))| {
            pattern
                .score(Utf32Str::new(&hay, &mut buf), &mut matcher)
                .map(|s| (s, i, item))
        })
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    scored.into_iter().take(limit).map(|(_, _, i)| i).collect()
}

const PROGRESS_PRESETS: [u8; 7] = [0, 10, 25, 50, 75, 90, 100];

/// Parses "40", "40%" or " 40 % " into a percentage, clamped to 100.
fn parse_percent(q: &str) -> Option<u8> {
    let n: u32 = q.trim().trim_end_matches('%').trim().parse().ok()?;
    Some(n.min(100) as u8)
}

pub fn build(app: &App, mode: &Mode, query: &str) -> Vec<Item> {
    let store = &app.store;
    match mode {
        Mode::Commands => {
            let mut order: Vec<Cmd> = app.recent.clone();
            for c in Cmd::ALL {
                if !order.contains(c) && *c != Cmd::Palette {
                    order.push(*c);
                }
            }
            let items = order
                .into_iter()
                .map(|c| {
                    let m = c.meta();
                    let mut item = Item::new(m.icon, m.title, PAction::Cmd(c)).hint(m.keys);
                    if app.recent.contains(&c) && query.trim().is_empty() {
                        item.detail = "Recent".into();
                    }
                    (format!("{} {}", m.title, m.alias), item)
                })
                .collect();
            rank(query, items, 100)
        }
        Mode::Tasks => {
            let mut tasks: Vec<_> = store.tasks.iter().collect();
            // Current board first.
            tasks.sort_by_key(|t| store.board_of_task(t) != app.board);
            let items = tasks
                .into_iter()
                .filter_map(|t| {
                    let sec = store.section(t.section_id)?;
                    let board = store.board(sec.board_id)?;
                    let mut item = Item::new(
                        if t.completed_at.is_some() { icons::CIRCLE_CHECK } else { icons::CIRCLE },
                        t.title.clone(),
                        PAction::Task(t.id),
                    )
                    .detail(format!("{} › {}", board.name, sec.name));
                    item.priority = Some(t.priority);
                    Some((format!("{} {} {}", t.title, sec.name, board.name), item))
                })
                .collect();
            rank(query, items, 80)
        }
        Mode::Boards => {
            let items = store
                .boards
                .iter()
                .enumerate()
                .map(|(i, b)| {
                    let n = store.task_count(b.id);
                    let item = Item::new(icons::KANBAN, b.name.clone(), PAction::Board(b.id))
                        .detail(format!("{n} task{}", if n == 1 { "" } else { "s" }))
                        .hint(if i < 9 { format!("Ctrl+{}", i + 1) } else { String::new() })
                        .checked(app.board == Some(b.id));
                    (b.name.clone(), item)
                })
                .collect();
            let mut out = rank(query, items, 50);
            let q = query.trim();
            if !q.is_empty() && !store.boards.iter().any(|b| b.name.eq_ignore_ascii_case(q)) {
                out.push(Item::new(icons::PLUS, format!("Create board “{q}”"), PAction::CreateBoard(q.into())));
            }
            out
        }
        Mode::Priority(task) => {
            let cur = store.task(*task).map(|t| t.priority);
            let items = Priority::ALL
                .iter()
                .map(|p| {
                    let mut item = Item::new(icons::FLAG, p.label(), PAction::Priority(*p))
                        .hint((*p as u8).to_string())
                        .checked(cur == Some(*p));
                    item.priority = Some(*p);
                    (p.label().to_string(), item)
                })
                .collect();
            rank(query, items, 10)
        }
        Mode::Progress(task) => {
            let Some(t) = store.task(*task) else { return vec![] };
            if let Some(v) = parse_percent(query) {
                return vec![Item::new(icons::GAUGE, format!("Set to {v}%"), PAction::Progress(Some(v))).hint("Enter")];
            }
            let mut items: Vec<(String, Item)> = PROGRESS_PRESETS
                .iter()
                .map(|v| {
                    let label = format!("{v}%");
                    let item = Item::new(icons::GAUGE, label.clone(), PAction::Progress(Some(*v)))
                        .checked(t.progress == Some(*v));
                    (label, item)
                })
                .collect();
            let (_, total) = t.checklist();
            let auto_detail = if total > 0 {
                let mut auto = t.clone();
                auto.progress = None;
                format!("{}% from checklist", auto.percent().unwrap_or(0))
            } else {
                "No checklist".into()
            };
            items.push((
                "Automatic checklist reset clear".into(),
                Item::new(icons::LIST_CHECKS, "Automatic", PAction::Progress(None))
                    .detail(auto_detail)
                    .checked(t.progress.is_none()),
            ));
            rank(query, items, 10)
        }
        Mode::Labels(task) => {
            let Some(t) = store.task(*task) else { return vec![] };
            let Some(board) = store.board_of_task(t) else { return vec![] };
            let items = store
                .labels_of(board)
                .map(|l| {
                    let mut item =
                        Item::new(icons::TAG, l.name.clone(), PAction::ToggleLabel(l.id)).checked(t.labels.contains(&l.id));
                    item.swatch = Some(l.color);
                    (l.name.clone(), item)
                })
                .collect();
            let mut out = rank(query, items, 50);
            let q = query.trim();
            if !q.is_empty() && !store.labels_of(board).any(|l| l.name.eq_ignore_ascii_case(q)) {
                out.push(Item::new(icons::PLUS, format!("Create label “{q}”"), PAction::CreateLabel(q.into())));
            }
            out
        }
        Mode::MoveTo(task) => {
            let Some(t) = store.task(*task) else { return vec![] };
            let Some(board) = store.board_of_task(t) else { return vec![] };
            let items = store
                .sections_of(board)
                .map(|s| {
                    let n = store.tasks_of(s.id).count();
                    let item = Item::new(icons::COLUMNS, s.name.clone(), PAction::Section(s.id))
                        .detail(format!("{n}"))
                        .checked(s.id == t.section_id);
                    (s.name.clone(), item)
                })
                .collect();
            rank(query, items, 50)
        }
        Mode::Due(task) => {
            let today = app.today;
            let has_due = store.task(*task).and_then(|t| t.due).is_some();
            let mut out = vec![];
            if let Some(d) = dates::parse(query, today, app.settings.week_start) {
                out.push(
                    Item::new(icons::CALENDAR, dates::long(d), PAction::Due(Some(d)))
                        .detail(dates::short(d, today))
                        .hint("Enter"),
                );
            }
            let next_week = dates::next_week(today, app.settings.week_start);
            let presets = [
                ("Today", today),
                ("Tomorrow", today + Duration::days(1)),
                ("Next week", next_week),
                ("In two weeks", today + Duration::days(14)),
            ];
            let mut items: Vec<(String, Item)> = presets
                .iter()
                .map(|(name, d)| {
                    (
                        name.to_string(),
                        Item::new(icons::CALENDAR, *name, PAction::Due(Some(*d))).detail(dates::short(*d, today)),
                    )
                })
                .collect();
            if has_due {
                items.push(("Clear remove none".into(), Item::new(icons::CALENDAR_X, "Clear due date", PAction::Due(None))));
            }
            if out.is_empty() {
                out.extend(rank(query, items, 10));
            } else if query.trim().is_empty() {
                out.extend(items.into_iter().map(|(_, i)| i));
            }
            out
        }
        Mode::PickLabel(op) => {
            let Some(board) = app.board else { return vec![] };
            let items = store
                .labels_of(board)
                .map(|l| {
                    let icon = match op {
                        LabelOp::Delete => icons::TRASH,
                        LabelOp::Recolor => icons::PALETTE,
                        LabelOp::Rename => icons::PENCIL,
                    };
                    let mut item = Item::new(icon, l.name.clone(), PAction::PickLabel(l.id));
                    item.swatch = Some(l.color);
                    (l.name.clone(), item)
                })
                .collect();
            rank(query, items, 50)
        }
        Mode::RenameLabel(_) => {
            let q = query.trim();
            if q.is_empty() {
                vec![]
            } else {
                vec![Item::new(icons::PENCIL, format!("Rename to “{q}”"), PAction::Rename(q.into())).hint("Enter")]
            }
        }
        Mode::LabelColor(id) => {
            let cur = store.label(*id).map(|l| l.color);
            let items = crate::theme::LABEL_COLORS
                .iter()
                .enumerate()
                .map(|(i, (_, _, name))| {
                    let mut item = Item::new(icons::CIRCLE, *name, PAction::Color(i as u8)).checked(cur == Some(i as u8));
                    item.swatch = Some(i as u8);
                    (name.to_string(), item)
                })
                .collect();
            rank(query, items, 10)
        }
    }
}

pub fn default_selection(_app: &App, p: &Palette) -> usize {
    match p.mode {
        Mode::Priority(_) | Mode::Progress(_) | Mode::MoveTo(_) | Mode::LabelColor(_) | Mode::Boards
            if p.query.is_empty() =>
        {
            p.items.iter().position(|i| i.checked == Some(true)).unwrap_or(0)
        }
        _ => 0,
    }
}

pub fn activate(app: &mut App, mode: Mode, query: String, item: Option<Item>) -> Act {
    let Some(item) = item else {
        return Act::none();
    };
    match item.action {
        PAction::Cmd(c) => {
            app.palette = None;
            app.run(c)
        }
        PAction::Task(id) => {
            let close = app.close_palette();
            let Some(t) = app.store.task(id) else { return close };
            let section = t.section_id;
            let Some(board) = app.store.board_of_task(t) else { return close };
            app.detail = None;
            app.switch_board_raw(board);
            if !app.filter.matches(app.store.task(id).unwrap(), &app.store, app.today) {
                app.filter_text.clear();
                app.filter = Default::default();
            }
            if app.store.section(section).is_some_and(|s| s.collapsed) {
                app.update_section(section, |s| s.collapsed = false);
            }
            app.sel = Sel { section: Some(section), task: Some(id) };
            app.ensure_sel();
            let r = app.reveal();
            Act::batch([close, r])
        }
        PAction::Board(id) => {
            let close = app.close_palette();
            let s = app.switch_board(id);
            Act::batch([close, s])
        }
        PAction::CreateBoard(name) => {
            let close = app.close_palette();
            app.edit = Some(crate::app::Edit::NewBoard { value: name });
            let a = app.finish_edit(true);
            Act::batch([close, a])
        }
        PAction::Priority(p) => {
            if let Mode::Priority(t) = mode {
                app.set_priority(t, p);
            }
            app.close_palette()
        }
        PAction::Progress(v) => {
            if let Mode::Progress(t) = mode {
                app.set_progress(t, v);
            }
            app.close_palette()
        }
        PAction::ToggleLabel(l) => {
            if let Mode::Labels(t) = mode {
                app.toggle_label(t, l);
            }
            app.refresh_palette();
            Act::none()
        }
        PAction::CreateLabel(name) => {
            if let Mode::Labels(t) = mode {
                app.create_label(Some(t), name);
            }
            if let Some(p) = &mut app.palette {
                p.query.clear();
            }
            app.refresh_palette();
            Act::none()
        }
        PAction::Section(s) => {
            if let Mode::MoveTo(t) = mode {
                app.move_task_to(t, s, None);
                if app.detail.is_some() || app.sel.task == Some(t) {
                    app.sel = Sel { section: Some(s), task: Some(t) };
                }
            }
            let close = app.close_palette();
            let r = app.reveal();
            Act::batch([close, r])
        }
        PAction::Due(d) => {
            if let Mode::Due(t) = mode {
                app.set_due(t, d);
            }
            app.close_palette()
        }
        PAction::PickLabel(l) => match mode {
            Mode::PickLabel(LabelOp::Rename) => app.open_palette(Mode::RenameLabel(l)),
            Mode::PickLabel(LabelOp::Recolor) => app.open_palette(Mode::LabelColor(l)),
            Mode::PickLabel(LabelOp::Delete) => {
                app.delete_label(l);
                app.close_palette()
            }
            _ => Act::none(),
        },
        PAction::Color(c) => {
            if let Mode::LabelColor(l) = mode {
                app.update_label(l, |x| x.color = c);
            }
            app.close_palette()
        }
        PAction::Rename(name) => {
            if let Mode::RenameLabel(l) = mode {
                let name = if name.is_empty() { query } else { name };
                app.update_label(l, |x| x.name = name);
            }
            app.close_palette()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_parsing() {
        assert_eq!(parse_percent("40"), Some(40));
        assert_eq!(parse_percent(" 75 % "), Some(75));
        assert_eq!(parse_percent("250"), Some(100));
        assert_eq!(parse_percent(""), None);
        assert_eq!(parse_percent("auto"), None);
        assert_eq!(parse_percent("-5"), None);
    }
}
