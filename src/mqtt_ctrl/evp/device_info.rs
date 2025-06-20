#[allow(unused)]
use {
    super::ReqId,
    super::ResInfo,
    crate::error::DMError,
    error_stack::{Report, Result},
    json::JsonValue,
    serde::{Deserialize, Serialize},
    std::collections::{BTreeMap, HashMap},
    std::fmt::Display,
};

#[derive(Serialize, Deserialize, Debug, PartialEq, Default)]
pub struct AiModel {
    pub version: String,
    pub hash: String,
    pub update_date: String,
}

impl AiModel {
    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn hash(&self) -> &str {
        &self.hash
    }

    pub fn update_date(&self) -> &str {
        &self.update_date
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct ChipInfo {
    name: String,
    id: String,
    hardware_version: Option<String>,
    temperature: i32,
    loader_version: Option<String>,
    loader_hash: Option<String>,
    update_date_loader: Option<String>,
    firmware_version: Option<String>,
    firmware_hash: Option<String>,
    update_date_firmware: Option<String>,
    ai_models: Vec<AiModel>,
}

impl Default for ChipInfo {
    fn default() -> Self {
        let v = || Some("-".to_owned());
        Self {
            name: String::default(),
            id: String::default(),
            hardware_version: v(),
            temperature: -300,
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
    pub fn check_chip_name(name: &str) -> bool {
        matches!(name, "main_chip" | "companion_chip" | "sensor_chip")
    }

    pub fn new(name: &str) -> Result<Self, DMError> {
        if ChipInfo::check_chip_name(name) {
            Ok(Self {
                name: name.to_owned(),
                ..Default::default()
            })
        } else {
            Err(Report::new(DMError::InvalidData))
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn hardware_version(&self) -> Option<&str> {
        self.hardware_version.as_deref()
    }

    pub fn temperature(&self) -> i32 {
        self.temperature
    }

    pub fn loader_version(&self) -> Option<&str> {
        self.loader_version.as_deref()
    }

    pub fn loader_hash(&self) -> Option<&str> {
        self.loader_hash.as_deref()
    }

    pub fn update_date_loader(&self) -> Option<&str> {
        self.update_date_loader.as_deref()
    }

    pub fn firmware_version(&self) -> Option<&str> {
        self.firmware_version.as_deref()
    }

    pub fn firmware_hash(&self) -> Option<&str> {
        self.firmware_hash.as_deref()
    }

    pub fn update_date_firmware(&self) -> Option<&str> {
        self.update_date_firmware.as_deref()
    }

    pub fn ai_models(&self) -> &Vec<AiModel> {
        &self.ai_models
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct DeviceInfo {
    device_manifest: Option<String>,
    chips: Vec<ChipInfo>,
}

impl Default for DeviceInfo {
    fn default() -> Self {
        let chips = vec![];
        Self {
            device_manifest: None,
            chips,
        }
    }
}

impl DeviceInfo {
    pub fn parse(s: &str) -> Result<Self, DMError> {
        serde_json::from_str(s).map_err(|_| Report::new(DMError::InvalidData))
    }

    pub fn device_manifest(&self) -> Option<&str> {
        self.device_manifest.as_deref()
    }

    pub fn main_chip(&self) -> Option<&ChipInfo> {
        self.chips.iter().find(|&c| c.name == *"main_chip")
    }

    pub fn companion_chip(&self) -> Option<&ChipInfo> {
        self.chips.iter().find(|&c| c.name == *"companion_chip")
    }

    pub fn sensor_chip(&self) -> Option<&ChipInfo> {
        self.chips.iter().find(|&c| c.name == *"sensor_chip")
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct PowerSource {
    #[serde(alias = "type")]
    _type: i8,
    level: i8,
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
                power_sources.push_str(&format!("{p}"));
            } else {
                power_sources.push_str(&format!(",{p}"));
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
            process_state: String::default(),
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
    is_battery_supported: Option<bool>,
    supported_wireless_mode: Option<i8>,
    is_periodic_supported: Option<bool>,
    is_sensor_postprocess_supported: Option<bool>,
}

impl Default for DeviceCapabilities {
    fn default() -> Self {
        Self {
            is_battery_supported: Some(false),
            supported_wireless_mode: Some(-1),
            is_periodic_supported: Some(false),
            is_sensor_postprocess_supported: Some(false),
        }
    }
}

impl DeviceCapabilities {
    pub fn is_battery_supported(&self) -> Option<bool> {
        self.is_battery_supported
    }

    pub fn supported_wireless_mode(&self) -> Option<String> {
        self.supported_wireless_mode.map(|mode| match mode {
            -1 => "Unknown".to_owned(),
            0 => "None".to_owned(),
            1 => "Station mode".to_owned(),
            2 => "AP mode".to_owned(),
            3 => "Station and AP mode".to_owned(),
            _ => panic!("Invalid wireless mode"),
        })
    }

    pub fn is_periodic_supported(&self) -> Option<bool> {
        self.is_periodic_supported
    }

    pub fn is_sensor_postprocess_supported(&self) -> Option<bool> {
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

    pub fn level_str(&self) -> &'static str {
        match self.level {
            0 => "Critical",
            1 => "Error",
            2 => "Warn",
            3 => "Info",
            4 => "Debug",
            5 => "Trace",
            _ => "Invalid",
        }
    }

    pub fn destination_str(&self) -> &str {
        match self.destination {
            0 => "uart",
            1 => "cloud_storage",
            _ => "invalid",
        }
    }

    pub fn destination(&self) -> u32 {
        self.destination
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
    led_enabled: Option<bool>,
    temperature_update_interval: Option<u32>,
    log_settings: Option<Vec<LogSetting>>,
    res_info: ResInfo,
}

impl SystemSettings {
    pub fn req_info(&self) -> &ReqId {
        &self.req_info
    }
    pub fn led_enabled(&self) -> Option<bool> {
        self.led_enabled
    }
    pub fn temperature_update_interval(&self) -> Option<u32> {
        self.temperature_update_interval
    }
    pub fn log_settings(&self) -> Option<&Vec<LogSetting>> {
        self.log_settings.as_ref()
    }
    pub fn res_info(&self) -> &ResInfo {
        &self.res_info
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Default)]
pub struct ProxySettings {
    proxy_url: String,
    proxy_port: u32,
    proxy_user_name: Option<String>,
    proxy_password: Option<String>,
}

impl ProxySettings {
    pub fn url(&self) -> &str {
        &self.proxy_url
    }

    pub fn port(&self) -> u32 {
        self.proxy_port
    }

    pub fn user_name(&self) -> Option<&str> {
        self.proxy_user_name.as_deref()
    }

    pub fn password(&self) -> Option<&str> {
        self.proxy_password.as_deref()
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Default)]
pub struct IpSetting {
    ip_address: Option<String>,
    subnet_mask: Option<String>,
    gateway_address: Option<String>,
    dns_address: Option<String>,
}

impl IpSetting {
    pub fn ip_address(&self) -> &str {
        self.ip_address.as_deref().unwrap_or_default()
    }

    pub fn subnet_mask(&self) -> &str {
        self.subnet_mask.as_deref().unwrap_or_default()
    }

    pub fn gateway(&self) -> &str {
        self.gateway_address.as_deref().unwrap_or_default()
    }

    pub fn dns(&self) -> &str {
        self.dns_address.as_deref().unwrap_or_default()
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Default)]
pub struct NetworkSettings {
    req_info: ReqId,
    ip_method: Option<u8>,
    ntp_url: Option<String>,
    static_settings_ipv6: Option<IpSetting>,
    static_settings_ipv4: Option<IpSetting>,
    proxy_settings: Option<ProxySettings>,
    res_info: ResInfo,
}

impl NetworkSettings {
    pub fn req_info(&self) -> &ReqId {
        &self.req_info
    }

    pub fn ip_method(&self) -> &'static str {
        let ip_method = self.ip_method.unwrap_or(u8::MAX);
        match ip_method {
            0 => "dhcp",
            1 => "static",
            _ => "unknown",
        }
    }

    pub fn ipv4(&self) -> Option<&IpSetting> {
        self.static_settings_ipv4.as_ref()
    }

    pub fn ipv6(&self) -> Option<&IpSetting> {
        self.static_settings_ipv6.as_ref()
    }

    pub fn ntp_url(&self) -> &str {
        self.ntp_url.as_deref().unwrap_or_default()
    }

    pub fn proxy(&self) -> Option<&ProxySettings> {
        self.proxy_settings.as_ref()
    }

    pub fn res_info(&self) -> &ResInfo {
        &self.res_info
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct StationModeSetting {
    ssid: String,
    password: String,
    encryption: u8,
}

impl Default for StationModeSetting {
    fn default() -> Self {
        Self {
            ssid: String::default(),
            password: String::default(),
            encryption: u8::MAX,
        }
    }
}

impl StationModeSetting {
    pub fn ssid(&self) -> &str {
        &self.ssid
    }

    pub fn password(&self) -> &str {
        &self.password
    }

    pub fn encryption(&self) -> &'static str {
        match self.encryption {
            0 => "wpa2_psk",
            1 => "wpa3_psk",
            2 => "wpa2_wpa3_psk",
            _ => "unknown",
        }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Default)]
pub struct WirelessSettings {
    req_info: ReqId,
    sta_mode_setting: Option<StationModeSetting>,
    res_info: ResInfo,
}

impl WirelessSettings {
    pub fn req_info(&self) -> &ReqId {
        &self.req_info
    }

    pub fn sta_mode_setting(&self) -> Option<&StationModeSetting> {
        self.sta_mode_setting.as_ref()
    }

    pub fn res_info(&self) -> &ResInfo {
        &self.res_info
    }
}

mod tests {
    #[test]
    fn test_reserved_parse_01() {
        use super::DeviceReserved;
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
