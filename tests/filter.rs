use steamlite::filter::is_game;

#[test]
fn drops_runtimes_and_tools_by_appid() {
    for id in [228980, 1070560, 1391110, 1628350, 4183110, 1493710, 1161040, 1826330, 2805730] {
        assert!(!is_game(id, "Some Name"), "{id} should be filtered");
    }
}

#[test]
fn drops_by_name_pattern_for_unknown_appids() {
    for name in [
        "Proton 11.0",
        "Proton Experimental",
        "Proton Hotfix",
        "Proton-GE",
        "proton",
        "Steam Linux Runtime 5.0 (future)",
        "Steamworks Common Redistributables",
        "Some Game Dedicated Server",
    ] {
        assert!(!is_game(1, name), "{name:?} should be filtered");
    }
}

#[test]
fn keeps_real_games() {
    for (id, name) in [
        (105600, "Terraria"),
        (1245620, "ELDEN RING"),
        (553850, "HELLDIVERS™ 2"),
        (1771300, "Kingdom Come: Deliverance II"),
        (1, "Protonica"),            // starts with "Proton" but isn't one
        (2, "The Proton Collider"),  // contains the word, not a prefix
        (3, "Steamworld Dig"),
    ] {
        assert!(is_game(id, name), "{name:?} should be kept");
    }
}
