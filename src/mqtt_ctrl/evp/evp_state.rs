#[allow(unused)]
use {
    super::JsonUtility,
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
#[derive(Debug, PartialEq)]
pub struct AgentSystemInfo {
    os: String,
    arch: String,
    evp_agent: String,
    evp_agent_commit_hash: Option<String>,
    wasmMicroRuntime: String,
    protocolVersion: String,
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
        }
    }
}

impl AgentSystemInfo {
    #[allow(non_snake_case)]
    pub fn parse(j: &str) -> Result<Self, DMError> {
        let v = json::parse(j).map_err(|_| Report::new(DMError::InvalidData))?;

        jdebug!(func = "AgentSystemInfo:parse()", line = line!());

        if let JsonValue::Object(o) = v {
            jdebug!(func = "AgentSystemInfo:parse()", line = line!());
            let mut os = String::new();
            let mut arch = String::new();
            let mut evp_agent = String::new();
            let mut evp_agent_commit_hash = None;
            let mut wasmMicroRuntime = String::new();
            let mut protocolVersion = String::new();

            for (k, v) in o.iter() {
                jdebug!(func = "AgentSystemInfo:parse()", line = line!(), key = k);
                match k {
                    "os" => {
                        os = v
                            .as_str()
                            .map(|s| s.to_owned())
                            .ok_or(Report::new(DMError::InvalidData))?
                    }
                    "arch" => {
                        arch = v
                            .as_str()
                            .map(|s| s.to_owned())
                            .ok_or(Report::new(DMError::InvalidData))?
                    }
                    "evp_agent" => {
                        evp_agent = v
                            .as_str()
                            .map(|s| s.to_owned())
                            .ok_or(Report::new(DMError::InvalidData))?
                    }
                    "evp_agent_commit_hash" => {
                        evp_agent_commit_hash = Some(
                            v.as_str()
                                .map(|s| s.to_owned())
                                .ok_or(Report::new(DMError::InvalidData))?,
                        )
                    }
                    "wasmMicroRuntime" => {
                        wasmMicroRuntime = v
                            .as_str()
                            .map(|s| s.to_owned())
                            .ok_or(Report::new(DMError::InvalidData))?
                    }
                    "protocolVersion" => {
                        protocolVersion = v
                            .as_str()
                            .map(|s| s.to_owned())
                            .ok_or(Report::new(DMError::InvalidData))?
                    }
                    _ => return Err(Report::new(DMError::InvalidData)),
                }
            }

            // Validate required fields are present and non-empty
            if os.is_empty()
                || arch.is_empty()
                || evp_agent.is_empty()
                || wasmMicroRuntime.is_empty()
                || protocolVersion.is_empty()
            {
                return Err(Report::new(DMError::InvalidData));
            }

            return Ok(AgentSystemInfo {
                os,
                arch,
                evp_agent,
                evp_agent_commit_hash,
                wasmMicroRuntime,
                protocolVersion,
            });
        }

        Err(Report::new(DMError::InvalidData))
    }

    pub fn os(&self) -> &str {
        &self.os
    }

    pub fn arch(&self) -> &str {
        &self.arch
    }

    pub fn evp_agent(&self) -> &str {
        &self.evp_agent
    }

    pub fn evp_agent_commit_hash(&self) -> Option<&str> {
        self.evp_agent_commit_hash.as_deref()
    }

    pub fn wasm_micro_runtime(&self) -> &str {
        &self.wasmMicroRuntime
    }

    pub fn protocol_version(&self) -> &str {
        &self.protocolVersion
    }
}

#[derive(Debug, PartialEq, Default)]
pub struct AgentDeviceConfig {
    pub report_status_interval_min: u32,
    pub report_status_interval_max: u32,
    pub registry_auth: String,
    pub configuration_id: String,
}

#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Eq, Hash, PartialEq, Clone)]
pub struct UUID {
    id: String,
}

impl UUID {
    pub fn from(uuid: &str) -> Result<UUID, DMError> {
        if UUID::is_valid(uuid) {
            Ok(Self {
                id: uuid.to_owned(),
            })
        } else {
            Err(Report::new(DMError::InvalidData))
        }
    }

    pub fn is_valid(uuid: &str) -> bool {
        uuid::Uuid::try_parse(uuid).is_ok()
    }

    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
        }
    }

    pub fn uuid(&self) -> &str {
        &self.id
    }
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Instance {
    status: String,
    moduleId: String,
    failureMessage: Option<String>,
}

impl Instance {
    pub fn status(&self) -> &str {
        &self.status
    }

    pub fn module_id(&self) -> &str {
        &self.moduleId
    }

    pub fn failure_message(&self) -> Option<&str> {
        self.failureMessage.as_deref()
    }
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Module {
    pub status: String,
    pub failureMessage: Option<String>,
}

impl Module {
    pub fn status(&self) -> &str {
        &self.status
    }

    pub fn failure_message(&self) -> Option<&str> {
        self.failureMessage.as_deref()
    }
}

#[allow(non_snake_case)]
#[derive(Debug, PartialEq, Default)]
pub struct DeploymentStatus {
    instances: HashMap<UUID, Instance>,
    modules: HashMap<UUID, Module>,
    deploymentId: Option<UUID>,
    reconcileStatus: Option<String>,
}

impl DeploymentStatus {
    #[allow(non_snake_case)]
    pub fn parse(j: &str) -> Result<Self, DMError> {
        let mut instances = HashMap::new();
        let mut modules = HashMap::new();
        let mut deploymentId = None;
        let mut reconcileStatus = None;

        jdebug!(func = "DeploymentStatus:parse()", line = line!());
        let v = json::parse(j).map_err(|_| Report::new(DMError::InvalidData))?;

        if let JsonValue::Object(o) = v {
            for (k, v) in o.iter() {
                match k {
                    "instances" => {
                        if let JsonValue::Object(o) = v {
                            for (k, v) in o.iter() {
                                let uuid = UUID::from(k)?;
                                let s = JsonUtility::json_value_to_string(v);
                                let instance: Instance = serde_json::from_str(&s)
                                    .map_err(|_| Report::new(DMError::InvalidData))?;
                                instances.insert(uuid, instance);
                            }
                        }
                    }
                    "modules" => {
                        if let JsonValue::Object(o) = v {
                            for (k, v) in o.iter() {
                                let uuid = UUID::from(k)?;
                                let s = JsonUtility::json_value_to_string(v);
                                let instance: Module = serde_json::from_str(&s)
                                    .map_err(|_| Report::new(DMError::InvalidData))?;
                                modules.insert(uuid, instance);
                            }
                        }
                    }
                    "deploymentId" => {
                        let s = JsonUtility::json_value_to_string(v);
                        let uuid = UUID::from(s.as_str())?;
                        deploymentId = Some(uuid);
                    }
                    "reconcileStatus" => {
                        let s = JsonUtility::json_value_to_string(v);
                        reconcileStatus = Some(s);
                    }
                    _ => return Err(Report::new(DMError::InvalidData)),
                }
            }

            return Ok(DeploymentStatus {
                instances,
                modules,
                deploymentId,
                reconcileStatus,
            });
        }

        Err(Report::new(DMError::InvalidData))
    }

    pub fn instances(&self) -> &HashMap<UUID, Instance> {
        &self.instances
    }

    pub fn modules(&self) -> &HashMap<UUID, Module> {
        &self.modules
    }

    pub fn deployment_id(&self) -> Option<&UUID> {
        self.deploymentId.as_ref()
    }

    pub fn reconcile_status(&self) -> Option<&str> {
        self.reconcileStatus.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use json::object;

    use super::*;

    #[test]
    fn test_uuid_01() {
        let id = "b218f90b-9228-423f-8e02-a6d3527bc15d";

        assert!(UUID::is_valid(id))
    }

    #[test]
    fn test_uuid_02() {
        let id = "b218f90b-9228-423f-8e02a6d3527bc15d";

        assert!(!UUID::is_valid(id))
    }

    #[test]
    fn test_uuid_03() {
        let id = UUID::new();
        eprintln!("{}", id.id);
        assert!(UUID::is_valid(&id.id));
    }

    #[test]
    fn test_instance_01() {
        let instance = json::object! {
            status: "ok",
            moduleId: "b218f90b-9228-423f-8e02-a6d3527bc15d",
        };

        //eprintln!("{}", instance.dump());

        let parsed: Instance = serde_json::from_str(&instance.dump()).unwrap();
        assert_eq!(parsed.status(), "ok");
        assert_eq!(parsed.module_id(), "b218f90b-9228-423f-8e02-a6d3527bc15d");
        assert!(parsed.failure_message().is_none());
    }

    #[test]
    fn test_instance_02() {
        let instance = json::object! {
            status: "ok",
            moduleId: "b218f90b-9228-423f-8e02-a6d3527bc15d",
            failureMessage: "crashed.",
        };

        // eprintln!("{}", instance.dump());

        let parsed: Instance = serde_json::from_str(&instance.dump()).unwrap();
        assert_eq!(parsed.status(), "ok");
        assert_eq!(parsed.module_id(), "b218f90b-9228-423f-8e02-a6d3527bc15d");
        assert_eq!(parsed.failure_message(), Some("crashed."));
    }

    #[test]
    fn test_module_01() {
        let module = json::object! {
            status: "ok",
        };

        //eprintln!("{}", instance.dump());

        let parsed: Module = serde_json::from_str(&module.dump()).unwrap();
        assert_eq!(parsed.status(), "ok");
        assert!(parsed.failure_message().is_none());
    }

    #[test]
    fn test_module_02() {
        let module = json::object! {
            status: "ok",
            failureMessage: "expired",
        };

        //eprintln!("{}", instance.dump());

        let parsed: Module = serde_json::from_str(&module.dump()).unwrap();
        assert_eq!(parsed.status(), "ok");
        assert_eq!(parsed.failure_message(), Some("expired"));
    }

    #[test]
    fn test_deployment_status_01() {
        let status = r#"
{
        "instances": {
            "b218f90b-9228-423f-8e02-000000000001": {
                "status": "ok",
                "moduleId": "b218f90b-9228-423f-8e02-a6d3527bc15d"
            },
            "c8fba53c-ffd9-439b-849d-000000000001": {
                "status": "ok",
                "moduleId": "c8fba53c-ffd9-439b-849d-d069e7017951"
            },
            "c8fba53c-ffd9-439b-849d-000000000002": {
                "status": "ok",
                "moduleId": "c8fba53c-ffd9-439b-849d-d069e7017951"
            },
            "f3a018c5-1997-489a-8f1d-000000000001": {
                "status": "error",
                "moduleId": "f3a018c5-1997-489a-8f1d-a758df12977a",
                "failureMessage": "Module is not ready"
            }
        },
        "modules": {
            "b218f90b-9228-423f-8e02-a6d3527bc15d": {
                "status": "ok"
            },
            "c8fba53c-ffd9-439b-849d-d069e7017951": {
                "status": "ok"
            },
            "f3a018c5-1997-489a-8f1d-a758df12977a": {
                "status": "error",
                "failureMessage": "Failed to load (error=11)"
            }
        },
        "deploymentId": "1C169145-8EB1-45AE-8267-35427323515E",
        "reconcileStatus": "ok"
} "#;

        let deployment_status = DeploymentStatus::parse(status).unwrap();

        // Validate counts
        assert_eq!(deployment_status.instances().len(), 4);
        assert_eq!(deployment_status.modules().len(), 3);

        // Validate reconcile status and deployment id
        assert_eq!(deployment_status.reconcile_status(), Some("ok"));
        assert_eq!(
            deployment_status
                .deployment_id()
                .map(|u| u.uuid().to_ascii_uppercase())
                .as_deref(),
            Some("1C169145-8EB1-45AE-8267-35427323515E")
        );

        // Validate one of the instances and modules
        let key = "f3a018c5-1997-489a-8f1d-000000000001";
        let uuid = UUID::from(key).unwrap();
        let inst = deployment_status
            .instances()
            .get(&uuid)
            .expect("instance exists");
        assert_eq!(inst.status(), "error");
        assert_eq!(inst.module_id(), "f3a018c5-1997-489a-8f1d-a758df12977a");
        assert_eq!(inst.failure_message(), Some("Module is not ready"));

        let mod_uuid = UUID::from("f3a018c5-1997-489a-8f1d-a758df12977a").unwrap();
        let module = deployment_status
            .modules()
            .get(&mod_uuid)
            .expect("module exists");
        assert_eq!(module.status(), "error");
        assert_eq!(module.failure_message(), Some("Failed to load (error=11)"));
    }

    #[test]
    fn test_deployment_status_parse_invalid_uuid() {
        // deployment JSON with invalid UUID should return error
        let status = r#"{"deploymentId":"not-a-uuid"}"#;
        assert!(DeploymentStatus::parse(status).is_err());
    }

    #[test]
    fn test_agent_system_info_01() {
        let system_info = object! {
            os: "Linux",
            arch:"aarch64",
            evp_agent:"v1.43.0",
            evp_agent_commit_hash: "03770507d0041f8952d3fff0a519376ce8e86c4e",
            wasmMicroRuntime:"v2.2.0",
            protocolVersion:"EVP2-TB",
        };

        let system_info = AgentSystemInfo::parse(&system_info.dump()).unwrap();
        assert_eq!(system_info.os(), "Linux");
        assert_eq!(system_info.arch(), "aarch64");
        assert_eq!(system_info.evp_agent(), "v1.43.0");
        assert_eq!(
            system_info.evp_agent_commit_hash(),
            Some("03770507d0041f8952d3fff0a519376ce8e86c4e")
        );
        assert_eq!(system_info.wasm_micro_runtime(), "v2.2.0");
        assert_eq!(system_info.protocol_version(), "EVP2-TB");
    }

    #[test]
    fn test_agent_system_info_missing_field_should_error() {
        // missing 'os' field
        let system_info = object! {
            arch:"aarch64",
            evp_agent:"v1.43.0",
            wasmMicroRuntime:"v2.2.0",
            protocolVersion:"EVP2-TB",
        };
        assert!(AgentSystemInfo::parse(&system_info.dump()).is_err());
    }
}
