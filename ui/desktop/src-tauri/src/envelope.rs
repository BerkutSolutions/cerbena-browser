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

pub fn ok<T>(correlation_id: String, data: T) -> UiEnvelope<T>
where
    T: Serialize,
{
    UiEnvelope {
        ok: true,
        message_key: "command.success",
        correlation_id,
        data,
    }
}
