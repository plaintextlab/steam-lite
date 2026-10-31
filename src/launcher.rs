//! Talk to the Steam client: detect it and launch games through it.

use crate::library::RootKind;
use std::fs;
use std::io;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};

/// Running as a Flatpak? Then host processes are invisible in `/proc` and
/// `steam` has to be started on the host through `flatpak-spawn --host`.
pub fn in_flatpak() -> bool {
    Path::new("/.flatpak-info").exists()
}

/// Is the Steam client running? Checks `steam.pid` files first, then falls
/// back to scanning `/proc` for a process named `steam` (cheaper than pgrep).
pub fn steam_running(home: &Path) -> bool {
    if in_flatpak() {
        return Command::new("flatpak-spawn")
            .args(["--host", "pgrep", "-x", "steam"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success());
    }
    let pid_files = [
        home.join(".steam/steam.pid"),
        home.join(".var/app/com.valvesoftware.Steam/.steam/steam.pid"),
    ];
    for f in &pid_files {
        if let Some(pid) = fs::read_to_string(f).ok().and_then(|s| s.trim().parse::<u32>().ok()) {
            if is_steam_pid(pid) {
                return true;
            }
        }
    }
    let Ok(procs) = fs::read_dir("/proc") else { return false };
    procs.flatten().any(|e| {
        e.file_name().to_str().and_then(|s| s.parse::<u32>().ok()).is_some_and(is_steam_pid)
    })
}

fn is_steam_pid(pid: u32) -> bool {
    fs::read_to_string(format!("/proc/{pid}/comm")).is_ok_and(|c| c.trim_end() == "steam")
}

/// Resolve the configured steam command into argv (wrapped in
/// `flatpak-spawn --host` when sandboxed).
pub fn steam_argv(configured: &str, kind: RootKind) -> Vec<String> {
    let argv = configured_argv(configured, kind);
    if in_flatpak() {
        return ["flatpak-spawn", "--host"].iter().map(|s| s.to_string()).chain(argv).collect();
    }
    argv
}

fn configured_argv(configured: &str, kind: RootKind) -> Vec<String> {
    let cmd = configured.trim();
    if cmd.is_empty() || cmd == "auto" {
        return match kind {
            RootKind::Native => vec!["steam".into()],
            RootKind::Flatpak => ["flatpak", "run", "com.valvesoftware.Steam"].map(String::from).to_vec(),
        };
    }
    cmd.split_whitespace().map(String::from).collect()
}

/// Full argv to launch `appid`. If Steam isn't running it is started hidden
/// in the same invocation (`-silent` plus the configured CEF flags), which
/// avoids racing a second `steam` process against the starting one.
pub fn launch_argv(steam: &[String], running: bool, flags: &[String], appid: u32) -> Vec<String> {
    let mut argv = steam.to_vec();
    if !running {
        argv.push("-silent".into());
        argv.extend(flags.iter().cloned());
    }
    argv.push("-applaunch".into());
    argv.push(appid.to_string());
    argv
}

/// Spawn fully detached: own process group, no inherited stdio. A reaper
/// thread collects the exit status so no zombie is left if we keep running.
pub fn spawn_detached(argv: &[String]) -> io::Result<()> {
    if std::env::var_os("STEAMLITE_DRY_RUN").is_some() {
        eprintln!("steamlite: dry run: {argv:?}");
        return Ok(());
    }
    let (prog, args) = argv.split_first().ok_or_else(|| io::Error::other("empty command"))?;
    let mut child = Command::new(prog)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}
