//! User preferences, stored as key/value rows in the `settings` table.
//!
//! Every setting is a small set of named options, so the settings panel can
//! render and cycle them generically through [`Key`].

use crate::db::Db;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemePref {
    #[default]
    System,
    Dark,
    Light,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Density {
    #[default]
    Comfortable,
    Compact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CardProgress {
    #[default]
    Bar,
    Percent,
    Hidden,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DateStyle {
    #[default]
    Relative,
    Absolute,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NewTaskPos {
    #[default]
    Top,
    Bottom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WeekStart {
    #[default]
    Monday,
    Sunday,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    pub theme: ThemePref,
    pub density: Density,
    pub card_progress: CardProgress,
    pub show_subtasks: bool,
    pub date_style: DateStyle,
    pub move_done: bool,
    pub complete_at_100: bool,
    pub new_task_pos: NewTaskPos,
    pub week_start: WeekStart,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            theme: ThemePref::System,
            density: Density::Comfortable,
            card_progress: CardProgress::Bar,
            show_subtasks: false,
            date_style: DateStyle::Relative,
            move_done: true,
            complete_at_100: false,
            new_task_pos: NewTaskPos::Top,
            week_start: WeekStart::Monday,
        }
    }
}

/// One row in the settings panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    Theme,
    Density,
    CardProgress,
    ShowSubtasks,
    DateStyle,
    MoveDone,
    CompleteAt100,
    NewTaskPos,
    WeekStart,
}

impl Key {
    pub const ALL: [Key; 9] = [
        Key::Theme,
        Key::Density,
        Key::CardProgress,
        Key::ShowSubtasks,
        Key::DateStyle,
        Key::MoveDone,
        Key::CompleteAt100,
        Key::NewTaskPos,
        Key::WeekStart,
    ];

    pub fn group(self) -> &'static str {
        match self {
            Key::Theme | Key::Density | Key::CardProgress | Key::ShowSubtasks | Key::DateStyle => "APPEARANCE",
            Key::MoveDone | Key::CompleteAt100 | Key::NewTaskPos => "BEHAVIOUR",
            Key::WeekStart => "DATES",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Key::Theme => "Theme",
            Key::Density => "Card density",
            Key::CardProgress => "Progress on cards",
            Key::ShowSubtasks => "Subtasks on the board",
            Key::DateStyle => "Due date style",
            Key::MoveDone => "Move completed tasks",
            Key::CompleteAt100 => "Complete at 100%",
            Key::NewTaskPos => "New tasks go to",
            Key::WeekStart => "Week starts on",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Key::Theme => "Follow the system, or always dark or light",
            Key::Density => "Compact fits more cards on screen",
            Key::CardProgress => "How task progress shows on cards",
            Key::ShowSubtasks => "Show subtasks as their own cards, or only inside their parent",
            Key::DateStyle => "“Tomorrow, Fri” or “Nov 3”",
            Key::MoveDone => "Move a task to the Done section when it is completed",
            Key::CompleteAt100 => "Mark a task done when its progress reaches 100%",
            Key::NewTaskPos => "Where n on a section header adds the task",
            Key::WeekStart => "Used by “next week” and the calendar",
        }
    }

    /// Labels shown in the panel. Toggles use `["Off", "On"]`.
    pub fn options(self) -> &'static [&'static str] {
        match self {
            Key::Theme => &["System", "Dark", "Light"],
            Key::Density => &["Comfortable", "Compact"],
            Key::CardProgress => &["Bar", "Percent", "Hidden"],
            Key::DateStyle => &["Relative", "Absolute"],
            Key::ShowSubtasks => &["Hidden", "Shown"],
            Key::MoveDone | Key::CompleteAt100 => &["Off", "On"],
            Key::NewTaskPos => &["Top", "Bottom"],
            Key::WeekStart => &["Monday", "Sunday"],
        }
    }

    fn db_key(self) -> &'static str {
        match self {
            Key::Theme => "theme",
            Key::Density => "density",
            Key::CardProgress => "card_progress",
            Key::ShowSubtasks => "show_subtasks",
            Key::DateStyle => "date_style",
            Key::MoveDone => "move_done",
            Key::CompleteAt100 => "complete_at_100",
            Key::NewTaskPos => "new_task_pos",
            Key::WeekStart => "week_start",
        }
    }

    /// Stored values, parallel to [`Key::options`].
    fn values(self) -> &'static [&'static str] {
        match self {
            Key::Theme => &["system", "dark", "light"],
            Key::Density => &["comfortable", "compact"],
            Key::CardProgress => &["bar", "percent", "hidden"],
            Key::DateStyle => &["relative", "absolute"],
            Key::MoveDone | Key::CompleteAt100 | Key::ShowSubtasks => &["0", "1"],
            Key::NewTaskPos => &["top", "bottom"],
            Key::WeekStart => &["monday", "sunday"],
        }
    }
}

impl Settings {
    pub fn load(db: &Db) -> Self {
        let mut s = Settings::default();
        for k in Key::ALL {
            if let Some(v) = db.setting(k.db_key())
                && let Some(i) = k.values().iter().position(|x| *x == v)
            {
                s.set(k, i);
            }
        }
        s
    }

    pub fn save(&self, db: &Db, k: Key) {
        db.set_setting(k.db_key(), k.values()[self.get(k)]);
    }

    /// Index of the current option for `k`.
    pub fn get(&self, k: Key) -> usize {
        match k {
            Key::Theme => self.theme as usize,
            Key::Density => self.density as usize,
            Key::CardProgress => self.card_progress as usize,
            Key::ShowSubtasks => self.show_subtasks as usize,
            Key::DateStyle => self.date_style as usize,
            Key::MoveDone => self.move_done as usize,
            Key::CompleteAt100 => self.complete_at_100 as usize,
            Key::NewTaskPos => self.new_task_pos as usize,
            Key::WeekStart => self.week_start as usize,
        }
    }

    /// Sets `k` to option `i`. Out-of-range indexes are ignored.
    pub fn set(&mut self, k: Key, i: usize) {
        if i >= k.options().len() {
            return;
        }
        match k {
            Key::Theme => self.theme = [ThemePref::System, ThemePref::Dark, ThemePref::Light][i],
            Key::Density => self.density = [Density::Comfortable, Density::Compact][i],
            Key::CardProgress => {
                self.card_progress = [CardProgress::Bar, CardProgress::Percent, CardProgress::Hidden][i]
            }
            Key::ShowSubtasks => self.show_subtasks = i == 1,
            Key::DateStyle => self.date_style = [DateStyle::Relative, DateStyle::Absolute][i],
            Key::MoveDone => self.move_done = i == 1,
            Key::CompleteAt100 => self.complete_at_100 = i == 1,
            Key::NewTaskPos => self.new_task_pos = [NewTaskPos::Top, NewTaskPos::Bottom][i],
            Key::WeekStart => self.week_start = [WeekStart::Monday, WeekStart::Sunday][i],
        }
    }

    /// Moves `k` to the next (or previous) option, wrapping around.
    pub fn cycle(&mut self, k: Key, forward: bool) {
        let n = k.options().len();
        let i = self.get(k);
        self.set(k, if forward { (i + 1) % n } else { (i + n - 1) % n });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_through_db() {
        let db = Db::open_in_memory().unwrap();
        assert_eq!(Settings::load(&db), Settings::default());
        let mut s = Settings::default();
        for k in Key::ALL {
            s.cycle(k, true);
            s.save(&db, k);
        }
        let loaded = Settings::load(&db);
        assert_eq!(loaded, s);
        assert_eq!(loaded.theme, ThemePref::Dark);
        assert!(!loaded.move_done);
        assert!(loaded.show_subtasks);
        assert!(loaded.complete_at_100);
    }

    #[test]
    fn get_set_agree_and_cycle_wraps() {
        let mut s = Settings::default();
        for k in Key::ALL {
            for i in 0..k.options().len() {
                s.set(k, i);
                assert_eq!(s.get(k), i, "{k:?}");
            }
            let last = k.options().len() - 1;
            s.cycle(k, true);
            assert_eq!(s.get(k), 0);
            s.cycle(k, false);
            assert_eq!(s.get(k), last);
        }
    }

    #[test]
    fn ignores_unknown_stored_values() {
        let db = Db::open_in_memory().unwrap();
        db.set_setting("theme", "purple");
        assert_eq!(Settings::load(&db).theme, ThemePref::System);
    }
}
