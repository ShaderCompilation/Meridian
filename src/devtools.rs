//! Scripted UI driving for development: `MERIDIAN_SCRIPT="j;j;Enter;shot:detail"`
//! replays key presses / commands and saves raw window screenshots
//! (`<name>.<w>x<h>.rgba`) into `MERIDIAN_SHOTS` (default: current dir).
//! Inactive unless the environment variable is set.

use std::time::Duration;

use iced::keyboard::{self, Key, key};

use crate::app::{Act, App, KeyPress, Message};

#[derive(Debug, Clone)]
pub enum Step {
    Key(KeyPress),
    Type(String),
    Wait(u64),
    Shot(String),
    Mouse(f32, f32),
    /// Press on the selected task (or section header) as if with the mouse.
    Grab,
    Release,
    Quit,
}

pub fn script() -> Option<Vec<Step>> {
    let s = std::env::var("MERIDIAN_SCRIPT").ok()?;
    Some(s.split(';').filter(|t| !t.is_empty()).map(parse).collect())
}

fn parse(tok: &str) -> Step {
    if let Some(t) = tok.strip_prefix("type:") {
        return Step::Type(t.to_string());
    }
    if let Some(t) = tok.strip_prefix("wait:") {
        return Step::Wait(t.parse().unwrap_or(200));
    }
    if let Some(t) = tok.strip_prefix("shot:") {
        return Step::Shot(t.to_string());
    }
    if let Some(t) = tok.strip_prefix("mouse:") {
        let (x, y) = t.split_once(',').unwrap_or(("0", "0"));
        return Step::Mouse(x.parse().unwrap_or(0.0), y.parse().unwrap_or(0.0));
    }
    match tok {
        "quit" => return Step::Quit,
        "grab" => return Step::Grab,
        "release" => return Step::Release,
        _ => {}
    }
    let mut mods = keyboard::Modifiers::empty();
    let mut rest = tok;
    loop {
        if let Some(r) = rest.strip_prefix("C-") {
            mods |= keyboard::Modifiers::CTRL;
            rest = r;
        } else if let Some(r) = rest.strip_prefix("S-") {
            mods |= keyboard::Modifiers::SHIFT;
            rest = r;
        } else {
            break;
        }
    }
    let named = match rest {
        "Enter" => Some(key::Named::Enter),
        "Esc" => Some(key::Named::Escape),
        "Tab" => Some(key::Named::Tab),
        "Space" => Some(key::Named::Space),
        "Up" => Some(key::Named::ArrowUp),
        "Down" => Some(key::Named::ArrowDown),
        "Left" => Some(key::Named::ArrowLeft),
        "Right" => Some(key::Named::ArrowRight),
        "Del" => Some(key::Named::Delete),
        "PgUp" => Some(key::Named::PageUp),
        "PgDn" => Some(key::Named::PageDown),
        _ => None,
    };
    let (k, modified) = match named {
        Some(n) => (Key::Named(n), Key::Named(n)),
        None => {
            if rest.chars().any(|c| c.is_uppercase()) {
                mods |= keyboard::Modifiers::SHIFT;
            }
            (Key::Character(rest.to_lowercase().into()), Key::Character(rest.into()))
        }
    };
    Step::Key(KeyPress {
        key: k,
        modified,
        physical: key::Physical::Unidentified(key::NativeCode::Unidentified),
        mods,
        captured: false,
    })
}

fn after(ms: u64, msg: Message) -> Act {
    Act::perform(
        async move {
            std::thread::sleep(Duration::from_millis(ms));
        },
        move |_| msg.clone(),
    )
}

pub fn start() -> Act {
    if script().is_some() {
        // Keep the window on top so it keeps rendering while the script runs.
        let top = iced::window::latest().and_then(|id| {
            Act::batch([
                iced::window::set_level(id, iced::window::Level::AlwaysOnTop),
                iced::window::gain_focus(id),
            ])
        });
        Act::batch([top, after(900, Message::Script(0))])
    } else {
        Act::none()
    }
}

pub fn run_step(app: &mut App, i: usize) -> Act {
    let Some(steps) = script() else { return Act::none() };
    let Some(step) = steps.get(i).cloned() else { return Act::none() };
    if std::env::var("MERIDIAN_SCRIPT_LOG").is_ok() {
        eprintln!("step {i}: {step:?}");
    }
    let next = after(140, Message::Script(i + 1));
    match step {
        Step::Key(k) => {
            let a = app.update(Message::Key(k));
            Act::batch([a, next])
        }
        Step::Type(t) => {
            let msg = if app.palette.is_some() {
                Message::PaletteInput(t)
            } else if app.edit.is_some() {
                Message::EditInput(t)
            } else if app.detail.is_some() {
                Message::DetailSubInput(t)
            } else {
                Message::FilterInput(t)
            };
            let a = app.update(msg);
            Act::batch([a, next])
        }
        Step::Wait(ms) => after(ms, Message::Script(i + 1)),
        Step::Mouse(x, y) => {
            let a = app.update(Message::CursorMoved(iced::Point::new(x, y)));
            Act::batch([a, next])
        }
        Step::Grab => {
            let msg = match (app.sel.task, app.sel.section) {
                (Some(t), _) => Message::PressTask(t, iced::Point::new(30.0, 20.0)),
                (None, Some(s)) => Message::PressSection(s, iced::Point::new(30.0, 12.0)),
                _ => Message::Noop,
            };
            let a = app.update(msg);
            Act::batch([a, next])
        }
        Step::Release => {
            let a = app.update(Message::MouseReleased);
            Act::batch([a, next])
        }
        Step::Shot(name) => iced::window::latest()
            .and_then(iced::window::screenshot)
            .map(move |s| Message::Shot(name.clone(), s, i + 1)),
        Step::Quit => iced::exit(),
    }
}

pub fn save(name: &str, shot: &iced::window::Screenshot) {
    let dir = std::env::var("MERIDIAN_SHOTS").unwrap_or_else(|_| ".".into());
    let path = format!("{dir}/{name}.{}x{}.rgba", shot.size.width, shot.size.height);
    if let Err(e) = std::fs::write(&path, &shot.rgba) {
        eprintln!("screenshot {path}: {e}");
    }
}

pub fn continue_after_shot(i: usize) -> Act {
    after(60, Message::Script(i))
}
