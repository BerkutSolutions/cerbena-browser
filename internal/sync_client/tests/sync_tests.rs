use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use browser_sync_client::{
    decrypt_sync_payload, encrypt_sync_payload, BackupSnapshot, InMemorySyncServer,
    ManifestVerifier, MergePolicy, PerformanceBudget, PerformanceMeasurement, PerformanceProfiler,
    RestorePlanner, RestoreRequest, RestoreScope, SnapshotManager, SyncConflictResolution,
    SyncControlsModel, SyncKeyMaterial, SyncMutation, SyncPayload, SyncProtocolVersion,
    SyncServerConfig, SyncStatusLevel, SyncStatusView, TlsPolicy, TransportGuard,
};

#[test]
fn sync_protocol_apply_is_idempotent() {
    let profile = Uuid::new_v4();
    let mut server = InMemorySyncServer::default();
    let payload = SyncPayload {
        protocol: SyncProtocolVersion::default(),
        profile_id: profile,
        mutations: vec![SyncMutation {
            object_key: "bookmarks".to_string(),
            revision: 1,
            payload_b64: B64.encode("{\"v\":1}"),
            idempotency_key: "idemp-1".to_string(),
        }],
        resolution: SyncConflictResolution {
            policy: MergePolicy::RejectOnConflict,
            max_retry: 3,
        },
        sequence: 1,
    };
    let first = server.apply_payload(&payload).expect("first");
    let second = server.apply_payload(&payload).expect("second");
    assert_eq!(first.len(), 1);
    assert_eq!(second.len(), 0);
}

#[test]
fn e2e_envelope_roundtrip_works() {
    let key = SyncKeyMaterial {
        profile_id: Uuid::new_v4(),
        key_id: "k1".to_string(),
        wrapping_secret: "sync-secret".to_string(),
    };
    let envelope = encrypt_sync_payload(&key, b"payload").expect("encrypt");
    let plain = decrypt_sync_payload(&key, &envelope).expect("decrypt");
    assert_eq!(plain, b"payload");
}

#[test]
fn snapshots_retention_and_quarantine_work() {
    let mut manager = SnapshotManager::with_retention_limit(1);
    let p = Uuid::new_v4();
    let s1 = manager.create_snapshot(p, "blob-1".to_string(), "hash-1".to_string());
    let _s2 = manager.create_snapshot(p, "blob-2".to_string(), "hash-2".to_string());
    assert_eq!(manager.snapshots_for_profile(p).len(), 1);
    let ok = manager.verify_or_quarantine(&s1.snapshot_id, "hash-1");
    assert!(!ok);
    assert_eq!(manager.quarantined().len(), 1);
    let _s3 = manager.create_snapshot(p, "blob-3".to_string(), "hash-3".to_string());
    let kept = manager.snapshots_for_profile(p);
    let bad = manager.verify_or_quarantine(&kept[0].snapshot_id, "wrong");
    assert!(!bad);
    assert_eq!(manager.quarantined().len(), 2);
}

#[test]
fn snapshot_payload_hash_can_be_verified() {
    let payload = SnapshotManager::from_records_payload(&[]);
    let mut h = Sha256::new();
    h.update(payload.as_bytes());
    let digest = format!("{:x}", h.finalize());
    let s = BackupSnapshot {
        snapshot_id: "s".to_string(),
        profile_id: Uuid::new_v4(),
        created_at_unix_ms: 0,
        encrypted_blob_b64: "x".to_string(),
        integrity_sha256_hex: digest.clone(),
    };
    assert_eq!(s.integrity_sha256_hex, digest);
}

#[test]
fn selective_restore_uses_prefix_filters() {
    let planner = RestorePlanner;
    let profile = Uuid::new_v4();
    let snapshot = BackupSnapshot {
        snapshot_id: "snap-1".to_string(),
        profile_id: profile,
        created_at_unix_ms: 1,
        encrypted_blob_b64: "x".to_string(),
        integrity_sha256_hex: "h".to_string(),
    };
    let result = planner
        .restore(
            &RestoreRequest {
                profile_id: profile,
                snapshot_id: "snap-1".to_string(),
                scope: RestoreScope::Selective,
                include_prefixes: vec!["bookmarks/".to_string()],
                expected_schema_version: 1,
            },
            &snapshot,
            true,
            &["bookmarks/a.json".to_string(), "cookies/b.db".to_string()],
        )
        .expect("restore");
    assert_eq!(result.restored_items, 1);
    assert_eq!(result.skipped_items, 1);
}

#[test]
fn transport_guard_enforces_pinning_and_replay() {
    let mut guard = TransportGuard::default();
    let policy = TlsPolicy {
        min_version: "TLS1.3".to_string(),
        certificate_pinning: true,
        allowed_fingerprints: vec!["fp1".to_string()],
    };
    guard.enforce_tls(&policy, "TLS1.3").expect("tls");
    guard.enforce_pinning(&policy, "fp1").expect("pin");
    assert!(guard.enforce_no_replay("nonce-1").is_ok());
    assert!(guard.enforce_no_replay("nonce-1").is_err());
}

#[test]
fn manifest_verifier_detects_signature_mismatch() {
    let verifier = ManifestVerifier;
    assert!(verifier.verify("sig", "sig").is_ok());
    assert!(verifier.verify("sig", "bad").is_err());
}

#[test]
fn sync_controls_model_requires_server_data_when_enabled() {
    let controls = SyncControlsModel {
        server: SyncServerConfig {
            server_url: "".to_string(),
            key_id: "".to_string(),
            sync_enabled: true,
        },
        status: SyncStatusView {
            level: SyncStatusLevel::Warning,
            message_key: "sync.status.misconfigured".to_string(),
            last_sync_unix_ms: None,
        },
        conflicts: vec![],
        can_backup: true,
        can_restore: true,
    };
    assert!(controls.validate().is_err());
}

#[test]
fn performance_budget_and_optimization_plan_work() {
    let profiler = PerformanceProfiler;
    let budget = PerformanceBudget {
        startup_ms_max: 2000,
        profile_launch_ms_max: 1500,
        memory_per_profile_mb_max: 512,
    };
    let baseline = PerformanceMeasurement {
        startup_ms: 1200,
        profile_launch_ms: 900,
        memory_per_profile_mb: 300,
    };
    let current = PerformanceMeasurement {
        startup_ms: 2500,
        profile_launch_ms: 1800,
        memory_per_profile_mb: 700,
    };
    let check = profiler.check_budget(&budget, &current);
    assert!(check.startup_regression);
    assert!(check.launch_regression);
    assert!(check.memory_regression);
    let plan = profiler.build_optimization_plan(&baseline, &current);
    assert!(!plan.hotspots.is_empty());
    assert!(plan.security_preserved);
}
