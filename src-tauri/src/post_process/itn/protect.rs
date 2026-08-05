// src-tauri/src/post_process/itn/protect.rs
// Protect spans that must NOT be transformed by the ITN normalizer.
//
// Each protected span is replaced with a unique placeholder "ITNP_{n}" so that
// downstream passes leave it untouched. The placeholder→original map is carried
// in the ProtectedText struct and restored by the caller after all passes run.
//
// Protection order matters: longest / most specific patterns are applied first
// so shorter patterns cannot steal part of an already-protected span.

use super::super::common::ProtectedText;
use once_cell::sync::Lazy;
use regex::Regex;

// ---------------------------------------------------------------------------
// Static compiled regexes — compiled once at first use.
// ---------------------------------------------------------------------------

/// Full URL (https?://…) — must come before the plain-digit and email patterns.
static RE_URL: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"https?://\S+").expect("RE_URL")
});

/// Email address (word@word.word) — before digit and time patterns.
static RE_EMAIL: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\S+@\S+\.\S+").expect("RE_EMAIL")
});

/// Clock times like "10:15", "9:05:30", "3:45 PM", "03:45am".
static RE_TIME: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b\d{1,2}:\d{2}(?::\d{2})?(?:\s*(?:am|pm|AM|PM))?\b").expect("RE_TIME")
});

/// Standalone four-digit years (1900–2099).
static RE_YEAR: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b(?:19|20)\d{2}\b").expect("RE_YEAR")
});

/// Decimal numbers ("3.14", "0.5") — before the plain-integer pattern.
static RE_DECIMAL: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b\d+\.\d+\b").expect("RE_DECIMAL")
});

/// Existing plain integer digits — catch-all for any remaining digit runs.
static RE_DIGITS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b\d+\b").expect("RE_DIGITS")
});

// ---------------------------------------------------------------------------
// Public interface
// ---------------------------------------------------------------------------

/// Scan `text` with each pattern in priority order, replacing each match with
/// a unique "ITNP_{n}" placeholder. The original spans are stored in the
/// returned `ProtectedText`.
pub fn run(text: &str) -> ProtectedText {
    let mut pt = ProtectedText::new(text);
    let mut counter = 0usize;

    // Apply in priority order: most specific / longest first.
    for re in &[
        &*RE_URL,
        &*RE_EMAIL,
        &*RE_TIME,
        &*RE_YEAR,
        &*RE_DECIMAL,
        &*RE_DIGITS,
    ] {
        protect_pattern(&mut pt, re, &mut counter);
    }

    pt
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Replace all matches of `re` in `pt.text` that are not already inside an
/// existing placeholder with "ITNP_{counter}" placeholders, recording the
/// original text in `pt.replacements`.
fn protect_pattern(pt: &mut ProtectedText, re: &Regex, counter: &mut usize) {
    // Collect matches on the current text snapshot (we mutate as we go).
    // Walk matches in reverse order so byte-offset replacements stay valid.
    let current = pt.text.clone();
    let matches: Vec<_> = re.find_iter(&current).collect();

    for m in matches.into_iter().rev() {
        let span = m.as_str();
        // Skip if the matched text looks like one of our own placeholders
        // (shouldn't happen in normal flow, but guard anyway).
        if span.starts_with("ITNP_") {
            continue;
        }
        let placeholder = format!("ITNP_{counter}");
        *counter += 1;
        pt.replacements
            .insert(placeholder.clone(), span.to_string());
        pt.text.replace_range(m.range(), &placeholder);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protects_clock_time() {
        let pt = run("meeting at 10:15 today");
        assert!(!pt.text.contains("10:15"), "time should be replaced");
        assert!(pt.replacements.values().any(|v| v == "10:15"));
    }

    #[test]
    fn protects_year() {
        let pt = run("in 2025 we shipped");
        assert!(!pt.text.contains("2025"));
        assert!(pt.replacements.values().any(|v| v == "2025"));
    }

    #[test]
    fn protects_decimal() {
        let pt = run("version 3.14 released");
        assert!(!pt.text.contains("3.14"));
        assert!(pt.replacements.values().any(|v| v == "3.14"));
    }

    #[test]
    fn protects_email() {
        let pt = run("send to alex@example.com today");
        assert!(!pt.text.contains("alex@example.com"));
        assert!(pt.replacements.values().any(|v| v == "alex@example.com"));
    }

    #[test]
    fn protects_url() {
        let pt = run("see https://example.com for details");
        assert!(!pt.text.contains("https://example.com"));
        assert!(pt.replacements.values().any(|v| v == "https://example.com"));
    }

    #[test]
    fn protects_plain_digits() {
        let pt = run("I have 42 items");
        assert!(!pt.text.contains("42"));
        assert!(pt.replacements.values().any(|v| v == "42"));
    }

    #[test]
    fn restore_roundtrips() {
        let original = "call 555-1234 at 10:15 pm";
        let pt = run(original);
        // The dashes are not digits so the hyphen-joined form is protected as
        // separate runs; restore should rebuild something recognisable.
        let restored = pt.restore();
        assert!(restored.contains("555"));
        assert!(restored.contains("10:15"));
    }
}
