//! Keyboard shortcuts cheat sheet.

use iced::widget::{Column, column, container, row, scrollable, space, text};
use iced::{Alignment, Element, Fill, Theme};

use super::icons::{self, icon};
use super::widgets::{faint, kbd, muted};
use crate::app::{HELP_SCROLL, Message};
use crate::commands::{Cmd, Group};
use crate::theme::{self, tokens};

fn entry<'a>(keys: &'a str, label: &'a str) -> Element<'a, Message> {
    let mut ks = row![].spacing(4).align_y(Alignment::Center);
    for (i, k) in keys.split(" / ").enumerate() {
        if i > 0 {
            ks = ks.push(text("/").size(11).style(faint));
        }
        ks = ks.push(kbd(k));
    }
    row![text(label).size(13), space::horizontal(), ks]
        .spacing(10)
        .align_y(Alignment::Center)
        .into()
}

fn group<'a>(title: &'a str, entries: Vec<Element<'a, Message>>) -> (usize, Element<'a, Message>) {
    let n = entries.len() + 2;
    let mut c: Column<'a, Message> = column![text(title).size(12).font(icons::SEMIBOLD).style(|th: &Theme| {
        iced::widget::text::Style {
            color: Some(tokens(th).accent),
        }
    })]
    .spacing(9);
    for e in entries {
        c = c.push(e);
    }
    (n, c.width(Fill).into())
}

pub fn view<'a>() -> Element<'a, Message> {
    let nav = group(
        "Navigation",
        vec![
            entry("h / ←", "Previous section"),
            entry("l / →", "Next section"),
            entry("j / ↓", "Next task"),
            entry("k / ↑", "Previous task (header at top)"),
            entry("gg / Home", "First task"),
            entry("G / End", "Last task"),
            entry("Enter", "Open task (header: rename, collapsed: expand)"),
            entry("Esc", "Go back one step"),
            entry("Ctrl+1", "Board 1…9"),
            entry("[ / ]", "Previous / next board"),
        ],
    );
    let typing = group(
        "While typing",
        vec![
            entry("Enter", "Save (new task: add the next one)"),
            entry("Esc", "Cancel (panel fields keep the text)"),
            entry("Ctrl+Enter", "Save and leave the field"),
            entry("Tab", "Next field / leave"),
            entry("Shift+Tab", "Previous field"),
        ],
    );

    let mut by_group: Vec<(Group, Vec<Element<'a, Message>>)> = vec![];
    for c in Cmd::ALL {
        let m = c.meta();
        if m.keys.is_empty() {
            continue;
        }
        let e = entry(m.keys, m.title);
        match by_group.iter_mut().find(|(g, _)| *g == m.group) {
            Some((_, v)) => v.push(e),
            None => by_group.push((m.group, vec![e])),
        }
    }
    let mut groups: Vec<(usize, Element<'a, Message>)> = vec![nav];
    let extra = group(
        "Task panel",
        vec![
            entry("j / k", "Select subtask"),
            entry("↓", "Past the last: add a subtask"),
            entry("Enter", "Open subtask (none: description)"),
            entry("Space / x", "Toggle done"),
            entry("r / e", "Rename subtask (none: title)"),
            entry("J / K", "Reorder subtask"),
            entry("dd / Del", "Delete subtask"),
            entry("a", "Add subtask"),
            entry("i", "Edit description"),
            entry("Tab", "Title → description → add subtask"),
            entry("h / ←", "Back to the board, panel stays open"),
            entry("Enter / Tab", "From the board: into the panel"),
            entry("Esc", "Deselect, back to parent, close"),
        ],
    );
    for (g, entries) in by_group {
        groups.push(group(g.title(), entries));
    }
    groups.push(extra);
    groups.push(typing);

    // Three columns: each group goes to the currently shortest one.
    let mut cols: Vec<Column<'a, Message>> = vec![column![].spacing(26), column![].spacing(26), column![].spacing(26)];
    let mut heights = [0usize; 3];
    for (h, g) in groups {
        let c = (0..3).min_by_key(|i| heights[*i]).unwrap_or(0);
        heights[c] += h;
        let col = std::mem::replace(&mut cols[c], column![]);
        cols[c] = col.push(g).spacing(26);
    }
    let grid = row(cols.into_iter().map(|c| c.width(Fill).into())).spacing(36);

    let filters = column![
        text("Filter syntax").size(12).font(icons::SEMIBOLD).style(muted),
        text("words   #label   !high (or !1–4)   @today   @overdue   @week   @nodate   @done   @open")
            .size(12)
            .style(faint),
        text("Task keys (x, p, t, D, %, dd…) act on the highlighted card, or in the panel on the selected subtask (else the open task). p then 0–4 sets priority directly.")
            .size(12)
            .style(faint),
    ]
    .spacing(6);

    let content = column![
        row![
            icon(icons::KEYBOARD).size(18).style(muted),
            text("Keyboard shortcuts").size(17).font(icons::SEMIBOLD),
            space::horizontal(),
            kbd("Esc"),
        ]
        .spacing(10)
        .align_y(Alignment::Center),
        scrollable(column![grid, filters].spacing(24))
            .id(HELP_SCROLL)
            .style(theme::scroll)
            .height(iced::Length::Shrink),
    ]
    .spacing(20)
    .padding(26);

    super::modal(
        container(content).max_width(1000).style(theme::modal).into(),
        Message::Cmd(Cmd::Help),
        56.0,
    )
}
