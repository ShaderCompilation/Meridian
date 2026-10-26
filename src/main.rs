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
    // iced asks for the most powerful GPU by default, which on hybrid laptops wakes
    // the discrete card (and on some drivers fails outright). A Kanban board is happy
    // on the integrated GPU. Setting WGPU_POWER_PREF yourself still overrides this.
    if std::env::var_os("WGPU_POWER_PREF").is_none() {
        // SAFETY: runs first thing in `main`, before any other thread exists.
        unsafe { std::env::set_var("WGPU_POWER_PREF", "low") };
    }

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
