pub mod audit;
pub mod first_launch;
pub mod import_sources;
pub mod manager;
pub mod model;
pub mod policy;

pub use audit::{ExtensionAuditEntry, ExtensionAuditLog};
pub use first_launch::{ExtensionInstallResult, FirstLaunchInstaller};
pub use import_sources::{ImportSource, ImportSourceKind, SourceValidator};
pub use manager::ExtensionManager;
pub use model::{
    ExtensionImportState, ExtensionRecord, ExtensionStatus, ExtensionUpdatePolicy,
    ProfileExtensionState,
};
pub use policy::{ExtensionPolicyDecision, ExtensionPolicyEnforcer, OverrideGuardrails};
