pub mod device_info;
pub mod evp_state;

use pest::Token;
#[allow(unused)]
use {
    crate::error::DMError,
    device_info::{
        DeviceCapabilities, DeviceInfo, DeviceReserved, DeviceStates, NetworkSettings,
        SystemSettings, WirelessSettings,
    },
    error_stack::{Report, Result},
    evp_state::{AgentDeviceConfig, AgentSystemInfo},
    jlogger_tracing::{JloggerBuilder, LevelFilter, LogTimeFormat, jdebug, jerror, jinfo},
    json::JsonValue,
    pest::Parser,
    regex::Regex,
    rumqttc::{Client, Connection, MqttOptions, QoS},
    std::{
        collections::HashMap,
        time::{self, Duration, Instant},
    },
};

#[derive(pest_derive::Parser)]
#[grammar = "src/mqtt_ctrl/evp/evp.pest"]
struct EvpParser;

pub struct JsonUtility {}

impl JsonUtility {
    pub fn json_value_to_string(v: &JsonValue) -> String {
        v.as_str().map(|s| s.to_owned()).unwrap_or_else(|| v.dump())
    }

    pub fn json_type(v: &JsonValue) -> String {
        match v {
            JsonValue::Null => "null".to_owned(),
            JsonValue::Short(_v) => "Short".to_owned(),
            JsonValue::Array(_v) => "array".to_owned(),
            JsonValue::String(_v) => "string".to_owned(),
            JsonValue::Number(_v) => "number".to_owned(),
            JsonValue::Object(_v) => "object".to_owned(),
            JsonValue::Boolean(_v) => "boolean".to_owned(),
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum EvpMsg {
    ConnectMsg((String, u32)),
    ConnectRespMsg((String, u32)),
    DeviceInfoMsg(DeviceInfo),
    DeviceStatesMsg(DeviceStates),
    DeviceCapabilities(DeviceCapabilities),
    DeviceReserved(DeviceReserved),
    SystemSettings(SystemSettings),
    NetworkSettings(NetworkSettings),
    WirelessSettings(WirelessSettings),
    AgentDeviceConfig(AgentDeviceConfig),
    AgentSystemInfo(AgentSystemInfo),
    ClientMsg(HashMap<String, String>),
    ServerMsg(HashMap<String, String>),
    RpcServer(HashMap<String, String>),
    RpcClient(HashMap<String, String>),
    NonEvp(HashMap<String, String>),
}

impl EvpMsg {
    fn parse_connect_request(topic: &str, _payload: &str) -> Result<Vec<EvpMsg>, DMError> {
        let pairs = EvpParser::parse(Rule::client_attr_connection, topic)
            .map_err(|_| Report::new(DMError::InvalidData))?;

        let mut who_start = 0;
        let mut who_end = 0;
        let mut req_id_start = 0;
        let mut req_id_end = 0;

        for token in pairs.tokens() {
            match token {
                Token::Start { rule, pos } => match rule {
                    Rule::who => who_start = pos.pos(),
                    Rule::id => req_id_start = pos.pos(),
                    _ => {}
                },
                Token::End { rule, pos } => match rule {
                    Rule::who => who_end = pos.pos(),
                    Rule::id => req_id_end = pos.pos(),
                    _ => {}
                },
            }
        }

        let who = topic[who_start..who_end].to_owned();
        let req_id = topic[req_id_start..req_id_end]
            .parse()
            .map_err(|_| Report::new(DMError::InvalidData))?;

        Ok(vec![EvpMsg::ConnectMsg((who, req_id))])
    }

    fn parse_connect_response(topic: &str, _payload: &str) -> Result<Vec<EvpMsg>, DMError> {
        let pairs = EvpParser::parse(Rule::server_attr_connection, topic)
            .map_err(|_| Report::new(DMError::InvalidData))?;

        let mut who_start = 0;
        let mut who_end = 0;
        let mut req_id_start = 0;
        let mut req_id_end = 0;

        for token in pairs.tokens() {
            match token {
                Token::Start { rule, pos } => match rule {
                    Rule::who => who_start = pos.pos(),
                    Rule::id => req_id_start = pos.pos(),
                    _ => {}
                },
                Token::End { rule, pos } => match rule {
                    Rule::who => who_end = pos.pos(),
                    Rule::id => req_id_end = pos.pos(),
                    _ => {}
                },
            }
        }

        let who = topic[who_start..who_end].to_owned();
        let req_id = topic[req_id_start..req_id_end]
            .parse()
            .map_err(|_| Report::new(DMError::InvalidData))?;

        Ok(vec![EvpMsg::ConnectRespMsg((who, req_id))])
    }

    fn parse_state_msg(_topic: &str, payload: &str) -> Result<Vec<EvpMsg>, DMError> {
        if let Ok(JsonValue::Object(obj)) =
            json::parse(payload).map_err(|_| Report::new(DMError::InvalidData))
        {
            let mut result = vec![];
            let mut agent_device_config: Option<AgentDeviceConfig> = None;
            let mut system_info: Option<AgentSystemInfo> = None;
            let mut device_info: Option<DeviceInfo> = None;
            let mut device_states: Option<DeviceStates> = None;
            let mut device_capabilities: Option<DeviceCapabilities> = None;
            let mut device_reserved: Option<DeviceReserved> = None;
            let mut system_settings: Option<SystemSettings> = None;
            let mut network_settings: Option<NetworkSettings> = None;
            let mut wireless_settings: Option<WirelessSettings> = None;

            for (k, v) in obj.iter() {
                jdebug!(
                    func = "EvpMsg::parse_state_msg()",
                    line = line!(),
                    key = k,
                    value_type = JsonUtility::json_type(v)
                );

                if k == "state/$agent/report-status-interval-min" {
                    let value = v.as_u32().ok_or(Report::new(DMError::InvalidData))?;
                    if agent_device_config.is_none() {
                        agent_device_config = Some(AgentDeviceConfig::default());
                    }

                    agent_device_config
                        .as_mut()
                        .unwrap()
                        .report_status_interval_min = value;

                    continue;
                }

                if k == "state/$agent/report-status-interval-max" {
                    let value = v.as_u32().ok_or(Report::new(DMError::InvalidData))?;
                    if agent_device_config.is_none() {
                        agent_device_config = Some(AgentDeviceConfig::default());
                    }

                    agent_device_config
                        .as_mut()
                        .unwrap()
                        .report_status_interval_max = value;

                    continue;
                }

                if k == "systemInfo" {
                    let s = JsonUtility::json_value_to_string(v);
                    system_info = Some(AgentSystemInfo::parse(&s)?);
                    continue;
                }

                if k == "state/$system/device_info" {
                    let s = JsonUtility::json_value_to_string(v);
                    device_info = Some(
                        serde_json::from_str(&s).map_err(|_| Report::new(DMError::InvalidData))?,
                    );

                    continue;
                }

                if k == "state/$system/device_states" {
                    let s = JsonUtility::json_value_to_string(v);
                    jdebug!(
                        func = "EvpMsg::parse_state_msg()",
                        line = line!(),
                        key = k,
                        v_string = s
                    );
                    device_states = Some(
                        serde_json::from_str(&s).map_err(|_| Report::new(DMError::InvalidData))?,
                    );

                    continue;
                }

                if k == "state/$system/device_capabilities" {
                    let s = JsonUtility::json_value_to_string(v);
                    jdebug!(
                        func = "EvpMsg::parse_state_msg()",
                        line = line!(),
                        key = k,
                        v_string = s
                    );
                    device_capabilities = Some(
                        serde_json::from_str(&s).map_err(|_| Report::new(DMError::InvalidData))?,
                    );

                    continue;
                }

                if k == "state/$system/PRIVATE_reserved" {
                    let s = JsonUtility::json_value_to_string(v);
                    jdebug!(
                        func = "EvpMsg::parse_state_msg()",
                        line = line!(),
                        key = k,
                        v_string = s
                    );
                    device_reserved = Some(
                        serde_json::from_str(&s).map_err(|_| Report::new(DMError::InvalidData))?,
                    );

                    continue;
                }

                if k == "state/$system/system_settings" {
                    let s = JsonUtility::json_value_to_string(v);
                    jdebug!(
                        func = "EvpMsg::parse_state_msg()",
                        line = line!(),
                        key = k,
                        v_string = s
                    );

                    system_settings = Some(
                        serde_json::from_str(&s)
                            .map_err(|_| Report::new(DMError::InvalidData))
                            .unwrap(),
                    );

                    continue;
                }

                if k == "state/$system/network_settings" {
                    let s = JsonUtility::json_value_to_string(v);
                    jdebug!(
                        func = "EvpMsg::parse_state_msg()",
                        line = line!(),
                        key = k,
                        v_string = s
                    );

                    network_settings = Some(
                        serde_json::from_str(&s)
                            .map_err(|_| Report::new(DMError::InvalidData))
                            .unwrap(),
                    );

                    continue;
                }

                if k == "state/$system/wireless_setting" {
                    let s = JsonUtility::json_value_to_string(v);
                    jdebug!(
                        func = "EvpMsg::parse_state_msg()",
                        line = line!(),
                        key = k,
                        wireless_setting = s
                    );

                    wireless_settings = Some(
                        serde_json::from_str(&s)
                            .map_err(|_| Report::new(DMError::InvalidData))
                            .unwrap(),
                    );

                    continue;
                }
                jdebug!(
                    func = "EvpMsg::parse_state_msg()",
                    line = line!(),
                    key = k,
                    value_type = JsonUtility::json_type(v),
                    note = "Not processed"
                );
            }

            if let Some(config) = agent_device_config {
                jdebug!(
                    func = "EvpMsg::parse_state_msg()",
                    line = line!(),
                    agent_device_config = format!("{:?}", config)
                );

                result.push(EvpMsg::AgentDeviceConfig(config));
            }

            if let Some(sys) = system_info {
                jdebug!(
                    func = "EvpMsg::parse_state_msg()",
                    line = line!(),
                    agent_system_info = format!("{:?}", sys)
                );
                result.push(EvpMsg::AgentSystemInfo(sys));
            }

            if let Some(dev) = device_info {
                jdebug!(
                    func = "EvpMsg::parse_state_msg()",
                    line = line!(),
                    device_info = format!("{:?}", dev)
                );
                result.push(EvpMsg::DeviceInfoMsg(dev));
            }

            if let Some(dev) = device_states {
                jdebug!(
                    func = "EvpMsg::parse_state_msg()",
                    line = line!(),
                    device_states = format!("{:?}", dev)
                );
                result.push(EvpMsg::DeviceStatesMsg(dev));
            }

            if let Some(dev) = device_capabilities {
                jdebug!(
                    func = "EvpMsg::parse_state_msg()",
                    line = line!(),
                    device_capabilities = format!("{:?}", dev)
                );
                result.push(EvpMsg::DeviceCapabilities(dev));
            }

            if let Some(dev) = device_reserved {
                jdebug!(
                    func = "EvpMsg::parse_state_msg()",
                    line = line!(),
                    device_reserved = format!("{:?}", dev)
                );
                result.push(EvpMsg::DeviceReserved(dev));
            }

            if let Some(dev) = system_settings {
                jdebug!(
                    func = "EvpMsg::parse_state_msg()",
                    line = line!(),
                    system_settings = format!("{:?}", dev)
                );
                result.push(EvpMsg::SystemSettings(dev));
            }

            if let Some(dev) = network_settings {
                jdebug!(
                    func = "EvpMsg::parse_state_msg()",
                    line = line!(),
                    network_settings = format!("{:?}", dev)
                );
                result.push(EvpMsg::NetworkSettings(dev));
            }

            if let Some(dev) = wireless_settings {
                jdebug!(
                    func = "EvpMsg::parse_state_msg()",
                    line = line!(),
                    wireless_settings = format!("{:?}", dev)
                );
                result.push(EvpMsg::WirelessSettings(dev));
            }

            Ok(result)
        } else {
            Err(Report::new(DMError::InvalidData))
        }
    }

    fn parse_config_msg(topic: &str, payload: &str) -> Result<Vec<EvpMsg>, DMError> {
        if payload.starts_with("configuration/") {
            let mut hash = HashMap::new();
            hash.insert(topic.to_owned(), payload.to_owned());
            Ok(vec![EvpMsg::ServerMsg(hash)])
        } else {
            Err(Report::new(DMError::InvalidData))
        }
    }

    pub fn parse(topic: &str, payload: &str) -> Result<Vec<EvpMsg>, DMError> {
        let mut result = vec![];
        let mut hash = HashMap::new();
        hash.insert(topic.to_owned(), payload.to_owned());

        // "v1/devices/me/attributes"
        // https://thingsboard.io/docs/reference/mqtt-api/#subscribe-to-attribute-updates-from-the-server
        if let Ok(_) = EvpParser::parse(Rule::attribute_common, topic) {
            // "v1/devices/me/attributes/request"
            if let Ok(msg) = EvpMsg::parse_connect_request(topic, payload) {
                jinfo!(event = "CONNECTION", note = "request");
                return Ok(msg);
            }

            // "v1/devices/me/attributes/response"
            if let Ok(msg) = EvpMsg::parse_connect_response(topic, payload) {
                jinfo!(event = "CONNECTION", note = "response");
                return Ok(msg);
            }

            jdebug!(func = "EvpMsg::parse()", line = line!(), check = payload);

            // "v1/devices/me/attributes"
            if let Ok(msg) = EvpMsg::parse_state_msg(topic, payload) {
                return Ok(msg);
            }

            jdebug!(func = "EvpMsg::parse()", line = line!(), check = payload);

            // "v1/devices/me/attributes"
            if let Ok(msg) = EvpMsg::parse_config_msg(topic, payload) {
                return Ok(msg);
            }

            return Ok(vec![EvpMsg::ClientMsg(hash)]);
        }

        jdebug!(func = "EvpMsg::parse()", line = line!(), check = payload);
        // https://thingsboard.io/docs/reference/mqtt-api/#request-attribute-values-from-the-server
        if let Ok(_) = EvpParser::parse(Rule::server_attr_common, topic) {
            return Ok(vec![EvpMsg::ServerMsg(hash)]);
        }

        //"v1/devices/me/rpc/request/
        // https://thingsboard.io/docs/reference/mqtt-api/#server-side-rpc
        if let Ok(_) = EvpParser::parse(Rule::server_rpc_common, topic) {
            return Ok(vec![EvpMsg::RpcServer(hash)]);
        }

        jdebug!(func = "EvpMsg::parse()", line = line!(), check = payload);
        // https://thingsboard.io/docs/reference/mqtt-api/#client-side-rpc
        if let Ok(_) = EvpParser::parse(Rule::client_rpc_common, topic) {
            return Ok(vec![EvpMsg::RpcClient(hash)]);
        }
        jdebug!(func = "EvpMsg::parse()", line = line!(), check = payload);

        result.push(EvpMsg::NonEvp(hash));

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_connect_request_01() {
        let topic = "v1/devices/me/attributes/request/1000";
        let payload = "";

        assert_eq!(
            EvpMsg::parse_connect_request(topic, payload).unwrap(),
            vec![EvpMsg::ConnectMsg(("me".to_owned(), 1000))]
        );
    }

    #[test]
    fn test_parse_01() {
        let topic = "v1/devices/me/attributes/request/1000";
        let payload = "";

        assert_eq!(
            EvpMsg::parse(topic, payload).unwrap(),
            vec![EvpMsg::ConnectMsg(("me".to_owned(), 1000))]
        );
    }

    #[test]
    fn test_parse_02() {
        let topic = "v1/devices/me/attributes";
        let payload = "abc";
        let mut expected = HashMap::new();
        expected.insert(topic.to_owned(), payload.to_owned());

        assert_eq!(
            EvpMsg::parse(topic, payload).unwrap(),
            vec![EvpMsg::ClientMsg(expected)]
        );
    }

    #[test]
    fn test_device_states_01() {
        let v = "{\"power_states\":{\"source\":[{\"type\":-1,\"level\":100}],\"in_use\":-1,\"is_battery_low\":false},\"process_state\":\"Idle\",\"hours_meter\":12,\"bootup_reason\":0,\"last_bootup_time\":\"2025-05-04T17:41:53.869Z\"}";
        let _device_states: DeviceStates = serde_json::from_str(v).unwrap();
    }
}
