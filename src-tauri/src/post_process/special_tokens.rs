// src-tauri/src/post_process/special_tokens.rs
// Strip leaked model control tokens such as <|endoftext|>, <|en|>, <|transcribe|>
// that some Whisper variants emit into the transcription text.

use once_cell::sync::Lazy;
use regex::Regex;

/// Matches Whisper-style special tokens: <|anything|>
static SPECIAL_TOKEN_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"<\|[^|>]+\|>").expect("valid special token regex"));

/// Remove all special tokens from `text`.
/// Fast-path: returns the original string unchanged if no tokens are present.
pub fn strip(text: &str) -> String {
    if !text.contains("<|") {
        return text.to_string();
    }
    let result = SPECIAL_TOKEN_RE.replace_all(text, "");
    // Collapse any double-spaces left by removed tokens
    let result = result.split_whitespace().collect::<Vec<_>>().join(" ");
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_whisper_language_token() {
        assert_eq!(strip("<|en|>Hello world"), "Hello world");
    }

    #[test]
    fn strips_multiple_tokens() {
        assert_eq!(
            strip("<|startoftranscript|><|en|><|transcribe|> Hello"),
            "Hello"
        );
    }

    #[test]
    fn strips_endoftext() {
        assert_eq!(strip("Hello world<|endoftext|>"), "Hello world");
    }

    #[test]
    fn no_tokens_returns_unchanged() {
        let input = "Hello world";
        assert_eq!(strip(input), input);
    }

    #[test]
    fn preserves_real_angle_brackets() {
        // Only <|...|> form is matched, not plain < >
        assert_eq!(strip("a < b > c"), "a < b > c");
    }

    #[test]
    fn collapses_double_space_after_removal() {
        assert_eq!(strip("hello <|en|> world"), "hello world");
    }
}
