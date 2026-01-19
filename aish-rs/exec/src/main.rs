//! Entry-point for the `aish-exec` binary.
//!
//! When this CLI is invoked normally, it parses the standard `aish-exec` CLI
//! options and launches the non-interactive aish agent. However, if it is
//! invoked with arg0 as `aish-linux-sandbox`, we instead treat the invocation
//! as a request to run the logic for the standalone `aish-linux-sandbox`
//! executable (i.e., parse any -s args and then run a *sandboxed* command under
//! Landlock + seccomp.
//!
//! This allows us to ship a completely separate set of functionality as part
//! of the `aish-exec` binary.
use aish_arg0::arg0_dispatch_or_else;
use aish_common::CliConfigOverrides;
use aish_exec::Cli;
use aish_exec::run_main;
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
        // Merge root-level overrides into inner CLI struct so downstream logic remains unchanged.
        let mut inner = top_cli.inner;
        inner
            .config_overrides
            .raw_overrides
            .splice(0..0, top_cli.config_overrides.raw_overrides);

        run_main(inner, aish_linux_sandbox_exe).await?;
        Ok(())
    })
}
