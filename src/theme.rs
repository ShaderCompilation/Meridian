//! Design tokens and widget styles for the dark and light themes.

use iced::border::Radius;
use iced::widget::{button, checkbox, container, progress_bar, scrollable, text_editor, text_input};
use iced::{Background, Border, Color, Shadow, Theme, Vector, color};

pub struct Tokens {
    pub dark: bool,
    pub bg: Color,
    pub sidebar: Color,
    pub column: Color,
    pub card: Color,
    pub card_hover: Color,
    pub elevated: Color,
    pub border: Color,
    pub border_strong: Color,
    pub text: Color,
    pub muted: Color,
    pub faint: Color,
    pub accent: Color,
    pub accent_text: Color,
    pub success: Color,
    pub warning: Color,
    pub danger: Color,
    pub scrim: Color,
    pub shadow: Color,
}

pub static DARK: Tokens = Tokens {
    dark: true,
    bg: color!(0x0d0f13),
    sidebar: color!(0x111318),
    column: color!(0x15181e),
    card: color!(0x1d2129),
    card_hover: color!(0x232833),
    elevated: color!(0x1a1d24),
    border: color!(0x262a33),
    border_strong: color!(0x343a46),
    text: color!(0xe7e9ee),
    muted: color!(0x9aa1ad),
    faint: color!(0x646b78),
    accent: color!(0x8187ff),
    accent_text: color!(0xffffff),
    success: color!(0x4cc38a),
    warning: color!(0xf2b84b),
    danger: color!(0xf0616d),
    scrim: color!(0x000000, 0.55),
    shadow: color!(0x000000, 0.45),
};

pub static LIGHT: Tokens = Tokens {
    dark: false,
    bg: color!(0xf6f7f9),
    sidebar: color!(0xeef0f3),
    column: color!(0xeceef2),
    card: color!(0xffffff),
    card_hover: color!(0xfafbff),
    elevated: color!(0xffffff),
    border: color!(0xdfe2e8),
    border_strong: color!(0xc9cdd6),
    text: color!(0x1b1e24),
    muted: color!(0x5e6573),
    faint: color!(0x9097a3),
    accent: color!(0x5b61f0),
    accent_text: color!(0xffffff),
    success: color!(0x1f9d63),
    warning: color!(0xc98a0c),
    danger: color!(0xd93a48),
    scrim: color!(0x0b0d12, 0.30),
    shadow: color!(0x1b1e24, 0.14),
};

pub fn tokens(theme: &Theme) -> &'static Tokens {
    if theme.extended_palette().is_dark { &DARK } else { &LIGHT }
}

pub fn make_theme(dark: bool) -> Theme {
    let t = if dark { &DARK } else { &LIGHT };
    Theme::custom(
        if dark { "Meridian Dark" } else { "Meridian Light" },
        iced::theme::Palette {
            background: t.bg,
            text: t.text,
            primary: t.accent,
            success: t.success,
            warning: t.warning,
            danger: t.danger,
        },
    )
}

/// Label colours: (dark-theme tint, light-theme tint).
pub const LABEL_COLORS: [(Color, Color, &str); 8] = [
    (color!(0xff7b7b), color!(0xd23c3c), "Red"),
    (color!(0xffa25c), color!(0xc96512), "Orange"),
    (color!(0xf5cf4f), color!(0x9c7a00), "Yellow"),
    (color!(0x5fd38d), color!(0x1d8a4d), "Green"),
    (color!(0x4fd1c5), color!(0x0f8378), "Teal"),
    (color!(0x6aa9ff), color!(0x2266cc), "Blue"),
    (color!(0xb08cff), color!(0x7246d6), "Violet"),
    (color!(0xff7ac6), color!(0xc23787), "Pink"),
];

pub fn label_color(theme: &Theme, idx: u8) -> Color {
    let (d, l, _) = LABEL_COLORS[idx as usize % LABEL_COLORS.len()];
    if tokens(theme).dark { d } else { l }
}

pub fn priority_color(t: &Tokens, p: crate::model::Priority) -> Option<Color> {
    use crate::model::Priority::*;
    match p {
        None => Option::None,
        Low => Some(if t.dark { color!(0x6aa9ff) } else { color!(0x2266cc) }),
        Medium => Some(t.warning),
        High => Some(if t.dark { color!(0xff9152) } else { color!(0xd2601a) }),
        Urgent => Some(t.danger),
    }
}

pub fn alpha(c: Color, a: f32) -> Color {
    Color { a, ..c }
}

pub const RADIUS: f32 = 10.0;
pub const RADIUS_SM: f32 = 6.0;

fn border(color: Color, width: f32, radius: impl Into<Radius>) -> Border {
    Border { color, width, radius: radius.into() }
}

fn shadow(color: Color, y: f32, blur: f32) -> Shadow {
    Shadow { color, offset: Vector::new(0.0, y), blur_radius: blur }
}

// ---------- containers ----------

pub fn app_bg(theme: &Theme) -> container::Style {
    let t = tokens(theme);
    container::Style {
        background: Some(t.bg.into()),
        text_color: Some(t.text),
        ..Default::default()
    }
}

pub fn sidebar(theme: &Theme) -> container::Style {
    let t = tokens(theme);
    container::Style {
        background: Some(t.sidebar.into()),
        border: border(t.border, 0.0, 0.0),
        ..Default::default()
    }
}

pub fn column(theme: &Theme, selected: bool, drop_target: bool) -> container::Style {
    let t = tokens(theme);
    let edge = if drop_target {
        alpha(t.accent, 0.6)
    } else if selected {
        alpha(t.accent, 0.35)
    } else {
        t.border
    };
    container::Style {
        background: Some(t.column.into()),
        border: border(edge, 1.0, RADIUS + 2.0),
        ..Default::default()
    }
}

pub fn card(theme: &Theme, hovered: bool, selected: bool, ghost: bool) -> container::Style {
    let t = tokens(theme);
    if ghost {
        return container::Style {
            background: Some(alpha(t.accent, 0.06).into()),
            border: border(alpha(t.accent, 0.45), 1.0, RADIUS),
            ..Default::default()
        };
    }
    let bg = if hovered { t.card_hover } else { t.card };
    let (edge, width) = if selected {
        (t.accent, 2.0)
    } else if hovered {
        (t.border_strong, 1.0)
    } else {
        (t.border, 1.0)
    };
    container::Style {
        background: Some(bg.into()),
        border: border(edge, width, RADIUS),
        shadow: if selected || hovered {
            shadow(t.shadow, 2.0, 8.0)
        } else {
            shadow(alpha(t.shadow, t.shadow.a * 0.5), 1.0, 2.0)
        },
        text_color: Some(t.text),
        snap: true,
    }
}

pub fn dragging_card(theme: &Theme) -> container::Style {
    let t = tokens(theme);
    container::Style {
        background: Some(t.card_hover.into()),
        border: border(t.accent, 1.5, RADIUS),
        shadow: shadow(t.shadow, 12.0, 28.0),
        text_color: Some(t.text),
        snap: false,
    }
}

pub fn section_header(theme: &Theme, hovered: bool, selected: bool) -> container::Style {
    let t = tokens(theme);
    container::Style {
        background: if selected {
            Some(alpha(t.accent, 0.14).into())
        } else if hovered {
            Some(alpha(t.text, 0.04).into())
        } else {
            None
        },
        border: border(if selected { alpha(t.accent, 0.8) } else { Color::TRANSPARENT }, 1.0, RADIUS_SM + 2.0),
        text_color: Some(t.text),
        ..Default::default()
    }
}

/// The task panel; its edge lights up while it has the keyboard.
pub fn panel(theme: &Theme, focused: bool) -> container::Style {
    let t = tokens(theme);
    container::Style {
        background: Some(t.elevated.into()),
        border: if focused { border(alpha(t.accent, 0.55), 1.0, 0.0) } else { border(t.border, 1.0, 0.0) },
        shadow: shadow(t.shadow, 0.0, 24.0),
        text_color: Some(t.text),
        ..Default::default()
    }
}

pub fn modal(theme: &Theme) -> container::Style {
    let t = tokens(theme);
    container::Style {
        background: Some(t.elevated.into()),
        border: border(t.border_strong, 1.0, RADIUS + 4.0),
        shadow: shadow(t.shadow, 18.0, 48.0),
        text_color: Some(t.text),
        ..Default::default()
    }
}

pub fn scrim(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(tokens(theme).scrim.into()),
        ..Default::default()
    }
}

pub fn pill(color: Color, dark: bool) -> container::Style {
    container::Style {
        background: Some(alpha(color, if dark { 0.16 } else { 0.12 }).into()),
        border: border(alpha(color, 0.0), 0.0, 999.0),
        text_color: Some(color),
        ..Default::default()
    }
}

pub fn kbd(theme: &Theme) -> container::Style {
    let t = tokens(theme);
    container::Style {
        background: Some(alpha(t.text, 0.06).into()),
        border: border(t.border_strong, 1.0, 4.0),
        text_color: Some(t.muted),
        ..Default::default()
    }
}

pub fn drop_line(theme: &Theme) -> container::Style {
    let t = tokens(theme);
    container::Style {
        background: Some(t.accent.into()),
        border: border(t.accent, 0.0, 2.0),
        shadow: shadow(alpha(t.accent, 0.5), 0.0, 6.0),
        ..Default::default()
    }
}

pub fn toast(theme: &Theme) -> container::Style {
    let t = tokens(theme);
    container::Style {
        background: Some(if t.dark { color!(0x2a2f3a) } else { color!(0x23262d) }.into()),
        border: border(alpha(Color::WHITE, 0.08), 1.0, RADIUS),
        shadow: shadow(t.shadow, 8.0, 24.0),
        text_color: Some(color!(0xf0f2f5)),
        ..Default::default()
    }
}

pub fn divider(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(tokens(theme).border.into()),
        ..Default::default()
    }
}

pub fn input_shell(theme: &Theme, focused: bool) -> container::Style {
    let t = tokens(theme);
    container::Style {
        background: Some(if t.dark { alpha(Color::BLACK, 0.18) } else { t.card }.into()),
        border: border(if focused { t.accent } else { t.border }, 1.0, RADIUS_SM + 2.0),
        ..Default::default()
    }
}

// ---------- buttons ----------

pub fn ghost_button(theme: &Theme, status: button::Status) -> button::Style {
    let t = tokens(theme);
    let bg = match status {
        button::Status::Hovered => Some(alpha(t.text, 0.07).into()),
        button::Status::Pressed => Some(alpha(t.text, 0.11).into()),
        _ => None,
    };
    button::Style {
        background: bg,
        text_color: if matches!(status, button::Status::Disabled) { t.faint } else { t.muted },
        border: border(Color::TRANSPARENT, 0.0, RADIUS_SM),
        shadow: Shadow::default(),
        snap: true,
    }
}

pub fn primary_button(theme: &Theme, status: button::Status) -> button::Style {
    let t = tokens(theme);
    let bg = match status {
        button::Status::Hovered => lighten(t.accent, 0.06),
        button::Status::Pressed => lighten(t.accent, -0.06),
        _ => t.accent,
    };
    button::Style {
        background: Some(bg.into()),
        text_color: t.accent_text,
        border: border(Color::TRANSPARENT, 0.0, RADIUS_SM),
        shadow: shadow(alpha(t.accent, 0.3), 2.0, 8.0),
        snap: true,
    }
}

pub fn chip_button(theme: &Theme, status: button::Status) -> button::Style {
    let t = tokens(theme);
    let bg = match status {
        button::Status::Hovered => alpha(t.text, 0.08),
        button::Status::Pressed => alpha(t.text, 0.12),
        _ => alpha(t.text, 0.04),
    };
    button::Style {
        background: Some(bg.into()),
        text_color: t.muted,
        border: border(t.border, 1.0, RADIUS_SM),
        shadow: Shadow::default(),
        snap: true,
    }
}

/// The track behind a segmented control.
pub fn segment_track(theme: &Theme) -> container::Style {
    let t = tokens(theme);
    container::Style {
        background: Some(alpha(t.text, 0.05).into()),
        border: border(t.border, 1.0, RADIUS_SM + 2.0),
        ..Default::default()
    }
}

/// One option in a segmented control.
pub fn segment(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme, status| {
        let t = tokens(theme);
        let bg = if active {
            Some(if t.dark { t.card_hover } else { t.card }.into())
        } else {
            match status {
                button::Status::Hovered => Some(alpha(t.text, 0.06).into()),
                button::Status::Pressed => Some(alpha(t.text, 0.10).into()),
                _ => None,
            }
        };
        button::Style {
            background: bg,
            text_color: if active { t.text } else { t.muted },
            border: border(if active { t.border_strong } else { Color::TRANSPARENT }, 1.0, RADIUS_SM),
            shadow: Shadow::default(),
            snap: true,
        }
    }
}

/// A row in a keyboard-navigable list, such as the settings panel.
pub fn selectable_row(theme: &Theme, selected: bool) -> container::Style {
    let t = tokens(theme);
    container::Style {
        background: selected.then(|| alpha(t.accent, if t.dark { 0.14 } else { 0.08 }).into()),
        border: border(if selected { alpha(t.accent, 0.5) } else { Color::TRANSPARENT }, 1.0, RADIUS_SM + 2.0),
        ..Default::default()
    }
}

pub fn list_item(selected: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme, status| {
        let t = tokens(theme);
        let bg = if selected {
            Some(alpha(t.accent, if t.dark { 0.20 } else { 0.12 }).into())
        } else {
            match status {
                button::Status::Hovered => Some(alpha(t.text, 0.05).into()),
                button::Status::Pressed => Some(alpha(t.text, 0.09).into()),
                _ => None,
            }
        };
        button::Style {
            background: bg,
            text_color: t.text,
            border: border(Color::TRANSPARENT, 0.0, RADIUS_SM + 1.0),
            shadow: Shadow::default(),
            snap: true,
        }
    }
}

pub fn day_button(
    selected: bool,
    today: bool,
    in_month: bool,
) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme, status| {
        let t = tokens(theme);
        let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
        let bg = if selected {
            Some(t.accent.into())
        } else if hovered {
            Some(alpha(t.text, 0.08).into())
        } else {
            None
        };
        let text_color = if selected {
            t.accent_text
        } else if today {
            t.accent
        } else if in_month {
            t.text
        } else {
            t.faint
        };
        button::Style {
            background: bg,
            text_color,
            border: border(if today && !selected { alpha(t.accent, 0.6) } else { Color::TRANSPARENT }, 1.0, RADIUS_SM),
            shadow: Shadow::default(),
            snap: true,
        }
    }
}

fn lighten(c: Color, amount: f32) -> Color {
    Color {
        r: (c.r + amount).clamp(0.0, 1.0),
        g: (c.g + amount).clamp(0.0, 1.0),
        b: (c.b + amount).clamp(0.0, 1.0),
        a: c.a,
    }
}

// ---------- inputs ----------

/// Borderless input that blends into its container.
pub fn bare_input(theme: &Theme, _status: text_input::Status) -> text_input::Style {
    let t = tokens(theme);
    text_input::Style {
        background: Background::Color(Color::TRANSPARENT),
        border: border(Color::TRANSPARENT, 0.0, 0.0),
        icon: t.faint,
        placeholder: t.faint,
        value: t.text,
        selection: alpha(t.accent, 0.35),
    }
}

pub fn field_input(theme: &Theme, status: text_input::Status) -> text_input::Style {
    let t = tokens(theme);
    let focused = matches!(status, text_input::Status::Focused { .. });
    text_input::Style {
        background: Background::Color(if t.dark { alpha(Color::BLACK, 0.18) } else { t.card }),
        border: border(if focused { t.accent } else { t.border }, 1.0, RADIUS_SM + 2.0),
        icon: t.faint,
        placeholder: t.faint,
        value: t.text,
        selection: alpha(t.accent, 0.35),
    }
}

pub fn title_input(theme: &Theme, status: text_input::Status) -> text_input::Style {
    let t = tokens(theme);
    let focused = matches!(status, text_input::Status::Focused { .. });
    let hovered = matches!(status, text_input::Status::Hovered | text_input::Status::Focused { is_hovered: true });
    text_input::Style {
        background: Background::Color(if focused || hovered { alpha(t.text, 0.04) } else { Color::TRANSPARENT }),
        border: border(if focused { alpha(t.accent, 0.8) } else { Color::TRANSPARENT }, 1.0, RADIUS_SM),
        icon: t.faint,
        placeholder: t.faint,
        value: t.text,
        selection: alpha(t.accent, 0.35),
    }
}

pub fn editor(theme: &Theme, status: text_editor::Status) -> text_editor::Style {
    let t = tokens(theme);
    let focused = matches!(status, text_editor::Status::Focused { .. });
    text_editor::Style {
        background: Background::Color(if t.dark { alpha(Color::BLACK, 0.18) } else { t.card }),
        border: border(if focused { t.accent } else { t.border }, 1.0, RADIUS_SM + 2.0),
        placeholder: t.faint,
        value: t.text,
        selection: alpha(t.accent, 0.35),
    }
}

pub fn check(theme: &Theme, status: checkbox::Status) -> checkbox::Style {
    let t = tokens(theme);
    let (checked, hovered) = match status {
        checkbox::Status::Active { is_checked } => (is_checked, false),
        checkbox::Status::Hovered { is_checked } => (is_checked, true),
        checkbox::Status::Disabled { is_checked } => (is_checked, false),
    };
    checkbox::Style {
        background: if checked {
            t.accent.into()
        } else if hovered {
            alpha(t.text, 0.06).into()
        } else {
            Color::TRANSPARENT.into()
        },
        icon_color: t.accent_text,
        border: border(if checked { t.accent } else { t.border_strong }, 1.5, 5.0),
        text_color: Some(if checked { t.faint } else { t.text }),
    }
}

pub fn progress(theme: &Theme) -> progress_bar::Style {
    let t = tokens(theme);
    progress_bar::Style {
        background: alpha(t.text, 0.08).into(),
        bar: t.success.into(),
        border: border(Color::TRANSPARENT, 0.0, 999.0),
    }
}

pub fn scroll(theme: &Theme, status: scrollable::Status) -> scrollable::Style {
    let t = tokens(theme);
    let mut s = scrollable::default(theme, status);
    let (hv, hh) = match status {
        scrollable::Status::Hovered { is_vertical_scrollbar_hovered, is_horizontal_scrollbar_hovered, .. } => {
            (is_vertical_scrollbar_hovered, is_horizontal_scrollbar_hovered)
        }
        scrollable::Status::Dragged { is_vertical_scrollbar_dragged, is_horizontal_scrollbar_dragged, .. } => {
            (is_vertical_scrollbar_dragged, is_horizontal_scrollbar_dragged)
        }
        _ => (false, false),
    };
    let rail = |active: bool| scrollable::Rail {
        background: None,
        border: border(Color::TRANSPARENT, 0.0, 999.0),
        scroller: scrollable::Scroller {
            background: alpha(t.text, if active { 0.30 } else { 0.14 }).into(),
            border: border(Color::TRANSPARENT, 0.0, 999.0),
        },
    };
    s.container = container::Style::default();
    s.vertical_rail = rail(hv);
    s.horizontal_rail = rail(hh);
    s.gap = None;
    s
}
