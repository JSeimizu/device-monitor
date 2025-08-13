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
pub enum Component {
    #[serde(rename = "loader")]
    Loader = 0,
    #[serde(rename = "firmware")]
    Firmware = 1,
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

#[derive(Debug, Clone)]
pub struct Firmware {
    pub main_chip: Target,
    pub companion_chip: Target,
    pub sensor_chip: Target,
}

impl Default for Firmware {
    fn default() -> Self {
        Self {
            main_chip: Target {
                component: Component::Firmware,
                chip: "main_chip".to_string(),
                version: String::new(),
                progress: 0,
                process_state: ProcessState::Idle,
                package_url: String::new(),
                hash: String::new(),
                size: 0,
            },
            companion_chip: Target {
                component: Component::Firmware,
                chip: "companion_chip".to_string(),
                version: String::new(),
                progress: 0,
                process_state: ProcessState::Idle,
                package_url: String::new(),
                hash: String::new(),
                size: 0,
            },
            sensor_chip: Target {
                component: Component::Firmware,
                chip: "sensor_chip".to_string(),
                version: String::new(),
                progress: 0,
                process_state: ProcessState::Idle,
                package_url: String::new(),
                hash: String::new(),
                size: 0,
            },
        }
    }
}

impl Firmware {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_chip_by_name(&self, chip_name: &str) -> Option<&Target> {
        match chip_name {
            "main_chip" => Some(&self.main_chip),
            "companion_chip" => Some(&self.companion_chip),
            "sensor_chip" => Some(&self.sensor_chip),
            _ => None,
        }
    }

    pub fn get_chip_by_name_mut(&mut self, chip_name: &str) -> Option<&mut Target> {
        match chip_name {
            "main_chip" => Some(&mut self.main_chip),
            "companion_chip" => Some(&mut self.companion_chip),
            "sensor_chip" => Some(&mut self.sensor_chip),
            _ => None,
        }
    }

    pub fn all_chips(&self) -> [&Target; 3] {
        [&self.main_chip, &self.companion_chip, &self.sensor_chip]
    }

    pub fn all_chips_mut(&mut self) -> [&mut Target; 3] {
        [&mut self.main_chip, &mut self.companion_chip, &mut self.sensor_chip]
    }
}