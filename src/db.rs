//! SQLite persistence. The in-memory [`Store`] is the source of truth while the
//! app runs; every committed [`Change`] is mirrored here in one transaction.

use crate::model::{Board, Change, Id, Label, Priority, Section, Store, Task};
use chrono::NaiveDate;
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use std::path::Path;

pub struct Db {
    conn: Connection,
}

const MIGRATIONS: &[&str] = &[r#"
CREATE TABLE boards (
    id       INTEGER PRIMARY KEY,
    name     TEXT NOT NULL,
    position REAL NOT NULL
);
CREATE TABLE sections (
    id        INTEGER PRIMARY KEY,
    board_id  INTEGER NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
    name      TEXT NOT NULL,
    position  REAL NOT NULL,
    collapsed INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE tasks (
    id           INTEGER PRIMARY KEY,
    section_id   INTEGER NOT NULL REFERENCES sections(id) ON DELETE CASCADE,
    title        TEXT NOT NULL,
    description  TEXT NOT NULL DEFAULT '',
    priority     INTEGER NOT NULL DEFAULT 0,
    due          TEXT,
    position     REAL NOT NULL,
    created_at   INTEGER NOT NULL,
    completed_at INTEGER
);
CREATE TABLE subtasks (
    id       INTEGER PRIMARY KEY,
    task_id  INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    title    TEXT NOT NULL,
    done     INTEGER NOT NULL DEFAULT 0,
    position INTEGER NOT NULL
);
CREATE TABLE labels (
    id       INTEGER PRIMARY KEY,
    board_id INTEGER NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
    name     TEXT NOT NULL,
    color    INTEGER NOT NULL
);
CREATE TABLE task_labels (
    task_id  INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    label_id INTEGER NOT NULL REFERENCES labels(id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    PRIMARY KEY (task_id, label_id)
);
CREATE TABLE settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
CREATE INDEX idx_sections_board ON sections(board_id);
CREATE INDEX idx_tasks_section ON tasks(section_id);
CREATE INDEX idx_subtasks_task ON subtasks(task_id);
"#,
    // Manual task progress, 0–100. NULL follows the checklist.
    "ALTER TABLE tasks ADD COLUMN progress INTEGER;",
    // Subtasks become full tasks with a parent link. Checklist items move over
    // keeping their ids (all ids come from one shared counter).
    r#"
ALTER TABLE tasks ADD COLUMN parent_id INTEGER
    REFERENCES tasks(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED;
ALTER TABLE tasks ADD COLUMN child_pos REAL NOT NULL DEFAULT 0;
INSERT INTO tasks(id, section_id, title, description, priority, due, position, created_at,
                  completed_at, progress, parent_id, child_pos)
    SELECT s.id, t.section_id, s.title, '', 0, NULL, t.position, t.created_at,
           CASE WHEN s.done THEN CAST(strftime('%s', 'now') AS INTEGER) END, NULL, t.id,
           (s.position + 1) * 1024.0
    FROM subtasks s JOIN tasks t ON t.id = s.task_id;
DROP INDEX idx_subtasks_task;
DROP TABLE subtasks;
CREATE INDEX idx_tasks_parent ON tasks(parent_id);
"#,
];

type R<T> = rusqlite::Result<T>;

impl Db {
    pub fn open(path: &Path) -> R<Self> {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        Self::init(conn)
    }

    #[cfg(test)]
    pub fn open_in_memory() -> R<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> R<Self> {
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let mut db = Db { conn };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&mut self) -> R<()> {
        let version: i64 = self.conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        for (i, sql) in MIGRATIONS.iter().enumerate().skip(version as usize) {
            let tx = self.conn.transaction()?;
            tx.execute_batch(sql)?;
            tx.pragma_update(None, "user_version", i as i64 + 1)?;
            tx.commit()?;
        }
        Ok(())
    }

    pub fn load(&self) -> R<Store> {
        let mut store = Store::default();
        let c = &self.conn;

        let mut q = c.prepare("SELECT id, name, position FROM boards ORDER BY position, id")?;
        store.boards = q
            .query_map([], |r| {
                Ok(Board { id: r.get(0)?, name: r.get(1)?, position: r.get(2)? })
            })?
            .collect::<R<_>>()?;

        let mut q = c.prepare(
            "SELECT id, board_id, name, position, collapsed FROM sections ORDER BY position, id",
        )?;
        store.sections = q
            .query_map([], |r| {
                Ok(Section {
                    id: r.get(0)?,
                    board_id: r.get(1)?,
                    name: r.get(2)?,
                    position: r.get(3)?,
                    collapsed: r.get(4)?,
                })
            })?
            .collect::<R<_>>()?;

        let mut q = c.prepare("SELECT id, board_id, name, color FROM labels ORDER BY id")?;
        store.labels = q
            .query_map([], |r| {
                Ok(Label { id: r.get(0)?, board_id: r.get(1)?, name: r.get(2)?, color: r.get(3)? })
            })?
            .collect::<R<_>>()?;

        let mut q = c.prepare(
            "SELECT id, section_id, title, description, priority, due, position, created_at, completed_at, progress,
                    parent_id, child_pos
             FROM tasks ORDER BY position, id",
        )?;
        store.tasks = q
            .query_map([], |r| {
                let due: Option<String> = r.get(5)?;
                Ok(Task {
                    id: r.get(0)?,
                    section_id: r.get(1)?,
                    title: r.get(2)?,
                    description: r.get(3)?,
                    priority: Priority::from_i64(r.get(4)?),
                    due: due.and_then(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok()),
                    position: r.get(6)?,
                    created_at: r.get(7)?,
                    completed_at: r.get(8)?,
                    progress: r.get(9)?,
                    labels: vec![],
                    parent_id: r.get(10)?,
                    child_pos: r.get(11)?,
                })
            })?
            .collect::<R<_>>()?;

        let index: std::collections::HashMap<Id, usize> =
            store.tasks.iter().enumerate().map(|(i, t)| (t.id, i)).collect();

        let mut q =
            c.prepare("SELECT task_id, label_id FROM task_labels ORDER BY task_id, position")?;
        let rows = q.query_map([], |r| Ok((r.get::<_, Id>(0)?, r.get::<_, Id>(1)?)))?;
        for row in rows {
            let (task_id, label_id) = row?;
            if let Some(&i) = index.get(&task_id) {
                store.tasks[i].labels.push(label_id);
            }
        }

        let max_id: Option<Id> = c.query_row(
            "SELECT MAX(m) FROM (
                SELECT MAX(id) m FROM boards UNION ALL SELECT MAX(id) FROM sections
                UNION ALL SELECT MAX(id) FROM tasks
                UNION ALL SELECT MAX(id) FROM labels)",
            [],
            |r| r.get(0),
        )?;
        store.next_id = max_id.unwrap_or(0);
        Ok(store)
    }

    /// Writes a batch of changes atomically.
    pub fn apply(&mut self, changes: &[Change]) -> R<()> {
        let tx = self.conn.transaction()?;
        for change in changes {
            apply_one(&tx, change)?;
        }
        tx.commit()
    }

    pub fn setting(&self, key: &str) -> Option<String> {
        self.conn
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| r.get(0))
            .optional()
            .ok()
            .flatten()
    }

    pub fn set_setting(&self, key: &str, value: &str) {
        let _ = self.conn.execute(
            "INSERT INTO settings(key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        );
    }
}

fn apply_one(tx: &Transaction, change: &Change) -> R<()> {
    match change {
        Change::Board(before, after) => match (before, after) {
            (_, Some(b)) => {
                tx.execute(
                    "INSERT INTO boards(id, name, position) VALUES (?1, ?2, ?3)
                     ON CONFLICT(id) DO UPDATE SET name = excluded.name, position = excluded.position",
                    params![b.id, b.name, b.position],
                )?;
            }
            (Some(b), None) => {
                tx.execute("DELETE FROM boards WHERE id = ?1", [b.id])?;
            }
            (None, None) => {}
        },
        Change::Section(before, after) => match (before, after) {
            (_, Some(s)) => {
                tx.execute(
                    "INSERT INTO sections(id, board_id, name, position, collapsed) VALUES (?1, ?2, ?3, ?4, ?5)
                     ON CONFLICT(id) DO UPDATE SET board_id = excluded.board_id, name = excluded.name,
                        position = excluded.position, collapsed = excluded.collapsed",
                    params![s.id, s.board_id, s.name, s.position, s.collapsed],
                )?;
            }
            (Some(s), None) => {
                tx.execute("DELETE FROM sections WHERE id = ?1", [s.id])?;
            }
            (None, None) => {}
        },
        Change::Label(before, after) => match (before, after) {
            (_, Some(l)) => {
                tx.execute(
                    "INSERT INTO labels(id, board_id, name, color) VALUES (?1, ?2, ?3, ?4)
                     ON CONFLICT(id) DO UPDATE SET board_id = excluded.board_id, name = excluded.name,
                        color = excluded.color",
                    params![l.id, l.board_id, l.name, l.color],
                )?;
            }
            (Some(l), None) => {
                tx.execute("DELETE FROM labels WHERE id = ?1", [l.id])?;
            }
            (None, None) => {}
        },
        Change::Task(before, after) => match (before, after) {
            (_, Some(t)) => {
                tx.execute(
                    "INSERT INTO tasks(id, section_id, title, description, priority, due, position, created_at, completed_at, progress,
                                      parent_id, child_pos)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
                     ON CONFLICT(id) DO UPDATE SET section_id = excluded.section_id, title = excluded.title,
                        description = excluded.description, priority = excluded.priority, due = excluded.due,
                        position = excluded.position, created_at = excluded.created_at,
                        completed_at = excluded.completed_at, progress = excluded.progress,
                        parent_id = excluded.parent_id, child_pos = excluded.child_pos",
                    params![
                        t.id,
                        t.section_id,
                        t.title,
                        t.description,
                        t.priority as i64,
                        t.due.map(|d| d.format("%Y-%m-%d").to_string()),
                        t.position,
                        t.created_at,
                        t.completed_at,
                        t.progress,
                        t.parent_id,
                        t.child_pos
                    ],
                )?;
                if before.as_ref().is_none_or(|b| b.labels != t.labels) {
                    tx.execute("DELETE FROM task_labels WHERE task_id = ?1", [t.id])?;
                    let mut ins = tx.prepare_cached(
                        "INSERT OR IGNORE INTO task_labels(task_id, label_id, position) VALUES (?1, ?2, ?3)",
                    )?;
                    for (i, l) in t.labels.iter().enumerate() {
                        ins.execute(params![t.id, l, i as i64])?;
                    }
                }
            }
            (Some(t), None) => {
                tx.execute("DELETE FROM tasks WHERE id = ?1", [t.id])?;
            }
            (None, None) => {}
        },
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> (Board, Section, Label, Task) {
        let b = Board { id: 1, name: "B".into(), position: 1.0 };
        let s = Section { id: 2, board_id: 1, name: "Todo".into(), position: 1.0, collapsed: false };
        let l = Label { id: 3, board_id: 1, name: "bug".into(), color: 0 };
        let t = Task {
            id: 4,
            section_id: 2,
            title: "Write tests".into(),
            description: "desc".into(),
            priority: Priority::High,
            due: NaiveDate::from_ymd_opt(2026, 11, 3),
            position: 1.0,
            created_at: 100,
            completed_at: None,
            progress: Some(40),
            labels: vec![3],
            parent_id: None,
            child_pos: 0.0,
        };
        (b, s, l, t)
    }

    fn child(parent: &Task, id: Id, child_pos: f64) -> Task {
        Task {
            id,
            title: format!("sub {id}"),
            description: String::new(),
            priority: Priority::None,
            due: None,
            progress: None,
            labels: vec![],
            parent_id: Some(parent.id),
            child_pos,
            ..parent.clone()
        }
    }

    #[test]
    fn roundtrip_and_undo() {
        let mut db = Db::open_in_memory().unwrap();
        let (b, s, l, t) = sample();
        let mut c5 = child(&t, 5, 1.0);
        c5.position = 2.0;
        c5.completed_at = Some(7);
        let mut c6 = child(&t, 6, 2.0);
        c6.position = 3.0;
        let changes = vec![
            Change::Board(None, Some(b.clone())),
            Change::Section(None, Some(s.clone())),
            Change::Label(None, Some(l.clone())),
            Change::task(None, Some(t.clone())),
            Change::task(None, Some(c5.clone())),
            Change::task(None, Some(c6.clone())),
        ];
        db.apply(&changes).unwrap();
        let store = db.load().unwrap();
        assert_eq!(store.boards, vec![b.clone()]);
        assert_eq!(store.sections, vec![s.clone()]);
        assert_eq!(store.labels, vec![l.clone()]);
        assert_eq!(store.tasks, vec![t.clone(), c5.clone(), c6.clone()]);
        assert_eq!(store.next_id, 6);

        // Update task, delete one subtask
        db.apply(&[Change::task(Some(&c5), None)]).unwrap();
        let mut t2 = t.clone();
        t2.title = "Renamed".into();
        t2.labels.clear();
        t2.progress = None;
        db.apply(&[Change::task(Some(&t), Some(t2.clone()))]).unwrap();
        assert_eq!(db.load().unwrap().tasks, vec![t2.clone(), c6.clone()]);

        // Delete board (children first) and undo (reverse inverses).
        let del = vec![
            Change::task(Some(&c6), None),
            Change::task(Some(&t2), None),
            Change::Label(Some(l.clone()), None),
            Change::Section(Some(s.clone()), None),
            Change::Board(Some(b.clone()), None),
        ];
        db.apply(&del).unwrap();
        assert!(db.load().unwrap().boards.is_empty());
        let undo: Vec<Change> = del.iter().rev().map(|c| c.inverse()).collect();
        db.apply(&undo).unwrap();
        let store = db.load().unwrap();
        assert_eq!(store.tasks, vec![t2, c6]);
        assert_eq!(store.boards, vec![b]);
    }

    #[test]
    fn deleting_parent_row_cascades() {
        let mut db = Db::open_in_memory().unwrap();
        let (b, s, _, mut t) = sample();
        t.labels.clear();
        let c = child(&t, 5, 1.0);
        db.apply(&[
            Change::Board(None, Some(b)),
            Change::Section(None, Some(s)),
            // Child before parent is fine: the parent check is deferred to commit.
            Change::task(None, Some(c)),
            Change::task(None, Some(t.clone())),
        ])
        .unwrap();
        db.apply(&[Change::task(Some(&t), None)]).unwrap();
        assert!(db.load().unwrap().tasks.is_empty());
    }

    #[test]
    fn migrates_checklist_items_to_subtasks() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(MIGRATIONS[0]).unwrap();
        conn.execute_batch(MIGRATIONS[1]).unwrap();
        conn.pragma_update(None, "user_version", 2).unwrap();
        conn.execute_batch(
            "INSERT INTO boards VALUES (1, 'B', 1.0);
             INSERT INTO sections VALUES (2, 1, 'Todo', 1.0, 0);
             INSERT INTO tasks VALUES (3, 2, 'Parent', '', 0, NULL, 5.0, 100, NULL, NULL);
             INSERT INTO subtasks VALUES (9, 3, 'second', 0, 1);
             INSERT INTO subtasks VALUES (8, 3, 'first', 1, 0);",
        )
        .unwrap();
        let db = Db::init(conn).unwrap();
        let store = db.load().unwrap();
        assert_eq!(store.tasks.len(), 3);
        let kids = store.children_of(3);
        let got: Vec<(Id, &str, bool, Id)> = kids
            .iter()
            .map(|t| (t.id, t.title.as_str(), t.completed_at.is_some(), t.section_id))
            .collect();
        assert_eq!(got, vec![(8, "first", true, 2), (9, "second", false, 2)]);
        assert_eq!(store.next_id, 9);
        assert_eq!(store.percent(store.task(3).unwrap()), Some(50));
    }

    #[test]
    fn migrates_old_tasks_to_automatic_progress() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(MIGRATIONS[0]).unwrap();
        conn.pragma_update(None, "user_version", 1).unwrap();
        conn.execute_batch(
            "INSERT INTO boards VALUES (1, 'B', 1.0);
             INSERT INTO sections VALUES (2, 1, 'Todo', 1.0, 0);
             INSERT INTO tasks VALUES (3, 2, 'Old', '', 0, NULL, 1.0, 100, NULL);",
        )
        .unwrap();
        let db = Db::init(conn).unwrap();
        let store = db.load().unwrap();
        assert_eq!(store.tasks.len(), 1);
        assert_eq!(store.tasks[0].progress, None);
    }

    #[test]
    fn settings() {
        let db = Db::open_in_memory().unwrap();
        assert_eq!(db.setting("theme"), None);
        db.set_setting("theme", "dark");
        db.set_setting("theme", "light");
        assert_eq!(db.setting("theme").as_deref(), Some("light"));
    }
}
