/*
Copyright [2025] Seimizu Joukan

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
*/

use {
    crate::mqtt_ctrl::evp::ResInfo,
    crate::{app::ConfigKey, error::DMError, mqtt_ctrl::evp::evp_state::UUID},
    error_stack::{Report, Result},
    json::{self, JsonValue, object::Object},
    serde::{Deserialize, Serialize},
    std::fmt::Display,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReqInfo {
    pub req_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum ChipId {
    MainChip = 0,
    CompanionChip = 1,
    SensorChip = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum Component {
    #[serde(rename = "loader")]
    Loader = 0,
    #[serde(rename = "firmware")]
    Firmware = 1,

    Invalid = 2, // Must be the last variant
}

const COMPONENT_COUNT: usize = Component::Invalid as usize;
fn get_index(chip_id: ChipId, component: Component) -> usize {
    (chip_id as usize * COMPONENT_COUNT) + (component as usize)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProcessState {
    #[serde(rename = "idle")]
    Idle,
    #[serde(rename = "request_received")]
    RequestReceived,
    #[serde(rename = "downloading")]
    Downloading,
    #[serde(rename = "installing")]
    Installing,
    #[serde(rename = "done")]
    Done,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "failed_invalid_argument")]
    FailedInvalidArgument,
    #[serde(rename = "failed_token_expired")]
    FailedTokenExpired,
    #[serde(rename = "failed_download_retry_exceeded")]
    FailedDownloadRetryExceeded,
}

impl Default for ProcessState {
    fn default() -> Self {
        ProcessState::Idle
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Target {
    pub component: Component,
    pub chip: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub process_state: Option<ProcessState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum ResponseCode {
    #[serde(rename = "ok")]
    Ok = 0,
    #[serde(rename = "cancelled")]
    Cancelled = 1,
    #[serde(rename = "unknown")]
    Unknown = 2,
    #[serde(rename = "invalid_argument")]
    InvalidArgument = 3,
    #[serde(rename = "deadline_exceeded")]
    DeadlineExceeded = 4,
    #[serde(rename = "not_found")]
    NotFound = 5,
    #[serde(rename = "already_exists")]
    AlreadyExists = 6,
    #[serde(rename = "permission_denied")]
    PermissionDenied = 7,
    #[serde(rename = "resource_exhausted")]
    ResourceExhausted = 8,
    #[serde(rename = "failed_precondition")]
    FailedPrecondition = 9,
    #[serde(rename = "aborted")]
    Aborted = 10,
    #[serde(rename = "out_of_range")]
    OutOfRange = 11,
    #[serde(rename = "unimplemented")]
    Unimplemented = 12,
    #[serde(rename = "internal")]
    Internal = 13,
    #[serde(rename = "unavailable")]
    Unavailable = 14,
    #[serde(rename = "data_loss")]
    DataLoss = 15,
    #[serde(rename = "unauthenticated")]
    Unauthenticated = 16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename = "PRIVATE_deploy_firmware")]
pub struct FirmwareProperty {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub req_info: Option<ReqInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub targets: Option<Vec<Target>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub res_info: Option<ResInfo>,
}

impl Default for FirmwareProperty {
    fn default() -> Self {
        let mut targets = Vec::new();

        let chips = [ChipId::MainChip, ChipId::CompanionChip, ChipId::SensorChip];

        let components = [Component::Loader, Component::Firmware];

        for chip in &chips {
            for &component in &components {
                let chip_name = match chip {
                    ChipId::MainChip => "ApFw",
                    ChipId::CompanionChip => "AI-ISP",
                    ChipId::SensorChip => "IMX500",
                };
                targets.push(Target {
                    component,
                    chip: chip_name.to_string(),
                    version: None,
                    progress: None,
                    process_state: Some(ProcessState::Idle),
                    package_url: None,
                    hash: None,
                    size: None,
                });
            }
        }

        Self {
            req_info: None,
            version: None,
            targets: Some(targets),
            res_info: None,
        }
    }
}

pub fn parse_ota_configuration(config_keys: &[String]) -> Result<String, DMError> {
    let key_value = |key: ConfigKey| -> Option<String> {
        let val = config_keys[key as usize]
            .as_str()
            .trim_matches('"')
            .to_owned();
        if val.is_empty() { None } else { Some(val) }
    };

    let req_id = ReqInfo {
        req_id: UUID::new().uuid().to_string(),
    };

    let version = key_value(ConfigKey::OtaVersion);
    let mut targets = vec![];
    let mut add_target = |component: Component,
                          chip: ConfigKey,
                          version: ConfigKey,
                          url: ConfigKey,
                          hash: ConfigKey,
                          size: ConfigKey| {
        let chip = key_value(chip);
        let version = key_value(version);
        let url = key_value(url);
        let hash = key_value(hash);
        let size = key_value(size).and_then(|size| size.parse::<i32>().ok());

        if chip.is_some()
            && (version.is_some() || url.is_some() || hash.is_some() || size.is_some())
        {
            targets.push(Target {
                component,
                chip: chip.as_ref().unwrap().to_string(),
                version: version,
                progress: None,
                process_state: None,
                package_url: url,
                hash: hash,
                size: size,
            });
        }
    };

    // main chip loader
    add_target(
        Component::Loader,
        ConfigKey::OtaMainChipLoaderChip,
        ConfigKey::OtaMainChipLoaderVersion,
        ConfigKey::OtaMainChipLoaderPackageUrl,
        ConfigKey::OtaMainChipLoaderHash,
        ConfigKey::OtaMainChipLoaderSize,
    );

    // main chip firmware
    add_target(
        Component::Firmware,
        ConfigKey::OtaMainChipFirmwareChip,
        ConfigKey::OtaMainChipFirmwareVersion,
        ConfigKey::OtaMainChipFirmwarePackageUrl,
        ConfigKey::OtaMainChipFirmwareHash,
        ConfigKey::OtaMainChipFirmwareSize,
    );

    // companion chip loader
    add_target(
        Component::Loader,
        ConfigKey::OtaCompanionChipLoaderChip,
        ConfigKey::OtaCompanionChipLoaderVersion,
        ConfigKey::OtaCompanionChipLoaderPackageUrl,
        ConfigKey::OtaCompanionChipLoaderHash,
        ConfigKey::OtaCompanionChipLoaderSize,
    );

    // companion chip firmware
    add_target(
        Component::Firmware,
        ConfigKey::OtaCompanionChipFirmwareChip,
        ConfigKey::OtaCompanionChipFirmwareVersion,
        ConfigKey::OtaCompanionChipFirmwarePackageUrl,
        ConfigKey::OtaCompanionChipFirmwareHash,
        ConfigKey::OtaCompanionChipFirmwareSize,
    );

    // sensor chip loader
    add_target(
        Component::Loader,
        ConfigKey::OtaSensorChipLoaderChip,
        ConfigKey::OtaSensorChipLoaderVersion,
        ConfigKey::OtaSensorChipLoaderPackageUrl,
        ConfigKey::OtaSensorChipLoaderHash,
        ConfigKey::OtaSensorChipLoaderSize,
    );

    // sensor chip firmware
    add_target(
        Component::Firmware,
        ConfigKey::OtaSensorChipFirmwareChip,
        ConfigKey::OtaSensorChipFirmwareVersion,
        ConfigKey::OtaSensorChipFirmwarePackageUrl,
        ConfigKey::OtaSensorChipFirmwareHash,
        ConfigKey::OtaSensorChipFirmwareSize,
    );

    let firmware_property = FirmwareProperty {
        req_info: Some(req_id),
        version,
        targets: Some(targets),
        res_info: None,
    };

    let content = serde_json::to_string(&firmware_property)
        .map_err(|e| Report::new(DMError::InvalidData).attach_printable(e))?;

    let mut root = Object::new();

    root.insert(
        "configuration/$system/PRIVATE_deploy_firmware",
        JsonValue::String(content),
    );

    Ok(json::stringify_pretty(root, 4))
}

#[allow(dead_code)]
impl FirmwareProperty {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_target(&self, chip_id: ChipId, component: Component) -> Option<&Target> {
        let index = get_index(chip_id, component);
        self.targets.as_ref().and_then(|targets| targets.get(index))
    }

    pub fn get_target_mut(&mut self, chip_id: ChipId, component: Component) -> Option<&mut Target> {
        let index = get_index(chip_id, component);
        self.targets
            .as_mut()
            .and_then(|targets| targets.get_mut(index))
    }

    pub fn get_targets_by_chip(&self, chip_name: &str) -> Vec<&Target> {
        self.targets
            .as_ref()
            .and_then(|targets| {
                Some(
                    targets
                        .iter()
                        .filter(|target| target.chip == chip_name)
                        .collect(),
                )
            })
            .unwrap_or_else(Vec::new)
    }

    pub fn get_targets_by_chip_mut(&mut self, chip_id: ChipId) -> Vec<&mut Target> {
        self.targets
            .as_mut()
            .and_then(|targets| {
                let start = chip_id as usize * 2;
                let end = start + 2;
                Some(targets[start..end].iter_mut().collect())
            })
            .unwrap_or_else(Vec::new)
    }

    pub fn get_all_chips(&self) -> Vec<&str> {
        self.targets
            .as_ref()
            .and_then(|targets| {
                let mut result = targets
                    .iter()
                    .map(|target| target.chip.as_str())
                    .collect::<std::collections::HashSet<_>>()
                    .into_iter()
                    .collect::<Vec<_>>();
                result.sort();
                Some(result)
            })
            .unwrap_or_else(Vec::new)
    }

    pub fn get_all_targets(&self) -> Option<&Vec<Target>> {
        self.targets.as_ref()
    }

    pub fn get_all_targets_mut(&mut self) -> Option<&mut Vec<Target>> {
        self.targets.as_mut()
    }
}
