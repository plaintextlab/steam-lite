//! Embedded stylesheets plus a live-reloaded user override.
//!
//! GTK's base theme is pinned to its built-in Adwaita (see `main`), and our
//! CSS is added just above USER priority: APPLICATION (600) alone would still
//! lose to ~/.config/gtk-4.0/gtk.css (800), which commonly carries global
//! rules like `* { border-radius: 0; }`.

use gtk::{gdk, gio, prelude::*};
use std::cell::RefCell;
use std::path::{Path, PathBuf};

const BASE: &str = include_str!("../data/css/base.css");
pub const THEMES: &[(&str, &str, bool)] = &[
    // (name, css, dark)
    ("midnight", include_str!("../data/css/midnight.css"), true),
    ("paper", include_str!("../data/css/paper.css"), false),
    ("cde", include_str!("../data/css/cde.css"), false),
];

pub struct Theme {
    builtin: gtk::CssProvider,
    user: gtk::CssProvider,
    user_path: PathBuf,
    monitors: RefCell<Vec<gio::FileMonitor>>,
}

fn report_errors(p: &gtk::CssProvider, origin: &'static str) {
    p.connect_parsing_error(move |_, section, err| {
        let loc = section.start_location();
        eprintln!("steamlite: CSS {origin}:{}:{}: {err}", loc.lines() + 1, loc.line_chars() + 1);
    });
}

impl Theme {
    pub fn new(display: &gdk::Display, user_path: PathBuf) -> Theme {
        let builtin = gtk::CssProvider::new();
        let user = gtk::CssProvider::new();
        report_errors(&builtin, "builtin");
        report_errors(&user, "style.css");

        // In-process only. A blinking caret makes the cairo renderer repaint
        // the whole window twice a second (~8% CPU) while the search box
        // has focus; a steady caret keeps idle at 0%.
        gtk::Settings::for_display(display).set_gtk_cursor_blink(false);
        let prio = gtk::STYLE_PROVIDER_PRIORITY_USER + 1;
        gtk::style_context_add_provider_for_display(display, &builtin, prio);
        gtk::style_context_add_provider_for_display(display, &user, prio + 1);
        Theme { builtin, user, user_path, monitors: RefCell::new(Vec::new()) }
    }

    /// Apply a built-in theme by name (unknown names fall back to midnight).
    pub fn set_builtin(&self, display: &gdk::Display, name: &str) {
        let (_, css, dark) = THEMES.iter().find(|(n, ..)| *n == name).unwrap_or(&THEMES[0]);
        gtk::Settings::for_display(display).set_gtk_application_prefer_dark_theme(*dark);
        // Base first: theme rules after it win at equal specificity.
        self.builtin.load_from_string(&format!("{BASE}\n{css}"));
    }

    pub fn reload_user(&self) {
        let css = std::fs::read_to_string(&self.user_path).unwrap_or_default();
        self.user.load_from_string(&css);
    }

    /// Watch `path`, calling `f` whenever it is written, created or removed.
    pub fn watch(&self, path: &Path, f: impl Fn() + 'static) {
        let file = gio::File::for_path(path);
        match file.monitor_file(gio::FileMonitorFlags::WATCH_MOVES, gio::Cancellable::NONE) {
            Ok(m) => {
                m.set_rate_limit(100);
                m.connect_changed(move |_, _, _, ev| {
                    use gio::FileMonitorEvent as E;
                    if matches!(ev, E::ChangesDoneHint | E::Created | E::Deleted | E::MovedIn | E::Renamed) {
                        f();
                    }
                });
                self.monitors.borrow_mut().push(m);
            }
            Err(e) => eprintln!("steamlite: cannot watch {}: {e}", path.display()),
        }
    }
}

pub fn is_known(name: &str) -> bool {
    THEMES.iter().any(|(n, ..)| *n == name)
}

/// Value for `GTK_THEME` so GTK never loads the system theme at all
/// (faster startup, nothing leaks into our widgets).
pub fn gtk_theme_env(name: &str) -> &'static str {
    match THEMES.iter().find(|(n, ..)| *n == name).unwrap_or(&THEMES[0]) {
        (_, _, true) => "Adwaita:dark",
        _ => "Adwaita",
    }
}
