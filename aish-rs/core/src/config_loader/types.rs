use aish_utils_absolute_path::AbsolutePathBuf;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value as JsonValue;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigLayerSource {
    Mdm,
    System { file: AbsolutePathBuf },
    User { file: AbsolutePathBuf },
    SessionFlags,
    LegacyManagedConfigTomlFromFile { file: AbsolutePathBuf },
    LegacyManagedConfigTomlFromMdm,
}

impl ConfigLayerSource {
    pub fn precedence(&self) -> u8 {
        match self {
            ConfigLayerSource::Mdm => 0,
            ConfigLayerSource::System { .. } => 1,
            ConfigLayerSource::User { .. } => 2,
            ConfigLayerSource::SessionFlags => 3,
            ConfigLayerSource::LegacyManagedConfigTomlFromFile { .. } => 4,
            ConfigLayerSource::LegacyManagedConfigTomlFromMdm => 5,
        }
    }
}

impl PartialOrd for ConfigLayerSource {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ConfigLayerSource {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.precedence().cmp(&other.precedence())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfigLayerMetadata {
    pub name: ConfigLayerSource,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfigLayer {
    pub name: ConfigLayerSource,
    pub version: String,
    pub config: JsonValue,
}
