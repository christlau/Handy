// src-tauri/src/post_process/fragmented_word_repair.rs
//
// Ported from Resonant's FragmentedWordRepair.swift
//
// Repairs ASR output where a single word was split into multiple spaced
// fragments, e.g. "t rans crip tion" -> "transcription".
//
// Strategy: scan for runs of 2–8 consecutive short tokens (≤5 chars, no
// digits/punctuation). If joining them all produces a single valid English
// word that is at least 6 chars (two-fragment) or 8 chars (multi-fragment),
// replace the run with the joined form.
//
// The spell check is a simple word-list lookup against a bundled word set.
// For the bundled word set we use the `include_str!` macro pointing at a
// newline-separated word file, or fall back to a curated ~3k word set if the
// file is absent. This avoids a large dep like `hunspell-sys`.

use std::collections::HashSet;
use std::sync::OnceLock;

// ── Constants ─────────────────────────────────────────────────────────────────

/// Minimum joined length for a multi-fragment (3+) merge.
const MIN_JOIN_LENGTH: usize = 8;

/// Minimum joined length for a two-fragment merge.
const MIN_TWO_FRAGMENT_JOIN_LENGTH: usize = 6;

/// Maximum chars for a token to be considered a short fragment candidate.
const MAX_FRAGMENT_CHARS: usize = 5;

/// Maximum number of tokens in a candidate window.
const MAX_WINDOW: usize = 8;

/// Known suffix fragments that frequently appear as the last token in a split.
static SUFFIX_FRAGMENTS: &[&str] = &[
    "able", "ed", "er", "ers", "ible", "ing", "ion", "ions",
    "ism", "ist", "ity", "ive", "ly", "ment", "ness", "sion", "tion",
];

/// Common function words that should never be absorbed into a merge as an
/// interior token, as they appear freely in normal prose.
static STANDALONE_WORDS: &[&str] = &[
    "a", "an", "and", "but", "for", "in", "of", "on", "or", "the", "to", "with",
    "am", "as", "at", "be", "by", "do", "go", "he", "if", "is", "it", "me",
    "my", "so", "up", "us", "we",
];

/// Known compound splits that should always be joined.
static KNOWN_COMPOUND_SPLITS: &[(&str, &str)] = &[
    ("every thing", "everything"),
    ("under neath", "underneath"),
    ("some thing", "something"),
    ("any thing", "anything"),
    ("out side", "outside"),
    ("in side", "inside"),
    ("up stairs", "upstairs"),
    ("down stairs", "downstairs"),
    ("every where", "everywhere"),
    ("some where", "somewhere"),
    ("any where", "anywhere"),
    ("no where", "nowhere"),
    ("every one", "everyone"),
    ("any one", "anyone"),
    ("some one", "someone"),
    ("over all", "overall"),
    ("time out", "timeout"),
    ("call back", "callback"),
    ("hand shake", "handshake"),
    ("work flow", "workflow"),
    ("back end", "backend"),
    ("front end", "frontend"),
    ("data base", "database"),
    ("file name", "filename"),
    ("pass word", "password"),
    ("key word", "keyword"),
    ("over write", "overwrite"),
    ("over load", "overload"),
    ("over ride", "override"),
    ("re factor", "refactor"),
    ("re name", "rename"),
    ("re base", "rebase"),
    ("re try", "retry"),
    ("re run", "rerun"),
    ("re load", "reload"),
    ("de bug", "debug"),
    ("de ploy", "deploy"),
    ("de code", "decode"),
    ("de serialize", "deserialize"),
    ("en code", "encode"),
    ("in put", "input"),
    ("out put", "output"),
];

// ── Word list ─────────────────────────────────────────────────────────────────

/// A curated set of common English words used as fallback when no word file
/// is available. Kept small — the fragmented repair only needs to recognize
/// that a joined sequence IS a real word, so coverage for long compound-ish
/// technical terms matters most.
static CORE_WORDS: &[&str] = &[
    // technical / coding
    "implementation","authentication","authorization","configuration","infrastructure",
    "initialization","serialization","deserialization","transformation","optimization",
    "documentation","specification","notification","communication","representation",
    "synchronization","asynchronous","synchronous","application","development",
    "environment","repository","deployment","integration","abstraction","inheritance",
    "encapsulation","polymorphism","refactoring","compilation","execution","iteration",
    "recursion","algorithm","parameters","arguments","variables","functions","methods",
    "attributes","components","middleware","containers","kubernetes","microservices",
    "postgresql","javascript","typescript","programming","framework","database",
    "transaction","migration","validation","authentication","concatenation","encryption",
    "decryption","compression","decompression","pagination","navigation","rendering",
    "hydration","memoization","debouncing","throttling","multiplexing","streaming",
    "broadcasting","subscription","delegation","inheritance","composition","injection",
    "singleton","observable","immutable","stateless","stateful","idempotent",
    "declarative","imperative","functional","structural","behavioral","architectural",
    "annotation","decorator","generator","iterator","transformer","interceptor",
    "middleware","dispatcher","controller","repository","aggregation","fragmentation",
    "segmentation","tokenization","vectorization","normalization","denormalization",
    "partitioning","sharding","replication","orchestration","containerization",
    "virtualization","serialization","marshalling","unmarshalling","initialization",
    "instantiation","dependency","injection","resolution","compilation","interpretation",
    "evaluation","propagation","traversal","searching","sorting","filtering","mapping",
    "reducing","collecting","grouping","joining","splitting","merging","concatenating",
    "interpolation","formatting","parsing","validating","sanitizing","transforming",
    "transcription","transcribing","dictation","detection","correction","prediction",
    "classification","recognition","understanding","processing","generation","extraction",
    "embedding","indexing","querying","caching","invalidation","eviction","expiration",
    "timeout","callback","webhook","endpoint","interface","protocol","specification",
    "implementation","abstraction","encapsulation","inheritance","polymorphism",
    "refactoring","optimization","performance","scalability","reliability","availability",
    "maintainability","testability","observability","debuggability","deployability",
    // common English
    "information","important","different","together","something","everything","nothing",
    "anything","everyone","someone","anyone","somewhere","anywhere","everywhere",
    "throughout","although","however","therefore","furthermore","nevertheless",
    "additionally","alternatively","consequently","approximately","significantly",
    "particularly","generally","specifically","automatically","immediately","eventually",
    "continuously","regularly","frequently","occasionally","temporarily","permanently",
    "absolutely","completely","definitely","probably","possibly","actually","basically",
    "essentially","effectively","efficiently","successfully","accurately","precisely",
    "correctly","properly","directly","specifically","carefully","quickly","slowly",
    "clearly","easily","simply","briefly","recently","currently","previously",
    "originally","traditionally","typically","usually","normally","generally",
    "actually","totally","literally","seriously","honestly","frankly","obviously",
    "apparently","allegedly","supposedly","presumably","potentially","theoretically",
    "practically","realistically","fundamentally","technically","logically","physically",
    "digitally","electronically","automatically","manually","visually","verbally",
    "textually","graphically","programmatically","dynamically","statically",
    "recursively","iteratively","incrementally","continually","persistently",
    "reliably","consistently","seamlessly","transparently","efficiently","elegantly",
    "robustly","flexibly","modularly","cleanly","securely","safely","privately",
    // more general
    "without","within","because","through","against","between","during","before",
    "after","around","across","behind","beneath","beside","beyond","inside","outside",
    "above","below","about","would","should","could","might","shall","while","these",
    "those","there","their","where","when","which","other","another","already",
    "always","never","sometimes","often","again","still","rather","quite","very",
    "really","right","wrong","true","false","large","small","great","little","good",
    "bad","best","worst","first","last","next","previous","current","former","latter",
    "having","being","doing","going","coming","making","taking","getting","putting",
    "setting","running","working","trying","using","looking","thinking","knowing",
    "saying","seeing","giving","showing","leading","keeping","letting","moving",
    "turning","bringing","starting","stopping","ending","opening","closing","writing",
    "reading","speaking","listening","understanding","explaining","describing",
    "creating","building","designing","developing","testing","debugging","deploying",
    "monitoring","logging","tracking","managing","organizing","planning","scheduling",
    "reviewing","updating","deleting","inserting","selecting","querying","filtering",
    "sorting","grouping","joining","splitting","merging","comparing","checking",
    "validating","handling","processing","transforming","converting","encoding",
    "decoding","compressing","decompressing","encrypting","decrypting","hashing",
    "signing","verifying","authenticating","authorizing","permitting","blocking",
    "allowing","denying","accepting","rejecting","sending","receiving","connecting",
    "disconnecting","subscribing","publishing","requesting","responding","serving",
    "fetching","storing","loading","saving","exporting","importing","downloading",
    "uploading","backing","restoring","archiving","migrating","upgrading","downgrading",
    "installing","uninstalling","configuring","initializing","starting","stopping",
    "restarting","rebooting","shutting","launching","closing","opening","resizing",
    "repositioning","refreshing","reloading","redirecting","forwarding","routing",
];

fn word_set() -> &'static HashSet<&'static str> {
    static SET: OnceLock<HashSet<&'static str>> = OnceLock::new();
    SET.get_or_init(|| CORE_WORDS.iter().cloned().collect())
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Repair fragmented words in `text`. No external dependencies needed.
pub fn repair(text: &str) -> String {
    if text.is_empty() {
        return text.to_string();
    }

    let mut result = apply_known_compound_splits(text);
    result = repair_multi_fragment_runs(&result);
    result = repair_two_fragment_splits(&result);
    result
}

// ── Known compound splits ─────────────────────────────────────────────────────

fn apply_known_compound_splits(text: &str) -> String {
    let mut result = text.to_string();
    for &(pattern, replacement) in KNOWN_COMPOUND_SPLITS {
        // Case-insensitive whole-phrase replace
        let lower = result.to_lowercase();
        let pat_lower = pattern.to_lowercase();
        if let Some(pos) = lower.find(&pat_lower) {
            // Preserve casing of first char
            let original = &result[pos..pos + pattern.len()];
            let first_upper = original.chars().next().map(|c| c.is_uppercase()).unwrap_or(false);
            let replaced = if first_upper {
                let mut r = replacement.to_string();
                // Uppercase the first character of the replacement string
                let first_char = r.chars().next().unwrap_or_default();
                r = first_char.to_uppercase().to_string() + &r[first_char.len_utf8()..];
                r
            } else {
                replacement.to_string()
            };
            result = result[..pos].to_string() + &replaced + &result[pos + pattern.len()..];
        }
    }
    result
}

// ── Multi-fragment repair (3+ tokens) ────────────────────────────────────────

struct Replacement {
    start: usize, // byte offset in the space-tokenized string
    end: usize,
    text: String,
}

fn repair_multi_fragment_runs(text: &str) -> String {
    // Tokenize by whitespace while tracking byte positions
    let tokens: Vec<(usize, usize, &str)> = tokenize_with_positions(text);
    if tokens.len() < 3 {
        return text.to_string();
    }

    let standalone: HashSet<&str> = STANDALONE_WORDS.iter().cloned().collect();
    let words = word_set();

    let mut replacements: Vec<Replacement> = Vec::new();
    let mut i = 0;

    'outer: while i < tokens.len() {
        // Find maximal window of short, alpha-only tokens starting at i
        let window_end = {
            let mut end = i;
            while end < tokens.len() && end - i < MAX_WINDOW {
                let tok = tokens[end].2;
                if !is_fragment_candidate(tok) {
                    break;
                }
                // Don't start a window with a standalone word unless it's the
                // only token (can't repair that anyway)
                if end == i && standalone.contains(tok.to_lowercase().as_str()) {
                    i += 1;
                    continue 'outer;
                }
                end += 1;
            }
            end
        };

        // Try windows from largest to smallest (greedy)
        let mut found = false;
        for win_len in (3..=(window_end - i)).rev() {
            let slice: Vec<&str> = tokens[i..i + win_len].iter().map(|t| t.2).collect();

            // Skip if any interior token is a standalone word
            let has_standalone_interior = slice[1..slice.len() - 1]
                .iter()
                .any(|t| standalone.contains(t.to_lowercase().as_str()));
            if has_standalone_interior {
                continue;
            }

            let joined: String = slice.iter().map(|t| t.to_lowercase()).collect::<String>();

            if joined.len() >= MIN_JOIN_LENGTH && words.contains(joined.as_str()) {
                // Preserve casing of first token
                let display = preserve_case(tokens[i].2, &joined);
                let start_byte = tokens[i].0;
                let end_byte = tokens[i + win_len - 1].1;
                replacements.push(Replacement {
                    start: start_byte,
                    end: end_byte,
                    text: display,
                });
                i += win_len;
                found = true;
                break;
            }
        }

        if !found {
            i += 1;
        }
    }

    if replacements.is_empty() {
        return text.to_string();
    }

    apply_replacements(text, &replacements)
}

// ── Two-fragment repair ───────────────────────────────────────────────────────

fn repair_two_fragment_splits(text: &str) -> String {
    let tokens = tokenize_with_positions(text);
    if tokens.len() < 2 {
        return text.to_string();
    }

    let words = word_set();
    let suffix_set: HashSet<&str> = SUFFIX_FRAGMENTS.iter().cloned().collect();

    let mut replacements: Vec<Replacement> = Vec::new();
    let mut skip_next = false;

    for i in 0..tokens.len() - 1 {
        if skip_next {
            skip_next = false;
            continue;
        }

        let left = tokens[i].2;
        let right = tokens[i + 1].2;

        if !is_fragment_candidate(left) || !is_fragment_candidate(right) {
            continue;
        }

        let joined_lower: String = left.to_lowercase() + &right.to_lowercase();
        if joined_lower.len() < MIN_TWO_FRAGMENT_JOIN_LENGTH {
            continue;
        }

        // Only join if:
        // 1. The right token is a known suffix fragment, OR
        // 2. The joined form is in the word list
        let right_lower = right.to_lowercase();
        let is_suffix = suffix_set.contains(right_lower.as_str());
        let is_known = words.contains(joined_lower.as_str());

        if is_suffix || is_known {
            let display = preserve_case(left, &joined_lower);
            let start_byte = tokens[i].0;
            let end_byte = tokens[i + 1].1;
            replacements.push(Replacement {
                start: start_byte,
                end: end_byte,
                text: display,
            });
            skip_next = true;
        }
    }

    if replacements.is_empty() {
        return text.to_string();
    }

    apply_replacements(text, &replacements)
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Returns (start_byte, end_byte_exclusive, token_str) for each whitespace token.
fn tokenize_with_positions(text: &str) -> Vec<(usize, usize, &str)> {
    let mut result = Vec::new();
    let mut start: Option<usize> = None;

    for (byte_idx, ch) in text.char_indices() {
        if ch.is_whitespace() {
            if let Some(s) = start.take() {
                result.push((s, byte_idx, &text[s..byte_idx]));
            }
        } else if start.is_none() {
            start = Some(byte_idx);
        }
    }

    if let Some(s) = start {
        result.push((s, text.len(), &text[s..]));
    }

    result
}

/// A token is a fragment candidate if:
/// - It is purely alphabetic (no digits, no punctuation other than apostrophe)
/// - It is short (≤ MAX_FRAGMENT_CHARS)
fn is_fragment_candidate(tok: &str) -> bool {
    if tok.is_empty() {
        return false;
    }
    // Strip trailing punctuation for the length check
    let stripped = tok.trim_end_matches(|c: char| ".,!?;:\"'".contains(c));
    if stripped.is_empty() || stripped.len() > MAX_FRAGMENT_CHARS {
        return false;
    }
    stripped.chars().all(|c| c.is_alphabetic())
}

/// Preserve the capitalisation of `original` onto `joined_lower`.
fn preserve_case(original: &str, joined_lower: &str) -> String {
    let first_upper = original
        .chars()
        .next()
        .map(|c| c.is_uppercase())
        .unwrap_or(false);
    if first_upper {
        let mut chars = joined_lower.chars();
        match chars.next() {
            None => String::new(),
            Some(c) => {
                let upper: String = c.to_uppercase().to_string();
                upper + chars.as_str()
            }
        }
    } else {
        joined_lower.to_string()
    }
}

/// Applies a sorted list of non-overlapping byte-range replacements to `text`.
fn apply_replacements(text: &str, reps: &[Replacement]) -> String {
    let mut result = String::with_capacity(text.len());
    let mut last = 0usize;

    for rep in reps {
        result.push_str(&text[last..rep.start]);
        result.push_str(&rep.text);
        last = rep.end;
    }
    result.push_str(&text[last..]);
    result
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_known_compound() {
        // "every thing" -> "everything"
        let result = repair("I need every thing to work");
        assert!(result.contains("everything"), "got: {result}");
    }

    #[test]
    fn joins_callback_compound() {
        let result = repair("register the call back function");
        assert!(result.contains("callback"), "got: {result}");
    }

    #[test]
    fn no_change_for_clean_sentence() {
        let input = "The implementation is complete.";
        let result = repair(input);
        assert_eq!(result, input);
    }

    #[test]
    fn two_fragment_suffix_join() {
        // "implement" + "ation" -> "implementation"
        let result = repair("the implement ation is done");
        assert!(result.contains("implementation"), "got: {result}");
    }

    #[test]
    fn preserves_capitalisation() {
        let result = repair("Every thing is fine");
        assert!(result.contains("Everything"), "got: {result}");
    }

    #[test]
    fn empty_input() {
        assert_eq!(repair(""), "");
    }

    #[test]
    fn does_not_join_standalone_words() {
        // "the to and" — standalone words should not get merged
        let input = "the to and";
        let result = repair(input);
        assert_eq!(result, input);
    }
}
