//! Locate cover art in Steam's local `appcache/librarycache`.
//!
//! Layouts seen in the wild:
//!   old flat:   librarycache/<appid>_library_600x900.jpg
//!   per-appid:  librarycache/<appid>/library_600x900.jpg
//!   hashed:     librarycache/<appid>/<sha1>/library_600x900.jpg
//!               librarycache/<appid>/<sha1>/library_capsule.jpg

use std::fs;
use std::path::{Path, PathBuf};

/// Portrait art file names, best first.
const PORTRAIT_NAMES: &[&str] = &[
    "library_600x900_2x.jpg",
    "library_600x900.jpg",
    "library_capsule_2x.jpg",
    "library_capsule.jpg",
];

pub fn find_cover(roots: &[PathBuf], appid: u32) -> Option<PathBuf> {
    roots.iter().find_map(|r| find_cover_in(&r.join("appcache/librarycache"), appid))
}

pub fn find_cover_in(cache: &Path, appid: u32) -> Option<PathBuf> {
    let dir = cache.join(appid.to_string());
    // Collect the per-appid dir and its hashed subdirs once.
    let mut dirs = vec![dir.clone()];
    if let Ok(rd) = fs::read_dir(&dir) {
        let mut subs: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
        subs.sort();
        dirs.extend(subs);
    }
    for name in PORTRAIT_NAMES {
        if let Some(p) = dirs.iter().map(|d| d.join(name)).find(|p| p.is_file()) {
            return Some(p);
        }
        let flat = cache.join(format!("{appid}_{name}"));
        if flat.is_file() {
            return Some(flat);
        }
    }
    None
}

/// Up to two initials for the placeholder tile: "Kingdom Come" → "KC".
pub fn initials(name: &str) -> String {
    let words: Vec<&str> = name
        .split(|c: char| c.is_whitespace() || c == ':' || c == '-')
        .filter(|w| w.chars().next().is_some_and(char::is_alphanumeric))
        .collect();
    let mut out: String = words.iter().take(2).filter_map(|w| w.chars().next()).collect();
    if out.is_empty() {
        out = name.chars().take(2).collect();
    }
    out.to_uppercase()
}

/// Stable small hash used to pick a placeholder colour class.
pub fn placeholder_variant(appid: u32, variants: u32) -> u32 {
    appid.wrapping_mul(2654435761) % variants
}
