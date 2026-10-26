//! Natural-language date input and friendly formatting.

use chrono::{Datelike, Duration, Local, NaiveDate, Weekday};

use crate::settings::{DateStyle, WeekStart};

pub fn today() -> NaiveDate {
    Local::now().date_naive()
}

fn weekday(s: &str) -> Option<Weekday> {
    let w = match s {
        "mon" | "monday" => Weekday::Mon,
        "tue" | "tues" | "tuesday" => Weekday::Tue,
        "wed" | "wednesday" => Weekday::Wed,
        "thu" | "thur" | "thurs" | "thursday" => Weekday::Thu,
        "fri" | "friday" => Weekday::Fri,
        "sat" | "saturday" => Weekday::Sat,
        "sun" | "sunday" => Weekday::Sun,
        _ => return None,
    };
    Some(w)
}

fn month(s: &str) -> Option<u32> {
    const M: [&str; 12] = [
        "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
    ];
    if s.len() < 3 {
        return None;
    }
    M.iter().position(|m| s.starts_with(m)).map(|i| i as u32 + 1)
}

/// Parses things like `today`, `tom`, `+3d`, `2w`, `fri`, `next mon`, `nov 3`,
/// `3 nov`, `2026-11-03`, `11/03`, `3`.
pub fn parse(input: &str, today: NaiveDate, week: WeekStart) -> Option<NaiveDate> {
    let s = input.trim().to_lowercase();
    if s.is_empty() {
        return None;
    }
    match s.as_str() {
        "today" | "tod" | "now" => return Some(today),
        "tomorrow" | "tom" | "tmr" | "tmrw" => return Some(today + Duration::days(1)),
        "yesterday" => return Some(today - Duration::days(1)),
        "next week" | "nw" => return Some(next_week(today, week)),
        "next month" => {
            return today
                .with_day(1)
                .and_then(|d| d.checked_add_months(chrono::Months::new(1)));
        }
        _ => {}
    }

    // Relative: +3, +3d, 3d, 2w, 1m
    let rel = s.strip_prefix('+').unwrap_or(&s);
    if let Some(last) = rel.chars().last() {
        let (num, unit) = if last.is_ascii_digit() {
            (rel, 'd')
        } else {
            (&rel[..rel.len() - last.len_utf8()], last)
        };
        if (s.starts_with('+') || !last.is_ascii_digit())
            && let Ok(n) = num.trim().parse::<i64>()
        {
            match unit {
                'd' => return Some(today + Duration::days(n)),
                'w' => return Some(today + Duration::weeks(n)),
                'm' => {
                    return today.checked_add_months(chrono::Months::new(n.max(0) as u32));
                }
                _ => {}
            }
        }
    }

    // Weekdays: "fri" -> next friday (today counts if it is friday? no: strictly upcoming, unless "this")
    let (skip_week, name) = match s.strip_prefix("next ") {
        Some(rest) => (true, rest),
        None => (false, s.strip_prefix("this ").unwrap_or(&s)),
    };
    if let Some(w) = weekday(name) {
        let cur = today.weekday().num_days_from_monday() as i64;
        let target = w.num_days_from_monday() as i64;
        let mut diff = (target - cur).rem_euclid(7);
        if diff == 0 {
            diff = 7;
        }
        if skip_week && diff < 7 {
            diff += 7;
        }
        return Some(today + Duration::days(diff));
    }

    // ISO
    if let Ok(d) = NaiveDate::parse_from_str(&s, "%Y-%m-%d") {
        return Some(d);
    }

    // "nov 3", "3 nov", "november 3rd"
    let parts: Vec<&str> = s.split(|c: char| c.is_whitespace() || c == ',').filter(|p| !p.is_empty()).collect();
    let num = |p: &str| -> Option<u32> {
        p.trim_end_matches(|c: char| c.is_alphabetic()).parse().ok()
    };
    let upcoming = |m: u32, d: u32, y: Option<i32>| -> Option<NaiveDate> {
        match y {
            Some(y) => NaiveDate::from_ymd_opt(y, m, d),
            None => {
                let this = NaiveDate::from_ymd_opt(today.year(), m, d)?;
                if this < today {
                    NaiveDate::from_ymd_opt(today.year() + 1, m, d)
                } else {
                    Some(this)
                }
            }
        }
    };
    if parts.len() >= 2 {
        let year = parts.get(2).and_then(|p| p.parse::<i32>().ok());
        if let (Some(m), Some(d)) = (month(parts[0]), num(parts[1])) {
            return upcoming(m, d, year);
        }
        if let (Some(d), Some(m)) = (num(parts[0]), month(parts[1])) {
            return upcoming(m, d, year);
        }
    }

    // "11/03" (month/day)
    if let Some((a, b)) = s.split_once('/')
        && let (Ok(m), Ok(d)) = (a.parse::<u32>(), b.parse::<u32>())
    {
        return upcoming(m, d, None);
    }

    // Bare day of month: next occurrence
    if let Ok(d) = s.parse::<u32>()
        && (1..=31).contains(&d)
    {
        let mut date = today.with_day(1)?;
        for _ in 0..3 {
            if let Some(c) = date.with_day(d)
                && c >= today
            {
                return Some(c);
            }
            date = date.checked_add_months(chrono::Months::new(1))?;
        }
    }
    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Urgency {
    Overdue,
    Today,
    Soon,
    Later,
}

pub fn urgency(due: NaiveDate, today: NaiveDate) -> Urgency {
    let d = (due - today).num_days();
    if d < 0 {
        Urgency::Overdue
    } else if d == 0 {
        Urgency::Today
    } else if d <= 2 {
        Urgency::Soon
    } else {
        Urgency::Later
    }
}

/// "Today", "Tomorrow", "Fri", "Nov 3", "Nov 3, 2027".
pub fn short(due: NaiveDate, today: NaiveDate) -> String {
    let d = (due - today).num_days();
    match d {
        0 => "Today".into(),
        1 => "Tomorrow".into(),
        -1 => "Yesterday".into(),
        2..=6 => due.format("%a").to_string(),
        _ if due.year() == today.year() => due.format("%b %-d").to_string(),
        _ => due.format("%b %-d, %Y").to_string(),
    }
}

/// "Nov 3", or "Nov 3, 2027" outside the current year.
pub fn absolute(due: NaiveDate, today: NaiveDate) -> String {
    if due.year() == today.year() {
        due.format("%b %-d").to_string()
    } else {
        due.format("%b %-d, %Y").to_string()
    }
}

/// Due-date label in the user's chosen style.
pub fn label(due: NaiveDate, today: NaiveDate, style: DateStyle) -> String {
    match style {
        DateStyle::Relative => short(due, today),
        DateStyle::Absolute => absolute(due, today),
    }
}

/// Days since the start of the week `d` is in (0 on the first day).
pub fn days_into_week(d: NaiveDate, week: WeekStart) -> i64 {
    match week {
        WeekStart::Monday => d.weekday().num_days_from_monday() as i64,
        WeekStart::Sunday => d.weekday().num_days_from_sunday() as i64,
    }
}

/// The first day of next week.
pub fn next_week(today: NaiveDate, week: WeekStart) -> NaiveDate {
    today + Duration::days(7 - days_into_week(today, week))
}

pub fn long(due: NaiveDate) -> String {
    due.format("%A, %B %-d, %Y").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn parses() {
        // Saturday, Oct 24 2026
        let t = d(2026, 10, 24);
        let p = |s: &str, t| parse(s, t, WeekStart::Monday);
        assert_eq!(t.weekday(), Weekday::Sat);
        assert_eq!(p("today", t), Some(t));
        assert_eq!(p("tom", t), Some(d(2026, 10, 25)));
        assert_eq!(p("+3", t), Some(d(2026, 10, 27)));
        assert_eq!(p("3d", t), Some(d(2026, 10, 27)));
        assert_eq!(p("2w", t), Some(d(2026, 11, 7)));
        assert_eq!(p("1m", t), Some(d(2026, 11, 24)));
        assert_eq!(p("mon", t), Some(d(2026, 10, 26)));
        assert_eq!(p("sat", t), Some(d(2026, 10, 31)));
        assert_eq!(p("next mon", t), Some(d(2026, 11, 2)));
        assert_eq!(p("next week", t), Some(d(2026, 10, 26)));
        assert_eq!(p("nov 3", t), Some(d(2026, 11, 3)));
        assert_eq!(p("3 november", t), Some(d(2026, 11, 3)));
        assert_eq!(p("jan 5", t), Some(d(2027, 1, 5)));
        assert_eq!(p("2027-02-01", t), Some(d(2027, 2, 1)));
        assert_eq!(p("12/25", t), Some(d(2026, 12, 25)));
        assert_eq!(p("30", t), Some(d(2026, 10, 30)));
        assert_eq!(p("2", t), Some(d(2026, 11, 2)));
        assert_eq!(p("garbage", t), None);
        assert_eq!(p("", t), None);
    }

    #[test]
    fn formats() {
        let t = d(2026, 10, 24);
        assert_eq!(short(t, t), "Today");
        assert_eq!(short(d(2026, 10, 25), t), "Tomorrow");
        assert_eq!(short(d(2026, 10, 27), t), "Tue");
        assert_eq!(short(d(2026, 11, 3), t), "Nov 3");
        assert_eq!(short(d(2027, 1, 3), t), "Jan 3, 2027");
        assert_eq!(urgency(d(2026, 10, 23), t), Urgency::Overdue);
        assert_eq!(label(d(2026, 10, 25), t, DateStyle::Absolute), "Oct 25");
        assert_eq!(label(d(2027, 1, 3), t, DateStyle::Absolute), "Jan 3, 2027");
        assert_eq!(label(d(2026, 10, 25), t, DateStyle::Relative), "Tomorrow");
    }

    #[test]
    fn week_start() {
        let sat = d(2026, 10, 24);
        assert_eq!(next_week(sat, WeekStart::Monday), d(2026, 10, 26));
        assert_eq!(next_week(sat, WeekStart::Sunday), d(2026, 10, 25));
        assert_eq!(parse("next week", sat, WeekStart::Sunday), Some(d(2026, 10, 25)));
        let sun = d(2026, 10, 25);
        assert_eq!(next_week(sun, WeekStart::Sunday), d(2026, 11, 1));
        assert_eq!(days_into_week(sun, WeekStart::Sunday), 0);
        assert_eq!(days_into_week(sun, WeekStart::Monday), 6);
    }
}
