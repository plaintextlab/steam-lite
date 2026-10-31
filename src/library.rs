//! Discover Steam roots, library folders and installed games. Read-only.

use crate::{filter, vdf};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

/// `StateFlags` bit meaning "fully installed" (other bits such as 2 =
/// update required may be set alongside it).
const STATE_FULLY_INSTALLED: u32 = 4;

#[derive(Debug, Clone, PartialEq)]
pub struct Game {
    pub appid: u32,
    pub name: String,
    pub size_on_disk: u64,
    pub state_flags: u32,
    /// Unix seconds, 0 if never played.
    pub last_played: u64,
    /// `<library>/steamapps/common/<installdir>`
    pub install_dir: PathBuf,
}

impl Game {
    pub fn needs_update(&self) -> bool {
        self.state_flags & 2 != 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootKind {
    Native,
    Flatpak,
}

#[derive(Debug, Clone)]
pub struct SteamRoot {
    pub path: PathBuf,
    pub kind: RootKind,
}

/// Candidate Steam installation roots, existing ones only, de-duplicated by
/// canonical path (`~/.steam/steam` is usually a symlink).
pub fn find_roots(home: &Path) -> Vec<SteamRoot> {
    let candidates = [
        (home.join(".local/share/Steam"), RootKind::Native),
        (home.join(".steam/steam"), RootKind::Native),
        (home.join(".steam/root"), RootKind::Native),
        (home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"), RootKind::Flatpak),
        (home.join(".var/app/com.valvesoftware.Steam/data/Steam"), RootKind::Flatpak),
    ];
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for (p, kind) in candidates {
        let Ok(canon) = fs::canonicalize(&p) else { continue };
        if canon.join("steamapps").is_dir() && seen.insert(canon.clone()) {
            out.push(SteamRoot { path: canon, kind });
        }
    }
    out
}

/// Library folder paths listed in `libraryfolders.vdf` (the root itself is
/// always included, even if the file is missing or broken).
pub fn library_folders(root: &Path) -> Vec<PathBuf> {
    let mut out = vec![root.to_path_buf()];
    let file = root.join("steamapps/libraryfolders.vdf");
    if let Ok(text) = fs::read_to_string(&file) {
        match vdf::parse(&text) {
            Ok(doc) => out.extend(parse_library_folders(&doc)),
            Err(e) => eprintln!("steamlite: {}: {e}", file.display()),
        }
    }
    out
}

pub fn parse_library_folders(doc: &vdf::Value) -> Vec<PathBuf> {
    let Some(lf) = doc.get("libraryfolders") else { return Vec::new() };
    lf.entries()
        .iter()
        .filter_map(|(_, v)| match v {
            // Current format: "0" { "path" "..." ... }
            vdf::Value::Obj(_) => v.get_str("path").map(PathBuf::from),
            // Legacy format: "1" "/path/to/library"
            vdf::Value::Str(s) if s.starts_with('/') => Some(PathBuf::from(s)),
            _ => None,
        })
        .collect()
}

/// Parse one `appmanifest_*.acf`. Returns None if it's not an installed app.
pub fn parse_manifest(text: &str, library: &Path) -> Option<Game> {
    let doc = vdf::parse(text).ok()?;
    let st = doc.get("AppState")?;
    let num = |k: &str| st.get_str(k).and_then(|s| s.trim().parse::<u64>().ok()).unwrap_or(0);
    let appid = st.get_str("appid")?.trim().parse().ok()?;
    let installdir = st.get_str("installdir").unwrap_or_default();
    let name = st
        .get_str("name")
        .filter(|n| !n.is_empty())
        .unwrap_or(installdir)
        .to_string();
    Some(Game {
        appid,
        name,
        size_on_disk: num("SizeOnDisk"),
        state_flags: num("StateFlags") as u32,
        last_played: num("LastPlayed"),
        install_dir: library.join("steamapps/common").join(installdir),
    })
}

/// Map appid → LastPlayed from every `userdata/*/config/localconfig.vdf`.
/// Missing or unreadable files are simply skipped.
pub fn last_played(root: &Path) -> HashMap<u32, u64> {
    let mut out = HashMap::new();
    let Ok(users) = fs::read_dir(root.join("userdata")) else { return out };
    for user in users.flatten() {
        let Ok(text) = fs::read_to_string(user.path().join("config/localconfig.vdf")) else {
            continue;
        };
        if let Ok(doc) = vdf::parse(&text) {
            merge_last_played(&doc, &mut out);
        }
    }
    out
}

pub fn merge_last_played(doc: &vdf::Value, out: &mut HashMap<u32, u64>) {
    let path = ["UserLocalConfigStore", "Software", "Valve", "Steam", "apps"];
    let Some(apps) = doc.path(&path) else { return };
    for (id, app) in apps.entries() {
        let (Ok(id), Some(t)) = (id.parse::<u32>(), app.get_str("LastPlayed")) else {
            continue;
        };
        if let Ok(t) = t.parse::<u64>() {
            let e = out.entry(id).or_insert(0);
            *e = (*e).max(t);
        }
    }
}

pub struct Scan {
    pub games: Vec<Game>,
    pub roots: Vec<SteamRoot>,
}

/// Scan all roots plus `extra_libraries`; returns filtered, de-duplicated
/// installed games.
pub fn scan(home: &Path, extra_libraries: &[PathBuf]) -> Scan {
    let roots = find_roots(home);
    let mut libs: Vec<PathBuf> = Vec::new();
    let mut seen = HashSet::new();
    let all = roots
        .iter()
        .flat_map(|r| library_folders(&r.path))
        .chain(extra_libraries.iter().cloned());
    for lib in all {
        let canon = fs::canonicalize(&lib).unwrap_or(lib);
        if seen.insert(canon.clone()) {
            libs.push(canon);
        }
    }

    let mut played = HashMap::new();
    for r in &roots {
        for (id, t) in last_played(&r.path) {
            let e = played.entry(id).or_insert(0);
            *e = (*e).max(t);
        }
    }

    let mut games: HashMap<u32, Game> = HashMap::new();
    for lib in &libs {
        let Ok(dir) = fs::read_dir(lib.join("steamapps")) else { continue };
        for entry in dir.flatten() {
            let fname = entry.file_name();
            let fname = fname.to_string_lossy();
            if !(fname.starts_with("appmanifest_") && fname.ends_with(".acf")) {
                continue;
            }
            let Ok(text) = fs::read_to_string(entry.path()) else { continue };
            let Some(mut g) = parse_manifest(&text, lib) else { continue };
            if g.state_flags & STATE_FULLY_INSTALLED == 0 || !filter::is_game(g.appid, &g.name) {
                continue;
            }
            if let Some(&t) = played.get(&g.appid) {
                g.last_played = g.last_played.max(t);
            }
            games.entry(g.appid).or_insert(g);
        }
    }
    Scan { games: games.into_values().collect(), roots }
}
