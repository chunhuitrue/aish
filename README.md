This software was modified by LiChunhui(chunhui_true@163.com)
Base on Original project: [OpenAI Codex](https://github.com/openai/codex).

Aish is an AI-powered shell assistant designed for command-line environments. If you forget or are unsure how to write complex shell commands while working in the terminal, you can call aish from any directory to get instant assistance. Aish does not need to be executed within a specific project directory.

Before using aish, you need to add a configuration file at `~/.aish/config.toml` with the following template:

```toml
profile                     = "glm"
model_context_window        = 200000
check_for_update_on_startup = false

[model_providers.zhipu]
name                   = "GLM Coding Plan"
base_url               = "https://open.bigmodel.cn/api/coding/paas/v4"
env_key                = "ZHIPU_API_KEY"
wire_api               = "chat"
requires_openai_auth   = false
request_max_retries    = 4
stream_max_retries     = 10
stream_idle_timeout_ms = 300000

[profiles.glm]
model          = "glm-4.7"
model_provider = "zhipu"
```

---

## License

This repository is licensed under the [Apache-2.0 License](LICENSE).
