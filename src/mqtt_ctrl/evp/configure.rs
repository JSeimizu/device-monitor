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
        app::ConfigKey,
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

pub fn parse_evp_device_config(
    agent_device_config: &AgentDeviceConfig,
    config_key: &Vec<String>,
) -> Result<String, DMError> {
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
            .attach_printable(format!("report_status_interval_min must be number"))?;
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
            .attach_printable(format!("report_status_interval_max must be number"))?;
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
        let result = parse_evp_device_config(&agent_device_config, &config_key).unwrap();
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
        let result = parse_evp_device_config(&agent_device_config, &config_key).unwrap();
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
        let result = parse_evp_device_config(&agent_device_config, &config_key).unwrap();
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
        let result = parse_evp_device_config(&agent_device_config, &config_key);
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
        let result = parse_evp_device_config(&agent_device_config, &config_key);
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
        let result = parse_evp_device_config(&agent_device_config, &config_key).unwrap();
        assert!(result.is_empty())
    }
}
