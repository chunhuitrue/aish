use aish_protocol::openai_models::ModelPreset;
use std::sync::Arc;

use crate::auth::AuthManager;
use crate::config::Config;
use crate::models_manager::model_family::ModelFamily;

/// Manages model family construction and model listing.
#[derive(Debug)]
pub struct ModelsManager {
    local_models: Vec<ModelPreset>,
    #[expect(dead_code)]
    auth_manager: Arc<AuthManager>,
}

impl ModelsManager {
    /// Construct a manager scoped to the provided `AuthManager`.
    pub fn new(auth_manager: Arc<AuthManager>) -> Self {
        Self {
            local_models: Vec::new(),
            auth_manager,
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    /// Construct a manager for testing.
    pub fn for_testing(auth_manager: Arc<AuthManager>) -> Self {
        Self {
            local_models: Vec::new(),
            auth_manager,
        }
    }

    /// Return available models (currently just local models).
    pub fn list_models(&self) -> Vec<ModelPreset> {
        self.build_available_models()
    }

    /// Return available models (non-async version for UI).
    pub fn try_list_models(&self) -> Vec<ModelPreset> {
        self.build_available_models()
    }

    fn find_family_for_model(slug: &str) -> ModelFamily {
        super::model_family::find_family_for_model(slug)
    }

    /// Look up the requested model family while applying config overrides.
    pub fn construct_model_family(&self, model: &str, config: &Config) -> ModelFamily {
        Self::find_family_for_model(model).with_config_overrides(config)
    }

    /// Get a model - returns the provided model or the first available default.
    pub fn get_model(&self, model: &Option<String>) -> Option<String> {
        if let Some(model) = model.as_ref() {
            return Some(model.to_string());
        }

        let available_models = self.build_available_models();

        // Return the first available model marked as default, or the first model if none is default
        available_models
            .iter()
            .find(|m| m.is_default)
            .or_else(|| available_models.first())
            .map(|m| m.model.clone())
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn get_model_offline(model: Option<&str>) -> Option<String> {
        model.map(std::string::ToString::to_string)
    }

    #[cfg(any(test, feature = "test-support"))]
    /// Offline helper that builds a `ModelFamily` without consulting remote state.
    pub fn construct_model_family_offline(model: &str, config: &Config) -> ModelFamily {
        Self::find_family_for_model(model).with_config_overrides(config)
    }

    /// Build available models list from local models.
    fn build_available_models(&self) -> Vec<ModelPreset> {
        let mut models = self.local_models.clone();
        models = self.filter_visible_models(models);

        let has_default = models.iter().any(|preset| preset.is_default);
        if let Some(default) = models.first_mut()
            && !has_default
        {
            default.is_default = true;
        }

        models
    }

    fn filter_visible_models(&self, models: Vec<ModelPreset>) -> Vec<ModelPreset> {
        // Only API key mode is supported, filter models that are supported in API
        models
            .into_iter()
            .filter(|model| model.show_in_picker && model.supported_in_api)
            .collect()
    }
}
