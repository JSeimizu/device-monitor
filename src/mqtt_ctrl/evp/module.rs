use json::object;
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
    pub sas_url: String,
}

impl ModuleInfo {
    pub fn undeployment_json() -> Result<String, DMError> {
        let mut deployment = JsonValue::new_object();

        deployment
            .insert("deploymentId", UUID::new().uuid().to_string())
            .map_err(|_| {
                Report::new(DMError::InvalidData).attach_printable("Failed to insert deploymentId")
            })?;
        deployment
            .insert("instanceSpecs", JsonValue::new_object())
            .map_err(|_| {
                Report::new(DMError::InvalidData).attach_printable("Failed to insert instanceSpecs")
            })?;

        deployment
            .insert("modules", JsonValue::new_object())
            .map_err(|_| {
                Report::new(DMError::InvalidData).attach_printable("Failed to insert modules")
            })?;

        deployment
            .insert("publishTopics", JsonValue::new_object())
            .map_err(|_| {
                Report::new(DMError::InvalidData).attach_printable("Failed to insert publishTopics")
            })?;
        deployment
            .insert("subscribeTopics", JsonValue::new_object())
            .map_err(|_| {
                Report::new(DMError::InvalidData)
                    .attach_printable("Failed to insert subscribeTopics")
            })?;

        let mut root = JsonValue::new_object();
        root.insert("deployment", deployment).map_err(|_| {
            Report::new(DMError::InvalidData).attach_printable("Failed to insert deployment")
        })?;

        Ok(json::stringify_pretty(root, 4))
    }

    pub fn deployment_json(&self) -> Result<String, DMError> {
        let mut deployment = JsonValue::new_object();

        deployment
            .insert("deploymentId", UUID::new().uuid().to_string())
            .map_err(|_| {
                Report::new(DMError::InvalidData).attach_printable("Failed to insert deploymentId")
            })?;
        let instance_id = UUID::new().uuid().to_string();
        let instance = object! {
            "name" : self.blob_name.clone(),
            "moduleId" : self.id.uuid().to_string(),
            "publish" : {},
            "subscribe": {},
        };
        let mut instance_specs = JsonValue::new_object();
        instance_specs.insert(&instance_id, instance).map_err(|_| {
            Report::new(DMError::InvalidData).attach_printable("Failed to insert instanceId")
        })?;
        deployment
            .insert("instanceSpecs", instance_specs)
            .map_err(|_| {
                Report::new(DMError::InvalidData).attach_printable("Failed to insert instanceSpecs")
            })?;

        let mut module = JsonValue::new_object();
        let m = object! {
            "entryPoint" : "main",
            "moduleImpl" : "wasm",
            "downloadUrl" : self.sas_url.clone(),
            "hash" : self.hash.clone(),
        };
        module.insert(self.id.uuid(), m).map_err(|_| {
            Report::new(DMError::InvalidData).attach_printable("Failed to insert module")
        })?;
        deployment.insert("modules", module).map_err(|_| {
            Report::new(DMError::InvalidData).attach_printable("Failed to insert modules")
        })?;

        deployment
            .insert("publishTopics", JsonValue::new_object())
            .map_err(|_| {
                Report::new(DMError::InvalidData).attach_printable("Failed to insert publishTopics")
            })?;
        deployment
            .insert("subscribeTopics", JsonValue::new_object())
            .map_err(|_| {
                Report::new(DMError::InvalidData)
                    .attach_printable("Failed to insert subscribeTopics")
            })?;

        let mut root = JsonValue::new_object();
        root.insert("deployment", deployment).map_err(|_| {
            Report::new(DMError::InvalidData).attach_printable("Failed to insert deployment")
        })?;

        Ok(json::stringify_pretty(root, 4))
    }
}
