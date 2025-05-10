#[allow(unused)]
use {
    super::{
        device_info::{
            DeviceCapabilities, DeviceInfo, DeviceReserved, DeviceStates, NetworkSettings,
            SystemSettings, WirelessSettings,
        },
        evp_state::{AgentDeviceConfig, AgentSystemInfo},
    },
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

pub fn parse_evp_device_config(config_key: &Vec<String>) -> Result<String, DMError> {
    let mut json = Object::new();
    {
        let report_status_interval_min = config_key
            .get(usize::from(ConfigKey::ReportStatusIntervalMin))
            .unwrap();

        if !report_status_interval_min.is_empty() {
            let v: u32 = report_status_interval_min
                .parse()
                .map_err(|_| Report::new(DMError::InvalidData))
                .attach_printable(format!("report_status_interval_min must be number"))?;

            json.insert("report-status-interval-min", JsonValue::Number(v.into()));
        }
    }

    {
        let report_status_interval_max: &String = config_key
            .get(usize::from(ConfigKey::ReportStatusIntervalMax))
            .unwrap();

        if !report_status_interval_max.is_empty() {
            let v: u32 = report_status_interval_max
                .parse()
                .map_err(|_| Report::new(DMError::InvalidData))
                .attach_printable(format!("report_status_interval_max must be number"))?;

            json.insert("report-status-interval-max", JsonValue::Number(v.into()));
        }
    }

    if !json.is_empty() {
        Ok(json::stringify_pretty(json, 4))
    } else {
        Ok(String::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_evp_device_config_01() {
        let config_key = vec!["3".to_owned(), "180".to_owned()];
        assert_eq!(
            parse_evp_device_config(&config_key).unwrap(),
            r#"{"report-status-interval-min":3,"report-status-interval-max":180}"#
        );
    }

    #[test]
    fn test_parse_evp_device_config_02() {
        let config_key = vec!["3".to_owned(), String::new()];
        assert_eq!(
            parse_evp_device_config(&config_key).unwrap(),
            r#"{"report-status-interval-min":3}"#
        )
    }

    #[test]
    fn test_parse_evp_device_config_03() {
        let config_key = vec![String::new(), "180".to_owned()];
        assert_eq!(
            parse_evp_device_config(&config_key).unwrap(),
            r#"{"report-status-interval-max":180}"#
        );
    }

    #[test]
    fn test_parse_evp_device_config_04() {
        let config_key = vec!["a".to_owned(), "180".to_owned()];
        let result = parse_evp_device_config(&config_key);
        assert!(result.is_err());

        assert_eq!(
            result.unwrap_err().error_str(),
            Some("report_status_interval_min must be number".to_owned())
        )
    }

    #[test]
    fn test_parse_evp_device_config_05() {
        let config_key = vec!["3".to_owned(), "b".to_owned()];
        let result = parse_evp_device_config(&config_key);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().error_str(),
            Some("report_status_interval_max must be number".to_owned())
        )
    }

    #[test]
    fn test_parse_evp_device_config_06() {
        let config_key = vec![String::new(), String::new()];
        let result = parse_evp_device_config(&config_key).unwrap();
        assert!(result.is_empty())
    }
}
