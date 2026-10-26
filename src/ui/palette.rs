//! Command palette overlay (also used for pickers: priority, labels, due date…).

use chrono::{Datelike, Duration, NaiveDate};
use iced::widget::{Column, button, column, container, row, scrollable, space, text, text_input};
use iced::{Alignment, Element, Fill, Length, Padding, Theme};

use super::icons::{self, icon};
use super::widgets::{faint, kbd, muted};
use crate::app::{App, Message, PALETTE_ID, PALETTE_LIST};
use crate::palette::{Mode, Palette};
use crate::settings::WeekStart;
use crate::theme::{self, tokens};

pub const ITEM_H: f32 = 40.0;
pub const LIST_MAX_H: f32 = 392.0;

pub fn view<'a>(app: &'a App, p: &'a Palette) -> Element<'a, Message> {
    let mode_icon = match p.mode {
        Mode::Commands => icons::COMMAND,
        Mode::Tasks => icons::SEARCH,
        Mode::Boards => icons::KANBAN,
        Mode::Priority(_) => icons::FLAG,
        Mode::Progress(_) => icons::GAUGE,
        Mode::Labels(_) | Mode::PickLabel(_) | Mode::RenameLabel(_) => icons::TAG,
        Mode::LabelColor(_) => icons::PALETTE,
        Mode::MoveTo(_) => icons::ARROW_RIGHT_LEFT,
        Mode::Parent(_) => icons::CORNER_DOWN_RIGHT,
        Mode::Due(_) => icons::CALENDAR,
    };

    let mut head = row![icon(mode_icon).size(17).style(muted)].spacing(12).align_y(Alignment::Center);
    if let Some(title) = p.mode.title() {
        head = head.push(
            container(text(title).size(12).font(icons::SEMIBOLD))
                .padding([3, 8])
                .style(|th: &Theme| theme::pill(tokens(th).accent, tokens(th).dark)),
        );
    }
    head = head.push(
        text_input(p.mode.placeholder(), &p.query)
            .id(PALETTE_ID)
            .on_input(Message::PaletteInput)
            .size(16)
            .padding([4, 0])
            .style(theme::bare_input),
    );

    let mut list: Column<'_, Message> = column![].spacing(2).padding(6);
    for (i, item) in p.items.iter().enumerate() {
        let selected = i == p.sel;
        let swatch = item.swatch;
        let prio = item.priority;
        let ic = icon(item.icon).size(15).style(move |th: &Theme| {
            let tk = tokens(th);
            let color = if let Some(s) = swatch {
                theme::label_color(th, s)
            } else if let Some(p) = prio {
                theme::priority_color(tk, p).unwrap_or(tk.faint)
            } else if selected {
                tk.accent
            } else {
                tk.muted
            };
            iced::widget::text::Style { color: Some(color) }
        });
        let mut r = row![ic, text(&item.title).size(14)].spacing(12).align_y(Alignment::Center);
        if !item.detail.is_empty() {
            r = r.push(text(&item.detail).size(12).style(faint));
        }
        r = r.push(space::horizontal());
        if item.checked == Some(true) {
            r = r.push(icon(icons::CHECK).size(15).style(super::widgets::accent));
        }
        if !item.hint.is_empty() {
            r = r.push(kbd(&item.hint));
        }
        list = list.push(
            button(container(r).height(ITEM_H - 2.0).align_y(Alignment::Center))
                .width(Fill)
                .padding([0, 12])
                .style(theme::list_item(selected))
                .on_press(Message::PaletteActivate(i)),
        );
    }
    if p.items.is_empty() {
        let msg = match p.mode {
            Mode::RenameLabel(_) => "Type the new name",
            Mode::Tasks => "No tasks match",
            _ => "No matches",
        };
        list = list.push(container(text(msg).size(13).style(faint)).padding([14, 12]));
    }

    let list_h = ((p.items.len().max(1)) as f32 * (ITEM_H + 2.0) + 12.0).min(LIST_MAX_H);
    let list = scrollable(list)
        .id(PALETTE_LIST)
        .style(theme::scroll)
        .height(Length::Fixed(list_h));

    let body: Element<'_, Message> = if let Mode::Due(_) = p.mode {
        row![container(calendar(app, p.cal)).padding(Padding { left: 6.0, ..Padding::ZERO }), list.width(Fill)]
            .spacing(6)
            .into()
    } else {
        list.into()
    };

    let footer_hints: &[(&str, &str)] = match p.mode {
        Mode::Due(_) => &[("↑↓", "select"), ("Shift+←→", "day"), ("Shift+↑↓", "week"), ("PgUp", "month"), ("Enter", "set")],
        Mode::Labels(_) => &[("↑↓", "navigate"), ("Enter", "toggle"), ("Esc", "done")],
        Mode::Priority(_) => &[("0–4", "set directly"), ("↑↓", "navigate"), ("Enter", "select")],
        Mode::Progress(_) => &[("0–100", "type a value"), ("↑↓", "navigate"), ("Enter", "set")],
        _ => &[("↑↓", "navigate"), ("Enter", "select"), ("Esc", "close")],
    };
    let mut footer = row![].spacing(14).align_y(Alignment::Center);
    for (k, l) in footer_hints {
        footer = footer.push(row![kbd(k), text(*l).size(11).style(faint)].spacing(5).align_y(Alignment::Center));
    }

    let content = column![
        container(head).padding([14, 16]),
        container(space::horizontal()).height(1).width(Fill).style(theme::divider),
        body,
        container(space::horizontal()).height(1).width(Fill).style(theme::divider),
        container(footer).padding([9, 16]),
    ];

    let width = if matches!(p.mode, Mode::Due(_)) { 720 } else { 620 };
    super::modal(
        container(content).width(width).style(theme::modal).into(),
        Message::PaletteClose,
        90.0,
    )
}

fn calendar(app: &App, cursor: NaiveDate) -> Element<'_, Message> {
    let first = cursor.with_day(1).unwrap_or(cursor);
    let week = app.settings.week_start;
    let start = first - Duration::days(crate::dates::days_into_week(first, week));
    let names: [&str; 7] = match week {
        WeekStart::Monday => ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"],
        WeekStart::Sunday => ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"],
    };
    let header = row![
        super::widgets::small_icon_button(icons::CHEVRON_LEFT, Message::CalMonth(-1)),
        container(text(first.format("%B %Y").to_string()).size(13).font(icons::SEMIBOLD))
            .width(Fill)
            .align_x(Alignment::Center),
        super::widgets::small_icon_button(icons::CHEVRON_RIGHT, Message::CalMonth(1)),
    ]
    .align_y(Alignment::Center);

    let mut grid = column![
        row(names.iter().map(|d| {
            container(text(*d).size(11).style(faint)).width(34).align_x(Alignment::Center).into()
        }))
        .spacing(2)
    ]
    .spacing(2);
    for w in 0..6 {
        let mut r = row![].spacing(2);
        for d in 0..7 {
            let day = start + Duration::days(w * 7 + d);
            let selected = day == cursor;
            let today = day == app.today;
            let in_month = day.month() == first.month();
            r = r.push(
                button(container(text(day.day().to_string()).size(12)).center(Length::Fill))
                    .width(34)
                    .height(30)
                    .padding(0)
                    .style(theme::day_button(selected, today, in_month))
                    .on_press(Message::PickDate(day)),
            );
        }
        grid = grid.push(r);
    }
    column![header, grid].spacing(8).padding([10, 8]).width(270).into()
}
