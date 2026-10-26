//! Lucide icon codepoints and font constants.
#![allow(dead_code)]

use iced::Font;
use iced::font::Weight;
use iced::widget::{Text, text};

pub const INTER: Font = Font::with_name("Inter");
pub const MEDIUM: Font = Font { weight: Weight::Medium, ..INTER };
pub const SEMIBOLD: Font = Font { weight: Weight::Semibold, ..INTER };
pub const LUCIDE: Font = Font::with_name("lucide");

pub fn icon<'a>(ch: char) -> Text<'a> {
    text(ch.to_string()).font(LUCIDE).size(15).line_height(1.0)
}

pub const PLUS: char = '\u{e13d}';
pub const PANEL_RIGHT: char = '\u{e431}';
pub const PENCIL: char = '\u{e1f9}';
pub const CIRCLE_CHECK: char = '\u{e226}';
pub const CIRCLE: char = '\u{e076}';
pub const FLAG: char = '\u{e0d1}';
pub const TAG: char = '\u{e17f}';
pub const CALENDAR: char = '\u{e063}';
pub const CALENDAR_X: char = '\u{e2be}';
pub const COPY: char = '\u{e09e}';
pub const TRASH: char = '\u{e18e}';
pub const ARROW_RIGHT_LEFT: char = '\u{e417}';
pub const ARROW_LEFT: char = '\u{e048}';
pub const ARROW_RIGHT: char = '\u{e049}';
pub const ARROW_UP: char = '\u{e04a}';
pub const ARROW_DOWN: char = '\u{e042}';
pub const ARROW_UP_TO_LINE: char = '\u{e45b}';
pub const ARROW_DOWN_TO_LINE: char = '\u{e455}';
pub const COLUMNS: char = '\u{e099}';
pub const CHEVRONS_LEFT_RIGHT: char = '\u{e293}';
pub const KANBAN: char = '\u{e170}';
pub const LAYOUT: char = '\u{e1c1}';
pub const CHEVRON_RIGHT: char = '\u{e06f}';
pub const CHEVRON_LEFT: char = '\u{e06e}';
pub const CHEVRON_DOWN: char = '\u{e06d}';
pub const PALETTE: char = '\u{e1dd}';
pub const SEARCH: char = '\u{e151}';
pub const X: char = '\u{e1b2}';
pub const PANEL_LEFT: char = '\u{e12a}';
pub const SUN_MOON: char = '\u{e2b2}';
pub const MONITOR: char = '\u{e11d}';
pub const MOON: char = '\u{e11e}';
pub const SUN: char = '\u{e178}';
pub const COMMAND: char = '\u{e09a}';
pub const UNDO: char = '\u{e2a1}';
pub const REDO: char = '\u{e2a0}';
pub const KEYBOARD: char = '\u{e284}';
pub const LOG_OUT: char = '\u{e10e}';
pub const GRIP: char = '\u{e0eb}';
pub const MORE: char = '\u{e0b6}';
pub const TEXT: char = '\u{e185}';
pub const LIST_CHECKS: char = '\u{e1d0}';
pub const CHECK: char = '\u{e06c}';
pub const CLOCK: char = '\u{e087}';
pub const HASH: char = '\u{e0ef}';
pub const INBOX: char = '\u{e0f7}';
pub const ALERT: char = '\u{e077}';
pub const SPARKLES: char = '\u{e412}';
pub const ENTER: char = '\u{e0a1}';
pub const FILTER: char = '\u{e460}';
pub const SQUARE: char = '\u{e167}';
pub const SQUARE_CHECK: char = '\u{e559}';
pub const LAYERS: char = '\u{e529}';
pub const GAUGE: char = '\u{e1bf}';
pub const MINUS: char = '\u{e11c}';
pub const PERCENT: char = '\u{e132}';
pub const SETTINGS: char = '\u{e154}';
pub const DATABASE: char = '\u{e0ad}';
pub const CLIPBOARD_COPY: char = '\u{e225}';
