// src-tauri/src/post_process/itn/nemo_stub.rs
// No-op NeMo ITN stub for Phase 2.
// Will be replaced with a real NeMo ITN call once the crate is available.

/// Pass the text through unchanged.
/// In the full pipeline this will invoke NeMo's text normalization grammar
/// which handles cardinals, ordinals, currency, dates, and more in a
/// context-aware, language-model-backed manner.
pub fn run(text: &str) -> String {
    text.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passthrough_unchanged() {
        let input = "twenty five apples cost 5 dollars";
        assert_eq!(run(input), input);
    }

    #[test]
    fn empty_string() {
        assert_eq!(run(""), "");
    }
}
