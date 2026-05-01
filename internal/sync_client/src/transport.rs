use std::collections::{HashSet, VecDeque};

use serde::{Deserialize, Serialize};
use thiserror::Error;

const NONCE_WINDOW_SIZE: usize = 32;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum TlsVersion {
    Tls12,
    Tls13,
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
    #[error("unsupported tls version")]
    UnsupportedTlsVersion,
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

#[derive(Debug, Clone)]
pub struct TransportGuard {
    recent_nonces: VecDeque<String>,
    recent_nonce_index: HashSet<String>,
}

impl Default for TransportGuard {
    fn default() -> Self {
        Self {
            recent_nonces: VecDeque::with_capacity(NONCE_WINDOW_SIZE),
            recent_nonce_index: HashSet::with_capacity(NONCE_WINDOW_SIZE),
        }
    }
}

impl TransportGuard {
    pub fn enforce_tls(
        &self,
        policy: &TlsPolicy,
        actual_tls_version: &str,
    ) -> Result<(), TransportError> {
        let actual = parse_tls_version(actual_tls_version)?;
        let minimum = parse_tls_version(&policy.min_version)?;
        if actual < minimum {
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
        if self.recent_nonce_index.contains(nonce) {
            return Err(TransportError::ReplayBlocked);
        }
        let nonce_owned = nonce.to_string();
        self.recent_nonce_index.insert(nonce_owned.clone());
        self.recent_nonces.push_back(nonce_owned);

        while self.recent_nonces.len() > NONCE_WINDOW_SIZE {
            if let Some(expired) = self.recent_nonces.pop_front() {
                self.recent_nonce_index.remove(&expired);
            }
        }
        Ok(())
    }
}

fn parse_tls_version(raw: &str) -> Result<TlsVersion, TransportError> {
    let normalized = raw.trim().to_ascii_uppercase().replace(' ', "");
    match normalized.as_str() {
        "TLS1.2" | "TLSV1.2" | "1.2" => Ok(TlsVersion::Tls12),
        "TLS1.3" | "TLSV1.3" | "1.3" => Ok(TlsVersion::Tls13),
        _ => Err(TransportError::UnsupportedTlsVersion),
    }
}
