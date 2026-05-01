use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use browser_profile::crypto::{decrypt_blob, encrypt_blob, EncryptedBlob};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncKeyMaterial {
    pub profile_id: Uuid,
    pub key_id: String,
    pub wrapping_secret: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct E2EEnvelope {
    pub key_id: String,
    pub wrapped_data_key_b64: String,
    pub encrypted_payload: EncryptedBlob,
}

#[derive(Debug, Error)]
pub enum E2EError {
    #[error("crypto: {0}")]
    Crypto(String),
    #[error("key id mismatch")]
    KeyMismatch,
}

pub fn encrypt_sync_payload(
    key: &SyncKeyMaterial,
    plaintext: &[u8],
) -> Result<E2EEnvelope, E2EError> {
    // Minimal model: wrapped key is deterministic from key id + profile id marker.
    let wrapped = format!("{}:{}", key.key_id, key.profile_id);
    let encrypted_payload =
        encrypt_blob(&key.profile_id.to_string(), &key.wrapping_secret, plaintext)
            .map_err(|e| E2EError::Crypto(e.to_string()))?;
    Ok(E2EEnvelope {
        key_id: key.key_id.clone(),
        wrapped_data_key_b64: B64.encode(wrapped),
        encrypted_payload,
    })
}

pub fn decrypt_sync_payload(
    key: &SyncKeyMaterial,
    envelope: &E2EEnvelope,
) -> Result<Vec<u8>, E2EError> {
    if envelope.key_id != key.key_id {
        return Err(E2EError::KeyMismatch);
    }
    decrypt_blob(
        &key.profile_id.to_string(),
        &key.wrapping_secret,
        &envelope.encrypted_payload,
    )
    .map_err(|e| E2EError::Crypto(e.to_string()))
}
