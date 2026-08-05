// src-tauri/src/post_process/grammar_sets.rs
// Shared static word/pattern sets used across pipeline stages.

/// Words that indicate the continuation of a sentence (not sentence starters).
/// Used by pause segmentation to avoid inserting breaks before these.
pub const CONTINUATION_OPENERS: &[&str] = &[
    "and", "but", "or", "nor", "so", "yet", "for",
    "because", "although", "though", "while", "since",
    "unless", "until", "when", "where", "which", "who",
    "that", "if", "as", "than", "then", "also",
];

/// Spoken correction cues — phrases that signal the speaker wants to redo
/// something they just said. Everything from the prior sentence boundary
/// to the end of the cue phrase is a false start candidate.
pub const CORRECTION_CUES: &[&str] = &[
    "scratch that",
    "never mind",
    "forget it",
    "cancel that",
    "delete that",
    "remove that",
    "ignore that",
    "undo that",
    "actually",
    "wait no",
    "no wait",
    "i mean",
    "i meant",
    "let me rephrase",
    "let me try again",
    "strike that",
    "correction",
];

/// English filler words — sounds with no semantic content.
pub const FILLER_WORDS_EN: &[&str] = &[
    "uh", "um", "uhm", "umm", "uhh", "uhhh",
    "ah", "hmm", "hm", "mmm", "mm", "mh",
    "eh", "ehh", "ha",
];

/// Filler words for languages where 'um' and 'ha' are real words.
pub const FILLER_WORDS_PT: &[&str] = &["ahm", "hmm", "mmm", "hm"];
pub const FILLER_WORDS_ES: &[&str] = &["ehm", "mmm", "hmm", "hm"];
pub const FILLER_WORDS_FR: &[&str] = &["euh", "hmm", "hm", "mmm"];
pub const FILLER_WORDS_DE: &[&str] = &["\u{e4}h", "\u{e4}hm", "hmm", "hm", "mmm"];
pub const FILLER_WORDS_FALLBACK: &[&str] = &[
    "uh", "uhm", "umm", "uhh", "uhhh",
    "ah", "hmm", "hm", "mmm", "mm", "mh", "ehh",
];

/// Returns the filler word list for the given BCP-47 language code.
pub fn filler_words_for_lang(lang: &str) -> &'static [&'static str] {
    let base = lang.split(&['-', '_'][..]).next().unwrap_or(lang);
    match base {
        "en" => FILLER_WORDS_EN,
        "pt" => FILLER_WORDS_PT,
        "es" => FILLER_WORDS_ES,
        "fr" => FILLER_WORDS_FR,
        "de" => FILLER_WORDS_DE,
        _ => FILLER_WORDS_FALLBACK,
    }
}

/// Spoken layout commands mapped to their text equivalents.
/// Matched case-insensitively as whole phrases.
pub const SPOKEN_COMMANDS: &[(&str, &str)] = &[
    ("new paragraph", "\n\n"),
    ("new line", "\n"),
    ("period", "."),
    ("full stop", "."),
    ("comma", ","),
    ("exclamation mark", "!"),
    ("exclamation point", "!"),
    ("question mark", "?"),
    ("open bracket", "("),
    ("close bracket", ")"),
    ("open parenthesis", "("),
    ("close parenthesis", ")"),
    ("colon", ":"),
    ("semicolon", ";"),
    ("dash", " - "),
    ("hyphen", "-"),
    ("bullet point", "\n\u{2022} "),
    ("new bullet", "\n\u{2022} "),
    ("tab", "\t"),
    ("open quote", "\u{201c}"),
    ("close quote", "\u{201d}"),
    ("open double quote", "\u{201c}"),
    ("close double quote", "\u{201d}"),
];

/// Question-starting words (English) used by question inference.
pub const QUESTION_STARTERS: &[&str] = &[
    "who", "what", "when", "where", "why", "how",
    "is", "are", "was", "were", "will", "would",
    "can", "could", "should", "shall", "may", "might",
    "do", "does", "did", "have", "has", "had",
    "am", "isn't", "aren't", "wasn't", "weren't",
    "won't", "wouldn't", "can't", "couldn't", "shouldn't",
    "don't", "doesn't", "didn't",
];

/// Phrases that strongly signal a question even mid-sentence.
pub const QUESTION_PHRASES: &[&str] = &[
    "can you", "could you", "would you", "will you",
    "can we", "could we", "should we",
    "do you", "does it", "did you", "have you",
    "is it", "is there", "are there", "was it",
    "how do", "how does", "how can", "how should",
    "what is", "what are", "what does", "what should",
    "why is", "why are", "why does", "why would",
    "when is", "when should", "when can",
    "where is", "where are", "where should",
    "which is", "which should", "which one",
    "who is", "who are", "who should",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filler_words_en_contains_um() {
        assert!(FILLER_WORDS_EN.contains(&"um"));
    }

    #[test]
    fn filler_words_pt_does_not_contain_um() {
        // 'um' means 'a/an' in Portuguese
        assert!(!FILLER_WORDS_PT.contains(&"um"));
    }

    #[test]
    fn filler_words_for_lang_region_code() {
        // pt-BR should resolve to pt
        let words = filler_words_for_lang("pt-BR");
        assert!(!words.contains(&"um"));
    }

    #[test]
    fn spoken_commands_has_new_line() {
        let nl = SPOKEN_COMMANDS.iter().find(|(k, _)| *k == "new line");
        assert!(nl.is_some());
        assert_eq!(nl.unwrap().1, "\n");
    }
}
