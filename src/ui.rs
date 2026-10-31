//! Main window: search box, cover grid, status line.

use crate::art_loader::ArtLoader;
use crate::theme::{self, Theme};
use gtk::{gdk, gio, glib, prelude::*};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;
use steamlite::config::{self, Config};
use steamlite::library::{self, Game, RootKind, SteamRoot};
use steamlite::sort::{self, SortKey};
use steamlite::{art, fuzzy, launcher};

const PLACEHOLDER_VARIANTS: u32 = 6;

#[derive(Clone, Copy)]
enum Move {
    Cols(i64),
    Rows(i64),
    Pages(i64),
    First,
    Last,
}

struct Tile {
    child: gtk::FlowBoxChild,
    cover: gtk::Overlay,
    subtitle: gtk::Label,
    requested: Cell<bool>,
}

pub struct Ui {
    window: gtk::ApplicationWindow,
    entry: gtk::Entry,
    flow: gtk::FlowBox,
    scroller: gtk::ScrolledWindow,
    stack: gtk::Stack,
    count: gtk::Label,
    message: gtk::Label,
    sort_btn: gtk::Button,
    steam: gtk::Label,

    home: PathBuf,
    config_path: PathBuf,
    cfg: RefCell<Config>,
    theme: Theme,
    art: Rc<ArtLoader>,

    games: RefCell<Vec<Game>>,
    tiles: RefCell<Vec<Tile>>,
    index: RefCell<HashMap<gtk::FlowBoxChild, usize>>,
    roots: RefCell<Vec<SteamRoot>>,
    /// Fuzzy score per game index while a query is active.
    scores: RefCell<HashMap<usize, i32>>,
    sort: Cell<SortKey>,
    /// A context menu is open: it gets the keyboard, not our shortcuts.
    menu_open: Cell<bool>,
    message_timeout: RefCell<Option<glib::SourceId>>,
}

pub fn build(app: &gtk::Application, cfg: Config, config_path: PathBuf) -> Rc<Ui> {
    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .title("steamlite")
        .default_width(1020)
        .default_height(720)
        .css_classes(["steamlite"])
        .build();

    // No icon on purpose: one symbolic icon drags in the icon theme cache,
    // librsvg and libxml2 (~8 MB RSS).
    let entry = gtk::Entry::builder()
        .placeholder_text("Search games…   ( / )")
        .hexpand(true)
        .css_classes(["search"])
        .build();
    let header = gtk::Box::builder().css_classes(["header"]).build();
    header.append(&entry);

    let flow = gtk::FlowBox::builder()
        .valign(gtk::Align::Start)
        .homogeneous(true)
        .selection_mode(gtk::SelectionMode::Single)
        .activate_on_single_click(false)
        .min_children_per_line(1)
        .max_children_per_line(64)
        .row_spacing(4)
        .column_spacing(4)
        .css_classes(["grid"])
        .build();
    let scroller = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(&flow)
        .build();
    let empty = gtk::Label::builder().css_classes(["empty"]).wrap(true).justify(gtk::Justification::Center).build();
    let stack = gtk::Stack::builder().vexpand(true).build();
    stack.add_named(&scroller, Some("grid"));
    stack.add_named(&empty, Some("empty"));

    let count = gtk::Label::new(None);
    let message = gtk::Label::builder()
        .hexpand(true)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .css_classes(["message"])
        .build();
    let sort_btn = gtk::Button::builder()
        .css_classes(["sort"])
        .tooltip_text("Change sort order (s)")
        .focusable(false)
        .build();
    let steam = gtk::Label::builder().css_classes(["steam"]).build();
    let status = gtk::Box::builder().spacing(12).css_classes(["statusbar"]).build();
    status.append(&count);
    status.append(&message);
    status.append(&sort_btn);
    status.append(&steam);

    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.append(&header);
    root.append(&stack);
    root.append(&status);
    window.set_child(Some(&root));

    let display = gtk::prelude::WidgetExt::display(&window);
    let theme = Theme::new(&display, config::config_dir().join("style.css"));
    theme.set_builtin(&display, &cfg.theme);
    if !theme::is_known(&cfg.theme) {
        eprintln!("steamlite: unknown theme {:?}, using midnight", cfg.theme);
    }
    theme.reload_user();

    let ui = Rc::new(Ui {
        sort: Cell::new(SortKey::parse(&cfg.sort)),
        window,
        entry,
        flow,
        scroller,
        stack,
        count,
        message,
        sort_btn,
        steam,
        home: config::home_dir(),
        config_path,
        cfg: RefCell::new(cfg),
        theme,
        art: Rc::new(ArtLoader::default()),
        games: RefCell::default(),
        tiles: RefCell::default(),
        index: RefCell::default(),
        roots: RefCell::default(),
        scores: RefCell::default(),
        menu_open: Cell::new(false),
        message_timeout: RefCell::default(),
    });

    ui.install_filter_and_sort();
    ui.connect_signals();
    ui.watch_files();
    ui.rescan();
    ui.refresh_steam_status();
    {
        let ui = Rc::downgrade(&ui);
        glib::timeout_add_seconds_local(3, move || match ui.upgrade() {
            Some(ui) => {
                ui.refresh_steam_status();
                glib::ControlFlow::Continue
            }
            None => glib::ControlFlow::Break,
        });
    }
    ui
}

impl Ui {
    pub fn window(&self) -> &gtk::ApplicationWindow {
        &self.window
    }

    // ---- scanning / tiles ----------------------------------------------------

    fn rescan(self: &Rc<Self>) {
        let extra = self.cfg.borrow().extra_library_paths.clone();
        let scan = library::scan(&self.home, &extra);
        self.art.cancel_pending();
        while let Some(c) = self.flow.first_child() {
            self.flow.remove(&c);
        }
        let mut tiles = Vec::with_capacity(scan.games.len());
        let mut index = HashMap::new();
        for (i, g) in scan.games.iter().enumerate() {
            let t = self.make_tile(g);
            index.insert(t.child.clone(), i);
            self.flow.append(&t.child);
            tiles.push(t);
        }
        if scan.roots.is_empty() {
            self.set_empty("No Steam installation found.\nLooked in ~/.local/share/Steam, ~/.steam/steam and the Flatpak path.");
        } else if scan.games.is_empty() {
            self.set_empty("No installed games found.");
        } else {
            self.stack.set_visible_child_name("grid");
        }
        *self.games.borrow_mut() = scan.games;
        *self.tiles.borrow_mut() = tiles;
        *self.index.borrow_mut() = index;
        *self.roots.borrow_mut() = scan.roots;
        self.refilter();
    }

    fn set_empty(&self, text: &str) {
        if let Some(l) = self.stack.child_by_name("empty").and_downcast::<gtk::Label>() {
            l.set_text(text);
        }
        self.stack.set_visible_child_name("empty");
    }

    fn tile_size(&self) -> (i32, i32) {
        let w = self.cfg.borrow().tile_width.clamp(80, 600);
        (w, w * 3 / 2)
    }

    fn make_tile(self: &Rc<Self>, g: &Game) -> Tile {
        let (w, h) = self.tile_size();
        let cover = gtk::Overlay::builder()
            .width_request(w)
            .height_request(h)
            .halign(gtk::Align::Center)
            .overflow(gtk::Overflow::Hidden)
            .css_classes(["cover"])
            .build();
        let variant = art::placeholder_variant(g.appid, PLACEHOLDER_VARIANTS);
        let ph = gtk::Label::builder()
            .label(art::initials(&g.name))
            .css_classes(["placeholder", &format!("ph-{variant}")])
            .build();
        cover.set_child(Some(&ph));
        if g.needs_update() {
            let badge = gtk::Label::builder()
                .label("UPDATE")
                .halign(gtk::Align::End)
                .valign(gtk::Align::Start)
                .css_classes(["badge"])
                .build();
            cover.add_overlay(&badge);
        }

        let title = gtk::Label::builder()
            .label(&g.name)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .max_width_chars(1)
            .width_request(w)
            .xalign(0.0)
            .css_classes(["title"])
            .build();
        let subtitle = gtk::Label::builder()
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .max_width_chars(1)
            .width_request(w)
            .xalign(0.0)
            .css_classes(["subtitle"])
            .build();
        let vbox = gtk::Box::new(gtk::Orientation::Vertical, 0);
        vbox.append(&cover);
        vbox.append(&title);
        vbox.append(&subtitle);

        // Tooltip text is built lazily: set_tooltip_text() forces an X
        // pointer round-trip per call (~8 ms each here).
        let child = gtk::FlowBoxChild::builder()
            .child(&vbox)
            .css_classes(["tile"])
            .has_tooltip(true)
            .build();
        let tip = format!(
            "{}\n{} · appid {}{}",
            g.name,
            sort::human_size(g.size_on_disk),
            g.appid,
            if g.needs_update() { "\nUpdate pending" } else { "" }
        );
        child.connect_query_tooltip(move |_, _, _, _, tooltip| {
            tooltip.set_text(Some(&tip));
            true
        });
        let click = gtk::GestureClick::builder().button(gdk::BUTTON_SECONDARY).build();
        let weak = Rc::downgrade(self);
        click.connect_pressed(move |gesture, _, x, y| {
            let (Some(ui), Some(w)) = (weak.upgrade(), gesture.widget()) else { return };
            if let Ok(child) = w.downcast::<gtk::FlowBoxChild>() {
                ui.flow.select_child(&child);
                child.grab_focus();
                ui.show_menu(&child, Some((x, y)));
            }
        });
        child.add_controller(click);

        Tile { child, cover, subtitle, requested: Cell::new(false) }
    }

    fn update_subtitles(&self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let games = self.games.borrow();
        for (g, t) in games.iter().zip(self.tiles.borrow().iter()) {
            let text = match self.sort.get() {
                SortKey::Size => sort::human_size(g.size_on_disk),
                _ => sort::relative_time(g.last_played, now),
            };
            t.subtitle.set_text(&text);
        }
    }

    /// Request art for tiles inside (or just below) the viewport.
    fn load_visible(self: &Rc<Self>) {
        let adj = self.scroller.vadjustment();
        let top = adj.value() - adj.page_size() * 0.5;
        let bottom = adj.value() + adj.page_size() * 2.0;
        let scale = self.window.scale_factor().max(1);
        let (w, h) = self.tile_size();
        let tiles = self.tiles.borrow();
        let games = self.games.borrow();
        let mut wanted: Vec<(f32, usize)> = Vec::new();
        for (i, t) in tiles.iter().enumerate() {
            if t.requested.get() || !t.child.is_child_visible() {
                continue;
            }
            let Some(b) = t.child.compute_bounds(&self.flow) else { continue };
            if b.width() <= 0.0 {
                continue; // not laid out yet
            }
            let (y0, y1) = (b.y() as f64, (b.y() + b.height()) as f64);
            if y1 >= top && y0 <= bottom {
                wanted.push((b.y() * 10_000.0 + b.x(), i));
            }
        }
        wanted.sort_by(|a, b| a.0.total_cmp(&b.0));
        let roots: Vec<PathBuf> = self.roots.borrow().iter().map(|r| r.path.clone()).collect();
        for (_, i) in wanted {
            let t = &tiles[i];
            t.requested.set(true);
            let Some(path) = art::find_cover(&roots, games[i].appid) else { continue };
            let cover = t.cover.clone();
            self.art.request(
                games[i].appid,
                path,
                w * scale,
                h * scale,
                Box::new(move |tex| {
                    let pic = gtk::Picture::for_paintable(tex);
                    pic.set_content_fit(gtk::ContentFit::Cover);
                    pic.set_can_shrink(true);
                    cover.set_child(Some(&pic));
                }),
            );
        }
    }

    fn schedule_load_visible(self: &Rc<Self>) {
        // Give the FlowBox a frame to lay children out first.
        let weak = Rc::downgrade(self);
        glib::timeout_add_local_once(Duration::from_millis(16), move || {
            if let Some(ui) = weak.upgrade() {
                ui.load_visible();
            }
        });
    }

    // ---- filtering / sorting ---------------------------------------------------

    fn install_filter_and_sort(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.flow.set_filter_func(move |child| {
            let Some(ui) = weak.upgrade() else { return true };
            if ui.entry.text().trim().is_empty() {
                return true;
            }
            let idx = ui.index.borrow().get(child).copied();
            idx.is_some_and(|i| ui.scores.borrow().contains_key(&i))
        });
        let weak = Rc::downgrade(self);
        self.flow.set_sort_func(move |a, b| {
            let Some(ui) = weak.upgrade() else { return gtk::Ordering::Equal };
            let index = ui.index.borrow();
            let (Some(&ia), Some(&ib)) = (index.get(a), index.get(b)) else {
                return gtk::Ordering::Equal;
            };
            let scores = ui.scores.borrow();
            let games = ui.games.borrow();
            // Unmatched (hidden) children count as the lowest score so the
            // ordering stays total; with no query every score is MIN.
            let score = |i| scores.get(&i).copied().unwrap_or(i32::MIN);
            let by_score = score(ib).cmp(&score(ia));
            by_score.then_with(|| ui.sort.get().compare(&games[ia], &games[ib])).into()
        });
    }

    fn refilter(self: &Rc<Self>) {
        let q = self.entry.text().to_string();
        {
            let games = self.games.borrow();
            let mut scores = self.scores.borrow_mut();
            scores.clear();
            if !q.trim().is_empty() {
                for (i, g) in games.iter().enumerate() {
                    if let Some(s) = fuzzy::score(&q, &g.name) {
                        scores.insert(i, s);
                    }
                }
            }
        }
        self.flow.invalidate_filter();
        self.flow.invalidate_sort();
        self.update_subtitles();
        self.sort_btn.set_label(&format!("sort: {}", self.sort.get().label()));

        let total = self.games.borrow().len();
        let shown = if q.trim().is_empty() { total } else { self.scores.borrow().len() };
        self.count.set_text(&if shown == total {
            format!("{total} games")
        } else {
            format!("{shown} of {total} games")
        });
        if total > 0 {
            if shown == 0 {
                self.set_empty(&format!("Nothing matches “{}”", q.trim()));
            } else {
                self.stack.set_visible_child_name("grid");
            }
        }
        self.select_first();
        self.scroller.vadjustment().set_value(0.0);
        self.schedule_load_visible();
    }

    fn first_visible(&self) -> Option<gtk::FlowBoxChild> {
        self.visible_children().into_iter().next()
    }

    fn select_first(&self) {
        match self.first_visible() {
            Some(c) => self.flow.select_child(&c),
            None => self.flow.unselect_all(),
        }
    }

    fn selected(&self) -> Option<(gtk::FlowBoxChild, Game)> {
        let child = self.flow.selected_children().into_iter().next()?;
        let i = *self.index.borrow().get(&child)?;
        let g = self.games.borrow().get(i)?.clone();
        Some((child, g))
    }

    fn cycle_sort(self: &Rc<Self>) {
        self.sort.set(self.sort.get().next());
        let keep = self.selected().map(|(c, _)| c);
        self.flow.invalidate_sort();
        self.update_subtitles();
        self.sort_btn.set_label(&format!("sort: {}", self.sort.get().label()));
        if let Some(c) = keep {
            self.flow.select_child(&c);
            // Re-layout happens on the next frame; follow the tile there.
            let weak = Rc::downgrade(self);
            glib::timeout_add_local_once(Duration::from_millis(32), move || {
                if let Some(ui) = weak.upgrade() {
                    ui.scroll_to(&c);
                    ui.load_visible();
                }
            });
        }
        self.schedule_load_visible();
    }

    // ---- actions -------------------------------------------------------------

    fn launch(self: &Rc<Self>, g: &Game) {
        let cfg = self.cfg.borrow().clone();
        let kind = self.roots.borrow().first().map(|r| r.kind).unwrap_or(RootKind::Native);
        let running = launcher::steam_running(&self.home);
        let steam = launcher::steam_argv(&cfg.steam_command, kind);
        let argv = launcher::launch_argv(&steam, running, &cfg.steam_flags, g.appid);
        match launcher::spawn_detached(&argv) {
            Ok(()) => {
                let note = if running { "" } else { " (starting Steam in the background)" };
                self.flash(&format!("Launching {}{note}…", g.name));
                if cfg.close_on_launch {
                    // Let the message paint once; the child is already detached.
                    let win = self.window.clone();
                    glib::timeout_add_local_once(Duration::from_millis(150), move || win.close());
                }
            }
            Err(e) => self.flash(&format!("Could not run “{}”: {e}", argv.join(" "))),
        }
    }

    fn open_folder(&self, g: &Game) {
        let uri = gio::File::for_path(&g.install_dir).uri();
        if gio::AppInfo::launch_default_for_uri(&uri, gio::AppLaunchContext::NONE).is_ok() {
            return;
        }
        let argv = ["xdg-open".to_string(), g.install_dir.display().to_string()];
        match launcher::spawn_detached(&argv) {
            Ok(()) => self.flash(&format!("Opening {}", g.install_dir.display())),
            Err(e) => self.flash(&format!("Could not open {}: {e}", g.install_dir.display())),
        }
    }

    fn show_appid(&self, g: &Game) {
        self.window.clipboard().set_text(&g.appid.to_string());
        self.flash(&format!("{} — appid {} (copied to clipboard)", g.name, g.appid));
    }

    fn flash(&self, text: &str) {
        self.message.set_text(text);
        if let Some(id) = self.message_timeout.take() {
            id.remove();
        }
        let msg = self.message.clone();
        let id = glib::timeout_add_seconds_local_once(5, move || msg.set_text(""));
        self.message_timeout.replace(Some(id));
    }

    fn refresh_steam_status(&self) {
        let running = launcher::steam_running(&self.home);
        self.steam.set_text(if running { "● Steam running" } else { "○ Steam not running" });
        self.steam.set_css_classes(&["steam", if running { "running" } else { "stopped" }]);
    }

    /// Per-game context menu. A plain Popover with our own key handling:
    /// GtkPopoverMenu lets unhandled arrow keys bubble up to the FlowBox
    /// under it, which moved the grid instead of the menu.
    fn show_menu(self: &Rc<Self>, child: &gtk::FlowBoxChild, at: Option<(f64, f64)>) {
        let Some(&i) = self.index.borrow().get(child) else { return };
        let game = self.games.borrow()[i].clone();
        let pop = gtk::Popover::builder().has_arrow(false).css_classes(["menu"]).build();
        let vbox = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let items: [(&str, fn(&Rc<Ui>, &Game)); 3] = [
            ("Launch", |ui, g| ui.launch(g)),
            ("Open install folder", |ui, g| ui.open_folder(g)),
            ("Show appid", |ui, g| ui.show_appid(g)),
        ];
        let mut buttons = Vec::new();
        for (label, f) in items {
            let b = gtk::Button::builder().label(label).css_classes(["menuitem"]).build();
            if let Some(l) = b.child().and_downcast::<gtk::Label>() {
                l.set_xalign(0.0);
            }
            let (weak, g, p) = (Rc::downgrade(self), game.clone(), pop.clone());
            b.connect_clicked(move |_| {
                p.popdown();
                if let Some(ui) = weak.upgrade() {
                    f(&ui, &g);
                }
            });
            vbox.append(&b);
            buttons.push(b);
        }
        pop.set_child(Some(&vbox));

        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let (p, btns) = (pop.clone(), buttons.clone());
        keys.connect_key_pressed(move |_, key, _, _| {
            use gdk::Key;
            let cur = btns.iter().position(|b| b.has_focus()).unwrap_or(0);
            let n = btns.len();
            let focus = |i: usize| {
                btns[i].grab_focus();
            };
            match key {
                Key::Down | Key::j | Key::Tab => focus((cur + 1) % n),
                Key::Up | Key::k | Key::ISO_Left_Tab => focus((cur + n - 1) % n),
                Key::Home => focus(0),
                Key::End => focus(n - 1),
                Key::Return | Key::KP_Enter | Key::space => btns[cur].emit_clicked(),
                Key::Escape | Key::Menu | Key::q => p.popdown(),
                _ => return glib::Propagation::Proceed,
            };
            glib::Propagation::Stop
        });
        pop.add_controller(keys);

        pop.set_parent(child);
        // Keep the tile tooltip from popping up over the menu.
        child.set_has_tooltip(false);
        if let Some((x, y)) = at {
            pop.set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
        }
        let c = child.clone();
        let weak = Rc::downgrade(self);
        pop.connect_closed(move |p| {
            let p = p.clone();
            c.set_has_tooltip(true);
            if let Some(ui) = weak.upgrade() {
                ui.menu_open.set(false);
                c.grab_focus();
            }
            // Unparent after the clicked handler has run.
            glib::idle_add_local_once(move || p.unparent());
        });
        self.menu_open.set(true);
        pop.popup();
        buttons[0].grab_focus();
    }

    // ---- input -------------------------------------------------------------

    fn focus_in_entry(&self) -> bool {
        gtk::prelude::RootExt::focus(&self.window).is_some_and(|w| w.is_ancestor(&self.entry) || w == self.entry)
    }

    fn focus_grid(&self) {
        if let Some((c, _)) = self.selected() {
            c.grab_focus();
        } else if let Some(c) = self.first_visible() {
            self.flow.select_child(&c);
            c.grab_focus();
        }
    }

    fn connect_signals(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.entry.connect_changed(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.refilter();
            }
        });
        let weak = Rc::downgrade(self);
        self.entry.connect_activate(move |_| {
            let Some(ui) = weak.upgrade() else { return };
            if let Some((_, g)) = ui.selected() {
                ui.launch(&g);
            }
        });
        let weak = Rc::downgrade(self);
        self.flow.connect_child_activated(move |_, child| {
            let Some(ui) = weak.upgrade() else { return };
            let g = ui.index.borrow().get(child).map(|&i| ui.games.borrow()[i].clone());
            if let Some(g) = g {
                ui.launch(&g);
            }
        });
        let weak = Rc::downgrade(self);
        self.sort_btn.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.cycle_sort();
            }
        });

        let adj = self.scroller.vadjustment();
        for sig in ["value-changed", "changed"] {
            let weak = Rc::downgrade(self);
            adj.connect_local(sig, false, move |_| {
                if let Some(ui) = weak.upgrade() {
                    ui.load_visible();
                }
                None
            });
        }

        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = Rc::downgrade(self);
        keys.connect_key_pressed(move |_, key, _, state| {
            let Some(ui) = weak.upgrade() else { return glib::Propagation::Proceed };
            ui.on_key(key, state)
        });
        self.window.add_controller(keys);

        self.entry.grab_focus();
    }

    fn on_key(self: &Rc<Self>, key: gdk::Key, state: gdk::ModifierType) -> glib::Propagation {
        use gdk::Key;
        use glib::Propagation::{Proceed, Stop};
        if self.menu_open.get() {
            return Proceed;
        }
        let ctrl = state.contains(gdk::ModifierType::CONTROL_MASK);
        let in_entry = self.focus_in_entry();

        match key {
            Key::Escape => {
                if !self.entry.text().is_empty() {
                    self.entry.set_text("");
                    self.entry.grab_focus();
                } else {
                    self.window.close();
                }
                return Stop;
            }
            Key::q | Key::w if ctrl => {
                self.window.close();
                return Stop;
            }
            Key::F5 => {
                self.rescan_with_message();
                return Stop;
            }
            Key::Menu => {
                if let Some((c, _)) = self.selected() {
                    self.show_menu(&c, None);
                }
                return Stop;
            }
            Key::F10 if state.contains(gdk::ModifierType::SHIFT_MASK) => {
                if let Some((c, _)) = self.selected() {
                    self.show_menu(&c, None);
                }
                return Stop;
            }
            _ => {}
        }

        if in_entry {
            return match key {
                Key::Down | Key::Tab | Key::Page_Down => {
                    self.focus_grid();
                    Stop
                }
                _ => Proceed,
            };
        }

        if ctrl || state.contains(gdk::ModifierType::ALT_MASK) {
            return Proceed;
        }
        let mv = |m| {
            self.move_selection(m);
            Stop
        };
        match key {
            Key::slash => {
                self.entry.grab_focus();
                Stop
            }
            Key::h | Key::Left => mv(Move::Cols(-1)),
            Key::l | Key::Right => mv(Move::Cols(1)),
            Key::j | Key::Down => mv(Move::Rows(1)),
            Key::k | Key::Up => mv(Move::Rows(-1)),
            Key::Page_Down => mv(Move::Pages(1)),
            Key::Page_Up => mv(Move::Pages(-1)),
            Key::g | Key::Home => mv(Move::First),
            Key::G | Key::End => mv(Move::Last),
            Key::r => {
                self.rescan_with_message();
                Stop
            }
            Key::s => {
                self.cycle_sort();
                Stop
            }
            Key::q => {
                self.window.close();
                Stop
            }
            _ => Proceed,
        }
    }

    /// Visible children in display order.
    fn visible_children(&self) -> Vec<gtk::FlowBoxChild> {
        let mut out = Vec::new();
        let mut i = 0;
        while let Some(c) = self.flow.child_at_index(i) {
            if c.is_child_visible() {
                out.push(c);
            }
            i += 1;
        }
        out
    }

    /// Grid navigation done by hand: GtkFlowBox only moves its internal
    /// cursor after a click, so select_child() + arrow keys don't work.
    fn move_selection(&self, m: Move) {
        let kids = self.visible_children();
        if kids.is_empty() {
            return;
        }
        let y_of = |c: &gtk::FlowBoxChild| c.compute_bounds(&self.flow).map(|b| b.y()).unwrap_or(0.0);
        let y0 = y_of(&kids[0]);
        let cols = kids.iter().take_while(|c| (y_of(c) - y0).abs() < 1.0).count().max(1) as i64;
        let n = kids.len() as i64;
        let cur = self.selected().and_then(|(c, _)| kids.iter().position(|k| *k == c)).unwrap_or(0) as i64;
        let row_h = kids.get(cols as usize).map(|c| y_of(c) - y0).filter(|h| *h > 0.0).unwrap_or(1.0) as f64;
        let page_rows = ((self.scroller.vadjustment().page_size() / row_h).floor() as i64).max(1);
        let next = match m {
            Move::Cols(d) => (cur + d).clamp(0, n - 1),
            Move::Rows(d) | Move::Pages(d) => {
                let d = if matches!(m, Move::Pages(_)) { d * page_rows } else { d };
                let t = cur + d * cols;
                if t < 0 {
                    if matches!(m, Move::Rows(_)) {
                        // Up from the top row goes back to the search box.
                        self.entry.grab_focus();
                        return;
                    }
                    cur % cols
                } else if t >= n {
                    // Moving down into a short last row lands on its last tile.
                    if cur / cols < (n - 1) / cols { n - 1 } else { cur }
                } else {
                    t
                }
            }
            Move::First => 0,
            Move::Last => n - 1,
        };
        let c = &kids[next as usize];
        self.flow.select_child(c);
        c.grab_focus();
        self.scroll_to(c);
    }

    fn scroll_to(&self, c: &gtk::FlowBoxChild) {
        let Some(b) = c.compute_bounds(&self.flow) else { return };
        let adj = self.scroller.vadjustment();
        let (top, bottom) = (b.y() as f64, (b.y() + b.height()) as f64);
        if top < adj.value() {
            adj.set_value(top);
        } else if bottom > adj.value() + adj.page_size() {
            adj.set_value(bottom - adj.page_size());
        }
    }

    fn rescan_with_message(self: &Rc<Self>) {
        self.rescan();
        self.flash(&format!("Rescanned: {} games", self.games.borrow().len()));
    }

    // ---- live reload -----------------------------------------------------------

    fn watch_files(self: &Rc<Self>) {
        let dir = config::config_dir();
        let weak = Rc::downgrade(self);
        self.theme.watch(&dir.join("style.css"), move || {
            if let Some(ui) = weak.upgrade() {
                ui.theme.reload_user();
                ui.flash("Reloaded style.css");
            }
        });
        let weak = Rc::downgrade(self);
        self.theme.watch(&self.config_path.clone(), move || {
            if let Some(ui) = weak.upgrade() {
                ui.reload_config();
            }
        });
    }

    fn reload_config(self: &Rc<Self>) {
        let Ok(text) = std::fs::read_to_string(&self.config_path) else { return };
        let new = match config::parse(&text) {
            Ok(c) => c,
            Err(e) => {
                self.flash(&format!("config.toml: {}", e.message()));
                return;
            }
        };
        let old = self.cfg.replace(new.clone());
        if old.theme != new.theme {
            let display = gtk::prelude::WidgetExt::display(&self.window);
            self.theme.set_builtin(&display, &new.theme);
        }
        if old.tile_width != new.tile_width || old.extra_library_paths != new.extra_library_paths {
            if old.tile_width != new.tile_width {
                self.art.clear();
            }
            self.rescan();
        }
        self.flash("Reloaded config.toml");
    }
}
