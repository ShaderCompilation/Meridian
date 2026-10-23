//! Board list sidebar.

use iced::widget::{button, column, container, row, scrollable, space, text, text_input};
use iced::{Alignment, Element, Fill, Padding, Theme};

use super::icons::{self, icon};
use super::widgets::{faint, kbd, muted, small_icon_button};
use crate::app::{App, Edit, INLINE, Message, SIDEBAR_W};
use crate::commands::Cmd;
use crate::theme::{self, tokens};

pub fn view(app: &App) -> Element<'_, Message> {
    let logo = row![
        container(icon(icons::KANBAN).size(15).style(|th: &Theme| iced::widget::text::Style {
            color: Some(tokens(th).accent_text)
        }))
        .center(26)
        .style(|th: &Theme| iced::widget::container::Style {
            background: Some(tokens(th).accent.into()),
            border: iced::Border { radius: 7.0.into(), ..Default::default() },
            ..Default::default()
        }),
        text("Meridian").size(16).font(icons::SEMIBOLD),
        space::horizontal(),
        small_icon_button(icons::PANEL_LEFT, Message::Cmd(Cmd::ToggleSidebar)),
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    let caption = row![
        text("BOARDS").size(11).font(icons::SEMIBOLD).style(faint),
        space::horizontal(),
        small_icon_button(icons::PLUS, Message::Cmd(Cmd::NewBoard)),
    ]
    .align_y(Alignment::Center);

    let mut list = column![].spacing(2);
    for b in &app.store.boards {
        let selected = app.board == Some(b.id);
        let n = app.store.task_count(b.id);
        let mut r = row![
            icon(icons::LAYOUT).size(14).style(if selected { super::widgets::accent } else { muted }),
            text(&b.name).size(13).font(if selected { icons::MEDIUM } else { icons::INTER }),
            space::horizontal(),
        ]
        .spacing(10)
        .align_y(Alignment::Center);
        r = r.push(text(n.to_string()).size(11).style(faint));
        list = list.push(
            button(r)
                .width(Fill)
                .padding([7, 10])
                .style(theme::list_item(selected))
                .on_press(Message::SelectBoard(b.id)),
        );
    }
    if let Some(Edit::NewBoard { value }) = &app.edit {
        list = list.push(
            text_input("Board name…", value)
                .id(INLINE)
                .on_input(Message::EditInput)
                .on_submit(Message::EditSubmit)
                .size(13)
                .padding([7, 10])
                .style(theme::field_input),
        );
    } else {
        list = list.push(
            button(
                row![icon(icons::PLUS).size(14), text("New board").size(13)]
                    .spacing(10)
                    .align_y(Alignment::Center),
            )
            .width(Fill)
            .padding([7, 10])
            .style(theme::ghost_button)
            .on_press(Message::Cmd(Cmd::NewBoard)),
        );
    }

    let tip = |k: &str, label: &'static str| {
        row![kbd(k), text(label).size(12).style(faint)].spacing(8).align_y(Alignment::Center)
    };
    let tips = column![
        tip("Ctrl+K", "Command palette"),
        tip("Ctrl+P", "Go to task"),
        tip("?", "All shortcuts"),
    ]
    .spacing(8);

    let content = column![
        logo,
        column![caption, scrollable(list).style(theme::scroll).height(Fill)].spacing(6).height(Fill),
        tips,
    ]
    .spacing(22)
    .padding(Padding { top: 16.0, bottom: 16.0, left: 14.0, right: 12.0 });

    row![
        container(content).width(SIDEBAR_W - 1.0).height(Fill).style(theme::sidebar),
        container(space::horizontal()).width(1).height(Fill).style(theme::divider),
    ]
    .into()
}
