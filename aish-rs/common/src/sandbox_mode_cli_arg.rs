//! Standard type to use with the `--sandbox` (`-s`) CLI option.
//!
//! This mirrors the variants of [`aish_core::protocol::SandboxPolicy`], but
//! without any of the associated data so it can be expressed as a simple flag
//! on the command-line. Users that need to tweak the advanced options for
//! `current-dir-write` can continue to do so via `-c` overrides or their
//! `config.toml`.

use aish_protocol::config_types::SandboxMode;
use clap::ValueEnum;

#[derive(Clone, Copy, Debug, ValueEnum)]
#[value(rename_all = "kebab-case")]
pub enum SandboxModeCliArg {
    ReadOnly,
    CurrentDirWrite,
    DangerFullAccess,
}

impl From<SandboxModeCliArg> for SandboxMode {
    fn from(value: SandboxModeCliArg) -> Self {
        match value {
            SandboxModeCliArg::ReadOnly => SandboxMode::ReadOnly,
            SandboxModeCliArg::CurrentDirWrite => SandboxMode::CurrentDirWrite,
            SandboxModeCliArg::DangerFullAccess => SandboxMode::DangerFullAccess,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn maps_cli_args_to_protocol_modes() {
        assert_eq!(SandboxMode::ReadOnly, SandboxModeCliArg::ReadOnly.into());
        assert_eq!(
            SandboxMode::CurrentDirWrite,
            SandboxModeCliArg::CurrentDirWrite.into()
        );
        assert_eq!(
            SandboxMode::DangerFullAccess,
            SandboxModeCliArg::DangerFullAccess.into()
        );
    }
}
