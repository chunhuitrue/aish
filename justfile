set working-directory := "aish-rs"

set positional-arguments

# Display help
help:
    just -l

# `aish`
alias a := aish
aish *args:
    cargo run --bin aish -- "$@"

# `aish exec`
exec *args:
    cargo run --bin aish -- exec "$@"

# `aish tui`
tui *args:
    cargo run --bin aish -- tui "$@"

# Run the CLI version of the file-search crate.
file-search *args:
    cargo run --bin aish-file-search -- "$@"

# Build the CLI and run the app-server test client
app-server-test-client *args:
    cargo build -p aish-cli
    cargo run -p aish-app-server-test-client -- --aish-bin ./target/debug/aish "$@"

# format code
fmt:
    cargo fmt -- --config imports_granularity=Item

fix *args:
    cargo clippy --fix --all-features --tests --allow-dirty "$@"

clippy:
    cargo clippy --all-features --tests "$@"

install:
    rustup show active-toolchain
    cargo fetch

# Run `cargo nextest` since it's faster than `cargo test`, though including
# --no-fail-fast is important to ensure all tests are run.
#
# Run `cargo install cargo-nextest` if you don't have it installed.
test:
    cargo nextest run --no-fail-fast

# Run the MCP server
mcp-server-run *args:
    cargo run -p aish-mcp-server -- "$@"
