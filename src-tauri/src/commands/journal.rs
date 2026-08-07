// src-tauri/src/commands/journal.rs
//
// Tauri commands for the Journal feature.
// Journal groups transcription_history by local date and exposes full-text search.

use crate::managers::history::HistoryEntry;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::AppHandle;

// ── Types ─────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct JournalDay {
    pub date_str: String,
    pub entry_count: i64,
    pub word_count: i64,
    pub first_ts: i64,
    pub last_ts: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct JournalSearchResult {
    pub id: i64,
    pub timestamp: i64,
    pub title: String,
    pub snippet: String,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn open_db(app: &AppHandle) -> Result<rusqlite::Connection, String> {
    let app_data_dir = crate::portable::app_data_dir(app)
        .map_err(|e| format!("app_data_dir: {e}"))?;
    let db_path = app_data_dir.join("history.db");
    rusqlite::Connection::open(db_path).map_err(|e| format!("open db: {e}"))
}

/// Recomputes the `journal_days` summary table from scratch.
/// Called lazily when listing days if the table looks stale, and on startup.
fn rebuild_journal_days(conn: &rusqlite::Connection) -> Result<(), String> {
    conn.execute("DELETE FROM journal_days", [])
        .map_err(|e| e.to_string())?;

    // SQLite date() with 'unixepoch' returns the UTC date; we use localtime to
    // match what the user sees. For simplicity we use 'localtime' modifier.
    conn.execute(
        "INSERT INTO journal_days (date_str, entry_count, word_count, first_ts, last_ts)
         SELECT
             date(timestamp, 'unixepoch', 'localtime')           AS date_str,
             COUNT(*)                                             AS entry_count,
             SUM(
                 LENGTH(transcription_text) -
                 LENGTH(REPLACE(transcription_text, ' ', '')) + 1
             )                                                    AS word_count,
             MIN(timestamp)                                       AS first_ts,
             MAX(timestamp)                                       AS last_ts
         FROM transcription_history
         GROUP BY date_str
         ORDER BY first_ts DESC",
        [],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

// ── Commands ──────────────────────────────────────────────────────────────────

/// Returns a list of journal days (most recent first) with entry/word counts.
/// Rebuilds the summary table from transcription_history on every call so the
/// view is always fresh (the table is a cache, not the source of truth).
#[tauri::command]
#[specta::specta]
pub fn list_journal_days(app: AppHandle) -> Result<Vec<JournalDay>, String> {
    let conn = open_db(&app)?;

    rebuild_journal_days(&conn)?;

    let mut stmt = conn
        .prepare(
            "SELECT date_str, entry_count, word_count, first_ts, last_ts
             FROM journal_days
             ORDER BY first_ts DESC",
        )
        .map_err(|e| e.to_string())?;

    let days = stmt
        .query_map([], |row| {
            Ok(JournalDay {
                date_str: row.get(0)?,
                entry_count: row.get(1)?,
                word_count: row.get(2)?,
                first_ts: row.get(3)?,
                last_ts: row.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(days)
}

/// Returns all history entries for a given ISO date string (YYYY-MM-DD), most
/// recent first.
#[tauri::command]
#[specta::specta]
pub fn get_journal_day(
    app: AppHandle,
    date_str: String,
) -> Result<Vec<HistoryEntry>, String> {
    let conn = open_db(&app)?;

    let mut stmt = conn
        .prepare(
            "SELECT id, file_name, timestamp, saved, title,
                    transcription_text, post_processed_text,
                    post_process_prompt, post_process_requested
             FROM transcription_history
             WHERE date(timestamp, 'unixepoch', 'localtime') = ?1
             ORDER BY timestamp DESC",
        )
        .map_err(|e| e.to_string())?;

    let entries = stmt
        .query_map(params![date_str], |row| {
            Ok(HistoryEntry {
                id: row.get(0)?,
                file_name: row.get(1)?,
                timestamp: row.get(2)?,
                saved: row.get(3)?,
                title: row.get(4)?,
                transcription_text: row.get(5)?,
                post_processed_text: row.get(6)?,
                post_process_prompt: row.get(7)?,
                post_process_requested: row.get(8)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(entries)
}

/// Full-text search over all transcriptions.
/// Returns up to `limit` results (default 50) with a short snippet around each hit.
#[tauri::command]
#[specta::specta]
pub fn search_journal(
    app: AppHandle,
    query: String,
    limit: Option<i64>,
) -> Result<Vec<JournalSearchResult>, String> {
    if query.trim().is_empty() {
        return Ok(vec![]);
    }

    let conn = open_db(&app)?;
    let cap = limit.unwrap_or(50).clamp(1, 200);

    // Use FTS5 snippet() function for context around the match.
    // snippet(table, column_index, start_marker, end_marker, ellipsis, num_tokens)
    let mut stmt = conn
        .prepare(
            "SELECT h.id, h.timestamp, h.title,
                    snippet(journal_fts, 0, '**', '**', '…', 20) AS snippet
             FROM journal_fts
             JOIN transcription_history h ON h.id = journal_fts.rowid
             WHERE journal_fts MATCH ?1
             ORDER BY bm25(journal_fts) -- lower = more relevant
             LIMIT ?2",
        )
        .map_err(|e| e.to_string())?;

    let results = stmt
        .query_map(params![query, cap], |row| {
            Ok(JournalSearchResult {
                id: row.get(0)?,
                timestamp: row.get(1)?,
                title: row.get(2)?,
                snippet: row.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(results)
}
