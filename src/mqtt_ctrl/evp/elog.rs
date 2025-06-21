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

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Elog {
    serial: String,
    level: u8,
    timestamp: String,
    component_id: u32,
    component_name: Option<String>,
    event_id: u32,
    event_description: Option<String>,
}

#[allow(dead_code)]
impl Elog {
    pub fn parse(s: &str) -> Result<Self, DMError> {
        jdebug!(func="Elog::parse", line=line!(), s=s);
        serde_json::from_str(s).map_err(|_| Report::new(DMError::InvalidData))
    }

    pub fn serial(&self) -> &str {
        &self.serial
    }

    pub fn level(&self) -> u8 {
        self.level
    }

    pub fn timestamp(&self) -> &str {
        &self.timestamp
    }

    pub fn component_id(&self) -> u32 {
        self.component_id
    }
    pub fn component_name(&self) -> Option<&str> {
        self.component_name.as_deref()
    }

    pub fn event_id(&self) -> u32 {
        self.event_id
    }

    pub fn event_description(&self) -> Option<&str> {
        self.event_description.as_deref()
    }
}
