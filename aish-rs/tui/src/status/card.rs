use crate::history_cell::CompositeHistoryCell;
use crate::history_cell::HistoryCell;
use crate::history_cell::PlainHistoryCell;
use crate::history_cell::with_border_with_inner_width;
use crate::version::AISH_CLI_VERSION;
use aish_common::create_config_summary_entries;
use aish_core::config::Config;
use aish_core::models_manager::model_family::ModelFamily;
use aish_core::protocol::NetworkAccess;
use aish_core::protocol::SandboxPolicy;
use aish_core::protocol::TokenUsage;
use aish_protocol::ConversationId;
use aish_protocol::account::PlanType;
use chrono::DateTime;
use chrono::Local;
use ratatui::prelude::*;
use ratatui::style::Stylize;
use std::collections::BTreeSet;
use std::path::PathBuf;

use super::format::FieldFormatter;
use super::format::line_display_width;
use super::format::push_label;
use super::format::truncate_line_to_width;
use super::helpers::compose_agents_summary;
use super::helpers::compose_model_display;
use super::helpers::format_tokens_compact;

#[derive(Debug, Clone)]
struct StatusContextWindowData {
    percent_remaining: i64,
    tokens_in_context: i64,
    window: i64,
}

#[derive(Debug, Clone)]
pub(crate) struct StatusTokenUsageData {
    total: i64,
    input: i64,
    output: i64,
    context_window: Option<StatusContextWindowData>,
}

#[derive(Debug)]
struct StatusHistoryCell {
    model_name: String,
    model_details: Vec<String>,
    directory: PathBuf,
    approval: String,
    sandbox: String,
    agents_summary: String,
    session_id: Option<String>,
    token_usage: StatusTokenUsageData,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn new_status_output(
    config: &Config,
    model_family: &ModelFamily,
    total_usage: &TokenUsage,
    context_usage: Option<&TokenUsage>,
    session_id: &Option<ConversationId>,
    plan_type: Option<PlanType>,
    now: DateTime<Local>,
    model_name: &str,
) -> CompositeHistoryCell {
    let command = PlainHistoryCell::new(vec!["/status".magenta().into()]);
    let card = StatusHistoryCell::new(
        config,
        model_family,
        total_usage,
        context_usage,
        session_id,
        plan_type,
        now,
        model_name,
    );

    CompositeHistoryCell::new(vec![Box::new(command), Box::new(card)])
}

impl StatusHistoryCell {
    #[allow(clippy::too_many_arguments)]
    fn new(
        config: &Config,
        model_family: &ModelFamily,
        total_usage: &TokenUsage,
        context_usage: Option<&TokenUsage>,
        session_id: &Option<ConversationId>,
        _plan_type: Option<PlanType>,
        _now: DateTime<Local>,
        model_name: &str,
    ) -> Self {
        let config_entries = create_config_summary_entries(config, model_name);
        let (model_name, model_details) = compose_model_display(model_name, &config_entries);
        let approval = config_entries
            .iter()
            .find(|(k, _)| *k == "approval")
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| "<unknown>".to_string());
        let sandbox = match config.sandbox_policy.get() {
            SandboxPolicy::DangerFullAccess => "danger-full-access".to_string(),
            SandboxPolicy::ReadOnly => "read-only".to_string(),
            SandboxPolicy::CurrentDirWrite { .. } => "current-dir-write".to_string(),
            SandboxPolicy::ExternalSandbox { network_access } => {
                if matches!(network_access, NetworkAccess::Enabled) {
                    "external-sandbox (network access enabled)".to_string()
                } else {
                    "external-sandbox".to_string()
                }
            }
        };
        let agents_summary = compose_agents_summary(config);
        let session_id = session_id.as_ref().map(std::string::ToString::to_string);
        let context_window = model_family.context_window.and_then(|window| {
            context_usage.map(|usage| StatusContextWindowData {
                percent_remaining: usage.percent_of_context_window_remaining(window),
                tokens_in_context: usage.tokens_in_context_window(),
                window,
            })
        });

        let token_usage = StatusTokenUsageData {
            total: total_usage.blended_total(),
            input: total_usage.non_cached_input(),
            output: total_usage.output_tokens,
            context_window,
        };

        Self {
            model_name,
            model_details,
            directory: config.cwd.clone(),
            approval,
            sandbox,
            agents_summary,
            session_id,
            token_usage,
        }
    }

    fn token_usage_spans(&self) -> Vec<Span<'static>> {
        let total_fmt = format_tokens_compact(self.token_usage.total);
        let input_fmt = format_tokens_compact(self.token_usage.input);
        let output_fmt = format_tokens_compact(self.token_usage.output);

        vec![
            Span::from(total_fmt),
            Span::from(" total "),
            Span::from(" (").dim(),
            Span::from(input_fmt).dim(),
            Span::from(" input").dim(),
            Span::from(" + ").dim(),
            Span::from(output_fmt).dim(),
            Span::from(" output").dim(),
            Span::from(")").dim(),
        ]
    }

    fn context_window_spans(&self) -> Option<Vec<Span<'static>>> {
        let context = self.token_usage.context_window.as_ref()?;
        let percent = context.percent_remaining;
        let used_fmt = format_tokens_compact(context.tokens_in_context);
        let window_fmt = format_tokens_compact(context.window);

        Some(vec![
            Span::from(format!("{percent}% left")),
            Span::from(" (").dim(),
            Span::from(used_fmt).dim(),
            Span::from(" used / ").dim(),
            Span::from(window_fmt).dim(),
            Span::from(")").dim(),
        ])
    }
}

impl HistoryCell for StatusHistoryCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        let mut lines: Vec<Line<'static>> = Vec::new();
        lines.push(Line::from(vec![
            Span::from(FieldFormatter::INDENT).dim(),
            Span::from("Aish").bold(),
            Span::from(" ").dim(),
            Span::from(format!("(v{AISH_CLI_VERSION})")).dim(),
        ]));
        lines.push(Line::from(Vec::<Span<'static>>::new()));

        let available_inner_width = usize::from(width.saturating_sub(4));
        if available_inner_width == 0 {
            return Vec::new();
        }

        let mut labels: Vec<String> = vec!["Model", "Approval", "Sandbox"]
            .into_iter()
            .map(str::to_string)
            .collect();
        let mut seen: BTreeSet<String> = labels.iter().cloned().collect();

        if self.session_id.is_some() {
            push_label(&mut labels, &mut seen, "Session");
        }
        push_label(&mut labels, &mut seen, "Token usage");
        if self.token_usage.context_window.is_some() {
            push_label(&mut labels, &mut seen, "Context window");
        }

        let formatter = FieldFormatter::from_labels(labels.iter().map(String::as_str));
        let _ = formatter.value_width(available_inner_width);

        let _ = super::helpers::format_directory_display(&self.directory, None);
        let _ = &self.agents_summary;

        let mut model_spans = vec![Span::from(self.model_name.clone())];
        if !self.model_details.is_empty() {
            model_spans.push(Span::from(" (").dim());
            model_spans.push(Span::from(self.model_details.join(", ")).dim());
            model_spans.push(Span::from(")").dim());
        }

        lines.push(formatter.line("Model", model_spans));
        lines.push(formatter.line("Approval", vec![Span::from(self.approval.clone())]));
        lines.push(formatter.line("Sandbox", vec![Span::from(self.sandbox.clone())]));
        // Agents.md removed

        if let Some(session) = self.session_id.as_ref() {
            lines.push(formatter.line("Session", vec![Span::from(session.clone())]));
        }

        lines.push(Line::from(Vec::<Span<'static>>::new()));
        lines.push(formatter.line("Token usage", self.token_usage_spans()));

        if let Some(spans) = self.context_window_spans() {
            lines.push(formatter.line("Context window", spans));
        }

        let content_width = lines.iter().map(line_display_width).max().unwrap_or(0);
        let inner_width = content_width.min(available_inner_width);
        let truncated_lines: Vec<Line<'static>> = lines
            .into_iter()
            .map(|line| truncate_line_to_width(line, inner_width))
            .collect();

        with_border_with_inner_width(truncated_lines, inner_width)
    }
}
