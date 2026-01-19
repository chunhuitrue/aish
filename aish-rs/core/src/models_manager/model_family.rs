use aish_protocol::config_types::Verbosity;
use aish_protocol::openai_models::ApplyPatchToolType;
use aish_protocol::openai_models::ConfigShellToolType;
use aish_protocol::openai_models::ReasoningEffort;

use crate::config::Config;
use crate::truncate::TruncationPolicy;

/// The `instructions` field in the payload sent to a model should always start
/// with this content.
const BASE_INSTRUCTIONS: &str = include_str!("../../prompt.md");

/// A model family is a group of models that share certain characteristics.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModelFamily {
    /// The full model slug used to derive this model family, e.g.
    /// "gpt-4.1-2025-04-14".
    pub slug: String,

    /// The model family name, e.g. "gpt-4.1". This string is used when deriving
    /// default metadata for the family, such as context windows.
    pub family: String,

    /// True if the model needs additional instructions on how to use the
    /// "virtual" `apply_patch` CLI.
    pub needs_special_apply_patch_instructions: bool,

    /// Maximum supported context window, if known.
    pub context_window: Option<i64>,

    /// Token threshold for automatic compaction if config does not override it.
    auto_compact_token_limit: Option<i64>,

    // Whether the `reasoning` field can be set when making a request to this
    // model family. Note it has `effort` and `summary` subfields (though
    // `summary` is optional).
    pub supports_reasoning_summaries: bool,

    // The reasoning effort to use for this model family when none is explicitly chosen.
    pub default_reasoning_effort: Option<ReasoningEffort>,

    /// Whether this model supports parallel tool calls when using the
    /// Responses API.
    pub supports_parallel_tool_calls: bool,

    /// Present if the model performs better when `apply_patch` is provided as
    /// a tool call instead of just a bash command
    pub apply_patch_tool_type: Option<ApplyPatchToolType>,

    // Instructions to use for querying the model
    pub base_instructions: String,

    /// Names of beta tools that should be exposed to this model family.
    pub experimental_supported_tools: Vec<String>,

    /// Percentage of the context window considered usable for inputs, after
    /// reserving headroom for system prompts, tool overhead, and model output.
    /// This is applied when computing the effective context window seen by
    /// consumers.
    pub effective_context_window_percent: i64,

    /// If the model family supports setting the verbosity level when using Responses API.
    pub support_verbosity: bool,

    // The default verbosity level for this model family when using Responses API.
    pub default_verbosity: Option<Verbosity>,

    /// Preferred shell tool type for this model family when features do not override it.
    pub shell_type: ConfigShellToolType,

    pub truncation_policy: TruncationPolicy,
}

impl ModelFamily {
    pub(super) fn with_config_overrides(mut self, config: &Config) -> Self {
        if let Some(supports_reasoning_summaries) = config.model_supports_reasoning_summaries {
            self.supports_reasoning_summaries = supports_reasoning_summaries;
        }
        if let Some(context_window) = config.model_context_window {
            self.context_window = Some(context_window);
        }
        if let Some(auto_compact_token_limit) = config.model_auto_compact_token_limit {
            self.auto_compact_token_limit = Some(auto_compact_token_limit);
        }
        self
    }

    pub fn auto_compact_token_limit(&self) -> Option<i64> {
        self.auto_compact_token_limit
            .or(self.context_window.map(Self::default_auto_compact_limit))
    }

    const fn default_auto_compact_limit(context_window: i64) -> i64 {
        (context_window * 9) / 10
    }

    pub fn get_model_slug(&self) -> &str {
        &self.slug
    }
}

macro_rules! model_family {
    (
        $slug:expr, $family:expr $(, $key:ident : $value:expr )* $(,)?
    ) => {{
        // defaults
        #[allow(unused_mut)]
        let mut mf = ModelFamily {
            slug: $slug.to_string(),
            family: $family.to_string(),
            needs_special_apply_patch_instructions: false,
            context_window: None,
            auto_compact_token_limit: None,
            supports_reasoning_summaries: false,
            supports_parallel_tool_calls: false,
            apply_patch_tool_type: None,
            base_instructions: BASE_INSTRUCTIONS.to_string(),
            experimental_supported_tools: Vec::new(),
            effective_context_window_percent: 95,
            support_verbosity: false,
            shell_type: ConfigShellToolType::Default,
            default_verbosity: None,
            default_reasoning_effort: None,
            truncation_policy: TruncationPolicy::Bytes(10_000),
        };

        // apply overrides
        $(
            mf.$key = $value;
        )*
        mf
    }};
}

/// Internal offline helper for `ModelsManager` that returns a `ModelFamily` for the given
/// model slug.
pub(super) fn find_family_for_model(slug: &str) -> ModelFamily {
    // Test models with experimental tools and apply_patch support.
    if slug.starts_with("test-") {
        model_family!(
            slug, slug,
            supports_reasoning_summaries: true,
            base_instructions: BASE_INSTRUCTIONS.to_string(),
            experimental_supported_tools: vec![
                "grep_files".to_string(),
                "list_dir".to_string(),
                "read_file".to_string(),
                "test_sync_tool".to_string(),
            ],
            supports_parallel_tool_calls: true,
            shell_type: ConfigShellToolType::ShellCommand,
            support_verbosity: true,
            default_verbosity: Some(Verbosity::Low),
            default_reasoning_effort: Some(ReasoningEffort::Medium),
            apply_patch_tool_type: Some(ApplyPatchToolType::Freeform),
            truncation_policy: TruncationPolicy::Tokens(10_000),
            context_window: None,
        )
    } else {
        derive_default_model_family(slug)
    }
}

fn derive_default_model_family(model: &str) -> ModelFamily {
    ModelFamily {
        slug: model.to_string(),
        family: model.to_string(),
        needs_special_apply_patch_instructions: false,
        context_window: None,
        auto_compact_token_limit: None,
        supports_reasoning_summaries: false,
        supports_parallel_tool_calls: false,
        apply_patch_tool_type: None,
        base_instructions: BASE_INSTRUCTIONS.to_string(),
        experimental_supported_tools: Vec::new(),
        effective_context_window_percent: 95,
        support_verbosity: false,
        shell_type: ConfigShellToolType::Default,
        default_verbosity: None,
        default_reasoning_effort: None,
        truncation_policy: TruncationPolicy::Bytes(10_000),
    }
}
