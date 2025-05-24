#[allow(unused)]
use {
    super::ReqId,
    super::ResInfo,
    crate::error::DMError,
    crate::mqtt_ctrl::evp::JsonUtility,
    error_stack::{Report, Result, ResultExt},
    json::JsonValue,
    serde::{Deserialize, Serialize},
    std::collections::{BTreeMap, HashMap},
    std::fmt::Display,
};

/// image is only used for the `dire_get_image` RPC call
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Default)]
pub struct RpcResInfo {
    res_info: ResInfo,
    image: Option<String>,
}

impl Display for RpcResInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let json = serde_json::to_string(&self.res_info).unwrap_or("Invalid JSON".to_string());
        write!(f, "{}", json)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Default)]
pub struct RpcResponse {
    response: RpcResInfo,
}

impl Display for RpcResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let json = serde_json::to_string(&self.response).unwrap_or("Invalid JSON".to_string());
        write!(f, "{}", json)
    }
}

pub fn parse_rpc_response(response: &str) -> Result<RpcResInfo, DMError> {
    json::parse(response)
        .ok()
        .and_then(|o| {
            if let JsonValue::Object(o) = o {
                Some(o)
            } else {
                None
            }
        })
        .and_then(|obj| obj.get("direct-command-response").cloned())
        .and_then(|obj| {
            if let JsonValue::Object(o) = obj {
                Some(o)
            } else {
                None
            }
        })
        .and_then(|obj| obj.get("response").cloned())
        .and_then(|o| {
            if let JsonValue::String(s) = o {
                serde_json::from_str::<RpcResInfo>(&s).ok()
            } else {
                None
            }
        })
        .ok_or_else(|| Report::new(DMError::InvalidData))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_rpc_response() {
        let response = r#"{"direct-command-response":{"status":"ok","reqid":"1000","response":"{\"res_info\":{\"code\":0,\"detail_msg\":\"ok\"}}"}}"#;

        let parsed = parse_rpc_response(response);
        assert!(parsed.is_ok());
    }
}
