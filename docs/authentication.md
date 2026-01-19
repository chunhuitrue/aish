# Authentication

Aish uses environment variables to authenticate with API providers. This is the only supported authentication method.

## Setting up your API key

### OpenAI API key

Set your OpenAI API key via the `OPENAI_API_KEY` environment variable:

```shell
export OPENAI_API_KEY="sk-your-key-here"
```

You can add this to your shell profile (e.g., `~/.bashrc`, `~/.zshrc`) to make it persistent:

```bash
echo 'export OPENAI_API_KEY="sk-your-key-here"' >> ~/.bashrc
source ~/.bashrc
```

### Aish API key

Alternatively, you can use the `AISH_API_KEY` environment variable:

```shell
export AISH_API_KEY="sk-your-key-here"
```

`AISH_API_KEY` takes precedence over `OPENAI_API_KEY` if both are set.

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

1. Verify the environment variable is set:
   ```shell
   echo $OPENAI_API_KEY
   # or
   echo $AISH_API_KEY
   ```

2. Make sure you've exported the variable in your current shell session:

   ```shell
   export OPENAI_API_KEY="sk-your-key-here"
   ```

3. If using a model provider's custom `env_key`, verify it's set correctly.

### Invalid API key

If you receive authentication errors:

1. Verify your API key is valid and active
2. Check that the API key has the necessary permissions
3. Ensure you're using the correct API key for the model provider you've configured
