mod art_loader;
mod theme;
mod ui;

use gtk::{glib, prelude::*};
use std::time::{SystemTime, UNIX_EPOCH};
use steamlite::config;

fn epoch_ms() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64() * 1000.0).unwrap_or(0.0)
}

/// One malloc arena instead of one per thread: the cover-decoding workers
/// otherwise each leave a mostly-empty arena resident (~4 MB RSS).
#[cfg(target_env = "gnu")]
fn limit_malloc_arenas() {
    extern "C" {
        fn mallopt(param: std::os::raw::c_int, value: std::os::raw::c_int) -> std::os::raw::c_int;
    }
    const M_ARENA_MAX: std::os::raw::c_int = -8;
    // SAFETY: called first thing in main, before any thread exists.
    unsafe { mallopt(M_ARENA_MAX, 1) };
}

#[cfg(not(target_env = "gnu"))]
fn limit_malloc_arenas() {}

fn main() -> glib::ExitCode {
    let t0 = std::time::Instant::now();
    limit_malloc_arenas();
    let config_path = config::config_dir().join("config.toml");
    let cfg = config::load_or_create(&config_path);

    // Must happen before GTK initialises. With the cairo renderer we also
    // stop GDK from probing GL/Vulkan at all: on X11 it otherwise loads the
    // GPU driver just to pick a visual (~85 MB RSS with NVIDIA).
    if std::env::var_os("GSK_RENDERER").is_none() && !cfg.renderer.trim().is_empty() {
        std::env::set_var("GSK_RENDERER", cfg.renderer.trim());
    }
    let cairo = std::env::var("GSK_RENDERER").is_ok_and(|r| r == "cairo");
    if cairo && std::env::var_os("GDK_DISABLE").is_none() {
        std::env::set_var("GDK_DISABLE", "gl,vulkan");
    }
    if std::env::var_os("GTK_THEME").is_none() {
        std::env::set_var("GTK_THEME", theme::gtk_theme_env(&cfg.theme));
    }
    glib::set_prgname(Some("steamlite"));
    glib::set_application_name("steamlite");

    // No application id: on Wayland the app_id then is the prgname
    // ("steamlite"), matching steamlite.desktop and `[app_id="steamlite"]`.
    let app = gtk::Application::builder().flags(gtk::gio::ApplicationFlags::NON_UNIQUE).build();
    let cfg = std::cell::RefCell::new(Some(cfg));
    app.connect_activate(move |app| {
        let Some(cfg) = cfg.borrow_mut().take() else { return };
        let timing = std::env::var_os("STEAMLITE_TIMING").is_some();
        if timing {
            eprintln!("steamlite: activate at {:.1} ms", t0.elapsed().as_secs_f64() * 1000.0);
        }
        let ui = ui::build(app, cfg, config_path.clone());
        if timing {
            eprintln!("steamlite: ui built at {:.1} ms", t0.elapsed().as_secs_f64() * 1000.0);
        }
        let win = ui.window().clone();
        if std::env::var_os("STEAMLITE_TIMING").is_some() {
            win.connect_map(move |w| {
                let Some(clock) = w.frame_clock() else { return };
                let fired = std::cell::Cell::new(false);
                clock.connect_after_paint(move |_| {
                    if !fired.replace(true) {
                        eprintln!(
                            "steamlite: first frame after {:.1} ms (main) epoch_ms={:.1}",
                            t0.elapsed().as_secs_f64() * 1000.0,
                            epoch_ms()
                        );
                    }
                });
            });
        }
        // The Ui lives as long as the window.
        unsafe { win.set_data("steamlite-ui", ui) };
        win.present();
    });
    app.run_with_args::<&str>(&[])
}
