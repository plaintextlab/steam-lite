//! Tiny fuzzy matcher: the query must appear as a case-insensitive
//! subsequence of the candidate. Higher score = better match.

/// Returns None if `query` is not a subsequence of `text`.
pub fn score(query: &str, text: &str) -> Option<i32> {
    let q: Vec<char> = query.chars().filter(|c| !c.is_whitespace()).flat_map(char::to_lowercase).collect();
    if q.is_empty() {
        return Some(0);
    }
    let t: Vec<char> = text.chars().flat_map(char::to_lowercase).collect();

    let mut score = 0;
    let mut qi = 0;
    let mut prev_match: Option<usize> = None;
    for (ti, &c) in t.iter().enumerate() {
        if qi == q.len() {
            break;
        }
        if c != q[qi] {
            continue;
        }
        let word_start = ti == 0 || !t[ti - 1].is_alphanumeric();
        score += 1;
        if word_start {
            score += 8;
        }
        match prev_match {
            Some(p) if p + 1 == ti => score += 5,
            Some(p) => score -= ((ti - p - 1) as i32).min(5),
            None => score -= (ti as i32).min(10),
        }
        prev_match = Some(ti);
        qi += 1;
    }
    if qi < q.len() {
        return None;
    }
    let lq: String = q.iter().collect();
    let lt: String = t.iter().collect();
    if lt.starts_with(&lq) {
        score += 20;
    } else if lt.contains(&lq) {
        score += 10;
    }
    Some(score)
}
