use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use pbkdf2::pbkdf2_hmac;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::errors::ProfileError;

pub const CRYPTO_VERSION: u32 = 1;
const KDF_ROUNDS: u32 = 120_000;
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;
const KEY_LEN: usize = 32;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedBlob {
    pub crypto_version: u32,
    pub salt_b64: String,
    pub nonce_b64: String,
    pub ciphertext_b64: String,
}

pub fn derive_key(profile_id: &str, secret: &str, salt: &[u8]) -> [u8; KEY_LEN] {
    let mut key = [0u8; KEY_LEN];
    let mut material = Vec::with_capacity(profile_id.len() + secret.len() + 1);
    material.extend_from_slice(profile_id.as_bytes());
    material.push(b':');
    material.extend_from_slice(secret.as_bytes());
    pbkdf2_hmac::<Sha256>(&material, salt, KDF_ROUNDS, &mut key);
    key
}

pub fn encrypt_blob(
    profile_id: &str,
    secret: &str,
    plaintext: &[u8],
) -> Result<EncryptedBlob, ProfileError> {
    let mut salt = [0u8; SALT_LEN];
    rand::thread_rng().fill_bytes(&mut salt);
    let key = derive_key(profile_id, secret, &salt);
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| ProfileError::Crypto(format!("cipher init failed: {e}")))?;

    let mut nonce = [0u8; NONCE_LEN];
    rand::thread_rng().fill_bytes(&mut nonce);

    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce), plaintext)
        .map_err(|e| ProfileError::Crypto(format!("encryption failed: {e}")))?;

    Ok(EncryptedBlob {
        crypto_version: CRYPTO_VERSION,
        salt_b64: B64.encode(salt),
        nonce_b64: B64.encode(nonce),
        ciphertext_b64: B64.encode(ciphertext),
    })
}

pub fn decrypt_blob(
    profile_id: &str,
    secret: &str,
    blob: &EncryptedBlob,
) -> Result<Vec<u8>, ProfileError> {
    if blob.crypto_version != CRYPTO_VERSION {
        return Err(ProfileError::Crypto(format!(
            "unsupported crypto_version: {}",
            blob.crypto_version
        )));
    }
    let salt = B64
        .decode(&blob.salt_b64)
        .map_err(|e| ProfileError::Crypto(format!("invalid salt: {e}")))?;
    let nonce = B64
        .decode(&blob.nonce_b64)
        .map_err(|e| ProfileError::Crypto(format!("invalid nonce: {e}")))?;
    let ciphertext = B64
        .decode(&blob.ciphertext_b64)
        .map_err(|e| ProfileError::Crypto(format!("invalid ciphertext: {e}")))?;

    if salt.len() != SALT_LEN || nonce.len() != NONCE_LEN {
        return Err(ProfileError::Crypto(
            "invalid salt/nonce length".to_string(),
        ));
    }

    let key = derive_key(profile_id, secret, &salt);
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| ProfileError::Crypto(format!("cipher init failed: {e}")))?;
    cipher
        .decrypt(Nonce::from_slice(&nonce), ciphertext.as_ref())
        .map_err(|_| ProfileError::Crypto("decrypt failed (wrong key or data)".to_string()))
}
