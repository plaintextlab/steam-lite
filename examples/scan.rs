//! Print what steamlite would show, and how long the scan takes.
fn main() {
    let home = steamlite::config::home_dir();
    let t = std::time::Instant::now();
    let scan = steamlite::library::scan(&home, &[]);
    let el = t.elapsed();
    let roots: Vec<_> = scan.roots.iter().map(|r| r.path.clone()).collect();
    let mut games = scan.games;
    games.sort_by(|a, b| steamlite::sort::SortKey::LastPlayed.compare(a, b));
    for g in &games {
        let art = steamlite::art::find_cover(&roots, g.appid);
        println!("{:>8} {:<45} {:>9} {}", g.appid, g.name, steamlite::sort::human_size(g.size_on_disk),
            art.map(|p| p.display().to_string()).unwrap_or_else(|| "(placeholder)".into()));
    }
    eprintln!("{} games, scan took {:.1} ms", games.len(), el.as_secs_f64() * 1000.0);
}
