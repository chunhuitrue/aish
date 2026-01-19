## Advanced

If you already lean on Aish every day and just need a little more control, this page collects the knobs you are most likely to reach for: tweak defaults in [Config](./config.md), add extra tools through [Model Context Protocol support](#model-context-protocol), and script full runs with [`aish exec`](./exec.md). Jump to the section you need and keep building.

## Config quickstart

Most day-to-day tuning lives in `config.toml`: set approval + sandbox presets, pin model defaults, and add MCP server launchers. The [Config guide](./config.md) walks through every option and provides copy-paste examples for common setups.

## Tracing / verbose logging

Because Aish is written in Rust, it honors the `RUST_LOG` environment variable to configure its logging behavior.

The TUI defaults to `RUST_LOG=aish_core=info,aish_tui=info,aish_rmcp_client=info` and log messages are written to `~/.aish/log/aish-tui.log`, so you can leave the following running in a separate terminal to monitor log messages as they are written:

```bash
tail -F ~/.aish/log/aish-tui.log
```

By comparison, the non-interactive mode (`aish exec`) defaults to `RUST_LOG=error`, but messages are printed inline, so there is no need to monitor a separate file.

See the Rust documentation on [`RUST_LOG`](https://docs.rs/env_logger/latest/env_logger/#enabling-logging) for more information on the configuration options.

## Model Context Protocol (MCP)

The Aish CLI is an MCP client which means that it can be configured to connect to MCP servers. For more information, refer to the [`config docs`](./config.md#mcp-integration).

## Using Aish with MCP

Use `aish mcp` to add/list/get/remove MCP server launchers in your configuration. Running Aish itself as an MCP server is not supported in this project.

### MCP Quickstart
You can use the [Model Context Protocol Inspector](https://modelcontextprotocol.io/legacy/tools/inspector) to explore MCP servers and verify configuration.

### Trying it Out

> [!TIP]
> Aish often takes a few minutes to run. To accommodate this, adjust the MCP inspector's Request and Total timeouts to 600000ms (10 minutes) under ⛭ Configuration.

Use the MCP inspector and your configured servers to build a simple tic-tac-toe game with the following settings:

**approval-policy:** never

**prompt:** Implement a simple tic-tac-toe game with HTML, JavaScript, and CSS. Write the game in a single file called index.html.

**sandbox:** workspace-write

Click \"Run Tool\" and you should see a list of events emitted from the MCP server as it builds the game.
