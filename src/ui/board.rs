//! Header, board columns and status bar.

use iced::widget::scrollable::{Direction, Scrollbar};
use iced::widget::{button, column, container, row, scrollable, space, text, text_input};
use iced::{Alignment, Element, Fill, Length, Padding, Theme, mouse};

use super::area::area;
use super::card;
use super::icons::{self, icon};
use super::widgets::{self, faint, icon_button, kbd, muted};
use crate::app::{
    self, App, BOARD_PAD, BOARD_SCROLL, COL_COLLAPSED_W, COL_GAP, COL_W, DragItem, Edit, FILTER_ID, Field, INLINE,
    Message,
};
use crate::commands::Cmd;
use crate::model::Section;
use crate::theme::{self, tokens};

pub fn header(app: &App) -> Element<'_, Message> {
    let mut r = row![].spacing(8).align_y(Alignment::Center);
    if !app.sidebar {
        r = r.push(icon_button(icons::PANEL_LEFT, Message::Cmd(Cmd::ToggleSidebar), "Show sidebar  (Ctrl+B)"));
    }

    let board = app.board.and_then(|b| app.store.board(b));
    let title: Element<'_, Message> = match (&app.edit, board) {
        (Some(Edit::Board { value, .. }), _) => text_input("Board name", value)
            .id(INLINE)
            .on_input(Message::EditInput)
            .on_submit(Message::EditSubmit)
            .size(19)
            .font(icons::SEMIBOLD)
            .padding([4, 8])
            .style(theme::title_input)
            .width(360)
            .into(),
        (_, Some(b)) => area(text(&b.name).size(19).font(icons::SEMIBOLD))
            .on_double_click(Message::Cmd(Cmd::RenameBoard))
            .into(),
        (_, None) => text("Meridian").size(19).font(icons::SEMIBOLD).into(),
    };
    r = r.push(title);
    if let Some(b) = board {
        let n = app.store.task_count(b.id, app.settings.show_subtasks);
        r = r.push(text(format!("{n} task{}", if n == 1 { "" } else { "s" })).size(13).style(faint));
    }
    r = r.push(space::horizontal());

    // Filter
    let focused = app.filter_focused();
    let mut filter = row![
        icon(icons::SEARCH).size(14).style(faint),
        text_input("Filter   #label  !high  @today", &app.filter_text)
            .id(FILTER_ID)
            .on_input(Message::FilterInput)
            .on_submit(Message::FilterSubmit)
            .size(13)
            .padding([7, 0])
            .style(theme::bare_input),
    ]
    .spacing(8)
    .align_y(Alignment::Center);
    filter = if app.filter_text.is_empty() {
        filter.push(kbd("/"))
    } else {
        filter.push(widgets::small_icon_button(icons::X, Message::FilterClear))
    };
    let compact = app.detail.is_some() || app.window.width < 1100.0;
    r = r.push(
        container(filter)
            .padding([0, 10])
            .width(if compact { 220 } else { 300 })
            .style(move |th: &Theme| theme::input_shell(th, focused)),
    );

    let mut cmd_label = row![icon(icons::COMMAND).size(14)].spacing(8).align_y(Alignment::Center);
    if !compact {
        cmd_label = cmd_label.push(text("Commands").size(13));
    }
    cmd_label = cmd_label.push(kbd("Ctrl+K"));
    r = r.push(
        button(cmd_label)
        .padding([6, 10])
        .style(theme::chip_button)
        .on_press(Message::Cmd(Cmd::Palette)),
    );
    r = r.push(icon_button(
        if app.is_dark() { icons::SUN } else { icons::MOON },
        Message::Cmd(Cmd::ToggleTheme),
        "Toggle theme  (Ctrl+Shift+L)",
    ));
    r = r.push(icon_button(icons::KEYBOARD, Message::Cmd(Cmd::Help), "Keyboard shortcuts  (?)"));

    container(r).padding(Padding { top: 14.0, bottom: 6.0, left: BOARD_PAD, right: BOARD_PAD - 6.0 }).into()
}

pub fn board(app: &App) -> Element<'_, Message> {
    if app.board.is_none() {
        return container(
            column![
                icon(icons::KANBAN).size(44).style(faint),
                text("No boards yet").size(18).font(icons::SEMIBOLD),
                text("Boards hold sections, sections hold tasks.").size(13).style(muted),
                button(row![icon(icons::PLUS).size(14), text("Create a board").size(14)].spacing(8).align_y(Alignment::Center))
                    .padding([8, 14])
                    .style(theme::primary_button)
                    .on_press(Message::Cmd(Cmd::NewBoard)),
            ]
            .spacing(12)
            .align_x(Alignment::Center),
        )
        .center(Fill)
        .into();
    }

    let secs = app.board_sections();
    let mut r = row![].spacing(COL_GAP).height(Fill);
    let new_section_after = match &app.edit {
        Some(Edit::NewSection { after, .. }) => Some(*after),
        _ => None,
    };
    for s in &secs {
        r = r.push(section_column(app, s));
        if new_section_after == Some(Some(s.id)) {
            r = r.push(new_section_column(app));
        }
    }
    if new_section_after == Some(None) {
        r = r.push(new_section_column(app));
    }
    if new_section_after.is_none() {
        r = r.push(
            button(
                row![icon(icons::PLUS).size(14), text("Add section").size(13)]
                    .spacing(8)
                    .align_y(Alignment::Center),
            )
            .width(COL_W)
            .padding([10, 14])
            .style(theme::ghost_button)
            .on_press(Message::AddSection),
        );
    }

    let content = container(r).padding(Padding { top: 8.0, bottom: BOARD_PAD, left: BOARD_PAD, right: BOARD_PAD }).height(Fill);
    area(
        scrollable(area(content).on_bounds(Message::BoardContent))
            .direction(Direction::Horizontal(Scrollbar::new().width(6).scroller_width(6).margin(3)))
            .id(BOARD_SCROLL)
            .on_scroll(|v| Message::BoardScrolled(v.absolute_offset().x, v.bounds().width))
            .style(theme::scroll)
            .width(Fill)
            .height(Fill),
    )
    .on_bounds(Message::BoardView)
    .into()
}

fn new_section_column(app: &App) -> Element<'_, Message> {
    let value = app.edit.as_ref().map(|e| e.value()).unwrap_or("");
    container(
        column![
            text_input("Section name…", value)
                .id(INLINE)
                .on_input(Message::EditInput)
                .on_submit(Message::EditSubmit)
                .size(14)
                .font(icons::SEMIBOLD)
                .padding([6, 8])
                .style(theme::field_input),
            text("Enter to add · Esc to cancel").size(11).style(faint),
        ]
        .spacing(8),
    )
    .padding(10)
    .width(COL_W)
    .style(|th: &Theme| theme::column(th, true, false))
    .into()
}

fn section_column<'a>(app: &'a App, s: &'a Section) -> Element<'a, Message> {
    let id = s.id;
    let selected_header = app.sel.section == Some(id) && app.sel.task.is_none();
    let in_sel = app.sel.section == Some(id);
    let drop_here = app.drop_section() == Some(id);
    let dragging_me = app.drag.as_ref().is_some_and(|d| d.active && d.item == DragItem::Section(id));
    let total = app.store.tasks_of(id).filter(|t| app.on_board(t)).count();

    if s.collapsed {
        let letters = column(
            s.name
                .chars()
                .take(14)
                .map(|c| text(c.to_string()).size(13).font(icons::SEMIBOLD).into())
                .collect::<Vec<Element<'_, Message>>>(),
        )
        .spacing(0)
        .align_x(Alignment::Center);
        let content = column![
            icon(icons::CHEVRONS_LEFT_RIGHT).size(14).style(muted),
            text(total.to_string()).size(12).style(faint),
            letters,
        ]
        .spacing(10)
        .align_x(Alignment::Center)
        .width(Fill);
        return area(container(content).padding([12, 4]).width(COL_COLLAPSED_W).height(Fill))
            .style(move |th, hovered| {
                let mut st = theme::column(th, in_sel, drop_here);
                if hovered || selected_header {
                    st.background = Some(theme::alpha(tokens(th).accent, 0.10).into());
                }
                st
            })
            .on_press(move |p| Message::PressSection(id, p))
            .on_double_click(Message::ToggleCollapse(id))
            .on_bounds(move |r| Message::ColumnBounds(id, r))
            .interaction(mouse::Interaction::Pointer)
            .into();
    }

    // Header
    let name: Element<'_, Message> = match &app.edit {
        Some(Edit::Section { id: eid, value }) if *eid == id => text_input("Section name", value)
            .id(INLINE)
            .on_input(Message::EditInput)
            .on_submit(Message::EditSubmit)
            .size(14)
            .font(icons::SEMIBOLD)
            .padding([2, 4])
            .style(theme::field_input)
            .into(),
        _ => text(&s.name).size(14).font(icons::SEMIBOLD).into(),
    };
    let visible = app.visible_tasks(s);
    let count = if app.filter.is_empty() { total.to_string() } else { format!("{}/{}", visible.len(), total) };
    let header_row = row![
        name,
        container(text(count).size(11).font(icons::MEDIUM).style(muted))
            .padding([2, 7])
            .style(|th: &Theme| theme::pill(tokens(th).muted, tokens(th).dark)),
        space::horizontal(),
        widgets::small_icon_button(icons::PLUS, Message::AddTaskAt(id)),
        widgets::small_icon_button(icons::CHEVRONS_LEFT_RIGHT, Message::ToggleCollapse(id)),
    ]
    .spacing(8)
    .align_y(Alignment::Center);
    let header = area(container(header_row).padding([6, 6]).width(Fill))
        .style(move |th, hovered| theme::section_header(th, hovered, selected_header))
        .on_press(move |p| Message::PressSection(id, p))
        .on_double_click(Message::RenameSectionMouse(id))
        .interaction(mouse::Interaction::Grab);

    // Cards
    let new_before = match &app.edit {
        Some(Edit::NewTask { section, before, value }) if *section == id => Some((*before, value.as_str())),
        _ => None,
    };
    let mut list = column![].spacing(8).padding(Padding { top: 2.0, bottom: 6.0, left: 2.0, right: 10.0 });
    for t in &visible {
        if let Some((Some(b), v)) = new_before
            && b == t.id
        {
            list = list.push(new_task_input(v));
        }
        list = list.push(card::view(app, t, app.sel.task == Some(t.id)));
    }
    if let Some((None, v)) = new_before {
        list = list.push(new_task_input(v));
    }
    if visible.is_empty() && new_before.is_none() {
        let msg = if total > 0 { "No matching tasks" } else { "No tasks yet" };
        list = list.push(
            container(text(msg).size(12).style(faint))
                .width(Fill)
                .padding([18, 0])
                .align_x(Alignment::Center),
        );
    }

    let cards = area(
        scrollable(area(list).on_bounds(move |r| Message::SectionContent(id, r)))
            .id(app::sec_scroll_id(id))
            .direction(Direction::Vertical(Scrollbar::new().width(5).scroller_width(5).margin(0)))
            .on_scroll(move |v| Message::SectionScrolled(id, v.absolute_offset().y))
            .style(theme::scroll)
            .height(Length::Fill),
    )
    .on_bounds(move |r| Message::SectionView(id, r));

    let add = button(
        row![icon(icons::PLUS).size(13), text("Add task").size(13)]
            .spacing(8)
            .align_y(Alignment::Center),
    )
    .width(Fill)
    .padding([7, 8])
    .style(theme::ghost_button)
    .on_press(Message::AddTaskAt(id));

    let content = column![header, cards, add].spacing(6).height(Fill);
    area(container(content).padding(Padding { top: 8.0, bottom: 8.0, left: 8.0, right: 0.0 }).width(COL_W).height(Fill))
        .style(move |th, _| {
            let mut st = theme::column(th, in_sel, drop_here);
            if dragging_me {
                st.background = Some(theme::alpha(tokens(th).accent, 0.06).into());
            }
            st
        })
        .on_bounds(move |r| Message::ColumnBounds(id, r))
        .into()
}

fn new_task_input(value: &str) -> Element<'_, Message> {
    container(
        text_input("Task title…   Enter to add, Esc to stop", value)
            .id(INLINE)
            .on_input(Message::EditInput)
            .on_submit(Message::EditSubmit)
            .size(14)
            .padding(0)
            .style(theme::bare_input),
    )
    .padding([12, 12])
    .width(Fill)
    .style(|th: &Theme| theme::card(th, false, true, false))
    .into()
}

pub fn section_ghost(s: &Section) -> Element<'_, Message> {
    container(
        row![icon(icons::GRIP).size(14).style(muted), text(&s.name).size(14).font(icons::SEMIBOLD)]
            .spacing(8)
            .align_y(Alignment::Center),
    )
    .padding([12, 14])
    .width(COL_W)
    .style(theme::dragging_card)
    .into()
}

pub fn footer(app: &App) -> Element<'_, Message> {
    let mut r = row![].spacing(14).align_y(Alignment::Center);
    for (k, label) in hints(app) {
        r = r.push(row![kbd(k), text(label).size(11).style(faint)].spacing(5).align_y(Alignment::Center));
    }
    r = r.push(space::horizontal());
    if let Some(c) = app.chord {
        r = r.push(
            container(text(format!("{c} …")).size(12).font(icons::SEMIBOLD).style(widgets::accent))
                .padding([2, 8])
                .style(|th: &Theme| theme::pill(tokens(th).accent, tokens(th).dark)),
        );
    }
    if !app.filter.is_empty() {
        r = r.push(
            row![icon(icons::FILTER).size(12).style(widgets::accent), text("Filtered").size(11).style(widgets::accent)]
                .spacing(5)
                .align_y(Alignment::Center),
        );
    }
    if app.detail.is_none() {
        r = r.push(
            row![icon(icons::CHECK).size(11).style(faint), text("Saved locally").size(11).style(faint)]
                .spacing(5)
                .align_y(Alignment::Center),
        );
    }
    container(r)
        .padding([6, BOARD_PAD as u16])
        .width(Fill)
        .style(|th: &Theme| iced::widget::container::Style {
            border: iced::Border { color: tokens(th).border, width: 0.0, radius: 0.0.into() },
            ..Default::default()
        })
        .into()
}

/// Status-bar key hints for whatever has focus right now.
fn hints(app: &App) -> Vec<(&'static str, &'static str)> {
    if app.palette.is_some() {
        return vec![("↑↓", "navigate"), ("Enter", "select"), ("Esc", "close")];
    }
    match app.focus {
        Some(Field::Inline) => {
            return match app.edit {
                Some(Edit::NewTask { .. }) => vec![("Enter", "add & next"), ("Tab", "add & stop"), ("Esc", "cancel")],
                _ => vec![("Enter", "save"), ("Esc", "cancel")],
            };
        }
        Some(Field::Filter) => return vec![("Enter / ↓", "go to board"), ("Esc", "clear filter")],
        Some(Field::Title) => {
            return vec![("Enter", "done"), ("Tab", "next field"), ("Shift+Tab", "previous"), ("Esc", "done")];
        }
        Some(Field::Desc) => {
            return vec![("Ctrl+Enter", "done"), ("Tab", "next field"), ("Shift+Tab", "previous"), ("Esc", "done")];
        }
        Some(Field::SubAdd) => {
            return vec![("Enter", "add"), ("↑", "subtasks"), ("Tab", "next field"), ("Esc", "done")];
        }
        Some(Field::SubEdit) => return vec![("Enter", "save"), ("Esc", "cancel")],
        Some(Field::Palette) | None => {}
    }
    if let Some(d) = app.detail.as_ref().filter(|d| d.focused) {
        let mut v = vec![("↑↓", "subtasks")];
        if d.sub_sel.is_some() {
            v.extend([("Enter", "open"), ("Space", "done"), ("J/K", "reorder")]);
        } else {
            v.extend([("a", "add subtask"), ("i", "description"), ("Tab", "fields")]);
        }
        v.push(("←", "board"));
        let is_sub = app.store.task(d.task).is_some_and(|t| t.parent_id.is_some());
        v.push(("Esc", if d.sub_sel.is_some() { "deselect" } else if is_sub { "back to parent" } else { "close" }));
        return v;
    }
    if app.detail.is_some() {
        return vec![("hjkl", "move, panel follows"), ("Enter / Tab", "into panel"), ("x", "done"), ("Esc", "close panel")];
    }
    let mut v = vec![
        ("hjkl", "move"),
        ("HJKL", "move task"),
        ("Enter", "open"),
        ("n", "new"),
        ("x", "done"),
        ("/", "filter"),
        ("Ctrl+K", "commands"),
        ("?", "help"),
    ];
    if !app.filter_text.is_empty() {
        v.push(("Esc", "clear filter"));
    }
    v
}
