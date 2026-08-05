// src-tauri/src/post_process/pause_segment.rs
// Inserts sentence/paragraph boundaries and number-list commas derived from
// the speaker's real pauses in the ASR token timing stream.

use super::common::{align_words, TokenTiming};
use super::grammar_sets::CONTINUATION_OPENERS;

pub const SENTENCE_BREAK_THRESHOLD: f64 = 0.8;
pub const PARAGRAPH_BREAK_THRESHOLD: f64 = 2.0;
pub const NUMBER_LIST_THRESHOLD: f64 = 0.4;

/// Spoken number words that can appear as bare list items.
const NUMBER_WORDS: &[&str] = &[
    "zero", "one", "two", "three", "four", "five", "six", "seven", "eight",
    "nine", "ten", "eleven", "twelve", "thirteen", "fourteen", "fifteen",
    "sixteen", "seventeen", "eighteen", "nineteen", "twenty", "thirty",
    "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
];

fn is_number_word(word: &str) -> bool {
    let lower = word.to_lowercase();
    let normalized: String = lower.chars().filter(|c| c.is_alphanumeric()).collect();
    NUMBER_WORDS.contains(&normalized.as_str())
}

fn is_continuation_opener(word: &str) -> bool {
    let normalized: String = word
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect();
    CONTINUATION_OPENERS.contains(&normalized.as_str())
}

fn capitalize_first(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => {
            let upper: String = first.to_uppercase().collect();
            upper + chars.as_str()
        }
    }
}

/// Insert sentence and paragraph boundaries at real speech pauses.
///
/// - gap > 2.0s and next word is not a continuation opener → `\n\n`
/// - gap > 0.8s and next word is not a continuation opener → `". "` and
///   capitalize next word
///
/// Returns `text` unchanged if `timings` is empty or alignment fails.
pub fn segment(text: &str, timings: &[TokenTiming]) -> String {
    if timings.is_empty() {
        return text.to_string();
    }

    let words: Vec<&str> = text
        .split_ascii_whitespace()
        .collect();

    if words.len() < 2 {
        return text.to_string();
    }

    let word_timings = match align_words(&words, timings) {
        Some(wt) => wt,
        None => return text.to_string(),
    };

    let mut output = String::with_capacity(text.len() + 16);
    let mut capitalize_next = false;

    for (i, &word) in words.iter().enumerate() {
        let w = if capitalize_next {
            capitalize_next = false;
            capitalize_first(word)
        } else {
            word.to_string()
        };

        if i + 1 >= words.len() {
            output.push_str(&w);
            break;
        }

        let gap = word_timings[i + 1].start - word_timings[i].end;
        let next_is_opener = is_continuation_opener(words[i + 1]);

        if gap > PARAGRAPH_BREAK_THRESHOLD && !next_is_opener {
            output.push_str(&w);
            output.push_str("\n\n");
            capitalize_next = true;
        } else if gap > SENTENCE_BREAK_THRESHOLD && !next_is_opener {
            output.push_str(&w);
            output.push_str(". ");
            capitalize_next = true;
        } else {
            output.push_str(&w);
            output.push(' ');
        }
    }

    output
}

/// Insert ", " between spoken number words separated by a pause > 0.4s.
///
/// Only applies within runs of consecutive number words; isolated pairs of
/// single-digit candidates are intentionally not special-cased here (kept
/// simple per spec). Returns `text` unchanged if `timings` is empty or
/// alignment fails.
pub fn delimit_number_lists(text: &str, timings: &[TokenTiming]) -> String {
    if timings.is_empty() {
        return text.to_string();
    }

    let words: Vec<&str> = text.split_ascii_whitespace().collect();
    if words.len() < 2 {
        return text.to_string();
    }

    let word_timings = match align_words(&words, timings) {
        Some(wt) => wt,
        None => return text.to_string(),
    };

    let mut result: Vec<String> = words.iter().map(|w| w.to_string()).collect();
    let mut changed = false;

    for i in 0..words.len().saturating_sub(1) {
        if is_number_word(words[i]) && is_number_word(words[i + 1]) {
            let gap = word_timings[i + 1].start - word_timings[i].end;
            if gap > NUMBER_LIST_THRESHOLD {
                result[i].push(',');
                changed = true;
            }
        }
    }

    if changed {
        result.join(" ")
    } else {
        text.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::post_process::common::TokenTiming;

    /// Builds one timing token per word. `gaps_before[i]` is the silence
    /// before word i (first gap is typically 0.0). Word duration is 0.2s.
    fn make_timings(words: &[&str], gaps_before: &[f64]) -> Vec<TokenTiming> {
        assert_eq!(words.len(), gaps_before.len());
        let mut clock = 0.0_f64;
        words
            .iter()
            .zip(gaps_before.iter())
            .map(|(&word, &gap)| {
                clock += gap;
                let t = TokenTiming {
                    token: word.to_string(),
                    start_time: clock,
                    end_time: clock + 0.2,
                };
                clock += 0.2;
                t
            })
            .collect()
    }

    #[test]
    fn empty_timings_returns_input_unchanged_segment() {
        let text = "no timings available here";
        assert_eq!(segment(text, &[]), text);
    }

    #[test]
    fn empty_timings_returns_input_unchanged_delimit() {
        let text = "seven eleven";
        assert_eq!(delimit_number_lists(text, &[]), text);
    }

    #[test]
    fn segment_inserts_sentence_break_at_gap_over_threshold() {
        let words = &["we", "shipped", "the", "feature", "it", "works", "now"];
        let gaps = &[0.0, 0.1, 0.1, 0.1, 0.9, 0.1, 0.1];
        let timings = make_timings(words, gaps);
        let result = segment("we shipped the feature it works now", &timings);
        assert_eq!(result, "we shipped the feature. It works now");
    }

    #[test]
    fn segment_inserts_paragraph_break_at_gap_over_2s() {
        let words = &["that", "wraps", "up", "next", "topic", "is", "hiring"];
        let gaps = &[0.0, 0.1, 0.1, 2.1, 0.1, 0.1, 0.1];
        let timings = make_timings(words, gaps);
        let result = segment("that wraps up next topic is hiring", &timings);
        assert_eq!(result, "that wraps up\n\nNext topic is hiring");
    }

    #[test]
    fn continuation_opener_prevents_break() {
        // "and" is a continuation opener — no break should be inserted
        let words = &["we", "shipped", "the", "feature", "and", "it", "works"];
        let gaps = &[0.0, 0.1, 0.1, 0.1, 0.9, 0.1, 0.1];
        let timings = make_timings(words, gaps);
        let result = segment("we shipped the feature and it works", &timings);
        assert_eq!(result, "we shipped the feature and it works");
    }

    #[test]
    fn delimit_number_lists_inserts_comma_between_paused_numbers() {
        let words = &["seven", "eleven"];
        let gaps = &[0.0, 0.5];
        let timings = make_timings(words, gaps);
        let result = delimit_number_lists("seven eleven", &timings);
        assert_eq!(result, "seven, eleven");
    }

    #[test]
    fn delimit_number_lists_no_comma_below_threshold() {
        let words = &["twenty", "three"];
        let gaps = &[0.0, 0.1]; // fluent compound, gap < threshold
        let timings = make_timings(words, gaps);
        let result = delimit_number_lists("twenty three", &timings);
        assert_eq!(result, "twenty three");
    }

    #[test]
    fn segment_misaligned_timings_returns_text_unchanged() {
        let text = "these words do not match the tokens";
        let timings = vec![
            TokenTiming { token: "completely".to_string(), start_time: 0.0, end_time: 0.2 },
            TokenTiming { token: "different".to_string(), start_time: 0.3, end_time: 0.5 },
        ];
        assert_eq!(segment(text, &timings), text);
    }
}
