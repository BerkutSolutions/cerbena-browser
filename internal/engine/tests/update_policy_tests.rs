use browser_engine::{EngineUpdateArtifact, EngineUpdatePolicy, EngineUpdateService, UpdateMode};

#[test]
fn update_disabled_by_default_blocks_apply() {
    let svc = EngineUpdateService;
    let policy = EngineUpdatePolicy::default();
    let artifact = EngineUpdateArtifact {
        version: "1.2.1".to_string(),
        signature: "sig".to_string(),
    };
    assert!(svc.verify_and_apply(&policy, &artifact, "sig").is_err());
}

#[test]
fn manual_update_requires_valid_signature() {
    let svc = EngineUpdateService;
    let policy = EngineUpdatePolicy {
        mode: UpdateMode::Manual,
        allow_user_enable: true,
    };
    let artifact = EngineUpdateArtifact {
        version: "1.2.1".to_string(),
        signature: "sig-ok".to_string(),
    };
    assert!(svc.verify_and_apply(&policy, &artifact, "sig-ok").is_ok());
    assert!(svc.verify_and_apply(&policy, &artifact, "bad").is_err());
}
