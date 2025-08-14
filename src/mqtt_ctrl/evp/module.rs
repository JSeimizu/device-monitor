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
    pub size: usize,
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

#[cfg(test)]
mod tests {
    use super::*;
    use json::JsonValue;

    #[test]
    fn test_undeployment_json_contains_expected_keys() {
        let s = ModuleInfo::undeployment_json().expect("undeployment json");
        let v = json::parse(&s).expect("parse");
        assert!(v["deployment"].is_object());
        let d = &v["deployment"];
        assert!(d.has_key("deploymentId"));
        assert!(d.has_key("instanceSpecs"));
        assert!(d.has_key("modules"));
        assert!(d.has_key("publishTopics"));
        assert!(d.has_key("subscribeTopics"));
    }

    #[test]
    fn test_deployment_json_contains_expected_values() {
        let mi = ModuleInfo {
            id: UUID::new(),
            blob_name: "test_blob".to_string(),
            container_name: "test_container".to_string(),
            hash: "abcd1234".to_string(),
            sas_url: "https://example.com/blob?sas".to_string(),
            size: 1024,
        };
        let s = mi.deployment_json().expect("deployment json");
        let v = json::parse(&s).expect("parse");
        let d = &v["deployment"];

        // modules object contains module id key
        assert!(d["modules"].is_object());
        let module_key = mi.id.uuid();
        assert!(d["modules"].has_key(module_key));
        let module_entry = &d["modules"][module_key];
        assert_eq!(module_entry["hash"].as_str().unwrap(), mi.hash);
        assert_eq!(module_entry["downloadUrl"].as_str().unwrap(), mi.sas_url);

        // instanceSpecs should have one entry whose name matches blob_name and moduleId matches id
        assert!(d["instanceSpecs"].is_object());
        let instance_specs = &d["instanceSpecs"];
        assert_eq!(instance_specs.len(), 1);

        let mut found = false;
        for (_k, val) in instance_specs.entries() {
            assert_eq!(val["name"].as_str().unwrap(), mi.blob_name);
            assert_eq!(val["moduleId"].as_str().unwrap(), mi.id.uuid());
            found = true;
        }
        assert!(found);
    }
}
