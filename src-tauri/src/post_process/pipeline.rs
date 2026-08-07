// src-tauri/src/post_process/pipeline.rs

use super::common::TokenTiming;
use super::{filler, fragmented_word_repair, pause_segment, punctuation, question_inference, special_tokens};
use log::warn;

#[derive(Clone, Debug)]
pub struct PostProcessConfig {
    pub remove_fillers_enabled: bool,
    pub remove_false_starts_enabled: bool,
    pub auto_punctuation_enabled: bool,
    pub bullet_points_enabled: bool,
    pub itn_enabled: bool,
    pub fragmented_word_repair_enabled: bool,
    pub app_language: String,
}

impl Default for PostProcessConfig {
    fn default() -> Self {
        Self {
            remove_fillers_enabled: false,
            remove_false_starts_enabled: false,
            auto_punctuation_enabled: false,
            bullet_points_enabled: false,
            itn_enabled: false,
            fragmented_word_repair_enabled: false,
            app_language: "en".to_string(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct PostProcessResult {
    pub text: String,
    /// Names of stages that changed the text, for debug logging.
    pub changed_stages: Vec<&'static str>,
}

/// Run the full deterministic pipeline.
/// All stages are fail-open: a panic inside any transform is caught and
/// that stage is skipped, returning the input unchanged.
pub fn apply_pipeline(
    raw: &str,
    config: &PostProcessConfig,
    token_timings: &[TokenTiming],
) -> PostProcessResult {
    if raw.is_empty() {
        return PostProcessResult {
            text: raw.to_string(),
            changed_stages: vec![],
        };
    }
    let mut text = raw.to_string();
    let mut changed: Vec<&'static str> = Vec::new();

    // Stage 1: strip leaked model control tokens
    run_stage("special_tokens", &mut text, &mut changed, |t| {
        special_tokens::strip(t)
    });

    // Stage 1b: fragmented word repair — fuse ASR-split tokens before any
    // other processing (e.g. "im ple ment ation" -> "implementation")
    if config.fragmented_word_repair_enabled {
        run_stage("fragmented_word_repair", &mut text, &mut changed, |t| {
            fragmented_word_repair::repair(t)
        });
    }

    // Stage 2: pause-driven segmentation (requires token timings)
    if config.auto_punctuation_enabled && !token_timings.is_empty() {
        let timings = token_timings.to_vec();
        run_stage("pause_segmentation", &mut text, &mut changed, move |t| {
            pause_segment::segment(t, &timings)
        });
    }

    // Stage 3: filler and false-start removal
    {
        let remove_fillers = config.remove_fillers_enabled;
        let remove_false_starts = config.remove_false_starts_enabled;
        run_stage("filler_removal", &mut text, &mut changed, move |t| {
            filler::process(t, remove_fillers, remove_false_starts)
        });
    }

    // Stage: ITN — convert number words to digits
    if config.itn_enabled {
        run_stage("itn", &mut text, &mut changed, |t| {
            crate::post_process::itn::process(t, &crate::post_process::itn::ItnConfig::default())
        });
    }

    // Stage 4: spoken command substitution and final formatting
    if config.auto_punctuation_enabled {
        run_stage("spoken_commands", &mut text, &mut changed, |t| {
            punctuation::process_spoken_commands(t)
        });

        let bullet = config.bullet_points_enabled;
        run_stage("final_formatting", &mut text, &mut changed, move |t| {
            punctuation::finalize_formatting(
                t,
                &punctuation::FormattingConfig {
                    bullet_points_enabled: bullet,
                    ensure_trailing_punctuation: true,
                    capitalize_leading_letters: true,
                },
            )
        });

        // Stage 5: question mark inference
        run_stage("question_inference", &mut text, &mut changed, |t| {
            question_inference::apply(t, false)
        });
    }

    PostProcessResult {
        text,
        changed_stages: changed,
    }
}

/// Runs a transform stage. Records stage name if text changed. Never panics.
fn run_stage(
    name: &'static str,
    text: &mut String,
    changed: &mut Vec<&'static str>,
    transform: impl FnOnce(&str) -> String + std::panic::UnwindSafe,
) {
    let before = text.clone();
    let after =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| transform(&before)))
            .unwrap_or_else(|_| {
                warn!("post_process stage '{}' panicked \u{2014} skipping", name);
                before.clone()
            });
    if after != before {
        changed.push(name);
    }
    *text = after;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> PostProcessConfig {
        PostProcessConfig {
            remove_fillers_enabled: true,
            remove_false_starts_enabled: false,
            auto_punctuation_enabled: true,
            bullet_points_enabled: false,
            itn_enabled: false,
            app_language: "en".to_string(),
        }
    }

    #[test]
    fn empty_input_returns_empty() {
        let result = apply_pipeline("", &default_config(), &[]);
        assert_eq!(result.text, "");
        assert!(result.changed_stages.is_empty());
    }

    #[test]
    fn strips_special_tokens() {
        let result = apply_pipeline("<|en|>Hello world", &default_config(), &[]);
        assert!(result.text.contains("Hello world"));
        assert!(!result.text.contains("<|"));
    }

    #[test]
    fn removes_fillers_when_enabled() {
        let result = apply_pipeline("um the build failed", &default_config(), &[]);
        assert!(!result.text.contains("um"));
        assert!(result.text.contains("build failed"));
    }

    #[test]
    fn no_op_when_all_disabled() {
        let config = PostProcessConfig {
            remove_fillers_enabled: false,
            remove_false_starts_enabled: false,
            auto_punctuation_enabled: false,
            bullet_points_enabled: false,
            itn_enabled: false,
            app_language: "en".to_string(),
        };
        let input = "um hello world";
        let result = apply_pipeline(input, &config, &[]);
        assert_eq!(result.text, input);
    }

    #[test]
    fn changed_stages_recorded() {
        let result = apply_pipeline("<|en|>hello", &default_config(), &[]);
        assert!(result.changed_stages.contains(&"special_tokens"));
    }
}
