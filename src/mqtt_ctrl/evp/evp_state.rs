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

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct AgentSystemInfo {
    pub os: String,
    pub arch: String,
    pub evp_agent: String,
    pub evp_agent_commit_hash: Option<String>,
    pub wasmMicroRuntime: String,
    pub protocolVersion: String,
    pub deploymentStatus: Option<String>,
}

impl Default for AgentSystemInfo {
    fn default() -> Self {
        let v = || Some("-".to_owned());
        Self {
            os: String::new(),
            arch: String::new(),
            evp_agent: String::new(),
            evp_agent_commit_hash: v(),
            wasmMicroRuntime: String::new(),
            protocolVersion: String::new(),
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
