// src-tauri/src/post_process/itn/normalize.rs
// Convert number-word phrases in transcription text to digit form.
//
// Design: single left-to-right word scan. Each time we land on a number word
// we greedily consume the phrase and emit the digit string. Non-number words
// are emitted verbatim. This keeps the logic simple, predictable, and fast
// without a full grammar parser.
//
// Supported patterns:
//   • Simple cardinals:  "twenty five" → "25", "forty two" → "42"
//   • Hundreds:          "three hundred" → "300"
//                        "three hundred and twenty five" → "325"
//   • Thousands:         "two thousand" → "2000"
//                        "two thousand and five" → "2005"
//   • Ordinals:          looked up from number_words::ORDINALS
//   • Currency suffix:   "twenty five dollars" → "$25", "forty euros" → "€40"

use super::number_words::{
    ones_int, tens_int, CURRENCY_SUFFIXES, ORDINALS,
};

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Scan `text` word by word, replacing number-word phrases with digit strings.
pub fn run(text: &str) -> String {
    // Split preserving inter-word whitespace so we can reassemble faithfully.
    // We work on a vec of tokens; each token is either a "word" or punctuation.
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() {
        return text.to_string();
    }

    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    let mut first = true;

    while i < words.len() {
        // Try ordinal at current position (multi-word ordinals like
        // "twenty-first" are single tokens after hyphenation).
        if let Some((ordinal_str, advance)) = try_ordinal(&words, i) {
            maybe_space(&mut out, first);
            out.push_str(&ordinal_str);
            i += advance;
            first = false;
            continue;
        }

        // Try cardinal number phrase (may consume multiple words).
        if let Some((value, advance)) = try_cardinal(&words, i) {
            // Look ahead for a currency suffix.
            let next_idx = i + advance;
            if let Some(symbol) = currency_at(&words, next_idx) {
                maybe_space(&mut out, first);
                out.push_str(symbol);
                out.push_str(&value.to_string());
                i = next_idx + 1;
            } else {
                maybe_space(&mut out, first);
                out.push_str(&value.to_string());
                i = next_idx;
            }
            first = false;
            continue;
        }

        // Plain word — emit as-is.
        maybe_space(&mut out, first);
        out.push_str(words[i]);
        i += 1;
        first = false;
    }

    out
}

// ---------------------------------------------------------------------------
// Ordinal matching
// ---------------------------------------------------------------------------

/// Try to match an ordinal at `words[i]`. Ordinals are single tokens (possibly
/// hyphenated like "twenty-first"). Returns (replacement_str, words_consumed).
fn try_ordinal(words: &[&str], i: usize) -> Option<(String, usize)> {
    let word = words[i].to_lowercase();
    // Strip trailing punctuation for matching.
    let cleaned = word.trim_end_matches(|c: char| !c.is_alphanumeric() && c != '-');
    for (key, val) in ORDINALS {
        if *key == cleaned {
            // Preserve any trailing punctuation from the original token.
            let suffix = &words[i][cleaned.len()..];
            return Some((format!("{val}{suffix}"), 1));
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Cardinal phrase parsing
// ---------------------------------------------------------------------------

/// Try to parse a cardinal number phrase starting at `words[i]`.
/// Returns (numeric_value, words_consumed) on success.
///
/// Grammar (simplified, left-to-right greedy):
///   cardinal = [thousands_part] [hundreds_part] [sub_hundred_part]
///   thousands_part  = sub_hundred_part "thousand"
///   hundreds_part   = ones_word "hundred"
///   sub_hundred_part = tens_word [ones_word]  |  ones_word
///   connector = "and" (optional, consumed but ignored)
fn try_cardinal(words: &[&str], start: usize) -> Option<(u64, usize)> {
    let mut i = start;
    let mut total: u64 = 0;
    let mut advanced = false;

    // --- thousands ---
    if let Some((sub, sub_adv)) = try_sub_hundred(words, i) {
        let j = i + sub_adv;
        if word_eq(words, j, "thousand") {
            total += sub * 1_000;
            i = j + 1;
            advanced = true;
            // optional "and"
            if word_eq(words, i, "and") {
                i += 1;
            }
        }
    }

    // --- hundreds ---
    if let Some(h) = ones_int_at(words, i) {
        if word_eq(words, i + 1, "hundred") {
            total += h * 100;
            i += 2;
            advanced = true;
            // optional "and"
            if word_eq(words, i, "and") {
                i += 1;
            }
        }
    }

    // --- sub-hundred (tens + optional ones, or a plain ones/teen) ---
    if let Some((sub, sub_adv)) = try_sub_hundred(words, i) {
        total += sub;
        i += sub_adv;
        advanced = true;
    }

    if advanced {
        Some((total, i - start))
    } else {
        None
    }
}

/// Parse a sub-hundred phrase: tens [ones] | ones_or_teen.
/// Returns (value, words_consumed).
fn try_sub_hundred(words: &[&str], i: usize) -> Option<(u64, usize)> {
    if i >= words.len() {
        return None;
    }
    let w = strip_punct(words[i]);

    // Tens word (20–90) optionally followed by a ones word.
    if let Some(t) = tens_int(&w) {
        if let Some(o) = ones_int_at_strict(words, i + 1) {
            return Some((t + o, 2));
        }
        return Some((t, 1));
    }

    // Ones / teens (0–19).
    if let Some(o) = ones_int(&w) {
        return Some((o, 1));
    }

    None
}

// ---------------------------------------------------------------------------
// Currency lookahead
// ---------------------------------------------------------------------------

/// Return the currency symbol if `words[idx]` is a currency word, else None.
fn currency_at(words: &[&str], idx: usize) -> Option<&'static str> {
    if idx >= words.len() {
        return None;
    }
    let w = strip_punct(words[idx]).to_lowercase();
    CURRENCY_SUFFIXES
        .iter()
        .find(|(k, _)| *k == w.as_str())
        .map(|(_, sym)| *sym)
}

// ---------------------------------------------------------------------------
// Small helpers
// ---------------------------------------------------------------------------

/// Return ones/teens integer value at `words[i]`, or None.
fn ones_int_at(words: &[&str], i: usize) -> Option<u64> {
    if i >= words.len() {
        return None;
    }
    ones_int(&strip_punct(words[i]))
}

/// Like `ones_int_at` but returns None for "oh"/"o"/"zero" (not valid as the
/// hundreds digit — we don't want "oh hundred" to fire).
fn ones_int_at_strict(words: &[&str], i: usize) -> Option<u64> {
    if i >= words.len() {
        return None;
    }
    let w = strip_punct(words[i]).to_lowercase();
    if w == "oh" || w == "o" || w == "zero" {
        return None;
    }
    ones_int(&w)
}

/// True if `words[i]` (case-insensitive, punct-stripped) equals `target`.
fn word_eq(words: &[&str], i: usize, target: &str) -> bool {
    if i >= words.len() {
        return false;
    }
    strip_punct(words[i]).to_lowercase() == target
}

/// Strip leading/trailing punctuation that is not alphanumeric or hyphen.
fn strip_punct(s: &str) -> String {
    s.trim_matches(|c: char| !c.is_alphanumeric() && c != '-')
        .to_string()
}

/// Append a space to `out` unless this is the first token.
fn maybe_space(out: &mut String, first: bool) {
    if !first {
        out.push(' ');
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_tens_ones() {
        assert_eq!(run("twenty five"), "25");
        assert_eq!(run("forty two"), "42");
        assert_eq!(run("ninety nine"), "99");
    }

    #[test]
    fn plain_teen() {
        assert_eq!(run("thirteen"), "13");
        assert_eq!(run("nineteen"), "19");
    }

    #[test]
    fn hundreds() {
        assert_eq!(run("three hundred"), "300");
        assert_eq!(run("three hundred and twenty five"), "325");
        assert_eq!(run("five hundred"), "500");
    }

    #[test]
    fn thousands() {
        assert_eq!(run("two thousand"), "2000");
        assert_eq!(run("two thousand and five"), "2005");
        assert_eq!(run("seventy five thousand"), "75000");
    }

    #[test]
    fn ordinals_basic() {
        assert_eq!(run("first"), "1st");
        assert_eq!(run("second"), "2nd");
        assert_eq!(run("third"), "3rd");
        assert_eq!(run("twenty-first"), "21st");
    }

    #[test]
    fn currency_suffix() {
        assert_eq!(run("twenty five dollars"), "$25");
        assert_eq!(run("forty euros"), "€40");
        assert_eq!(run("one hundred pounds"), "£100");
    }

    #[test]
    fn mixed_sentence() {
        assert_eq!(run("I have twenty one apples"), "I have 21 apples");
    }

    #[test]
    fn plain_words_pass_through() {
        assert_eq!(run("hello world"), "hello world");
    }

    #[test]
    fn large_number() {
        assert_eq!(run("two thousand three hundred and forty five"), "2345");
    }
}
