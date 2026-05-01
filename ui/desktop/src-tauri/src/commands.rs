use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UiEnvelope<T>
where
    T: Serialize,
{
    pub ok: bool,
    pub message_key: &'static str,
    pub correlation_id: String,
    pub data: T,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapData {
    pub app_name: String,
    pub stage: String,
    pub locale: String,
}

#[tauri::command]
pub fn bootstrap(locale: String, correlation_id: String) -> UiEnvelope<BootstrapData> {
    UiEnvelope {
        ok: true,
        message_key: "command.success",
        correlation_id,
        data: BootstrapData {
            app_name: "Cerbena".to_owned(),
            stage: "U0".to_owned(),
            locale,
        },
    }
}

#[tauri::command]
pub fn health(correlation_id: String) -> UiEnvelope<&'static str> {
    UiEnvelope {
        ok: true,
        message_key: "command.success",
        correlation_id,
        data: "ok",
    }
}
