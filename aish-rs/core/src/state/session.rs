//! Session-wide mutable state.

use aish_protocol::models::ResponseItem;

use crate::aish::SessionConfiguration;
use crate::context_manager::ContextManager;
use crate::protocol::TokenUsage;
use crate::protocol::TokenUsageInfo;
use crate::truncate::TruncationPolicy;

/// Persistent, session-scoped state previously stored directly on `Session`.
pub(crate) struct SessionState {
    pub(crate) session_configuration: SessionConfiguration,
    pub(crate) history: ContextManager,
}

impl SessionState {
    /// Create a new session state mirroring previous `State::default()` semantics.
    pub(crate) fn new(session_configuration: SessionConfiguration) -> Self {
        let history = ContextManager::new();
        Self {
            session_configuration,
            history,
        }
    }

    // History helpers
    pub(crate) fn record_items<I>(&mut self, items: I, policy: TruncationPolicy)
    where
        I: IntoIterator,
        I::Item: std::ops::Deref<Target = ResponseItem>,
    {
        self.history.record_items(items, policy);
    }

    pub(crate) fn clone_history(&self) -> ContextManager {
        self.history.clone()
    }

    pub(crate) fn replace_history(&mut self, items: Vec<ResponseItem>) {
        self.history.replace(items);
    }

    pub(crate) fn set_token_info(&mut self, info: Option<TokenUsageInfo>) {
        self.history.set_token_info(info);
    }

    // Token/rate limit helpers
    pub(crate) fn update_token_info_from_usage(
        &mut self,
        usage: &TokenUsage,
        model_context_window: Option<i64>,
    ) {
        self.history.update_token_info(usage, model_context_window);
    }

    pub(crate) fn token_info(&self) -> Option<TokenUsageInfo> {
        self.history.token_info()
    }

    pub(crate) fn set_token_usage_full(&mut self, context_window: i64) {
        self.history.set_token_usage_full(context_window);
    }

    pub(crate) fn get_total_token_usage(&self) -> i64 {
        self.history.get_total_token_usage()
    }
}

