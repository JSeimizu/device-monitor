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

use crate::mqtt_ctrl::evp::{ProcessState, ReqInfo, ResInfo};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct Target {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chip: Option<String>,
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
    pub size: Option<i64>,
}

impl Target {
    pub fn new_idle() -> Self {
        Self {
            chip: Some(String::new()),
            version: Some(String::new()),
            progress: Some(0),
            process_state: Some(ProcessState::Idle),
            package_url: Some(String::new()),
            hash: Some(String::new()),
            size: Some(0),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct AiModel {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub req_info: Option<ReqInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub targets: Option<Vec<Target>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub res_info: Option<ResInfo>,
}

impl AiModel {
    pub fn new() -> Self {
        Self {
            req_info: Some(ReqInfo {
                req_id: String::new(),
            }),
            targets: Some(vec![
                Target::new_idle(),
                Target::new_idle(),
                Target::new_idle(),
                Target::new_idle(),
            ]),
            res_info: Some(ResInfo::default()),
        }
    }

    pub fn targets(&self) -> &[Target] {
        self.targets.as_deref().unwrap_or(&[])
    }

    pub fn req_info(&self) -> Option<&ReqInfo> {
        self.req_info.as_ref()
    }

    pub fn res_info(&self) -> Option<&ResInfo> {
        self.res_info.as_ref()
    }
}
