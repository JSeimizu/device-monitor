#[allow(unused)]
use {
    crate::error::DMError,
    error_stack::{Report, Result},
    json::JsonValue,
    serde::{Deserialize, Serialize},
    std::collections::{BTreeMap, HashMap},
    std::fmt::Display,
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

impl Default for DeviceInfo {
    fn default() -> Self {
        let device_manifest = Some("-".to_owned());
        let mut chips = vec![];
        chips.push(ChipInfo {
            name: Some("main_chip".to_owned()),
            ..Default::default()
        });
        chips.push(ChipInfo {
            name: Some("companion_chip".to_owned()),
            ..Default::default()
        });
        chips.push(ChipInfo {
            name: Some("sensor_chip".to_owned()),
            ..Default::default()
        });
        Self {
            device_manifest,
            chips,
        }
    }
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

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct PowerSource {
    #[serde(alias = "type")]
    _type: i8,
    level: u8,
}

impl Default for PowerSource {
    fn default() -> Self {
        Self {
            _type: -1,
            level: 0,
        }
    }
}

impl Display for PowerSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match self._type {
            0 => format!("{}@poe", self.level),
            1 => format!("{}@usb", self.level),
            2 => format!("{}@dc_plug", self.level),
            3 => format!("{}@primary_battery", self.level),
            4 => format!("{}@secondary_battery", self.level),
            -1 => format!("{}@unknown", self.level),
            _ => panic!("Invalid power source"),
        };
        write!(f, "{}", msg)
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct PowerStates {
    source: Vec<PowerSource>,
    in_use: i8,
    is_battery_low: bool,
}

impl Default for PowerStates {
    fn default() -> Self {
        Self {
            source: vec![PowerSource::default()],
            in_use: -1,
            is_battery_low: false,
        }
    }
}

impl PowerStates {
    pub fn power_sources(&self) -> String {
        let mut power_sources = String::new();

        for p in self.source.iter() {
            if power_sources.is_empty() {
                power_sources.push_str(&format!("{}", p.to_string()));
            } else {
                power_sources.push_str(&format!(",{}", p.to_string()));
            }
        }

        power_sources
    }

    pub fn power_sources_in_use(&self) -> String {
        match self.in_use {
            -1 => "Unknown".to_owned(),
            0 => "PoE".to_owned(),
            1 => "USB".to_owned(),
            2 => "DC Plug".to_owned(),
            3 => "Primary Battery".to_owned(),
            4 => "Secondary Battery".to_owned(),
            _ => panic!("Invalid power source in use."),
        }
    }

    pub fn is_battery_low(&self) -> bool {
        self.is_battery_low
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct DeviceStates {
    power_states: PowerStates,
    process_state: String,
    hours_meter: i8,
    bootup_reason: i8,
    last_bootup_time: String,
}

impl Default for DeviceStates {
    fn default() -> Self {
        Self {
            power_states: PowerStates::default(),
            process_state: "Idle".to_owned(),
            hours_meter: -1,
            bootup_reason: -1,
            last_bootup_time: String::default(),
        }
    }
}

impl DeviceStates {
    pub fn power_state(&self) -> &PowerStates {
        &self.power_states
    }

    pub fn process_state(&self) -> &str {
        &self.process_state
    }

    pub fn hours_meter(&self) -> i8 {
        self.hours_meter
    }

    pub fn bootup_reason(&self) -> String {
        match self.bootup_reason {
            -1 => "Unknown".to_owned(),
            0 => "Power supply".to_owned(),
            1 => "Hardware reset".to_owned(),
            2 => "Software reset".to_owned(),
            3 => "Software update".to_owned(),
            4 => "User request (cloud)".to_owned(),
            _ => panic!("Invalid bootup reason"),
        }
    }

    pub fn last_bootup_time(&self) -> &str {
        &self.last_bootup_time
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct DeviceCapabilities {
    is_battery_supported: bool,
    supported_wireless_mode: i8,
    is_periodic_supported: bool,
    is_sensor_postprocess_supported: bool,
}

impl Default for DeviceCapabilities {
    fn default() -> Self {
        Self {
            is_battery_supported: false,
            supported_wireless_mode: -1,
            is_periodic_supported: false,
            is_sensor_postprocess_supported: false,
        }
    }
}

impl DeviceCapabilities {
    pub fn is_battery_supported(&self) -> bool {
        self.is_battery_supported
    }

    pub fn supported_wireless_mode(&self) -> String {
        match self.supported_wireless_mode {
            -1 => "Unknown".to_owned(),
            0 => "None".to_owned(),
            1 => "Station mode".to_owned(),
            2 => "AP mode".to_owned(),
            3 => "Station and AP mode".to_owned(),
            _ => panic!("Invalid wireless mode"),
        }
    }

    pub fn is_periodic_supported(&self) -> bool {
        self.is_periodic_supported
    }

    pub fn is_sensor_postprocess_supported(&self) -> bool {
        self.is_sensor_postprocess_supported
    }
}
