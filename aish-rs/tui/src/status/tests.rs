use super::new_status_output;
use crate::history_cell::HistoryCell;
use aish_core::config::Config;
use aish_core::config::ConfigBuilder;
use aish_core::models_manager::manager::ModelsManager;
use aish_core::models_manager::model_family::ModelFamily;
use aish_core::protocol::TokenUsage;
use chrono::TimeZone;
use std::path::PathBuf;
use tempfile::TempDir;

async fn test_config(temp_home: &TempDir) -> Config {
    ConfigBuilder::default()
        .codex_home(temp_home.path().to_path_buf())
        .build()
        .await
        .expect("load config")
}

fn test_model_family(model_slug: Option<&str>, config: &Config) -> ModelFamily {
    ModelsManager::construct_model_family_offline(model_slug.unwrap_or("test-model"), config)
}

fn render_lines(lines: &[ratatui::text::Line<'static>]) -> Vec<String> {
    lines
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
        })
        .collect()
}

#[tokio::test]
async fn status_card_token_usage_excludes_cached_tokens() {
    let temp_home = TempDir::new().expect("temp home");
    let mut config = test_config(&temp_home).await;
    config.model = Some("gpt-5.1-codex-max".to_string());
    config.cwd = PathBuf::from("/workspace/tests");

    let usage = TokenUsage {
        input_tokens: 1_200,
        cached_input_tokens: 200,
        output_tokens: 900,
        reasoning_output_tokens: 0,
        total_tokens: 2_100,
    };

    let now = chrono::Local
        .with_ymd_and_hms(2024, 1, 1, 0, 0, 0)
        .single()
        .expect("timestamp");

    let model_slug = ModelsManager::get_model_offline(config.model.as_deref());
    let model_family = test_model_family(model_slug.as_deref(), &config);
    let composite = new_status_output(
        &config,
        &model_family,
        &usage,
        Some(&usage),
        &None,
        None,
        now,
        model_slug.as_deref().unwrap_or("test-model"),
    );
    let rendered = render_lines(&composite.display_lines(120));

    assert!(
        rendered.iter().all(|line| !line.contains("cached")),
        "cached tokens should not be displayed, got: {rendered:?}"
    );
}

#[tokio::test]
async fn status_context_window_uses_last_usage() {
    let temp_home = TempDir::new().expect("temp home");
    let mut config = test_config(&temp_home).await;
    config.model_context_window = Some(272_000);

    let total_usage = TokenUsage {
        input_tokens: 12_800,
        cached_input_tokens: 0,
        output_tokens: 879,
        reasoning_output_tokens: 0,
        total_tokens: 102_000,
    };
    let last_usage = TokenUsage {
        input_tokens: 12_800,
        cached_input_tokens: 0,
        output_tokens: 879,
        reasoning_output_tokens: 0,
        total_tokens: 13_679,
    };

    let now = chrono::Local
        .with_ymd_and_hms(2024, 6, 1, 12, 0, 0)
        .single()
        .expect("timestamp");

    let model_slug = ModelsManager::get_model_offline(config.model.as_deref());
    let model_family = test_model_family(model_slug.as_deref(), &config);
    let composite = new_status_output(
        &config,
        &model_family,
        &total_usage,
        Some(&last_usage),
        &None,
        None,
        now,
        model_slug.as_deref().unwrap_or("test-model"),
    );
    let rendered_lines = render_lines(&composite.display_lines(80));
    let context_line = rendered_lines
        .into_iter()
        .find(|line| line.contains("Context window"))
        .expect("context line");

    assert!(
        context_line.contains("13.7K used / 272K"),
        "expected context line to reflect last usage tokens, got: {context_line}"
    );
    assert!(
        !context_line.contains("102K"),
        "context line should not use total aggregated tokens, got: {context_line}"
    );
}
