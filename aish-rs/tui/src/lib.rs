// Forbid accidental stdout/stderr writes in the *library* portion of the TUI.
// The standalone `codex-tui` binary prints a short help message before the
// alternate‑screen mode starts; that file opts‑out locally via `allow`.
#![deny(clippy::print_stdout, clippy::print_stderr)]
#![deny(clippy::disallowed_methods)]
use additional_dirs::add_dir_warning_message;
use aish_common::oss::ensure_oss_provider_ready;
use aish_common::oss::get_default_model_for_oss_provider;
use aish_core::INTERACTIVE_SESSION_SOURCES;
use aish_core::RolloutRecorder;
use aish_core::auth::read_aish_api_key_from_env;
use aish_core::auth::read_openai_api_key_from_env;
use aish_core::config::Config;
use aish_core::config::ConfigOverrides;
use aish_core::config::find_codex_home;
use aish_core::config::load_config_as_toml_with_cli_overrides;
use aish_core::config::resolve_oss_provider;
use aish_core::find_conversation_path_by_id_str;
use aish_core::protocol::AskForApproval;
use aish_protocol::config_types::SandboxMode;
use aish_utils_absolute_path::AbsolutePathBuf;
use app::App;
pub use app::AppExitInfo;
use std::fs::OpenOptions;
use std::path::PathBuf;
use tracing::error;
use tracing_appender::non_blocking;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::prelude::*;

mod additional_dirs;
mod app;
mod app_backtrack;
mod app_event;
mod app_event_sender;
mod bottom_pane;
mod chatwidget;
mod cli;
mod clipboard_paste;
mod color;
pub mod custom_terminal;
mod diff_render;
mod exec_cell;
mod exec_command;
mod external_editor;
mod file_search;
mod history_cell;
pub mod insert_history;
mod key_hint;
pub mod live_wrap;
mod markdown;
mod markdown_render;
mod markdown_stream;
mod notifications;
mod oss_selection;
mod pager_overlay;
pub mod public_widgets;
mod render;
mod resume_picker;
mod session_log;
mod shimmer;
mod slash_command;
mod status;
mod status_indicator_widget;
mod streaming;
mod style;
mod terminal_palette;
mod text_formatting;
mod tui;
mod ui_consts;
mod version;

mod wrapping;

#[cfg(test)]
pub mod test_backend;

use crate::tui::Tui;
pub use cli::Cli;
pub use markdown_render::render_markdown_text;
pub use public_widgets::composer_input::ComposerAction;
pub use public_widgets::composer_input::ComposerInput;
use std::io::Write as _;

// (tests access modules directly within the crate)

pub async fn run_main(
    mut cli: Cli,
    aish_linux_sandbox_exe: Option<PathBuf>,
) -> std::io::Result<AppExitInfo> {
    let (sandbox_mode, approval_policy) = if cli.full_auto {
        (
            Some(SandboxMode::CurrentDirWrite),
            Some(AskForApproval::OnRequest),
        )
    } else if cli.dangerously_bypass_approvals_and_sandbox {
        (
            Some(SandboxMode::DangerFullAccess),
            Some(AskForApproval::Never),
        )
    } else {
        (
            cli.sandbox_mode.map(Into::<SandboxMode>::into),
            cli.approval_policy.map(Into::into),
        )
    };

    // Map the legacy --search flag to the new feature toggle.
    if cli.web_search {
        cli.config_overrides
            .raw_overrides
            .push("features.web_search_request=true".to_string());
    }

    // When using `--oss`, let the bootstrapper pick the model (defaulting to
    // gpt-oss:20b) and ensure it is present locally. Also, force the built‑in
    let raw_overrides = cli.config_overrides.raw_overrides.clone();
    // `oss` model provider.
    let overrides_cli = aish_common::CliConfigOverrides { raw_overrides };
    let cli_kv_overrides = match overrides_cli.parse_overrides() {
        // Parse `-c` overrides from the CLI.
        Ok(v) => v,
        #[allow(clippy::print_stderr)]
        Err(e) => {
            eprintln!("Error parsing -c overrides: {e}");
            std::process::exit(1);
        }
    };

    // we load config.toml here to determine project state.
    #[allow(clippy::print_stderr)]
    let codex_home = match find_codex_home() {
        Ok(codex_home) => codex_home.to_path_buf(),
        Err(err) => {
            eprintln!("Error finding aish home: {err}");
            std::process::exit(1);
        }
    };

    let cwd = cli.cwd.clone();
    let config_cwd = match cwd.as_deref() {
        Some(path) => AbsolutePathBuf::from_absolute_path(path.canonicalize()?)?,
        None => AbsolutePathBuf::current_dir()?,
    };

    #[allow(clippy::print_stderr)]
    let config_toml = match load_config_as_toml_with_cli_overrides(
        &codex_home,
        &config_cwd,
        cli_kv_overrides.clone(),
    )
    .await
    {
        Ok(config_toml) => config_toml,
        Err(err) => {
            eprintln!("Error loading config.toml: {err}");
            std::process::exit(1);
        }
    };

    let model_provider_override = if cli.oss {
        let resolved = resolve_oss_provider(
            cli.oss_provider.as_deref(),
            &config_toml,
            cli.config_profile.clone(),
        );

        if let Some(provider) = resolved {
            Some(provider)
        } else {
            // No provider configured, prompt the user
            let provider = oss_selection::select_oss_provider(&codex_home).await?;
            if provider == "__CANCELLED__" {
                return Err(std::io::Error::other(
                    "OSS provider selection was cancelled by user",
                ));
            }
            Some(provider)
        }
    } else {
        None
    };

    // When using `--oss`, let the bootstrapper pick the model based on selected provider
    let model = if let Some(model) = &cli.model {
        Some(model.clone())
    } else if cli.oss {
        // Use the provider from model_provider_override
        model_provider_override
            .as_ref()
            .and_then(|provider_id| get_default_model_for_oss_provider(provider_id))
            .map(std::borrow::ToOwned::to_owned)
    } else {
        None // No model specified, will use the default.
    };

    let overrides = ConfigOverrides {
        model,
        approval_policy,
        sandbox_mode,
        cwd,
        model_provider: model_provider_override.clone(),
        config_profile: cli.config_profile.clone(),
        aish_linux_sandbox_exe,
        show_raw_agent_reasoning: cli.oss.then_some(true),
        ..Default::default()
    };

    let config = load_config_or_exit(cli_kv_overrides.clone(), overrides.clone()).await;

    // Check if there's a valid model configuration
    // If no model is configured and no API key is available, show a helpful message
    let has_api_key = {
        read_openai_api_key_from_env().is_some()
            || read_aish_api_key_from_env().is_some()
            || config
                .model_provider
                .env_key
                .as_ref()
                .is_some_and(|k| std::env::var(k).is_ok_and(|v| !v.is_empty()))
    };
    #[allow(clippy::print_stderr)]
    if config.model.is_none() && !has_api_key {
        eprintln!("No API key configured. Please configure one of the following:");
        eprintln!();
        eprintln!("  1. Set environment variable OPENAI_API_KEY or AISH_API_KEY");
        eprintln!("  2. Configure model_provider and API key in ~/.aish/config.toml");
        eprintln!();
        std::process::exit(1);
    }

    if let Some(warning) = add_dir_warning_message(&cli.add_dir, config.sandbox_policy.get()) {
        #[allow(clippy::print_stderr)]
        {
            eprintln!("Error adding directories: {warning}");
            std::process::exit(1);
        }
    }

    let active_profile = config.active_profile.clone();
    let log_dir = aish_core::config::log_dir(&config)?;
    std::fs::create_dir_all(&log_dir)?;
    // Open (or create) your log file, appending to it.
    let mut log_file_opts = OpenOptions::new();
    log_file_opts.create(true).append(true);

    // Ensure the file is only readable and writable by the current user.
    // Doing the equivalent to `chmod 600` on Windows is quite a bit more code
    // and requires the Windows API crates, so we can reconsider that when
    // Codex CLI is officially supported on Windows.
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        log_file_opts.mode(0o600);
    }

    let log_file = log_file_opts.open(log_dir.join("aish-tui.log"))?;

    // Wrap file in non‑blocking writer.
    let (non_blocking, _guard) = non_blocking(log_file);

    // use RUST_LOG env var, default to info for aish crates.
    let env_filter = || {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            EnvFilter::new("aish_core=info,aish_tui=info,aish_rmcp_client=info")
        })
    };

    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking)
        // `with_target(true)` is the default, but we previously disabled it for file output.
        // Keep it enabled so we can selectively enable targets via `RUST_LOG=...` and then
        // grep for a specific module/target while troubleshooting.
        .with_target(true)
        .with_ansi(false)
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::FULL)
        .with_filter(env_filter());

    if cli.oss && model_provider_override.is_some() {
        // We're in the oss section, so provider_id should be Some
        // Let's handle None case gracefully though just in case
        let provider_id = match model_provider_override.as_ref() {
            Some(id) => id,
            None => {
                error!("OSS provider unexpectedly not set when oss flag is used");
                return Err(std::io::Error::other(
                    "OSS provider not set but oss flag was used",
                ));
            }
        };
        ensure_oss_provider_ready(provider_id, &config).await?;
    }

    let _ = tracing_subscriber::registry().with(file_layer).try_init();

    run_ratatui_app(cli, config, overrides, cli_kv_overrides, active_profile)
        .await
        .map_err(|err| std::io::Error::other(err.to_string()))
}

async fn run_ratatui_app(
    cli: Cli,
    initial_config: Config,
    _overrides: ConfigOverrides,
    _cli_kv_overrides: Vec<(String, toml::Value)>,
    active_profile: Option<String>,
) -> color_eyre::Result<AppExitInfo> {
    color_eyre::install()?;

    // Forward panic reports through tracing so they appear in the UI status
    // line, but do not swallow the default/color-eyre panic handler.
    // Chain to the previous hook so users still get a rich panic report
    // (including backtraces) after we restore the terminal.
    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        tracing::error!("panic: {info}");
        prev_hook(info);
    }));
    let mut terminal = tui::init()?;
    terminal.clear()?;

    let mut tui = Tui::new(terminal);

    // Initialize high-fidelity session event logging if enabled.
    session_log::maybe_init(&initial_config);

    // Onboarding has been disabled - use initial_config directly
    let config = initial_config;

    // Determine resume behavior: explicit id, then resume last, then picker.
    let resume_selection = if let Some(id_str) = cli.resume_session_id.as_deref() {
        match find_conversation_path_by_id_str(&config.codex_home, id_str).await? {
            Some(path) => resume_picker::ResumeSelection::Resume(path),
            None => {
                error!("Error finding conversation path: {id_str}");
                restore();
                session_log::log_session_end();
                let _ = tui.terminal.clear();
                if let Err(err) = writeln!(
                    std::io::stdout(),
                    "No saved session found with ID {id_str}. Run `aish resume` without an ID to choose from existing sessions."
                ) {
                    error!("Failed to write resume error message: {err}");
                }
                return Ok(AppExitInfo {
                    token_usage: aish_core::protocol::TokenUsage::default(),
                    conversation_id: None,
                });
            }
        }
    } else if cli.resume_last {
        let provider_filter = vec![config.model_provider_id.clone()];
        match RolloutRecorder::list_conversations(
            &config.codex_home,
            1,
            None,
            INTERACTIVE_SESSION_SOURCES,
            Some(provider_filter.as_slice()),
            &config.model_provider_id,
        )
        .await
        {
            Ok(page) => page
                .items
                .first()
                .map(|it| resume_picker::ResumeSelection::Resume(it.path.clone()))
                .unwrap_or(resume_picker::ResumeSelection::StartFresh),
            Err(_) => resume_picker::ResumeSelection::StartFresh,
        }
    } else if cli.resume_picker {
        match resume_picker::run_resume_picker(
            &mut tui,
            &config.codex_home,
            &config.model_provider_id,
        )
        .await?
        {
            resume_picker::ResumeSelection::Exit => {
                restore();
                session_log::log_session_end();
                return Ok(AppExitInfo {
                    token_usage: aish_core::protocol::TokenUsage::default(),
                    conversation_id: None,
                });
            }
            other => other,
        }
    } else {
        resume_picker::ResumeSelection::StartFresh
    };

    let Cli { prompt, images, .. } = cli;

    let app_result = App::run(
        &mut tui,
        config,
        active_profile,
        prompt,
        images,
        resume_selection,
        false, // Trust screen is always skipped now
    )
    .await;

    restore();
    // Mark the end of the recorded session.
    session_log::log_session_end();
    // ignore error when collecting usage – report underlying error instead
    app_result
}

#[expect(
    clippy::print_stderr,
    reason = "TUI should no longer be displayed, so we can write to stderr."
)]
fn restore() {
    if let Err(err) = tui::restore() {
        eprintln!(
            "failed to restore terminal. Run `reset` or restart your terminal to recover: {err}"
        );
    }
}

async fn load_config_or_exit(
    cli_kv_overrides: Vec<(String, toml::Value)>,
    overrides: ConfigOverrides,
) -> Config {
    #[allow(clippy::print_stderr)]
    match Config::load_with_cli_overrides_and_harness_overrides(cli_kv_overrides, overrides).await {
        Ok(config) => config,
        Err(err) => {
            eprintln!("Error loading configuration: {err}");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    // Tests removed - login screen functionality has been completely removed
}
