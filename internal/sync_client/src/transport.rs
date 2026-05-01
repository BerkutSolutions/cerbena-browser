use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsPolicy {
    pub min_version: String,
    pub certificate_pinning: bool,
    pub allowed_fingerprints: Vec<String>,
}

impl Default for TlsPolicy {
    fn default() -> Self {
        Self {
            min_version: "TLS1.3".to_string(),
            certificate_pinning: false,
            allowed_fingerprints: Vec::new(),
        }
    }
}

#[derive(Debug, Error)]
pub enum TransportError {
    #[error("tls version too weak")]
    WeakTls,
    #[error("certificate pin mismatch")]
    PinMismatch,
    #[error("manifest signature mismatch")]
    SignatureMismatch,
    #[error("replay blocked")]
    ReplayBlocked,
}

#[derive(Debug, Default, Clone)]
pub struct ManifestVerifier;

impl ManifestVerifier {
    pub fn verify(
        &self,
        expected_signature: &str,
        actual_signature: &str,
    ) -> Result<(), TransportError> {
        if expected_signature == actual_signature {
            Ok(())
        } else {
            Err(TransportError::SignatureMismatch)
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct TransportGuard {
    last_nonce: Option<String>,
}

impl TransportGuard {
    pub fn enforce_tls(
        &self,
        policy: &TlsPolicy,
        actual_tls_version: &str,
    ) -> Result<(), TransportError> {
        if actual_tls_version < policy.min_version.as_str() {
            return Err(TransportError::WeakTls);
        }
        Ok(())
    }

    pub fn enforce_pinning(
        &self,
        policy: &TlsPolicy,
        fingerprint: &str,
    ) -> Result<(), TransportError> {
        if !policy.certificate_pinning {
            return Ok(());
        }
        if policy.allowed_fingerprints.iter().any(|v| v == fingerprint) {
            Ok(())
        } else {
            Err(TransportError::PinMismatch)
        }
    }

    pub fn enforce_no_replay(&mut self, nonce: &str) -> Result<(), TransportError> {
        if self.last_nonce.as_deref() == Some(nonce) {
            return Err(TransportError::ReplayBlocked);
        }
        self.last_nonce = Some(nonce.to_string());
        Ok(())
    }
}
