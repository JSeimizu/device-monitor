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
pub struct AgentSystemInfo {
    pub os: Option<String>,
    pub arch: Option<String>,
    pub evp_agent: Option<String>,
    pub evp_agent_commit_hash: Option<String>,
    pub wasmMicroRuntime: Option<String>,
    pub protocolVersion: Option<String>,
    pub deploymentStatus: Option<String>,
}

impl Default for AgentSystemInfo {
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
pub struct AgentDeviceConfig {
    pub report_status_interval_min: u32,
    pub report_status_interval_max: u32,
    pub registry_auth: String,
    pub configuration_id: String,
}

impl Default for AgentDeviceConfig {
    fn default() -> Self {
        Self {
            report_status_interval_min: 0,
            report_status_interval_max: 0,
            registry_auth: String::new(),
            configuration_id: String::new(),
        }
    }
}
