```text
                 _____    _____   _    _
         /\     |_   _|  / ____| | |  | |
        /  \      | |   | (___   | |__| |
       / /\ \     | |    \___ \  |  __  |
      / ____ \   _| |_   ____) | | |  | |
     /_/    \_\ |_____| |_____/  |_|  |_|
```


# Aish
Aish is an AI-powered shell assistant designed for command-line environments. If you forget or are unsure how to write complex shell commands while working in the terminal, you can call aish from any directory to get instant assistance. Aish does not need to be executed within a specific project directory.

Before using aish, you need to add a configuration file at `~/.aish/config.toml` with the following template:

```toml
profile                 = "glm"
model_context_window    = 200000

[model_providers.zhipu]
name                   = "GLM Coding Plan"
base_url               = "https://open.bigmodel.cn/api/coding/paas/v4"
env_key                = "ZHIPU_API_KEY"
wire_api               = "chat"
request_max_retries    = 4
stream_max_retries     = 10
stream_idle_timeout_ms = 300000

[profiles.glm]
model          = "glm-4.7"
model_provider = "zhipu"
```


## Build and Install

To build and install aish, follow these steps:

```shell
cd aish-rs
cargo build --release
cp target/release/aish ~/bin/
```


# Usage

aish is read-only by default. If you need it to perform write operations, you can grant write permissions through /approvals.


# Credits

Modified Base on: [OpenAI Codex](https://github.com/openai/codex).


# License

[Apache-2.0 License](LICENSE).

