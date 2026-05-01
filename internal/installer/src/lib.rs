use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TargetOs {
    Windows,
    Macos,
    Linux,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallerArtifact {
    pub target: TargetOs,
    pub portable: bool,
    pub file_name: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedArtifact {
    pub file_name: String,
    pub sha256: String,
    pub signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SbomEntry {
    pub name: String,
    pub version: String,
    pub license: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VulnerabilityFinding {
    pub package: String,
    pub severity: String,
}

#[derive(Debug, Error)]
pub enum ReleaseGateError {
    #[error("signature mismatch")]
    SignatureMismatch,
    #[error("critical vulnerabilities found")]
    CriticalVulns,
}

#[derive(Debug, Default, Clone)]
pub struct InstallerPipeline;

impl InstallerPipeline {
    pub fn build_artifacts(&self, app_name: &str) -> Vec<InstallerArtifact> {
        vec![
            self.pack(app_name, TargetOs::Windows, false),
            self.pack(app_name, TargetOs::Windows, true),
            self.pack(app_name, TargetOs::Macos, false),
            self.pack(app_name, TargetOs::Linux, false),
        ]
    }

    pub fn sign(&self, artifact: &InstallerArtifact, signing_key_hint: &str) -> SignedArtifact {
        let sha = hex_sha256(&artifact.bytes);
        SignedArtifact {
            file_name: artifact.file_name.clone(),
            sha256: sha.clone(),
            signature: format!("sig:{}:{}", signing_key_hint, sha),
        }
    }

    pub fn verify_signature(
        &self,
        signed: &SignedArtifact,
        expected_key_hint: &str,
    ) -> Result<(), ReleaseGateError> {
        if signed
            .signature
            .starts_with(&format!("sig:{}:", expected_key_hint))
        {
            Ok(())
        } else {
            Err(ReleaseGateError::SignatureMismatch)
        }
    }

    pub fn generate_sbom(&self, deps: &[(String, String, String)]) -> Vec<SbomEntry> {
        deps.iter()
            .map(|(name, version, license)| SbomEntry {
                name: name.clone(),
                version: version.clone(),
                license: license.clone(),
            })
            .collect()
    }

    pub fn release_gate(&self, findings: &[VulnerabilityFinding]) -> Result<(), ReleaseGateError> {
        if findings
            .iter()
            .any(|f| f.severity.eq_ignore_ascii_case("critical"))
        {
            return Err(ReleaseGateError::CriticalVulns);
        }
        Ok(())
    }

    fn pack(&self, app_name: &str, target: TargetOs, portable: bool) -> InstallerArtifact {
        let os = match target {
            TargetOs::Windows => "windows",
            TargetOs::Macos => "macos",
            TargetOs::Linux => "linux",
        };
        let suffix = if portable { "-portable" } else { "" };
        let file_name = format!("{app_name}-{os}{suffix}.pkg");
        let bytes = format!("artifact:{app_name}:{os}:{portable}").into_bytes();
        InstallerArtifact {
            target,
            portable,
            file_name,
            bytes,
        }
    }
}

fn hex_sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}
