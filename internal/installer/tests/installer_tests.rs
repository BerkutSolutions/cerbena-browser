use browser_installer::{InstallerPipeline, ReleaseGateError, VulnerabilityFinding};

#[test]
fn builds_signs_and_verifies_artifacts() {
    let pipeline = InstallerPipeline;
    let artifacts = pipeline.build_artifacts("browser");
    assert!(artifacts.len() >= 4);
    let signed = pipeline.sign(&artifacts[0], "release-key");
    assert!(pipeline.verify_signature(&signed, "release-key").is_ok());
}

#[test]
fn generates_sbom_and_blocks_critical_vulns() {
    let pipeline = InstallerPipeline;
    let sbom = pipeline.generate_sbom(&[
        ("serde".to_string(), "1.0".to_string(), "MIT".to_string()),
        ("sha2".to_string(), "0.10".to_string(), "MIT".to_string()),
    ]);
    assert_eq!(sbom.len(), 2);
    let findings = vec![VulnerabilityFinding {
        package: "x".to_string(),
        severity: "critical".to_string(),
    }];
    let gate = pipeline.release_gate(&findings);
    assert!(matches!(gate, Err(ReleaseGateError::CriticalVulns)));
}
