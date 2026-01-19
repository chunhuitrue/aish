mod config_requirements;
mod fingerprint;
mod layer_io;
#[cfg(target_os = "macos")]
mod macos;
mod merge;
mod overrides;
mod state;
pub mod types;
pub use types::*;

#[cfg(test)]
mod tests;

use crate::config::CONFIG_TOML_FILE;
use crate::config::ConfigToml;
use crate::config_loader::config_requirements::ConfigRequirementsToml;
use crate::config_loader::layer_io::LoadedConfigLayers;
use aish_protocol::config_types::SandboxMode;
use aish_protocol::protocol::AskForApproval;
use aish_utils_absolute_path::AbsolutePathBuf;
use aish_utils_absolute_path::AbsolutePathBufGuard;
use serde::Deserialize;
use std::io;
use std::path::Path;
use toml::Value as TomlValue;

pub use config_requirements::ConfigRequirements;
pub use merge::merge_toml_values;
pub use state::ConfigLayerEntry;
pub use state::ConfigLayerStack;
pub use state::ConfigLayerStackOrdering;
pub use state::LoaderOverrides;

/// On Unix systems, load requirements from this file path, if present.
const DEFAULT_REQUIREMENTS_TOML_FILE_UNIX: &str = "/etc/aish/requirements.toml";

/// On Unix systems, load default settings from this file path, if present.
/// Note that /etc/aish/ is treated as a "config folder," so subfolders such
/// as skills/ and rules/ will also be honored.
pub const SYSTEM_CONFIG_TOML_FILE_UNIX: &str = "/etc/aish/config.toml";

/// To build up the set of admin-enforced constraints, we build up from multiple
/// configuration layers in the following order, but a constraint defined in an
/// earlier layer cannot be overridden by a later layer:
///
/// - admin:    managed preferences (*)
/// - system    `/etc/aish/requirements.toml`
///
/// For backwards compatibility, we also load from
/// `/etc/aish/managed_config.toml` and map it to
/// `/etc/aish/requirements.toml`.
///
/// Configuration is built up from multiple layers in the following order:
///
/// - admin:    managed preferences (*)
/// - system    `/etc/aish/config.toml`
/// - user      `${AISH_HOME}/config.toml`
/// - runtime   e.g., --config flags, model selector in UI
///
/// Note: Project-specific config layers (cwd, tree, repo) have been removed.
/// aish no longer searches for `.aish/config.toml` in the current directory,
/// parent directories, or git repository root. Only global config is used.
///
/// (*) Only available on macOS via managed device profiles.
pub async fn load_config_layers_state(
    codex_home: &Path,
    cwd: Option<AbsolutePathBuf>,
    cli_overrides: &[(String, TomlValue)],
    overrides: LoaderOverrides,
) -> io::Result<ConfigLayerStack> {
    let mut config_requirements_toml = ConfigRequirementsToml::default();

    // TODO(gt): Support an entry in MDM for config requirements and use it
    // with `config_requirements_toml.merge_unset_fields(...)`, if present.

    // Honor /etc/aish/requirements.toml.
    if cfg!(unix) {
        load_requirements_toml(
            &mut config_requirements_toml,
            DEFAULT_REQUIREMENTS_TOML_FILE_UNIX,
        )
        .await?;
    }

    // Make a best-effort to support the legacy `managed_config.toml` as a
    // requirements specification.
    let loaded_config_layers = layer_io::load_config_layers_internal(codex_home, overrides).await?;
    load_requirements_from_legacy_scheme(
        &mut config_requirements_toml,
        loaded_config_layers.clone(),
    )
    .await?;

    let mut layers = Vec::<ConfigLayerEntry>::new();

    // TODO(gt): Honor managed preferences (macOS only).

    // Include an entry for the "system" config folder, loading its config.toml,
    // if it exists.
    let system_config_toml_file = if cfg!(unix) {
        Some(AbsolutePathBuf::from_absolute_path(
            SYSTEM_CONFIG_TOML_FILE_UNIX,
        )?)
    } else {
        // TODO(gt): Determine the path to load on Windows.
        None
    };
    if let Some(system_config_toml_file) = system_config_toml_file {
        let system_layer =
            load_config_toml_for_required_layer(&system_config_toml_file, |config_toml| {
                ConfigLayerEntry::new(
                    ConfigLayerSource::System {
                        file: system_config_toml_file.clone(),
                    },
                    config_toml,
                )
            })
            .await?;
        layers.push(system_layer);
    }

    // Add a layer for $AISH_HOME/config.toml if it exists. Note if the file
    // exists, but is malformed, then this error should be propagated to the
    // user.
    let user_file = AbsolutePathBuf::resolve_path_against_base(CONFIG_TOML_FILE, codex_home)?;
    let user_layer = load_config_toml_for_required_layer(&user_file, |config_toml| {
        ConfigLayerEntry::new(
            ConfigLayerSource::User {
                file: user_file.clone(),
            },
            config_toml,
        )
    })
    .await?;
    layers.push(user_layer);

    // Note: Project-specific config loading has been removed.
    // aish no longer searches parent directories for .aish/config.toml.
    // Only the global config (~/.aish/config.toml) is used.
    let _ = cwd; // Suppress unused variable warning

    // Add a layer for runtime overrides from the CLI or UI, if any exist.
    if !cli_overrides.is_empty() {
        let cli_overrides_layer = overrides::build_cli_overrides_layer(cli_overrides);
        layers.push(ConfigLayerEntry::new(
            ConfigLayerSource::SessionFlags,
            cli_overrides_layer,
        ));
    }

    // Make a best-effort to support the legacy `managed_config.toml` as a
    // config layer on top of everything else. For fields in
    // `managed_config.toml` that do not have an equivalent in
    // `ConfigRequirements`, note users can still override these values on a
    // per-turn basis in the TUI and VS Code.
    let LoadedConfigLayers {
        managed_config,
        managed_config_from_mdm,
    } = loaded_config_layers;
    if let Some(config) = managed_config {
        let managed_parent = config.file.as_path().parent().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Managed config file {} has no parent directory",
                    config.file.as_path().display()
                ),
            )
        })?;
        let managed_config =
            resolve_relative_paths_in_config_toml(config.managed_config, managed_parent)?;
        layers.push(ConfigLayerEntry::new(
            ConfigLayerSource::LegacyManagedConfigTomlFromFile { file: config.file },
            managed_config,
        ));
    }
    if let Some(config) = managed_config_from_mdm {
        layers.push(ConfigLayerEntry::new(
            ConfigLayerSource::LegacyManagedConfigTomlFromMdm,
            config,
        ));
    }

    ConfigLayerStack::new(layers, config_requirements_toml.try_into()?)
}

/// Attempts to load a config.toml file from `config_toml`.
/// - If the file exists and is valid TOML, passes the parsed `toml::Value` to
///   `create_entry` and returns the resulting layer entry.
/// - If the file does not exist, uses an empty `Table` with `create_entry` and
///   returns the resulting layer entry.
/// - If there is an error reading the file or parsing the TOML, returns an
///   error.
async fn load_config_toml_for_required_layer(
    config_toml: impl AsRef<Path>,
    create_entry: impl FnOnce(TomlValue) -> ConfigLayerEntry,
) -> io::Result<ConfigLayerEntry> {
    let toml_file = config_toml.as_ref();
    let toml_value = match tokio::fs::read_to_string(toml_file).await {
        Ok(contents) => {
            let config: TomlValue = toml::from_str(&contents).map_err(|e| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Error parsing config file {}: {e}", toml_file.display()),
                )
            })?;
            let config_parent = toml_file.parent().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "Config file {} has no parent directory",
                        toml_file.display()
                    ),
                )
            })?;
            resolve_relative_paths_in_config_toml(config, config_parent)
        }
        Err(e) => {
            if e.kind() == io::ErrorKind::NotFound {
                Ok(TomlValue::Table(toml::map::Map::new()))
            } else {
                Err(io::Error::new(
                    e.kind(),
                    format!("Failed to read config file {}: {e}", toml_file.display()),
                ))
            }
        }
    }?;

    Ok(create_entry(toml_value))
}

/// If available, apply requirements from `/etc/aish/requirements.toml` to
/// `config_requirements_toml` by filling in any unset fields.
async fn load_requirements_toml(
    config_requirements_toml: &mut ConfigRequirementsToml,
    requirements_toml_file: impl AsRef<Path>,
) -> io::Result<()> {
    match tokio::fs::read_to_string(&requirements_toml_file).await {
        Ok(contents) => {
            let requirements_config: ConfigRequirementsToml =
                toml::from_str(&contents).map_err(|e| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "Error parsing requirements file {}: {e}",
                            requirements_toml_file.as_ref().display(),
                        ),
                    )
                })?;
            config_requirements_toml.merge_unset_fields(requirements_config);
        }
        Err(e) => {
            if e.kind() != io::ErrorKind::NotFound {
                return Err(io::Error::new(
                    e.kind(),
                    format!(
                        "Failed to read requirements file {}: {e}",
                        requirements_toml_file.as_ref().display(),
                    ),
                ));
            }
        }
    }

    Ok(())
}

/// from a file like `/etc/aish/managed_config.toml` that has the same
/// shape as `config.toml`, and apply it as a requirement.
async fn load_requirements_from_legacy_scheme(
    config_requirements_toml: &mut ConfigRequirementsToml,
    loaded_config_layers: LoadedConfigLayers,
) -> io::Result<()> {
    // In this implementation, earlier layers cannot be overwritten by later
    // layers, so list managed_config_from_mdm first because it has the highest
    // precedence.
    let LoadedConfigLayers {
        managed_config,
        managed_config_from_mdm,
    } = loaded_config_layers;
    for config in [
        managed_config_from_mdm,
        managed_config.map(|c| c.managed_config),
    ]
    .into_iter()
    .flatten()
    {
        let legacy_config: LegacyManagedConfigToml =
            config.try_into().map_err(|err: toml::de::Error| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Failed to parse config requirements as TOML: {err}"),
                )
            })?;

        let new_requirements_toml = ConfigRequirementsToml::from(legacy_config);
        config_requirements_toml.merge_unset_fields(new_requirements_toml);
    }

    Ok(())
}

/// Takes a `toml::Value` parsed from a config.toml file and walks through it,
/// resolving any `AbsolutePathBuf` fields against `base_dir`, returning a new
/// `toml::Value` with the same shape but with paths resolved.
///
/// This ensures that multiple config layers can be merged together correctly
/// even if they were loaded from different directories.
fn resolve_relative_paths_in_config_toml(
    value_from_config_toml: TomlValue,
    base_dir: &Path,
) -> io::Result<TomlValue> {
    // Use the serialize/deserialize round-trip to convert the
    // `toml::Value` into a `ConfigToml` with `AbsolutePath
    let _guard = AbsolutePathBufGuard::new(base_dir);
    let Ok(resolved) = value_from_config_toml.clone().try_into::<ConfigToml>() else {
        return Ok(value_from_config_toml);
    };
    drop(_guard);

    let resolved_value = TomlValue::try_from(resolved).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Failed to serialize resolved config: {e}"),
        )
    })?;

    Ok(copy_shape_from_original(
        &value_from_config_toml,
        &resolved_value,
    ))
}

/// Ensure that every field in `original` is present in the returned
/// `toml::Value`, taking the value from `resolved` where possible. This ensures
/// the fields that we "removed" during the serialize/deserialize round-trip in
/// `resolve_config_paths` are preserved, out of an abundance of caution.
fn copy_shape_from_original(original: &TomlValue, resolved: &TomlValue) -> TomlValue {
    match (original, resolved) {
        (TomlValue::Table(original_table), TomlValue::Table(resolved_table)) => {
            let mut table = toml::map::Map::new();
            for (key, original_value) in original_table {
                let resolved_value = resolved_table.get(key).unwrap_or(original_value);
                table.insert(
                    key.clone(),
                    copy_shape_from_original(original_value, resolved_value),
                );
            }
            TomlValue::Table(table)
        }
        (TomlValue::Array(original_array), TomlValue::Array(resolved_array)) => {
            let mut items = Vec::new();
            for (index, original_value) in original_array.iter().enumerate() {
                let resolved_value = resolved_array.get(index).unwrap_or(original_value);
                items.push(copy_shape_from_original(original_value, resolved_value));
            }
            TomlValue::Array(items)
        }
        (_, resolved_value) => resolved_value.clone(),
    }
}

/// The legacy mechanism for specifying admin-enforced configuration is to read
/// from a file like `/etc/aish/managed_config.toml` that has the same
/// structure as `config.toml` where fields like `approval_policy` can specify
/// exactly one value rather than a list of allowed values.
///
/// If present, re-interpret `managed_config.toml` as a `requirements.toml`
/// where each specified field is treated as a constraint allowing only that
/// value.
#[derive(Deserialize, Debug, Clone, Default, PartialEq)]
struct LegacyManagedConfigToml {
    approval_policy: Option<AskForApproval>,
    sandbox_mode: Option<SandboxMode>,
}

impl From<LegacyManagedConfigToml> for ConfigRequirementsToml {
    fn from(legacy: LegacyManagedConfigToml) -> Self {
        let mut config_requirements_toml = ConfigRequirementsToml::default();

        let LegacyManagedConfigToml {
            approval_policy,
            sandbox_mode,
        } = legacy;
        if let Some(approval_policy) = approval_policy {
            config_requirements_toml.allowed_approval_policies = Some(vec![approval_policy]);
        }
        if let Some(sandbox_mode) = sandbox_mode {
            config_requirements_toml.allowed_sandbox_modes = Some(vec![sandbox_mode.into()]);
        }
        config_requirements_toml
    }
}

// Cannot name this `mod tests` because of tests.rs in this folder.
#[cfg(test)]
mod unit_tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn ensure_resolve_relative_paths_in_config_toml_preserves_all_fields() -> anyhow::Result<()> {
        let tmp = tempdir()?;
        let base_dir = tmp.path();
        let contents = r#"
# This is a field recognized by config.toml that is an AbsolutePathBuf in
# the ConfigToml struct.
experimental_instructions_file = "./some_file.md"

# This is a field recognized by config.toml.
model = "gpt-1000"

# This is a field not recognized by config.toml.
foo = "xyzzy"
"#;
        let user_config: TomlValue = toml::from_str(contents)?;

        let normalized_toml_value = resolve_relative_paths_in_config_toml(user_config, base_dir)?;
        let mut expected_toml_value = toml::map::Map::new();
        expected_toml_value.insert(
            "experimental_instructions_file".to_string(),
            TomlValue::String(
                AbsolutePathBuf::resolve_path_against_base("./some_file.md", base_dir)?
                    .as_path()
                    .to_string_lossy()
                    .to_string(),
            ),
        );
        expected_toml_value.insert(
            "model".to_string(),
            TomlValue::String("gpt-1000".to_string()),
        );
        expected_toml_value.insert("foo".to_string(), TomlValue::String("xyzzy".to_string()));
        assert_eq!(normalized_toml_value, TomlValue::Table(expected_toml_value));
        Ok(())
    }
}
