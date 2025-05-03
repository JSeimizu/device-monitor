#[allow(unused)]
use {
    crate::error::DMError,
    error_stack::{Report, Result},
    jlogger_tracing::{JloggerBuilder, LevelFilter, LogTimeFormat, jdebug, jerror, jinfo},
    regex::Regex,
    rumqttc::{Client, Connection, MqttOptions, QoS},
    serde_json::Deserializer,
    std::{
        collections::HashMap,
        time::{self, Duration, Instant},
    },
};

pub fn validate(payload: &str) -> bool {
    if let Ok(schema) = &serde_json::from_str(include_str!(
        "../../../evp-onwire-schema/schema/systeminfo.schema.json"
    )) {
        if let Ok(payload) = &serde_json::from_str(payload) {
            jsonschema::is_valid(schema, payload)
        } else {
            false
        }
    } else {
        false
    }
}
