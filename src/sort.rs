//! Sort orders and the small formatting helpers shown under tiles.

use crate::library::Game;
use std::cmp::Ordering;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortKey {
    LastPlayed,
    Name,
    Size,
}

impl SortKey {
    pub fn parse(s: &str) -> SortKey {
        match s.trim().to_ascii_lowercase().as_str() {
            "name" => SortKey::Name,
            "size" | "size_on_disk" => SortKey::Size,
            _ => SortKey::LastPlayed,
        }
    }

    pub fn next(self) -> SortKey {
        match self {
            SortKey::LastPlayed => SortKey::Name,
            SortKey::Name => SortKey::Size,
            SortKey::Size => SortKey::LastPlayed,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SortKey::LastPlayed => "last played",
            SortKey::Name => "name",
            SortKey::Size => "size",
        }
    }

    pub fn compare(self, a: &Game, b: &Game) -> Ordering {
        let by_name = || sort_name(&a.name).cmp(&sort_name(&b.name));
        match self {
            SortKey::LastPlayed => b.last_played.cmp(&a.last_played).then_with(by_name),
            SortKey::Name => by_name(),
            SortKey::Size => b.size_on_disk.cmp(&a.size_on_disk).then_with(by_name),
        }
    }
}

/// Case-insensitive, ignoring a leading "The ".
pub fn sort_name(name: &str) -> String {
    let l = name.to_lowercase();
    l.strip_prefix("the ").map(str::to_string).unwrap_or(l)
}

pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = bytes as f64;
    let mut u = 0;
    while v >= 1000.0 && u < UNITS.len() - 1 {
        v /= 1000.0;
        u += 1;
    }
    if u == 0 {
        format!("{bytes} B")
    } else if v >= 100.0 {
        format!("{v:.0} {}", UNITS[u])
    } else {
        format!("{v:.1} {}", UNITS[u])
    }
}

pub fn relative_time(then: u64, now: u64) -> String {
    if then == 0 {
        return "never played".into();
    }
    let d = now.saturating_sub(then);
    match d {
        0..=3599 => "just now".into(),
        3600..=86_399 => format!("{}h ago", d / 3600),
        86_400..=172_799 => "yesterday".into(),
        _ if d < 86_400 * 30 => format!("{}d ago", d / 86_400),
        _ if d < 86_400 * 365 => format!("{}mo ago", d / (86_400 * 30)),
        _ => format!("{}y ago", d / (86_400 * 365)),
    }
}
