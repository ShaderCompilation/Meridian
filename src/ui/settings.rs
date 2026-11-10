//! Settings panel.

use iced::widget::{Column, button, column, container, row, scrollable, space, text};
use iced::{Alignment, Element, Fill, Padding, Theme};

use super::icons::{self, icon};
use super::widgets::{faint, kbd, muted};
use crate::app::{App, Message, data_path};
use crate::settings::Key;
use crate::theme::{self, tokens};

fn caption(s: &str) -> Element<'_, Message> {
    container(
        text(s)
            .size(11)
            .font(icons::SEMIBOLD)
            .style(|th: &Theme| iced::widget::text::Style {
                color: Some(tokens(th).accent),
            }),
    )
    .padding(Padding {
        top: 14.0,
        bottom: 4.0,
        left: 12.0,
        right: 0.0,
    })
    .into()
}

fn label<'a>(title: &'a str, description: &'a str) -> Element<'a, Message> {
    column![
        text(title).size(13).font(icons::MEDIUM),
        text(description).size(12).style(faint)
    ]
    .spacing(2)
    .width(Fill)
    .into()
}

fn segmented<'a>(key: Key, current: usize) -> Element<'a, Message> {
    let mut seg = row![].spacing(2);
    for (i, opt) in key.options().iter().enumerate() {
        seg = seg.push(
            button(text(*opt).size(12).font(icons::MEDIUM))
                .padding([4, 10])
                .style(theme::segment(i == current))
                .on_press(Message::SettingSet(key, i)),
        );
    }
    container(seg).padding(2).style(theme::segment_track).into()
}

pub fn view(app: &App) -> Element<'_, Message> {
    let mut body: Column<'_, Message> = column![].spacing(2);
    let mut group = "";
    for (i, key) in Key::ALL.into_iter().enumerate() {
        if key.group() != group {
            group = key.group();
            body = body.push(caption(group));
        }
        let selected = i == app.settings_sel;
        let r = row![
            label(key.title(), key.description()),
            segmented(key, app.settings.get(key))
        ]
        .spacing(16)
        .align_y(Alignment::Center);
        body = body.push(
            container(r)
                .padding([8, 12])
                .width(Fill)
                .style(move |th: &Theme| theme::selectable_row(th, selected)),
        );
    }

    let demo = crate::app::demo_mode();
    let path = if demo {
        "Changes are cleared when Meridian closes".to_string()
    } else {
        data_path().display().to_string()
    };
    body = body.push(caption("DATA"));
    body = body.push(
        container(
            row![
                column![
                    text(if demo { "Demo data" } else { "Database" })
                        .size(13)
                        .font(icons::MEDIUM),
                    text(path).size(12).style(faint),
                ]
                .spacing(2)
                .width(Fill),
                button(
                    row![icon(icons::CLIPBOARD_COPY).size(13), text("Copy path").size(12)]
                        .spacing(6)
                        .align_y(Alignment::Center)
                )
                .padding([5, 10])
                .style(theme::chip_button)
                .on_press_maybe((!demo).then_some(Message::CopyDbPath)),
            ]
            .spacing(16)
            .align_y(Alignment::Center),
        )
        .padding([8, 12])
        .width(Fill),
    );

    let hints = row![
        kbd("j / k"),
        text("select").size(11).style(faint),
        kbd("h / l"),
        text("change").size(11).style(faint),
        kbd("1–3"),
        text("pick").size(11).style(faint),
        kbd("Esc"),
        text("close").size(11).style(faint),
    ]
    .spacing(6)
    .align_y(Alignment::Center);

    let content = column![
        row![
            icon(icons::SETTINGS).size(18).style(muted),
            text("Settings").size(17).font(icons::SEMIBOLD),
            space::horizontal(),
            kbd("Ctrl+,"),
        ]
        .spacing(10)
        .align_y(Alignment::Center),
        scrollable(body).style(theme::scroll).height(iced::Length::Shrink),
        hints,
    ]
    .spacing(14)
    .padding(Padding {
        top: 22.0,
        bottom: 20.0,
        left: 18.0,
        right: 18.0,
    });

    super::modal(
        container(content).max_width(680).style(theme::modal).into(),
        Message::SettingsClose,
        56.0,
    )
}
