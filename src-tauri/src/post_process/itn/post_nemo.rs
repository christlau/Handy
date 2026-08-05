// src-tauri/src/post_process/itn/post_nemo.rs
// Post-NeMo cleanup passes.  Runs after NeMo (or its stub) has finished.
//
// Passes:
//   1. Percent normalization  — "25 percent" / "25 %" → "25%"
//   2. Phone number grouping  — spaced digit sequences → hyphen-grouped form
//   3. Double-space collapse  — artifacts from span removals / replacements

use once_cell::sync::Lazy;
use regex::Regex;

// --- compiled regexes -------------------------------------------------------

/// "25 percent"  or  "25 %" (with optional space before the symbol).
static RE_PERCENT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(\d+)\s+(?:percent|%)").expect("RE_PERCENT")
});

/// A sequence of single space-separated digits at least 7 characters long.
/// Captures the whole run so we can reformat it.
static RE_DIGIT_SEQ: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b(\d(?:\s+\d){6,})\b").expect("RE_DIGIT_SEQ")
});

/// Two or more consecutive spaces.
static RE_DOUBLE_SPACE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"  +").expect("RE_DOUBLE_SPACE")
});

// ----------------------------------------------------------------------------

/// Apply all post-NeMo cleanup passes in order and return the cleaned string.
pub fn run(text: &str) -> String {
    let s = normalize_percent(text);
    let s = format_phone_sequences(&s);
    collapse_spaces(&s)
}

// --- pass 1: percent --------------------------------------------------------

fn normalize_percent(text: &str) -> String {
    RE_PERCENT.replace_all(text, "${1}%").into_owned()
}

// --- pass 2: phone grouping -------------------------------------------------

/// Reformat a spaced-digit run into a hyphen-grouped phone number.
/// Supported lengths: 7 → NXX-XXXX, 10 → NXX-NXX-XXXX, 11 → N NXX-NXX-XXXX.
fn format_phone_sequences(text: &str) -> String {
    RE_DIGIT_SEQ.replace_all(text, |caps: &regex::Captures| {
        let run = &caps[1];
        // Collect the individual digits from the spaced run.
        let digits: Vec<char> = run
            .split_whitespace()
            .filter_map(|t| t.chars().next())
            .collect();

        match digits.len() {
            7 => format!(
                "{}{}{}-{}{}{}{}",
                digits[0], digits[1], digits[2],
                digits[3], digits[4], digits[5], digits[6]
            ),
            10 => format!(
                "{}{}{}-{}{}{}-{}{}{}{}",
                digits[0], digits[1], digits[2],
                digits[3], digits[4], digits[5],
                digits[6], digits[7], digits[8], digits[9]
            ),
            11 => format!(
                "{} {}{}{}-{}{}{}-{}{}{}{}",
                digits[0],
                digits[1], digits[2], digits[3],
                digits[4], digits[5], digits[6],
                digits[7], digits[8], digits[9], digits[10]
            ),
            // Unknown length — leave unchanged.
            _ => run.to_string(),
        }
    }).into_owned()
}

// --- pass 3: double-space collapse ------------------------------------------

fn collapse_spaces(text: &str) -> String {
    RE_DOUBLE_SPACE.replace_all(text, " ").into_owned()
}

// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_word() {
        assert_eq!(normalize_percent("25 percent"), "25%");
        assert_eq!(normalize_percent("99 percent off"), "99% off");
    }

    #[test]
    fn percent_symbol_with_space() {
        assert_eq!(normalize_percent("25 %"), "25%");
    }

    #[test]
    fn phone_seven_digit() {
        assert_eq!(format_phone_sequences("5 5 5 1 2 3 4"), "555-1234");
    }

    #[test]
    fn phone_ten_digit() {
        assert_eq!(format_phone_sequences("5 5 5 1 2 3 4 5 6 7"), "555-123-4567");
    }

    #[test]
    fn phone_eleven_digit() {
        assert_eq!(format_phone_sequences("1 5 5 5 1 2 3 4 5 6 7"), "1 555-123-4567");
    }

    #[test]
    fn double_space_collapse() {
        assert_eq!(collapse_spaces("hello  world"), "hello world");
        assert_eq!(collapse_spaces("a   b   c"), "a b c");
    }

    #[test]
    fn full_pipeline() {
        assert_eq!(run("25 percent  off"), "25% off");
    }
}
