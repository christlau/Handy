// src-tauri/src/post_process/itn/mod.rs
// Inverse Text Normalization pipeline.
// Ported from Resonant (macOS) ITNProcessor.swift to cross-platform Rust.

pub mod number_words;
mod protect;
mod normalize;
mod nemo_stub;
mod post_nemo;

/// ITN pipeline configuration.
#[derive(Clone, Debug, Default)]
pub struct ItnConfig {
    /// Phone number formatting style: "hyphenated" (default), "dotted", "spaced".
    pub phone_style: Option<String>,
}

/// Apply the ITN pipeline to transcription text:
///
/// 1. Protect times, years, emails, and existing digit spans with placeholders
///    so downstream passes cannot accidentally transform them.
/// 2. Normalize number-word phrases to digit form ("twenty five" → "25").
/// 3. NeMo stub — no-op in Phase 2; will become a real NeMo ITN call.
/// 4. Post-NeMo cleanup: percent spacing, phone grouping, double-space removal.
/// 5. Restore all protected placeholders to their original spans.
///
/// Returns the processed string.
pub fn process(text: &str, _config: &ItnConfig) -> String {
    let protected = protect::run(text);
    let normalized = normalize::run(&protected.text);
    let after_nemo = nemo_stub::run(&normalized);
    let result = post_nemo::run(&after_nemo);
    // Restore protected spans
    let mut output = result;
    for (placeholder, original) in &protected.replacements {
        output = output.replace(placeholder.as_str(), original.as_str());
    }
    output
}
