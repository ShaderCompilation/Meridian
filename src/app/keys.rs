//! Keyboard handling and focus tracking.
//!
//! A key goes to the first layer that takes it:
//!
//! 1. A drag in progress: Esc cancels it, other keys wait.
//! 2. Global shortcuts (Ctrl+K, Ctrl+P, Ctrl+,, F1…): work everywhere, even while typing.
//! 3. An open modal: help, settings or the palette.
//! 4. The focused text field.
//! 5. The task panel, when it has focus.
//! 6. The board.
//!
//! Keys mean the same thing everywhere:
//! - Arrows / hjkl move inside whatever has focus; Shift (or HJKL) moves the item itself.
//! - Enter opens or confirms. In a field it saves (in the description: Ctrl+Enter).
//! - Esc goes back exactly one step. In a field it cancels what was typed, except in
//!   the task panel, where text is saved as you type.
//! - Tab moves into the task panel and through its fields; Shift+Tab goes back.
//! - Commands act on what is highlighted: the selected card, or in the panel the
//!   selected subtask (else the open task).
//!
//! The app tracks which text field has focus (`App::focus`). Mouse clicks can move
//! focus behind our back, so after each click we ask the widget tree (`sync_focus`).

use iced::Rectangle;
use iced::advanced::widget::operation::{Focusable, Operation, Outcome};
use iced::keyboard::Key;
use iced::keyboard::key::Named;
use iced::widget::operation::{self, AbsoluteOffset, RelativeOffset};

use super::*;

/// A text field that can hold keyboard focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    /// The inline input on the board or sidebar (new task, rename…).
    Inline,
    Filter,
    Palette,
    Title,
    Desc,
    /// The "Add a subtask…" input.
    SubAdd,
    /// Renaming a subtask in place.
    SubEdit,
}

impl Field {
    const ALL: [Field; 7] = [
        Field::Inline,
        Field::Filter,
        Field::Palette,
        Field::Title,
        Field::Desc,
        Field::SubAdd,
        Field::SubEdit,
    ];

    pub fn id(self) -> WId {
        match self {
            Field::Inline => INLINE,
            Field::Filter => FILTER_ID,
            Field::Palette => PALETTE_ID,
            Field::Title => DETAIL_TITLE,
            Field::Desc => DETAIL_DESC,
            Field::SubAdd => DETAIL_SUB,
            Field::SubEdit => SUB_EDIT,
        }
    }

    fn from_id(id: &WId) -> Option<Field> {
        Self::ALL.into_iter().find(|f| f.id() == *id)
    }

    pub fn in_panel(self) -> bool {
        matches!(self, Field::Title | Field::Desc | Field::SubAdd | Field::SubEdit)
    }
}

/// Stops of the task panel's Tab ring; `None` is the panel itself (no field).
const PANEL_RING: [Option<Field>; 4] = [None, Some(Field::Title), Some(Field::Desc), Some(Field::SubAdd)];

/// Property rows, in their visual order in the task panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Property {
    Priority,
    Due,
    Progress,
    Labels,
    Parent,
    Section,
}

impl Property {
    const ALL: [Self; 6] = [
        Self::Priority,
        Self::Due,
        Self::Progress,
        Self::Labels,
        Self::Parent,
        Self::Section,
    ];

    fn command(self) -> Cmd {
        match self {
            Self::Priority => Cmd::SetPriority,
            Self::Due => Cmd::SetDue,
            Self::Progress => Cmd::SetProgress,
            Self::Labels => Cmd::EditLabels,
            Self::Parent => Cmd::SetParent,
            Self::Section => Cmd::MoveTo,
        }
    }
}

/// Finds the focused widget's id (`None` when nothing is focused).
struct FindFocused(Option<WId>);

impl Operation<Option<WId>> for FindFocused {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation<Option<WId>>)) {
        operate(self);
    }

    fn focusable(&mut self, id: Option<&WId>, _bounds: Rectangle, state: &mut dyn Focusable) {
        if state.is_focused() {
            self.0 = id.cloned();
        }
    }

    fn finish(&self) -> Outcome<Option<WId>> {
        Outcome::Some(self.0.clone())
    }
}

/// A key press, decoded once.
struct K {
    named: Option<Named>,
    /// The typed character (with Shift applied); `None` while Ctrl is held.
    ch: Option<String>,
    /// The typed character even with Ctrl held.
    typed: Option<String>,
    /// Layout-independent latin letter, for Ctrl shortcuts.
    latin: Option<char>,
    ctrl: bool,
    shift: bool,
    /// A focused widget already handled the key.
    captured: bool,
}

impl K {
    fn new(k: &KeyPress) -> K {
        let ctrl = k.mods.command();
        let typed = match &k.modified {
            Key::Character(c) => Some(c.to_string()),
            _ => None,
        };
        K {
            named: match &k.key {
                Key::Named(n) => Some(*n),
                _ => None,
            },
            ch: typed.clone().filter(|_| !ctrl),
            typed,
            latin: k.key.to_latin(k.physical),
            ctrl,
            shift: k.mods.shift(),
            captured: k.captured,
        }
    }

    fn is(&self, n: Named) -> bool {
        self.named == Some(n)
    }

    fn c(&self) -> Option<&str> {
        self.ch.as_deref()
    }
}

impl App {
    // ------------------------------------------------------------------
    // Focus
    // ------------------------------------------------------------------

    /// Puts keyboard focus in `f`.
    pub(crate) fn focus_field(&mut self, f: Field) -> Act {
        self.focus = Some(f);
        self.focus_gen += 1;
        self.panel_follows_focus(f);
        let id = f.id();
        match f {
            Field::Filter | Field::Title => Act::batch([operation::focus(id.clone()), operation::select_all(id)]),
            _ => Act::batch([operation::focus(id.clone()), operation::move_cursor_to_end(id)]),
        }
    }

    /// Takes keyboard focus out of any text field.
    pub(crate) fn blur(&mut self) -> Act {
        self.focus = None;
        self.focus_gen += 1;
        operation::focus(NOWHERE)
    }

    /// Asks the widget tree which field really has focus.
    pub(crate) fn sync_focus(&self) -> Act {
        let generation = self.focus_gen;
        iced::advanced::widget::operate(FindFocused(None)).map(move |id| Message::FocusSync(generation, id))
    }

    /// A field reported input, so it has focus.
    pub(crate) fn typing_in(&mut self, f: Field) {
        if self.focus != Some(f) {
            self.focus = Some(f);
            self.focus_gen += 1;
            self.panel_follows_focus(f);
        }
    }

    /// Typing in a panel field focuses the panel; typing on the board leaves it.
    fn panel_follows_focus(&mut self, f: Field) {
        if let Some(d) = &mut self.detail {
            if f != Field::Palette {
                d.prop_sel = None;
            }
            if f.in_panel() {
                d.focused = true;
            } else if f != Field::Palette {
                d.focused = false;
                d.sub_sel = None;
            }
        }
    }

    pub(crate) fn on_focus_sync(&mut self, generation: u64, id: Option<WId>) -> Act {
        if generation != self.focus_gen {
            return Act::none();
        }
        let now = id.as_ref().and_then(Field::from_id);
        // Clicking around inside the palette must not stop typing in it.
        if self.palette.is_some() && now != Some(Field::Palette) {
            return self.focus_field(Field::Palette);
        }
        if now == self.focus {
            return Act::none();
        }
        let old = std::mem::replace(&mut self.focus, now);
        if let Some(f) = now {
            self.panel_follows_focus(f);
        }
        self.left(old)
    }

    pub(crate) fn on_mouse_down(&mut self) -> Act {
        self.chord = None;
        self.sync_focus()
    }

    /// Drops widgets' focus state once their widget is gone.
    pub(crate) fn tidy_focus(&mut self) {
        let gone = match self.focus {
            Some(Field::Inline) => self.edit.is_none(),
            Some(Field::Palette) => self.palette.is_none(),
            Some(Field::SubEdit) => self.detail.as_ref().is_none_or(|d| d.sub_edit.is_none()),
            Some(f) if f.in_panel() => self.detail.is_none(),
            _ => false,
        };
        if gone {
            self.focus = None;
            self.focus_gen += 1;
        }
    }

    /// Focus left `old` without a key saying how (a click elsewhere): keep what was typed.
    fn left(&mut self, old: Option<Field>) -> Act {
        match old {
            Some(Field::Inline) => {
                let a = self.finish_edit(true);
                Act::batch([a, self.reveal()])
            }
            Some(Field::SubEdit) => {
                self.commit_sub_edit();
                Act::none()
            }
            Some(Field::Title | Field::Desc) => {
                self.flush_detail();
                Act::none()
            }
            _ => Act::none(),
        }
    }

    /// Leaves a field with Esc (`cancel`) or a "done" key.
    fn leave_field(&mut self, f: Field, cancel: bool) -> Act {
        let blur = self.blur();
        let act = match f {
            Field::Inline => {
                let a = if cancel {
                    self.edit = None;
                    Act::none()
                } else {
                    self.finish_edit(true)
                };
                Act::batch([a, self.reveal()])
            }
            Field::Filter => {
                if cancel {
                    self.set_filter(String::new());
                }
                self.reveal()
            }
            Field::Title | Field::Desc => {
                // Saved as you type, so Esc keeps the text too.
                self.flush_detail();
                Act::none()
            }
            Field::SubAdd => {
                if cancel {
                    if let Some(d) = &mut self.detail {
                        d.sub_input.clear();
                    }
                } else {
                    self.flush_detail();
                    self.add_subtask();
                }
                Act::none()
            }
            Field::SubEdit => {
                if cancel {
                    if let Some(d) = &mut self.detail {
                        d.sub_edit = None;
                    }
                } else {
                    self.commit_sub_edit();
                }
                Act::none()
            }
            Field::Palette => self.close_palette(),
        };
        if f.in_panel()
            && let Some(d) = &mut self.detail
        {
            d.focused = true;
        }
        Act::batch([blur, act])
    }

    /// Enter in a field.
    pub(crate) fn submit_field(&mut self, f: Field) -> Act {
        match f {
            Field::Inline => self.submit_edit(),
            Field::Filter | Field::Title | Field::SubEdit => self.leave_field(f, false),
            Field::Desc | Field::Palette => Act::none(),
            Field::SubAdd => {
                // Keeps adding; Enter on an empty input is done.
                if self.detail.as_ref().is_none_or(|d| d.sub_input.trim().is_empty()) {
                    self.leave_field(f, true)
                } else {
                    self.flush_detail();
                    self.add_subtask();
                    Act::none()
                }
            }
        }
    }

    fn commit_sub_edit(&mut self) {
        self.flush_detail();
        let Some((sub, value)) = self.detail.as_mut().and_then(|d| d.sub_edit.take()) else {
            return;
        };
        let value = value.trim().to_string();
        // An emptied title keeps the subtask: it may hold details now.
        if !value.is_empty() {
            self.update_task(sub, |t| t.title = value);
        }
    }

    /// Tab / Shift+Tab out of `f`.
    fn tab_from(&mut self, f: Field, back: bool) -> Act {
        if !f.in_panel() {
            return self.leave_field(f, false);
        }
        if f == Field::SubEdit {
            self.commit_sub_edit();
        } else {
            self.flush_detail();
        }
        // A subtask being renamed sits between the description and "Add a subtask".
        let at = match (f, back) {
            (Field::Title, _) => 1,
            (Field::Desc, _) | (Field::SubEdit, false) => 2,
            _ => 3,
        };
        self.panel_ring_step(at, back)
    }

    fn panel_ring_step(&mut self, at: usize, back: bool) -> Act {
        let n = PANEL_RING.len();
        let next = if back { (at + n - 1) % n } else { (at + 1) % n };
        if let Some(d) = &mut self.detail {
            d.focused = true;
            d.sub_sel = None;
            d.prop_sel = None;
        }
        match PANEL_RING[next] {
            Some(f) => self.focus_field(f),
            None => self.blur(),
        }
    }

    fn focus_sub_add(&mut self) -> Act {
        if let Some(d) = &mut self.detail {
            d.sub_sel = None;
        }
        self.focus_field(Field::SubAdd)
    }

    fn select_property(&mut self, property: Property) -> Act {
        self.flush_detail();
        let blur = self.blur();
        if let Some(d) = &mut self.detail {
            d.focused = true;
            d.sub_sel = None;
            d.prop_sel = Some(property);
        }
        blur
    }

    /// Rename in the panel: the selected subtask, else the open task's title.
    pub(crate) fn rename_in_panel(&mut self) -> Act {
        let sub = self
            .detail
            .as_ref()
            .and_then(|d| d.sub_sel)
            .and_then(|i| self.subtask_at(i));
        match sub.and_then(|s| self.store.task(s)).map(|t| (t.id, t.title.clone())) {
            Some(s) => {
                if let Some(d) = &mut self.detail {
                    d.sub_edit = Some(s);
                }
                self.focus_field(Field::SubEdit)
            }
            None => self.focus_field(Field::Title),
        }
    }

    pub(crate) fn subtask_selected(&self) -> bool {
        self.panel_focused() && self.detail.as_ref().is_some_and(|d| d.sub_sel.is_some())
    }

    fn open_children_len(&self) -> usize {
        self.detail.as_ref().map_or(0, |d| self.store.children_of(d.task).len())
    }

    // ------------------------------------------------------------------
    // Navigation helpers
    // ------------------------------------------------------------------

    fn after_nav(&mut self) -> Act {
        let follow = self.follow_selection();
        Act::batch([follow, self.reveal()])
    }

    /// While the board has focus, an open panel shows the selected card.
    pub(crate) fn follow_selection(&mut self) -> Act {
        let Some(d) = &self.detail else { return Act::none() };
        let Some(t) = self.sel.task else { return Act::none() };
        if d.focused || self.board_anchor(d.task).map(|a| a.id) == Some(t) {
            return Act::none();
        }
        let a = self.open_detail(t);
        if let Some(d) = &mut self.detail {
            d.focused = false;
        }
        a
    }

    /// First / last item of what has focus: subtasks in the panel, else the section's cards.
    fn go_edge(&mut self, first: bool) -> Act {
        if self.panel_focused() {
            let len = self.open_children_len();
            if let Some(d) = &mut self.detail
                && len > 0
            {
                d.sub_sel = Some(if first { 0 } else { len - 1 });
                d.prop_sel = None;
            }
            return Act::none();
        }
        self.nav_edge(first);
        self.after_nav()
    }

    // ------------------------------------------------------------------
    // Keys
    // ------------------------------------------------------------------

    pub(crate) fn on_key(&mut self, kp: KeyPress) -> Act {
        let k = K::new(&kp);

        if self.drag.is_some() {
            if k.is(Named::Escape) {
                self.drag = None;
                return Act::none();
            }
            if self.drag.as_ref().is_some_and(|d| d.active) {
                return Act::none();
            }
        }

        if let Some(a) = self.global_key(&k) {
            return a;
        }

        if self.help {
            return self.help_key(&k);
        }
        if self.settings_open {
            return self.settings_key(&k);
        }
        if self.palette.is_some() {
            return self.palette_key(&k);
        }

        if let Some(f) = self.focus {
            if !k.captured && k.c().is_some() {
                // A focused field would have taken this character, so the field
                // lost focus without us noticing: leave it and treat the key as a command.
                let old = self.focus.take();
                self.focus_gen += 1;
                let left = self.left(old);
                return Act::batch([left, self.command_key(&k)]);
            }
            return self.field_key(f, &k);
        }
        if k.captured {
            // A field we didn't know about has focus (e.g. it was clicked).
            return self.sync_focus();
        }
        self.command_key(&k)
    }

    /// Shortcuts that work everywhere.
    fn global_key(&mut self, k: &K) -> Option<Act> {
        if k.is(Named::F1) {
            return Some(self.run(Cmd::Help));
        }
        if !k.ctrl {
            return None;
        }
        let palette = self.palette.is_some();
        let typing = self.focus.is_some();
        let cmd = match (k.latin, k.shift, k.named) {
            // Inside the palette Ctrl+K / Ctrl+P / Ctrl+N belong to it.
            (Some('k'), _, _) if !palette => Cmd::Palette,
            (Some('p'), false, _) if !palette => Cmd::GoToTask,
            (Some(','), _, _) => Cmd::Settings,
            (Some('q'), _, _) => Cmd::Quit,
            (Some('b'), _, _) => Cmd::ToggleSidebar,
            (Some('l'), true, _) => Cmd::ToggleTheme,
            (Some('f'), _, _) => Cmd::Filter,
            (_, false, Some(Named::Tab)) => Cmd::NextBoard,
            (_, true, Some(Named::Tab)) => Cmd::PrevBoard,
            // While typing these would undo board changes, not text.
            (Some('z'), false, _) if !typing => Cmd::Undo,
            (Some('z'), true, _) | (Some('y'), _, _) if !typing => Cmd::Redo,
            (Some('n'), false, _) if !palette => Cmd::NewTask,
            (Some('n'), true, _) => Cmd::NewBoard,
            (Some(d), _, _) if ('1'..='9').contains(&d) => {
                let b = self.store.boards.get(d as usize - '1' as usize)?.id;
                self.close_modals();
                return Some(self.switch_board(b));
            }
            // Some layouts only report "," as the typed character.
            _ if k.typed.as_deref() == Some(",") => Cmd::Settings,
            _ => return None,
        };
        if !matches!(
            cmd,
            Cmd::ToggleSidebar | Cmd::ToggleTheme | Cmd::Undo | Cmd::Redo | Cmd::Quit | Cmd::Settings
        ) {
            self.close_modals();
        }
        Some(self.run(cmd))
    }

    fn close_modals(&mut self) {
        self.help = false;
        self.settings_open = false;
        self.palette = None;
    }

    fn help_key(&mut self, k: &K) -> Act {
        let by = |y: f32| operation::scroll_by(HELP_SCROLL, AbsoluteOffset { x: 0.0, y });
        match (k.named, k.c()) {
            (Some(Named::Escape), _) | (_, Some("?")) | (_, Some("q")) => {
                self.help = false;
                Act::none()
            }
            (Some(Named::ArrowDown), _) | (_, Some("j")) => by(48.0),
            (Some(Named::ArrowUp), _) | (_, Some("k")) => by(-48.0),
            (Some(Named::PageDown), _) | (Some(Named::Space), _) => by(320.0),
            (Some(Named::PageUp), _) => by(-320.0),
            (Some(Named::Home), _) | (_, Some("g")) => operation::snap_to(HELP_SCROLL, RelativeOffset::START),
            (Some(Named::End), _) | (_, Some("G")) => operation::snap_to_end(HELP_SCROLL),
            _ => Act::none(),
        }
    }

    fn settings_key(&mut self, k: &K) -> Act {
        let keys = prefs::Key::ALL;
        let n = keys.len();
        let sel = self.settings_sel.min(n - 1);
        let key = keys[sel];
        let up = (sel + n - 1) % n;
        let down = (sel + 1) % n;
        match (k.named, k.c()) {
            (Some(Named::Escape), _) | (_, Some("q")) => self.settings_open = false,
            (Some(Named::Tab), _) => self.settings_sel = if k.shift { up } else { down },
            (Some(Named::ArrowDown), _) | (_, Some("j")) => self.settings_sel = down,
            (Some(Named::ArrowUp), _) | (_, Some("k")) => self.settings_sel = up,
            (Some(Named::Home), _) | (_, Some("g")) => self.settings_sel = 0,
            (Some(Named::End), _) | (_, Some("G")) => self.settings_sel = n - 1,
            (Some(Named::ArrowLeft), _) | (_, Some("h")) => {
                self.settings.cycle(key, false);
                self.settings.save(&self.db, key);
            }
            (Some(Named::ArrowRight), _) | (Some(Named::Space), _) | (Some(Named::Enter), _) | (_, Some("l")) => {
                self.settings.cycle(key, true);
                self.settings.save(&self.db, key);
            }
            (_, Some(d)) if d.len() == 1 && d.chars().all(|c| c.is_ascii_digit()) => {
                let i: usize = d.parse().unwrap_or(0);
                if i >= 1 {
                    self.set_setting(key, i - 1);
                }
            }
            _ => {}
        }
        Act::none()
    }

    fn palette_key(&mut self, k: &K) -> Act {
        let Some(p) = &mut self.palette else { return Act::none() };
        let n = p.items.len();
        let is_due = matches!(p.mode, palette::Mode::Due(_));

        if is_due {
            let step = match k.named {
                Some(Named::ArrowLeft) if k.shift => Some(chrono::Duration::days(-1)),
                Some(Named::ArrowRight) if k.shift => Some(chrono::Duration::days(1)),
                Some(Named::ArrowUp) if k.shift => Some(chrono::Duration::days(-7)),
                Some(Named::ArrowDown) if k.shift => Some(chrono::Duration::days(7)),
                _ => None,
            };
            if let Some(step) = step {
                p.cal += step;
                p.cal_active = true;
                return Act::none();
            }
            let month = match k.named {
                Some(Named::PageUp) => Some(-1),
                Some(Named::PageDown) => Some(1),
                _ => None,
            };
            if let Some(m) = month {
                p.cal_active = true;
                return self.update(Message::CalMonth(m));
            }
        }

        let mut delta: i32 = match k.named {
            Some(Named::Escape) => {
                return match p.mode.parent() {
                    Some(parent) => self.open_palette(parent),
                    None => self.close_palette(),
                };
            }
            Some(Named::Enter) => {
                if is_due && p.cal_active {
                    let d = p.cal;
                    return self.update(Message::PickDate(d));
                }
                let sel = p.sel;
                return self.palette_activate(sel);
            }
            Some(Named::ArrowDown) => 1,
            Some(Named::ArrowUp) => -1,
            Some(Named::Tab) => {
                if k.shift {
                    -1
                } else {
                    1
                }
            }
            Some(Named::PageDown) => 8,
            Some(Named::PageUp) => -8,
            _ => 0,
        };
        if k.ctrl {
            match k.latin {
                Some('j') | Some('n') => delta = 1,
                Some('p') => delta = -1,
                // Ctrl+K always means the command palette: close it, or switch to it.
                Some('k') if matches!(p.mode, palette::Mode::Commands) => return self.close_palette(),
                Some('k') => return self.open_palette(palette::Mode::Commands),
                _ => {}
            }
        }
        if delta == 0 || n == 0 {
            return Act::none();
        }
        let big = delta.abs() > 1;
        p.sel = if big {
            (p.sel as i32 + delta).clamp(0, n as i32 - 1) as usize
        } else {
            (p.sel as i32 + delta).rem_euclid(n as i32) as usize
        };
        self.palette_scroll()
    }

    /// Keys while typing in `f`.
    fn field_key(&mut self, f: Field, k: &K) -> Act {
        match k.named {
            Some(Named::Escape) => self.leave_field(f, true),
            Some(Named::Enter) if k.ctrl => self.leave_field(f, false),
            // Fields submit through `on_submit`; this covers scripted keys.
            Some(Named::Enter) if !k.captured => self.submit_field(f),
            Some(Named::Tab) => self.tab_from(f, k.shift),
            Some(Named::ArrowDown) if f == Field::Filter => self.leave_field(f, false),
            Some(Named::ArrowDown) if f == Field::Title && !k.shift && !k.ctrl => {
                self.select_property(Property::Priority)
            }
            Some(Named::ArrowUp) if f == Field::SubAdd => {
                // Back up into the list; the half-typed subtask stays in the input.
                let len = self.open_children_len();
                if len == 0 {
                    return self.select_property(Property::Section);
                }
                let blur = self.blur();
                if let Some(d) = &mut self.detail {
                    d.focused = true;
                    d.sub_sel = len.checked_sub(1);
                }
                blur
            }
            _ => Act::none(),
        }
    }

    /// Keys when no field is focused: chords, then the panel, then the board.
    fn command_key(&mut self, k: &K) -> Act {
        if let Some(c) = self.chord.take() {
            if let Some(a) = self.chord_key(c, k) {
                return a;
            }
            // Esc only cancels the chord; any other key does its usual thing.
            if k.is(Named::Escape) {
                return Act::none();
            }
        }
        if self.panel_focused()
            && let Some(a) = self.panel_key(k)
        {
            return a;
        }
        self.board_key(k)
    }

    fn chord_key(&mut self, c: char, k: &K) -> Option<Act> {
        match (c, k.c()?) {
            ('g', "g") => Some(self.go_edge(true)),
            ('g', "J") => Some(self.run(Cmd::MoveBottom)),
            ('g', "K") => Some(self.run(Cmd::MoveTop)),
            ('g', "b") => Some(self.run(Cmd::GoToBoard)),
            ('d', "d") => Some(self.run(Cmd::Delete)),
            _ => None,
        }
    }

    /// Keys for the focused task panel. Anything not handled here falls through
    /// to the board keys, which then act on the selected subtask or the open task.
    fn panel_key(&mut self, k: &K) -> Option<Act> {
        let d = self.detail.as_ref()?;
        let len = self.store.children_of(d.task).len();
        let sel = d.sub_sel;
        let property = d.prop_sel;
        let set = |app: &mut App, v: Option<usize>| {
            if let Some(d) = &mut app.detail {
                d.sub_sel = v;
                d.prop_sel = None;
            }
        };
        let act = match (k.named, k.c()) {
            (Some(Named::Escape), _) => {
                if sel.is_some() || property.is_some() {
                    set(self, None);
                    Act::none()
                } else {
                    self.detail_back()
                }
            }
            (Some(Named::ArrowDown), _) if !k.shift && property != Some(Property::Section) && sel.is_none() => {
                let next = property.map_or(0, |p| p as usize + 1);
                self.select_property(Property::ALL[next])
            }
            (Some(Named::ArrowDown), _) if !k.shift => match sel {
                Some(i) if i + 1 < len => {
                    set(self, Some(i + 1));
                    Act::none()
                }
                None if len > 0 => {
                    set(self, Some(0));
                    Act::none()
                }
                // ↓ past the last subtask lands in "Add a subtask…".
                _ => self.focus_sub_add(),
            },
            // j stops at the last subtask, so it never ends up typing into the input.
            (_, Some("j")) => {
                if len > 0 {
                    set(self, Some(sel.map_or(0, |i| (i + 1).min(len - 1))));
                }
                Act::none()
            }
            (Some(Named::ArrowUp), _) if !k.shift => match (property, sel) {
                (Some(Property::Priority), _) => self.focus_field(Field::Title),
                (Some(p), _) => self.select_property(Property::ALL[p as usize - 1]),
                (None, Some(i)) if i > 0 => {
                    set(self, Some(i - 1));
                    Act::none()
                }
                _ => self.select_property(Property::Section),
            },
            (_, Some("k")) if !k.shift => {
                set(self, sel.and_then(|i| i.checked_sub(1)));
                Act::none()
            }
            (Some(Named::Home), _) => self.go_edge(true),
            (Some(Named::End), _) | (_, Some("G")) => self.go_edge(false),
            // The panel sits right of the board: ← goes back to it, keeping the panel open.
            (Some(Named::ArrowLeft), _) | (_, Some("h")) if !k.shift => {
                if let Some(d) = &mut self.detail {
                    d.focused = false;
                    d.sub_sel = None;
                    d.prop_sel = None;
                }
                self.reveal()
            }
            (Some(Named::ArrowRight), _) | (_, Some("l")) if !k.shift => Act::none(),
            (Some(Named::Tab), _) => self.panel_ring_step(0, k.shift),
            (Some(Named::Enter | Named::Space), _) if property.is_some() => self.run(property.unwrap().command()),
            (Some(Named::Enter), _) => match sel.and_then(|i| self.subtask_at(i)) {
                Some(sub) => self.open_detail(sub),
                None => self.focus_field(Field::Desc),
            },
            (Some(Named::F2), _) | (_, Some("r")) | (_, Some("e")) => self.rename_in_panel(),
            (_, Some("i")) => self.focus_field(Field::Desc),
            (_, Some("a")) | (_, Some("n")) | (_, Some("o")) | (_, Some("O")) => self.focus_sub_add(),
            _ => return None,
        };
        Some(act)
    }

    fn board_key(&mut self, k: &K) -> Act {
        if k.is(Named::Escape) {
            if self.detail.is_some() {
                return self.close_detail();
            }
            if !self.filter_text.is_empty() {
                self.set_filter(String::new());
                return self.reveal();
            }
            return Act::none();
        }
        if k.is(Named::Tab) {
            // Into the open panel.
            if let Some(d) = &mut self.detail {
                d.focused = true;
            }
            return Act::none();
        }
        let shift = k.shift;
        let cmd = match (k.named, k.c()) {
            (Some(Named::ArrowDown), _) if shift => Cmd::MoveDown,
            (Some(Named::ArrowUp), _) if shift => Cmd::MoveUp,
            (Some(Named::ArrowLeft), _) if shift => Cmd::MoveLeft,
            (Some(Named::ArrowRight), _) if shift => Cmd::MoveRight,
            (Some(Named::ArrowDown), _) | (_, Some("j")) => {
                self.nav_vertical(1);
                return self.after_nav();
            }
            (Some(Named::ArrowUp), _) | (_, Some("k")) => {
                self.nav_vertical(-1);
                return self.after_nav();
            }
            (Some(Named::ArrowLeft), _) | (_, Some("h")) => {
                self.nav_horizontal(-1);
                return self.after_nav();
            }
            (Some(Named::ArrowRight), _) | (_, Some("l")) => {
                self.nav_horizontal(1);
                return self.after_nav();
            }
            (Some(Named::Home), _) => return self.go_edge(true),
            (Some(Named::End), _) | (_, Some("G")) => return self.go_edge(false),
            (_, Some("g")) | (_, Some("d")) => {
                self.chord = k.c().and_then(|c| c.chars().next());
                return Act::none();
            }
            (Some(Named::Enter), _) | (_, Some("e")) => Cmd::Open,
            (Some(Named::F2), _) | (_, Some("r")) => Cmd::Rename,
            (Some(Named::Delete), _) => Cmd::Delete,
            (Some(Named::Space), _) | (_, Some("x")) => Cmd::ToggleDone,
            (_, Some("n")) | (_, Some("o")) => Cmd::NewTask,
            (_, Some("O")) => Cmd::NewTaskAbove,
            (_, Some("N")) => Cmd::NewSection,
            (_, Some("H")) => Cmd::MoveLeft,
            (_, Some("L")) => Cmd::MoveRight,
            (_, Some("J")) => Cmd::MoveDown,
            (_, Some("K")) => Cmd::MoveUp,
            (_, Some("p")) => Cmd::SetPriority,
            (_, Some("t")) | (_, Some("#")) => Cmd::EditLabels,
            (_, Some("D")) | (_, Some("@")) => Cmd::SetDue,
            (_, Some("m")) => Cmd::MoveTo,
            (_, Some("%")) => Cmd::SetProgress,
            (_, Some("+")) | (_, Some("=")) => Cmd::ProgressUp,
            (_, Some("-")) => Cmd::ProgressDown,
            (_, Some("y")) => Cmd::Duplicate,
            (_, Some("z")) => Cmd::CollapseSection,
            (_, Some("S")) => Cmd::ToggleSubtasks,
            (_, Some("P")) => Cmd::SetParent,
            (_, Some("b")) => Cmd::GoToBoard,
            (_, Some("/")) => Cmd::Filter,
            (_, Some(":")) => Cmd::Palette,
            (_, Some("?")) => Cmd::Help,
            (_, Some("u")) => Cmd::Undo,
            (_, Some("U")) => Cmd::Redo,
            (_, Some("[")) => Cmd::PrevBoard,
            (_, Some("]")) => Cmd::NextBoard,
            _ => return Act::none(),
        };
        self.run(cmd)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::tests::app;

    /// Presses `;`-separated keys (script syntax). Like scripted keys, none
    /// count as handled by a widget, so text is typed with messages instead.
    fn press(a: &mut App, keys: &str) {
        for k in keys.split(';') {
            let _ = a.update(Message::Key(crate::devtools::key(k)));
        }
    }

    fn kids(a: &App, id: Id) -> Vec<Id> {
        a.store.children_of(id).iter().map(|t| t.id).collect()
    }

    fn sub_sel(a: &App) -> Option<usize> {
        a.detail.as_ref().and_then(|d| d.sub_sel)
    }

    #[test]
    fn panel_takes_keys_and_left_returns_to_the_board() {
        let mut a = app();
        let welcome = a.sel.task.unwrap();
        press(&mut a, "Enter");
        assert!(a.panel_focused());
        press(&mut a, "j");
        assert_eq!(sub_sel(&a), Some(0));
        // x acts on the highlighted subtask, not the open task.
        press(&mut a, "x");
        let first = kids(&a, welcome)[0];
        assert!(a.store.task(first).unwrap().completed_at.is_some());
        assert!(a.store.task(welcome).unwrap().completed_at.is_none());
        // ← hands the keys back to the board; the panel stays and follows the selection.
        press(&mut a, "Left");
        assert!(a.detail.is_some() && !a.panel_focused());
        press(&mut a, "j");
        let drag = a.sel.task.unwrap();
        assert_ne!(drag, welcome);
        assert_eq!(a.detail.as_ref().unwrap().task, drag);
        assert!(!a.panel_focused());
        // Tab goes back in; Esc closes.
        press(&mut a, "Tab");
        assert!(a.panel_focused());
        press(&mut a, "Esc");
        assert!(a.detail.is_none());
    }

    #[test]
    fn enter_moves_into_an_open_panel_without_reopening() {
        let mut a = app();
        press(&mut a, "Enter;j;Enter");
        let sub = a.detail.as_ref().unwrap().task;
        assert!(a.store.task(sub).unwrap().parent_id.is_some());
        press(&mut a, "Left;Enter");
        // Still the subtask: Enter just moved the keyboard back into the panel.
        assert_eq!(a.detail.as_ref().unwrap().task, sub);
        assert!(a.panel_focused());
    }

    #[test]
    fn esc_goes_back_one_step_at_a_time() {
        let mut a = app();
        let welcome = a.sel.task.unwrap();
        press(&mut a, "Enter;j;Enter");
        assert_ne!(a.detail.as_ref().unwrap().task, welcome);
        press(&mut a, "Esc");
        assert_eq!(a.detail.as_ref().unwrap().task, welcome);
        assert_eq!(sub_sel(&a), Some(0));
        press(&mut a, "Esc");
        assert_eq!(sub_sel(&a), None);
        assert!(a.detail.is_some());
        press(&mut a, "Esc");
        assert!(a.detail.is_none());
        a.set_filter("flaky".into());
        press(&mut a, "Esc");
        assert!(a.filter_text.is_empty());
    }

    #[test]
    fn tab_cycles_panel_fields_and_never_reaches_the_filter() {
        let mut a = app();
        press(&mut a, "Enter;Tab");
        assert_eq!(a.focus, Some(Field::Title));
        press(&mut a, "Tab");
        assert_eq!(a.focus, Some(Field::Desc));
        press(&mut a, "Tab");
        assert_eq!(a.focus, Some(Field::SubAdd));
        press(&mut a, "Tab");
        assert_eq!(a.focus, None);
        assert!(a.panel_focused());
        press(&mut a, "S-Tab");
        assert_eq!(a.focus, Some(Field::SubAdd));
        press(&mut a, "S-Tab;S-Tab");
        assert_eq!(a.focus, Some(Field::Title));
    }

    #[test]
    fn title_saves_on_enter_and_esc() {
        let mut a = app();
        let id = a.sel.task.unwrap();
        press(&mut a, "Enter;r");
        assert_eq!(a.focus, Some(Field::Title));
        let _ = a.update(Message::DetailTitle("Hello".into()));
        press(&mut a, "Esc");
        assert_eq!(a.focus, None);
        assert!(a.panel_focused());
        assert_eq!(a.store.task(id).unwrap().title, "Hello");
    }

    #[test]
    fn arrows_move_between_subtasks_and_the_add_input() {
        let mut a = app();
        let welcome = a.sel.task.unwrap();
        let n = kids(&a, welcome).len();
        press(&mut a, "Enter;End");
        assert_eq!(sub_sel(&a), Some(n - 1));
        // j stops at the last subtask…
        press(&mut a, "j");
        assert_eq!(sub_sel(&a), Some(n - 1));
        // …↓ steps into "Add a subtask", and ↑ comes back.
        press(&mut a, "Down");
        assert_eq!(a.focus, Some(Field::SubAdd));
        press(&mut a, "Up");
        assert_eq!(a.focus, None);
        assert_eq!(sub_sel(&a), Some(n - 1));
        press(&mut a, "Home");
        assert_eq!(sub_sel(&a), Some(0));
    }

    #[test]
    fn arrows_navigate_properties_between_title_and_subtasks() {
        let mut a = app();
        press(&mut a, "Enter;Tab;Down");
        assert_eq!(a.focus, None);
        for property in Property::ALL {
            assert_eq!(a.detail.as_ref().unwrap().prop_sel, Some(property));
            assert_eq!(sub_sel(&a), None);
            press(&mut a, "Down");
        }
        assert_eq!(sub_sel(&a), Some(0));
        assert_eq!(a.detail.as_ref().unwrap().prop_sel, None);
        for property in Property::ALL.into_iter().rev() {
            press(&mut a, "Up");
            assert_eq!(a.detail.as_ref().unwrap().prop_sel, Some(property));
            assert_eq!(sub_sel(&a), None);
        }
        press(&mut a, "Up");
        assert_eq!(a.focus, Some(Field::Title));
        assert_eq!(a.detail.as_ref().unwrap().prop_sel, None);
    }

    #[test]
    fn property_activation_edits_open_task_and_preserves_navigation() {
        let mut a = app();
        let task = a.sel.task.unwrap();
        press(&mut a, "Enter;j;Up;Up;Up;Up;Up;Up;Enter");
        assert!(matches!(a.palette.as_ref().unwrap().mode, palette::Mode::Priority(id) if id == task));
        press(&mut a, "Esc;Down;Space");
        assert!(matches!(a.palette.as_ref().unwrap().mode, palette::Mode::Due(id) if id == task));
        press(&mut a, "Esc;Tab");
        assert_eq!(a.focus, Some(Field::Title));
        assert_eq!(a.detail.as_ref().unwrap().prop_sel, None);
        press(&mut a, "Tab");
        assert_eq!(a.focus, Some(Field::Desc));
    }

    #[test]
    fn properties_work_without_subtasks_and_escape_clears_selection() {
        let mut a = app();
        press(&mut a, "j;Enter;Down");
        assert_eq!(a.detail.as_ref().unwrap().prop_sel, Some(Property::Priority));
        press(&mut a, "Esc");
        assert!(a.detail.is_some());
        assert_eq!(a.detail.as_ref().unwrap().prop_sel, None);
        press(&mut a, "a;Up");
        assert_eq!(a.focus, None);
        assert_eq!(a.detail.as_ref().unwrap().prop_sel, Some(Property::Section));
        press(&mut a, "Down");
        assert_eq!(a.focus, Some(Field::SubAdd));
        assert_eq!(a.detail.as_ref().unwrap().prop_sel, None);
    }

    #[test]
    fn add_subtask_input_keeps_adding_until_enter_on_empty() {
        let mut a = app();
        let welcome = a.sel.task.unwrap();
        let n = kids(&a, welcome).len();
        press(&mut a, "Enter;a");
        let _ = a.update(Message::DetailSubInput("One".into()));
        press(&mut a, "Enter");
        assert_eq!(a.focus, Some(Field::SubAdd));
        assert_eq!(kids(&a, welcome).len(), n + 1);
        press(&mut a, "Enter");
        assert_eq!(a.focus, None);
        assert!(a.panel_focused());
    }

    #[test]
    fn task_keys_in_the_panel_act_on_the_selected_subtask() {
        let mut a = app();
        let welcome = a.sel.task.unwrap();
        let before = kids(&a, welcome);
        // Shift+↓ reorders, Delete deletes the subtask (not the open task).
        press(&mut a, "Enter;j;S-Down");
        assert_eq!(kids(&a, welcome)[1], before[0]);
        press(&mut a, "Del");
        assert!(a.store.task(before[0]).is_none());
        assert!(a.store.task(welcome).is_some() && a.detail.is_some());
        press(&mut a, "u");
        assert!(a.store.task(before[0]).is_some());
        // Duplicate keeps the board selection and selects the copy in the panel.
        press(&mut a, "Esc;j;y");
        assert_eq!(a.sel.task, Some(welcome));
        assert_eq!(kids(&a, welcome).len(), before.len() + 1);
        assert_eq!(sub_sel(&a), Some(1));
    }

    #[test]
    fn filter_enter_keeps_and_esc_clears() {
        let mut a = app();
        press(&mut a, "/");
        assert!(a.filter_focused());
        let _ = a.update(Message::FilterInput("flaky".into()));
        press(&mut a, "Enter");
        assert_eq!(a.focus, None);
        assert_eq!(a.filter_text, "flaky");
        press(&mut a, "/");
        press(&mut a, "Esc");
        assert_eq!(a.focus, None);
        assert!(a.filter_text.is_empty());
    }

    #[test]
    fn lost_focus_is_noticed_and_the_key_still_works() {
        let mut a = app();
        let welcome = a.sel.task.unwrap();
        press(&mut a, "n");
        assert_eq!(a.focus, Some(Field::Inline));
        let _ = a.update(Message::EditInput("Buy milk".into()));
        // The input lost focus behind our back (no "captured"): the edit is
        // saved and `k` navigates instead of being swallowed.
        press(&mut a, "k");
        assert!(a.edit.is_none() && a.focus.is_none());
        assert!(a.store.tasks.iter().any(|t| t.title == "Buy milk"));
        assert_eq!(a.sel.task, Some(welcome));
    }

    #[test]
    fn focus_sync_saves_an_edit_left_by_clicking_away() {
        let mut a = app();
        let id = a.sel.task.unwrap();
        press(&mut a, "r");
        let _ = a.update(Message::EditInput("Renamed".into()));
        // A stale measurement is ignored…
        let _ = a.update(Message::FocusSync(a.focus_gen - 1, None));
        assert!(a.edit.is_some());
        // …a current one ends the edit, keeping the text.
        let _ = a.update(Message::FocusSync(a.focus_gen, None));
        assert!(a.edit.is_none());
        assert_eq!(a.store.task(id).unwrap().title, "Renamed");
        // A click into the filter is picked up as well.
        let _ = a.update(Message::FocusSync(a.focus_gen, Some(FILTER_ID)));
        assert!(a.filter_focused());
    }

    #[test]
    fn ctrl_z_does_not_undo_board_changes_while_typing() {
        let mut a = app();
        let id = a.sel.task.unwrap();
        press(&mut a, "x");
        assert!(a.store.task(id).unwrap().completed_at.is_some());
        press(&mut a, "/;C-z");
        assert!(a.store.task(id).unwrap().completed_at.is_some());
        press(&mut a, "Esc;C-z");
        assert!(a.store.task(id).unwrap().completed_at.is_none());
    }

    #[test]
    fn p_opens_the_priority_picker_at_once() {
        let mut a = app();
        let id = a.sel.task.unwrap();
        press(&mut a, "p");
        assert!(matches!(a.palette.as_ref().unwrap().mode, palette::Mode::Priority(_)));
        let _ = a.update(Message::PaletteInput("3".into()));
        assert!(a.palette.is_none());
        assert_eq!(a.store.task(id).unwrap().priority, Priority::from_i64(3));
    }

    #[test]
    fn ctrl_k_always_means_the_command_palette() {
        let mut a = app();
        press(&mut a, "t;C-k");
        assert!(matches!(a.palette.as_ref().unwrap().mode, palette::Mode::Commands));
        press(&mut a, "C-k");
        assert!(a.palette.is_none());
    }

    #[test]
    fn settings_tab_and_shift_tab_go_both_ways() {
        let mut a = app();
        press(&mut a, "C-,");
        assert!(a.settings_open);
        press(&mut a, "Tab;Tab;S-Tab");
        assert_eq!(a.settings_sel, 1);
        press(&mut a, "S-Tab;S-Tab");
        assert_eq!(a.settings_sel, prefs::Key::ALL.len() - 1);
        press(&mut a, "Esc");
        assert!(!a.settings_open);
    }

    #[test]
    fn enter_on_a_collapsed_section_expands_it() {
        let mut a = app();
        let sec = a.sel.section.unwrap();
        press(&mut a, "z");
        assert!(a.store.section(sec).unwrap().collapsed);
        press(&mut a, "Enter");
        assert!(!a.store.section(sec).unwrap().collapsed);
        assert!(a.edit.is_none());
    }

    #[test]
    fn clicking_a_card_gives_the_keys_to_the_board() {
        let mut a = app();
        let welcome = a.sel.task.unwrap();
        press(&mut a, "Enter");
        let drag = a.grid()[0].1[1];
        let _ = a.update(Message::PressTask(drag, Point::ORIGIN));
        let _ = a.update(Message::MouseReleased);
        assert_eq!(a.detail.as_ref().unwrap().task, drag);
        assert!(!a.panel_focused());
        press(&mut a, "k");
        assert_eq!(a.detail.as_ref().unwrap().task, welcome);
    }
}
