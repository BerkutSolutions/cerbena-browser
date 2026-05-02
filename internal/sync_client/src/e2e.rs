use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use browser_profile::crypto::{decrypt_blob, encrypt_blob, EncryptedBlob};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

const WRAP_VERSION_V1: u32 = 1;
const WRAP_VERSION_LEGACY: u32 = 0;
const WRAP_NONCE_LEN: usize = 12;
const DATA_KEY_LEN: usize = 32;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncKeyMaterial {
    pub profile_id: Uuid,
    pub key_id: String,
    pub wrapping_secret: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct E2EEnvelope {
    pub key_id: String,
    #[serde(default)]
    pub wrap_version: u32,
    #[serde(default)]
    pub wrap_nonce_b64: String,
    pub wrapped_data_key_b64: String,
    pub encrypted_payload: EncryptedBlob,
}

#[derive(Debug, Error)]
pub enum E2EError {
    #[error("crypto: {0}")]
    Crypto(String),
    #[error("key id mismatch")]
    KeyMismatch,
    #[error("unsupported wrap version")]
    UnsupportedWrapVersion,
}

pub fn encrypt_sync_payload(
    key: &SyncKeyMaterial,
    plaintext: &[u8],
) -> Result<E2EEnvelope, E2EError> {
    let mut data_key = [0u8; DATA_KEY_LEN];
    rand::thread_rng().fill_bytes(&mut data_key);
    let data_key_secret = B64.encode(data_key);
    let encrypted_payload = encrypt_blob(&key.profile_id.to_string(), &data_key_secret, plaintext)
        .map_err(|e| E2EError::Crypto(e.to_string()))?;
    let wrapped = wrap_data_key(key, &data_key)?;

    Ok(E2EEnvelope {
        key_id: key.key_id.clone(),
        wrap_version: WRAP_VERSION_V1,
        wrap_nonce_b64: B64.encode(wrapped.nonce),
        wrapped_data_key_b64: B64.encode(wrapped.ciphertext),
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
    if envelope.wrap_version == WRAP_VERSION_LEGACY {
        return decrypt_blob(
            &key.profile_id.to_string(),
            &key.wrapping_secret,
            &envelope.encrypted_payload,
        )
        .map_err(|e| E2EError::Crypto(e.to_string()));
    }
    if envelope.wrap_version != WRAP_VERSION_V1 {
        return Err(E2EError::UnsupportedWrapVersion);
    }

    let wrapped_data_key = unwrap_data_key(key, envelope)?;
    let data_key_secret = B64.encode(wrapped_data_key);
    decrypt_blob(
        &key.profile_id.to_string(),
        &data_key_secret,
        &envelope.encrypted_payload,
    )
    .map_err(|e| E2EError::Crypto(e.to_string()))
}

struct WrappedDataKey {
    nonce: [u8; WRAP_NONCE_LEN],
    ciphertext: Vec<u8>,
}

fn wrap_data_key(
    key: &SyncKeyMaterial,
    data_key: &[u8; DATA_KEY_LEN],
) -> Result<WrappedDataKey, E2EError> {
    let kek = derive_wrap_key(key);
    let cipher = Aes256Gcm::new_from_slice(&kek)
        .map_err(|e| E2EError::Crypto(format!("wrap cipher init failed: {e}")))?;
    let mut nonce = [0u8; WRAP_NONCE_LEN];
    rand::thread_rng().fill_bytes(&mut nonce);
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce), data_key.as_ref())
        .map_err(|e| E2EError::Crypto(format!("wrap encryption failed: {e}")))?;
    Ok(WrappedDataKey { nonce, ciphertext })
}

fn unwrap_data_key(key: &SyncKeyMaterial, envelope: &E2EEnvelope) -> Result<Vec<u8>, E2EError> {
    let nonce = B64
        .decode(&envelope.wrap_nonce_b64)
        .map_err(|e| E2EError::Crypto(format!("decode wrap nonce: {e}")))?;
    let ciphertext = B64
        .decode(&envelope.wrapped_data_key_b64)
        .map_err(|e| E2EError::Crypto(format!("decode wrapped key: {e}")))?;
    if nonce.len() != WRAP_NONCE_LEN {
        return Err(E2EError::Crypto(
            "invalid wrapped-key nonce length".to_string(),
        ));
    }

    let kek = derive_wrap_key(key);
    let cipher = Aes256Gcm::new_from_slice(&kek)
        .map_err(|e| E2EError::Crypto(format!("wrap cipher init failed: {e}")))?;
    cipher
        .decrypt(Nonce::from_slice(&nonce), ciphertext.as_ref())
        .map_err(|_| E2EError::Crypto("wrapped key authentication failed".to_string()))
}

fn derive_wrap_key(key: &SyncKeyMaterial) -> [u8; DATA_KEY_LEN] {
    let mut hasher = Sha256::new();
    hasher.update(b"sync-wrap-key-v1|");
    hasher.update(key.profile_id.as_bytes());
    hasher.update(b"|");
    hasher.update(key.key_id.as_bytes());
    hasher.update(b"|");
    hasher.update(key.wrapping_secret.as_bytes());
    let digest = hasher.finalize();
    let mut out = [0u8; DATA_KEY_LEN];
    out.copy_from_slice(&digest);
    out
}
