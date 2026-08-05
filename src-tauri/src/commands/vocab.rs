use crate::managers::vocab::{CorrectionEntry, VocabTerm};
use tauri::AppHandle;

#[tauri::command]
#[specta::specta]
pub fn vocab_list_terms(app: AppHandle) -> Result<Vec<VocabTerm>, String> {
    let conn = crate::managers::vocab::open_db(&app).map_err(|e| e.to_string())?;
    crate::managers::vocab::list_terms(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn vocab_add_term(app: AppHandle, term: String) -> Result<VocabTerm, String> {
    let conn = crate::managers::vocab::open_db(&app).map_err(|e| e.to_string())?;
    crate::managers::vocab::add_term(&conn, &term).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn vocab_suppress_term(app: AppHandle, id: i64) -> Result<(), String> {
    let conn = crate::managers::vocab::open_db(&app).map_err(|e| e.to_string())?;
    crate::managers::vocab::suppress_term(&conn, id).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn vocab_delete_term(app: AppHandle, id: i64) -> Result<(), String> {
    let conn = crate::managers::vocab::open_db(&app).map_err(|e| e.to_string())?;
    crate::managers::vocab::delete_term(&conn, id).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn vocab_list_corrections(
    app: AppHandle,
    limit: Option<i64>,
) -> Result<Vec<CorrectionEntry>, String> {
    let conn = crate::managers::vocab::open_db(&app).map_err(|e| e.to_string())?;
    let limit = limit.unwrap_or(100);
    crate::managers::vocab::list_corrections(&conn, limit).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn vocab_import_terms(app: AppHandle, terms: Vec<String>) -> Result<i64, String> {
    let conn = crate::managers::vocab::open_db(&app).map_err(|e| e.to_string())?;
    let mut count: i64 = 0;
    for term in &terms {
        let trimmed = term.trim();
        if trimmed.is_empty() {
            continue;
        }
        if crate::managers::vocab::add_term(&conn, trimmed).is_ok() {
            count += 1;
        }
    }
    Ok(count)
}

#[tauri::command]
#[specta::specta]
pub fn vocab_get_boost_terms(app: AppHandle) -> Result<Vec<String>, String> {
    let conn = crate::managers::vocab::open_db(&app).map_err(|e| e.to_string())?;
    crate::managers::vocab::active_boost_terms(&conn).map_err(|e| e.to_string())
}
