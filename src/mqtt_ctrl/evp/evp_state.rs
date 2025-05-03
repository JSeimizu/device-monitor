#[allow(unused)]
use {
    crate::error::DMError,
    error_stack::{Report, Result},
    jlogger_tracing::{JloggerBuilder, LevelFilter, LogTimeFormat, jdebug, jerror, jinfo},
    json::JsonValue,
    regex::Regex,
    rumqttc::{Client, Connection, MqttOptions, QoS},
    serde::{Deserialize, Serialize},
    serde_json::Deserializer,
    std::{
        collections::HashMap,
        time::{self, Duration, Instant},
    },
};

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct SystemInfo {
    pub os: Option<String>,
    pub arch: Option<String>,
    pub evp_agent: Option<String>,
    pub evp_agent_commit_hash: Option<String>,
    pub wasmMicroRuntime: Option<String>,
    pub protocolVersion: Option<String>,
    pub deploymentStatus: Option<String>,
}

impl Default for SystemInfo {
    fn default() -> Self {
        let v = || Some("-".to_owned());
        Self {
            os: v(),
            arch: v(),
            evp_agent: v(),
            evp_agent_commit_hash: v(),
            wasmMicroRuntime: v(),
            protocolVersion: v(),
            deploymentStatus: v(),
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct AgentState {
    pub report_status_interval_min: u32,
    pub report_status_interval_max: u32,
    pub system_info: SystemInfo,
}

impl Default for AgentState {
    fn default() -> Self {
        Self {
            report_status_interval_min: 0,
            report_status_interval_max: 0,
            system_info: SystemInfo::default(),
        }
    }
}

impl AgentState {
    pub fn parse(s: &str) -> Result<Self, DMError> {
        let mut report_status_interval_min = 0_u32;
        let mut report_status_interval_max = 0_u32;

        let v = json::parse(s).map_err(|e| Report::new(DMError::InvalidData))?;

        if let JsonValue::Object(o) = v {
            for (k, v) in o.iter() {
                if k == "state/$agent/report-status-interval-min" {
                    if let JsonValue::Number(n) = v {
                        if let Some(v) = n.as_fixed_point_u64(0) {
                            report_status_interval_min = v as u32;
                        }
                    }
                }

                if k == "state/$agent/report-status-interval-max" {
                    if let JsonValue::Number(n) = v {
                        if let Some(v) = n.as_fixed_point_u64(0) {
                            report_status_interval_max = v as u32;
                        }
                    }
                }

                if k == "systemInfo" {
                    let s = json::stringify(v.clone());
                    let system_info: SystemInfo =
                        serde_json::from_str(&s).map_err(|_| Report::new(DMError::InvalidData))?;
                    return Ok(AgentState {
                        report_status_interval_min,
                        report_status_interval_max,
                        system_info,
                    });
                }
            }
        }

        Err(Report::new(DMError::InvalidData))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_state_parse_01() {}
}
