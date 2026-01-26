# Authentication

Aish authenticates using the active model provider configuration from `~/.aish/config.toml`.

## Setting up your API key

### Recommended: provider env_key

Configure an environment variable name on your active model provider via `env_key`, then export it before launching Aish.

Example:

```toml
[model_providers.openai]
env_key = "AISH_MODEL_API_KEY"
```

```bash
export AISH_MODEL_API_KEY="sk-your-key-here"
aish
```

### Alternative: experimental bearer token

For development-only setups, you can set a direct bearer token in `config.toml`:

```toml
[model_providers.openai]
experimental_bearer_token = "sk-your-key-here"
```

## Model provider specific keys

Some model providers use their own environment variables. For example, if you're using a custom model provider configured in `config.toml`, you may need to set a provider-specific environment key. Check the [configuration documentation](./config.md#model-providers) for details.

## Verifying your setup

To verify that your API key is correctly configured, run:

```shell
aish
```

If your API key is set correctly, Aish will start without authentication errors.

## Troubleshooting

### API key not found

If you see an error about a missing API key:

1. Verify `model_providers.<id>.env_key` is configured for your active provider.
2. Verify the referenced environment variable is set and non-empty.
3. If using `experimental_bearer_token`, verify it is set for the active provider.

### Invalid API key

If you receive authentication errors:

1. Verify your API key is valid and active
2. Check that the API key has the necessary permissions
3. Ensure you're using the correct API key for the model provider you've configured
