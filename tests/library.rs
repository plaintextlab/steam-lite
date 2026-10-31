use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use steamlite::{art, config, fuzzy, library, sort, vdf};

const FIX: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/");

fn fixture(name: &str) -> String {
    fs::read_to_string(format!("{FIX}{name}")).unwrap()
}

/// Build a fake home with a native Steam root and a second library.
fn fake_home(tag: &str) -> PathBuf {
    let home = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("home-{tag}"));
    let _ = fs::remove_dir_all(&home);
    let root = home.join(".local/share/Steam");
    let lib2 = home.join("games/SteamLibrary");
    fs::create_dir_all(root.join("steamapps")).unwrap();
    fs::create_dir_all(lib2.join("steamapps")).unwrap();
    fs::create_dir_all(root.join("userdata/123/config")).unwrap();
    fs::create_dir_all(home.join(".steam")).unwrap();
    std::os::unix::fs::symlink(&root, home.join(".steam/steam")).unwrap();

    let lf = fixture("libraryfolders.vdf")
        .replace("@ROOT@", root.to_str().unwrap())
        .replace("@LIB2@", lib2.to_str().unwrap());
    fs::write(root.join("steamapps/libraryfolders.vdf"), lf).unwrap();
    fs::write(root.join("steamapps/appmanifest_228980.acf"), fixture("appmanifest_228980.acf")).unwrap();
    for id in ["105600", "553850", "1493710", "999999"] {
        let f = format!("appmanifest_{id}.acf");
        fs::write(lib2.join("steamapps").join(&f), fixture(&f)).unwrap();
    }
    fs::write(root.join("userdata/123/config/localconfig.vdf"), fixture("localconfig.vdf")).unwrap();
    home
}

#[test]
fn library_folders_current_and_legacy() {
    let cur = vdf::parse(&fixture("libraryfolders.vdf")).unwrap();
    assert_eq!(library::parse_library_folders(&cur), vec![PathBuf::from("@ROOT@"), PathBuf::from("@LIB2@")]);
    let legacy = vdf::parse(&fixture("libraryfolders_legacy.vdf")).unwrap();
    assert_eq!(
        library::parse_library_folders(&legacy),
        vec![PathBuf::from("/mnt/games/SteamLibrary"), PathBuf::from("/media/other")]
    );
}

#[test]
fn manifest_fields() {
    let g = library::parse_manifest(&fixture("appmanifest_553850.acf"), Path::new("/lib")).unwrap();
    assert_eq!(g.appid, 553850);
    assert_eq!(g.size_on_disk, 24150575918);
    assert_eq!(g.state_flags, 6);
    assert!(g.needs_update());
    assert_eq!(g.install_dir, PathBuf::from("/lib/steamapps/common/Helldivers 2"));
    assert!(library::parse_manifest("garbage {", Path::new("/")).is_none());
}

#[test]
fn last_played_from_localconfig() {
    let doc = vdf::parse(&fixture("localconfig.vdf")).unwrap();
    let mut m = HashMap::new();
    library::merge_last_played(&doc, &mut m);
    assert_eq!(m.get(&105600), Some(&1792000000));
    assert_eq!(m.get(&553850), Some(&1000));
    assert_eq!(m.len(), 2);
}

#[test]
fn full_scan() {
    let home = fake_home("scan");
    let scan = library::scan(&home, &[]);
    assert_eq!(scan.roots.len(), 1, "symlinked ~/.steam/steam must be de-duplicated");
    let mut games = scan.games;
    games.sort_by_key(|g| g.appid);
    let ids: Vec<u32> = games.iter().map(|g| g.appid).collect();
    // Proton, redist and the half-downloaded app are gone.
    assert_eq!(ids, vec![105600, 553850]);
    // localconfig is newer than the manifest for Terraria...
    assert_eq!(games[0].last_played, 1792000000);
    // ...but older for Helldivers, so the manifest value wins.
    assert_eq!(games[1].last_played, 1791656262);
}

#[test]
fn scan_without_steam_is_empty() {
    let home = Path::new(env!("CARGO_TARGET_TMPDIR")).join("home-empty");
    let _ = fs::create_dir_all(&home);
    let scan = library::scan(&home, &[]);
    assert!(scan.games.is_empty() && scan.roots.is_empty());
}

#[test]
fn cover_layouts() {
    let cache = Path::new(env!("CARGO_TARGET_TMPDIR")).join("librarycache");
    let _ = fs::remove_dir_all(&cache);
    fs::create_dir_all(cache.join("20/abc")).unwrap();
    fs::create_dir_all(cache.join("30/def")).unwrap();
    fs::create_dir_all(cache.join("40")).unwrap();
    fs::write(cache.join("10_library_600x900.jpg"), b"x").unwrap();
    fs::write(cache.join("20/abc/library_600x900.jpg"), b"x").unwrap();
    fs::write(cache.join("20/header.jpg"), b"x").unwrap();
    fs::write(cache.join("30/def/library_capsule.jpg"), b"x").unwrap();
    fs::write(cache.join("30/library_600x900.jpg"), b"x").unwrap();
    fs::write(cache.join("40/header.jpg"), b"x").unwrap();

    assert_eq!(art::find_cover_in(&cache, 10), Some(cache.join("10_library_600x900.jpg")));
    assert_eq!(art::find_cover_in(&cache, 20), Some(cache.join("20/abc/library_600x900.jpg")));
    // 600x900 beats capsule even when the capsule is in a hashed dir.
    assert_eq!(art::find_cover_in(&cache, 30), Some(cache.join("30/library_600x900.jpg")));
    // Landscape header only → no portrait cover → placeholder.
    assert_eq!(art::find_cover_in(&cache, 40), None);
    assert_eq!(art::find_cover_in(&cache, 50), None);
}

#[test]
fn initials() {
    assert_eq!(art::initials("Kingdom Come: Deliverance II"), "KC");
    assert_eq!(art::initials("GTFO"), "G");
    assert_eq!(art::initials("RV There Yet?"), "RT");
    assert_eq!(art::initials("- 7 Days"), "7D");
}

#[test]
fn fuzzy_ranking() {
    assert!(fuzzy::score("ringe", "ELDEN RING").is_none());
    assert!(fuzzy::score("eldr", "ELDEN RING").is_some());
    assert!(fuzzy::score("e r", "ELDEN RING").is_some());
    assert!(fuzzy::score("zzz", "Terraria").is_none());
    assert_eq!(fuzzy::score("", "anything"), Some(0));
    let s = |q| fuzzy::score(q, "Kingdom Come: Deliverance II").unwrap();
    assert!(s("kcd") > s("kmd"), "word starts should beat scattered letters");
    let a = fuzzy::score("ori", "Ori and the Blind Forest").unwrap();
    let b = fuzzy::score("ori", "Graveyard Keeper Original").unwrap();
    assert!(a > b, "prefix match should win");
}

#[test]
fn sort_and_format() {
    use sort::SortKey;
    let g = |name: &str, lp, size| library::Game {
        appid: 1,
        name: name.into(),
        size_on_disk: size,
        state_flags: 4,
        last_played: lp,
        install_dir: PathBuf::new(),
    };
    let mut v = vec![g("b", 10, 5), g("The A", 30, 1), g("c", 20, 9)];
    v.sort_by(|a, b| SortKey::LastPlayed.compare(a, b));
    assert_eq!(v.iter().map(|g| g.name.as_str()).collect::<Vec<_>>(), ["The A", "c", "b"]);
    v.sort_by(|a, b| SortKey::Name.compare(a, b));
    assert_eq!(v.iter().map(|g| g.name.as_str()).collect::<Vec<_>>(), ["The A", "b", "c"]);
    v.sort_by(|a, b| SortKey::Size.compare(a, b));
    assert_eq!(v.iter().map(|g| g.name.as_str()).collect::<Vec<_>>(), ["c", "b", "The A"]);

    assert_eq!(sort::human_size(831365954), "831 MB");
    assert_eq!(sort::human_size(24150575918), "24.2 GB");
    assert_eq!(sort::relative_time(0, 100), "never played");
    assert_eq!(sort::relative_time(100, 100 + 3 * 86_400), "3d ago");
}

#[test]
fn default_config_matches_builtin() {
    let parsed = config::parse(config::DEFAULT_CONFIG).unwrap();
    let b = config::Config::builtin();
    assert_eq!(format!("{parsed:?}"), format!("{b:?}"));
    // Partial configs fall back per field.
    let partial = config::parse("theme = \"cde\"").unwrap();
    assert_eq!(partial.theme, "cde");
    assert!(partial.close_on_launch);
}

#[test]
fn launch_argv() {
    use steamlite::launcher::{launch_argv, steam_argv};
    use steamlite::library::RootKind;
    let steam = steam_argv("auto", RootKind::Native);
    assert_eq!(launch_argv(&steam, true, &["-x".into()], 42), ["steam", "-applaunch", "42"]);
    assert_eq!(
        launch_argv(&steam, false, &["-cef-disable-gpu-compositing".into()], 42),
        ["steam", "-silent", "-cef-disable-gpu-compositing", "-applaunch", "42"]
    );
    assert_eq!(steam_argv("", RootKind::Flatpak), ["flatpak", "run", "com.valvesoftware.Steam"]);
    assert_eq!(steam_argv("distrobox-host-exec steam", RootKind::Native), ["distrobox-host-exec", "steam"]);
}
