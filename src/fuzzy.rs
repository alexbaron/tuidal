//! fzf-style fuzzy matching over the visible list, backed by nucleo-matcher.

use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};

#[derive(Debug, Clone)]
pub struct FuzzyMatch {
    /// Index into the original (unfiltered) list.
    pub index: usize,
    pub score: u32,
    /// Matched char positions in the haystack string (for highlighting).
    pub indices: Vec<u32>,
}

/// Rank `haystack` items against `needle` (case-insensitive, fzf-like smart
/// matching). Returns matches sorted by descending score; an empty needle
/// returns all items in original order with no indices.
pub fn rank<'a, I: IntoIterator<Item = &'a str>>(needle: &str, haystack: I) -> Vec<FuzzyMatch> {
    let items: Vec<&str> = haystack.into_iter().collect();
    if needle.is_empty() {
        return items
            .iter()
            .enumerate()
            .map(|(i, _)| FuzzyMatch {
                index: i,
                score: 0,
                indices: Vec::new(),
            })
            .collect();
    }
    let mut matcher = Matcher::new(Config::DEFAULT);
    let pattern = Pattern::parse(needle, CaseMatching::Ignore, Normalization::Smart);
    let mut buf = Vec::new();
    let mut out: Vec<FuzzyMatch> = items
        .iter()
        .enumerate()
        .filter_map(|(i, item)| {
            buf.clear();
            let mut indices = Vec::new();
            pattern
                .indices(Utf32Str::new(item, &mut buf), &mut matcher, &mut indices)
                .map(|score| FuzzyMatch {
                    index: i,
                    score,
                    indices,
                })
        })
        .collect();
    out.sort_by_key(|m| std::cmp::Reverse(m.score));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_needle_returns_all_in_order() {
        let m = rank("", ["b", "a", "c"]);
        assert_eq!(m.len(), 3);
        assert_eq!(
            m.iter().map(|x| x.index).collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
        assert!(m.iter().all(|x| x.indices.is_empty()));
    }

    #[test]
    fn exact_and_prefix_beat_scattered() {
        let m = rank(
            "massive attack",
            [
                "My Abysmal Scatter of Tacks", // scattered
                "Massive Attack Essentials",   // prefix
                "Massive Attack",              // exact
            ],
        );
        // The scattered candidate must not outrank the exact/prefix ones —
        // nucleo either rejects it or scores it far lower.
        assert!(m.len() >= 2, "got {m:?}");
        let top: Vec<usize> = m[..2].iter().map(|x| x.index).collect();
        assert!(top.contains(&1) && top.contains(&2), "got {m:?}");
        if let Some(last) = m.get(2) {
            assert_eq!(last.index, 0, "got {m:?}");
            assert!(last.score < m[0].score, "got {m:?}");
        }
    }

    #[test]
    fn indices_land_on_matched_chars() {
        let m = rank("mss atk", ["Massive Attack"]);
        assert_eq!(m.len(), 1);
        let chars: Vec<char> = "Massive Attack".chars().collect();
        let needle: Vec<char> = "mss atk".chars().filter(|c| *c != ' ').collect();
        assert_eq!(m[0].indices.len(), needle.len());
        // Indices are strictly increasing and each hits the right char
        // (the needle order is preserved in the haystack positions).
        for w in m[0].indices.windows(2) {
            assert!(w[0] < w[1]);
        }
        for (i, pos) in m[0].indices.iter().enumerate() {
            let got = chars[*pos as usize].to_ascii_lowercase();
            // Position hits the i-th needle char OR a better-scoring earlier
            // occurrence the matcher preferred; it must always be a needle char.
            assert!(
                needle[i] == got || needle.contains(&got),
                "index {pos} hit '{got}', expected around '{needle:?}'"
            );
        }
    }
}
