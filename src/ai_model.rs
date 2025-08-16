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

#[allow(unused)]
use {
    crate::{
        app::ConfigKey,
        error::DMError,
        mqtt_ctrl::evp::ProcessState,
        mqtt_ctrl::evp::evp_state::UUID,
        mqtt_ctrl::evp::{ReqInfo, ResInfo},
    },
    error_stack::{Report, Result},
    jlogger_tracing::{JloggerBuilder, LevelFilter, LogTimeFormat, jdebug, jerror, jinfo},
    json::{self, JsonValue, object::Object},
    serde::{Deserialize, Serialize},
};

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq)]
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
    pub size: Option<i32>,
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

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq)]
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

pub fn parse_ai_model_configuration(config_keys: &[String]) -> Result<String, DMError> {
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

    let mut targets = vec![];
    let mut add_target =
        |chip: ConfigKey, version: ConfigKey, url: ConfigKey, hash: ConfigKey, size: ConfigKey| {
            let chip = key_value(chip);
            let version = key_value(version);
            let url = key_value(url);
            let hash = key_value(hash);
            let size = key_value(size).and_then(|size| size.parse::<i32>().ok());
            if let Some(chip_value) = &chip {
                if version.is_some() || url.is_some() || hash.is_some() || size.is_some() {
                    targets.push(Target {
                        chip: Some(chip_value.to_string()),
                        version,
                        progress: None,
                        process_state: None,
                        package_url: url,
                        hash,
                        size,
                    });
                }
            }
        };

    // Ai Model 0
    add_target(
        ConfigKey::AiModel0Chip,
        ConfigKey::AiModel0Version,
        ConfigKey::AiModel0PackageUrl,
        ConfigKey::AiModel0Hash,
        ConfigKey::AiModel0Size,
    );

    // Ai Model 1
    add_target(
        ConfigKey::AiModel1Chip,
        ConfigKey::AiModel1Version,
        ConfigKey::AiModel1PackageUrl,
        ConfigKey::AiModel1Hash,
        ConfigKey::AiModel1Size,
    );

    // Ai Model 2
    add_target(
        ConfigKey::AiModel2Chip,
        ConfigKey::AiModel2Version,
        ConfigKey::AiModel2PackageUrl,
        ConfigKey::AiModel2Hash,
        ConfigKey::AiModel2Size,
    );

    // Ai Model 3
    add_target(
        ConfigKey::AiModel3Chip,
        ConfigKey::AiModel3Version,
        ConfigKey::AiModel3PackageUrl,
        ConfigKey::AiModel3Hash,
        ConfigKey::AiModel3Size,
    );

    let ai_model = AiModel {
        req_info: Some(req_id),
        targets: Some(targets),
        res_info: None,
    };

    let content = serde_json::to_string(&ai_model)
        .map_err(|e| Report::new(DMError::InvalidData).attach_printable(e))?;

    let mut root = Object::new();

    root.insert(
        "configuration/$system/PRIVATE_deploy_ai_model",
        json::JsonValue::String(content),
    );

    jdebug!(func = "parse_ai_model_configuration",
        root= ?root);

    Ok(json::stringify_pretty(root, 4))
}
