/// Lightweight fuzzy matching used by the command palette, quick-open and
/// symbol search. Rewards consecutive matches, word boundaries and prefix hits.
pub fn fuzzy_score(query: &str, text: &str) -> Option<(i32, Vec<usize>)> {
    if query.is_empty() {
        return Some((0, vec![]));
    }
    let q: Vec<char> = query.to_lowercase().chars().collect();
    let t: Vec<char> = text.to_lowercase().chars().collect();
    if q.len() > t.len() {
        return None;
    }

    let mut qi = 0usize;
    let mut score = 0i32;
    let mut indices = Vec::with_capacity(q.len());
    let mut prev_match = None::<usize>;

    for (ti, &tc) in t.iter().enumerate() {
        if qi >= q.len() {
            break;
        }
        if tc == q[qi] {
            let mut bonus = 1i32;
            if ti == 0 {
                bonus += 8; // beginning of string
            }
            if let Some(p) = prev_match {
                if ti == p + 1 {
                    bonus += 6; // consecutive
                }
            }
            let prev_char = if ti > 0 { t[ti - 1] } else { ' ' };
            if ti > 0 && (prev_char == '/' || prev_char == '\\' || prev_char == '_' || prev_char == '-' || prev_char == ' ' || prev_char == '.') {
                bonus += 5; // word boundary
            }
            if tc.is_uppercase() && ti > 0 && t[ti - 1].is_lowercase() {
                bonus += 3; // camelCase boundary
            }
            score += bonus;
            indices.push(ti);
            prev_match = Some(ti);
            qi += 1;
        }
    }

    if qi != q.len() {
        return None;
    }
    // Prefer shorter candidates when scores are close.
    score -= (t.len() as i32) / 8;
    Some((score, indices))
}

/// Returns candidate indices with their scores and match positions, best first.
pub fn fuzzy_filter<T: AsRef<str>>(query: &str, items: &[T], limit: usize) -> Vec<(usize, i32, Vec<usize>)> {
    let mut out: Vec<(usize, i32, Vec<usize>)> = items
        .iter()
        .enumerate()
        .filter_map(|(i, item)| fuzzy_score(query, item.as_ref()).map(|(s, m)| (i, s, m)))
        .collect();
    out.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    out.truncate(limit);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_subsequence() {
        let (score, idx) = fuzzy_score("gtof", "Go to File...").unwrap();
        assert!(score > 0);
        assert_eq!(idx.len(), 4);
    }

    #[test]
    fn rejects_missing_characters() {
        assert!(fuzzy_score("zzz", "Go to File").is_none());
    }

    #[test]
    fn filters_and_sorts() {
        let items = ["Go to File", "Go to Line", "Find File"];
        let r = fuzzy_filter("gofile", &items, 10);
        assert!(!r.is_empty());
    }
}
