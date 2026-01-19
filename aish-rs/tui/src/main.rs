use aish_arg0::arg0_dispatch_or_else;
use aish_common::CliConfigOverrides;
use aish_tui::Cli;
use aish_tui::run_main;
use clap::Parser;

#[derive(Parser, Debug)]
struct TopCli {
    #[clap(flatten)]
    config_overrides: CliConfigOverrides,

    #[clap(flatten)]
    inner: Cli,
}

fn main() -> anyhow::Result<()> {
    arg0_dispatch_or_else(|aish_linux_sandbox_exe| async move {
        let top_cli = TopCli::parse();
        let mut inner = top_cli.inner;
        inner
            .config_overrides
            .raw_overrides
            .splice(0..0, top_cli.config_overrides.raw_overrides);
        let exit_info = run_main(inner, aish_linux_sandbox_exe).await?;
        let token_usage = exit_info.token_usage;
        if !token_usage.is_zero() {
            println!("{}", aish_core::protocol::FinalOutput::from(token_usage),);
        }
        Ok(())
    })
}
