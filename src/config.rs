//! `~/.config/steamlite/config.toml`

use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

pub const DEFAULT_CONFIG: &str = r#"# steamlite configuration. Delete this file to regenerate defaults.

# Built-in theme: "midnight", "paper" or "cde".
# ~/.config/steamlite/style.css (if present) is loaded on top and live-reloaded.
theme = "midnight"

# Cover tile width in pixels (height follows the 2:3 cover aspect).
tile_width = 160

# Initial sort: "last_played", "name" or "size". Press `s` to cycle.
sort = "last_played"

# Quit steamlite after starting a game.
close_on_launch = true

# Command used to talk to Steam. "auto" picks `steam`, or
# `flatpak run com.valvesoftware.Steam` for a Flatpak install.
# Inside a distrobox you may want "distrobox-host-exec steam".
steam_command = "auto"

# Extra flags passed only when steamlite has to start Steam itself
# (it always adds -silent), e.g. to make the hidden CEF process cheaper:
# steam_flags = ["-cef-disable-gpu-compositing", "-cef-disable-gpu"]
steam_flags = []

# Library folders not listed in Steam's libraryfolders.vdf.
# extra_library_paths = ["/mnt/games/SteamLibrary"]
extra_library_paths = []

# Terminal used for steamcmd (phase 2). Empty = $TERMINAL.
terminal = ""

# GSK renderer: "cairo" (lowest memory, default), "gl", "ngl", "vulkan",
# or "" to let GTK decide. The GSK_RENDERER env var always wins.
renderer = "cairo"
"#;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    pub theme: String,
    pub tile_width: i32,
    pub sort: String,
    pub close_on_launch: bool,
    pub steam_command: String,
    pub steam_flags: Vec<String>,
    pub extra_library_paths: Vec<PathBuf>,
    pub terminal: String,
    pub renderer: String,
}

impl Default for Config {
    fn default() -> Self {
        Self::builtin()
    }
}

// Field defaults live here; a test checks DEFAULT_CONFIG agrees with them.
impl Config {
    pub fn builtin() -> Self {
        Config {
            theme: "midnight".into(),
            tile_width: 160,
            sort: "last_played".into(),
            close_on_launch: true,
            steam_command: "auto".into(),
            steam_flags: Vec::new(),
            extra_library_paths: Vec::new(),
            terminal: String::new(),
            renderer: "cairo".into(),
        }
    }
}

pub fn config_dir() -> PathBuf {
    // Inside Flatpak XDG_CONFIG_HOME points into ~/.var/app; keep using the
    // real ~/.config/steamlite (the manifest grants access to it).
    if crate::launcher::in_flatpak() {
        return home_dir().join(".config/steamlite");
    }
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| home_dir().join(".config"))
        .join("steamlite")
}

pub fn home_dir() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/"))
}

pub fn parse(text: &str) -> Result<Config, toml::de::Error> {
    toml::from_str(text)
}

/// Load the config, writing commented defaults on first run.
pub fn load_or_create(path: &Path) -> Config {
    match fs::read_to_string(path) {
        Ok(text) => parse(&text).unwrap_or_else(|e| {
            eprintln!("steamlite: {}: {e}; using defaults", path.display());
            Config::builtin()
        }),
        Err(_) => {
            if let Some(dir) = path.parent() {
                let _ = fs::create_dir_all(dir);
            }
            if let Err(e) = fs::write(path, DEFAULT_CONFIG) {
                eprintln!("steamlite: could not write {}: {e}", path.display());
            }
            Config::builtin()
        }
    }
}
