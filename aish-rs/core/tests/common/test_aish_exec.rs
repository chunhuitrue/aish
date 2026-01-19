#![allow(clippy::expect_used)]
use aish_core::auth::AISH_API_KEY_ENV_VAR;
use std::fs;
use std::path::Path;
use tempfile::TempDir;
use wiremock::MockServer;

pub struct TestAishExecBuilder {
    home: TempDir,
    cwd: TempDir,
}

impl TestAishExecBuilder {
    pub fn cmd(&self) -> assert_cmd::Command {
        let mut cmd = assert_cmd::Command::new(
            aish_utils_cargo_bin::cargo_bin("aish-exec").expect("should find binary for aish-exec"),
        );
        cmd.current_dir(self.cwd.path())
            .env("AISH_HOME", self.home.path())
            .env(AISH_API_KEY_ENV_VAR, "dummy");
        cmd
    }
    pub fn cmd_with_server(&self, server: &MockServer) -> assert_cmd::Command {
        let mut cmd = self.cmd();
        let base = format!("{}/v1", server.uri());
        cmd.env("OPENAI_BASE_URL", base);
        cmd
    }

    pub fn cwd_path(&self) -> &Path {
        self.cwd.path()
    }
    pub fn home_path(&self) -> &Path {
        self.home.path()
    }
}

pub fn test_aish_exec() -> TestAishExecBuilder {
    let home = TempDir::new().expect("create temp home");
    // Create a config file with a default model since built-in presets have been removed
    // Also enable include_apply_patch_tool for apply_patch tests
    let config_content = r#"model = "test-model"
include_apply_patch_tool = true
"#;
    fs::write(home.path().join("config.toml"), config_content).expect("write default config.toml");
    TestAishExecBuilder {
        home,
        cwd: TempDir::new().expect("create temp cwd"),
    }
}
