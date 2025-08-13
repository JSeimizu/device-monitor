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

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
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
}

fn get_index(chip_id: ChipId, component: Component) -> usize {
    (chip_id as usize * 2) + (component as usize)
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Target {
    pub component: Component,
    pub chip: String,
    pub version: String,
    pub progress: i32,
    pub process_state: ProcessState,
    pub package_url: String,
    pub hash: String,
    pub size: i32,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResInfo {
    pub res_id: String,
    pub code: ResponseCode,
    pub detail_msg: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirmwareProperty {
    pub req_info: ReqInfo,
    pub version: String,
    pub targets: Vec<Target>,
    pub res_info: ResInfo,
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
                    version: String::new(),
                    progress: 0,
                    process_state: ProcessState::Idle,
                    package_url: String::new(),
                    hash: String::new(),
                    size: 0,
                });
            }
        }

        Self {
            req_info: ReqInfo {
                req_id: String::new(),
            },
            version: String::new(),
            targets,
            res_info: ResInfo {
                res_id: String::new(),
                code: ResponseCode::Ok,
                detail_msg: String::new(),
            },
        }
    }
}

impl FirmwareProperty {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_target(&self, chip_id: ChipId, component: Component) -> Option<&Target> {
        let index = get_index(chip_id, component);
        self.targets.get(index)
    }

    pub fn get_target_mut(&mut self, chip_id: ChipId, component: Component) -> Option<&mut Target> {
        let index = get_index(chip_id, component);
        self.targets.get_mut(index)
    }

    pub fn get_targets_by_chip(&self, chip_name: &str) -> Vec<&Target> {
        self.targets
            .iter()
            .filter(|target| target.chip == chip_name)
            .collect()
    }

    pub fn get_targets_by_chip_mut(&mut self, chip_id: ChipId) -> Vec<&mut Target> {
        let start = chip_id as usize * 2;
        let end = start + 2;
        self.targets[start..end].iter_mut().collect()
    }

    pub fn get_all_chips(&self) -> Vec<&str> {
        let mut chips: Vec<&str> = self
            .targets
            .iter()
            .map(|target| target.chip.as_str())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        chips.sort();
        chips
    }

    pub fn get_all_targets(&self) -> &Vec<Target> {
        &self.targets
    }

    pub fn get_all_targets_mut(&mut self) -> &mut Vec<Target> {
        &mut self.targets
    }
}
