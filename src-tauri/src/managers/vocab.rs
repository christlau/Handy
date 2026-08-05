use anyhow::{anyhow, Result};
use chrono::Utc;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashSet;
use tauri::AppHandle;

// ── Structs ───────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct VocabTerm {
    pub id: i64,
    pub term: String,
    pub term_lower: String,
    pub source: String,
    pub weight: f64,
    pub sightings: i64,
    pub suppressed: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct CorrectionEntry {
    pub id: i64,
    pub original_span: String,
    pub corrected_span: String,
    pub times_seen: i64,
    pub last_seen_at: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct TranscriptionEdit {
    pub history_entry_id: i64,
    pub original_text: String,
    pub edited_text: String,
}

// ── Row mappers ───────────────────────────────────────────────────────────────

fn map_vocab_term(row: &rusqlite::Row<'_>) -> rusqlite::Result<VocabTerm> {
    Ok(VocabTerm {
        id: row.get("id")?,
        term: row.get("term")?,
        term_lower: row.get("term_lower")?,
        source: row.get("source")?,
        weight: row.get("weight")?,
        sightings: row.get("sightings")?,
        suppressed: row.get::<_, i64>("suppressed")? != 0,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn map_correction_entry(row: &rusqlite::Row<'_>) -> rusqlite::Result<CorrectionEntry> {
    Ok(CorrectionEntry {
        id: row.get("id")?,
        original_span: row.get("original_span")?,
        corrected_span: row.get("corrected_span")?,
        times_seen: row.get("times_seen")?,
        last_seen_at: row.get("last_seen_at")?,
    })
}

// ── DB connection ─────────────────────────────────────────────────────────────

/// Opens the shared history.db (same file as HistoryManager).
pub fn open_db(app: &AppHandle) -> Result<Connection> {
    let app_data_dir = crate::portable::app_data_dir(app)?;
    let db_path = app_data_dir.join("history.db");
    Ok(Connection::open(db_path)?)
}

// ── Vocabulary terms CRUD ─────────────────────────────────────────────────────

/// List all vocab terms ordered by weight DESC, then term_lower ASC.
pub fn list_terms(conn: &Connection) -> Result<Vec<VocabTerm>> {
    let mut stmt = conn.prepare(
        "SELECT id, term, term_lower, source, weight, sightings, suppressed, created_at, updated_at
         FROM vocab_terms
         ORDER BY weight DESC, term_lower ASC",
    )?;
    let terms = stmt
        .query_map([], map_vocab_term)?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(terms)
}

/// Add a manual vocabulary term. Returns the inserted row.
/// Minimum 2 chars (after trimming). Errors if term already exists.
pub fn add_term(conn: &Connection, term: &str) -> Result<VocabTerm> {
    let term = term.trim();
    if term.len() < 2 {
        return Err(anyhow!("Term must be at least 2 characters"));
    }
    let term_lower = term.to_lowercase();
    let now = Utc::now().timestamp();

    conn.execute(
        "INSERT INTO vocab_terms (term, term_lower, source, weight, sightings, suppressed, created_at, updated_at)
         VALUES (?1, ?2, 'manual', 1.0, 0, 0, ?3, ?3)",
        params![term, term_lower, now],
    )?;

    let id = conn.last_insert_rowid();
    let inserted = conn.query_row(
        "SELECT id, term, term_lower, source, weight, sightings, suppressed, created_at, updated_at
         FROM vocab_terms WHERE id = ?1",
        params![id],
        map_vocab_term,
    )?;
    Ok(inserted)
}

/// Suppress a term (soft-delete: keeps the row, marks suppressed=1).
pub fn suppress_term(conn: &Connection, id: i64) -> Result<()> {
    let now = Utc::now().timestamp();
    let affected = conn.execute(
        "UPDATE vocab_terms SET suppressed = 1, updated_at = ?1 WHERE id = ?2",
        params![now, id],
    )?;
    if affected == 0 {
        return Err(anyhow!("vocab_term id {} not found", id));
    }
    Ok(())
}

/// Permanently delete a term row.
pub fn delete_term(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM vocab_terms WHERE id = ?1", params![id])?;
    Ok(())
}

// ── Correction log ────────────────────────────────────────────────────────────

/// List recent correction_log entries ordered by last_seen_at DESC.
pub fn list_corrections(conn: &Connection, limit: i64) -> Result<Vec<CorrectionEntry>> {
    let mut stmt = conn.prepare(
        "SELECT id, original_span, corrected_span, times_seen, last_seen_at
         FROM correction_log
         ORDER BY last_seen_at DESC
         LIMIT ?1",
    )?;
    let entries = stmt
        .query_map(params![limit], map_correction_entry)?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(entries)
}

// ── Boost terms for Whisper ───────────────────────────────────────────────────

/// Returns active (non-suppressed, weight >= 0.5) terms sorted by weight DESC, capped at 50.
/// Passed as initial prompt tokens to the Whisper decoder.
pub fn active_boost_terms(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT term FROM vocab_terms
         WHERE suppressed = 0 AND weight >= 0.5
         ORDER BY weight DESC
         LIMIT 50",
    )?;
    let terms = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(terms)
}

// ── Edit diffing & learning ───────────────────────────────────────────────────

/// Minimum word length to track in correction learning.
const MIN_WORD_LEN: usize = 3;

/// Learning threshold: how many times a correction must be seen before
/// it is promoted to vocab_terms.
const LEARNING_THRESHOLD: i64 = 2;

/// Tokenize text into words (alphanumeric + hyphen/underscore, no punctuation).
fn tokenize(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
        .filter(|t| !t.is_empty())
        .map(String::from)
        .collect()
}

/// Process a user edit event: diff original vs edited text, upsert correction_log,
/// promote recurring corrections to vocab_terms.
///
/// Returns a list of term strings that were promoted to the vocabulary during this call.
pub fn process_edit(conn: &Connection, edit: &TranscriptionEdit) -> Result<Vec<String>> {
    let now = Utc::now().timestamp();

    // Build case-insensitive word sets for both sides
    let original_words: HashSet<String> = tokenize(&edit.original_text)
        .into_iter()
        .map(|w| w.to_lowercase())
        .collect();

    let edited_tokens = tokenize(&edit.edited_text);

    // New words = present in edited text but absent in original (case-insensitive)
    let new_words: Vec<String> = edited_tokens
        .into_iter()
        .filter(|w| {
            let lower = w.to_lowercase();
            w.len() >= MIN_WORD_LEN
                && !original_words.contains(&lower)
                && w.chars().any(|c| c.is_alphabetic()) // not pure digits/punctuation
        })
        .collect();

    let mut promoted: Vec<String> = Vec::new();

    for word in &new_words {
        let word_lower = word.to_lowercase();

        // Upsert into correction_log: increment times_seen if exists
        conn.execute(
            "INSERT INTO correction_log
                (original_span, original_lower, corrected_span, corrected_lower,
                 history_entry_id, times_seen, last_seen_at, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?6)
             ON CONFLICT(original_lower) DO UPDATE SET
                 times_seen   = times_seen + 1,
                 last_seen_at = excluded.last_seen_at",
            params![
                word,            // original_span (we store the new word as both sides
                word_lower,      // for simple single-word learning)
                word,            // corrected_span
                word_lower,      // corrected_lower
                edit.history_entry_id,
                now
            ],
        )?;

        // Check if threshold reached
        let times_seen: i64 = conn.query_row(
            "SELECT times_seen FROM correction_log WHERE original_lower = ?1",
            params![word_lower],
            |row| row.get(0),
        )?;

        if times_seen >= LEARNING_THRESHOLD {
            // Promote to vocab_terms as auto_learned
            conn.execute(
                "INSERT INTO vocab_terms
                    (term, term_lower, source, weight, sightings, suppressed, created_at, updated_at)
                 VALUES (?1, ?2, 'auto_learned', 1.0, 1, 0, ?3, ?3)
                 ON CONFLICT(term_lower) DO UPDATE SET
                     sightings  = sightings + 1,
                     updated_at = excluded.updated_at
                 WHERE suppressed = 0",
                params![word, word_lower, now],
            )?;
            promoted.push(word.clone());
        }
    }

    Ok(promoted)
}

// ── Startup maintenance ───────────────────────────────────────────────────────

/// Run at app startup to decay stale term weights and prune old correction log entries.
///
/// - Decays weight by 0.1 for terms not updated in the last 30 days.
/// - Clamps weight to a minimum of 0.1 (terms are never fully zeroed).
/// - Deletes correction_log entries older than 90 days.
pub fn startup_maintenance(conn: &Connection) -> Result<()> {
    let now = Utc::now().timestamp();
    let thirty_days_ago = now - (30 * 24 * 60 * 60);
    let ninety_days_ago = now - (90 * 24 * 60 * 60);

    // Decay weight for terms not seen in 30+ days, clamp to min 0.1
    conn.execute(
        "UPDATE vocab_terms
         SET weight     = MAX(0.1, weight - 0.1),
             updated_at = ?1
         WHERE suppressed = 0
           AND updated_at < ?2",
        params![now, thirty_days_ago],
    )?;

    // Prune old correction log entries
    conn.execute(
        "DELETE FROM correction_log WHERE last_seen_at < ?1",
        params![ninety_days_ago],
    )?;

    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn setup_conn() -> Connection {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        conn.execute_batch(
            "CREATE TABLE vocab_terms (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                term        TEXT NOT NULL,
                term_lower  TEXT NOT NULL,
                source      TEXT NOT NULL DEFAULT 'manual',
                weight      REAL NOT NULL DEFAULT 1.0,
                sightings   INTEGER NOT NULL DEFAULT 0,
                suppressed  INTEGER NOT NULL DEFAULT 0,
                created_at  INTEGER NOT NULL,
                updated_at  INTEGER NOT NULL,
                UNIQUE(term_lower)
            );
            CREATE TABLE correction_log (
                id               INTEGER PRIMARY KEY AUTOINCREMENT,
                original_span    TEXT NOT NULL,
                original_lower   TEXT NOT NULL,
                corrected_span   TEXT NOT NULL,
                corrected_lower  TEXT NOT NULL,
                history_entry_id INTEGER,
                times_seen       INTEGER NOT NULL DEFAULT 1,
                last_seen_at     INTEGER NOT NULL,
                created_at       INTEGER NOT NULL,
                UNIQUE(original_lower)
            );",
        )
        .expect("create test tables");
        conn
    }

    #[test]
    fn add_term_rejects_short_input() {
        let conn = setup_conn();
        assert!(add_term(&conn, "x").is_err());
        assert!(add_term(&conn, " a ").is_err());
    }

    #[test]
    fn add_term_inserts_and_returns_row() {
        let conn = setup_conn();
        let term = add_term(&conn, "  TensorFlow  ").expect("add term");
        assert_eq!(term.term, "TensorFlow");
        assert_eq!(term.term_lower, "tensorflow");
        assert_eq!(term.source, "manual");
        assert!((term.weight - 1.0).abs() < f64::EPSILON);
        assert!(!term.suppressed);
    }

    #[test]
    fn list_terms_order_by_weight_desc() {
        let conn = setup_conn();
        add_term(&conn, "alpha").unwrap();
        conn.execute(
            "UPDATE vocab_terms SET weight = 2.0 WHERE term_lower = 'alpha'",
            [],
        )
        .unwrap();
        add_term(&conn, "beta").unwrap();

        let terms = list_terms(&conn).unwrap();
        assert_eq!(terms[0].term_lower, "alpha");
        assert_eq!(terms[1].term_lower, "beta");
    }

    #[test]
    fn suppress_term_sets_flag() {
        let conn = setup_conn();
        let t = add_term(&conn, "Rust").unwrap();
        suppress_term(&conn, t.id).unwrap();

        let updated: bool = conn
            .query_row(
                "SELECT suppressed FROM vocab_terms WHERE id = ?1",
                params![t.id],
                |r| r.get::<_, i64>(0),
            )
            .unwrap()
            != 0;
        assert!(updated);
    }

    #[test]
    fn delete_term_removes_row() {
        let conn = setup_conn();
        let t = add_term(&conn, "Tokio").unwrap();
        delete_term(&conn, t.id).unwrap();

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM vocab_terms WHERE id = ?1",
                params![t.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn active_boost_terms_filters_suppressed_and_low_weight() {
        let conn = setup_conn();
        let t1 = add_term(&conn, "Cargo").unwrap();
        let t2 = add_term(&conn, "Clippy").unwrap();
        let t3 = add_term(&conn, "Rustfmt").unwrap();

        // Suppress t2
        suppress_term(&conn, t2.id).unwrap();
        // Drop t3 weight below threshold
        conn.execute(
            "UPDATE vocab_terms SET weight = 0.3 WHERE id = ?1",
            params![t3.id],
        )
        .unwrap();

        let boost = active_boost_terms(&conn).unwrap();
        assert!(boost.contains(&"Cargo".to_string()));
        assert!(!boost.contains(&"Clippy".to_string()));
        assert!(!boost.contains(&"Rustfmt".to_string()));
    }

    #[test]
    fn process_edit_promotes_repeated_corrections() {
        let conn = setup_conn();

        let edit = TranscriptionEdit {
            history_entry_id: 1,
            original_text: "cargo run the tests".to_string(),
            edited_text: "cargo run the tests Tokio".to_string(),
        };

        // First occurrence: should NOT yet promote
        let promoted = process_edit(&conn, &edit).unwrap();
        assert!(promoted.is_empty());

        // Second occurrence: should promote
        let promoted = process_edit(&conn, &edit).unwrap();
        assert!(promoted.contains(&"Tokio".to_string()));

        // Verify it landed in vocab_terms
        let terms = list_terms(&conn).unwrap();
        assert!(terms.iter().any(|t| t.term == "Tokio" && t.source == "auto_learned"));
    }

    #[test]
    fn process_edit_ignores_short_words() {
        let conn = setup_conn();

        let edit = TranscriptionEdit {
            history_entry_id: 1,
            original_text: "hello".to_string(),
            edited_text: "hello at go".to_string(), // "at" and "go" are too short
        };

        process_edit(&conn, &edit).unwrap();
        process_edit(&conn, &edit).unwrap();

        let terms = list_terms(&conn).unwrap();
        assert!(terms.is_empty(), "short words should not be learned");
    }

    #[test]
    fn startup_maintenance_decays_stale_terms() {
        let conn = setup_conn();
        let t = add_term(&conn, "OldTerm").unwrap();

        // Backdate updated_at by 31 days
        let old_ts = Utc::now().timestamp() - (31 * 24 * 60 * 60);
        conn.execute(
            "UPDATE vocab_terms SET updated_at = ?1 WHERE id = ?2",
            params![old_ts, t.id],
        )
        .unwrap();

        startup_maintenance(&conn).unwrap();

        let weight: f64 = conn
            .query_row(
                "SELECT weight FROM vocab_terms WHERE id = ?1",
                params![t.id],
                |r| r.get(0),
            )
            .unwrap();
        // Original weight 1.0 - 0.1 = 0.9
        assert!((weight - 0.9).abs() < 0.001);
    }

    #[test]
    fn startup_maintenance_clamps_weight_to_minimum() {
        let conn = setup_conn();
        let t = add_term(&conn, "TinyWeight").unwrap();

        // Set weight very low and backdate
        let old_ts = Utc::now().timestamp() - (31 * 24 * 60 * 60);
        conn.execute(
            "UPDATE vocab_terms SET weight = 0.05, updated_at = ?1 WHERE id = ?2",
            params![old_ts, t.id],
        )
        .unwrap();

        startup_maintenance(&conn).unwrap();

        let weight: f64 = conn
            .query_row(
                "SELECT weight FROM vocab_terms WHERE id = ?1",
                params![t.id],
                |r| r.get(0),
            )
            .unwrap();
        // MAX(0.1, 0.05 - 0.1) = MAX(0.1, -0.05) = 0.1
        assert!((weight - 0.1).abs() < 0.001);
    }

    #[test]
    fn startup_maintenance_prunes_old_corrections() {
        let conn = setup_conn();

        let old_ts = Utc::now().timestamp() - (91 * 24 * 60 * 60);
        conn.execute(
            "INSERT INTO correction_log
                (original_span, original_lower, corrected_span, corrected_lower,
                 history_entry_id, times_seen, last_seen_at, created_at)
             VALUES ('foo', 'foo', 'bar', 'bar', NULL, 1, ?1, ?1)",
            params![old_ts],
        )
        .unwrap();

        let count_before: i64 = conn
            .query_row("SELECT COUNT(*) FROM correction_log", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count_before, 1);

        startup_maintenance(&conn).unwrap();

        let count_after: i64 = conn
            .query_row("SELECT COUNT(*) FROM correction_log", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count_after, 0);
    }
}
