// src-tauri/src/post_process/itn/number_words.rs
// Single source of truth for spoken-number word tables.
// Mirrors Resonant's NumberWords.swift.

// ---------------------------------------------------------------------------
// Core tables
// ---------------------------------------------------------------------------

/// Ones: zero through nineteen (including the spoken zero forms "oh" / "o").
pub static ONES: &[(&str, &str)] = &[
    ("zero", "0"),
    ("oh", "0"),
    ("o", "0"),
    ("one", "1"),
    ("two", "2"),
    ("three", "3"),
    ("four", "4"),
    ("five", "5"),
    ("six", "6"),
    ("seven", "7"),
    ("eight", "8"),
    ("nine", "9"),
    ("ten", "10"),
    ("eleven", "11"),
    ("twelve", "12"),
    ("thirteen", "13"),
    ("fourteen", "14"),
    ("fifteen", "15"),
    ("sixteen", "16"),
    ("seventeen", "17"),
    ("eighteen", "18"),
    ("nineteen", "19"),
];

/// Tens: twenty through ninety.
pub static TENS: &[(&str, &str)] = &[
    ("twenty", "20"),
    ("thirty", "30"),
    ("forty", "40"),
    ("fifty", "50"),
    ("sixty", "60"),
    ("seventy", "70"),
    ("eighty", "80"),
    ("ninety", "90"),
];

/// Ordinals — longest forms first to prevent prefix matches.
pub static ORDINALS: &[(&str, &str)] = &[
    ("twenty-first", "21st"),
    ("twenty-second", "22nd"),
    ("twenty-third", "23rd"),
    ("twenty-fourth", "24th"),
    ("twenty-fifth", "25th"),
    ("twenty-sixth", "26th"),
    ("twenty-seventh", "27th"),
    ("twenty-eighth", "28th"),
    ("twenty-ninth", "29th"),
    ("thirtieth", "30th"),
    ("fortieth", "40th"),
    ("fiftieth", "50th"),
    ("sixtieth", "60th"),
    ("seventieth", "70th"),
    ("eightieth", "80th"),
    ("ninetieth", "90th"),
    ("hundredth", "100th"),
    ("thousandth", "1000th"),
    ("nineteenth", "19th"),
    ("eighteenth", "18th"),
    ("seventeenth", "17th"),
    ("sixteenth", "16th"),
    ("fifteenth", "15th"),
    ("fourteenth", "14th"),
    ("thirteenth", "13th"),
    ("twelfth", "12th"),
    ("eleventh", "11th"),
    ("tenth", "10th"),
    ("ninth", "9th"),
    ("eighth", "8th"),
    ("seventh", "7th"),
    ("sixth", "6th"),
    ("fifth", "5th"),
    ("fourth", "4th"),
    ("third", "3rd"),
    ("second", "2nd"),
    ("first", "1st"),
    ("twentieth", "20th"),
];

/// Currency symbols keyed on the spoken word (singular and plural).
pub static CURRENCY_SUFFIXES: &[(&str, &str)] = &[
    ("dollars", "$"),
    ("dollar", "$"),
    ("euros", "\u{20ac}"),
    ("euro", "\u{20ac}"),
    ("pounds", "\u{a3}"),
    ("pound", "\u{a3}"),
    ("yen", "\u{a5}"),
    ("yuan", "\u{a5}"),
];

// ---------------------------------------------------------------------------
// Numeric value helpers (integer form)
// ---------------------------------------------------------------------------

/// Ones/teens integer value (0–19, including "oh"/"o" → 0).
pub static ONES_INT: &[(&str, u64)] = &[
    ("zero", 0),
    ("oh", 0),
    ("o", 0),
    ("one", 1),
    ("two", 2),
    ("three", 3),
    ("four", 4),
    ("five", 5),
    ("six", 6),
    ("seven", 7),
    ("eight", 8),
    ("nine", 9),
    ("ten", 10),
    ("eleven", 11),
    ("twelve", 12),
    ("thirteen", 13),
    ("fourteen", 14),
    ("fifteen", 15),
    ("sixteen", 16),
    ("seventeen", 17),
    ("eighteen", 18),
    ("nineteen", 19),
];

/// Tens integer values (20–90).
pub static TENS_INT: &[(&str, u64)] = &[
    ("twenty", 20),
    ("thirty", 30),
    ("forty", 40),
    ("fifty", 50),
    ("sixty", 60),
    ("seventy", 70),
    ("eighty", 80),
    ("ninety", 90),
];

// ---------------------------------------------------------------------------
// Lookup functions
// ---------------------------------------------------------------------------

/// Lookup a word in the ONES table, case-insensitive.
/// Covers 0–19 plus "oh" and "o" (spoken zero forms).
pub fn lookup_ones(word: &str) -> Option<&'static str> {
    let lower = word.to_lowercase();
    ONES.iter()
        .find(|(k, _)| *k == lower.as_str())
        .map(|(_, v)| *v)
}

/// Lookup a word in the TENS table, case-insensitive.
pub fn lookup_tens(word: &str) -> Option<&'static str> {
    let lower = word.to_lowercase();
    TENS.iter()
        .find(|(k, _)| *k == lower.as_str())
        .map(|(_, v)| *v)
}

/// Returns true if the word is any cardinal number word (ones, teens, or tens).
pub fn is_number_word(word: &str) -> bool {
    let lower = word.to_lowercase();
    ONES.iter().any(|(k, _)| *k == lower.as_str())
        || TENS.iter().any(|(k, _)| *k == lower.as_str())
}

/// Integer value for a ones/teens word (0–19). Returns None if not found.
pub fn ones_int(word: &str) -> Option<u64> {
    let lower = word.to_lowercase();
    ONES_INT
        .iter()
        .find(|(k, _)| *k == lower.as_str())
        .map(|(_, v)| *v)
}

/// Integer value for a tens word (20–90). Returns None if not found.
pub fn tens_int(word: &str) -> Option<u64> {
    let lower = word.to_lowercase();
    TENS_INT
        .iter()
        .find(|(k, _)| *k == lower.as_str())
        .map(|(_, v)| *v)
}
