// src-tauri/src/post_process/question_inference.rs
//
// Lightweight question classifier and punctuation rewriter.
//
// Design intent: high precision, not recall. Ambiguous constructions are
// left untouched rather than guessing. Logic mirrors the Swift
// QuestionInference in Resonant but is intentionally simpler — it relies on
// the static QUESTION_STARTERS / QUESTION_PHRASES from grammar_sets rather
// than re-implementing full auxiliary-inversion analysis.

use std::ops::Range;

use once_cell::sync::Lazy;
use regex::Regex;

use super::grammar_sets::{QUESTION_PHRASES, QUESTION_STARTERS};

// ---------------------------------------------------------------------------
// Compiled patterns
// ---------------------------------------------------------------------------

/// Matches a leading whole-word question starter (case-insensitive).
static STARTER_RE: Lazy<Regex> = Lazy::new(|| {
    // Build alternation from QUESTION_STARTERS, sorted longest-first so
    // "doesn't" is tried before "does".
    let mut starters: Vec<&str> = QUESTION_STARTERS.to_vec();
    starters.sort_by_key(|s| std::cmp::Reverse(s.len()));
    let alts = starters
        .iter()
        .map(|s| regex::escape(s))
        .collect::<Vec<_>>()
        .join("|");
    Regex::new(&format!(r"(?i)^(?:{alts})\b")).expect("STARTER_RE regex")
});

/// Matches any phrase from QUESTION_PHRASES anywhere in the sentence
/// (case-insensitive, as full words).
static PHRASE_RE: Lazy<Regex> = Lazy::new(|| {
    let mut phrases: Vec<&str> = QUESTION_PHRASES.to_vec();
    phrases.sort_by_key(|s| std::cmp::Reverse(s.len()));
    let alts = phrases
        .iter()
        .map(|s| regex::escape(s))
        .collect::<Vec<_>>()
        .join("|");
    Regex::new(&format!(r"(?i)\b(?:{alts})\b")).expect("PHRASE_RE regex")
});

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Applies question-mark inference to all sentences in `text`.
///
/// * A sentence ending with `.` that classifies as a question has the period
///   replaced by `?`.
/// * If `append_when_unterminated` is true, a sentence that ends with a plain
///   letter/digit and classifies as a question also receives `?`.
/// * A sentence ending with `!` or already `?` is left untouched.
pub fn apply(text: &str, append_when_unterminated: bool) -> String {
    if text.is_empty() {
        return text.to_owned();
    }

    let ranges = semantic_sentence_ranges(text);
    if ranges.is_empty() {
        return text.to_owned();
    }

    let mut out = text.to_owned();

    // Process in reverse so byte offsets of earlier ranges remain valid.
    for range in ranges.into_iter().rev() {
        let sentence = &out[range.clone()];
        let rewritten = infer_sentence(sentence, append_when_unterminated);
        if rewritten != sentence {
            out.replace_range(range, &rewritten);
        }
    }

    out
}

/// Returns `true` when `sentence` appears to be a question.
///
/// Rules (in order):
/// 1. Empty → false.
/// 2. Ends with `!` → false (explicit exclamation, never a question).
/// 3. Already ends with `?` → true.
/// 4. Fewer than 3 whitespace-separated tokens → false (too ambiguous).
/// 5. Strip terminal `.` or `,` before matching (the period may be inferred,
///    not user-chosen, so we test the body for question shape).
/// 6. Starts with a word from QUESTION_STARTERS (case-insensitive whole word).
/// 7. Contains a phrase from QUESTION_PHRASES (case-insensitive).
///
/// Note: a sentence ending with `.` can still return `true` here — `apply`
/// and `infer_sentence` use this to decide whether to replace that period.
/// The caller must not re-append `?` when the sentence already ends with one.
pub fn is_question(sentence: &str) -> bool {
    let trimmed = sentence.trim();
    if trimmed.is_empty() {
        return false;
    }
    let last = trimmed.chars().next_back().unwrap();
    if last == '!' {
        return false;
    }
    if last == '?' {
        return true;
    }
    // Short fragments are too ambiguous.
    if trimmed.split_whitespace().count() < 3 {
        return false;
    }

    // Strip terminal punctuation (. , etc.) before pattern matching so that
    // "Can you verify." and "Can you verify" both hit the same rules.
    let body = trimmed.trim_end_matches(|c: char| !c.is_alphanumeric());

    STARTER_RE.is_match(body) || PHRASE_RE.is_match(body)
}

/// Returns the byte ranges of each semantic sentence in `text`.
///
/// A boundary is a `.`, `!`, or `?` followed by whitespace (or end-of-input),
/// or a `\n` character. Ranges are non-overlapping and trimmed of surrounding
/// whitespace.
pub fn semantic_sentence_ranges(text: &str) -> Vec<Range<usize>> {
    if text.is_empty() {
        return vec![];
    }

    let mut ranges: Vec<Range<usize>> = Vec::new();
    let bytes = text.as_bytes();
    let len = text.len();
    let mut start = 0usize;

    let mut i = 0usize;
    while i < len {
        let ch = text[i..].chars().next().unwrap();
        let ch_len = ch.len_utf8();

        let is_boundary = match ch {
            '\n' => true,
            '.' | '!' | '?' => {
                let after = i + ch_len;
                after >= len || bytes[after].is_ascii_whitespace()
            }
            _ => false,
        };

        if is_boundary {
            // Include the punctuation character itself (but not newline).
            let end = if ch == '\n' { i } else { i + ch_len };
            push_trimmed(text, start, end, &mut ranges);
            // Skip over whitespace after the boundary.
            let mut next = i + ch_len;
            while next < len && bytes[next].is_ascii_whitespace() {
                next += 1;
            }
            start = next;
            i = next;
            continue;
        }

        i += ch_len;
    }

    // Remaining text after the last boundary.
    push_trimmed(text, start, len, &mut ranges);

    ranges
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Core rewriter for a single sentence.
///
/// * If the sentence is a question and ends without any terminal punctuation
///   (or ends with `,`) → strip trailing `,`/whitespace and append `?`.
/// * If `append_when_unterminated` is true and the sentence is a question
///   ending with a letter/digit → append `?`.
/// * If the sentence ends with `.` and is a question → replace `.` with `?`.
/// * Otherwise return unchanged.
fn infer_sentence(sentence: &str, append_when_unterminated: bool) -> String {
    if !is_question(sentence) {
        return sentence.to_owned();
    }
    let last = sentence.trim_end().chars().next_back().unwrap_or('\0');

    match last {
        '?' | '!' => sentence.to_owned(), // already marked or exclamation
        '.' => {
            // Replace terminal period with question mark.
            let trimmed = sentence.trim_end();
            let without_dot = &trimmed[..trimmed.len() - '.'.len_utf8()];
            format!("{without_dot}?")
        }
        ',' => {
            // Remove trailing comma (and any space), then add '?'.
            let stripped = sentence.trim_end().trim_end_matches(',').trim_end();
            format!("{stripped}?")
        }
        c if c.is_alphanumeric() => {
            if append_when_unterminated {
                format!("{}?", sentence.trim_end())
            } else {
                sentence.to_owned()
            }
        }
        _ => sentence.to_owned(),
    }
}

/// Pushes a byte-range `start..end` (trimmed of whitespace) into `ranges`,
/// if it is non-empty.
fn push_trimmed(text: &str, start: usize, end: usize, ranges: &mut Vec<Range<usize>>) {
    if start >= end {
        return;
    }
    let slice = &text[start..end];
    // Trim leading whitespace.
    let leading = slice
        .chars()
        .take_while(|c| c.is_whitespace())
        .map(char::len_utf8)
        .sum::<usize>();
    // Trim trailing whitespace.
    let trailing = slice
        .chars()
        .rev()
        .take_while(|c| c.is_whitespace())
        .map(char::len_utf8)
        .sum::<usize>();
    let s = start + leading;
    let e = end - trailing;
    if s < e {
        ranges.push(s..e);
    }
}

// ---------------------------------------------------------------------------
// Tests — ported from QuestionInferenceTests.swift
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn q(input: &str) -> String {
        apply(input, false)
    }

    fn qu(input: &str) -> String {
        apply(input, true)
    }

    // --- auxiliary inversion ---

    #[test]
    fn can_you_becomes_question() {
        assert_eq!(q("Can you send me the file."), "Can you send me the file?");
    }

    #[test]
    fn is_the_build_green_becomes_question() {
        assert_eq!(q("Is the build green."), "Is the build green?");
    }

    #[test]
    fn negative_contraction_becomes_question() {
        assert_eq!(q("Isn't that the same bug."), "Isn't that the same bug?");
        assert_eq!(q("Didn't you see it."), "Didn't you see it?");
        assert_eq!(q("Doesn't the build pass."), "Doesn't the build pass?");
    }

    // --- do/have inversion ---

    #[test]
    fn did_you_becomes_question() {
        assert_eq!(q("Did you see the message."), "Did you see the message?");
    }

    #[test]
    fn have_you_tried_becomes_question() {
        assert_eq!(q("Have you tried restarting."), "Have you tried restarting?");
        assert_eq!(q("Has the deploy finished."), "Has the deploy finished?");
    }

    // --- wh-openers ---

    #[test]
    fn how_are_you_becomes_question() {
        assert_eq!(q("How are you."), "How are you?");
    }

    #[test]
    fn what_time_is_it_becomes_question() {
        assert_eq!(q("What time is it."), "What time is it?");
    }

    // --- multi-sentence ---

    #[test]
    fn only_question_sentence_flips_in_multi_sentence_text() {
        assert_eq!(
            q("I pushed the fix. Can you verify."),
            "I pushed the fix. Can you verify?"
        );
    }

    #[test]
    fn decimals_are_not_sentence_boundaries() {
        assert_eq!(
            q("It costs 2.5 million. Should we proceed."),
            "It costs 2.5 million. Should we proceed?"
        );
    }

    #[test]
    fn existing_question_mark_untouched() {
        let text = "Are we done?";
        assert_eq!(q(text), text);
    }

    #[test]
    fn plain_statement_untouched() {
        let text = "The deploy finished without errors.";
        assert_eq!(q(text), text);
    }

    // --- append_when_unterminated ---

    #[test]
    fn append_when_unterminated_adds_question_mark() {
        assert_eq!(qu("Can you ship the update"), "Can you ship the update?");
    }

    #[test]
    fn append_when_unterminated_no_double_mark() {
        assert_eq!(qu("Can you ship the update?"), "Can you ship the update?");
    }

    #[test]
    fn append_when_unterminated_no_mark_on_statement() {
        assert_eq!(qu("Ship the update"), "Ship the update");
    }

    #[test]
    fn no_append_without_flag() {
        assert_eq!(q("Can you ship the update"), "Can you ship the update");
    }

    // --- is_question edge cases ---

    #[test]
    fn empty_sentence_is_not_question() {
        assert!(!is_question(""));
    }

    #[test]
    fn ends_with_period_question_detected() {
        // is_question strips the trailing period before classifying —
        // apply() then replaces it with '?'.
        assert!(is_question("Can you do this."));
        // A plain statement is still false even with or without a period.
        assert!(!is_question("The deploy finished without errors."));
        assert!(!is_question("The deploy finished without errors"));
    }

    #[test]
    fn ends_with_exclamation_is_not_question() {
        assert!(!is_question("Can you do this!"));
    }

    #[test]
    fn short_fragment_is_not_question() {
        assert!(!is_question("Can you")); // 2 words
        assert!(!is_question("Who"));
    }

    #[test]
    fn already_ends_with_question_mark() {
        assert!(is_question("Are we done?"));
    }

    // --- semantic_sentence_ranges ---

    #[test]
    fn empty_text_gives_no_ranges() {
        assert!(semantic_sentence_ranges("").is_empty());
    }

    #[test]
    fn single_sentence_with_period() {
        let text = "Hello world.";
        let ranges = semantic_sentence_ranges(text);
        assert_eq!(ranges.len(), 1);
        assert_eq!(&text[ranges[0].clone()], "Hello world.");
    }

    #[test]
    fn two_sentences_split_correctly() {
        let text = "Hello. World.";
        let ranges = semantic_sentence_ranges(text);
        assert_eq!(ranges.len(), 2);
        assert_eq!(&text[ranges[0].clone()], "Hello.");
        assert_eq!(&text[ranges[1].clone()], "World.");
    }

    #[test]
    fn newline_splits_sentences() {
        let text = "First line\nSecond line";
        let ranges = semantic_sentence_ranges(text);
        assert_eq!(ranges.len(), 2);
        assert_eq!(&text[ranges[0].clone()], "First line");
        assert_eq!(&text[ranges[1].clone()], "Second line");
    }

    #[test]
    fn multiline_text_flips_per_line() {
        assert_eq!(
            q("Update is out.\n\nCan you test it."),
            "Update is out.\n\nCan you test it?"
        );
    }

    // --- trailing comma ---

    #[test]
    fn question_ending_with_comma_gets_mark() {
        // "Can you do this," → "Can you do this?"
        assert_eq!(
            apply("Can you do this,", false),
            "Can you do this?"
        );
    }
}
