//! The one place that decides what is *not* a game.
//!
//! Steam installs runtimes, compatibility tools and redistributables as
//! ordinary apps with their own manifests. We drop them by known appid and,
//! as a fallback for future versions, by name pattern.

/// Known non-game appids (Proton releases, Steam Linux Runtimes, redists).
pub const NON_GAME_APPIDS: &[u32] = &[
    228980,  // Steamworks Common Redistributables
    1070560, // Steam Linux Runtime 1.0 (scout)
    1391110, // Steam Linux Runtime 2.0 (soldier)
    1628350, // Steam Linux Runtime 3.0 (sniper)
    4183110, // Steam Linux Runtime 4.0
    1161040, // Proton BattlEye Runtime
    1826330, // Proton EasyAntiCheat Runtime
    1493710, // Proton Experimental
    2180100, // Proton Hotfix
    858280,  // Proton 3.7
    930400,  // Proton 3.7 Beta
    961940,  // Proton 3.16
    996510,  // Proton 3.16 Beta
    1054830, // Proton 4.2
    1113280, // Proton 4.11
    1245040, // Proton 5.0
    1420170, // Proton 5.13
    1580130, // Proton 6.3
    1887720, // Proton 7.0
    2348590, // Proton 8.0
    2805730, // Proton 9.0
    3658110, // Proton 10.0
];

/// Case-insensitive name prefixes that mark a non-game.
pub const NON_GAME_NAME_PREFIXES: &[&str] = &[
    "proton ",
    "proton-",
    "steam linux runtime",
    "steamworks common redistributables",
    "steamworks shared",
    "steamvr performance test",
];

/// Case-insensitive substrings that mark a non-game anywhere in the name.
pub const NON_GAME_NAME_CONTAINS: &[&str] = &["linux runtime", "dedicated server"];

pub fn is_game(appid: u32, name: &str) -> bool {
    if NON_GAME_APPIDS.contains(&appid) {
        return false;
    }
    let lower = name.trim().to_lowercase();
    if lower == "proton" {
        return false;
    }
    !NON_GAME_NAME_PREFIXES.iter().any(|p| lower.starts_with(p))
        && !NON_GAME_NAME_CONTAINS.iter().any(|p| lower.contains(p))
}
