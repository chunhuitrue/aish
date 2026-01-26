#![allow(clippy::expect_used)]
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
            .env("AISH_MODEL_API_KEY", "dummy");
        cmd
    }
    pub fn cmd_with_server(&self, server: &MockServer) -> assert_cmd::Command {
        let cmd = self.cmd();
        let base = format!("{}/v1", server.uri());
        let config_path = self.home.path().join("config.toml");
        let config = fs::read_to_string(&config_path).expect("read config.toml");
        let config = config.replace("http://127.0.0.1:1/v1", &base);
        fs::write(&config_path, config).expect("write updated config.toml");
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
model_provider = "test_provider"
include_apply_patch_tool = true

[model_providers.test_provider]
name = "Test Provider"
base_url = "http://127.0.0.1:1/v1"
wire_api = "responses"
env_key = "AISH_MODEL_API_KEY"
"#;
    fs::write(home.path().join("config.toml"), config_content).expect("write default config.toml");
    TestAishExecBuilder {
        home,
        cwd: TempDir::new().expect("create temp cwd"),
    }
}
