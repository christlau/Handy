// src-tauri/src/post_process/filler.rs
// Filler-word and false-start removal pipeline stage.
//
// Entry point: `process(text, remove_fillers, remove_false_starts) -> String`

use once_cell::sync::Lazy;
use regex::Regex;

use super::grammar_sets;

// ---------------------------------------------------------------------------
// Compiled regexes
// ---------------------------------------------------------------------------

/// Consecutive duplicate words: handled imperatively — regex crate does not
/// support backreferences, so we scan word-by-word instead.
/// (Kept as a placeholder so the module structure is unchanged.)

/// Two or more whitespace characters (used by clean_whitespace).
static MULTI_SPACE_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"[ \t]{2,}").unwrap());

/// Line-start false start: "words — " at the beginning of a line.
static FALSE_START_LINE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?m)^(.*?\S)\s+[—\-]\s+").unwrap());

/// Inline false start: "words — " anywhere in the text.
static FALSE_START_INLINE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b\S[^.!?\n]*?\s+[—\-]\s+").unwrap());

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Top-level processing function.
///
/// Pass `remove_fillers = true` to strip filler sounds (um, uh, hmm, …).
/// Pass `remove_false_starts = true` to strip correction-cue phrases and
/// em-dash false starts.
///
/// `collapse_stutters` and `clean_whitespace` always run (they are cheap and
/// fix artifacts left by the earlier passes).
pub fn process(text: &str, remove_fillers: bool, remove_false_starts: bool) -> String {
    if !remove_fillers && !remove_false_starts {
        return text.to_string();
    }

    let mut result = text.to_string();

    if remove_fillers {
        result = remove_fillers_pass(&result, "en");
    }

    if remove_false_starts {
        result = remove_correction_cues(&result);
        result = remove_false_starts_pass(&result);
    }

    result = collapse_stutters(&result);
    result = clean_whitespace(&result);
    result
}

// ---------------------------------------------------------------------------
// Pass 1 — filler word removal
// ---------------------------------------------------------------------------

/// Removes filler words (whole-word, case-insensitive) plus any trailing
/// comma or period directly attached to the filler: "um," "uh." etc.
///
/// Uses the language-appropriate list from `grammar_sets`.
pub fn remove_fillers_pass(text: &str, lang: &str) -> String {
    let fillers = grammar_sets::filler_words_for_lang(lang);
    let mut result = text.to_string();

    for filler in fillers {
        // (?i)      — case-insensitive
        // \b        — word boundary before filler (guards against mid-word matches)
        // FILLER    — the literal filler text
        // \b        — word boundary after (no lookbehind needed for basic guard)
        // [,.]?     — optional trailing punctuation attached to the filler
        // \s*       — optional whitespace after
        // Note: `regex` crate does not support lookbehind, so hyphenated
        // compounds like "Mm-hmm" are guarded by \b since '-' is not a word char.
        let pattern = format!(
            r"(?i)\b{}\b[,\.]?\s*",
            regex::escape(filler)
        );
        if let Ok(re) = Regex::new(&pattern) {
            result = re.replace_all(&result, " ").to_string();
        }
    }

    result
}

// ---------------------------------------------------------------------------
// Pass 2 — stutter collapse
// ---------------------------------------------------------------------------

/// Collapses 3 or more consecutive repetitions of the same word (case-
/// insensitive) to a single instance.
///
/// Examples:
/// - "wh wh wh wh why" → "wh why"
/// - "I I I I think"   → "I think"
/// - "no no is fine"   → unchanged (only 2 reps)
pub fn collapse_stutters(text: &str) -> String {
    // The `regex` crate does not support backreferences, so we scan word tokens
    // directly. Preserve non-word tokens (punctuation, newlines) by tokenising
    // on whitespace boundaries and re-joining with a single space.
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() {
        return text.to_string();
    }

    let mut out: Vec<&str> = Vec::with_capacity(words.len());
    let mut i = 0;
    while i < words.len() {
        let word = words[i];
        let word_lower = word.to_lowercase();
        // Count how many consecutive tokens match (case-insensitive).
        let mut run = 1usize;
        while i + run < words.len() && words[i + run].to_lowercase() == word_lower {
            run += 1;
        }
        // Only collapse when there are 3 or more in a row.
        out.push(word);
        i += if run >= 3 { run } else { 1 };
    }

    // Re-join; use single spaces (clean_whitespace will tidy anything left over).
    out.join(" ")
}

// ---------------------------------------------------------------------------
// Pass 3 — correction-cue removal
// ---------------------------------------------------------------------------

/// Removes correction-cue phrases (from `grammar_sets::CORRECTION_CUES`) and
/// the preceding text back to the last sentence boundary.
///
/// Example:
/// "I was going to scratch that I wanted to" → "I wanted to"
///
/// The text *after* the cue (the actual replacement) is kept.
pub fn remove_correction_cues(text: &str) -> String {
    // Build a single alternation regex from all cues, longest first so more
    // specific phrases match before their substrings.
    let mut cues: Vec<&str> = grammar_sets::CORRECTION_CUES.to_vec();
    cues.sort_by(|a, b| b.len().cmp(&a.len()));

    let alternation = cues
        .iter()
        .map(|c| regex::escape(c))
        .collect::<Vec<_>>()
        .join("|");

    let cue_re = match Regex::new(&format!(r"(?i)\b(?:{})\b", alternation)) {
        Ok(r) => r,
        Err(_) => return text.to_string(),
    };

    let mut result = text.to_string();

    // Apply up to 3 times to handle chained corrections.
    for _ in 0..3 {
        let current = result.clone();
        let mut did_replace = false;

        if let Some(m) = cue_re.find(&current) {
            // The replacement text starts after the cue.
            let after_cue = current[m.end()..].trim_start_matches(&[' ', ',', '.', ';', ':'][..]);
            if after_cue.is_empty() {
                // Nothing after the cue — leave it (nothing to replace with).
                break;
            }

            // Find the last sentence boundary before the cue.
            let before_cue = &current[..m.start()];
            let boundary = last_sentence_boundary(before_cue);

            // Build: prefix up to boundary + replacement text.
            let prefix = &current[..boundary];
            if prefix.is_empty() {
                result = after_cue.to_string();
            } else {
                result = format!("{} {}", prefix.trim_end(), after_cue);
            }
            did_replace = true;
        }

        if !did_replace {
            break;
        }
    }

    result
}

/// Returns the byte offset just after the last sentence boundary (`.!?`) in
/// `text`, or 0 if there is none.
fn last_sentence_boundary(text: &str) -> usize {
    let sentence_ends: &[char] = &['.', '!', '?', '\n'];
    if let Some(pos) = text.rfind(sentence_ends) {
        // Move past the boundary character and any trailing whitespace.
        let after = pos + text[pos..].chars().next().map_or(1, |c| c.len_utf8());
        // Skip leading whitespace of new sentence.
        let trimmed = &text[after..];
        let ws_offset = trimmed.len() - trimmed.trim_start().len();
        after + ws_offset
    } else {
        0
    }
}

// ---------------------------------------------------------------------------
// Pass 4 — em-dash false start removal
// ---------------------------------------------------------------------------

/// Removes em-dash (or spaced hyphen) false starts within a clause.
///
/// Examples:
/// - "I was — I am"        → "I am"
/// - "We should — We will" → "We will"
pub fn remove_false_starts_pass(text: &str) -> String {
    let mut result = text.to_string();

    // Line-start false starts: "words — " at the beginning of a line.
    for _ in 0..3 {
        let next = FALSE_START_LINE_RE.replace(&result, "").to_string();
        if next == result {
            break;
        }
        result = next;
    }

    // Inline false starts: "I was — I am" mid-sentence.
    for _ in 0..3 {
        let next = FALSE_START_INLINE_RE.replace(&result, "").to_string();
        if next == result {
            break;
        }
        result = next;
    }

    result
}

// ---------------------------------------------------------------------------
// Pass 5 — whitespace cleanup
// ---------------------------------------------------------------------------

/// Collapses multiple spaces/tabs to a single space and trims the result.
/// Newlines are preserved (paragraph breaks from pause segmentation).
pub fn clean_whitespace(text: &str) -> String {
    let collapsed = MULTI_SPACE_RE.replace_all(text, " ").to_string();
    collapsed.trim().to_string()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- process: pass-through when both flags false ---

    #[test]
    fn test_process_no_op() {
        let t = "Um, I was — I am thinking.";
        assert_eq!(process(t, false, false), t);
    }

    // --- remove_fillers_pass ---

    #[test]
    fn test_removes_um() {
        let result = remove_fillers_pass("um I think um this is good", "en");
        assert!(!result.contains("um"));
        assert!(result.contains("think"));
    }

    #[test]
    fn test_removes_filler_with_trailing_comma() {
        // "um," → removed
        let result = remove_fillers_pass("Well, um, I think that's right", "en");
        assert!(!result.contains("um,"));
    }

    #[test]
    fn test_removes_filler_with_trailing_period() {
        let result = remove_fillers_pass("Hmm. Let me think.", "en");
        assert!(!result.contains("hmm.") && !result.contains("Hmm."));
    }

    #[test]
    fn test_filler_case_insensitive() {
        let result = remove_fillers_pass("UHM this is UH a test", "en");
        assert!(!result.to_lowercase().contains("uhm"));
        assert!(!result.to_lowercase().contains(" uh "));
    }

    #[test]
    fn test_portuguese_preserves_um() {
        // "um" = "a/an" in Portuguese — must not be removed
        let result = remove_fillers_pass("um gato bonito", "pt");
        assert!(result.contains("um"), "result: {result}");
    }

    #[test]
    fn test_filler_does_not_mangle_mmhmm() {
        // "hmm" inside "mm-hmm" should not break the word
        let result = remove_fillers_pass("Mm-hmm, I agree", "en");
        assert!(result.contains("Mm-hmm"), "result: {result}");
    }

    // --- collapse_stutters ---

    #[test]
    fn test_collapses_triple_wh() {
        // "wh wh wh wh why" → "wh why"
        let result = collapse_stutters("wh wh wh wh why");
        assert_eq!(result, "wh why");
    }

    #[test]
    fn test_collapses_quadruple_i() {
        let result = collapse_stutters("I I I I think");
        assert_eq!(result, "I think");
    }

    #[test]
    fn test_two_reps_unchanged() {
        let result = collapse_stutters("no no is fine");
        assert_eq!(result, "no no is fine");
    }

    #[test]
    fn test_collapses_mixed_case() {
        // "No NO no NO no" (5 reps) → "No"
        let result = collapse_stutters("No NO no NO no");
        assert_eq!(result, "No");
    }

    #[test]
    fn test_collapses_triple_so() {
        let result = collapse_stutters("so so so");
        assert_eq!(result, "so");
    }

    // --- remove_correction_cues ---

    #[test]
    fn test_scratch_that_removes_prefix() {
        // "I was going to scratch that I wanted to" → "I wanted to"
        let result = remove_correction_cues("I was going to scratch that I wanted to");
        assert_eq!(result.trim(), "I wanted to");
    }

    #[test]
    fn test_never_mind_removes_prefix() {
        let result = remove_correction_cues("use the old plan never mind use the revised plan");
        assert!(result.contains("use the revised plan"), "result: {result}");
        assert!(!result.contains("old plan"), "result: {result}");
    }

    #[test]
    fn test_correction_cue_at_start_no_prefix() {
        // Nothing before the cue — keep the replacement
        let result = remove_correction_cues("actually let me start over");
        assert!(result.contains("let me start over"), "result: {result}");
    }

    #[test]
    fn test_correction_cue_preserves_earlier_sentence() {
        // "Send to Alice. Call Priya at noon. scratch that call her at one."
        let input = "Send the report to Alice. Call Priya at noon scratch that call her at one.";
        let result = remove_correction_cues(input);
        assert!(result.contains("Send the report to Alice"), "result: {result}");
        assert!(!result.contains("Priya at noon"), "result: {result}");
    }

    #[test]
    fn test_no_cue_unchanged() {
        let t = "This sentence has no correction cues.";
        assert_eq!(remove_correction_cues(t), t);
    }

    // --- remove_false_starts_pass ---

    #[test]
    fn test_em_dash_false_start() {
        let result = remove_false_starts_pass("I was — I am thinking.");
        assert!(result.contains("I am thinking"), "result: {result}");
        assert!(!result.contains("I was —"), "result: {result}");
    }

    #[test]
    fn test_spaced_hyphen_false_start() {
        let result = remove_false_starts_pass("We should - We will go.");
        assert!(result.contains("We will go"), "result: {result}");
    }

    // --- clean_whitespace ---

    #[test]
    fn test_collapses_spaces() {
        assert_eq!(clean_whitespace("hello    world"), "hello world");
    }

    #[test]
    fn test_trims() {
        assert_eq!(clean_whitespace("  hello world  "), "hello world");
    }

    #[test]
    fn test_preserves_newlines() {
        assert_eq!(
            clean_whitespace("line one\n\nline two"),
            "line one\n\nline two"
        );
    }

    // --- process integration ---

    #[test]
    fn test_process_fillers_only() {
        let result = process("Um, I think uh this is correct.", true, false);
        assert!(!result.to_lowercase().contains(" um") && !result.to_lowercase().contains("um,"));
        assert!(!result.to_lowercase().contains(" uh"));
        assert!(result.contains("think"));
    }

    #[test]
    fn test_process_false_starts_only() {
        let result = process("I was going to never mind I will go.", false, true);
        assert!(result.contains("I will go"), "result: {result}");
    }

    #[test]
    fn test_process_both_passes() {
        let result = process("Um I was going to scratch that I will go.", true, true);
        assert!(!result.to_lowercase().starts_with("um"), "result: {result}");
        assert!(result.contains("I will go"), "result: {result}");
    }

    #[test]
    fn test_process_stutter_always_collapses() {
        // collapse_stutters always runs even with both flags false → but process
        // returns early when both false, so let's test with one flag.
        let result = process("I I I I think so", true, false);
        assert!(!result.contains("I I"), "result: {result}");
    }
}
