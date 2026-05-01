use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use browser_profile::{
    crypto::{decrypt_blob, encrypt_blob, EncryptedBlob},
    ProfileMetadata,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportFile {
    pub relative_path: String,
    pub content_b64: String,
    pub sha256_hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileArchivePayload {
    pub schema_version: u32,
    pub profile_id: Uuid,
    pub metadata: ProfileMetadata,
    pub files: Vec<ExportFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedProfileArchive {
    pub schema_version: u32,
    pub encrypted: EncryptedBlob,
}

#[derive(Debug, Error)]
pub enum ImportExportError {
    #[error("profile mismatch")]
    ProfileMismatch,
    #[error("unsupported schema version: {0}")]
    UnsupportedSchemaVersion(u32),
    #[error("integrity check failed for: {0}")]
    IntegrityFailed(String),
    #[error("serialization: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("crypto: {0}")]
    Crypto(String),
}

pub fn export_profile_archive(
    metadata: &ProfileMetadata,
    files: Vec<(String, Vec<u8>)>,
    passphrase: &str,
) -> Result<EncryptedProfileArchive, ImportExportError> {
    let prepared = files
        .into_iter()
        .map(|(relative_path, bytes)| ExportFile {
            relative_path,
            sha256_hex: hash_hex(&bytes),
            content_b64: B64.encode(bytes),
        })
        .collect::<Vec<_>>();

    let payload = ProfileArchivePayload {
        schema_version: SCHEMA_VERSION,
        profile_id: metadata.id,
        metadata: metadata.clone(),
        files: prepared,
    };
    let data = serde_json::to_vec(&payload)?;
    let encrypted = encrypt_blob(&metadata.id.to_string(), passphrase, &data)
        .map_err(|e| ImportExportError::Crypto(e.to_string()))?;
    Ok(EncryptedProfileArchive {
        schema_version: SCHEMA_VERSION,
        encrypted,
    })
}

pub fn import_profile_archive(
    archive: &EncryptedProfileArchive,
    expected_profile_id: Uuid,
    passphrase: &str,
) -> Result<ProfileArchivePayload, ImportExportError> {
    if archive.schema_version != SCHEMA_VERSION {
        return Err(ImportExportError::UnsupportedSchemaVersion(
            archive.schema_version,
        ));
    }
    let plain = decrypt_blob(
        &expected_profile_id.to_string(),
        passphrase,
        &archive.encrypted,
    )
    .map_err(|e| ImportExportError::Crypto(e.to_string()))?;
    let payload: ProfileArchivePayload = serde_json::from_slice(&plain)?;
    if payload.schema_version != SCHEMA_VERSION {
        return Err(ImportExportError::UnsupportedSchemaVersion(
            payload.schema_version,
        ));
    }
    if payload.profile_id != expected_profile_id {
        return Err(ImportExportError::ProfileMismatch);
    }
    for file in &payload.files {
        let content = B64
            .decode(&file.content_b64)
            .map_err(|_| ImportExportError::IntegrityFailed(file.relative_path.clone()))?;
        let digest = hash_hex(&content);
        if digest != file.sha256_hex {
            return Err(ImportExportError::IntegrityFailed(
                file.relative_path.clone(),
            ));
        }
    }
    Ok(payload)
}

fn hash_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}
