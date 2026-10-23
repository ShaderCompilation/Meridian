//! Task cards.

use iced::widget::text::Span;
use iced::widget::{column, container, rich_text, row, text, text_input};
use iced::{Alignment, Element, Fill, Theme, mouse};

use crate::model::Priority;

use super::area::area;
use super::icons::{self, icon};
use super::widgets::{colored, faint, label_pill, muted};
use crate::app::{App, DragItem, Edit, INLINE, Message};
use crate::dates::{self, Urgency};
use crate::model::Task;
use crate::theme::{self, tokens};

fn content<'a>(app: &'a App, t: &'a Task) -> Element<'a, Message> {
    let done = t.completed_at.is_some();
    let mut col = column![].spacing(7);

    if !t.labels.is_empty() {
        let pills: Vec<Element<'_, Message>> = t
            .labels
            .iter()
            .filter_map(|l| app.store.label(*l))
            .map(|l| label_pill(&l.name, l.color))
            .collect();
        col = col.push(row(pills).spacing(5).wrap().vertical_spacing(5));
    }

    let editing = match &app.edit {
        Some(Edit::Task { id, value }) if *id == t.id => Some(value.as_str()),
        _ => None,
    };
    let title: Element<'_, Message> = match editing {
        Some(v) => text_input("Task title", v)
            .id(INLINE)
            .on_input(Message::EditInput)
            .size(14)
            .padding(0)
            .style(theme::bare_input)
            .into(),
        None if done => rich_text([Span::<(), _>::new(t.title.as_str()).strikethrough(true)])
            .size(14)
            .style(faint)
            .into(),
        None => text(&t.title).size(14).into(),
    };
    let title_row: Element<'_, Message> = if done {
        row![icon(icons::CIRCLE_CHECK).size(14).style(|th: &Theme| colored(tokens(th).success)(th)), title]
            .spacing(7)
            .align_y(Alignment::Start)
            .into()
    } else {
        title
    };
    col = col.push(title_row);

    let mut meta = row![].spacing(12).align_y(Alignment::Center);
    let mut has_meta = false;
    if t.priority != Priority::None && !done {
        has_meta = true;
        let p = t.priority;
        let mut chip = row![icon(icons::FLAG).size(12)].spacing(5).align_y(Alignment::Center);
        if p >= Priority::High {
            chip = chip.push(text(p.label()).size(12).font(icons::MEDIUM));
        }
        meta = meta.push(container(chip).style(move |th: &Theme| {
            container_text(theme::priority_color(tokens(th), p).unwrap_or(tokens(th).muted))
        }));
    }
    if let Some(due) = t.due {
        has_meta = true;
        let u = dates::urgency(due, app.today);
        let label = dates::short(due, app.today);
        let chip = row![icon(icons::CALENDAR).size(12), text(label).size(12).font(icons::MEDIUM)]
            .spacing(5)
            .align_y(Alignment::Center);
        let el: Element<'_, Message> = if done {
            container(chip).style(|th: &Theme| container_text(tokens(th).faint)).into()
        } else {
            match u {
                Urgency::Overdue | Urgency::Today => container(chip)
                    .padding([3, 7])
                    .style(move |th: &Theme| {
                        let tk = tokens(th);
                        theme::pill(if u == Urgency::Overdue { tk.danger } else { tk.warning }, tk.dark)
                    })
                    .into(),
                Urgency::Soon => container(chip).style(|th: &Theme| container_text(tokens(th).warning)).into(),
                Urgency::Later => container(chip).style(|th: &Theme| container_text(tokens(th).muted)).into(),
            }
        };
        meta = meta.push(el);
    }
    let (sd, st) = t.progress();
    if st > 0 {
        has_meta = true;
        let complete = sd == st;
        meta = meta.push(
            container(
                row![icon(icons::LIST_CHECKS).size(12), text(format!("{sd}/{st}")).size(12).font(icons::MEDIUM)]
                    .spacing(5)
                    .align_y(Alignment::Center),
            )
            .style(move |th: &Theme| container_text(if complete { tokens(th).success } else { tokens(th).muted })),
        );
    }
    if !t.description.trim().is_empty() {
        has_meta = true;
        meta = meta.push(icon(icons::TEXT).size(12).style(muted));
    }
    if has_meta {
        col = col.push(meta);
    }

    col.into()
}

fn container_text(c: iced::Color) -> iced::widget::container::Style {
    iced::widget::container::Style { text_color: Some(c), ..Default::default() }
}

pub fn view<'a>(app: &'a App, t: &'a Task, selected: bool) -> Element<'a, Message> {
    let id = t.id;
    let ghost = app.drag.as_ref().is_some_and(|d| d.active && d.item == DragItem::Task(id));
    let open = app.detail.as_ref().is_some_and(|d| d.task == id);
    area(container(content(app, t)).padding([11, 12]).width(Fill))
        .style(move |th, hovered| theme::card(th, hovered && !ghost, selected || open, ghost))
        .on_press(move |p| Message::PressTask(id, p))
        .on_double_click(Message::OpenTaskMouse(id))
        .on_bounds(move |r| Message::CardBounds(id, r))
        .interaction(mouse::Interaction::Pointer)
        .into()
}

pub fn ghost<'a>(app: &'a App, t: &'a Task, width: f32) -> Element<'a, Message> {
    container(content(app, t))
        .padding([11, 12])
        .width(width)
        .style(theme::dragging_card)
        .into()
}
