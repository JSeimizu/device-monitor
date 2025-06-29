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
        jdebug!(func = "Elog::parse", line = line!(), s = s);
        serde_json::from_str(s).map_err(|_| Report::new(DMError::InvalidData))
    }

    pub fn serial(&self) -> &str {
        &self.serial
    }

    pub fn level(&self) -> u8 {
        self.level
    }

    pub fn level_str(&self) -> &'static str {
        match self.level {
            0 => "CRITICAL",
            1 => "ERROR",
            2 => "WARN",
            3 => "INFO",
            4 => "DEBUG",
            5 => "TRACE",
            _ => "UNKNOWN",
        }
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

    pub fn event_str(&self) -> &'static str {
        let category = |x: u32, kind: u32| (x >> 8) << 8 == kind;
        match self.event_id {
            0x1010 => "metadata stopped (Sensor module)",
            0x1020 => "metadata started (Edge soft)",
            0x1030 => "Failed to receive data",
            0x1040 => "Token expired",
            0x2010 => "Failed to connect to console",
            0x2020 => "NTP failed",
            0x3010 => "Reset by watchdog",
            0x3020 => "Reboot by console request",
            0x4010 => "High temperature",
            0x4020 => "High temperature",
            0x4030 => "Storage high temperature",
            0x4040 => "Storage high temperature",
            0x4050 => "Storage low temperature",
            0x4060 => "Returned to normal temperature",
            0x4110 => "Low temperature",
            0x4120 => "Low temperature",
            0x5010 => "Input sensor stopped",
            0x5020 => "Edge software stopped",
            0x6001 => "Reset",
            0xb000 => "OTA started",
            0xb001 => "Reboot started",
            0xb002 => "Factory reset from console",
            0xb003 => "Factory reset from push-key",
            0xb004 => "DirectGetImage requested",
            0xb0b0 => "Failed to get sensor temperature",
            0xb0b1 => "DirectGetImage failed (sensor error)",
            0xb0b2 => "Download failed",
            0xb0b3 => "OTA failed (FwManager error)",
            0xd001 => "File open failed (sensor)",
            0xd002 => "Failed to communicate with AI device",
            0xd003 => "Failed to stop with AI device",
            _ if category(self.event_id, 0x8000) => "ESF button manager event",
            _ if category(self.event_id, 0x8100) => "ESF clock manager event",
            _ if category(self.event_id, 0x8200) => "ESF codec base64 event",
            _ if category(self.event_id, 0x8300) => "ESF codec jpeg event",
            _ if category(self.event_id, 0x8400) => "ESF codec json event",
            _ if category(self.event_id, 0x8500) => "ESF firmware manager event",
            _ if category(self.event_id, 0x8600) => "ESF led manager event",
            _ if category(self.event_id, 0x8700) => "ESF log manager event",
            _ if category(self.event_id, 0x8800) => "ESF main event",
            _ if category(self.event_id, 0x8900) => "ESF memory manager event",
            _ if category(self.event_id, 0x8a00) => "ESF network manager event",
            _ if category(self.event_id, 0x8b00) => "ESF parameter storage manager event",
            _ if category(self.event_id, 0x8c00) => "ESF power manager event",
            _ if category(self.event_id, 0x8d00) => "ESF system manager event",
            _ if category(self.event_id, 0x8e00) => "ESF security manager event",
            _ if category(self.event_id, 0x9000) => "ESF button manager porting layer event",
            _ if category(self.event_id, 0x9100) => "ESF cipher util porting layer event",
            _ if category(self.event_id, 0x9200) => "ESF firmware manager porting layer event",
            _ if category(self.event_id, 0x9300) => "ESF flash manager porting layer event",
            _ if category(self.event_id, 0x9400) => "ESF led manager porting layer event",
            _ if category(self.event_id, 0x9500) => "ESF memory manager porting layer event",
            _ if category(self.event_id, 0x9600) => "ESF network manager porting layer event",
            _ if category(self.event_id, 0x9700) => {
                "ESF parameter storage manager porting layer event"
            }
            _ if category(self.event_id, 0x9800) => "ESF power manager porting layer event",
            _ if category(self.event_id, 0x9900) => "ESF security util porting layer event",
            _ if category(self.event_id, 0x9a00) => "ESF hal driver event",
            _ if category(self.event_id, 0x9b00) => "ESF hal i2c event",
            _ if category(self.event_id, 0x9c00) => "ESF hal ioexp event",
            _ if category(self.event_id, 0xa000) => "ESF utility log event",
            _ if category(self.event_id, 0xa100) => "ESF utility message event",
            _ if category(self.event_id, 0xa200) => "ESF utility timer event",
            _ if category(self.event_id, 0xa300) => "ESF utility system error event",
            _ if category(self.event_id, 0xb000) => "SystemApp event",
            _ if category(self.event_id, 0xd000) => "Sensor event",
            _ if category(self.event_id, 0xf000) => "EVP event",
            _ => "Unknown event",
        }
    }

    pub fn event_description(&self) -> Option<&str> {
        self.event_description.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_elog(
        serial: &str,
        level: u8,
        timestamp: &str,
        component_id: u32,
        component_name: Option<&str>,
        event_id: u32,
        event_description: Option<&str>,
    ) -> Elog {
        Elog {
            serial: serial.to_string(),
            level,
            timestamp: timestamp.to_string(),
            component_id,
            component_name: component_name.map(|s| s.to_string()),
            event_id,
            event_description: event_description.map(|s| s.to_string()),
        }
    }

    #[test]
    fn test_parse_valid_json() {
        let json = r#"{
            "serial": "SN001",
            "level": 1,
            "timestamp": "2024-06-01T10:00:00Z",
            "component_id": 100,
            "component_name": "Main",
            "event_id": 4096,
            "event_description": "Critical error"
        }"#;
        let elog = Elog::parse(json).unwrap();
        assert_eq!(elog.serial(), "SN001");
        assert_eq!(elog.level(), 1);
        assert_eq!(elog.timestamp(), "2024-06-01T10:00:00Z");
        assert_eq!(elog.component_id(), 100);
        assert_eq!(elog.component_name(), Some("Main"));
        assert_eq!(elog.event_id(), 4096);
        assert_eq!(elog.event_description(), Some("Critical error"));
    }

    #[test]
    fn test_parse_invalid_json() {
        let json = r#"{"serial": 123}"#;
        assert!(Elog::parse(json).is_err());
    }

    #[test]
    fn test_level_str_variants() {
        let mut elog = make_elog("S", 0, "t", 0, None, 0, None);
        assert_eq!(elog.level_str(), "CRITICAL");
        elog.level = 1;
        assert_eq!(elog.level_str(), "ERROR");
        elog.level = 2;
        assert_eq!(elog.level_str(), "WARN");
        elog.level = 3;
        assert_eq!(elog.level_str(), "INFO");
        elog.level = 4;
        assert_eq!(elog.level_str(), "DEBUG");
        elog.level = 5;
        assert_eq!(elog.level_str(), "TRACE");
        elog.level = 100;
        assert_eq!(elog.level_str(), "UNKNOWN");
    }

    #[test]
    fn test_event_str_known_and_category() {
        let mut elog = make_elog("S", 0, "t", 0, None, 0x1010, None);
        assert_eq!(elog.event_str(), "metadata stopped (Sensor module)");
        elog.event_id = 0x8101;
        assert_eq!(elog.event_str(), "ESF clock manager event");
        elog.event_id = 0x8a10;
        assert_eq!(elog.event_str(), "ESF network manager event");
        elog.event_id = 0xf023;
        assert_eq!(elog.event_str(), "EVP event");
        elog.event_id = 0x0000;
        assert_eq!(elog.event_str(), "Unknown event");
    }

    #[test]
    fn test_component_name_and_event_description_none() {
        let elog = make_elog("S", 0, "t", 0, None, 0, None);
        assert_eq!(elog.component_name(), None);
        assert_eq!(elog.event_description(), None);
    }

    #[test]
    fn test_partial_eq() {
        let elog1 = make_elog("A", 1, "t", 2, Some("X"), 3, Some("desc"));
        let elog2 = make_elog("A", 1, "t", 2, Some("X"), 3, Some("desc"));
        assert_eq!(elog1, elog2);
    }

    #[test]
    fn test_serde_roundtrip() {
        let elog = make_elog(
            "B",
            2,
            "2024-06-01T12:34:56Z",
            42,
            Some("Comp"),
            0x1040,
            Some("desc"),
        );
        let json = serde_json::to_string(&elog).unwrap();
        let parsed = Elog::parse(&json).unwrap();
        assert_eq!(elog, parsed);
    }
}
