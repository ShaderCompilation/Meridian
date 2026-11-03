//! Views.

pub mod area;
pub mod icons;

mod board;
mod card;
mod detail;
mod help;
mod palette;
mod settings;
mod sidebar;
mod widgets;

pub use palette::{ITEM_H as PALETTE_ITEM_H, LIST_MAX_H as PALETTE_LIST_MAX_H};

use iced::widget::{Space, button, column, container, pin, row, stack, text};
use iced::{Alignment, Element, Fill, Padding};

use crate::app::{App, DragItem, Message};
use crate::theme;

pub fn view(app: &App) -> Element<'_, Message> {
    let main = column![board::header(app), board::board(app), board::footer(app)]
        .width(Fill)
        .height(Fill);

    let mut layout = row![];
    if app.sidebar {
        layout = layout.push(sidebar::view(app));
    }
    layout = layout.push(main);
    if let Some(d) = &app.detail {
        layout = layout.push(detail::view(app, d));
    }

    let base = container(layout).style(theme::app_bg).width(Fill).height(Fill);
    let mut layers = stack![base].width(Fill).height(Fill);

    // Drag ghost and drop indicator.
    if let Some(d) = app.drag.as_ref().filter(|d| d.active) {
        if let Some(r) = app.drop_indicator() {
            layers = layers.push(
                pin(container(Space::new())
                    .width(r.width)
                    .height(r.height)
                    .style(theme::drop_line))
                .position(r.position()),
            );
        }
        let ghost: Option<Element<'_, Message>> = match d.item {
            DragItem::Task(id) => app.store.task(id).map(|t| card::ghost(app, t, d.size.width)),
            DragItem::Section(id) => app.store.section(id).map(board::section_ghost),
        };
        if let Some(g) = ghost {
            let p = iced::Point::new(app.cursor.x - d.grab.x, app.cursor.y - d.grab.y);
            layers = layers.push(pin(g).position(p));
        }
    }

    if let Some(t) = &app.toast {
        let mut content = row![text(&t.text).size(13)].spacing(12).align_y(Alignment::Center);
        if t.undo {
            content = content.push(
                button(
                    row![text("Undo").size(13).font(icons::SEMIBOLD), widgets::kbd("u")]
                        .spacing(8)
                        .align_y(Alignment::Center),
                )
                .padding([4, 8])
                .style(theme::ghost_button)
                .on_press(Message::ToastUndo),
            );
        }
        layers = layers.push(
            container(container(content).padding(Padding::from([10, 16])).style(theme::toast))
                .width(Fill)
                .height(Fill)
                .align_x(Alignment::Center)
                .align_y(Alignment::End)
                .padding(Padding {
                    bottom: 48.0,
                    ..Padding::ZERO
                }),
        );
    }

    if let Some(p) = &app.palette {
        layers = layers.push(palette::view(app, p));
    }
    if app.help {
        layers = layers.push(help::view());
    }
    if app.settings_open {
        layers = layers.push(settings::view(app));
    }

    layers.into()
}

/// A full-window dimmed backdrop that closes on click, holding centered content.
pub fn modal<'a>(content: Element<'a, Message>, on_close: Message, top: f32) -> Element<'a, Message> {
    let inner = iced::widget::mouse_area(content).on_press(Message::Noop);
    iced::widget::opaque(
        iced::widget::mouse_area(
            container(inner)
                .width(Fill)
                .height(Fill)
                .align_x(Alignment::Center)
                .padding(Padding {
                    top,
                    left: 16.0,
                    right: 16.0,
                    bottom: 16.0,
                })
                .style(theme::scrim),
        )
        .on_press(on_close),
    )
}
