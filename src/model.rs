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
pub struct Subtask {
    pub id: Id,
    pub title: String,
    pub done: bool,
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
    /// Label ids, in display order.
    pub labels: Vec<Id>,
    /// Checklist items, in display order.
    pub subtasks: Vec<Subtask>,
}

impl Task {
    pub fn progress(&self) -> (usize, usize) {
        let done = self.subtasks.iter().filter(|s| s.done).count();
        (done, self.subtasks.len())
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

    pub fn task_count(&self, board: Id) -> usize {
        let secs: Vec<Id> = self.sections_of(board).map(|s| s.id).collect();
        self.tasks
            .iter()
            .filter(|t| secs.contains(&t.section_id))
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
            labels: vec![],
            subtasks: vec![],
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
    fn positions() {
        assert_eq!(position_between(None, None), Some(POS_STEP));
        assert_eq!(position_between(Some(1.0), Some(3.0)), Some(2.0));
        assert_eq!(position_between(Some(5.0), None), Some(5.0 + POS_STEP));
        assert_eq!(position_between(None, Some(5.0)), Some(5.0 - POS_STEP));
        assert_eq!(position_between(Some(1.0), Some(1.0 + 1e-9)), None);
    }
}
