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

use crate::app::MainWindowFocus;
#[allow(unused)]
use {
    super::{
        device_info::{
            DeviceCapabilities, DeviceInfo, DeviceReserved, DeviceStates, NetworkSettings,
            SystemSettings, WirelessSettings,
        },
        evp_state::UUID,
        evp_state::{AgentDeviceConfig, AgentSystemInfo},
    },
    crate::mqtt_ctrl::MqttCtrl,
    crate::{
        app::{App, ConfigKey},
        error::{DMError, DMErrorExt},
    },
    error_stack::{Context, Report, Result, ResultExt},
    jlogger_tracing::{JloggerBuilder, LevelFilter, LogTimeFormat, jdebug, jerror, jinfo},
    json::{JsonValue, object::Object},
    pest::{Parser, Token},
    regex::Regex,
    rumqttc::{Client, Connection, MqttOptions, QoS},
    std::{
        collections::HashMap,
        time::{self, Duration, Instant},
    },
};

fn fix_str(s: &str) -> String {
    let t = s.trim();
    // Treat any string consisting solely of double-quote characters (for example: "", """", etc.),
    // possibly surrounded by whitespace, as a sentinel that means the empty string.
    if !t.is_empty() && t.chars().all(|c| c == '"') {
        String::new()
    } else {
        t.to_owned()
    }
}

pub fn parse_evp_device_config(
    agent_device_config: Option<&AgentDeviceConfig>,
    config_key: &[String],
) -> Result<String, DMError> {
    if let Some(agent_device_config) = agent_device_config {
        let mut json = Object::new();
        let report_status_interval_min = config_key
            .get(usize::from(ConfigKey::ReportStatusIntervalMin))
            .unwrap();

        let report_status_interval_max: &String = config_key
            .get(usize::from(ConfigKey::ReportStatusIntervalMax))
            .unwrap();

        if report_status_interval_min.is_empty() && report_status_interval_max.is_empty() {
            return Ok(String::new());
        }

        let mut v = agent_device_config.report_status_interval_min;
        if !report_status_interval_min.is_empty() {
            v = report_status_interval_min
                .parse()
                .map_err(|_| Report::new(DMError::InvalidData))
                .attach_printable("report_status_interval_min must be number")?;
        }
        json.insert(
            "configuration/$agent/report-status-interval-min",
            JsonValue::Number(v.into()),
        );

        let mut v = agent_device_config.report_status_interval_max;
        if !report_status_interval_max.is_empty() {
            v = report_status_interval_max
                .parse()
                .map_err(|_| Report::new(DMError::InvalidData))
                .attach_printable("report_status_interval_max must be number")?;
        }
        json.insert(
            "configuration/$agent/report-status-interval-max",
            JsonValue::Number(v.into()),
        );

        let registry_auth = Object::new();
        json.insert(
            "configuration/$agent/registry-auth",
            JsonValue::Object(registry_auth),
        );

        let configure_id = JsonValue::String(UUID::new().uuid().to_owned());
        json.insert("configuration/$agent/configuration-id", configure_id);

        if !json.is_empty() {
            let mut tb_root = Object::new();
            tb_root.insert("desiredDeviceConfig", JsonValue::Object(json));
            let mut root = Object::new();
            root.insert("desiredDeviceConfig", JsonValue::Object(tb_root));

            Ok(json::stringify_pretty(root, 4))
        } else {
            Ok(String::default())
        }
    } else {
        Err(Report::new(DMError::InvalidData)
            .attach_printable("No agent device config data")
            .change_context(DMError::InvalidData))
    }
}

pub fn parse_user_config(focus: MainWindowFocus) -> Result<String, DMError> {
    // User configuration
    let config_file = format!("{}/{}", App::config_dir(), focus.user_config_file());
    let json_str = std::fs::read_to_string(&config_file).map_err(|_| {
        Report::new(DMError::InvalidData)
            .attach_printable(format!("Failed to read {}", config_file))
    })?;

    let json = json::parse(&json_str).map_err(|e| {
        Report::new(DMError::InvalidData).attach_printable(format!("Invalid json:\n{}", e))
    })?;

    let mut root = Object::new();
    match focus {
        MainWindowFocus::DeploymentStatus => {
            root.insert("deployment", json);

            Ok(json::stringify_pretty(root, 4))
        }
        MainWindowFocus::SystemSettings => {
            root.insert(
                "configuration/$system/system_settings",
                JsonValue::String(json.dump()),
            );

            Ok(json::stringify_pretty(root, 4))
        }
        MainWindowFocus::NetworkSettings => {
            root.insert(
                "configuration/$system/network_settings",
                JsonValue::String(json.dump()),
            );
            Ok(json::stringify_pretty(root, 4))
        }
        MainWindowFocus::WirelessSettings => {
            root.insert(
                "configuration/$system/wireless_setting",
                JsonValue::String(json.dump()),
            );
            Ok(json::stringify_pretty(root, 4))
        }

        _ => Ok(String::new()),
    }
}

pub fn parse_system_setting(config_key: &[String]) -> Result<String, DMError> {
    let mut json = Object::new();

    let led_enabled = config_key.get(usize::from(ConfigKey::LedEnabled)).unwrap();
    let temperature_update_interval = config_key
        .get(usize::from(ConfigKey::TemperatureUpdateInterval))
        .unwrap();

    if !led_enabled.is_empty() {
        let enabled: bool = led_enabled.parse().map_err(|_| {
            Report::new(DMError::InvalidData).attach_printable("led_enabled must be boolean")
        })?;

        json.insert("led_enabled", JsonValue::Boolean(enabled));
    }

    if !temperature_update_interval.is_empty() {
        let v: u32 = temperature_update_interval.parse().map_err(|_| {
            Report::new(DMError::InvalidData)
                .attach_printable("temperature_update_interval must be number")
        })?;

        json.insert("temperature_update_interval", JsonValue::Number(v.into()));
    }

    let mut log_settings = vec![];
    // all
    {
        let l = config_key
            .get(usize::from(ConfigKey::AllLogSettingLevel))
            .unwrap();
        let d = config_key
            .get(usize::from(ConfigKey::AllLogSettingDestination))
            .unwrap();
        let s = config_key
            .get(usize::from(ConfigKey::AllLogSettingStorageName))
            .unwrap();
        let p = config_key
            .get(usize::from(ConfigKey::AllLogSettingPath))
            .unwrap();
        let mut log = Object::new();
        if !l.is_empty() {
            let level: u32 = l.parse().map_err(|_| {
                Report::new(DMError::InvalidData)
                    .attach_printable(format!("level of {} must be 0, 1, 2, 3, 4 or 5.", "all"))
            })?;
            log.insert("level", JsonValue::Number(level.into()));
        }

        if !d.is_empty() {
            let destination: u32 = d.parse().map_err(|_| {
                Report::new(DMError::InvalidData)
                    .attach_printable(format!("destination of {} must be 0, 1.", "all"))
            })?;
            log.insert("destination", JsonValue::Number(destination.into()));
        }

        if !s.trim().is_empty() {
            log.insert("storage_name", JsonValue::String(fix_str(s)));
        }

        if !p.trim().is_empty() {
            log.insert("path", JsonValue::String(fix_str(p)));
        }

        if !log.is_empty() {
            log.insert("filter", JsonValue::String("all".to_owned()));
            log_settings.push(JsonValue::Object(log));
        }
    }

    // main
    {
        let l = config_key
            .get(usize::from(ConfigKey::MainLogSettingLevel))
            .unwrap();
        let d = config_key
            .get(usize::from(ConfigKey::MainLogSettingDestination))
            .unwrap();
        let s = config_key
            .get(usize::from(ConfigKey::MainLogSettingStorageName))
            .unwrap();
        let p = config_key
            .get(usize::from(ConfigKey::MainLogSettingPath))
            .unwrap();
        let mut log = Object::new();
        if !l.is_empty() {
            let level: u32 = l.parse().map_err(|_| {
                Report::new(DMError::InvalidData)
                    .attach_printable(format!("level of {} must be 0, 1, 2, 3, 4 or 5.", "main"))
            })?;
            log.insert("level", JsonValue::Number(level.into()));
        }

        if !d.is_empty() {
            let destination: u32 = d.parse().map_err(|_| {
                Report::new(DMError::InvalidData)
                    .attach_printable(format!("destination of {} must be 0, 1.", "all"))
            })?;
            log.insert("destination", JsonValue::Number(destination.into()));
        }

        if !s.trim().is_empty() {
            log.insert("storage_name", JsonValue::String(fix_str(s)));
        }

        if !p.trim().is_empty() {
            log.insert("path", JsonValue::String(fix_str(p)));
        }

        if !log.is_empty() {
            log.insert("filter", JsonValue::String("main".to_owned()));
            log_settings.push(JsonValue::Object(log));
        }
    }

    // sensor
    {
        let l = config_key
            .get(usize::from(ConfigKey::SensorLogSettingLevel))
            .unwrap();
        let d = config_key
            .get(usize::from(ConfigKey::SensorLogSettingDestination))
            .unwrap();
        let s = config_key
            .get(usize::from(ConfigKey::SensorLogSettingStorageName))
            .unwrap();
        let p = config_key
            .get(usize::from(ConfigKey::SensorLogSettingPath))
            .unwrap();
        let mut log = Object::new();
        if !l.is_empty() {
            let level: u32 = l.parse().map_err(|_| {
                Report::new(DMError::InvalidData)
                    .attach_printable(format!("level of {} must be 0, 1, 2, 3, 4 or 5.", "sensor"))
            })?;
            log.insert("level", JsonValue::Number(level.into()));
        }

        if !d.is_empty() {
            let destination: u32 = d.parse().map_err(|_| {
                Report::new(DMError::InvalidData)
                    .attach_printable(format!("destination of {} must be 0, 1.", "all"))
            })?;
            log.insert("destination", JsonValue::Number(destination.into()));
        }

        if !s.trim().is_empty() {
            log.insert("storage_name", JsonValue::String(fix_str(s)));
        }

        if !p.trim().is_empty() {
            log.insert("path", JsonValue::String(fix_str(p)));
        }

        if !log.is_empty() {
            log.insert("filter", JsonValue::String("sensor".to_owned()));
            log_settings.push(JsonValue::Object(log));
        }
    }

    // companion_fw
    {
        let l = config_key
            .get(usize::from(ConfigKey::CompanionFwLogSettingLevel))
            .unwrap();
        let d = config_key
            .get(usize::from(ConfigKey::CompanionFwLogSettingDestination))
            .unwrap();
        let s = config_key
            .get(usize::from(ConfigKey::CompanionFwLogSettingStorageName))
            .unwrap();
        let p = config_key
            .get(usize::from(ConfigKey::CompanionFwLogSettingPath))
            .unwrap();
        let mut log = Object::new();
        if !l.is_empty() {
            let level: u32 = l.parse().map_err(|_| {
                Report::new(DMError::InvalidData).attach_printable(format!(
                    "level of {} must be 0, 1, 2, 3, 4 or 5.",
                    "companion_fw"
                ))
            })?;
            log.insert("level", JsonValue::Number(level.into()));
        }

        if !d.is_empty() {
            let destination: u32 = d.parse().map_err(|_| {
                Report::new(DMError::InvalidData)
                    .attach_printable(format!("destination of {} must be 0, 1.", "companion_fw"))
            })?;
            log.insert("destination", JsonValue::Number(destination.into()));
        }

        if !s.trim().is_empty() {
            log.insert("storage_name", JsonValue::String(fix_str(s)));
        }

        if !p.trim().is_empty() {
            log.insert("path", JsonValue::String(fix_str(p)));
        }

        if !log.is_empty() {
            log.insert("filter", JsonValue::String("companion_fw".to_owned()));
            log_settings.push(JsonValue::Object(log));
        }
    }

    // companion_app
    {
        let l = config_key
            .get(usize::from(ConfigKey::CompanionAppLogSettingLevel))
            .unwrap();
        let d = config_key
            .get(usize::from(ConfigKey::CompanionAppLogSettingDestination))
            .unwrap();
        let s = config_key
            .get(usize::from(ConfigKey::CompanionAppLogSettingStorageName))
            .unwrap();
        let p = config_key
            .get(usize::from(ConfigKey::CompanionAppLogSettingPath))
            .unwrap();
        let mut log = Object::new();
        if !l.is_empty() {
            let level: u32 = l.parse().map_err(|_| {
                Report::new(DMError::InvalidData).attach_printable(format!(
                    "level of {} must be 0, 1, 2, 3, 4 or 5.",
                    "companion_app"
                ))
            })?;
            log.insert("level", JsonValue::Number(level.into()));
        }

        if !d.is_empty() {
            let destination: u32 = d.parse().map_err(|_| {
                Report::new(DMError::InvalidData)
                    .attach_printable(format!("destination of {} must be 0, 1.", "companion_app"))
            })?;
            log.insert("destination", JsonValue::Number(destination.into()));
        }

        if !s.trim().is_empty() {
            log.insert("storage_name", JsonValue::String(fix_str(s)));
        }

        if !p.trim().is_empty() {
            log.insert("path", JsonValue::String(fix_str(p)));
        }

        if !log.is_empty() {
            log.insert("filter", JsonValue::String("companion_app".to_owned()));
            log_settings.push(JsonValue::Object(log));
        }
    }

    if !log_settings.is_empty() {
        json.insert("log_settings", JsonValue::Array(log_settings));
    }

    if !json.is_empty() {
        let mut req_id = Object::new();
        let uuid = UUID::new().uuid().to_owned();
        req_id.insert("req_id", JsonValue::String(uuid));
        json.insert("req_info", JsonValue::Object(req_id));
        let mut root = Object::new();
        root.insert(
            "configuration/$system/system_settings",
            JsonValue::String(json.dump()),
        );

        Ok(json::stringify_pretty(root, 4))
    } else {
        Ok(String::new())
    }
}

pub fn parse_network_settings(config_key: &[String]) -> Result<String, DMError> {
    let mut json = Object::new();

    let ip_method = config_key.get(usize::from(ConfigKey::IpMethod)).unwrap();
    if !ip_method.is_empty() {
        let v: u32 = ip_method.parse().map_err(|_| {
            Report::new(DMError::InvalidData).attach_printable("ip_method must be 0 or 1")
        })?;
        json.insert("ip_method", JsonValue::Number(v.into()));
    }

    {
        let mut ipv4 = Object::new();
        let ipv4_ip = config_key
            .get(usize::from(ConfigKey::StaticIpv4Ip))
            .unwrap();
        if !ipv4_ip.trim().is_empty() {
            ipv4.insert("ip_address", JsonValue::String(fix_str(ipv4_ip)));
        }

        let ipv4_subnet_mask = config_key
            .get(usize::from(ConfigKey::StaticIpv4SubnetMask))
            .unwrap();
        if !ipv4_subnet_mask.trim().is_empty() {
            ipv4.insert("subnet_mask", JsonValue::String(fix_str(ipv4_subnet_mask)));
        }

        let ipv4_gateway_address = config_key
            .get(usize::from(ConfigKey::StaticIpv4Gateway))
            .unwrap();
        if !ipv4_gateway_address.trim().is_empty() {
            ipv4.insert(
                "gateway_address",
                JsonValue::String(fix_str(ipv4_gateway_address)),
            );
        }

        let ipv4_dns = config_key
            .get(usize::from(ConfigKey::StaticIpv4Dns))
            .unwrap();
        if !ipv4_dns.trim().is_empty() {
            ipv4.insert("dns_address", JsonValue::String(fix_str(ipv4_dns)));
        }

        if !ipv4.is_empty() {
            json.insert("static_settings_ipv4", JsonValue::Object(ipv4));
        }
    }

    {
        let mut ipv6 = Object::new();
        let ipv6_ip = config_key
            .get(usize::from(ConfigKey::StaticIpv6Ip))
            .unwrap();
        if !ipv6_ip.trim().is_empty() {
            ipv6.insert("ip_address", JsonValue::String(fix_str(ipv6_ip)));
        }

        let ipv6_subnet_mask = config_key
            .get(usize::from(ConfigKey::StaticIpv6SubnetMask))
            .unwrap();
        if !ipv6_subnet_mask.trim().is_empty() {
            ipv6.insert("subnet_mask", JsonValue::String(fix_str(ipv6_subnet_mask)));
        }

        let ipv6_gateway_address = config_key
            .get(usize::from(ConfigKey::StaticIpv6Gateway))
            .unwrap();
        if !ipv6_gateway_address.trim().is_empty() {
            ipv6.insert(
                "gateway_address",
                JsonValue::String(fix_str(ipv6_gateway_address)),
            );
        }

        let ipv6_dns = config_key
            .get(usize::from(ConfigKey::StaticIpv6Dns))
            .unwrap();
        if !ipv6_dns.trim().is_empty() {
            ipv6.insert("dns_address", JsonValue::String(fix_str(ipv6_dns)));
        }

        if !ipv6.is_empty() {
            json.insert("static_settings_ipv6", JsonValue::Object(ipv6));
        }
    }

    {
        let mut proxy = Object::new();
        let proxy_url = config_key.get(usize::from(ConfigKey::ProxyUrl)).unwrap();
        if !proxy_url.trim().is_empty() {
            proxy.insert("proxy_url", JsonValue::String(fix_str(proxy_url)));
        }

        let proxy_port = config_key.get(usize::from(ConfigKey::ProxyPort)).unwrap();
        if !proxy_port.is_empty() {
            let v: u32 = proxy_port.parse().map_err(|_| {
                Report::new(DMError::InvalidData).attach_printable("proxy_port must be an integer")
            })?;
            proxy.insert("proxy_port", JsonValue::Number(v.into()));
        }

        let proxy_user_name = config_key
            .get(usize::from(ConfigKey::ProxyUserName))
            .unwrap();
        if !proxy_user_name.trim().is_empty() {
            proxy.insert(
                "proxy_user_name",
                JsonValue::String(fix_str(proxy_user_name)),
            );
        }

        let proxy_password = config_key
            .get(usize::from(ConfigKey::ProxyPassword))
            .unwrap();
        if !proxy_password.trim().is_empty() {
            proxy.insert("proxy_password", JsonValue::String(fix_str(proxy_password)));
        }

        if !proxy.is_empty() {
            json.insert("proxy_settings", JsonValue::Object(proxy));
        }
    }

    let ntp_url = config_key.get(usize::from(ConfigKey::NtpUrl)).unwrap();
    if !ntp_url.trim().is_empty() {
        json.insert("ntp_url", JsonValue::String(fix_str(ntp_url)));
    }

    if !json.is_empty() {
        let mut req_id = Object::new();
        let uuid = UUID::new().uuid().to_owned();
        req_id.insert("req_id", JsonValue::String(uuid));
        json.insert("req_info", JsonValue::Object(req_id));

        let mut root = Object::new();
        root.insert(
            "configuration/$system/network_settings",
            JsonValue::String(json.dump()),
        );
        Ok(json::stringify_pretty(root, 4))
    } else {
        Ok(String::new())
    }
}

pub fn parse_wireless_settings(config_key: &[String]) -> Result<String, DMError> {
    let mut json = Object::new();

    let mut sta_mod = Object::new();
    let sta_mode_ssid = config_key.get(usize::from(ConfigKey::StaSsid)).unwrap();
    if !sta_mode_ssid.trim().is_empty() {
        sta_mod.insert("ssid", JsonValue::String(fix_str(sta_mode_ssid)));
    }

    let sta_mode_password = config_key.get(usize::from(ConfigKey::StaPassword)).unwrap();
    if !sta_mode_password.trim().is_empty() {
        sta_mod.insert("password", JsonValue::String(fix_str(sta_mode_password)));
    }

    let sta_mode_encryption = config_key
        .get(usize::from(ConfigKey::StaEncryption))
        .unwrap();

    if !sta_mode_encryption.is_empty() {
        let v: u32 = sta_mode_encryption.parse().map_err(|_| {
            Report::new(DMError::InvalidData).attach_printable("Encryption must be 0, 1 or 2")
        })?;
        sta_mod.insert("encryption", JsonValue::Number(v.into()));
    }

    if !sta_mod.is_empty() {
        let mut req_id = Object::new();
        let uuid = UUID::new().uuid().to_owned();
        req_id.insert("req_id", JsonValue::String(uuid));
        json.insert("req_info", JsonValue::Object(req_id));
        json.insert("sta_mode_setting", JsonValue::Object(sta_mod));

        let mut root = Object::new();
        root.insert(
            "configuration/$system/wireless_setting",
            JsonValue::String(json.dump()),
        );
        Ok(json::stringify_pretty(root, 4))
    } else {
        Ok(String::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_evp_device_config_01() {
        let agent_device_config = AgentDeviceConfig {
            report_status_interval_min: 3,
            report_status_interval_max: 180,
            registry_auth: String::new(),
            configuration_id: String::new(),
        };
        let config_key = vec!["5".to_owned(), "120".to_owned()];
        let result = parse_evp_device_config(Some(&agent_device_config), &config_key).unwrap();
        //eprintln!("{result}");

        let json = json::parse(&result).unwrap();
        if let JsonValue::Object(o) = json {
            let v = o.get("desiredDeviceConfig").unwrap();
            if let JsonValue::Object(o) = v {
                let v = o.get("desiredDeviceConfig").unwrap();
                if let JsonValue::Object(o) = v {
                    let v = o
                        .get("configuration/$agent/report-status-interval-min")
                        .unwrap();
                    assert_eq!(v.dump(), "5");

                    let v = o
                        .get("configuration/$agent/report-status-interval-max")
                        .unwrap();
                    assert_eq!(v.dump(), "120");
                    return;
                }
            }
        }

        panic!("Invalid result: {result}");
    }

    #[test]
    fn test_parse_evp_device_config_02() {
        let agent_device_config = AgentDeviceConfig {
            report_status_interval_min: 3,
            report_status_interval_max: 180,
            registry_auth: String::new(),
            configuration_id: String::new(),
        };

        let config_key = vec!["5".to_owned(), String::new()];
        let result = parse_evp_device_config(Some(&agent_device_config), &config_key).unwrap();
        //eprintln!("{result}");

        let json = json::parse(&result).unwrap();
        if let JsonValue::Object(o) = json {
            let v = o.get("desiredDeviceConfig").unwrap();
            if let JsonValue::Object(o) = v {
                let v = o.get("desiredDeviceConfig").unwrap();
                if let JsonValue::Object(o) = v {
                    let v = o
                        .get("configuration/$agent/report-status-interval-min")
                        .unwrap();
                    assert_eq!(v.dump(), "5");

                    let v = o
                        .get("configuration/$agent/report-status-interval-max")
                        .unwrap();
                    assert_eq!(v.dump(), "180");
                    return;
                }
            }
        }

        panic!("Invalid result: {result}");
    }

    #[test]
    fn test_parse_evp_device_config_03() {
        let agent_device_config = AgentDeviceConfig {
            report_status_interval_min: 3,
            report_status_interval_max: 180,
            registry_auth: String::new(),
            configuration_id: String::new(),
        };

        let config_key = vec![String::new(), "120".to_owned()];
        let result = parse_evp_device_config(Some(&agent_device_config), &config_key).unwrap();
        //eprintln!("{result}");

        let json = json::parse(&result).unwrap();
        if let JsonValue::Object(o) = json {
            let v = o.get("desiredDeviceConfig").unwrap();
            if let JsonValue::Object(o) = v {
                let v = o.get("desiredDeviceConfig").unwrap();
                if let JsonValue::Object(o) = v {
                    let v = o
                        .get("configuration/$agent/report-status-interval-min")
                        .unwrap();
                    assert_eq!(v.dump(), "3");

                    let v = o
                        .get("configuration/$agent/report-status-interval-max")
                        .unwrap();
                    assert_eq!(v.dump(), "120");
                    return;
                }
            }
        }

        panic!("Invalid result: {result}");
    }

    #[test]
    fn test_parse_evp_device_config_04() {
        let agent_device_config = AgentDeviceConfig {
            report_status_interval_min: 3,
            report_status_interval_max: 180,
            registry_auth: String::new(),
            configuration_id: String::new(),
        };
        let config_key = vec!["a".to_owned(), "180".to_owned()];
        let result = parse_evp_device_config(Some(&agent_device_config), &config_key);
        assert!(result.is_err());

        assert_eq!(
            result.unwrap_err().error_str(),
            Some("report_status_interval_min must be number".to_owned())
        )
    }

    #[test]
    fn test_parse_evp_device_config_05() {
        let agent_device_config = AgentDeviceConfig {
            report_status_interval_min: 3,
            report_status_interval_max: 180,
            registry_auth: String::new(),
            configuration_id: String::new(),
        };

        let config_key = vec!["3".to_owned(), "b".to_owned()];
        let result = parse_evp_device_config(Some(&agent_device_config), &config_key);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().error_str(),
            Some("report_status_interval_max must be number".to_owned())
        )
    }

    #[test]
    fn test_parse_evp_device_config_06() {
        let agent_device_config = AgentDeviceConfig {
            report_status_interval_min: 3,
            report_status_interval_max: 180,
            registry_auth: String::new(),
            configuration_id: String::new(),
        };
        let config_key = vec![String::new(), String::new()];
        let result = parse_evp_device_config(Some(&agent_device_config), &config_key).unwrap();
        assert!(result.is_empty())
    }

    #[test]
    fn test_fix_str() {
        // exact four double-quotes -> empty string
        assert_eq!(fix_str(r#""""#), "".to_owned());
        // trimmed whitespace
        assert_eq!(fix_str("  abc  "), "abc".to_owned());
        // string that is not the exact four-quote sentinel remains unchanged except trimming
        assert_eq!(
            fix_str("  hello \"\" world  "),
            "hello \"\" world".to_owned()
        );
        // input with surrounding whitespace and four quotes should be treated as the sentinel
        assert_eq!(fix_str("  \"\"\"\"  "), "".to_owned());
    }
}
