//! Fuzzy matching and ordering for launcher results.
//!
//! Scores favour, in order: an exact name, a name that starts with the query, a word that starts with
//! the query, a substring, and finally an in-order subsequence ("vsc" -> "Visual Studio Code").

/// How well `query` matches `name`; `None` means no match. Higher is better.
pub fn score(query: &str, name: &str) -> Option<u32> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return None;
    }
    let n = name.to_lowercase();
    if n == q {
        return Some(1000);
    }
    if n.starts_with(&q) {
        // shorter names are closer to the query
        return Some(800 + 100u32.saturating_sub(n.chars().count().saturating_sub(q.chars().count()) as u32).min(100));
    }
    if n.split(|c: char| !c.is_alphanumeric()).any(|w| !w.is_empty() && w.starts_with(&q)) {
        return Some(600);
    }
    if n.contains(&q) {
        return Some(400);
    }
    // subsequence: every query character appears in order; fewer gaps score higher
    let qc: Vec<char> = q.chars().filter(|c| !c.is_whitespace()).collect();
    if qc.len() < 2 {
        return None;
    }
    let mut qi = 0;
    let mut gaps = 0u32;
    let mut last: Option<usize> = None;
    for (i, c) in n.chars().enumerate() {
        if qi < qc.len() && c == qc[qi] {
            if let Some(l) = last {
                if i > l + 1 {
                    gaps += 1;
                }
            }
            last = Some(i);
            qi += 1;
        }
    }
    (qi == qc.len()).then(|| 200u32.saturating_sub(gaps * 20).max(20))
}

/// An item with its final ordering key.
#[derive(Debug, Clone, PartialEq)]
pub struct Ranked<T> {
    pub item: T,
    pub score: u32,
}

/// Rank items by name match plus a caller-supplied boost (pins, recency). Stable for equal scores
/// by name, truncated to `limit`.
pub fn rank<T: Clone>(items: &[T], query: &str, name: impl Fn(&T) -> &str, boost: impl Fn(&T) -> u32, limit: usize) -> Vec<Ranked<T>> {
    let mut out: Vec<Ranked<T>> = items
        .iter()
        .filter_map(|it| score(query, name(it)).map(|s| Ranked { item: it.clone(), score: s + boost(it) }))
        .collect();
    out.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| name(&a.item).to_lowercase().cmp(&name(&b.item).to_lowercase())));
    out.truncate(limit);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn match_tiers_are_ordered() {
        let exact = score("chrome", "Chrome").unwrap();
        let prefix = score("chro", "Chrome").unwrap();
        let word = score("studio", "Visual Studio Code").unwrap();
        let sub = score("sual", "Visual Studio Code").unwrap();
        let fuzzy = score("vsc", "Visual Studio Code").unwrap();
        assert!(exact > prefix && prefix > word && word > sub && sub > fuzzy);
    }

    #[test]
    fn no_match_and_empty_query() {
        assert_eq!(score("zzz", "Chrome"), None);
        assert_eq!(score("", "Chrome"), None);
        assert_eq!(score("   ", "Chrome"), None);
        assert_eq!(score("q", "Chrome"), None);
    }

    #[test]
    fn case_insensitive_and_unicode_safe() {
        assert!(score("CHROME", "chrome").is_some());
        assert!(score("zoë", "Zoë Player").is_some());
        assert!(score("ÉCOLE", "école").is_some());
    }

    #[test]
    fn shorter_prefix_match_wins() {
        assert!(score("note", "Notepad").unwrap() > score("note", "Notepad++ Portable Edition").unwrap());
    }

    #[test]
    fn rank_orders_boosts_and_limits() {
        let apps = vec!["Notepad", "Notes", "OneNote", "Calculator"];
        let r = rank(&apps, "note", |a| a, |a| if *a == "OneNote" { 500 } else { 0 }, 3);
        assert_eq!(r.len(), 3);
        assert_eq!(r[0].item, "OneNote", "a pin/recency boost lifts it above prefix matches");
        assert!(!r.iter().any(|x| x.item == "Calculator"));
        let plain = rank(&apps, "note", |a| a, |_| 0, 10);
        let names: Vec<_> = plain.iter().map(|x| x.item).collect();
        assert_eq!(names[0], "Notes", "shortest prefix match first");
    }

    #[test]
    fn rank_is_deterministic_for_ties() {
        let items = vec!["beta app", "alpha app"];
        let r = rank(&items, "app", |a| a, |_| 0, 10);
        assert_eq!(r[0].item, "alpha app");
    }
}
