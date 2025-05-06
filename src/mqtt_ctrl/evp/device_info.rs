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

        for (_i, c) in value.chips.iter().enumerate() {
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

#[derive(Debug, PartialEq, Default)]
pub struct DeviceReservedParsed<'a> {
    pub dtmi_version: u32,
    pub dtmi_path: &'a str,
    pub device: &'a str,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Default)]
pub struct DeviceReserved {
    schema: String,
}

impl DeviceReserved {
    pub fn schema(&self) -> &str {
        &self.schema
    }

    pub fn parse(&self) -> Result<DeviceReservedParsed, DMError> {
        if self.schema.is_empty() {
            return Err(Report::new(DMError::InvalidData));
        }

        let splitter1 = self
            .schema
            .as_str()
            .rfind(';')
            .ok_or(Report::new(DMError::InvalidData))?;

        let splitter2 = self
            .schema
            .as_str()
            .rfind(':')
            .ok_or(Report::new(DMError::InvalidData))?;

        let splitter3 = self
            .schema
            .as_str()
            .find(':')
            .ok_or(Report::new(DMError::InvalidData))?;

        Ok(DeviceReservedParsed {
            dtmi_version: self.schema.as_str()[(splitter1 + 1)..]
                .parse()
                .map_err(|_| Report::new(DMError::InvalidData))?,
            dtmi_path: &self.schema[(splitter3 + 1)..splitter1],
            device: &self.schema[(splitter2 + 1)..splitter1],
        })
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Default)]
pub struct ReqId {
    req_id: String,
}

impl Display for ReqId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "req_id={}", self.req_id)
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Default)]
pub struct ResInfo {
    res_id: String,
    code: i32,
    detail_msg: String,
}

impl Display for ResInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "req_id={}, code={}\n detail_msg={}",
            self.res_id, self.code, self.detail_msg
        )
    }
}

impl ResInfo {
    pub fn res_id(&self) -> &str {
        &self.res_id
    }

    pub fn code(&self) -> i32 {
        self.code
    }

    pub fn detail_msg(&self) -> &str {
        &self.detail_msg
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Default)]
pub struct LogSetting {
    filter: String,
    level: u32,
    destination: u32,
    storage_name: String,
    path: String,
}

impl LogSetting {
    pub fn filter(&self) -> &str {
        &self.filter
    }

    pub fn level(&self) -> u32 {
        self.level
    }

    pub fn destination(&self) -> &str {
        match self.destination {
            0 => "uart",
            1 => "cloud_storage",
            _ => "invalid",
        }
    }

    pub fn storage_name(&self) -> &str {
        &self.storage_name
    }

    pub fn path(&self) -> &str {
        &self.path
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Default)]
pub struct SystemSettings {
    req_info: ReqId,
    led_enabled: bool,
    temperature_update_interval: u32,
    log_settings: Vec<LogSetting>,
    res_info: ResInfo,
}

impl SystemSettings {
    pub fn req_info(&self) -> &ReqId {
        &self.req_info
    }
    pub fn led_enabled(&self) -> bool {
        self.led_enabled
    }
    pub fn temperature_update_interval(&self) -> u32 {
        self.temperature_update_interval
    }
    pub fn log_settings(&self) -> &Vec<LogSetting> {
        &self.log_settings
    }
    pub fn res_info(&self) -> &ResInfo {
        &self.res_info
    }
}

mod tests {
    #[test]
    fn test_reserved_parse_01() {
        use super::{DeviceReserved, SystemSettings};
        let schema = "dtmi:com:sony_semicon:aitrios:sss:edge:system:t3w;2".to_owned();
        let reserved = DeviceReserved {
            schema,
            ..Default::default()
        };

        let reserved_parsed = reserved.parse().unwrap();

        assert_eq!(reserved_parsed.dtmi_version, 2_u32);
        assert_eq!(
            reserved_parsed.dtmi_path,
            "com:sony_semicon:aitrios:sss:edge:system:t3w"
        );
        assert_eq!(reserved_parsed.device, "t3w");
    }
    #[test]
    fn test_system_settings_parse_01() {
        use super::*;
        use crate::mqtt_ctrl::evp::JsonUtility;

        let s = r#"
        "{\"req_info\":{\"req_id\":\"\"},\"led_enabled\":true,\"temperature_update_interval\":10,\"log_settings\":[{\"filter\":\"main\",\"level\":3,\"destination\":0,\"storage_name\":\"\",\"path\":\"\"},{\"filter\":\"sensor\",\"level\":3,\"destination\":0,\"storage_name\":\"\",\"path\":\"\"},{\"filter\":\"companion_fw\",\"level\":3,\"destination\":0,\"storage_name\":\"\",\"path\":\"\"},{\"filter\":\"companion_app\",\"level\":3,\"destination\":0,\"storage_name\":\"\",\"path\":\"\"}],\"res_info\":{\"res_id\":\"\",\"code\":0,\"detail_msg\":\"ok\"}}"
        "#;
        let json = json::parse(s).unwrap();
        let s = JsonUtility::json_value_to_string(&json);

        eprintln!("{s}");

        let system_settings: SystemSettings = serde_json::from_str(&s).unwrap();
        eprintln!("format!{:?}", system_settings)
    }
}
