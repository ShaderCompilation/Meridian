//! Board filter: free text plus `#label`, `!priority`, `@today` / `@overdue` /
//! `@week` / `@nodate` / `@done` / `@open` tokens.

use crate::model::{Priority, Store, Task};
use chrono::NaiveDate;

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Filter {
    words: Vec<String>,
    labels: Vec<String>,
    min_priority: Option<Priority>,
    when: Vec<When>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum When {
    Today,
    Overdue,
    Week,
    NoDate,
    Done,
    Open,
}

impl Filter {
    pub fn parse(query: &str) -> Filter {
        let mut f = Filter::default();
        for tok in query.split_whitespace() {
            let lower = tok.to_lowercase();
            if let Some(l) = lower.strip_prefix('#') {
                if !l.is_empty() {
                    f.labels.push(l.to_string());
                }
            } else if let Some(p) = lower.strip_prefix('!') {
                f.min_priority = match p {
                    "" | "1" | "low" | "l" => Some(Priority::Low),
                    "2" | "med" | "medium" | "m" => Some(Priority::Medium),
                    "3" | "high" | "h" => Some(Priority::High),
                    "4" | "urgent" | "u" => Some(Priority::Urgent),
                    _ => None,
                };
            } else if let Some(w) = lower.strip_prefix('@') {
                let when = match w {
                    "today" => Some(When::Today),
                    "overdue" | "late" => Some(When::Overdue),
                    "week" => Some(When::Week),
                    "nodate" | "none" => Some(When::NoDate),
                    "done" => Some(When::Done),
                    "open" | "todo" => Some(When::Open),
                    _ => None,
                };
                if let Some(when) = when {
                    f.when.push(when);
                } else if !w.is_empty() {
                    f.words.push(lower);
                }
            } else {
                f.words.push(lower);
            }
        }
        f
    }

    pub fn is_empty(&self) -> bool {
        self.words.is_empty()
            && self.labels.is_empty()
            && self.min_priority.is_none()
            && self.when.is_empty()
    }

    pub fn matches(&self, task: &Task, store: &Store, today: NaiveDate) -> bool {
        if self.is_empty() {
            return true;
        }
        if let Some(p) = self.min_priority
            && task.priority < p
        {
            return false;
        }
        for l in &self.labels {
            let hit = task.labels.iter().any(|id| {
                store
                    .label(*id)
                    .is_some_and(|label| label.name.to_lowercase().starts_with(l.as_str()))
            });
            if !hit {
                return false;
            }
        }
        for w in &self.when {
            let ok = match w {
                When::Today => task.due.is_some_and(|d| d <= today),
                When::Overdue => task.due.is_some_and(|d| d < today) && task.completed_at.is_none(),
                When::Week => task.due.is_some_and(|d| (d - today).num_days() <= 7),
                When::NoDate => task.due.is_none(),
                When::Done => task.completed_at.is_some(),
                When::Open => task.completed_at.is_none(),
            };
            if !ok {
                return false;
            }
        }
        if !self.words.is_empty() {
            let hay = format!("{}\n{}", task.title, task.description).to_lowercase();
            if !self.words.iter().all(|w| hay.contains(w.as_str())) {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Label;

    #[test]
    fn filter_tokens() {
        let mut store = Store::default();
        store.labels.push(Label { id: 9, board_id: 1, name: "Bug".into(), color: 0 });
        let today = NaiveDate::from_ymd_opt(2026, 10, 24).unwrap();
        let task = Task {
            id: 1,
            section_id: 1,
            title: "Fix login crash".into(),
            description: "on wayland".into(),
            priority: Priority::High,
            due: Some(today.pred_opt().unwrap()),
            position: 1.0,
            created_at: 0,
            completed_at: None,
            progress: None,
            labels: vec![9],
            subtasks: vec![],
        };
        let m = |q: &str| Filter::parse(q).matches(&task, &store, today);
        assert!(m(""));
        assert!(m("login"));
        assert!(m("WAYLAND crash"));
        assert!(!m("logout"));
        assert!(m("#bug"));
        assert!(!m("#feature"));
        assert!(m("!high"));
        assert!(!m("!urgent"));
        assert!(m("@overdue @open"));
        assert!(!m("@done"));
        assert!(m("@today"));
        assert!(!m("@nodate"));
    }
}
