#[allow(unused)]
use {
    super::ReqId,
    super::ResInfo,
    crate::error::DMError,
    crate::mqtt_ctrl::evp::JsonUtility,
    error_stack::{Report, Result, ResultExt},
    json::JsonValue,
    json::object::Object,
    serde::{Deserialize, Serialize},
    std::collections::{BTreeMap, HashMap},
    std::fmt::Display,
};

/// image is only used for the `dire_get_image` RPC call
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Default)]
pub struct RpcResInfo {
    pub res_info: ResInfo,
    pub image: Option<String>,
}

impl Display for RpcResInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut res_info = Object::new();
        if let Some(res_id) = &self.res_info.res_id {
            res_info.insert("res_id", res_id.clone().into());
        }

        res_info.insert("code", self.res_info.code.into());
        res_info.insert("detail_msg", self.res_info.detail_msg.clone().into());

        let mut root = Object::new();
        root.insert("res_info", res_info.into());

        if let Some(image) = &self.image {
            root.insert("image", image.clone().into());
        }

        write!(f, "{}", json::stringify_pretty(root, 4))
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

    #[test]
    fn test_parse_rpc_response_with_image() {
        let response = r#"{"direct-command-response":{"status":"ok","reqid":"1","response":"{\"res_info\":{\"code\":0,\"detail_msg\":\"ok\"},\"image\":\"data\"}"}}"#;

        let parsed = parse_rpc_response(response).expect("should parse");
        assert_eq!(parsed.image, Some("data".to_string()));
        assert_eq!(parsed.res_info.code, 0);
    }

    #[test]
    fn test_parse_rpc_response_invalid() {
        // Missing expected top-level key
        let response = r#"{"foo":"bar"}"#;
        assert!(parse_rpc_response(response).is_err());

        // direct-command-response exists but response field is not a string
        let response = r#"{"direct-command-response":{"response": {"not":"a string"}}}"#;
        assert!(parse_rpc_response(response).is_err());
    }

    #[test]
    fn test_rpc_response_and_display_roundtrip() {
        let response = r#"{"direct-command-response":{"status":"ok","reqid":"2","response":"{\"res_info\":{\"code\":0,\"detail_msg\":\"ok\"},\"image\":\"imgdata\"}"}}"#;
        let parsed = parse_rpc_response(response).expect("parse");
        // Ensure Display for RpcResInfo contains the expected fields
        let s = format!("{}", parsed);
        assert!(s.contains("\"code\""));
        assert!(s.contains("\"detail_msg\""));
        assert!(s.contains("\"image\""));

        // RpcResponse Display serializes the RpcResInfo via serde_json
        let rpc_resp = RpcResponse {
            response: parsed.clone(),
        };
        let s2 = format!("{}", rpc_resp);
        assert!(s2.contains("res_info"));
        assert!(s2.contains("detail_msg"));
    }
}
