//! Application state, messages, update loop and subscriptions.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use chrono::NaiveDate;
use iced::widget::operation::{self, AbsoluteOffset};
use iced::widget::{Id as WId, text_editor};
use iced::{Point, Rectangle, Size, Subscription, Theme, event, keyboard, mouse, window};

use crate::commands::Cmd;
use crate::dates;
use crate::db::Db;
use crate::filter::Filter;
use crate::model::*;
use crate::palette::{self, Palette};
use crate::settings::{self as prefs, Settings, ThemePref};
use crate::theme;

mod keys;
pub use keys::{Field, Property};

pub type Act = iced::Task<Message>;

pub const INLINE: WId = WId::new("inline");
pub const FILTER_ID: WId = WId::new("filter");
pub const PALETTE_ID: WId = WId::new("palette");
pub const PALETTE_LIST: WId = WId::new("palette-list");
pub const DETAIL_TITLE: WId = WId::new("detail-title");
pub const DETAIL_DESC: WId = WId::new("detail-desc");
pub const DETAIL_SUB: WId = WId::new("detail-sub");
pub const SUB_EDIT: WId = WId::new("sub-edit");
pub const BOARD_SCROLL: WId = WId::new("board-scroll");
pub const HELP_SCROLL: WId = WId::new("help-scroll");
pub const NOWHERE: WId = WId::new("nowhere");

pub fn sec_scroll_id(id: Id) -> WId {
    WId::from(format!("sec-{id}"))
}

// Layout metrics shared with the views (needed for scroll-into-view maths).
pub const COL_W: f32 = 300.0;
pub const COL_COLLAPSED_W: f32 = 52.0;
pub const COL_GAP: f32 = 14.0;
pub const BOARD_PAD: f32 = 20.0;
pub const SIDEBAR_W: f32 = 232.0;
pub const DETAIL_W: f32 = 460.0;

#[derive(Debug, Clone)]
pub struct KeyPress {
    pub key: keyboard::Key,
    pub modified: keyboard::Key,
    pub physical: keyboard::key::Physical,
    pub mods: keyboard::Modifiers,
    pub captured: bool,
}

#[derive(Debug, Clone)]
pub enum Message {
    Key(KeyPress),
    Cmd(Cmd),
    /// A command from a button in the task panel: acts on the open task.
    PanelCmd(Cmd),
    /// Click on the task panel background.
    FocusPanel,
    /// Left mouse button pressed anywhere (focus may have moved).
    MouseDown,
    WindowFocused,
    /// Which text field really has focus, measured at focus generation `u64`.
    FocusSync(u64, Option<WId>),

    // Board mouse interaction
    PressTask(Id, Point),
    PressSection(Id, Point),
    OpenTaskMouse(Id),
    RenameSectionMouse(Id),
    CursorMoved(Point),
    MouseReleased,
    ColumnBounds(Id, Rectangle),
    CardBounds(Id, Rectangle),
    SectionView(Id, Rectangle),
    SectionScrolled(Id, f32),
    BoardScrolled(f32, f32),
    BoardView(Rectangle),
    BoardContent(Rectangle),
    SectionContent(Id, Rectangle),
    WindowResized(Size),
    CloseRequested,

    AddTaskAt(Id),
    AddSection,
    ToggleCollapse(Id),
    EditInput(String),
    EditSubmit,

    // Detail panel
    DetailTitle(String),
    DetailTitleSubmit,
    DetailDesc(text_editor::Action),
    DetailSubInput(String),
    DetailSubSubmit,
    SubToggle(Id),
    SubOpen(Id),
    SubDelete(Id),
    SubEditInput(String),
    SubEditSubmit,
    DetailClose,
    RemoveLabel(Id),

    // Palette
    PaletteInput(String),
    PaletteActivate(usize),
    PaletteClose,
    CalMonth(i32),
    PickDate(NaiveDate),

    // Filter
    FilterInput(String),
    FilterSubmit,
    FilterClear,

    SelectBoard(Id),
    SystemTheme(iced::theme::Mode),
    Tick,
    ToastUndo,
    Noop,
    SettingSet(prefs::Key, usize),
    SettingsClose,
    CopyDbPath,
    /// Dev scripting (see `devtools`).
    Script(usize),
    Shot(String, iced::window::Screenshot, usize),
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Sel {
    pub section: Option<Id>,
    /// `None` means the section header itself is selected.
    pub task: Option<Id>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Edit {
    /// New task inserted before `before` (or at the end).
    NewTask {
        section: Id,
        before: Option<Id>,
        value: String,
    },
    Task {
        id: Id,
        value: String,
    },
    Section {
        id: Id,
        value: String,
    },
    NewSection {
        after: Option<Id>,
        value: String,
    },
    Board {
        id: Id,
        value: String,
    },
    NewBoard {
        value: String,
    },
}

impl Edit {
    pub fn value(&self) -> &str {
        match self {
            Edit::NewTask { value, .. }
            | Edit::Task { value, .. }
            | Edit::Section { value, .. }
            | Edit::NewSection { value, .. }
            | Edit::Board { value, .. }
            | Edit::NewBoard { value } => value,
        }
    }

    fn value_mut(&mut self) -> &mut String {
        match self {
            Edit::NewTask { value, .. }
            | Edit::Task { value, .. }
            | Edit::Section { value, .. }
            | Edit::NewSection { value, .. }
            | Edit::Board { value, .. }
            | Edit::NewBoard { value } => value,
        }
    }
}

pub struct Detail {
    pub task: Id,
    pub title: String,
    pub desc: text_editor::Content,
    pub sub_input: String,
    pub sub_sel: Option<usize>,
    pub prop_sel: Option<Property>,
    pub sub_edit: Option<(Id, String)>,
    /// Keys go to the panel (true) or to the board, with the panel following
    /// the board selection (false).
    pub focused: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DragItem {
    Task(Id),
    Section(Id),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DropTarget {
    /// Insert the dragged task before `before` (or at the end) of `section`.
    Task { section: Id, before: Option<Id> },
    /// Insert the dragged section before `before` (or at the end).
    Section { before: Option<Id> },
}

pub struct Drag {
    pub item: DragItem,
    /// Cursor position relative to the pressed element.
    pub grab: Point,
    /// Window cursor position where the drag was first observed.
    pub origin: Option<Point>,
    pub active: bool,
    pub target: Option<DropTarget>,
    pub size: Size,
}

pub struct Toast {
    pub text: String,
    pub undo: bool,
    pub until: Instant,
}

pub struct App {
    pub db: Db,
    pub store: Store,
    undo: Vec<Vec<Change>>,
    redo: Vec<Vec<Change>>,
    pub board: Option<Id>,
    pub sel: Sel,
    board_sel: HashMap<Id, Sel>,
    pub edit: Option<Edit>,
    pub detail: Option<Detail>,
    pub palette: Option<Palette>,
    pub help: bool,
    pub filter_text: String,
    pub filter: Filter,
    /// The text field that has keyboard focus, if any.
    pub focus: Option<Field>,
    /// Bumped on every focus change we make; stale `FocusSync` results are ignored.
    focus_gen: u64,
    pub sidebar: bool,
    pub toast: Option<Toast>,
    pub chord: Option<char>,
    pub drag: Option<Drag>,
    pub settings: Settings,
    pub settings_open: bool,
    pub settings_sel: usize,
    pub system_dark: bool,
    dark_theme: Theme,
    light_theme: Theme,
    pub window: Size,
    pub cursor: Point,
    board_offset: f32,
    board_view_w: Option<f32>,
    /// Measured board viewport and content widths, used to clamp `board_offset`.
    board_view: f32,
    board_content_w: f32,
    sec_offset: HashMap<Id, f32>,
    /// Measured content height of each section's card list.
    sec_content_h: HashMap<Id, f32>,
    sec_view: HashMap<Id, Rectangle>,
    card_bounds: HashMap<Id, Rectangle>,
    col_bounds: HashMap<Id, Rectangle>,
    pub recent: Vec<Cmd>,
    pub today: NaiveDate,
}

fn now_ts() -> i64 {
    chrono::Utc::now().timestamp()
}

pub fn data_path() -> std::path::PathBuf {
    if let Ok(p) = std::env::var("MERIDIAN_DB") {
        return p.into();
    }
    directories::ProjectDirs::from("", "", "meridian")
        .map(|d| d.data_dir().join("meridian.db"))
        .unwrap_or_else(|| "meridian.db".into())
}

pub fn demo_mode() -> bool {
    std::env::args().any(|arg| arg == "--demo")
}

impl App {
    pub fn boot() -> (App, Act) {
        let demo = demo_mode();
        let path = data_path();
        let db = if demo { Db::open_in_memory() } else { Db::open(&path) }.unwrap_or_else(|e| {
            eprintln!(
                "meridian: cannot open database{}: {e}",
                if demo {
                    " for demo".into()
                } else {
                    format!(" at {}", path.display())
                }
            );
            std::process::exit(1);
        });
        let store = db.load().unwrap_or_else(|e| {
            eprintln!("meridian: cannot read database: {e}");
            std::process::exit(1);
        });
        let settings = Settings::load(&db);
        let sidebar = db.setting("sidebar").as_deref() != Some("0");
        let last_board = db.setting("board").and_then(|b| b.parse::<Id>().ok());

        let mut app = App {
            db,
            store,
            undo: vec![],
            redo: vec![],
            board: None,
            sel: Sel::default(),
            board_sel: HashMap::new(),
            edit: None,
            detail: None,
            palette: None,
            help: false,
            filter_text: String::new(),
            filter: Filter::default(),
            focus: None,
            focus_gen: 0,
            sidebar,
            toast: None,
            chord: None,
            drag: None,
            settings,
            settings_open: false,
            settings_sel: 0,
            system_dark: true,
            dark_theme: theme::make_theme(true),
            light_theme: theme::make_theme(false),
            window: Size::new(1280.0, 800.0),
            cursor: Point::ORIGIN,
            board_offset: 0.0,
            board_view_w: None,
            board_view: 0.0,
            board_content_w: 0.0,
            sec_offset: HashMap::new(),
            sec_content_h: HashMap::new(),
            sec_view: HashMap::new(),
            card_bounds: HashMap::new(),
            col_bounds: HashMap::new(),
            recent: vec![],
            today: dates::today(),
        };

        if app.store.boards.is_empty() {
            app.seed();
        }
        app.board = last_board
            .filter(|b| app.store.board(*b).is_some())
            .or_else(|| app.store.boards.first().map(|b| b.id));
        app.ensure_sel();

        let detect = iced::system::theme().map(Message::SystemTheme);
        (app, Act::batch([detect, crate::devtools::start()]))
    }

    pub fn theme(&self) -> Theme {
        if self.is_dark() {
            self.dark_theme.clone()
        } else {
            self.light_theme.clone()
        }
    }

    pub fn is_dark(&self) -> bool {
        match self.settings.theme {
            ThemePref::System => self.system_dark,
            ThemePref::Dark => true,
            ThemePref::Light => false,
        }
    }

    pub fn title(&self) -> String {
        match self.board.and_then(|b| self.store.board(b)) {
            Some(b) => format!("{} — Meridian", b.name),
            None => "Meridian".into(),
        }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let mut subs = vec![
            event::listen_with(|event, status, _window| match event {
                iced::Event::Keyboard(keyboard::Event::KeyPressed {
                    key,
                    modified_key,
                    physical_key,
                    modifiers,
                    ..
                }) => Some(Message::Key(KeyPress {
                    key,
                    modified: modified_key,
                    physical: physical_key,
                    mods: modifiers,
                    captured: status == event::Status::Captured,
                })),
                iced::Event::Window(window::Event::Resized(size)) => Some(Message::WindowResized(size)),
                iced::Event::Window(window::Event::Opened { size, .. }) => Some(Message::WindowResized(size)),
                iced::Event::Window(window::Event::CloseRequested) => Some(Message::CloseRequested),
                iced::Event::Window(window::Event::Focused) => Some(Message::WindowFocused),
                iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => Some(Message::MouseDown),
                _ => None,
            }),
            iced::system::theme_changes().map(Message::SystemTheme),
            iced::time::every(Duration::from_secs(60)).map(|_| Message::Tick),
        ];
        if self.drag.is_some() {
            subs.push(event::listen_with(|event, _status, _window| match event {
                iced::Event::Mouse(mouse::Event::CursorMoved { position }) => Some(Message::CursorMoved(position)),
                iced::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => Some(Message::MouseReleased),
                iced::Event::Mouse(mouse::Event::CursorLeft) => Some(Message::MouseReleased),
                _ => None,
            }));
        }
        if self.toast.is_some() {
            subs.push(iced::time::every(Duration::from_millis(250)).map(|_| Message::Tick));
        }
        Subscription::batch(subs)
    }

    // ------------------------------------------------------------------
    // Seed data
    // ------------------------------------------------------------------

    fn seed(&mut self) {
        let mut ch = vec![];
        let board = Board {
            id: self.store.alloc_id(),
            name: "My Board".into(),
            position: POS_STEP,
        };
        ch.push(Change::Board(None, Some(board.clone())));
        let mut secs = vec![];
        for (i, name) in ["Todo", "Planned", "In Progress", "Done"].iter().enumerate() {
            let s = Section {
                id: self.store.alloc_id(),
                board_id: board.id,
                name: name.to_string(),
                position: (i as f64 + 1.0) * POS_STEP,
                collapsed: false,
            };
            secs.push(s.id);
            ch.push(Change::Section(None, Some(s)));
        }
        let mut labels = vec![];
        for (name, color) in [("Feature", 5u8), ("Bug", 0), ("Tip", 6)] {
            let l = Label {
                id: self.store.alloc_id(),
                board_id: board.id,
                name: name.into(),
                color,
            };
            labels.push(l.id);
            ch.push(Change::Label(None, Some(l)));
        }
        let today = self.today;
        let mk = |store: &mut Store,
                  section: usize,
                  pos: f64,
                  title: &str,
                  desc: &str,
                  priority: Priority,
                  due: Option<NaiveDate>,
                  lbls: Vec<Id>,
                  subs: &[(&str, bool)],
                  done: bool| {
            let parent = Task {
                id: store.alloc_id(),
                section_id: secs[section],
                title: title.into(),
                description: desc.into(),
                priority,
                due,
                position: pos,
                created_at: now_ts(),
                completed_at: done.then(now_ts),
                progress: None,
                labels: lbls,
                parent_id: None,
                child_pos: 0.0,
            };
            let mut out = vec![];
            for (i, (t, d)) in subs.iter().enumerate() {
                out.push(Change::task(
                    None,
                    Some(Task {
                        id: store.alloc_id(),
                        title: t.to_string(),
                        description: String::new(),
                        priority: Priority::None,
                        due: None,
                        position: pos + (i + 1) as f64,
                        completed_at: d.then(now_ts),
                        labels: vec![],
                        parent_id: Some(parent.id),
                        child_pos: (i + 1) as f64 * POS_STEP,
                        ..parent.clone()
                    }),
                ));
            }
            out.insert(0, Change::task(None, Some(parent)));
            out
        };
        let s = &mut self.store;
        ch.extend(mk(
            s,
            0,
            POS_STEP,
            "Welcome to Meridian",
            "Meridian is a keyboard-first board. Everything is saved instantly to a local database.\n\nOpen the command palette with Ctrl+K to see every action and its shortcut.",
            Priority::None,
            None,
            vec![labels[2]],
            &[
                ("Move around with h j k l or the arrow keys", false),
                ("Press Enter to open a task, Esc to close it", false),
                ("Press n to add a task, N to add a section", false),
                ("Move tasks with Shift + h j k l", false),
                ("Press ? to see all shortcuts", false),
            ],
            false,
        ));
        ch.extend(mk(
            s,
            0,
            2.0 * POS_STEP,
            "Drag me to another section",
            "",
            Priority::Low,
            None,
            vec![labels[2]],
            &[],
            false,
        ));
        ch.extend(mk(
            s,
            1,
            POS_STEP,
            "Plan the week",
            "Pick the three most important outcomes.",
            Priority::Medium,
            Some(today + chrono::Duration::days(2)),
            vec![labels[0]],
            &[
                ("Review last week", true),
                ("Pick priorities", false),
                ("Block focus time", false),
            ],
            false,
        ));
        ch.extend(mk(
            s,
            2,
            POS_STEP,
            "Fix the flaky login test",
            "",
            Priority::High,
            Some(today),
            vec![labels[1]],
            &[],
            false,
        ));
        ch.extend(mk(
            s,
            3,
            POS_STEP,
            "Install Meridian",
            "",
            Priority::None,
            None,
            vec![],
            &[],
            true,
        ));

        for c in &ch {
            self.store.apply(c);
        }
        let _ = self.db.apply(&ch);
    }

    // ------------------------------------------------------------------
    // Queries
    // ------------------------------------------------------------------

    pub fn board_sections(&self) -> Vec<&Section> {
        match self.board {
            Some(b) => self.store.sections_of(b).collect(),
            None => vec![],
        }
    }

    pub fn visible_tasks(&self, section: &Section) -> Vec<&Task> {
        if section.collapsed {
            return vec![];
        }
        self.store
            .tasks_of(section.id)
            .filter(|t| self.on_board(t) && self.filter.matches(t, &self.store, self.today))
            .collect()
    }

    /// Subtasks live inside their parent unless the board is set to show them.
    pub fn on_board(&self, t: &Task) -> bool {
        self.settings.show_subtasks || t.parent_id.is_none()
    }

    /// The card that stands for `id` on the board: the task itself, or its
    /// nearest ancestor that is shown when subtasks are hidden.
    pub fn board_anchor(&self, id: Id) -> Option<&Task> {
        let t = self.store.task(id)?;
        if self.on_board(t) {
            return Some(t);
        }
        self.store.ancestors_of(id).into_iter().find(|a| self.on_board(a))
    }

    /// Visible structure of the current board: (section id, visible task ids).
    pub fn grid(&self) -> Vec<(Id, Vec<Id>)> {
        self.board_sections()
            .into_iter()
            .map(|s| (s.id, self.visible_tasks(s).iter().map(|t| t.id).collect()))
            .collect()
    }

    pub fn selected_task(&self) -> Option<&Task> {
        self.sel.task.and_then(|t| self.store.task(t))
    }

    /// The task commands act on. In the task panel: the selected subtask, else
    /// the open task. On the board: the selected card.
    pub fn target_task(&self) -> Option<Id> {
        match &self.detail {
            Some(d) if d.focused => d.sub_sel.and_then(|i| self.subtask_at(i)).or(Some(d.task)),
            _ => self.sel.task,
        }
    }

    pub fn panel_focused(&self) -> bool {
        self.detail.as_ref().is_some_and(|d| d.focused)
    }

    pub fn filter_focused(&self) -> bool {
        self.focus == Some(Field::Filter)
    }

    pub fn ensure_sel(&mut self) {
        let grid = self.grid();
        if grid.is_empty() {
            self.sel = Sel::default();
            return;
        }
        if let Some(t) = self.sel.task {
            if let Some((s, _)) = grid.iter().find(|(_, ts)| ts.contains(&t)) {
                self.sel.section = Some(*s);
                return;
            }
            self.sel.task = None;
        }
        if !self.sel.section.is_some_and(|s| grid.iter().any(|(g, _)| *g == s)) {
            let (s, ts) = &grid[0];
            self.sel = Sel {
                section: Some(*s),
                task: ts.first().copied(),
            };
        }
    }

    // ------------------------------------------------------------------
    // History
    // ------------------------------------------------------------------

    pub fn commit(&mut self, changes: Vec<Change>) {
        if changes.is_empty() {
            return;
        }
        for c in &changes {
            self.store.apply(c);
        }
        if let Err(e) = self.db.apply(&changes) {
            self.notify(format!("Could not save: {e}"), false);
        }
        self.undo.push(changes);
        if self.undo.len() > 1000 {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    fn undo(&mut self) {
        self.flush_detail();
        let Some(changes) = self.undo.pop() else {
            self.notify("Nothing to undo".into(), false);
            return;
        };
        let inverse: Vec<Change> = changes.iter().rev().map(Change::inverse).collect();
        for c in &inverse {
            self.store.apply(c);
        }
        if let Err(e) = self.db.apply(&inverse) {
            self.notify(format!("Could not save: {e}"), false);
        }
        self.focus_on_changes(&inverse);
        self.redo.push(changes);
        self.after_history();
        self.notify("Undone".into(), false);
    }

    fn redo(&mut self) {
        self.flush_detail();
        let Some(changes) = self.redo.pop() else {
            self.notify("Nothing to redo".into(), false);
            return;
        };
        for c in &changes {
            self.store.apply(c);
        }
        if let Err(e) = self.db.apply(&changes) {
            self.notify(format!("Could not save: {e}"), false);
        }
        self.focus_on_changes(&changes);
        self.undo.push(changes);
        self.after_history();
        self.notify("Redone".into(), false);
    }

    /// After undo/redo, select the task that was affected (if it still exists).
    fn focus_on_changes(&mut self, changes: &[Change]) {
        for c in changes.iter().rev() {
            match c {
                Change::Task(_, Some(t)) => {
                    let Some(t) = self.board_anchor(t.id).cloned() else {
                        return;
                    };
                    if let Some(b) = self.store.board_of_task(&t) {
                        self.switch_board_raw(b);
                        self.sel = Sel {
                            section: Some(t.section_id),
                            task: Some(t.id),
                        };
                    }
                    return;
                }
                Change::Section(_, Some(s)) => {
                    self.switch_board_raw(s.board_id);
                    self.sel = Sel {
                        section: Some(s.id),
                        task: None,
                    };
                    return;
                }
                Change::Board(_, Some(b)) => {
                    self.switch_board_raw(b.id);
                    return;
                }
                _ => {}
            }
        }
    }

    fn after_history(&mut self) {
        if self.board.is_none_or(|b| self.store.board(b).is_none()) {
            self.board = self.store.boards.first().map(|b| b.id);
        }
        self.ensure_sel();
        self.sync_detail();
    }

    /// Reload the detail drafts from the store (after external changes).
    pub fn sync_detail(&mut self) {
        let Some(d) = &mut self.detail else { return };
        match self.store.task(d.task) {
            Some(t) => {
                if d.title != t.title {
                    d.title = t.title.clone();
                }
                if d.desc.text().trim_end() != t.description {
                    d.desc = text_editor::Content::with_text(&t.description);
                }
                let len = self.store.children_of(d.task).len();
                if let Some(i) = d.sub_sel
                    && i >= len
                {
                    d.sub_sel = len.checked_sub(1);
                }
            }
            None => self.detail = None,
        }
    }

    pub fn notify(&mut self, text: String, undo: bool) {
        self.toast = Some(Toast {
            text,
            undo,
            until: Instant::now() + Duration::from_millis(if undo { 5000 } else { 2200 }),
        });
    }

    // ------------------------------------------------------------------
    // Mutations
    // ------------------------------------------------------------------

    pub fn update_task(&mut self, id: Id, f: impl FnOnce(&mut Task)) {
        if let Some(t) = self.store.task(id) {
            let mut n = t.clone();
            f(&mut n);
            if n != *t {
                let c = Change::task(Some(t), Some(n));
                self.commit(vec![c]);
            }
        }
    }

    pub fn update_section(&mut self, id: Id, f: impl FnOnce(&mut Section)) {
        if let Some(s) = self.store.section(id) {
            let mut n = s.clone();
            f(&mut n);
            if n != *s {
                let c = Change::Section(Some(s.clone()), Some(n));
                self.commit(vec![c]);
            }
        }
    }

    /// Position for inserting at `index` among the tasks of `section` (excluding
    /// `exclude`), plus any renumbering changes needed to make room.
    fn task_slot(&self, section: Id, index: usize, exclude: Option<Id>) -> (f64, Vec<Change>) {
        let list: Vec<&Task> = self.store.tasks_of(section).filter(|t| Some(t.id) != exclude).collect();
        let index = index.min(list.len());
        let before = index.checked_sub(1).and_then(|i| list.get(i)).map(|t| t.position);
        let after = list.get(index).map(|t| t.position);
        if let Some(p) = position_between(before, after) {
            return (p, vec![]);
        }
        let mut changes = vec![];
        for (i, t) in list.iter().enumerate() {
            let mut n = (*t).clone();
            n.position = (i + if i >= index { 2 } else { 1 }) as f64 * POS_STEP;
            changes.push(Change::task(Some(t), Some(n)));
        }
        ((index + 1) as f64 * POS_STEP, changes)
    }

    fn section_slot(&self, board: Id, index: usize, exclude: Option<Id>) -> (f64, Vec<Change>) {
        let list: Vec<&Section> = self
            .store
            .sections_of(board)
            .filter(|s| Some(s.id) != exclude)
            .collect();
        let index = index.min(list.len());
        let before = index.checked_sub(1).and_then(|i| list.get(i)).map(|s| s.position);
        let after = list.get(index).map(|s| s.position);
        if let Some(p) = position_between(before, after) {
            return (p, vec![]);
        }
        let mut changes = vec![];
        for (i, s) in list.iter().enumerate() {
            let mut n = (*s).clone();
            n.position = (i + if i >= index { 2 } else { 1 }) as f64 * POS_STEP;
            changes.push(Change::Section(Some((*s).clone()), Some(n)));
        }
        ((index + 1) as f64 * POS_STEP, changes)
    }

    /// Index of `before` among the tasks of `section` excluding `exclude` (end if `None`).
    fn task_index(&self, section: Id, before: Option<Id>, exclude: Option<Id>) -> usize {
        let list = self.store.tasks_of(section).filter(|t| Some(t.id) != exclude);
        match before {
            Some(b) => {
                let mut n = 0;
                for t in list {
                    if t.id == b {
                        return n;
                    }
                    n += 1;
                }
                n
            }
            None => list.count(),
        }
    }

    pub fn move_task_to(&mut self, id: Id, section: Id, before: Option<Id>) {
        let Some(t) = self.store.task(id).cloned() else { return };
        if before == Some(id) {
            return;
        }
        let index = self.task_index(section, before, Some(id));
        if t.section_id == section {
            let old = self
                .store
                .tasks_of(section)
                .position(|x| x.id == id)
                .unwrap_or(usize::MAX);
            if old == index {
                return;
            }
        }
        let (pos, mut changes) = self.task_slot(section, index, Some(id));
        let mut n = t.clone();
        n.section_id = section;
        n.position = pos;
        changes.push(Change::task(Some(&t), Some(n)));
        self.commit(changes);
    }

    pub fn move_section_to(&mut self, id: Id, before: Option<Id>) {
        let Some(s) = self.store.section(id).cloned() else {
            return;
        };
        if before == Some(id) {
            return;
        }
        let list: Vec<Id> = self
            .store
            .sections_of(s.board_id)
            .filter(|x| x.id != id)
            .map(|x| x.id)
            .collect();
        let index = before
            .and_then(|b| list.iter().position(|x| *x == b))
            .unwrap_or(list.len());
        let old = self
            .store
            .sections_of(s.board_id)
            .position(|x| x.id == id)
            .unwrap_or(usize::MAX);
        if old == index {
            return;
        }
        let (pos, mut changes) = self.section_slot(s.board_id, index, Some(id));
        let mut n = s.clone();
        n.position = pos;
        changes.push(Change::Section(Some(s), Some(n)));
        self.commit(changes);
    }

    fn new_task(&mut self, section: Id, before: Option<Id>, title: String) -> Id {
        let index = self.task_index(section, before, None);
        let (pos, mut changes) = self.task_slot(section, index, None);
        let id = self.store.alloc_id();
        changes.push(Change::task(
            None,
            Some(Task {
                id,
                section_id: section,
                title,
                description: String::new(),
                priority: Priority::None,
                due: None,
                position: pos,
                created_at: now_ts(),
                completed_at: None,
                progress: None,
                labels: vec![],
                parent_id: None,
                child_pos: 0.0,
            }),
        ));
        self.commit(changes);
        id
    }

    /// Makes `id` a subtask of `parent` (last among its siblings), or a
    /// top-level task with `None`. Refuses to create a cycle.
    pub fn set_parent(&mut self, id: Id, parent: Option<Id>) {
        let Some(t) = self.store.task(id).cloned() else { return };
        if t.parent_id == parent {
            return;
        }
        if let Some(p) = parent
            && (p == id || self.store.task(p).is_none() || self.store.ancestors_of(p).iter().any(|a| a.id == id))
        {
            return;
        }
        let mut n = t.clone();
        n.parent_id = parent;
        n.child_pos = match parent {
            Some(p) => self.store.children_of(p).last().map(|c| c.child_pos).unwrap_or(0.0) + POS_STEP,
            None => 0.0,
        };
        let was_selected = self.sel.task == Some(id);
        self.commit(vec![Change::task(Some(&t), Some(n))]);
        // A card that just disappeared into its parent: keep the parent selected.
        if was_selected && let Some(a) = self.board_anchor(id) {
            self.sel = Sel {
                section: Some(a.section_id),
                task: Some(a.id),
            };
        }
        self.sync_detail();
        let text = match parent.and_then(|p| self.store.task(p)) {
            Some(p) => format!("Now a subtask of “{}”", truncate(&p.title, 30)),
            None => "Now a top-level task".into(),
        };
        self.notify(text, true);
    }

    /// Deletes for `id` and everything below it, children before parents so the
    /// reversed undo re-creates parents first.
    fn delete_changes(&self, id: Id) -> Vec<Change> {
        let mut out: Vec<Change> = self
            .store
            .descendants_of(id)
            .into_iter()
            .rev()
            .map(|t| Change::task(Some(t), None))
            .collect();
        if let Some(t) = self.store.task(id) {
            out.push(Change::task(Some(t), None));
        }
        out
    }

    /// Deletes for a set of tasks and all their subtasks, each task once.
    fn delete_many(&self, ids: impl IntoIterator<Item = Id>) -> Vec<Change> {
        let mut seen = std::collections::HashSet::new();
        let mut out = vec![];
        for id in ids {
            for c in self.delete_changes(id) {
                if let Change::Task(Some(t), None) = &c
                    && seen.insert(t.id)
                {
                    out.push(c);
                }
            }
        }
        out
    }

    fn new_section(&mut self, after: Option<Id>, name: String) -> Option<Id> {
        let board = self.board?;
        let list: Vec<Id> = self.store.sections_of(board).map(|s| s.id).collect();
        let index = after
            .and_then(|a| list.iter().position(|x| *x == a))
            .map(|i| i + 1)
            .unwrap_or(list.len());
        let (pos, mut changes) = self.section_slot(board, index, None);
        let id = self.store.alloc_id();
        changes.push(Change::Section(
            None,
            Some(Section {
                id,
                board_id: board,
                name,
                position: pos,
                collapsed: false,
            }),
        ));
        self.commit(changes);
        Some(id)
    }

    pub fn new_board(&mut self, name: String) -> Id {
        let pos = self
            .store
            .boards
            .last()
            .map(|b| b.position + POS_STEP)
            .unwrap_or(POS_STEP);
        let id = self.store.alloc_id();
        let mut changes = vec![Change::Board(
            None,
            Some(Board {
                id,
                name,
                position: pos,
            }),
        )];
        for (i, name) in ["To do", "In progress", "Done"].iter().enumerate() {
            changes.push(Change::Section(
                None,
                Some(Section {
                    id: self.store.alloc_id(),
                    board_id: id,
                    name: name.to_string(),
                    position: (i as f64 + 1.0) * POS_STEP,
                    collapsed: false,
                }),
            ));
        }
        self.commit(changes);
        id
    }

    fn delete_task(&mut self, id: Id) {
        let Some(t) = self.store.task(id).cloned() else { return };
        // Pick a neighbour to select afterwards.
        if self.sel.task == Some(id) {
            let grid = self.grid();
            if let Some((_, ts)) = grid.iter().find(|(s, _)| *s == t.section_id) {
                let i = ts.iter().position(|x| *x == id).unwrap_or(0);
                self.sel.task = ts.get(i + 1).or(i.checked_sub(1).and_then(|j| ts.get(j))).copied();
            }
        }
        // Close the panel if it shows this task or one of its subtasks.
        if let Some(d) = &self.detail
            && (d.task == id || self.store.ancestors_of(d.task).iter().any(|a| a.id == id))
        {
            self.detail = None;
        }
        let changes = self.delete_changes(id);
        let subs = changes.len() - 1;
        self.commit(changes);
        self.notify(
            format!(
                "Deleted “{}”{}",
                truncate(&t.title, 40),
                match subs {
                    0 => String::new(),
                    1 => " and 1 subtask".into(),
                    n => format!(" and {n} subtasks"),
                }
            ),
            true,
        );
    }

    fn delete_section(&mut self, id: Id) {
        let Some(s) = self.store.section(id).cloned() else {
            return;
        };
        let mut changes = self.delete_many(self.store.tasks_of(id).map(|t| t.id).collect::<Vec<_>>());
        let n = changes.len();
        changes.push(Change::Section(Some(s.clone()), None));
        let secs: Vec<Id> = self.board_sections().iter().map(|x| x.id).collect();
        let i = secs.iter().position(|x| *x == id).unwrap_or(0);
        let next = secs.get(i + 1).or(i.checked_sub(1).and_then(|j| secs.get(j))).copied();
        if let Some(d) = &self.detail
            && changes
                .iter()
                .any(|c| matches!(c, Change::Task(Some(t), None) if t.id == d.task))
        {
            self.detail = None;
        }
        self.commit(changes);
        self.sel = Sel {
            section: next,
            task: None,
        };
        self.notify(
            format!(
                "Deleted section “{}”{}",
                truncate(&s.name, 30),
                if n > 0 {
                    format!(" and {n} task{}", if n == 1 { "" } else { "s" })
                } else {
                    String::new()
                }
            ),
            true,
        );
    }

    fn delete_board(&mut self, id: Id) {
        let Some(b) = self.store.board(id).cloned() else { return };
        let secs: Vec<Section> = self.store.sections_of(id).cloned().collect();
        let tasks: Vec<Id> = secs
            .iter()
            .flat_map(|s| self.store.tasks_of(s.id).map(|t| t.id))
            .collect();
        let mut changes = self.delete_many(tasks);
        for l in self.store.labels_of(id) {
            changes.push(Change::Label(Some(l.clone()), None));
        }
        for s in secs {
            changes.push(Change::Section(Some(s), None));
        }
        changes.push(Change::Board(Some(b.clone()), None));
        self.detail = None;
        self.commit(changes);
        self.board = self.store.boards.first().map(|b| b.id);
        self.sel = Sel::default();
        self.ensure_sel();
        self.notify(format!("Deleted board “{}”", truncate(&b.name, 30)), true);
    }

    fn duplicate_task(&mut self, id: Id) {
        let Some(t) = self.store.task(id).cloned() else { return };
        let grid_next = {
            let list: Vec<Id> = self.store.tasks_of(t.section_id).map(|x| x.id).collect();
            let i = list.iter().position(|x| *x == id).unwrap_or(0);
            list.get(i + 1).copied()
        };
        let index = self.task_index(t.section_id, grid_next, None);
        let (pos, mut changes) = self.task_slot(t.section_id, index, None);
        let mut n = t.clone();
        n.id = self.store.alloc_id();
        n.position = pos;
        n.created_at = now_ts();
        if let Some(p) = t.parent_id {
            // Lands right after the original among its siblings.
            let sibs = self.store.children_of(p);
            let i = sibs.iter().position(|x| x.id == id).unwrap_or(0);
            n.child_pos = match sibs.get(i + 1) {
                Some(next) => (t.child_pos + next.child_pos) / 2.0,
                None => t.child_pos + POS_STEP,
            };
        }
        let new_id = n.id;
        // Copy the whole subtree; descendants come parents-first.
        let subtree: Vec<Task> = self.store.descendants_of(id).into_iter().cloned().collect();
        let mut ids = std::collections::HashMap::from([(id, new_id)]);
        changes.push(Change::task(None, Some(n)));
        for mut c in subtree {
            let nid = self.store.alloc_id();
            ids.insert(c.id, nid);
            c.id = nid;
            c.parent_id = c.parent_id.and_then(|p| ids.get(&p).copied());
            c.created_at = now_ts();
            changes.push(Change::task(None, Some(c)));
        }
        self.commit(changes);
        if self.store.task(new_id).is_some_and(|t| self.on_board(t)) {
            self.sel.task = Some(new_id);
        } else if let Some(d) = &mut self.detail
            && t.parent_id == Some(d.task)
        {
            d.sub_sel = self.store.children_of(d.task).iter().position(|x| x.id == new_id);
        }
        self.notify("Task duplicated".into(), true);
    }

    pub fn toggle_done(&mut self, id: Id) {
        let Some(t) = self.store.task(id).cloned() else { return };
        if t.completed_at.is_some() {
            self.update_task(id, |t| t.completed_at = None);
            return;
        }
        let board = self.store.board_of_task(&t).filter(|_| self.settings.move_done);
        let done_section = board.and_then(|b| {
            let secs: Vec<&Section> = self.store.sections_of(b).collect();
            secs.iter()
                .rev()
                .find(|s| {
                    let n = s.name.to_lowercase();
                    n.contains("done") || n.contains("complete") || n.contains("finished")
                })
                .or(secs.last())
                .map(|s| s.id)
        });
        let mut n = t.clone();
        n.completed_at = Some(now_ts());
        let mut changes = vec![];
        let mut moved_to = None;
        if let Some(ds) = done_section
            && ds != t.section_id
        {
            // Select a neighbour in the original section.
            if self.sel.task == Some(id) && !self.panel_focused() {
                let grid = self.grid();
                if let Some((_, ts)) = grid.iter().find(|(s, _)| *s == t.section_id) {
                    let i = ts.iter().position(|x| *x == id).unwrap_or(0);
                    self.sel.task = ts.get(i + 1).or(i.checked_sub(1).and_then(|j| ts.get(j))).copied();
                }
            }
            let (pos, ren) = self.task_slot(ds, 0, Some(id));
            changes = ren;
            n.section_id = ds;
            n.position = pos;
            moved_to = self.store.section(ds).map(|s| s.name.clone());
        }
        changes.push(Change::task(Some(&t), Some(n)));
        self.commit(changes);
        if let Some(name) = moved_to {
            self.notify(format!("Completed · moved to {name}"), true);
            if self.detail.as_ref().is_some_and(|d| d.task == id) && self.on_board(&t) {
                self.sel = Sel {
                    section: done_section,
                    task: Some(id),
                };
            }
        }
    }

    /// Sets manual progress (`None` = follow the subtasks). Reaching 100% can
    /// complete the task, as one undo step, when the setting is on.
    pub fn set_progress(&mut self, id: Id, p: Option<u8>) {
        let p = p.map(|v| v.min(100));
        let before = self.undo.len();
        self.update_task(id, |t| t.progress = p);
        let completes = p == Some(100)
            && self.settings.complete_at_100
            && self.store.task(id).is_some_and(|t| t.completed_at.is_none());
        if completes {
            let after_set = self.undo.len();
            self.toggle_done(id);
            if after_set > before && self.undo.len() > after_set {
                let done = self.undo.pop().unwrap_or_default();
                if let Some(set) = self.undo.last_mut() {
                    set.extend(done);
                }
            }
        }
    }

    /// Moves progress to the next 10% step up or down.
    pub fn bump_progress(&mut self, id: Id, up: bool) {
        let Some(t) = self.store.task(id) else { return };
        let cur = self.store.percent(t).unwrap_or(0) as i32;
        let next = if up {
            (cur / 10 + 1) * 10
        } else {
            ((cur + 9) / 10 - 1) * 10
        };
        self.set_progress(id, Some(next.clamp(0, 100) as u8));
    }

    pub fn set_priority(&mut self, id: Id, p: Priority) {
        self.update_task(id, |t| t.priority = p);
    }

    pub fn set_due(&mut self, id: Id, due: Option<NaiveDate>) {
        self.update_task(id, |t| t.due = due);
    }

    pub fn toggle_label(&mut self, task: Id, label: Id) {
        self.update_task(task, |t| {
            if let Some(i) = t.labels.iter().position(|l| *l == label) {
                t.labels.remove(i);
            } else {
                t.labels.push(label);
            }
        });
    }

    pub fn create_label(&mut self, task: Option<Id>, name: String) -> Option<Id> {
        let board = self.board?;
        let color = (self.store.labels_of(board).count() % theme::LABEL_COLORS.len()) as u8;
        let id = self.store.alloc_id();
        let label = Label {
            id,
            board_id: board,
            name,
            color,
        };
        let mut changes = vec![Change::Label(None, Some(label))];
        if let Some(t) = task.and_then(|t| self.store.task(t)) {
            let mut n = t.clone();
            n.labels.push(id);
            changes.push(Change::task(Some(t), Some(n)));
        }
        self.commit(changes);
        Some(id)
    }

    pub fn update_label(&mut self, id: Id, f: impl FnOnce(&mut Label)) {
        if let Some(l) = self.store.label(id) {
            let mut n = l.clone();
            f(&mut n);
            if n != *l {
                let c = Change::Label(Some(l.clone()), Some(n));
                self.commit(vec![c]);
            }
        }
    }

    pub fn delete_label(&mut self, id: Id) {
        let Some(l) = self.store.label(id).cloned() else { return };
        let mut changes: Vec<Change> = self
            .store
            .tasks
            .iter()
            .filter(|t| t.labels.contains(&id))
            .map(|t| {
                let mut n = t.clone();
                n.labels.retain(|x| *x != id);
                Change::task(Some(t), Some(n))
            })
            .collect();
        changes.push(Change::Label(Some(l.clone()), None));
        self.commit(changes);
        self.notify(format!("Deleted label “{}”", l.name), true);
    }

    // ------------------------------------------------------------------
    // Selection & navigation
    // ------------------------------------------------------------------

    fn nav_vertical(&mut self, delta: i32) {
        let grid = self.grid();
        let Some(sec) = self.sel.section else { return };
        let Some((_, ts)) = grid.iter().find(|(s, _)| *s == sec) else {
            return;
        };
        // Row 0 = header, row i+1 = task i
        let row = match self.sel.task {
            Some(t) => ts.iter().position(|x| *x == t).map(|i| i + 1).unwrap_or(0),
            None => 0,
        } as i32;
        let new = (row + delta).clamp(0, ts.len() as i32);
        self.sel.task = if new == 0 { None } else { Some(ts[new as usize - 1]) };
    }

    fn nav_horizontal(&mut self, delta: i32) {
        let grid = self.grid();
        let Some(i) = self.sel.section.and_then(|s| grid.iter().position(|(g, _)| *g == s)) else {
            return;
        };
        let row = match self.sel.task {
            Some(t) => grid[i].1.iter().position(|x| *x == t).map(|r| r + 1).unwrap_or(0),
            None => 0,
        };
        let j = (i as i32 + delta).clamp(0, grid.len() as i32 - 1) as usize;
        if j == i {
            return;
        }
        let ts = &grid[j].1;
        self.sel.section = Some(grid[j].0);
        self.sel.task = if row == 0 || ts.is_empty() {
            None
        } else {
            Some(ts[(row - 1).min(ts.len() - 1)])
        };
    }

    fn nav_edge(&mut self, first: bool) {
        let grid = self.grid();
        if let Some((_, ts)) = self.sel.section.and_then(|s| grid.iter().find(|(g, _)| *g == s)) {
            self.sel.task = if first { ts.first().copied() } else { ts.last().copied() };
        }
    }

    fn move_selected_vertical(&mut self, delta: i32) {
        let Some(id) = self.target_task() else { return };
        let Some(t) = self.store.task(id) else { return };
        let section = t.section_id;
        let grid = self.grid();
        let Some((_, ts)) = grid.iter().find(|(s, _)| *s == section) else {
            return;
        };
        let Some(i) = ts.iter().position(|x| *x == id) else {
            return;
        };
        if delta < 0 {
            if delta == i32::MIN {
                if let Some(first) = ts.first() {
                    self.move_task_to(id, section, Some(*first));
                }
            } else if i > 0 {
                self.move_task_to(id, section, Some(ts[i - 1]));
            }
        } else if delta == i32::MAX {
            self.move_task_to(id, section, None);
        } else if i + 1 < ts.len() {
            let before = ts.get(i + 2).copied();
            // Place after the next visible task.
            let before = match before {
                Some(b) => Some(b),
                None => {
                    let all: Vec<Id> = self.store.tasks_of(section).map(|t| t.id).collect();
                    let k = all.iter().position(|x| *x == ts[i + 1]).unwrap_or(all.len());
                    all.get(k + 1).copied()
                }
            };
            self.move_task_to(id, section, before);
        }
    }

    fn move_selected_horizontal(&mut self, delta: i32) {
        let grid = self.grid();
        let Some(i) = self.sel.section.and_then(|s| grid.iter().position(|(g, _)| *g == s)) else {
            return;
        };
        let j = i as i32 + delta;
        if j < 0 || j >= grid.len() as i32 {
            return;
        }
        let j = j as usize;
        match self.target_task() {
            Some(id) => {
                // Only cards on the board move between sections.
                let Some(row) = grid[i].1.iter().position(|x| *x == id) else {
                    return;
                };
                let target = &grid[j];
                let before = target.1.get(row).copied();
                self.move_task_to(id, target.0, before);
                self.sel = Sel {
                    section: Some(target.0),
                    task: Some(id),
                };
            }
            None => {
                // Move the section itself.
                let id = grid[i].0;
                let before = if delta < 0 {
                    Some(grid[j].0)
                } else {
                    grid.get(j + 1).map(|g| g.0)
                };
                self.move_section_to(id, before);
            }
        }
    }

    pub fn switch_board_raw(&mut self, id: Id) {
        if self.board == Some(id) {
            return;
        }
        if let Some(b) = self.board {
            self.board_sel.insert(b, self.sel);
        }
        self.board = Some(id);
        self.sel = self.board_sel.get(&id).copied().unwrap_or_default();
        self.board_offset = 0.0;
        self.db.set_setting("board", &id.to_string());
    }

    pub fn switch_board(&mut self, id: Id) -> Act {
        self.flush_detail();
        self.detail = None;
        self.edit = None;
        self.switch_board_raw(id);
        self.ensure_sel();
        Act::batch([
            operation::scroll_to(BOARD_SCROLL, AbsoluteOffset { x: Some(0.0), y: None }),
            self.reveal(),
        ])
    }

    fn cycle_board(&mut self, delta: i32) -> Act {
        let n = self.store.boards.len() as i32;
        if n == 0 {
            return Act::none();
        }
        let i = self
            .board
            .and_then(|b| self.store.boards.iter().position(|x| x.id == b))
            .unwrap_or(0) as i32;
        let j = (i + delta).rem_euclid(n) as usize;
        let id = self.store.boards[j].id;
        self.switch_board(id)
    }

    pub fn col_width(s: &Section) -> f32 {
        if s.collapsed { COL_COLLAPSED_W } else { COL_W }
    }

    /// Scroll the board and the selected section so the selection is visible.
    pub fn reveal(&mut self) -> Act {
        let mut acts = vec![];
        let secs = self.board_sections();
        if let Some(i) = self.sel.section.and_then(|s| secs.iter().position(|x| x.id == s)) {
            let x: f32 = BOARD_PAD + secs[..i].iter().map(|s| Self::col_width(s) + COL_GAP).sum::<f32>();
            let w = Self::col_width(secs[i]);
            let view_w = self.board_view_w.unwrap_or_else(|| {
                self.window.width
                    - if self.sidebar { SIDEBAR_W } else { 0.0 }
                    - if self.detail.is_some() { DETAIL_W } else { 0.0 }
            });
            let off = self.board_scroll();
            let new = if x - BOARD_PAD < off {
                Some(x - BOARD_PAD)
            } else if x + w + BOARD_PAD > off + view_w {
                Some(x + w + BOARD_PAD - view_w)
            } else {
                None
            };
            if let Some(n) = new {
                let n = n.max(0.0);
                self.board_offset = n;
                acts.push(operation::scroll_to(
                    BOARD_SCROLL,
                    AbsoluteOffset { x: Some(n), y: None },
                ));
            }
        }
        if let Some(sec) = self.sel.section {
            let id = sec_scroll_id(sec);
            match self.sel.task {
                None => {
                    if self.sec_offset.get(&sec).copied().unwrap_or(0.0) > 0.0 {
                        self.sec_offset.insert(sec, 0.0);
                        acts.push(operation::scroll_to(id, AbsoluteOffset { x: None, y: Some(0.0) }));
                    }
                }
                Some(t) => {
                    if let (Some(r), Some(v)) = (self.card_bounds.get(&t), self.sec_view.get(&sec)) {
                        let y = r.y - v.y;
                        let off = self.sec_scroll(sec);
                        let new = if y < off + 4.0 {
                            Some(y - 8.0)
                        } else if y + r.height > off + v.height - 4.0 {
                            Some(y + r.height - v.height + 8.0)
                        } else {
                            None
                        };
                        if let Some(n) = new {
                            let n = n.max(0.0);
                            self.sec_offset.insert(sec, n);
                            acts.push(operation::scroll_to(id, AbsoluteOffset { x: None, y: Some(n) }));
                        }
                    }
                }
            }
        }
        Act::batch(acts)
    }

    // ------------------------------------------------------------------
    // Drag and drop
    // ------------------------------------------------------------------

    /// The board's real horizontal scroll offset. The scrollable clamps its stored
    /// offset to the scrollable range and stops reporting scroll events once the
    /// content fits, so `board_offset` alone can be stale; clamp it the same way.
    fn board_scroll(&self) -> f32 {
        self.board_offset
            .clamp(0.0, (self.board_content_w - self.board_view).max(0.0))
    }

    /// A section list's real vertical scroll offset (see `board_scroll`).
    fn sec_scroll(&self, id: Id) -> f32 {
        let off = self.sec_offset.get(&id).copied().unwrap_or(0.0);
        let content = self.sec_content_h.get(&id).copied().unwrap_or(0.0);
        let view = self.sec_view.get(&id).map_or(0.0, |v| v.height);
        off.clamp(0.0, (content - view).max(0.0))
    }

    /// Works out where a drop at window position `p` would land, using the
    /// layout bounds the widgets reported.
    fn drop_target(&self, item: DragItem, p: Point) -> Option<DropTarget> {
        let secs = self.board_sections();
        for (i, s) in secs.iter().enumerate() {
            let Some(cb) = self.col_bounds.get(&s.id) else { continue };
            let x0 = cb.x - self.board_scroll();
            if p.x < x0 - COL_GAP / 2.0 || p.x > x0 + cb.width + COL_GAP / 2.0 {
                continue;
            }
            return match item {
                DragItem::Section(_) => {
                    let before = if p.x < x0 + cb.width / 2.0 {
                        Some(s.id)
                    } else {
                        secs.get(i + 1).map(|x| x.id)
                    };
                    Some(DropTarget::Section { before })
                }
                DragItem::Task(_) => {
                    if s.collapsed {
                        return Some(DropTarget::Task {
                            section: s.id,
                            before: None,
                        });
                    }
                    let Some(v) = self.sec_view.get(&s.id) else {
                        return Some(DropTarget::Task {
                            section: s.id,
                            before: None,
                        });
                    };
                    let off = self.sec_scroll(s.id);
                    let cy = p.y - v.y + off;
                    for t in self.visible_tasks(s) {
                        if let Some(r) = self.card_bounds.get(&t.id)
                            && cy < r.y - v.y + r.height / 2.0
                        {
                            return Some(DropTarget::Task {
                                section: s.id,
                                before: Some(t.id),
                            });
                        }
                    }
                    Some(DropTarget::Task {
                        section: s.id,
                        before: None,
                    })
                }
            };
        }
        None
    }

    /// Window-space rectangle for the drop indicator line.
    pub fn drop_indicator(&self) -> Option<Rectangle> {
        let d = self.drag.as_ref().filter(|d| d.active)?;
        match d.target? {
            DropTarget::Task { section, before } => {
                let s = self.store.section(section)?;
                if s.collapsed {
                    return None;
                }
                let v = self.sec_view.get(&section)?;
                let off = self.sec_scroll(section);
                let tasks = self.visible_tasks(s);
                let (x, w, y) = match before {
                    Some(b) => {
                        let r = self.card_bounds.get(&b)?;
                        (r.x, r.width, r.y - off - 5.5)
                    }
                    None => match tasks.last().and_then(|t| self.card_bounds.get(&t.id)) {
                        Some(r) => (r.x, r.width, r.y + r.height - off + 2.5),
                        None => (v.x + 4.0, v.width - 8.0, v.y + 2.0),
                    },
                };
                let y = y.clamp(v.y - 4.0, v.y + v.height);
                Some(Rectangle::new(
                    Point::new(x - self.board_scroll(), y),
                    Size::new(w, 3.0),
                ))
            }
            DropTarget::Section { before } => {
                let secs = self.board_sections();
                let (x, cb) = match before {
                    Some(b) => {
                        let cb = self.col_bounds.get(&b)?;
                        (cb.x - COL_GAP / 2.0 - 1.5, cb)
                    }
                    None => {
                        let cb = self.col_bounds.get(&secs.last()?.id)?;
                        (cb.x + cb.width + COL_GAP / 2.0 - 1.5, cb)
                    }
                };
                Some(Rectangle::new(
                    Point::new(x - self.board_scroll(), cb.y),
                    Size::new(3.0, cb.height),
                ))
            }
        }
    }

    pub fn drop_section(&self) -> Option<Id> {
        match self.drag.as_ref().filter(|d| d.active)?.target? {
            DropTarget::Task { section, .. } => Some(section),
            DropTarget::Section { .. } => None,
        }
    }

    // ------------------------------------------------------------------
    // Inline editing
    // ------------------------------------------------------------------

    pub fn start_edit(&mut self, edit: Edit) -> Act {
        let pre = self.finish_edit(true);
        self.edit = Some(edit);
        let focus = self.focus_field(Field::Inline);
        Act::batch([pre, focus])
    }

    /// Ends the current inline edit. Returns a follow-up action (e.g. focus the
    /// next "new task" input for rapid entry).
    pub fn finish_edit(&mut self, commit: bool) -> Act {
        let Some(edit) = self.edit.take() else {
            return Act::none();
        };
        if !commit {
            return Act::none();
        }
        let value = edit.value().trim().to_string();
        match edit {
            Edit::NewTask { section, before, .. } => {
                if value.is_empty() {
                    return Act::none();
                }
                let id = self.new_task(section, before, value);
                self.sel = Sel {
                    section: Some(section),
                    task: Some(id),
                };
            }
            Edit::Task { id, .. } => {
                if !value.is_empty() {
                    self.update_task(id, |t| t.title = value);
                }
            }
            Edit::Section { id, .. } => {
                if !value.is_empty() {
                    self.update_section(id, |s| s.name = value);
                }
            }
            Edit::NewSection { after, .. } => {
                if !value.is_empty()
                    && let Some(id) = self.new_section(after, value)
                {
                    self.sel = Sel {
                        section: Some(id),
                        task: None,
                    };
                }
            }
            Edit::Board { id, .. } => {
                if !value.is_empty()
                    && let Some(b) = self.store.board(id)
                {
                    let mut n = b.clone();
                    n.name = value;
                    let c = Change::Board(Some(b.clone()), Some(n));
                    self.commit(vec![c]);
                }
            }
            Edit::NewBoard { .. } => {
                if !value.is_empty() {
                    let id = self.new_board(value);
                    return self.switch_board(id);
                }
            }
        }
        Act::none()
    }

    // ------------------------------------------------------------------
    // Detail panel
    // ------------------------------------------------------------------

    pub fn open_detail(&mut self, id: Id) -> Act {
        self.flush_detail();
        let pre = self.finish_edit(true);
        let Some(t) = self.store.task(id) else { return pre };
        if let Some(a) = self.board_anchor(id) {
            self.sel = Sel {
                section: Some(a.section_id),
                task: Some(a.id),
            };
        }
        self.detail = Some(Detail {
            task: id,
            title: t.title.clone(),
            desc: text_editor::Content::with_text(&t.description),
            sub_input: String::new(),
            sub_sel: None,
            prop_sel: None,
            sub_edit: None,
            focused: true,
        });
        let reveal = self.reveal();
        let blur = self.blur();
        Act::batch([pre, reveal, blur])
    }

    pub fn flush_detail(&mut self) {
        let Some(d) = &self.detail else { return };
        let id = d.task;
        let title = d.title.trim().to_string();
        let desc = d.desc.text().trim_end().to_string();
        let Some(t) = self.store.task(id) else { return };
        if (!title.is_empty() && title != t.title) || desc != t.description {
            self.update_task(id, |t| {
                if !title.is_empty() {
                    t.title = title;
                }
                t.description = desc;
            });
        }
    }

    fn close_detail(&mut self) -> Act {
        self.flush_detail();
        self.detail = None;
        self.board_view_w = None;
        let blur = if self.focus.is_some_and(Field::in_panel) {
            self.blur()
        } else {
            Act::none()
        };
        Act::batch([blur, self.reveal()])
    }

    fn add_subtask(&mut self) {
        let Some(d) = &mut self.detail else { return };
        let title = d.sub_input.trim().to_string();
        if title.is_empty() {
            return;
        }
        d.sub_input.clear();
        let parent = d.task;
        let Some(p) = self.store.task(parent).cloned() else {
            return;
        };
        let child_pos = self
            .store
            .children_of(parent)
            .last()
            .map(|c| c.child_pos)
            .unwrap_or(0.0)
            + POS_STEP;
        let (pos, mut changes) = self.task_slot(p.section_id, usize::MAX, None);
        changes.push(Change::task(
            None,
            Some(Task {
                id: self.store.alloc_id(),
                section_id: p.section_id,
                title,
                description: String::new(),
                priority: Priority::None,
                due: None,
                position: pos,
                created_at: now_ts(),
                completed_at: None,
                progress: None,
                labels: vec![],
                parent_id: Some(parent),
                child_pos,
            }),
        ));
        self.commit(changes);
    }

    /// Id of the `i`-th subtask of the open task.
    fn subtask_at(&self, i: usize) -> Option<Id> {
        let d = self.detail.as_ref()?;
        self.store.children_of(d.task).get(i).map(|t| t.id)
    }

    fn delete_subtask(&mut self, sub: Id) {
        let Some(t) = self.store.task(sub).cloned() else { return };
        let changes = self.delete_changes(sub);
        let n = changes.len() - 1;
        self.commit(changes);
        self.sync_detail();
        let more = match n {
            0 => String::new(),
            1 => " and 1 subtask".into(),
            n => format!(" and {n} subtasks"),
        };
        self.notify(format!("Deleted “{}”{more}", truncate(&t.title, 30)), true);
    }

    /// Moves the selected subtask by `delta` places (clamped, so `i32::MIN` /
    /// `i32::MAX` move it to the top / bottom).
    fn move_subtask(&mut self, delta: i32) {
        let Some(d) = &self.detail else { return };
        let (task, Some(i)) = (d.task, d.sub_sel) else { return };
        let kids: Vec<Task> = self.store.children_of(task).into_iter().cloned().collect();
        if i >= kids.len() {
            return;
        }
        let j = (i as i64 + delta as i64).clamp(0, kids.len() as i64 - 1) as usize;
        if i == j {
            return;
        }
        let mut order: Vec<&Task> = kids.iter().collect();
        let moved = order.remove(i);
        order.insert(j, moved);
        let changes = order
            .iter()
            .enumerate()
            .filter_map(|(k, t)| {
                let pos = (k + 1) as f64 * POS_STEP;
                (t.child_pos != pos).then(|| {
                    let mut n = (*t).clone();
                    n.child_pos = pos;
                    Change::task(Some(t), Some(n))
                })
            })
            .collect();
        self.commit(changes);
        if let Some(d) = &mut self.detail {
            d.sub_sel = Some(j);
        }
    }

    /// Esc on a subtask goes back up to its parent.
    fn detail_back(&mut self) -> Act {
        let parent = self
            .detail
            .as_ref()
            .and_then(|d| self.store.task(d.task))
            .and_then(|t| t.parent_id);
        match parent {
            Some(p) => {
                let child = self.detail.as_ref().map(|d| d.task);
                let a = self.open_detail(p);
                let i = child.and_then(|c| self.store.children_of(p).iter().position(|t| t.id == c));
                if let Some(d) = &mut self.detail {
                    d.sub_sel = i;
                }
                a
            }
            None => self.close_detail(),
        }
    }

    // ------------------------------------------------------------------
    // Palette
    // ------------------------------------------------------------------

    pub fn open_palette(&mut self, mode: palette::Mode) -> Act {
        self.flush_detail();
        let pre = self.finish_edit(true);
        self.chord = None;
        let query = match &mode {
            palette::Mode::RenameLabel(id) => self.store.label(*id).map(|l| l.name.clone()).unwrap_or_default(),
            _ => String::new(),
        };
        let cal = match &mode {
            palette::Mode::Due(t) => self.store.task(*t).and_then(|t| t.due).unwrap_or(self.today),
            _ => self.today,
        };
        let mut p = Palette::new(mode, query, cal);
        p.items = palette::build(self, &p.mode, &p.query);
        p.sel = palette::default_selection(self, &p);
        self.palette = Some(p);
        self.help = false;
        self.settings_open = false;
        let scroll = self.palette_scroll();
        let focus = self.focus_field(Field::Palette);
        Act::batch([pre, focus, scroll])
    }

    pub fn refresh_palette(&mut self) {
        if let Some(mut p) = self.palette.take() {
            p.items = palette::build(self, &p.mode, &p.query);
            p.sel = p.sel.min(p.items.len().saturating_sub(1));
            self.palette = Some(p);
        }
    }

    fn palette_activate(&mut self, index: usize) -> Act {
        let Some(p) = &self.palette else { return Act::none() };
        let mode = p.mode.clone();
        let query = p.query.clone();
        let item = p.items.get(index).cloned();
        palette::activate(self, mode, query, item)
    }

    pub fn close_palette(&mut self) -> Act {
        self.palette = None;
        self.blur()
    }

    // ------------------------------------------------------------------
    // Commands
    // ------------------------------------------------------------------

    pub fn run(&mut self, cmd: Cmd) -> Act {
        self.flush_detail();
        self.chord = None;
        if !matches!(cmd, Cmd::Palette) {
            self.recent.retain(|c| *c != cmd);
            self.recent.insert(0, cmd);
            self.recent.truncate(8);
        }
        let target = self.target_task();
        let act = match cmd {
            Cmd::NewTask | Cmd::NewTaskAbove => {
                let grid = self.grid();
                let section = self.sel.section.or_else(|| grid.first().map(|g| g.0));
                let Some(section) = section else {
                    return self.start_edit(Edit::NewSection {
                        after: None,
                        value: String::new(),
                    });
                };
                if self.store.section(section).is_some_and(|s| s.collapsed) {
                    self.update_section(section, |s| s.collapsed = false);
                }
                let ts = self
                    .grid()
                    .into_iter()
                    .find(|g| g.0 == section)
                    .map(|g| g.1)
                    .unwrap_or_default();
                let on_board = !self.panel_focused();
                let before = match self.sel.task {
                    Some(t) if on_board => {
                        let i = ts.iter().position(|x| *x == t).unwrap_or(0);
                        if cmd == Cmd::NewTaskAbove {
                            Some(t)
                        } else {
                            ts.get(i + 1).copied()
                        }
                    }
                    _ => {
                        if cmd == Cmd::NewTaskAbove {
                            ts.first().copied()
                        } else {
                            None
                        }
                    }
                };
                // From a selected header, `n` adds to the top or bottom (a setting).
                let before = if self.sel.task.is_none() && cmd == Cmd::NewTask && on_board {
                    match self.settings.new_task_pos {
                        prefs::NewTaskPos::Top => ts.first().copied(),
                        prefs::NewTaskPos::Bottom => None,
                    }
                } else {
                    before
                };
                self.detail = None;
                self.start_edit(Edit::NewTask {
                    section,
                    before,
                    value: String::new(),
                })
            }
            Cmd::NewSection => {
                let after = self.sel.section;
                self.start_edit(Edit::NewSection {
                    after,
                    value: String::new(),
                })
            }
            Cmd::NewBoard => {
                self.sidebar = true;
                self.start_edit(Edit::NewBoard { value: String::new() })
            }
            Cmd::Open => match self.sel.task {
                Some(t) => {
                    // The panel already shows this card: just move into it.
                    let showing = self
                        .detail
                        .as_ref()
                        .and_then(|d| self.board_anchor(d.task))
                        .map(|a| a.id)
                        == Some(t);
                    match &mut self.detail {
                        Some(d) if showing => {
                            d.focused = true;
                            Act::none()
                        }
                        _ => self.open_detail(t),
                    }
                }
                None => match self.sel.section.and_then(|s| self.store.section(s)) {
                    Some(s) if s.collapsed => {
                        let id = s.id;
                        self.update_section(id, |s| s.collapsed = false);
                        Act::none()
                    }
                    Some(s) => {
                        let e = Edit::Section {
                            id: s.id,
                            value: s.name.clone(),
                        };
                        self.start_edit(e)
                    }
                    None => Act::none(),
                },
            },
            Cmd::Rename => {
                if self.panel_focused() {
                    self.rename_in_panel()
                } else if let Some(t) = self.selected_task() {
                    let e = Edit::Task {
                        id: t.id,
                        value: t.title.clone(),
                    };
                    self.start_edit(e)
                } else if let Some(s) = self.sel.section.and_then(|s| self.store.section(s)) {
                    let e = Edit::Section {
                        id: s.id,
                        value: s.name.clone(),
                    };
                    self.start_edit(e)
                } else {
                    Act::none()
                }
            }
            Cmd::ToggleDone => {
                if let Some(t) = target {
                    self.toggle_done(t);
                }
                Act::none()
            }
            Cmd::Delete => {
                let open = self.detail.as_ref().map(|d| d.task);
                if let Some(t) = target {
                    if self.panel_focused() && Some(t) != open {
                        self.delete_subtask(t);
                    } else {
                        self.delete_task(t);
                    }
                } else if let Some(s) = self.sel.section {
                    self.delete_section(s);
                }
                Act::none()
            }
            Cmd::Duplicate => {
                if let Some(t) = target {
                    self.duplicate_task(t);
                }
                Act::none()
            }
            Cmd::SetPriority => match target {
                Some(t) => self.open_palette(palette::Mode::Priority(t)),
                None => Act::none(),
            },
            Cmd::EditLabels => match target {
                Some(t) => self.open_palette(palette::Mode::Labels(t)),
                None => Act::none(),
            },
            Cmd::SetDue => match target {
                Some(t) => self.open_palette(palette::Mode::Due(t)),
                None => Act::none(),
            },
            Cmd::ClearDue => {
                if let Some(t) = target {
                    self.set_due(t, None);
                }
                Act::none()
            }
            Cmd::SetProgress => match target {
                Some(t) => self.open_palette(palette::Mode::Progress(t)),
                None => Act::none(),
            },
            Cmd::ProgressUp | Cmd::ProgressDown => {
                if let Some(t) = target {
                    self.bump_progress(t, cmd == Cmd::ProgressUp);
                }
                Act::none()
            }
            Cmd::ClearProgress => {
                if let Some(t) = target {
                    self.set_progress(t, None);
                }
                Act::none()
            }
            Cmd::MoveTo => match target {
                Some(t) => self.open_palette(palette::Mode::MoveTo(t)),
                None => Act::none(),
            },
            Cmd::SetParent => match target {
                Some(t) => self.open_palette(palette::Mode::Parent(t)),
                None => Act::none(),
            },
            Cmd::MoveLeft => {
                self.move_selected_horizontal(-1);
                Act::none()
            }
            Cmd::MoveRight => {
                self.move_selected_horizontal(1);
                Act::none()
            }
            Cmd::MoveUp | Cmd::MoveDown | Cmd::MoveTop | Cmd::MoveBottom => {
                let delta = match cmd {
                    Cmd::MoveUp => -1,
                    Cmd::MoveDown => 1,
                    Cmd::MoveTop => i32::MIN,
                    _ => i32::MAX,
                };
                if self.subtask_selected() {
                    self.move_subtask(delta);
                } else if self.target_task().is_some() {
                    self.move_selected_vertical(delta);
                } else if delta.abs() == 1 {
                    // A section header is selected: move the section.
                    self.move_selected_horizontal(delta);
                }
                Act::none()
            }
            Cmd::CollapseSection => {
                if let Some(s) = self.sel.section {
                    self.update_section(s, |s| s.collapsed = !s.collapsed);
                    self.sel.task = None;
                }
                Act::none()
            }
            Cmd::Filter => self.focus_field(Field::Filter),
            Cmd::ClearFilter => {
                self.set_filter(String::new());
                Act::none()
            }
            Cmd::Palette => self.open_palette(palette::Mode::Commands),
            Cmd::GoToTask => self.open_palette(palette::Mode::Tasks),
            Cmd::GoToBoard => self.open_palette(palette::Mode::Boards),
            Cmd::NextBoard => self.cycle_board(1),
            Cmd::PrevBoard => self.cycle_board(-1),
            Cmd::RenameBoard => match self.board.and_then(|b| self.store.board(b)) {
                Some(b) => {
                    let e = Edit::Board {
                        id: b.id,
                        value: b.name.clone(),
                    };
                    self.start_edit(e)
                }
                None => Act::none(),
            },
            Cmd::DeleteBoard => {
                if let Some(b) = self.board {
                    self.delete_board(b);
                }
                Act::none()
            }
            Cmd::RenameLabel => self.open_palette(palette::Mode::PickLabel(palette::LabelOp::Rename)),
            Cmd::RecolorLabel => self.open_palette(palette::Mode::PickLabel(palette::LabelOp::Recolor)),
            Cmd::DeleteLabel => self.open_palette(palette::Mode::PickLabel(palette::LabelOp::Delete)),
            Cmd::ToggleSubtasks => {
                let show = !self.settings.show_subtasks;
                self.set_setting(prefs::Key::ShowSubtasks, show as usize);
                self.notify(
                    if show {
                        "Showing subtasks on the board"
                    } else {
                        "Subtasks hidden from the board"
                    }
                    .into(),
                    false,
                );
                Act::none()
            }
            Cmd::ToggleSidebar => {
                self.sidebar = !self.sidebar;
                self.board_view_w = None;
                self.db.set_setting("sidebar", if self.sidebar { "1" } else { "0" });
                Act::none()
            }
            Cmd::Undo => {
                self.undo();
                Act::none()
            }
            Cmd::Redo => {
                self.redo();
                Act::none()
            }
            Cmd::ThemeSystem => {
                self.set_theme(ThemePref::System);
                Act::none()
            }
            Cmd::ThemeDark => {
                self.set_theme(ThemePref::Dark);
                Act::none()
            }
            Cmd::ThemeLight => {
                self.set_theme(ThemePref::Light);
                Act::none()
            }
            Cmd::ToggleTheme => {
                self.set_theme(if self.is_dark() {
                    ThemePref::Light
                } else {
                    ThemePref::Dark
                });
                Act::none()
            }
            Cmd::Help => {
                self.help = !self.help;
                self.settings_open = false;
                self.close_palette()
            }
            Cmd::Settings => {
                self.settings_open = !self.settings_open;
                self.help = false;
                self.close_palette()
            }
            Cmd::Quit => {
                self.flush_detail();
                iced::exit()
            }
        };
        self.ensure_sel();
        let follow = self.follow_selection();
        let reveal = self.reveal();
        Act::batch([act, follow, reveal])
    }

    fn set_theme(&mut self, pref: ThemePref) {
        self.settings.theme = pref;
        self.settings.save(&self.db, prefs::Key::Theme);
    }

    /// Changes one setting and saves it.
    pub fn set_setting(&mut self, k: prefs::Key, option: usize) {
        self.settings.set(k, option);
        self.settings.save(&self.db, k);
    }

    fn set_filter(&mut self, text: String) {
        self.filter = Filter::parse(&text);
        self.filter_text = text;
        self.ensure_sel();
    }

    /// Keep the selected palette row visible.
    pub fn palette_scroll(&mut self) -> Act {
        use crate::ui::{PALETTE_ITEM_H, PALETTE_LIST_MAX_H};
        let Some(p) = &mut self.palette else { return Act::none() };
        let row = PALETTE_ITEM_H + 2.0;
        let top = 6.0 + p.sel as f32 * row;
        let view = PALETTE_LIST_MAX_H;
        let new = if top < p.offset {
            Some(top - 6.0)
        } else if top + row > p.offset + view {
            Some(top + row - view + 6.0)
        } else {
            None
        };
        match new {
            Some(y) => {
                p.offset = y.max(0.0);
                operation::scroll_to(
                    PALETTE_LIST,
                    AbsoluteOffset {
                        x: None,
                        y: Some(p.offset),
                    },
                )
            }
            None => Act::none(),
        }
    }

    fn submit_edit(&mut self) -> Act {
        let Some(edit) = &self.edit else { return self.blur() };
        let again = match edit {
            Edit::NewTask { section, before, value } if !value.trim().is_empty() => Some((*section, *before)),
            _ => None,
        };
        let a = self.finish_edit(true);
        match again {
            // Rapid entry: keep adding below the task just created.
            Some((section, before)) => {
                let b = self.start_edit(Edit::NewTask {
                    section,
                    before,
                    value: String::new(),
                });
                let r = self.reveal();
                Act::batch([a, b, r])
            }
            None => {
                let blur = self.blur();
                let r = self.reveal();
                Act::batch([a, blur, r])
            }
        }
    }

    // ------------------------------------------------------------------
    // Update
    // ------------------------------------------------------------------

    pub fn update(&mut self, msg: Message) -> Act {
        let act = self.handle(msg);
        self.tidy_focus();
        act
    }

    fn handle(&mut self, msg: Message) -> Act {
        match msg {
            Message::Key(k) => self.on_key(k),
            Message::Cmd(c) => {
                if self.palette.is_some() {
                    self.palette = None;
                }
                self.run(c)
            }
            Message::PanelCmd(c) => {
                if let Some(d) = &mut self.detail {
                    d.focused = true;
                    d.sub_sel = None;
                }
                self.palette = None;
                self.run(c)
            }
            Message::FocusPanel => {
                if let Some(d) = &mut self.detail {
                    d.focused = true;
                }
                Act::none()
            }
            Message::MouseDown => self.on_mouse_down(),
            Message::WindowFocused => self.sync_focus(),
            Message::FocusSync(generation, id) => self.on_focus_sync(generation, id),

            Message::PressTask(id, grab) => {
                self.flush_detail();
                let mut act = self.finish_edit(true);
                if self.palette.is_some() {
                    act = self.close_palette();
                }
                let size = self
                    .card_bounds
                    .get(&id)
                    .map(|r| r.size())
                    .unwrap_or(Size::new(COL_W - 16.0, 60.0));
                if let Some(t) = self.store.task(id) {
                    self.sel = Sel {
                        section: Some(t.section_id),
                        task: Some(id),
                    };
                }
                self.drag = Some(Drag {
                    item: DragItem::Task(id),
                    grab,
                    origin: None,
                    active: false,
                    target: None,
                    size,
                });
                // A click selects the card: keys go to the board, the panel previews it.
                let open = match &self.detail {
                    Some(d) if d.task != id => self.open_detail(id),
                    _ => Act::none(),
                };
                if let Some(d) = &mut self.detail {
                    d.focused = false;
                    d.sub_sel = None;
                }
                let blur = self.blur();
                Act::batch([act, open, blur])
            }
            Message::PressSection(id, grab) => {
                self.flush_detail();
                let act = self.finish_edit(true);
                self.sel = Sel {
                    section: Some(id),
                    task: None,
                };
                self.drag = Some(Drag {
                    item: DragItem::Section(id),
                    grab,
                    origin: None,
                    active: false,
                    target: None,
                    size: Size::new(COL_W, 48.0),
                });
                if let Some(d) = &mut self.detail {
                    d.focused = false;
                    d.sub_sel = None;
                }
                let blur = self.blur();
                Act::batch([act, blur])
            }
            Message::OpenTaskMouse(id) => {
                self.drag = None;
                self.open_detail(id)
            }
            Message::RenameSectionMouse(id) => {
                self.drag = None;
                let name = self.store.section(id).map(|s| s.name.clone()).unwrap_or_default();
                self.start_edit(Edit::Section { id, value: name })
            }
            Message::CursorMoved(p) => {
                self.cursor = p;
                if let Some(d) = &mut self.drag {
                    match d.origin {
                        None => d.origin = Some(p),
                        Some(o) if !d.active && (p.x - o.x).hypot(p.y - o.y) > 5.0 => {
                            d.active = true;
                        }
                        _ => {}
                    }
                }
                if self.drag.as_ref().is_some_and(|d| d.active) {
                    let item = self.drag.as_ref().map(|d| d.item);
                    let target = item.and_then(|i| self.drop_target(i, p));
                    if let Some(d) = &mut self.drag {
                        d.target = target;
                    }
                }
                Act::none()
            }
            Message::MouseReleased => {
                if let Some(d) = self.drag.take()
                    && d.active
                    && let Some(target) = d.target
                {
                    match (d.item, target) {
                        (DragItem::Task(id), DropTarget::Task { section, before }) => {
                            self.move_task_to(id, section, before);
                            self.sel = Sel {
                                section: Some(section),
                                task: Some(id),
                            };
                        }
                        (DragItem::Section(id), DropTarget::Section { before }) => {
                            self.move_section_to(id, before);
                        }
                        _ => {}
                    }
                    self.ensure_sel();
                }
                Act::none()
            }
            Message::ColumnBounds(id, r) => {
                self.col_bounds.insert(id, r);
                Act::none()
            }
            Message::CardBounds(id, r) => {
                let first = !self.card_bounds.contains_key(&id);
                self.card_bounds.insert(id, r);
                if first && self.sel.task == Some(id) {
                    return self.reveal();
                }
                Act::none()
            }
            Message::SectionView(id, r) => {
                self.sec_view.insert(id, r);
                Act::none()
            }
            Message::SectionScrolled(id, y) => {
                self.sec_offset.insert(id, y);
                Act::none()
            }
            Message::BoardScrolled(x, w) => {
                self.board_offset = x;
                self.board_view_w = Some(w);
                Act::none()
            }
            Message::BoardView(r) => {
                self.board_view = r.width;
                Act::none()
            }
            Message::BoardContent(r) => {
                self.board_content_w = r.width;
                Act::none()
            }
            Message::SectionContent(id, r) => {
                self.sec_content_h.insert(id, r.height);
                Act::none()
            }
            Message::WindowResized(size) => {
                self.window = size;
                self.board_view_w = None;
                Act::none()
            }
            Message::CloseRequested => {
                self.flush_detail();
                let _ = self.finish_edit(true);
                iced::exit()
            }

            Message::AddTaskAt(section) => {
                self.flush_detail();
                self.detail = None;
                self.sel = Sel {
                    section: Some(section),
                    task: self.sel.task.filter(|_| false),
                };
                if self.store.section(section).is_some_and(|s| s.collapsed) {
                    self.update_section(section, |s| s.collapsed = false);
                }
                self.start_edit(Edit::NewTask {
                    section,
                    before: None,
                    value: String::new(),
                })
            }
            Message::AddSection => {
                let after = self.board_sections().last().map(|s| s.id);
                let a = self.start_edit(Edit::NewSection {
                    after,
                    value: String::new(),
                });
                let secs = self.board_sections().len();
                let x = BOARD_PAD + secs as f32 * (COL_W + COL_GAP);
                Act::batch([
                    a,
                    operation::scroll_to(BOARD_SCROLL, AbsoluteOffset { x: Some(x), y: None }),
                ])
            }
            Message::ToggleCollapse(id) => {
                self.update_section(id, |s| s.collapsed = !s.collapsed);
                self.ensure_sel();
                Act::none()
            }
            Message::EditInput(v) => {
                self.typing_in(Field::Inline);
                if let Some(e) = &mut self.edit {
                    *e.value_mut() = v;
                }
                Act::none()
            }
            Message::EditSubmit => self.submit_field(Field::Inline),

            Message::DetailTitle(v) => {
                self.typing_in(Field::Title);
                if let Some(d) = &mut self.detail {
                    d.title = v;
                }
                Act::none()
            }
            Message::DetailTitleSubmit => self.submit_field(Field::Title),
            Message::DetailDesc(action) => {
                if !matches!(action, text_editor::Action::Scroll { .. }) {
                    self.typing_in(Field::Desc);
                }
                if let Some(d) = &mut self.detail {
                    d.desc.perform(action);
                }
                Act::none()
            }
            Message::DetailSubInput(v) => {
                self.typing_in(Field::SubAdd);
                if let Some(d) = &mut self.detail {
                    d.sub_input = v;
                }
                Act::none()
            }
            Message::DetailSubSubmit => self.submit_field(Field::SubAdd),
            Message::SubToggle(id) => {
                self.flush_detail();
                if let Some(d) = &mut self.detail {
                    d.focused = true;
                }
                self.toggle_done(id);
                Act::none()
            }
            Message::SubOpen(id) => self.open_detail(id),
            Message::SubDelete(id) => {
                self.flush_detail();
                if let Some(d) = &mut self.detail {
                    d.focused = true;
                }
                self.delete_subtask(id);
                Act::none()
            }
            Message::SubEditInput(v) => {
                self.typing_in(Field::SubEdit);
                if let Some(d) = &mut self.detail
                    && let Some((_, value)) = &mut d.sub_edit
                {
                    *value = v;
                }
                Act::none()
            }
            Message::SubEditSubmit => self.submit_field(Field::SubEdit),
            Message::DetailClose => self.close_detail(),
            Message::RemoveLabel(label) => {
                self.flush_detail();
                if let Some(t) = self.detail.as_ref().map(|d| d.task) {
                    self.toggle_label(t, label);
                }
                Act::none()
            }

            Message::PaletteInput(q) => {
                if let Some(p) = &mut self.palette {
                    // Priority picker: a digit applies immediately.
                    if let palette::Mode::Priority(t) = p.mode
                        && q.len() == 1
                        && let Some(d) = q.chars().next().and_then(|c| c.to_digit(10))
                        && d <= 4
                    {
                        self.set_priority(t, Priority::from_i64(d as i64));
                        return self.close_palette();
                    }
                    if let palette::Mode::Due(_) = p.mode
                        && let Some(d) = dates::parse(&q, self.today, self.settings.week_start)
                    {
                        p.cal = d;
                    }
                    p.query = q;
                    p.sel = 0;
                    p.cal_active = false;
                    p.offset = 0.0;
                }
                self.refresh_palette();
                if let Some(p) = &self.palette {
                    let sel = palette::default_selection(self, p);
                    if let Some(p) = &mut self.palette {
                        p.sel = sel;
                    }
                }
                operation::scroll_to(PALETTE_LIST, AbsoluteOffset { x: None, y: Some(0.0) })
            }
            Message::PaletteActivate(i) => self.palette_activate(i),
            Message::PaletteClose => self.close_palette(),
            Message::CalMonth(delta) => {
                if let Some(p) = &mut self.palette {
                    let m = chrono::Months::new(delta.unsigned_abs());
                    let d = if delta >= 0 {
                        p.cal.checked_add_months(m)
                    } else {
                        p.cal.checked_sub_months(m)
                    };
                    if let Some(d) = d {
                        p.cal = d;
                    }
                }
                Act::none()
            }
            Message::PickDate(date) => {
                if let Some(palette::Mode::Due(t)) = self.palette.as_ref().map(|p| p.mode.clone()) {
                    self.set_due(t, Some(date));
                }
                self.close_palette()
            }

            Message::FilterInput(v) => {
                self.typing_in(Field::Filter);
                self.set_filter(v);
                Act::none()
            }
            Message::FilterSubmit => self.submit_field(Field::Filter),
            Message::FilterClear => {
                self.set_filter(String::new());
                if self.filter_focused() {
                    self.blur()
                } else {
                    Act::none()
                }
            }
            Message::SelectBoard(id) => {
                if self.palette.is_some() {
                    self.palette = None;
                }
                self.switch_board(id)
            }
            Message::SystemTheme(mode) => {
                self.system_dark = !matches!(mode, iced::theme::Mode::Light);
                Act::none()
            }
            Message::Tick => {
                self.today = dates::today();
                if self.toast.as_ref().is_some_and(|t| Instant::now() >= t.until) {
                    self.toast = None;
                }
                Act::none()
            }
            Message::ToastUndo => {
                self.toast = None;
                self.undo();
                self.reveal()
            }
            Message::Noop => Act::none(),
            Message::SettingSet(k, i) => {
                if let Some(pos) = prefs::Key::ALL.iter().position(|x| *x == k) {
                    self.settings_sel = pos;
                }
                self.set_setting(k, i);
                Act::none()
            }
            Message::SettingsClose => {
                self.settings_open = false;
                Act::none()
            }
            Message::CopyDbPath => {
                if demo_mode() {
                    Act::none()
                } else {
                    self.notify("Database path copied".into(), false);
                    iced::clipboard::write(data_path().display().to_string())
                }
            }
            Message::Script(i) => crate::devtools::run_step(self, i),
            Message::Shot(name, shot, next) => {
                crate::devtools::save(&name, &shot);
                crate::devtools::continue_after_shot(next)
            }
        }
    }
}

pub fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(n - 1).collect::<String>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn app() -> App {
        let db = Db::open_in_memory().unwrap();
        let store = db.load().unwrap();
        let mut a = App {
            db,
            store,
            undo: vec![],
            redo: vec![],
            board: None,
            sel: Sel::default(),
            board_sel: HashMap::new(),
            edit: None,
            detail: None,
            palette: None,
            help: false,
            filter_text: String::new(),
            filter: Filter::default(),
            focus: None,
            focus_gen: 0,
            sidebar: true,
            toast: None,
            chord: None,
            drag: None,
            settings: Settings::default(),
            settings_open: false,
            settings_sel: 0,
            system_dark: true,
            dark_theme: theme::make_theme(true),
            light_theme: theme::make_theme(false),
            window: Size::new(1280.0, 800.0),
            cursor: Point::ORIGIN,
            board_offset: 0.0,
            board_view_w: None,
            board_view: 0.0,
            board_content_w: 0.0,
            sec_offset: HashMap::new(),
            sec_content_h: HashMap::new(),
            sec_view: HashMap::new(),
            card_bounds: HashMap::new(),
            col_bounds: HashMap::new(),
            recent: vec![],
            today: NaiveDate::from_ymd_opt(2026, 10, 24).unwrap(),
        };
        a.seed();
        a.board = a.store.boards.first().map(|b| b.id);
        a.ensure_sel();
        a
    }

    fn titles(a: &App, section: Id) -> Vec<String> {
        a.store
            .tasks_of(section)
            .filter(|t| a.on_board(t))
            .map(|t| t.title.clone())
            .collect()
    }

    #[test]
    fn seeded_and_selected() {
        let a = app();
        assert_eq!(a.board_sections().len(), 4);
        assert_eq!(a.selected_task().unwrap().title, "Welcome to Meridian");
    }

    #[test]
    fn drop_target_ignores_stale_board_offset() {
        let mut a = app();
        let secs: Vec<Id> = a.board_sections().iter().map(|s| s.id).collect();
        for (i, id) in secs.iter().enumerate() {
            let x = BOARD_PAD + i as f32 * (COL_W + COL_GAP);
            a.col_bounds
                .insert(*id, Rectangle::new(Point::new(x, 50.0), Size::new(COL_W, 600.0)));
        }
        // The board was scrolled earlier, but now everything fits on screen, so the
        // scrollable shows offset 0 even though the last reported offset is larger.
        a.board_offset = 2.0 * (COL_W + COL_GAP);
        a.board_content_w = BOARD_PAD * 2.0 + secs.len() as f32 * (COL_W + COL_GAP);
        a.board_view = a.board_content_w + 100.0;
        let p = Point::new(BOARD_PAD + 2.0 * (COL_W + COL_GAP) + COL_W / 2.0, 300.0);
        let t = a.store.tasks_of(secs[0]).next().unwrap().id;
        assert_eq!(
            a.drop_target(DragItem::Task(t), p),
            Some(DropTarget::Task {
                section: secs[2],
                before: None
            })
        );
    }

    #[test]
    fn move_and_undo_redo() {
        let mut a = app();
        let secs: Vec<Id> = a.board_sections().iter().map(|s| s.id).collect();
        let id = a.sel.task.unwrap();
        let _ = a.run(Cmd::MoveDown);
        assert_eq!(
            titles(&a, secs[0]),
            vec!["Drag me to another section", "Welcome to Meridian"]
        );
        let _ = a.run(Cmd::MoveRight);
        assert_eq!(a.store.task(id).unwrap().section_id, secs[1]);
        assert_eq!(a.sel.section, Some(secs[1]));
        let _ = a.run(Cmd::Undo);
        assert_eq!(a.store.task(id).unwrap().section_id, secs[0]);
        let _ = a.run(Cmd::Undo);
        assert_eq!(
            titles(&a, secs[0]),
            vec!["Welcome to Meridian", "Drag me to another section"]
        );
        let _ = a.run(Cmd::Redo);
        let _ = a.run(Cmd::Redo);
        assert_eq!(a.store.task(id).unwrap().section_id, secs[1]);
        // DB mirrors memory.
        let reloaded = a.db.load().unwrap();
        assert_eq!(reloaded.tasks, a.store.tasks);
    }

    #[test]
    fn progress_bump_clear_and_undo() {
        let mut a = app();
        let id = a.sel.task.unwrap();
        a.update_task(id, |t| t.progress = None);
        let _ = a.run(Cmd::ProgressUp);
        assert_eq!(a.store.task(id).unwrap().progress, Some(10));
        a.set_progress(id, Some(95));
        let _ = a.run(Cmd::ProgressUp);
        assert_eq!(a.store.task(id).unwrap().progress, Some(100));
        let _ = a.run(Cmd::ProgressUp);
        assert_eq!(a.store.task(id).unwrap().progress, Some(100));
        a.set_progress(id, Some(35));
        let _ = a.run(Cmd::ProgressDown);
        assert_eq!(a.store.task(id).unwrap().progress, Some(30));
        let _ = a.run(Cmd::Undo);
        assert_eq!(a.store.task(id).unwrap().progress, Some(35));
        let _ = a.run(Cmd::ClearProgress);
        assert_eq!(a.store.task(id).unwrap().progress, None);
        // 100% does not complete the task by default.
        a.set_progress(id, Some(100));
        assert!(a.store.task(id).unwrap().completed_at.is_none());
    }

    #[test]
    fn complete_at_100_is_one_undo_step() {
        let mut a = app();
        a.settings.complete_at_100 = true;
        let secs: Vec<Id> = a.board_sections().iter().map(|s| s.id).collect();
        let id = a.sel.task.unwrap();
        let section = a.store.task(id).unwrap().section_id;
        a.set_progress(id, Some(100));
        let t = a.store.task(id).unwrap();
        assert!(t.completed_at.is_some());
        assert_eq!(t.section_id, secs[3]);
        let _ = a.run(Cmd::Undo);
        let t = a.store.task(id).unwrap();
        assert!(t.completed_at.is_none());
        assert_eq!(t.section_id, section);
        assert_ne!(t.progress, Some(100));
    }

    #[test]
    fn completing_without_moving() {
        let mut a = app();
        a.settings.move_done = false;
        let id = a.sel.task.unwrap();
        let section = a.store.task(id).unwrap().section_id;
        let _ = a.run(Cmd::ToggleDone);
        let t = a.store.task(id).unwrap();
        assert!(t.completed_at.is_some());
        assert_eq!(t.section_id, section);
    }

    #[test]
    fn toggle_done_moves_to_done() {
        let mut a = app();
        let secs: Vec<Id> = a.board_sections().iter().map(|s| s.id).collect();
        let id = a.sel.task.unwrap();
        let _ = a.run(Cmd::ToggleDone);
        let t = a.store.task(id).unwrap();
        assert!(t.completed_at.is_some());
        assert_eq!(t.section_id, secs[3]);
        assert_eq!(a.store.tasks_of(secs[3]).next().unwrap().id, id);
        let _ = a.run(Cmd::Undo);
        assert!(a.store.task(id).unwrap().completed_at.is_none());
    }

    #[test]
    fn delete_section_and_undo() {
        let mut a = app();
        let secs: Vec<Id> = a.board_sections().iter().map(|s| s.id).collect();
        let before = a.store.tasks.len();
        a.sel = Sel {
            section: Some(secs[0]),
            task: None,
        };
        let _ = a.run(Cmd::Delete);
        assert_eq!(a.board_sections().len(), 3);
        // The two cards plus the welcome task's five subtasks.
        assert_eq!(a.store.tasks.len(), before - 7);
        let _ = a.run(Cmd::Undo);
        assert_eq!(a.board_sections().len(), 4);
        assert_eq!(a.store.tasks.len(), before);
        assert_eq!(a.db.load().unwrap().tasks.len(), before);
    }

    #[test]
    fn navigation() {
        let mut a = app();
        let secs: Vec<Id> = a.board_sections().iter().map(|s| s.id).collect();
        a.nav_vertical(1);
        assert_eq!(a.selected_task().unwrap().title, "Drag me to another section");
        a.nav_vertical(5);
        assert_eq!(a.selected_task().unwrap().title, "Drag me to another section");
        a.nav_horizontal(1);
        assert_eq!(a.sel.section, Some(secs[1]));
        assert_eq!(a.selected_task().unwrap().title, "Plan the week");
        a.nav_vertical(-1);
        assert_eq!(a.sel.task, None);
        a.nav_horizontal(10);
        assert_eq!(a.sel.section, Some(secs[3]));
    }

    #[test]
    fn rapid_entry_and_renumber() {
        let mut a = app();
        let sec = a.board_sections()[0].id;
        let first = a.store.tasks_of(sec).next().unwrap().id;
        // Insert many tasks before the same task to force renumbering.
        for i in 0..80 {
            a.new_task(sec, Some(first), format!("n{i}"));
        }
        let ts: Vec<String> = titles(&a, sec);
        assert_eq!(ts.len(), 82);
        assert_eq!(ts[0], "n0");
        assert_eq!(ts[79], "n79");
        assert_eq!(ts[80], "Welcome to Meridian");
        let reloaded = a.db.load().unwrap();
        let rt: Vec<String> = reloaded
            .tasks_of(sec)
            .filter(|t| t.parent_id.is_none())
            .map(|t| t.title.clone())
            .collect();
        assert_eq!(rt, ts);
    }

    #[test]
    fn labels() {
        let mut a = app();
        let t = a.sel.task.unwrap();
        let l = a.create_label(Some(t), "Urgent stuff".into()).unwrap();
        assert!(a.store.task(t).unwrap().labels.contains(&l));
        a.delete_label(l);
        assert!(!a.store.task(t).unwrap().labels.contains(&l));
        let _ = a.run(Cmd::Undo);
        assert!(a.store.task(t).unwrap().labels.contains(&l));
    }

    #[test]
    fn subtasks_hidden_from_board_until_shown() {
        let mut a = app();
        let parent = a.sel.task.unwrap();
        let kids: Vec<Id> = a.store.children_of(parent).iter().map(|t| t.id).collect();
        assert_eq!(kids.len(), 5);
        let on_grid = |a: &App| a.grid().into_iter().flat_map(|(_, ts)| ts).collect::<Vec<_>>();
        assert!(kids.iter().all(|k| !on_grid(&a).contains(k)));
        let _ = a.run(Cmd::ToggleSubtasks);
        assert!(kids.iter().all(|k| on_grid(&a).contains(k)));
        let _ = a.run(Cmd::ToggleSubtasks);
        assert!(kids.iter().all(|k| !on_grid(&a).contains(k)));
    }

    #[test]
    fn subtask_is_a_full_task_and_nests() {
        let mut a = app();
        let parent = a.sel.task.unwrap();
        let _ = a.open_detail(parent);
        a.detail.as_mut().unwrap().sub_input = "Child".into();
        a.add_subtask();
        let child = a.store.children_of(parent).last().unwrap().id;
        // Open the child: board selection stays on the parent card.
        let _ = a.open_detail(child);
        assert_eq!(a.sel.task, Some(parent));
        a.set_priority(child, Priority::High);
        a.detail.as_mut().unwrap().sub_input = "Grandchild".into();
        a.add_subtask();
        let grand = a.store.children_of(child)[0].id;
        assert_eq!(
            a.store.ancestors_of(grand).iter().map(|t| t.id).collect::<Vec<_>>(),
            vec![child, parent]
        );
        assert_eq!(a.store.task(child).unwrap().priority, Priority::High);
        // Esc walks back up.
        let _ = a.detail_back();
        assert_eq!(a.detail.as_ref().unwrap().task, parent);
        // Completing subtasks drives the parent's progress.
        let total = a.store.children_of(parent).len();
        a.toggle_done(child);
        assert_eq!(a.store.subtask_counts(parent), (1, total));
    }

    #[test]
    fn delete_parent_removes_subtree_and_undo_restores_it() {
        let mut a = app();
        let parent = a.sel.task.unwrap();
        let kid = a.store.children_of(parent)[0].clone();
        let _ = a.open_detail(kid.id);
        a.detail.as_mut().unwrap().sub_input = "Grandchild".into();
        a.add_subtask();
        let before = a.store.tasks.clone();
        a.detail = None;
        a.delete_task(parent);
        assert!(a.store.task(parent).is_none());
        assert!(a.store.task(kid.id).is_none());
        assert_eq!(a.store.tasks.len(), before.len() - 7);
        let _ = a.run(Cmd::Undo);
        assert_eq!(a.store.tasks, before);
        assert_eq!(a.db.load().unwrap().tasks.len(), before.len());
    }

    #[test]
    fn duplicate_copies_subtree() {
        let mut a = app();
        let parent = a.sel.task.unwrap();
        let n = a.store.tasks.len();
        let _ = a.run(Cmd::Duplicate);
        let copy = a.sel.task.unwrap();
        assert_ne!(copy, parent);
        assert_eq!(a.store.tasks.len(), n + 6);
        let titles = |id| {
            a.store
                .children_of(id)
                .iter()
                .map(|t| t.title.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(titles(copy), titles(parent));
        assert!(
            a.store
                .children_of(copy)
                .iter()
                .all(|t| !a.store.children_of(parent).iter().any(|o| o.id == t.id))
        );
    }

    #[test]
    fn reorder_subtasks() {
        let mut a = app();
        let parent = a.sel.task.unwrap();
        let first = a.store.children_of(parent)[0].id;
        let _ = a.open_detail(parent);
        a.detail.as_mut().unwrap().sub_sel = Some(0);
        a.move_subtask(1);
        assert_eq!(a.store.children_of(parent)[1].id, first);
        assert_eq!(a.detail.as_ref().unwrap().sub_sel, Some(1));
    }

    #[test]
    fn set_parent_nests_refuses_cycles_and_undoes() {
        let mut a = app();
        let secs: Vec<Id> = a.board_sections().iter().map(|s| s.id).collect();
        let welcome = a.sel.task.unwrap();
        let drag = a
            .store
            .tasks_of(secs[0])
            .find(|t| t.title.starts_with("Drag"))
            .unwrap()
            .id;
        a.sel.task = Some(drag);
        a.set_parent(drag, Some(welcome));
        let t = a.store.task(drag).unwrap();
        assert_eq!(t.parent_id, Some(welcome));
        assert_eq!(a.store.children_of(welcome).last().unwrap().id, drag);
        // Hidden now, so the parent card takes the selection.
        assert_eq!(a.sel.task, Some(welcome));
        // No cycles: the parent can't go under its own child.
        a.set_parent(welcome, Some(drag));
        assert_eq!(a.store.task(welcome).unwrap().parent_id, None);
        a.set_parent(drag, Some(drag));
        assert_eq!(a.store.task(drag).unwrap().parent_id, Some(welcome));
        // Back to the top level, then undo.
        a.set_parent(drag, None);
        assert_eq!(a.store.task(drag).unwrap().parent_id, None);
        let _ = a.run(Cmd::Undo);
        assert_eq!(a.store.task(drag).unwrap().parent_id, Some(welcome));
        assert_eq!(a.db.load().unwrap().task(drag).unwrap().parent_id, Some(welcome));
    }

    #[test]
    fn parent_picker_skips_self_and_descendants() {
        let a = app();
        let welcome = a.sel.task.unwrap();
        let kid = a.store.children_of(welcome)[0].id;
        let ids = |task| {
            palette::build(&a, &palette::Mode::Parent(task), "")
                .into_iter()
                .map(|i| i.action)
                .collect::<Vec<_>>()
        };
        let top = ids(welcome);
        assert!(!top.contains(&palette::PAction::Parent(Some(welcome))));
        assert!(!top.contains(&palette::PAction::Parent(Some(kid))));
        assert!(!top.contains(&palette::PAction::Parent(None)));
        let sub = ids(kid);
        assert_eq!(sub[0], palette::PAction::Parent(None));
        assert!(sub.contains(&palette::PAction::Parent(Some(welcome))));
    }
}
