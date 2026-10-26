//! Task detail side panel.

use iced::keyboard;
use iced::widget::text::Span;
use iced::widget::text_editor::{Binding, KeyPress};
use iced::widget::{
    button, checkbox, column, container, progress_bar, rich_text, row, scrollable, space, text, text_editor, text_input,
};
use iced::{Alignment, Element, Fill, Padding, Theme, mouse};

use super::area::area;
use super::icons::{self, icon};
use super::widgets::{faint, icon_button, kbd, label_pill, muted, small_icon_button};
use crate::app::{App, DETAIL_DESC, DETAIL_SUB, DETAIL_TITLE, DETAIL_W, Detail, Message, SUB_EDIT};
use crate::commands::Cmd;
use crate::dates;
use crate::model::Priority;
use crate::theme::{self, tokens};

fn prop<'a>(ic: char, label: &'a str, value: Element<'a, Message>) -> Element<'a, Message> {
    row![
        row![icon(ic).size(14).style(faint), text(label).size(13).style(muted)]
            .spacing(8)
            .align_y(Alignment::Center)
            .width(110),
        value,
    ]
    .spacing(8)
    .align_y(Alignment::Center)
    .into()
}

fn chip<'a>(content: Element<'a, Message>, msg: Message) -> Element<'a, Message> {
    button(content).padding([5, 10]).style(theme::chip_button).on_press(msg).into()
}

fn caption(s: &str) -> iced::widget::Text<'_> {
    text(s).size(12).font(icons::SEMIBOLD).style(muted)
}

pub fn view<'a>(app: &'a App, d: &'a Detail) -> Element<'a, Message> {
    let Some(t) = app.store.task(d.task) else {
        return space::horizontal().into();
    };
    let section = app.store.section(t.section_id);
    let board = section.and_then(|s| app.store.board(s.board_id));
    let done = t.completed_at.is_some();

    let crumbs = text(format!(
        "{}  ›  {}",
        board.map(|b| b.name.as_str()).unwrap_or(""),
        section.map(|s| s.name.as_str()).unwrap_or("")
    ))
    .size(12)
    .style(faint);

    let top = row![
        crumbs,
        space::horizontal(),
        icon_button(
            if done { icons::CIRCLE } else { icons::CIRCLE_CHECK },
            Message::Cmd(Cmd::ToggleDone),
            if done { "Mark as not done  (x)" } else { "Mark as done  (x)" }
        ),
        icon_button(icons::COPY, Message::Cmd(Cmd::Duplicate), "Duplicate  (y)"),
        icon_button(icons::TRASH, Message::Cmd(Cmd::Delete), "Delete  (dd)"),
        icon_button(icons::X, Message::DetailClose, "Close  (Esc)"),
    ]
    .spacing(2)
    .align_y(Alignment::Center);

    let title = text_input("Task title", &d.title)
        .id(DETAIL_TITLE)
        .on_input(Message::DetailTitle)
        .on_submit(Message::DetailTitleSubmit)
        .size(21)
        .font(icons::SEMIBOLD)
        .padding([6, 8])
        .style(theme::title_input);

    // Properties
    let p = t.priority;
    let prio_value: Element<'_, Message> = row![
        icon(icons::FLAG).size(13).style(move |th: &Theme| iced::widget::text::Style {
            color: Some(theme::priority_color(tokens(th), p).unwrap_or(tokens(th).faint)),
        }),
        text(p.label()).size(13),
    ]
    .spacing(6)
    .align_y(Alignment::Center)
    .into();

    let due_value: Element<'_, Message> = match t.due {
        Some(due) => {
            let u = dates::urgency(due, app.today);
            row![
                text(dates::label(due, app.today, app.settings.date_style)).size(13).font(icons::MEDIUM).style(move |th: &Theme| {
                    let tk = tokens(th);
                    iced::widget::text::Style {
                        color: Some(match u {
                            dates::Urgency::Overdue if !done => tk.danger,
                            dates::Urgency::Today if !done => tk.warning,
                            _ => tk.text,
                        }),
                    }
                }),
                text(due.format(", %b %-d %Y").to_string()).size(12).style(faint),
            ]
            .spacing(0)
            .align_y(Alignment::Center)
            .into()
        }
        None => text("No date").size(13).style(faint).into(),
    };
    let mut due_row = row![chip(due_value, Message::Cmd(Cmd::SetDue))].spacing(4).align_y(Alignment::Center);
    if t.due.is_some() {
        due_row = due_row.push(small_icon_button(icons::X, Message::Cmd(Cmd::ClearDue)));
    }

    let progress_value: Element<'_, Message> = match t.percent() {
        Some(pc) if t.is_manual_progress() => text(format!("{pc}%")).size(13).font(icons::MEDIUM).into(),
        Some(pc) => row![
            text(format!("{pc}%")).size(13).font(icons::MEDIUM),
            text(" from checklist").size(12).style(faint),
        ]
        .align_y(Alignment::Center)
        .into(),
        None => text("Not set").size(13).style(faint).into(),
    };
    let mut progress_row = row![
        small_icon_button(icons::MINUS, Message::Cmd(Cmd::ProgressDown)),
        chip(progress_value, Message::Cmd(Cmd::SetProgress)),
        small_icon_button(icons::PLUS, Message::Cmd(Cmd::ProgressUp)),
    ]
    .spacing(4)
    .align_y(Alignment::Center);
    if let Some(pc) = t.percent() {
        progress_row = progress_row.push(
            container(progress_bar(0.0..=100.0, pc as f32).length(72).girth(4).style(theme::progress))
                .padding(Padding { left: 6.0, ..Padding::ZERO }),
        );
    }
    if t.is_manual_progress() {
        progress_row = progress_row.push(small_icon_button(icons::X, Message::Cmd(Cmd::ClearProgress)));
    }

    let mut labels = row![].spacing(5).align_y(Alignment::Center);
    for l in t.labels.iter().filter_map(|l| app.store.label(*l)) {
        labels = labels.push(
            button(label_pill(&l.name, l.color))
                .padding(0)
                .style(|_: &Theme, _| button::Style::default())
                .on_press(Message::RemoveLabel(l.id)),
        );
    }
    labels = labels.push(chip(
        row![icon(icons::PLUS).size(12), text(if t.labels.is_empty() { "Add label" } else { "" }).size(12)]
            .spacing(4)
            .align_y(Alignment::Center)
            .into(),
        Message::Cmd(Cmd::EditLabels),
    ));

    let props = column![
        prop(icons::FLAG, "Priority", chip(prio_value, Message::Cmd(Cmd::SetPriority))),
        prop(icons::CALENDAR, "Due date", due_row.into()),
        prop(icons::GAUGE, "Progress", progress_row.into()),
        prop(icons::TAG, "Labels", row![labels.wrap().vertical_spacing(5)].into()),
        prop(
            icons::COLUMNS,
            "Section",
            chip(
                text(section.map(|s| s.name.as_str()).unwrap_or("")).size(13).into(),
                Message::Cmd(Cmd::MoveTo)
            )
        ),
    ]
    .spacing(10);

    // Description
    let desc = text_editor(&d.desc)
        .id(DETAIL_DESC)
        .placeholder("Add a description…")
        .on_action(Message::DetailDesc)
        .size(14)
        .padding(12)
        .min_height(120)
        .max_height(420)
        .style(theme::editor)
        .key_binding(|kp: KeyPress| {
            if matches!(kp.key, keyboard::Key::Named(keyboard::key::Named::Enter)) && kp.modifiers.command() {
                return Some(Binding::Unfocus);
            }
            Binding::from_key_press(kp)
        });

    // Checklist
    let (sd, st) = t.checklist();
    let mut checklist = column![
        row![
            caption("CHECKLIST"),
            space::horizontal(),
            text(if st > 0 { format!("{sd} of {st}") } else { String::new() }).size(12).style(faint),
        ]
        .align_y(Alignment::Center)
    ]
    .spacing(6);
    if st > 0 {
        checklist = checklist.push(
            progress_bar(0.0..=st as f32, sd as f32)
                .girth(4)
                .style(theme::progress),
        );
    }
    let mut items = column![].spacing(1);
    for (i, s) in t.subtasks.iter().enumerate() {
        let sid = s.id;
        let selected = d.sub_sel == Some(i);
        let label: Element<'_, Message> = match &d.sub_edit {
            Some((eid, value)) if *eid == sid => text_input("Item", value)
                .id(SUB_EDIT)
                .on_input(Message::SubEditInput)
                .on_submit(Message::SubEditSubmit)
                .size(14)
                .padding([2, 4])
                .style(theme::field_input)
                .into(),
            _ => {
                let t: Element<'_, Message> = if s.done {
                    rich_text([Span::<(), _>::new(s.title.as_str()).strikethrough(true)]).size(14).style(faint).into()
                } else {
                    text(&s.title).size(14).into()
                };
                area(container(t).width(Fill).padding([2, 0]))
                    .on_press(move |_| Message::SubEditStart(sid))
                    .interaction(mouse::Interaction::Text)
                    .into()
            }
        };
        let r = row![
            checkbox(s.done).on_toggle(move |_| Message::SubToggle(sid)).size(16).style(theme::check),
            label,
            small_icon_button(icons::X, Message::SubDelete(sid)),
        ]
        .spacing(10)
        .align_y(Alignment::Center);
        items = items.push(area(container(r).padding([4, 8]).width(Fill)).style(move |th, hovered| {
            let tk = tokens(th);
            iced::widget::container::Style {
                background: if selected {
                    Some(theme::alpha(tk.accent, 0.14).into())
                } else if hovered {
                    Some(theme::alpha(tk.text, 0.04).into())
                } else {
                    None
                },
                border: iced::Border {
                    color: if selected { theme::alpha(tk.accent, 0.6) } else { iced::Color::TRANSPARENT },
                    width: 1.0,
                    radius: 6.0.into(),
                },
                ..Default::default()
            }
        }));
    }
    checklist = checklist.push(items);
    checklist = checklist.push(
        row![
            icon(icons::PLUS).size(14).style(faint),
            text_input("Add an item…", &d.sub_input)
                .id(DETAIL_SUB)
                .on_input(Message::DetailSubInput)
                .on_submit(Message::DetailSubSubmit)
                .size(14)
                .padding([7, 8])
                .style(theme::field_input),
        ]
        .spacing(8)
        .padding(Padding { left: 8.0, ..Padding::ZERO })
        .align_y(Alignment::Center),
    );

    let created = chrono::DateTime::from_timestamp(t.created_at, 0)
        .map(|d| d.with_timezone(&chrono::Local).format("%b %-d, %Y at %H:%M").to_string())
        .unwrap_or_default();
    let mut footer = row![text(format!("Created {created}")).size(11).style(faint)].spacing(6);
    if let Some(c) = t.completed_at.and_then(|c| chrono::DateTime::from_timestamp(c, 0)) {
        footer = footer.push(
            text(format!(
                "· Completed {}",
                c.with_timezone(&chrono::Local).format("%b %-d, %Y")
            ))
            .size(11)
            .style(faint),
        );
    }

    let mut status = row![].spacing(8);
    if done {
        status = status.push(
            container(
                row![icon(icons::CIRCLE_CHECK).size(12), text("Completed").size(12).font(icons::MEDIUM)]
                    .spacing(5)
                    .align_y(Alignment::Center),
            )
            .padding([3, 9])
            .style(|th: &Theme| theme::pill(tokens(th).success, tokens(th).dark)),
        );
    }
    if p >= Priority::High && !done {
        status = status.push(
            container(text(p.label()).size(12).font(icons::MEDIUM))
                .padding([3, 9])
                .style(move |th: &Theme| {
                    theme::pill(theme::priority_color(tokens(th), p).unwrap_or(tokens(th).muted), tokens(th).dark)
                }),
        );
    }

    let body = column![
        title,
        status,
        props,
        container(space::horizontal()).height(1).width(Fill).style(theme::divider),
        column![caption("DESCRIPTION"), desc].spacing(8),
        checklist,
        footer,
        row![kbd("Tab"), text("next field").size(11).style(faint), kbd("Ctrl+Enter"), text("save").size(11).style(faint), kbd("Esc"), text("close").size(11).style(faint)]
            .spacing(6)
            .align_y(Alignment::Center),
    ]
    .spacing(18)
    .padding(Padding { top: 4.0, bottom: 24.0, left: 20.0, right: 22.0 });

    container(
        column![
            container(top).padding(Padding { top: 12.0, bottom: 4.0, left: 20.0, right: 12.0 }),
            scrollable(body)
                .direction(iced::widget::scrollable::Direction::Vertical(
                    iced::widget::scrollable::Scrollbar::new().width(5).scroller_width(5)
                ))
                .style(theme::scroll)
                .height(Fill),
        ],
    )
    .width(DETAIL_W)
    .height(Fill)
    .style(theme::panel)
    .into()
}
