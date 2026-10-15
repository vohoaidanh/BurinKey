//! Gõ tắt: a short word typed and ended with a space or punctuation turns
//! into its full text ("vn " → "Việt Nam ").
//!
//! The list is plain text, one `short = full text` per line; `#` starts a
//! comment. Matching ignores case, and the expansion follows how the short
//! word was typed: "vn" → "Việt Nam", "Vn" → "Việt Nam", "VN" → "VIỆT NAM".

use std::collections::HashMap;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Macros {
    by_key: HashMap<String, String>,
}

impl Macros {
    pub fn parse(text: &str) -> Self {
        let by_key = text
            .lines()
            .filter_map(|line| {
                let line = line.trim();
                if line.starts_with('#') {
                    return None;
                }
                let (key, value) = line.split_once('=')?;
                let (key, value) = (key.trim(), value.trim());
                let valid =
                    !key.is_empty() && !value.is_empty() && !key.chars().any(|c| c.is_whitespace());
                valid.then(|| (key.to_lowercase(), value.to_string()))
            })
            .collect();
        Self { by_key }
    }

    pub fn len(&self) -> usize {
        self.by_key.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_key.is_empty()
    }

    /// The full text for `word`, cased like `word`.
    pub fn expand(&self, word: &str) -> Option<String> {
        let full = self.by_key.get(&word.to_lowercase())?;
        let letters: Vec<char> = word.chars().filter(|c| c.is_alphabetic()).collect();
        let all_caps = letters.len() > 1 && letters.iter().all(|c| c.is_uppercase());
        let capitalized = letters.first().is_some_and(|c| c.is_uppercase());
        Some(if all_caps {
            full.to_uppercase()
        } else if capitalized {
            let mut chars = full.chars();
            chars
                .next()
                .map(|first| first.to_uppercase().chain(chars).collect())
                .unwrap_or_default()
        } else {
            full.clone()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_lines_and_skips_junk() {
        let macros = Macros::parse(
            "# comment\nvn = Việt Nam\n  hcm=Hồ Chí Minh  \nbad line\n= no key\nx y = space\n",
        );
        assert_eq!(macros.len(), 2);
        assert_eq!(macros.expand("hcm").as_deref(), Some("Hồ Chí Minh"));
    }

    #[test]
    fn follows_the_typed_case() {
        let macros = Macros::parse("vn = Việt Nam\nko = không");
        assert_eq!(macros.expand("vn").as_deref(), Some("Việt Nam"));
        assert_eq!(macros.expand("VN").as_deref(), Some("VIỆT NAM"));
        assert_eq!(macros.expand("Ko").as_deref(), Some("Không"));
        assert_eq!(macros.expand("ok"), None);
    }
}
