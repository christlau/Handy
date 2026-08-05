// src-tauri/src/post_process/punctuation.rs
//
// Spoken-command replacement, final text formatting, and line-break flattening.

use once_cell::sync::Lazy;
use regex::Regex;

use super::grammar_sets::SPOKEN_COMMANDS;

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

pub struct FormattingConfig {
    pub bullet_points_enabled: bool,
    pub ensure_trailing_punctuation: bool,
    pub capitalize_leading_letters: bool,
}

// ---------------------------------------------------------------------------
// Compiled regexes
// ---------------------------------------------------------------------------

/// Matches positions that start a new sentence: after ". ", "! ", "? " or
/// after a newline (optionally followed by a bullet character + space).
/// Capture group 1 holds the leading punctuation+space; group 2 holds the
/// letter to capitalize.
static SENTENCE_START: Lazy<Regex> = Lazy::new(|| {
    // Matches: (sentence-ending punct + space) OR (newline + optional bullet)
    // followed by a lowercase letter we want to uppercase.
    Regex::new(r"([.!?] |\n[•\-\* ]*)([a-z])").unwrap()
});

/// Matches text that ends without terminal punctuation (ignoring trailing
/// whitespace). Used by `ensure_trailing_punctuation`.
static MISSING_TERMINAL: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"[^.!?:;\n\s]\s*$").unwrap()
});

// ---------------------------------------------------------------------------
// process_spoken_commands
// ---------------------------------------------------------------------------

/// Replaces spoken layout commands (e.g. "new line", "comma") with their
/// text equivalents. Processes longest matches first to prevent partial-phrase
/// clobber (e.g. "new paragraph" must fire before any lone "new" entry).
pub fn process_spoken_commands(text: &str) -> String {
    // Sort commands by phrase length descending (longest-first).
    let mut commands: Vec<(&str, &str)> = SPOKEN_COMMANDS.to_vec();
    commands.sort_by(|a, b| b.0.len().cmp(&a.0.len()));

    let mut result = text.to_string();
    for (phrase, replacement) in &commands {
        result = replace_spoken_command(&result, phrase, replacement);
    }
    result
}

/// Case-insensitive whole-phrase replacement. The phrase must be surrounded
/// by whitespace or be at the start/end of the string.
fn replace_spoken_command(text: &str, phrase: &str, replacement: &str) -> String {
    let lower = text.to_lowercase();
    let lower_phrase = phrase.to_lowercase();
    let phrase_len = phrase.len();

    if !lower.contains(lower_phrase.as_str()) {
        return text.to_string();
    }

    let mut result = String::with_capacity(text.len());
    let mut pos = 0;
    let bytes = lower.as_bytes();

    while pos <= lower.len().saturating_sub(phrase_len) {
        if let Some(found) = lower[pos..].find(lower_phrase.as_str()) {
            let match_start = pos + found;
            let match_end = match_start + phrase_len;

            // Verify word-boundary conditions.
            let before_ok = match_start == 0
                || bytes[match_start - 1] == b' '
                || bytes[match_start - 1] == b'\n'
                || bytes[match_start - 1] == b'\t';
            let after_ok = match_end == lower.len()
                || bytes[match_end] == b' '
                || bytes[match_end] == b'\n'
                || bytes[match_end] == b'\t';

            if before_ok && after_ok {
                // Absorb the space that preceded the phrase so we don't leave
                // a doubled space in the output.
                let trim_start = if match_start > 0 && bytes[match_start - 1] == b' ' {
                    match_start - 1
                } else {
                    match_start
                };
                result.push_str(&text[pos..trim_start]);
                result.push_str(replacement);

                // Skip the trailing space after the phrase unless replacement
                // already ends with whitespace (so "comma" → "," not ", ").
                let skip_trailing = if match_end < lower.len()
                    && bytes[match_end] == b' '
                    && !replacement.ends_with(' ')
                    && !replacement.ends_with('\n')
                {
                    1
                } else {
                    0
                };
                pos = match_end + skip_trailing;
            } else {
                // No valid match here; advance one character past this hit.
                result.push_str(&text[pos..match_start + 1]);
                pos = match_start + 1;
            }
        } else {
            break;
        }
    }
    result.push_str(&text[pos..]);
    result
}

// ---------------------------------------------------------------------------
// finalize_formatting
// ---------------------------------------------------------------------------

/// Applies sentence-case capitalization and/or trailing punctuation according
/// to `config`.
pub fn finalize_formatting(text: &str, config: &FormattingConfig) -> String {
    if text.is_empty() {
        return text.to_string();
    }

    let mut result = text.to_string();

    if config.capitalize_leading_letters {
        // Capitalize the very first letter.
        result = capitalize_first(&result);
        // Capitalize the first letter after each sentence boundary.
        result = SENTENCE_START
            .replace_all(&result, |caps: &regex::Captures| {
                let boundary = caps.get(1).map_or("", |m| m.as_str());
                let letter = caps.get(2).map_or("", |m| m.as_str());
                format!("{}{}", boundary, letter.to_uppercase())
            })
            .into_owned();
    }

    if config.ensure_trailing_punctuation {
        let trimmed = result.trim_end();
        if MISSING_TERMINAL.is_match(trimmed) {
            result = format!("{}.", trimmed);
        }
    }

    result
}

/// Capitalizes the very first alphabetic character in `text`.
fn capitalize_first(text: &str) -> String {
    let mut chars = text.char_indices();
    // Find the first alphabetic character.
    while let Some((i, ch)) = chars.next() {
        if ch.is_alphabetic() {
            if ch.is_uppercase() {
                return text.to_string();
            }
            let mut out = String::with_capacity(text.len());
            out.push_str(&text[..i]);
            for c in ch.to_uppercase() {
                out.push(c);
            }
            out.push_str(&text[i + ch.len_utf8()..]);
            return out;
        }
    }
    text.to_string()
}

// ---------------------------------------------------------------------------
// flatten_line_breaks
// ---------------------------------------------------------------------------

/// Flattens multi-line text for single-line paste targets.
/// `\n\n` → `"; "`, `\n` → `" "`.
pub fn flatten_line_breaks(text: &str) -> String {
    text.replace("\n\n", "; ").replace('\n', " ")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- spoken commands -----------------------------------------------------

    #[test]
    fn spoken_command_new_line() {
        assert_eq!(
            process_spoken_commands("hello new line world"),
            "hello\nworld"
        );
    }

    #[test]
    fn spoken_command_comma() {
        assert_eq!(process_spoken_commands("one comma two"), "one,two");
    }

    #[test]
    fn spoken_command_period() {
        assert_eq!(process_spoken_commands("done period next"), "done.next");
    }

    #[test]
    fn spoken_command_new_paragraph() {
        assert_eq!(
            process_spoken_commands("first new paragraph second"),
            "first\n\nsecond"
        );
    }

    #[test]
    fn spoken_command_longest_match_first() {
        // "new paragraph" must fire before any hypothetical "new" entry.
        let result = process_spoken_commands("intro new paragraph body");
        assert!(result.contains("\n\n"), "expected paragraph break, got: {result:?}");
        assert!(!result.contains("new paragraph"));
    }

    #[test]
    fn spoken_command_case_insensitive() {
        assert_eq!(process_spoken_commands("done Period"), "done.");
    }

    #[test]
    fn spoken_command_full_example() {
        let input = "implement login function new line then write tests";
        let output = process_spoken_commands(input);
        assert_eq!(output, "implement login function\nthen write tests");
    }

    // -- capitalization ------------------------------------------------------

    #[test]
    fn capitalize_first_letter() {
        let cfg = FormattingConfig {
            bullet_points_enabled: false,
            ensure_trailing_punctuation: false,
            capitalize_leading_letters: true,
        };
        assert_eq!(finalize_formatting("hello world", &cfg), "Hello world");
    }

    #[test]
    fn capitalize_after_period_space() {
        let cfg = FormattingConfig {
            bullet_points_enabled: false,
            ensure_trailing_punctuation: false,
            capitalize_leading_letters: true,
        };
        assert_eq!(finalize_formatting("hello. world", &cfg), "Hello. World");
    }

    #[test]
    fn capitalize_after_exclamation() {
        let cfg = FormattingConfig {
            bullet_points_enabled: false,
            ensure_trailing_punctuation: false,
            capitalize_leading_letters: true,
        };
        assert_eq!(
            finalize_formatting("great! now ship it", &cfg),
            "Great! Now ship it"
        );
    }

    #[test]
    fn capitalize_after_newline() {
        let cfg = FormattingConfig {
            bullet_points_enabled: false,
            ensure_trailing_punctuation: false,
            capitalize_leading_letters: true,
        };
        assert_eq!(
            finalize_formatting("line one\nline two", &cfg),
            "Line one\nLine two"
        );
    }

    // -- trailing punctuation ------------------------------------------------

    #[test]
    fn adds_trailing_period_when_missing() {
        let cfg = FormattingConfig {
            bullet_points_enabled: false,
            ensure_trailing_punctuation: true,
            capitalize_leading_letters: false,
        };
        assert_eq!(
            finalize_formatting("ship the update", &cfg),
            "ship the update."
        );
    }

    #[test]
    fn no_trailing_period_when_already_present() {
        let cfg = FormattingConfig {
            bullet_points_enabled: false,
            ensure_trailing_punctuation: true,
            capitalize_leading_letters: false,
        };
        assert_eq!(
            finalize_formatting("ship the update.", &cfg),
            "ship the update."
        );
    }

    #[test]
    fn no_trailing_period_after_question_mark() {
        let cfg = FormattingConfig {
            bullet_points_enabled: false,
            ensure_trailing_punctuation: true,
            capitalize_leading_letters: false,
        };
        assert_eq!(finalize_formatting("is this done?", &cfg), "is this done?");
    }

    #[test]
    fn no_trailing_period_after_exclamation() {
        let cfg = FormattingConfig {
            bullet_points_enabled: false,
            ensure_trailing_punctuation: true,
            capitalize_leading_letters: false,
        };
        assert_eq!(finalize_formatting("great!", &cfg), "great!");
    }

    #[test]
    fn no_trailing_period_after_colon() {
        let cfg = FormattingConfig {
            bullet_points_enabled: false,
            ensure_trailing_punctuation: true,
            capitalize_leading_letters: false,
        };
        assert_eq!(finalize_formatting("items:", &cfg), "items:");
    }

    // -- flatten_line_breaks -------------------------------------------------

    #[test]
    fn flatten_double_newline_to_semicolon() {
        assert_eq!(flatten_line_breaks("a\n\nb"), "a; b");
    }

    #[test]
    fn flatten_single_newline_to_space() {
        assert_eq!(flatten_line_breaks("a\nb"), "a b");
    }

    #[test]
    fn flatten_mixed_breaks() {
        assert_eq!(flatten_line_breaks("a\nb\n\nc"), "a b; c");
    }

    #[test]
    fn flatten_no_breaks_unchanged() {
        assert_eq!(flatten_line_breaks("plain text"), "plain text");
    }
}
