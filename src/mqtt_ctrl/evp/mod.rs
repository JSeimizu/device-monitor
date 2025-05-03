pub mod device_info;
pub mod evp_sysinfo;

use pest::Token;
#[allow(unused)]
use {
    crate::error::DMError,
    device_info::DeviceInfo,
    error_stack::{Report, Result},
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

#[derive(Debug, PartialEq)]
pub enum EvpMsg {
    ConnectMsg((String, u32)),
    ConnectRespMsg((String, u32)),
    DeviceInfoMsg(DeviceInfo),
    ClientMsg(HashMap<String, String>),
    ServerMsg(HashMap<String, String>),
    RpcServer(HashMap<String, String>),
    RpcClient(HashMap<String, String>),
    NonEvp(HashMap<String, String>),
}

impl EvpMsg {
    fn parse_connect_request(topic: &str, _payload: &str) -> Result<EvpMsg, DMError> {
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

        Ok(EvpMsg::ConnectMsg((who, req_id)))
    }

    fn parse_connect_response(topic: &str, _payload: &str) -> Result<EvpMsg, DMError> {
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

        Ok(EvpMsg::ConnectRespMsg((who, req_id)))
    }

    fn parse_state_msg(_topic: &str, payload: &str) -> Result<EvpMsg, DMError> {
        let v = json::parse(payload).map_err(|_| Report::new(DMError::InvalidData))?;
        if let JsonValue::Object(o) = v {
            for (_k, v) in o.iter() {
                if let JsonValue::String(s) = v {
                    if let Ok(device_info) = DeviceInfo::parse(s) {
                        return Ok(EvpMsg::DeviceInfoMsg(device_info));
                    }
                }
            }
        }

        Err(Report::new(DMError::InvalidData))
    }

    fn parse_config_msg(topic: &str, payload: &str) -> Result<EvpMsg, DMError> {
        let mut hash = HashMap::new();
        hash.insert(topic.to_owned(), payload.to_owned());
        Ok(EvpMsg::ServerMsg(hash))
    }

    pub fn parse(topic: &str, payload: &str) -> Result<EvpMsg, DMError> {
        let mut hash = HashMap::new();
        hash.insert(topic.to_owned(), payload.to_owned());

        // "v1/devices/me/attributes"
        // https://thingsboard.io/docs/reference/mqtt-api/#subscribe-to-attribute-updates-from-the-server
        if let Ok(_) = EvpParser::parse(Rule::attribute_common, topic) {
            // "v1/devices/me/attributes/request"
            if let Ok(msg) = EvpMsg::parse_connect_request(topic, payload) {
                return Ok(msg);
            }

            // "v1/devices/me/attributes/response"
            if let Ok(msg) = EvpMsg::parse_connect_response(topic, payload) {
                return Ok(msg);
            }

            // "v1/devices/me/attributes"
            if let Ok(msg) = EvpMsg::parse_state_msg(topic, payload) {
                return Ok(msg);
            }

            // "v1/devices/me/attributes"
            if let Ok(msg) = EvpMsg::parse_config_msg(topic, payload) {
                return Ok(msg);
            }
        }

        // https://thingsboard.io/docs/reference/mqtt-api/#request-attribute-values-from-the-server
        if let Ok(_) = EvpParser::parse(Rule::server_attr_common, topic) {}

        //"v1/devices/me/rpc/request/
        // https://thingsboard.io/docs/reference/mqtt-api/#server-side-rpc
        if let Ok(_) = EvpParser::parse(Rule::server_rpc_common, topic) {
            return Ok(EvpMsg::RpcServer(hash));
        }

        // https://thingsboard.io/docs/reference/mqtt-api/#client-side-rpc
        if let Ok(_) = EvpParser::parse(Rule::client_rpc_common, topic) {
            return Ok(EvpMsg::RpcClient(hash));
        }

        Ok(EvpMsg::NonEvp(hash))
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
            EvpMsg::ConnectMsg(("me".to_owned(), 1000))
        );
    }

    #[test]
    fn test_parse_01() {
        let topic = "v1/devices/me/attributes/request/1000";
        let payload = "";

        assert_eq!(
            EvpMsg::parse(topic, payload).unwrap(),
            EvpMsg::ConnectMsg(("me".to_owned(), 1000))
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
            EvpMsg::ClientMsg(expected)
        );
    }
}
