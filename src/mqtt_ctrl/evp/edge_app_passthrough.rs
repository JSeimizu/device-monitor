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
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReqInfo {
    pub req_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResInfo {
    pub res_id: Option<String>,
    pub code: Option<i32>,
    pub detail_msg: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceSettings {
    pub number_of_iterations: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraImageSize {
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub scaling_policy: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameRate {
    pub num: Option<i32>,
    pub denom: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraImageFlip {
    pub flip_horizontal: Option<i32>,
    pub flip_vertical: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoExposure {
    pub max_exposure_time: Option<i32>,
    pub min_exposure_time: Option<i32>,
    pub max_gain: Option<f64>,
    pub convergence_speed: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoExposureMetering {
    pub metering_mode: Option<i32>,
    pub top: Option<i32>,
    pub left: Option<i32>,
    pub bottom: Option<i32>,
    pub right: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManualExposure {
    pub exposure_time: Option<i32>,
    pub gain: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoWhiteBalance {
    pub convergence_speed: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManualWhiteBalancePreset {
    pub color_temperature: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageCropping {
    pub left: Option<i32>,
    pub top: Option<i32>,
    pub width: Option<i32>,
    pub height: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterAccess {
    pub bit_length: Option<i32>,
    pub id: Option<i32>,
    pub address: Option<String>,
    pub data: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PqSettings {
    pub camera_image_size: Option<CameraImageSize>,
    pub frame_rate: Option<FrameRate>,
    pub digital_zoom: Option<f64>,
    pub camera_image_flip: Option<CameraImageFlip>,
    pub exposure_mode: Option<i32>,
    pub auto_exposure: Option<AutoExposure>,
    pub auto_exposure_metering: Option<AutoExposureMetering>,
    pub ev_compensation: Option<f64>,
    pub ae_anti_flicker_mode: Option<i32>,
    pub manual_exposure: Option<ManualExposure>,
    pub white_balance_mode: Option<i32>,
    pub auto_white_balance: Option<AutoWhiteBalance>,
    pub manual_white_balance_preset: Option<ManualWhiteBalancePreset>,
    pub image_cropping: Option<ImageCropping>,
    pub image_rotation: Option<i32>,
    pub register_access: Option<Vec<RegisterAccess>>,
    pub gamma_mode: Option<i32>,
    pub gamma_parameter: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortInterface {
    pub method: Option<i32>,
    pub storage_name: Option<String>,
    pub endpoint: Option<String>,
    pub path: Option<String>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortSettings {
    pub metadata: Option<PortInterface>,
    pub input_tensor: Option<PortInterface>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodecSettings {
    pub format: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommonSettings {
    pub process_state: Option<i32>,
    pub log_level: Option<i32>,
    pub inference_settings: Option<InferenceSettings>,
    pub pq_settings: Option<PqSettings>,
    pub port_settings: Option<PortSettings>,
    pub codec_settings: Option<CodecSettings>,
    pub number_of_inference_per_message: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomResInfo {
    pub res_id: Option<String>,
    pub code: Option<i32>,
    pub detail_msg: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiModelBundle {
    pub ai_model_bundle_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomSettings {
    pub res_info: Option<CustomResInfo>,
    pub ai_models: Option<HashMap<String, AiModelBundle>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeAppPassthrough {
    pub req_info: Option<ReqInfo>,
    pub res_info: Option<ResInfo>,
    pub common_settings: Option<CommonSettings>,
    pub custom_settings: Option<CustomSettings>,
}

impl Default for EdgeAppPassthrough {
    fn default() -> Self {
        Self {
            req_info: None,
            res_info: None,
            common_settings: None,
            custom_settings: None,
        }
    }
}

impl EdgeAppPassthrough {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn parse(
        topic: &str,
        payload: &str,
    ) -> error_stack::Result<(String, EdgeAppPassthrough), crate::error::DMError> {
        use crate::mqtt_ctrl::with_mqtt_ctrl;
        use error_stack::Report;
        use regex::Regex;

        let topic_regex = Regex::new(r"^state/([^/]+)/edge_app$").unwrap();

        let instance_id = topic_regex
            .captures(topic)
            .and_then(|caps| caps.get(1))
            .map(|m| m.as_str().to_string())
            .ok_or_else(|| Report::new(crate::error::DMError::InvalidData))?;

        let uuid = crate::mqtt_ctrl::evp::evp_state::UUID::from(&instance_id)
            .map_err(|_| Report::new(crate::error::DMError::InvalidData))?;

        let instance_exists = with_mqtt_ctrl(|mqtt_ctrl| {
            mqtt_ctrl
                .deployment_status()
                .map(|ds| ds.instances().contains_key(&uuid))
                .unwrap_or(false)
        });

        if !instance_exists {
            return Err(Report::new(crate::error::DMError::InvalidData));
        }

        let edge_app: EdgeAppPassthrough = serde_json::from_str(payload)
            .map_err(|_| Report::new(crate::error::DMError::InvalidData))?;

        Ok((instance_id, edge_app))
    }

    pub fn get_process_state_str(&self) -> &'static str {
        if let Some(common_settings) = &self.common_settings {
            if let Some(process_state) = common_settings.process_state {
                match process_state {
                    1 => "stopped",
                    2 => "running",
                    _ => "unknown",
                }
            } else {
                "none"
            }
        } else {
            "none"
        }
    }

    pub fn get_log_level_str(&self) -> &'static str {
        if let Some(common_settings) = &self.common_settings {
            if let Some(log_level) = common_settings.log_level {
                match log_level {
                    0 => "critical",
                    1 => "error",
                    2 => "warn",
                    3 => "info",
                    4 => "debug",
                    5 => "trace",
                    _ => "unknown",
                }
            } else {
                "none"
            }
        } else {
            "none"
        }
    }

    pub fn get_response_code_str(&self) -> &'static str {
        if let Some(res_info) = &self.res_info {
            if let Some(code) = res_info.code {
                match code {
                    0 => "ok",
                    1 => "cancelled",
                    2 => "unknown",
                    3 => "invalid_argument",
                    4 => "deadline_exceeded",
                    5 => "not_found",
                    6 => "already_exists",
                    7 => "permission_denied",
                    8 => "resource_exhausted",
                    9 => "failed_precondition",
                    10 => "aborted",
                    11 => "out_of_range",
                    12 => "unimplemented",
                    13 => "internal",
                    14 => "unavailable",
                    15 => "data_loss",
                    16 => "unauthenticated",
                    _ => "unknown",
                }
            } else {
                "none"
            }
        } else {
            "none"
        }
    }

    pub fn get_exposure_mode_str(&self) -> &'static str {
        if let Some(common_settings) = &self.common_settings {
            if let Some(pq_settings) = &common_settings.pq_settings {
                if let Some(exposure_mode) = pq_settings.exposure_mode {
                    match exposure_mode {
                        0 => "auto",
                        3 => "manual",
                        _ => "unknown",
                    }
                } else {
                    "none"
                }
            } else {
                "none"
            }
        } else {
            "none"
        }
    }

    pub fn get_white_balance_mode_str(&self) -> &'static str {
        if let Some(common_settings) = &self.common_settings {
            if let Some(pq_settings) = &common_settings.pq_settings {
                if let Some(wb_mode) = pq_settings.white_balance_mode {
                    match wb_mode {
                        0 => "auto",
                        1 => "manual_preset",
                        _ => "unknown",
                    }
                } else {
                    "none"
                }
            } else {
                "none"
            }
        } else {
            "none"
        }
    }

    pub fn get_scaling_policy_str(&self) -> &'static str {
        if let Some(common_settings) = &self.common_settings {
            if let Some(pq_settings) = &common_settings.pq_settings {
                if let Some(camera_image_size) = &pq_settings.camera_image_size {
                    if let Some(scaling_policy) = camera_image_size.scaling_policy {
                        match scaling_policy {
                            1 => "sensitivity",
                            2 => "resolution",
                            _ => "unknown",
                        }
                    } else {
                        "none"
                    }
                } else {
                    "none"
                }
            } else {
                "none"
            }
        } else {
            "none"
        }
    }

    pub fn get_codec_format_str(&self) -> &'static str {
        if let Some(common_settings) = &self.common_settings {
            if let Some(codec_settings) = &common_settings.codec_settings {
                if let Some(format) = codec_settings.format {
                    match format {
                        0 => "raw_data",
                        1 => "JPEG",
                        2 => "BMP",
                        _ => "unknown",
                    }
                } else {
                    "none"
                }
            } else {
                "none"
            }
        } else {
            "none"
        }
    }

    pub fn get_port_method_str(method: Option<i32>) -> &'static str {
        match method {
            Some(0) => "evp_telemetry",
            Some(1) => "blob_storage",
            Some(2) => "http_storage",
            _ => "none",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_invalid_topic() {
        let topic = "invalid/topic/format";
        let payload = "{}";

        let result = EdgeAppPassthrough::parse(topic, payload);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_wrong_topic_pattern() {
        let topic = "state/some-id/wrong_endpoint";
        let payload = "{}";

        let result = EdgeAppPassthrough::parse(topic, payload);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_invalid_uuid() {
        let topic = "state/invalid-uuid/edge_app";
        let payload = "{}";

        let result = EdgeAppPassthrough::parse(topic, payload);
        assert!(result.is_err());
    }
}
