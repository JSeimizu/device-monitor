#[allow(unused)]
use {
    super::ReqId,
    super::ResInfo,
    crate::error::DMError,
    error_stack::{Report, Result},
    json::JsonValue,
    serde::{Deserialize, Serialize},
    std::collections::{BTreeMap, HashMap},
    std::fmt::Display,
};

/// image is only used for the `dire_get_image` RPC call
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct RpcResInfo {
    res_info: ResInfo,
    image: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct RpcResponse {
    response: RpcResInfo,
}

impl Display for RpcResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let json = serde_json::to_string(&self.response).unwrap_or("Invalid JSON".to_string());
        write!(f, "{}", json)
    }
}
