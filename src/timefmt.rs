//! Dates the way MailCraft shows them: a time for today, the weekday this week, a date before.

use chrono::{DateTime, Datelike, Local, TimeZone};

pub fn local(ms: i64) -> Option<DateTime<Local>> {
    Local.timestamp_millis_opt(ms).single()
}

fn time(d: &DateTime<Local>) -> String {
    d.format("%-I:%M %p").to_string()
}

/// The short form in the conversation list.
pub fn list(ms: i64) -> String {
    let Some(d) = local(ms) else { return String::new() };
    let now = Local::now();
    let days = (now.date_naive() - d.date_naive()).num_days();
    if days == 0 {
        time(&d)
    } else if days == 1 {
        "Yesterday".into()
    } else if (2..7).contains(&days) {
        d.format("%a").to_string()
    } else if d.year() == now.year() {
        d.format("%b %-d").to_string()
    } else {
        d.format("%-m/%-d/%Y").to_string()
    }
}

/// The group a conversation falls in, like MailCraft's Today / Yesterday / Last Week headings.
pub fn group(ms: i64) -> &'static str {
    let Some(d) = local(ms) else { return "Older" };
    let now = Local::now();
    let days = (now.date_naive() - d.date_naive()).num_days();
    match days {
        i64::MIN..=0 => "Today",
        1 => "Yesterday",
        2..=6 => "Earlier this week",
        7..=13 => "Last week",
        _ if d.year() == now.year() && d.month() == now.month() => "Earlier this month",
        _ => "Older",
    }
}

/// The day heading between messages.
pub fn day(ms: i64) -> String {
    let Some(d) = local(ms) else { return String::new() };
    let now = Local::now();
    let days = (now.date_naive() - d.date_naive()).num_days();
    match days {
        0 => "Today".into(),
        1 => "Yesterday".into(),
        2..=6 => d.format("%A").to_string(),
        _ if d.year() == now.year() => d.format("%a, %b %-d").to_string(),
        _ => d.format("%a, %b %-d, %Y").to_string(),
    }
}

pub fn clock(ms: i64) -> String {
    local(ms).map(|d| time(&d)).unwrap_or_default()
}

pub fn full(ms: i64) -> String {
    local(ms).map(|d| d.format("%a %-m/%-d/%Y %-I:%M %p").to_string()).unwrap_or_default()
}

pub fn same_day(a: i64, b: i64) -> bool {
    match (local(a), local(b)) {
        (Some(x), Some(y)) => x.date_naive() == y.date_naive(),
        _ => false,
    }
}

pub fn now_ms() -> i64 {
    Local::now().timestamp_millis()
}
