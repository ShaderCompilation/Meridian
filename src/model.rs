//! Plain data types and the in-memory store.

use chrono::NaiveDate;

pub type Id = i64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum Priority {
    #[default]
    None = 0,
    Low = 1,
    Medium = 2,
    High = 3,
    Urgent = 4,
}

impl Priority {
    pub const ALL: [Priority; 5] = [
        Priority::Urgent,
        Priority::High,
        Priority::Medium,
        Priority::Low,
        Priority::None,
    ];

    pub fn from_i64(v: i64) -> Self {
        match v {
            1 => Self::Low,
            2 => Self::Medium,
            3 => Self::High,
            4 => Self::Urgent,
            _ => Self::None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::None => "No priority",
            Self::Low => "Low",
            Self::Medium => "Medium",
            Self::High => "High",
            Self::Urgent => "Urgent",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Board {
    pub id: Id,
    pub name: String,
    pub position: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Section {
    pub id: Id,
    pub board_id: Id,
    pub name: String,
    pub position: f64,
    pub collapsed: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Task {
    pub id: Id,
    pub section_id: Id,
    pub title: String,
    pub description: String,
    pub priority: Priority,
    pub due: Option<NaiveDate>,
    pub position: f64,
    pub created_at: i64,
    pub completed_at: Option<i64>,
    /// Progress set by hand, 0–100. `None` means it follows the checklist.
    pub progress: Option<u8>,
    /// Label ids, in display order.
    pub labels: Vec<Id>,
    /// The task this is a subtask of.
    pub parent_id: Option<Id>,
    /// Order among siblings under the same parent (`position` orders the column).
    pub child_pos: f64,
}

impl Task {
    pub fn is_manual_progress(&self) -> bool {
        self.progress.is_some()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Label {
    pub id: Id,
    pub board_id: Id,
    pub name: String,
    /// Index into the label colour palette.
    pub color: u8,
}

/// One entity-level change. `before == None` is an insert, `after == None` a delete.
/// Every user action is a list of these, which makes undo/redo generic.
#[derive(Debug, Clone, PartialEq)]
pub enum Change {
    Board(Option<Board>, Option<Board>),
    Section(Option<Section>, Option<Section>),
    Task(Option<Box<Task>>, Option<Box<Task>>),
    Label(Option<Label>, Option<Label>),
}

impl Change {
    pub fn inverse(&self) -> Change {
        match self {
            Change::Board(b, a) => Change::Board(a.clone(), b.clone()),
            Change::Section(b, a) => Change::Section(a.clone(), b.clone()),
            Change::Task(b, a) => Change::Task(a.clone(), b.clone()),
            Change::Label(b, a) => Change::Label(a.clone(), b.clone()),
        }
    }

    pub fn task(before: Option<&Task>, after: Option<Task>) -> Change {
        Change::Task(before.cloned().map(Box::new), after.map(Box::new))
    }
}

/// Everything, held in memory. Vectors are kept sorted by `position` (boards,
/// sections, tasks) so views can iterate without sorting.
#[derive(Debug, Default, Clone)]
pub struct Store {
    pub boards: Vec<Board>,
    pub sections: Vec<Section>,
    pub tasks: Vec<Task>,
    pub labels: Vec<Label>,
    pub next_id: Id,
}

fn upsert<T: Clone>(
    list: &mut Vec<T>,
    id_of: impl Fn(&T) -> Id,
    pos_of: impl Fn(&T) -> f64,
    old_id: Option<Id>,
    new: Option<&T>,
) {
    if let Some(id) = old_id {
        list.retain(|x| id_of(x) != id);
    }
    if let Some(item) = new {
        list.retain(|x| id_of(x) != id_of(item));
        let key = (pos_of(item), id_of(item));
        let idx = list.partition_point(|x| (pos_of(x), id_of(x)) <= key);
        list.insert(idx, item.clone());
    }
}

impl Store {
    pub fn alloc_id(&mut self) -> Id {
        self.next_id += 1;
        self.next_id
    }

    pub fn apply(&mut self, change: &Change) {
        match change {
            Change::Board(b, a) => upsert(
                &mut self.boards,
                |x| x.id,
                |x| x.position,
                b.as_ref().map(|x| x.id),
                a.as_ref(),
            ),
            Change::Section(b, a) => upsert(
                &mut self.sections,
                |x| x.id,
                |x| x.position,
                b.as_ref().map(|x| x.id),
                a.as_ref(),
            ),
            Change::Task(b, a) => upsert(
                &mut self.tasks,
                |x| x.id,
                |x| x.position,
                b.as_ref().map(|x| x.id),
                a.as_deref(),
            ),
            Change::Label(b, a) => upsert(
                &mut self.labels,
                |x| x.id,
                |x| x.id as f64,
                b.as_ref().map(|x| x.id),
                a.as_ref(),
            ),
        }
    }

    pub fn board(&self, id: Id) -> Option<&Board> {
        self.boards.iter().find(|b| b.id == id)
    }

    pub fn section(&self, id: Id) -> Option<&Section> {
        self.sections.iter().find(|s| s.id == id)
    }

    pub fn task(&self, id: Id) -> Option<&Task> {
        self.tasks.iter().find(|t| t.id == id)
    }

    pub fn label(&self, id: Id) -> Option<&Label> {
        self.labels.iter().find(|l| l.id == id)
    }

    pub fn sections_of(&self, board: Id) -> impl Iterator<Item = &Section> {
        self.sections.iter().filter(move |s| s.board_id == board)
    }

    pub fn tasks_of(&self, section: Id) -> impl Iterator<Item = &Task> {
        self.tasks.iter().filter(move |t| t.section_id == section)
    }

    pub fn labels_of(&self, board: Id) -> impl Iterator<Item = &Label> {
        self.labels.iter().filter(move |l| l.board_id == board)
    }

    pub fn board_of_task(&self, task: &Task) -> Option<Id> {
        self.section(task.section_id).map(|s| s.board_id)
    }

    /// Direct subtasks, in sibling order.
    pub fn children_of(&self, id: Id) -> Vec<&Task> {
        let mut v: Vec<&Task> = self.tasks.iter().filter(|t| t.parent_id == Some(id)).collect();
        v.sort_by(|a, b| a.child_pos.total_cmp(&b.child_pos).then(a.id.cmp(&b.id)));
        v
    }

    /// All subtasks below `id`, depth-first (parents before their children).
    pub fn descendants_of(&self, id: Id) -> Vec<&Task> {
        let mut out = vec![];
        let mut stack: Vec<&Task> = self.children_of(id).into_iter().rev().collect();
        while let Some(t) = stack.pop() {
            // Guard against a corrupted cycle.
            if t.id == id || out.iter().any(|x: &&Task| x.id == t.id) {
                continue;
            }
            out.push(t);
            stack.extend(self.children_of(t.id).into_iter().rev());
        }
        out
    }

    /// Parent, grandparent, … up to the top-level task (nearest first).
    pub fn ancestors_of(&self, id: Id) -> Vec<&Task> {
        let mut out: Vec<&Task> = vec![];
        let mut cur = self.task(id).and_then(|t| t.parent_id);
        while let Some(p) = cur.and_then(|p| self.task(p)) {
            if p.id == id || out.iter().any(|x| x.id == p.id) {
                break;
            }
            out.push(p);
            cur = p.parent_id;
        }
        out
    }

    /// Completed and total direct subtasks.
    pub fn subtask_counts(&self, id: Id) -> (usize, usize) {
        let kids = self.children_of(id);
        (kids.iter().filter(|t| t.completed_at.is_some()).count(), kids.len())
    }

    /// Progress in percent: the manual value if set, otherwise derived from the
    /// subtasks. `None` when there is neither.
    pub fn percent(&self, task: &Task) -> Option<u8> {
        if let Some(p) = task.progress {
            return Some(p.min(100));
        }
        let (done, total) = self.subtask_counts(task.id);
        (total > 0).then(|| ((done * 100 + total / 2) / total) as u8)
    }

    /// Tasks on `board`; subtasks are counted only when `subtasks` is set.
    pub fn task_count(&self, board: Id, subtasks: bool) -> usize {
        let secs: Vec<Id> = self.sections_of(board).map(|s| s.id).collect();
        self.tasks
            .iter()
            .filter(|t| secs.contains(&t.section_id) && (subtasks || t.parent_id.is_none()))
            .count()
    }
}

/// Spacing between consecutive positions when appending.
pub const POS_STEP: f64 = 1024.0;

/// Position for an item inserted between `before` and `after` (either may be absent).
/// Returns `None` when the gap is too small and the list should be renumbered.
pub fn position_between(before: Option<f64>, after: Option<f64>) -> Option<f64> {
    let p = match (before, after) {
        (None, None) => POS_STEP,
        (Some(b), None) => b + POS_STEP,
        (None, Some(a)) => a - POS_STEP,
        (Some(b), Some(a)) => {
            if (a - b).abs() < 1e-6 {
                return None;
            }
            (a + b) / 2.0
        }
    };
    Some(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(id: Id, section: Id, pos: f64) -> Task {
        Task {
            id,
            section_id: section,
            title: format!("t{id}"),
            description: String::new(),
            priority: Priority::None,
            due: None,
            position: pos,
            created_at: 0,
            completed_at: None,
            progress: None,
            labels: vec![],
            parent_id: None,
            child_pos: 0.0,
        }
    }

    #[test]
    fn apply_keeps_sorted_and_inverse_restores() {
        let mut s = Store::default();
        let a = task(1, 1, 2.0);
        let b = task(2, 1, 1.0);
        s.apply(&Change::task(None, Some(a.clone())));
        s.apply(&Change::task(None, Some(b.clone())));
        assert_eq!(s.tasks.iter().map(|t| t.id).collect::<Vec<_>>(), vec![2, 1]);

        let mut moved = a.clone();
        moved.position = 0.5;
        let c = Change::task(Some(&a), Some(moved));
        s.apply(&c);
        assert_eq!(s.tasks.iter().map(|t| t.id).collect::<Vec<_>>(), vec![1, 2]);
        s.apply(&c.inverse());
        assert_eq!(s.tasks.iter().map(|t| t.id).collect::<Vec<_>>(), vec![2, 1]);

        let d = Change::task(Some(&b), None);
        s.apply(&d);
        assert_eq!(s.tasks.len(), 1);
        s.apply(&d.inverse());
        assert_eq!(s.tasks.len(), 2);
        assert_eq!(s.task(2), Some(&b));
    }

    #[test]
    fn percent_prefers_manual_then_subtasks() {
        let mut s = Store::default();
        let mut t = task(1, 1, 1.0);
        s.apply(&Change::task(None, Some(t.clone())));
        assert_eq!(s.percent(&t), None);
        for (id, done) in [(2, true), (3, false), (4, false)] {
            let mut c = task(id, 1, id as f64);
            c.parent_id = Some(1);
            c.completed_at = done.then_some(1);
            s.apply(&Change::task(None, Some(c)));
        }
        assert_eq!(s.percent(&t), Some(33));
        assert!(!t.is_manual_progress());
        t.progress = Some(80);
        assert_eq!(s.percent(&t), Some(80));
        t.progress = Some(0);
        assert_eq!(s.percent(&t), Some(0));
    }

    #[test]
    fn tree_helpers() {
        let mut s = Store::default();
        let mk = |id: Id, parent: Option<Id>, cp: f64| {
            let mut t = task(id, 1, id as f64);
            t.parent_id = parent;
            t.child_pos = cp;
            t
        };
        // 1 ─┬ 3 ── 4
        //    └ 2
        for t in [
            mk(1, None, 0.0),
            mk(2, Some(1), 2.0),
            mk(3, Some(1), 1.0),
            mk(4, Some(3), 1.0),
        ] {
            s.apply(&Change::task(None, Some(t)));
        }
        let ids = |v: Vec<&Task>| v.iter().map(|t| t.id).collect::<Vec<_>>();
        assert_eq!(ids(s.children_of(1)), vec![3, 2]);
        assert_eq!(ids(s.descendants_of(1)), vec![3, 4, 2]);
        assert_eq!(ids(s.ancestors_of(4)), vec![3, 1]);
        assert_eq!(ids(s.ancestors_of(1)), Vec::<Id>::new());
    }

    #[test]
    fn positions() {
        assert_eq!(position_between(None, None), Some(POS_STEP));
        assert_eq!(position_between(Some(1.0), Some(3.0)), Some(2.0));
        assert_eq!(position_between(Some(5.0), None), Some(5.0 + POS_STEP));
        assert_eq!(position_between(None, Some(5.0)), Some(5.0 - POS_STEP));
        assert_eq!(position_between(Some(1.0), Some(1.0 + 1e-9)), None);
    }
}
