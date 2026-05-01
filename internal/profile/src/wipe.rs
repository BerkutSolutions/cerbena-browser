use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WipeDataType {
    Cookies,
    History,
    Passwords,
    Cache,
    ExtensionsStorage,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SelectiveWipeRequest {
    pub data_types: Vec<WipeDataType>,
    pub site_scopes: Vec<String>,
    pub retain_paths: Vec<String>,
}
