#[allow(unused)]
use {
    crate::error::DMError,
    error_stack::{Report, Result},
    json::JsonValue,
    serde::{Deserialize, Serialize},
    std::collections::{BTreeMap, HashMap},
};

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct AiModel {
    pub version: Option<String>,
    pub hash: Option<String>,
    pub update_date: Option<String>,
}

impl Default for AiModel {
    fn default() -> Self {
        let v = || Some("-".to_owned());
        Self {
            version: v(),
            hash: v(),
            update_date: v(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct ChipInfo {
    pub name: Option<String>,
    pub id: Option<String>,
    pub hardware_version: Option<String>,
    pub temperature: i32,
    pub loader_version: Option<String>,
    pub loader_hash: Option<String>,
    pub update_date_loader: Option<String>,
    pub firmware_version: Option<String>,
    pub firmware_hash: Option<String>,
    pub update_date_firmware: Option<String>,
    pub ai_models: Vec<AiModel>,
}

impl Default for ChipInfo {
    fn default() -> Self {
        let v = || Some("-".to_owned());
        Self {
            name: None,
            id: v(),
            hardware_version: v(),
            temperature: -273,
            loader_version: v(),
            loader_hash: v(),
            update_date_loader: v(),
            firmware_version: v(),
            firmware_hash: v(),
            update_date_firmware: v(),
            ai_models: vec![],
        }
    }
}

impl ChipInfo {
    pub fn ai_models_pairs(&self) -> Vec<(String, String)> {
        let mut result = vec![];
        let fix = |v: Option<&str>| {
            v.map(|a| {
                if a == "" {
                    "-".to_owned()
                } else {
                    a.to_owned()
                }
            })
            .unwrap_or("-".to_owned())
        };

        for (i, v) in self.ai_models.iter().enumerate() {
            let key = format!("ai_models[{i}]");
            let value = format!(
                "version: {} update_date: {} hash:{}",
                fix(v.version.as_deref()),
                fix(v.update_date.as_deref()),
                fix(v.hash.as_deref())
            );
            result.push((key, value));
        }

        result
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct DeviceInfo {
    device_manifest: Option<String>,
    chips: Vec<ChipInfo>,
}

impl From<&DeviceInfo> for HashMap<String, String> {
    fn from(value: &DeviceInfo) -> Self {
        let mut hash = HashMap::new();
        let fix = |v: Option<&str>| {
            v.map(|a| {
                if a == "" {
                    "-".to_owned()
                } else {
                    a.to_owned()
                }
            })
            .unwrap_or("-".to_owned())
        };

        hash.insert(
            "device_manifest".to_owned(),
            fix(value.device_manifest.as_deref()),
        );

        for (i, c) in value.chips.iter().enumerate() {
            let name = fix(c.name.as_deref());
            //hash.insert(format!("chip[{}].name", name), fix(c.name.as_deref()));
            hash.insert(format!("chip[{}].id", name), fix(c.id.as_deref()));
            hash.insert(
                format!("chip[{}].hardware_version", name),
                fix(c.hardware_version.as_deref()),
            );
            hash.insert(
                format!("chip[{}].temperature", name),
                c.temperature.to_string(),
            );
            hash.insert(
                format!("chip[{}].loader_version", name),
                fix(c.loader_version.as_deref()),
            );
            hash.insert(
                format!("chip[{}].loader_hash", name),
                fix(c.loader_hash.as_deref()),
            );

            hash.insert(
                format!("chip[{}].update_date_loader", name),
                fix(c.update_date_loader.as_deref()),
            );
            hash.insert(
                format!("chip[{}].update_date_firmware", name),
                fix(c.update_date_firmware.as_deref()),
            );
            hash.insert(
                format!("chip[{}].firmware_version", name),
                fix(c.firmware_version.as_deref()),
            );
            hash.insert(
                format!("chip[{}].firmware_hash", name),
                fix(c.firmware_hash.as_deref()),
            );

            for (j, d) in c.ai_models.iter().enumerate() {
                let ai_model_info = format!(
                    "version: {} update_date: {} hash:{}",
                    fix(d.version.as_deref()),
                    fix(d.update_date.as_deref()),
                    fix(d.hash.as_deref())
                );

                hash.insert(
                    format!("chip[{}].ai_models[{}].version", name, j),
                    ai_model_info,
                );
            }
        }

        hash
    }
}

impl DeviceInfo {
    pub fn parse(s: &str) -> Result<Self, DMError> {
        serde_json::from_str(s).map_err(|_| Report::new(DMError::InvalidData))
    }

    pub fn get_map(&self) -> Result<(Vec<String>, HashMap<String, String>), DMError> {
        let hash = HashMap::from(self);
        let mut keys: Vec<String> = hash.keys().into_iter().map(|a| a.to_owned()).collect();
        keys.sort();

        Ok((keys, hash))
    }

    pub fn device_manifest(&self) -> Option<&str> {
        self.device_manifest.as_deref()
    }

    pub fn main_chip(&self) -> Option<&ChipInfo> {
        for c in self.chips.iter() {
            if c.name == Some("main_chip".to_owned()) {
                return Some(c);
            }
        }

        None
    }

    pub fn companion_chip(&self) -> Option<&ChipInfo> {
        for c in self.chips.iter() {
            if c.name == Some("companion_chip".to_owned()) {
                return Some(c);
            }
        }

        None
    }

    pub fn sensor_chip(&self) -> Option<&ChipInfo> {
        for c in self.chips.iter() {
            if c.name == Some("sensor_chip".to_owned()) {
                return Some(c);
            }
        }

        None
    }
}
