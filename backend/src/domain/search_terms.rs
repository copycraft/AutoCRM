//! Turning what someone typed into the search box into match patterns.
//!
//! Every word must match somewhere in a record (AND), in any order. A word matches either
//! literally (case-insensitive, `ILIKE`) or in a folded form: lower-cased, Hungarian accents
//! removed and everything that is not a letter or digit dropped, which is how "abc123" finds
//! the plate "ABC-123", "kovacs" finds "Kovács", and "06 30 123" finds "+36 30 123 4567".
//! The same fold exists in SQL as `search_fold()` (migration 0038); the two must agree.

use super::partner::normalize_phone;

/// Words beyond this many are ignored: a pasted paragraph is not a search.
const MAX_TERMS: usize = 5;

/// Lower-case, strip Hungarian accents, keep ASCII letters and digits only.
pub fn fold(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .filter_map(|c| match c {
            'á' => Some('a'),
            'é' => Some('e'),
            'í' => Some('i'),
            'ó' | 'ö' | 'ő' => Some('o'),
            'ú' | 'ü' | 'ű' => Some('u'),
            c if c.is_ascii_alphanumeric() => Some(c),
            _ => None,
        })
        .collect()
}

/// A search string parsed for the SQL: aligned arrays of literal and folded patterns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchTerms {
    /// `ILIKE` patterns, one per word.
    pub raw: Vec<String>,
    /// Regular expressions against the folded text, one per word: the plain fold, plus the
    /// phone-normalised form for a word that looks like a number. None when a word has
    /// nothing foldable (all punctuation), so only the literal pattern applies. The folded
    /// alphabet is [a-z0-9], so alternation is all the regex syntax there ever is.
    pub norm: Vec<Option<String>>,
    /// The whole input, folded: what exact and prefix matches rank against.
    pub phrase: String,
}

fn like_escape(word: &str) -> String {
    word.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

/// A word that is only digits and phone punctuation, with enough digits to be a number
/// (or a piece of one).
fn phone_like(word: &str) -> bool {
    let digits = word.chars().filter(char::is_ascii_digit).count();
    digits >= 2
        && word
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, '+' | '-' | '(' | ')' | '.' | '/'))
}

impl SearchTerms {
    /// None when there is nothing to search for.
    pub fn parse(input: &str) -> Option<SearchTerms> {
        let words: Vec<&str> = input.split_whitespace().take(MAX_TERMS).collect();
        if words.is_empty() {
            return None;
        }
        // Consecutive number-like words are one number typed with spaces ("06 30 123 4567"),
        // so they are matched together, not as four loose digits.
        let mut groups: Vec<Vec<&str>> = Vec::new();
        for word in words {
            match groups.last_mut() {
                Some(last) if phone_like(word) && phone_like(last[last.len() - 1]) => {
                    last.push(word)
                }
                _ => groups.push(vec![word]),
            }
        }
        let mut raw = Vec::new();
        let mut norm = Vec::new();
        for group in groups {
            let text = group.join(" ");
            raw.push(format!("%{}%", like_escape(&text)));
            let plain = fold(&text);
            let mut alternatives = Vec::new();
            if phone_like(group[0]) {
                // Hungarian prefixes unified: 06 and 0036 both become 36. The plain form
                // stays an alternative: "0610" may be the tail of an order number.
                let phone = normalize_phone(&text);
                if !phone.is_empty() {
                    alternatives.push(phone);
                }
            }
            if !plain.is_empty() && !alternatives.contains(&plain) {
                alternatives.push(plain);
            }
            norm.push(match alternatives.len() {
                0 => None,
                1 => Some(alternatives.remove(0)),
                _ => Some(format!("({})", alternatives.join("|"))),
            });
        }
        Some(SearchTerms {
            raw,
            norm,
            phrase: fold(input),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folding_drops_accents_case_and_punctuation() {
        assert_eq!(fold("Kovács-Őrmező Kft."), "kovacsormezokft");
        assert_eq!(fold("ABC-123"), "abc123");
        assert_eq!(fold("Üveg Ű"), "uvegu");
        assert_eq!(fold("   "), "");
    }

    #[test]
    fn every_word_becomes_its_own_pattern() {
        let t = SearchTerms::parse("  Kovács  sprinter ").unwrap();
        assert_eq!(t.raw, ["%Kovács%", "%sprinter%"]);
        assert_eq!(
            t.norm,
            [Some("kovacs".to_string()), Some("sprinter".to_string())]
        );
        assert_eq!(t.phrase, "kovacssprinter");
    }

    #[test]
    fn like_wildcards_typed_by_the_user_are_literal() {
        let t = SearchTerms::parse("50%_off").unwrap();
        assert_eq!(t.raw, ["%50\\%\\_off%"]);
    }

    #[test]
    fn phone_numbers_unify_their_prefixes() {
        for input in ["+36 30 123 4567", "06 30 123 4567", "0036 30 123 4567"] {
            let t = SearchTerms::parse(input).unwrap();
            // However it is spelled, it is one term that finds the stored number.
            assert_eq!(t.norm.len(), 1, "{input}");
            assert!(
                t.norm[0].as_deref().unwrap().contains("36301234567"),
                "{input}"
            );
        }
        assert_eq!(
            SearchTerms::parse("06301234567").unwrap().norm,
            [Some("(36301234567|06301234567)".to_string())]
        );
        assert_eq!(
            SearchTerms::parse("06 20 111 2222").unwrap().norm,
            [Some("(36201112222|06201112222)".to_string())]
        );
        // A number next to a word stays two terms.
        assert_eq!(SearchTerms::parse("kovacs 30 123").unwrap().norm.len(), 2);
        assert_eq!(
            SearchTerms::parse("0036301234567").unwrap().norm,
            [Some("(36301234567|0036301234567)".to_string())]
        );
        // A short number-like word keeps its plain form too (an order number tail).
        assert_eq!(
            SearchTerms::parse("0610").unwrap().norm,
            [Some("(3610|0610)".to_string())]
        );
    }

    #[test]
    fn nothing_to_search_is_none_and_long_input_is_capped() {
        assert_eq!(SearchTerms::parse("   "), None);
        let t = SearchTerms::parse("a b c d e f g h").unwrap();
        assert_eq!(t.raw.len(), 5);
        // A word with nothing foldable only has its literal pattern.
        let t = SearchTerms::parse("---").unwrap();
        assert_eq!(t.norm, [None]);
    }
}
