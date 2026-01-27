use aish_core::AishAuth;
use aish_core::ConversationManager;
use aish_core::built_in_model_providers;
use anyhow::Result;
use pretty_assertions::assert_eq;
use tempfile::tempdir;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn list_models_returns_api_key_models() -> Result<()> {
    let _codex_home = tempdir()?;
    let manager = ConversationManager::with_models_provider(
        AishAuth::from_api_key("sk-test"),
        built_in_model_providers()["openai"].clone(),
    );
    let models = manager.list_models();

    // After removing built-in model presets, list_models returns an empty list
    assert_eq!(0, models.len());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn list_models_returns_chatgpt_models() -> Result<()> {
    let _codex_home = tempdir()?;
    let manager = ConversationManager::with_models_provider(
        AishAuth::create_dummy_auth_for_testing(),
        built_in_model_providers()["openai"].clone(),
    );
    let models = manager.list_models();

    // After removing built-in model presets, list_models returns an empty list
    assert_eq!(0, models.len());

    Ok(())
}
