#[allow(unused)]
use {
    super::JsonUtility,
    super::evp_state::UUID,
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
    uuid::Uuid,
};

#[allow(non_snake_case)]
#[derive(Debug, PartialEq, Clone)]
pub struct ModuleInfo {
    pub id: UUID,
    pub blob_name: String,
    pub container_name: String,
    pub hash: String,
}
