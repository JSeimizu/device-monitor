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
            0xb0b0 => "Failed to get temperature",
            0xb0b1 => "DirectGetImage failed (sensor error)",
            0xb0b2 => "Download failed",
            0xb0b3 => "OTA failed (FwManager error)",
            0xd001 => "File open failed (sensor)",
            0xd002 => "Failed to communicate with AI device",
            0xd003 => "Failed to stop with AI device",
            _ if self.event_id & 0x8000 != 0 => "ESF button manager event",
            _ if self.event_id & 0x8100 != 0 => "ESF clock manager event",
            _ if self.event_id & 0x8200 != 0 => "ESF codec base64 event",
            _ if self.event_id & 0x8300 != 0 => "ESF codec jpeg event",
            _ if self.event_id & 0x8400 != 0 => "ESF codec json event",
            _ if self.event_id & 0x8500 != 0 => "ESF firmware manager event",
            _ if self.event_id & 0x8600 != 0 => "ESF led manager event",
            _ if self.event_id & 0x8700 != 0 => "ESF log manager event",
            _ if self.event_id & 0x8800 != 0 => "ESF main event",
            _ if self.event_id & 0x8900 != 0 => "ESF memory manager event",
            _ if self.event_id & 0x8a00 != 0 => "ESF network manager event",
            _ if self.event_id & 0x8b00 != 0 => "ESF parameter storage manager event",
            _ if self.event_id & 0x8c00 != 0 => "ESF power manager event",
            _ if self.event_id & 0x8d00 != 0 => "ESF system manager event",
            _ if self.event_id & 0x8e00 != 0 => "ESF security manager event",
            _ if self.event_id & 0x9000 != 0 => "ESF button manager porting layer event",
            _ if self.event_id & 0x9100 != 0 => "ESF cipher util porting layer event",
            _ if self.event_id & 0x9200 != 0 => "ESF firmware manager porting layer event",
            _ if self.event_id & 0x9300 != 0 => "ESF flash manager porting layer event",
            _ if self.event_id & 0x9400 != 0 => "ESF led manager porting layer event",
            _ if self.event_id & 0x9500 != 0 => "ESF memory manager porting layer event",
            _ if self.event_id & 0x9600 != 0 => "ESF network manager porting layer event",
            _ if self.event_id & 0x9700 != 0 => "ESF parameter storage manager porting layer event",
            _ if self.event_id & 0x9800 != 0 => "ESF power manager porting layer event",
            _ if self.event_id & 0x9900 != 0 => "ESF security util porting layer event",
            _ if self.event_id & 0x9a00 != 0 => "ESF hal driver event",
            _ if self.event_id & 0x9b00 != 0 => "ESF hal i2c event",
            _ if self.event_id & 0x9c00 != 0 => "ESF hal ioexp event",
            _ if self.event_id & 0xa000 != 0 => "ESF utility log event",
            _ if self.event_id & 0xa100 != 0 => "ESF utility message event",
            _ if self.event_id & 0xa200 != 0 => "ESF utility timer event",
            _ if self.event_id & 0xa300 != 0 => "ESF utility system error event",
            _ if self.event_id & 0xb000 != 0 => "SystemApp event",
            _ if self.event_id & 0xd000 != 0 => "Sensor event",
            _ if self.event_id & 0xf000 != 0 => "EVP event",
            _ => "Unknown event",
        }
    }

    pub fn event_description(&self) -> Option<&str> {
        self.event_description.as_deref()
    }
}
