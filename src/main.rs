//! Meridian — a fast, keyboard-first, offline Kanban board.

mod app;
mod commands;
mod dates;
mod db;
mod devtools;
mod filter;
mod model;
mod palette;
mod settings;
mod theme;
mod ui;

use app::App;

fn main() -> iced::Result {
    iced::application(App::boot, App::update, ui::view)
        .title(App::title)
        .theme(App::theme)
        .subscription(App::subscription)
        .font(include_bytes!("../assets/fonts/Inter-Regular.ttf").as_slice())
        .font(include_bytes!("../assets/fonts/Inter-Medium.ttf").as_slice())
        .font(include_bytes!("../assets/fonts/Inter-SemiBold.ttf").as_slice())
        .font(include_bytes!("../assets/fonts/lucide.ttf").as_slice())
        .default_font(ui::icons::INTER)
        .antialiasing(true)
        .window(iced::window::Settings {
            size: iced::Size::new(1360.0, 860.0),
            min_size: Some(iced::Size::new(720.0, 480.0)),
            exit_on_close_request: false,
            platform_specific: iced::window::settings::PlatformSpecific {
                application_id: "meridian".into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .run()
}
