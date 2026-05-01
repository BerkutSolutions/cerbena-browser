use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use pbkdf2::pbkdf2_hmac;
use rand::RngCore;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};

const SENSITIVE_STORE_CRYPTO_VERSION: u32 = 1;
const SENSITIVE_STORE_KDF_ROUNDS: u32 = 120_000;
const SENSITIVE_STORE_SALT_LEN: usize = 16;
const SENSITIVE_STORE_NONCE_LEN: usize = 12;
const SENSITIVE_STORE_KEY_LEN: usize = 32;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SensitiveStoreEnvelope {
    pub crypto_version: u32,
    pub scope: String,
    pub salt_b64: String,
    pub nonce_b64: String,
    pub ciphertext_b64: String,
    pub secret_fingerprint: String,
}

pub fn derive_app_secret_material(
    app_data_dir: &Path,
    current_exe: &Path,
    identifier: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(identifier.trim().as_bytes());
    hasher.update(b"|");
    hasher.update(app_data_dir.to_string_lossy().as_bytes());
    hasher.update(b"|");
    hasher.update(current_exe.to_string_lossy().as_bytes());
    hasher.update(b"|");
    hasher.update(std::env::var("USERNAME").unwrap_or_default().trim().as_bytes());
    hasher.update(b"|");
    hasher.update(std::env::var("USERDOMAIN").unwrap_or_default().trim().as_bytes());
    hasher.update(b"|");
    hasher.update(
        std::env::var("COMPUTERNAME")
            .unwrap_or_default()
            .trim()
            .as_bytes(),
    );
    hasher.update(b"|");
    hasher.update(
        std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .unwrap_or_default()
            .trim()
            .as_bytes(),
    );
    hex_string(&hasher.finalize())
}

pub fn load_sensitive_json_or_default<T>(
    path: &Path,
    scope: &str,
    secret_material: &str,
) -> Result<T, String>
where
    T: DeserializeOwned + Default,
{
    if !path.exists() {
        return Ok(T::default());
    }
    let raw = fs::read(path).map_err(|e| format!("read sensitive store {}: {e}", path.display()))?;
    if let Ok(envelope) = serde_json::from_slice::<SensitiveStoreEnvelope>(&raw) {
        return match decrypt_envelope_json(&envelope, scope, secret_material) {
            Ok(value) => Ok(value),
            Err(error) if should_reset_sensitive_store(&error) => {
                let _ = backup_incompatible_sensitive_store(path);
                Ok(T::default())
            }
            Err(error) => Err(error),
        };
    }
    serde_json::from_slice::<T>(&raw)
        .map_err(|e| format!("parse legacy plaintext store {}: {e}", path.display()))
}

pub fn persist_sensitive_json<T>(
    path: &Path,
    scope: &str,
    secret_material: &str,
    value: &T,
) -> Result<(), String>
where
    T: Serialize,
{
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("create sensitive store dir {}: {e}", parent.display()))?;
    }
    let mut plaintext =
        serde_json::to_vec_pretty(value).map_err(|e| format!("serialize sensitive store: {e}"))?;
    let envelope = encrypt_envelope(scope, secret_material, &plaintext)?;
    plaintext.fill(0);
    let bytes = serde_json::to_vec_pretty(&envelope)
        .map_err(|e| format!("serialize sensitive envelope: {e}"))?;
    fs::write(path, bytes).map_err(|e| format!("write sensitive store {}: {e}", path.display()))
}

fn encrypt_envelope(
    scope: &str,
    secret_material: &str,
    plaintext: &[u8],
) -> Result<SensitiveStoreEnvelope, String> {
    let mut salt = [0u8; SENSITIVE_STORE_SALT_LEN];
    rand::thread_rng().fill_bytes(&mut salt);
    let key = derive_store_key(scope, secret_material, &salt);
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| format!("sensitive cipher init failed: {e}"))?;

    let mut nonce = [0u8; SENSITIVE_STORE_NONCE_LEN];
    rand::thread_rng().fill_bytes(&mut nonce);
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce), plaintext)
        .map_err(|e| format!("sensitive encryption failed: {e}"))?;

    Ok(SensitiveStoreEnvelope {
        crypto_version: SENSITIVE_STORE_CRYPTO_VERSION,
        scope: scope.to_string(),
        salt_b64: B64.encode(salt),
        nonce_b64: B64.encode(nonce),
        ciphertext_b64: B64.encode(ciphertext),
        secret_fingerprint: secret_fingerprint(secret_material),
    })
}

fn decrypt_envelope_json<T>(
    envelope: &SensitiveStoreEnvelope,
    scope: &str,
    secret_material: &str,
) -> Result<T, String>
where
    T: DeserializeOwned,
{
    let mut plaintext = decrypt_envelope(envelope, scope, secret_material)?;
    let parsed = serde_json::from_slice::<T>(&plaintext)
        .map_err(|e| format!("parse decrypted sensitive store: {e}"))?;
    plaintext.fill(0);
    Ok(parsed)
}

fn decrypt_envelope(
    envelope: &SensitiveStoreEnvelope,
    scope: &str,
    secret_material: &str,
) -> Result<Vec<u8>, String> {
    if envelope.crypto_version != SENSITIVE_STORE_CRYPTO_VERSION {
        return Err(format!(
            "unsupported sensitive store crypto version: {}",
            envelope.crypto_version
        ));
    }
    if envelope.scope != scope {
        return Err(format!(
            "sensitive store scope mismatch: expected {scope}, got {}",
            envelope.scope
        ));
    }
    let expected_fingerprint = secret_fingerprint(secret_material);
    if envelope.secret_fingerprint != expected_fingerprint {
        return Err("sensitive store secret fingerprint mismatch".to_string());
    }

    let salt = B64
        .decode(&envelope.salt_b64)
        .map_err(|e| format!("invalid sensitive salt: {e}"))?;
    let nonce = B64
        .decode(&envelope.nonce_b64)
        .map_err(|e| format!("invalid sensitive nonce: {e}"))?;
    let ciphertext = B64
        .decode(&envelope.ciphertext_b64)
        .map_err(|e| format!("invalid sensitive ciphertext: {e}"))?;
    if salt.len() != SENSITIVE_STORE_SALT_LEN || nonce.len() != SENSITIVE_STORE_NONCE_LEN {
        return Err("invalid sensitive store salt/nonce length".to_string());
    }

    let key = derive_store_key(scope, secret_material, &salt);
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| format!("sensitive cipher init failed: {e}"))?;
    cipher
        .decrypt(Nonce::from_slice(&nonce), ciphertext.as_ref())
        .map_err(|_| "failed to decrypt sensitive store".to_string())
}

fn should_reset_sensitive_store(error: &str) -> bool {
    matches!(
        error.trim(),
        "sensitive store secret fingerprint mismatch" | "failed to decrypt sensitive store"
    )
}

fn backup_incompatible_sensitive_store(path: &Path) -> Result<(), String> {
    let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
        return Ok(());
    };
    let backup_name = format!(
        "{file_name}.incompatible-{}.bak",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|value| value.as_millis())
            .unwrap_or_default()
    );
    let backup_path = path
        .parent()
        .map(|parent| parent.join(&backup_name))
        .unwrap_or_else(|| PathBuf::from(&backup_name));
    fs::rename(path, &backup_path).map_err(|e| {
        format!(
            "backup incompatible sensitive store {} -> {}: {e}",
            path.display(),
            backup_path.display()
        )
    })
}

fn derive_store_key(scope: &str, secret_material: &str, salt: &[u8]) -> [u8; SENSITIVE_STORE_KEY_LEN] {
    let mut key = [0u8; SENSITIVE_STORE_KEY_LEN];
    let mut material = Vec::with_capacity(scope.len() + secret_material.len() + 1);
    material.extend_from_slice(scope.as_bytes());
    material.push(b':');
    material.extend_from_slice(secret_material.as_bytes());
    pbkdf2_hmac::<Sha256>(&material, salt, SENSITIVE_STORE_KDF_ROUNDS, &mut key);
    key
}

fn secret_fingerprint(secret_material: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(secret_material.as_bytes());
    hex_string(&hasher.finalize())
}

fn hex_string(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::{
        derive_app_secret_material, load_sensitive_json_or_default, persist_sensitive_json,
    };
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    #[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
    struct ExampleStore {
        value: String,
        enabled: bool,
    }

    fn temp_path(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("cerbena-{label}-{unique}.json"))
    }

    #[test]
    fn sensitive_store_roundtrip_encrypts_payload() {
        let path = temp_path("sensitive-roundtrip");
        let secret = derive_app_secret_material(
            Path::new("C:/tmp/app-data"),
            Path::new("C:/tmp/cerbena.exe"),
            "dev.cerbena.app",
        );
        let value = ExampleStore {
            value: "secret-value".to_string(),
            enabled: true,
        };

        persist_sensitive_json(&path, "scope-a", &secret, &value).expect("persist");
        let on_disk = fs::read_to_string(&path).expect("read");
        assert!(!on_disk.contains("secret-value"));

        let loaded =
            load_sensitive_json_or_default::<ExampleStore>(&path, "scope-a", &secret).expect("load");
        assert_eq!(loaded, value);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn sensitive_store_accepts_plaintext_legacy_json() {
        let path = temp_path("sensitive-legacy");
        fs::write(&path, r#"{"value":"legacy","enabled":true}"#).expect("write");
        let secret = derive_app_secret_material(
            Path::new("C:/tmp/app-data"),
            Path::new("C:/tmp/cerbena.exe"),
            "dev.cerbena.app",
        );
        let loaded =
            load_sensitive_json_or_default::<ExampleStore>(&path, "scope-b", &secret).expect("load");
        assert_eq!(
            loaded,
            ExampleStore {
                value: "legacy".to_string(),
                enabled: true,
            }
        );
        let _ = fs::remove_file(path);
    }

    #[test]
    fn sensitive_store_resets_to_default_after_secret_mismatch() {
        let path = temp_path("sensitive-mismatch");
        let original_secret = derive_app_secret_material(
            Path::new("C:/tmp/app-data"),
            Path::new("C:/tmp/cerbena-old.exe"),
            "dev.cerbena.app",
        );
        let next_secret = derive_app_secret_material(
            Path::new("C:/tmp/app-data"),
            Path::new("C:/tmp/cerbena-new.exe"),
            "dev.cerbena.app",
        );
        let value = ExampleStore {
            value: "secret-value".to_string(),
            enabled: true,
        };

        persist_sensitive_json(&path, "scope-c", &original_secret, &value).expect("persist");
        let loaded =
            load_sensitive_json_or_default::<ExampleStore>(&path, "scope-c", &next_secret)
                .expect("load default after mismatch");
        assert_eq!(loaded, ExampleStore::default());
        assert!(!path.exists(), "original incompatible store must be moved aside");

        let backup_exists = path
            .parent()
            .and_then(|parent| fs::read_dir(parent).ok())
            .map(|entries| {
                entries.flatten().any(|entry| {
                    entry
                        .file_name()
                        .to_string_lossy()
                        .starts_with("cerbena-sensitive-mismatch")
                })
            })
            .unwrap_or(false);
        assert!(backup_exists, "backup file should be created for incompatible store");

        if let Some(parent) = path.parent() {
            if let Ok(entries) = fs::read_dir(parent) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.starts_with("cerbena-sensitive-mismatch") {
                        let _ = fs::remove_file(entry.path());
                    }
                }
            }
        }
    }
}
