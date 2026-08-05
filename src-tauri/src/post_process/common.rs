// src-tauri/src/post_process/common.rs
// Shared types used across all post-processing pipeline stages.

use std::collections::HashMap;

/// Per-token timing from the ASR decoder.
/// start_time and end_time are in seconds from start of audio.
#[derive(Debug, Clone, PartialEq)]
pub struct TokenTiming {
    pub token: String,
    pub start_time: f64,
    pub end_time: f64,
}

/// Derived word-level timing (aggregated from token timings).
#[derive(Debug, Clone, PartialEq)]
pub struct WordTiming {
    pub start: f64,
    pub end: f64,
}

/// Text with placeholder-protected spans (e.g. protected times/dates for ITN).
#[derive(Debug, Clone)]
pub struct ProtectedText {
    pub text: String,
    pub replacements: HashMap<String, String>,
}

impl ProtectedText {
    pub fn new(text: &str) -> Self {
        Self {
            text: text.to_string(),
            replacements: HashMap::new(),
        }
    }

    /// Replace the placeholders back with original spans.
    pub fn restore(self) -> String {
        let mut result = self.text;
        for (placeholder, original) in &self.replacements {
            result = result.replace(placeholder.as_str(), original.as_str());
        }
        result
    }
}

/// Strip punctuation from a word for alignment matching.
pub fn normalize_for_alignment(word: &str) -> String {
    word.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// Attempt to align a slice of whitespace-split words to token timings.
/// Returns one WordTiming per word if alignment succeeds, or None if
/// timings are empty or the word count does not match.
pub fn align_words(words: &[&str], timings: &[TokenTiming]) -> Option<Vec<WordTiming>> {
    if timings.is_empty() || words.is_empty() {
        return None;
    }

    // Normalize token text for matching (strip leading space marker)
    let token_keys: Vec<String> = timings
        .iter()
        .map(|t| {
            let s = t.token.trim_start_matches('\u{2581}'); // SentencePiece space marker
            normalize_for_alignment(s)
        })
        .collect();

    let word_keys: Vec<String> = words.iter().map(|w| normalize_for_alignment(w)).collect();

    // Simple greedy forward scan: for each word, consume tokens whose
    // normalized text contributes to the word key.
    let mut result = Vec::with_capacity(words.len());
    let mut ti = 0usize;

    for wk in &word_keys {
        if ti >= token_keys.len() {
            return None;
        }
        let start = timings[ti].start_time;
        let mut matched = String::new();
        while ti < token_keys.len() && matched.len() < wk.len() {
            matched.push_str(&token_keys[ti]);
            ti += 1;
            if matched.len() >= wk.len() {
                break;
            }
        }
        let end = if ti > 0 { timings[ti - 1].end_time } else { start };
        result.push(WordTiming { start, end });
    }

    if result.len() == words.len() {
        Some(result)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_strips_punctuation_and_lowercases() {
        assert_eq!(normalize_for_alignment("Hello,"), "hello");
        assert_eq!(normalize_for_alignment("it's"), "its");
        assert_eq!(normalize_for_alignment("42"), "42");
    }

    #[test]
    fn protected_text_restores_replacements() {
        let mut pt = ProtectedText::new("call PLACEHOLDER_0 okay");
        pt.replacements.insert("PLACEHOLDER_0".to_string(), "555-1234".to_string());
        assert_eq!(pt.restore(), "call 555-1234 okay");
    }

    #[test]
    fn align_words_empty_timings_returns_none() {
        assert!(align_words(&["hello"], &[]).is_none());
    }
}
