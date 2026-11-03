//! Small reusable view helpers.

use iced::widget::{button, container, row, text, tooltip};
use iced::{Alignment, Color, Element, Theme};

use super::icons::{self, icon};
use crate::app::Message;
use crate::theme::{self, tokens};

pub fn muted(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(tokens(theme).muted),
    }
}

pub fn faint(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(tokens(theme).faint),
    }
}

pub fn accent(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(tokens(theme).accent),
    }
}

pub fn colored(c: Color) -> impl Fn(&Theme) -> text::Style {
    move |_| text::Style { color: Some(c) }
}

pub fn kbd<'a>(keys: &str) -> Element<'a, Message> {
    let parts: Vec<Element<'a, Message>> = split_keys(keys)
        .into_iter()
        .map(|k| {
            container(text(k).size(11).font(icons::MEDIUM).line_height(1.0))
                .padding([3, 6])
                .style(theme::kbd)
                .into()
        })
        .collect();
    row(parts).spacing(3).align_y(Alignment::Center).into()
}

/// "Ctrl+Shift+K" -> ["Ctrl", "Shift", "K"]; "dd" -> ["d", "d"]; "gK" -> ["g", "K"].
fn split_keys(keys: &str) -> Vec<String> {
    if keys.contains('+') && keys.len() > 1 {
        return keys.split('+').map(|s| s.to_string()).collect();
    }
    // Only vim-style letter sequences ("dd", "gK", "hjkl") are split into keys.
    if keys.chars().count() > 1 && keys.chars().all(|c| c.is_ascii_alphabetic()) && keys.len() <= 4 {
        let named = ["Enter", "Esc", "Tab", "Space", "Home", "End", "PgUp", "PgDn"];
        if !named.contains(&keys) {
            return keys.chars().map(|c| c.to_string()).collect();
        }
    }
    vec![keys.to_string()]
}

pub fn icon_button<'a>(ch: char, msg: Message, tip: &'a str) -> Element<'a, Message> {
    let b = button(container(icon(ch).size(16)).center(16))
        .padding(7)
        .style(theme::ghost_button)
        .on_press(msg);
    tooltip(
        b,
        container(text(tip).size(12)).padding([5, 9]).style(theme::toast),
        tooltip::Position::Bottom,
    )
    .gap(6)
    .into()
}

pub fn small_icon_button<'a>(ch: char, msg: Message) -> Element<'a, Message> {
    button(container(icon(ch).size(13)).center(14))
        .padding(4)
        .style(theme::ghost_button)
        .on_press(msg)
        .into()
}

pub fn label_pill<'a>(name: &'a str, color: u8) -> Element<'a, Message> {
    container(text(name).size(11).font(icons::MEDIUM).line_height(1.0))
        .padding([4, 8])
        .style(move |th: &Theme| theme::pill(theme::label_color(th, color), tokens(th).dark))
        .into()
}
