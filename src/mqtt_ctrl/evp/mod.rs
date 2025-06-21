pub mod configure;
pub mod device_info;
pub mod elog;
pub mod evp_state;
pub mod module;
pub mod rpc;

use evp_state::DeploymentStatus;
#[allow(unused)]
use {
    crate::app::DirectCommand,
    crate::error::DMError,
    device_info::{
        DeviceCapabilities, DeviceInfo, DeviceReserved, DeviceStates, NetworkSettings,
        SystemSettings, WirelessSettings,
    },
    elog::Elog,
    error_stack::{Report, Result},
    evp_state::{AgentDeviceConfig, AgentSystemInfo},
    jlogger_tracing::{JloggerBuilder, LevelFilter, LogTimeFormat, jdebug, jerror, jinfo},
    json::JsonValue,
    pest::Parser,
    pest::Token,
    regex::Regex,
    rpc::{RpcResInfo, RpcResponse, parse_rpc_response},
    rumqttc::{Client, Connection, MqttOptions, QoS},
    serde::{Deserialize, Serialize},
    std::fmt::Display,
    std::{
        collections::HashMap,
        time::{self, Duration, Instant},
    },
};

#[derive(pest_derive::Parser)]
#[grammar = "src/mqtt_ctrl/evp/evp.pest"]
struct EvpParser;

pub struct JsonUtility {}

#[allow(unused)]
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

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Default)]
pub struct ReqId {
    req_id: String,
}

impl Display for ReqId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "req_id={}", self.req_id)
    }
}

impl ReqId {
    pub fn req_id(&self) -> &str {
        &self.req_id
    }
}

/// ResInfo in direct command does not contain `res_id`
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct ResInfo {
    res_id: Option<String>,
    code: i32,
    detail_msg: String,
}

impl Default for ResInfo {
    fn default() -> Self {
        Self {
            res_id: None,
            code: i32::MAX,
            detail_msg: String::default(),
        }
    }
}

impl Display for ResInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "req_id={}, code={}\n detail_msg={}",
            self.res_id.as_deref().unwrap_or(""),
            self.code,
            self.detail_msg
        )
    }
}

impl ResInfo {
    pub fn res_id(&self) -> &str {
        self.res_id.as_deref().unwrap_or("")
    }

    pub fn code_str(&self) -> &'static str {
        match self.code {
            0 => "OK(0)",
            1 => "CANCELLED(1)",
            2 => "UNKNOWN(2)",
            3 => "INVALID_ARGUMENT(3)",
            4 => "DEADLINE_EXCEEDED(4)",
            5 => "NOT_FOUND(5)",
            6 => "ALREADY_EXISTS(6)",
            7 => "PERMITTED_DENIED(7)",
            8 => "RESOURCE_EXHAUSTED(8)",
            9 => "FAILED_PRECONDITION(9)",
            10 => "ABORTED(10)",
            11 => "OUT_OF_RANGE(11)",
            12 => "UNIMPLEMENTED(12)",
            13 => "INTERNAL(13)",
            14 => "UNAVAILABLE(14)",
            15 => "DATA_LOSS(15)",
            16 => "UNAUTHENTICATED(16)",
            _ => "",
        }
    }

    pub fn code(&self) -> i32 {
        self.code
    }

    pub fn detail_msg(&self) -> &str {
        &self.detail_msg
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
    NetworkSettings(Box<NetworkSettings>),
    WirelessSettings(WirelessSettings),
    AgentDeviceConfig(AgentDeviceConfig),
    AgentSystemInfo(Box<AgentSystemInfo>),
    DeploymentStatus(DeploymentStatus),
    Elog(Elog),
    RpcRequest((u32, DirectCommand)),
    RpcResponse((u32, RpcResInfo)),
    ClientMsg(HashMap<String, String>),
    ServerMsg(HashMap<String, String>),
    NonEvp(HashMap<String, String>),
}

impl EvpMsg {
    pub fn req_id_from_topic(topic: &str) -> Result<u32, DMError> {
        let re = Regex::new(r"/(\d+)$").map_err(|_| Report::new(DMError::InvalidData))?;
        if let Some(caps) = re.captures(topic) {
            if let Some(req_id) = caps.get(1) {
                return req_id
                    .as_str()
                    .parse()
                    .map_err(|_| Report::new(DMError::InvalidData));
            }
        }
        Err(Report::new(DMError::InvalidData))
    }

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

    fn parse_telemetry(_topic: &str, payload: &str) -> Result<Vec<EvpMsg>, DMError> {
        jdebug!(
            func = "EvpMsg::parse_telemetry()",
            line = line!(),
            check = payload
        );

        if let Ok(JsonValue::Object(obj)) = json::parse(payload) {
            jdebug!(
                func = "EvpMsg::parse_telemetry()",
                line = line!(),
                check = format!("{:?}", obj)
            );
            for (k, v) in obj.iter() {
                if k == "$system/event_log" {
                    return Elog::parse(&v.dump()).map(|elog| vec![EvpMsg::Elog(elog)]);
                }
            }
        }

        Err(Report::new(DMError::InvalidData))
    }

    fn parse_configure_state_msg(_topic: &str, payload: &str) -> Result<Vec<EvpMsg>, DMError> {
        jdebug!(
            func = "EvpMsg::parse_configure_state_msg()",
            line = line!(),
            check = payload
        );
        if let Ok(JsonValue::Object(obj)) = json::parse(payload) {
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
            let mut deployment_status: Option<DeploymentStatus> = None;

            for (k, v) in obj.iter() {
                if k.starts_with("state") {
                    jinfo!(
                        event = "STATE",
                        key = k,
                        value = JsonUtility::json_value_to_string(v)
                    );
                }

                if k.starts_with("desiredDeviceConfig") {
                    jinfo!(
                        event = "AGENT_CONFIGURATION",
                        key = k,
                        value = JsonUtility::json_value_to_string(v)
                    );
                }

                if k.starts_with("configuration") {
                    jinfo!(
                        event = "CONFIGURATION",
                        key = k,
                        value = JsonUtility::json_value_to_string(v)
                    );
                }

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

                if k == "deploymentStatus" {
                    let s = JsonUtility::json_value_to_string(v);
                    deployment_status = Some(DeploymentStatus::parse(&s)?);
                    continue;
                }

                if k == "state/$system/device_info" {
                    let s = JsonUtility::json_value_to_string(v);
                    device_info = Some(
                        serde_json::from_str(&s)
                            .map_err(|_| Report::new(DMError::InvalidData))
                            .unwrap(),
                    );

                    continue;
                }

                if k == "state/$system/device_states" {
                    let s = JsonUtility::json_value_to_string(v);
                    device_states = Some(
                        serde_json::from_str(&s)
                            .map_err(|_| Report::new(DMError::InvalidData))
                            .unwrap(),
                    );

                    continue;
                }

                if k == "state/$system/device_capabilities" {
                    let s = JsonUtility::json_value_to_string(v);
                    device_capabilities = Some(
                        serde_json::from_str(&s)
                            .map_err(|_| Report::new(DMError::InvalidData))
                            .unwrap(),
                    );

                    continue;
                }

                if k == "state/$system/PRIVATE_reserved" {
                    let s = JsonUtility::json_value_to_string(v);
                    device_reserved = Some(
                        serde_json::from_str(&s)
                            .map_err(|_| Report::new(DMError::InvalidData))
                            .unwrap(),
                    );

                    continue;
                }

                if k == "state/$system/system_settings" {
                    let s = JsonUtility::json_value_to_string(v);
                    system_settings = Some(
                        serde_json::from_str(&s)
                            .map_err(|_| Report::new(DMError::InvalidData))
                            .unwrap(),
                    );

                    continue;
                }

                if k == "state/$system/network_settings" {
                    let s = JsonUtility::json_value_to_string(v);
                    network_settings = Some(
                        serde_json::from_str(&s)
                            .map_err(|_| Report::new(DMError::InvalidData))
                            .unwrap(),
                    );

                    continue;
                }

                if k == "state/$system/wireless_setting" {
                    let s = JsonUtility::json_value_to_string(v);
                    wireless_settings = Some(
                        serde_json::from_str(&s)
                            .map_err(|_| Report::new(DMError::InvalidData))
                            .unwrap(),
                    );

                    continue;
                }
            }

            if let Some(config) = agent_device_config {
                result.push(EvpMsg::AgentDeviceConfig(config));
            }

            if let Some(sys) = system_info {
                result.push(EvpMsg::AgentSystemInfo(Box::new(sys)));
            }

            if let Some(status) = deployment_status {
                result.push(EvpMsg::DeploymentStatus(status));
            }

            if let Some(dev) = device_info {
                result.push(EvpMsg::DeviceInfoMsg(dev));
            }

            if let Some(dev) = device_states {
                result.push(EvpMsg::DeviceStatesMsg(dev));
            }

            if let Some(dev) = device_capabilities {
                result.push(EvpMsg::DeviceCapabilities(dev));
            }

            if let Some(dev) = device_reserved {
                result.push(EvpMsg::DeviceReserved(dev));
            }

            if let Some(dev) = system_settings {
                result.push(EvpMsg::SystemSettings(dev));
            }

            if let Some(dev) = network_settings {
                result.push(EvpMsg::NetworkSettings(Box::new(dev)));
            }

            if let Some(dev) = wireless_settings {
                result.push(EvpMsg::WirelessSettings(dev));
            }

            jdebug!(
                func = "EvpMsg::parse_configure_state_msg()",
                line = line!(),
                result = format!("{:?}", result)
            );

            Ok(result)
        } else {
            Err(Report::new(DMError::InvalidData))
        }
    }

    pub fn parse(topic: &str, payload: &str) -> Result<Vec<EvpMsg>, DMError> {
        let mut result = vec![];
        let mut hash = HashMap::new();
        hash.insert(topic.to_owned(), payload.to_owned());

        jdebug!(func = "EvpMsg::parse()", line = line!(), topic = topic);

        // "v1/devices/me/attributes"
        // https://thingsboard.io/docs/reference/mqtt-api/#subscribe-to-attribute-updates-from-the-server
        if EvpParser::parse(Rule::attribute_common, topic).is_ok() {
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

            // "v1/devices/me/attributes"
            if let Ok(msg) = EvpMsg::parse_configure_state_msg(topic, payload) {
                return Ok(msg);
            }

            return Ok(vec![EvpMsg::ClientMsg(hash)]);
        }

        // https://thingsboard.io/docs/reference/mqtt-api/#request-attribute-values-from-the-server
        if EvpParser::parse(Rule::server_attr_common, topic).is_ok() {
            return Ok(vec![EvpMsg::ServerMsg(hash)]);
        }

        jdebug!(func = "EvpMsg::parse()", line = line!(), topic = topic);

        //"v1/devices/me/rpc/request/
        // https://thingsboard.io/docs/reference/mqtt-api/#server-side-rpc
        if EvpParser::parse(Rule::server_rpc_common, topic).is_ok() {
            jinfo!(event = "RPC request", topic = topic, payload = payload);
            if let Ok(req_id) = EvpMsg::req_id_from_topic(topic) {
                if let Ok(JsonValue::Object(json)) = json::parse(payload) {
                    if let Some(cmd) = json
                        .get("params")
                        .and_then(|params| {
                            if let JsonValue::Object(obj) = params {
                                Some(obj)
                            } else {
                                None
                            }
                        })
                        .and_then(|params| params.get("direct-command-request"))
                        .and_then(|request| {
                            if let JsonValue::Object(obj) = request {
                                Some(obj)
                            } else {
                                None
                            }
                        })
                        .and_then(|request| request.get("method"))
                        .and_then(|method| method.as_str())
                        .map(|method| match method {
                            "reboot" => DirectCommand::Reboot,
                            "direct_get_image" => DirectCommand::GetDirectImage,
                            "factory_reset" => DirectCommand::FactoryReset,
                            _ => DirectCommand::Invalid,
                        })
                    {
                        jinfo!(
                            event = "RPC request",
                            req_id = req_id,
                            cmd = format!("{:?}", cmd)
                        );
                        return Ok(vec![EvpMsg::RpcRequest((req_id, cmd))]);
                    }
                }
            }
        }

        jdebug!(func = "EvpMsg::parse()", line = line!(), topic = topic);
        // https://thingsboard.io/docs/reference/mqtt-api/#client-side-rpc
        if EvpParser::parse(Rule::client_rpc_common, topic).is_ok() {
            jinfo!(event = "RPC Response", topic = topic, payload = payload);
            let req_id =
                EvpMsg::req_id_from_topic(topic).map_err(|_| Report::new(DMError::InvalidData))?;

            if let Ok(rpc_response) = parse_rpc_response(payload) {
                return Ok(vec![EvpMsg::RpcResponse((req_id, rpc_response))]);
            }
            jdebug!(
                func = "EvpMsg::parse()",
                line = line!(),
                req_id = req_id,
                payload = payload
            );
        }

        jdebug!(func = "EvpMsg::parse()", line = line!(), topic = topic);
        // "v1/devices/me/telemetry"
        if EvpParser::parse(Rule::telemetry, topic).is_ok() {
            jinfo!(event = "TELEMETRY", topic = topic, payload = payload);
            if let Ok(msg) = EvpMsg::parse_telemetry(topic, payload) {
                jinfo!(
                    event = "ELOG",
                    payload = ?msg[0]
                );
                return Ok(msg);
            }
        }

        jdebug!(func = "EvpMsg::parse()", line = line!(), topic = topic);
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
