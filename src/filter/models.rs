use serde::{Deserialize, Serialize};

use crate::error::AppError;

/// Longest search term accepted, in characters.
pub const MAX_SEARCH_LEN: usize = 100;

/// A list's search criteria, kept server-side per session and list (see
/// `filter::services`). `Default` is "no filter". New criteria become new
/// fields with `#[serde(default)]`, so rows stored by older code still load.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListFilter {
    #[serde(default)]
    pub search: Search,
}

impl ListFilter {
    pub fn is_empty(&self) -> bool {
        self.search.is_empty()
    }

    /// The count cache's `filter_key` for this filter on list `base`: equal
    /// filters give equal keys, so a `query_id` minted under other criteria
    /// is a miss and recomputes.
    pub fn cache_key(&self, base: &str) -> String {
        if self.is_empty() {
            return base.to_string();
        }
        let criteria = serde_json::to_string(self).expect("ListFilter serializes");
        format!("{base}{criteria}")
    }
}

/// A normalised free-text search term: trimmed, inner whitespace collapsed
/// to single spaces, at most [`MAX_SEARCH_LEN`] characters. Empty means
/// "match everything". Stored as a plain string and re-validated on load.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Search(String);

impl Search {
    pub fn new(raw: &str) -> Result<Self, AppError> {
        let term = raw.split_whitespace().collect::<Vec<_>>().join(" ");
        if term.chars().count() > MAX_SEARCH_LEN {
            return Err(AppError::Validation(format!(
                "Search must be at most {MAX_SEARCH_LEN} characters."
            )));
        }
        Ok(Search(term))
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// An `ILIKE` "contains" pattern, or `None` with no term. `\`, `%` and
    /// `_` are escaped (Postgres's default `LIKE` escape is `\`), so they
    /// match literally.
    pub fn like_pattern(&self) -> Option<String> {
        if self.is_empty() {
            return None;
        }
        let mut pattern = String::with_capacity(self.0.len() + 2);
        pattern.push('%');
        for ch in self.0.chars() {
            if matches!(ch, '\\' | '%' | '_') {
                pattern.push('\\');
            }
            pattern.push(ch);
        }
        pattern.push('%');
        Some(pattern)
    }

    /// The term as an order number (`"12"` or `"#12"`), if it is one.
    pub fn order_id(&self) -> Option<i64> {
        let digits = self.0.strip_prefix('#').unwrap_or(&self.0);
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        digits.parse().ok()
    }
}

impl TryFrom<String> for Search {
    type Error = AppError;

    fn try_from(raw: String) -> Result<Self, Self::Error> {
        Search::new(&raw)
    }
}

impl From<Search> for String {
    fn from(search: Search) -> Self {
        search.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn search(raw: &str) -> Search {
        Search::new(raw).unwrap()
    }

    #[test]
    fn trims_and_collapses_whitespace() {
        assert_eq!(search("  ada \t  love\nlace ").as_str(), "ada love lace");
        assert!(search("   ").is_empty());
        assert!(search("").is_empty());
    }

    #[test]
    fn rejects_terms_over_the_limit() {
        let ok = "é".repeat(MAX_SEARCH_LEN);
        assert_eq!(search(&ok).as_str(), ok);
        let too_long = "a".repeat(MAX_SEARCH_LEN + 1);
        assert!(matches!(
            Search::new(&too_long),
            Err(AppError::Validation(_))
        ));
    }

    #[test]
    fn like_pattern_escapes_wildcards() {
        assert_eq!(search("").like_pattern(), None);
        assert_eq!(search("ada").like_pattern().unwrap(), "%ada%");
        assert_eq!(
            search("50%_off\\").like_pattern().unwrap(),
            r"%50\%\_off\\%"
        );
    }

    #[test]
    fn order_id_accepts_plain_and_hash_numbers() {
        assert_eq!(search("12").order_id(), Some(12));
        assert_eq!(search("#12").order_id(), Some(12));
        assert_eq!(search(" 12 ").order_id(), Some(12));
        assert_eq!(search("12a").order_id(), None);
        assert_eq!(search("#").order_id(), None);
        assert_eq!(search("ada").order_id(), None);
        assert_eq!(search("99999999999999999999").order_id(), None);
    }

    #[test]
    fn cache_key_is_stable_and_differs_per_term() {
        let ada = ListFilter {
            search: search("ada"),
        };
        assert_eq!(
            ada.cache_key("customers"),
            ada.clone().cache_key("customers")
        );
        assert_ne!(
            ada.cache_key("customers"),
            ListFilter {
                search: search("bob")
            }
            .cache_key("customers")
        );
        assert_eq!(ListFilter::default().cache_key("customers"), "customers");
        assert_ne!(ada.cache_key("customers"), "customers");
    }

    #[test]
    fn stored_json_round_trips_and_tolerates_old_or_new_shapes() {
        let ada = ListFilter {
            search: search("ada"),
        };
        let json = serde_json::to_string(&ada).unwrap();
        assert_eq!(json, r#"{"search":"ada"}"#);
        assert_eq!(serde_json::from_str::<ListFilter>(&json).unwrap(), ada);
        // A row from before a field existed, or with a field since removed.
        assert_eq!(
            serde_json::from_str::<ListFilter>("{}").unwrap(),
            ListFilter::default()
        );
        assert_eq!(
            serde_json::from_str::<ListFilter>(r#"{"search":"ada","status":"paid"}"#).unwrap(),
            ada
        );
        // Stored terms are re-validated, not trusted.
        let long = format!(r#"{{"search":"{}"}}"#, "a".repeat(MAX_SEARCH_LEN + 1));
        assert!(serde_json::from_str::<ListFilter>(&long).is_err());
    }
}
