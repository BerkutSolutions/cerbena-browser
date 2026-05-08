pub mod artifact;
pub mod chromium;
pub mod contract;
pub mod librewolf;
pub mod progress;
pub mod registry;
pub mod runtime;
pub mod ungoogled_chromium;
pub mod update_policy;

pub use chromium::ChromiumAdapter;
pub use contract::{EngineAdapter, EngineError, EngineKind, LaunchPlan, LaunchRequest};
pub use librewolf::LibrewolfAdapter;
pub use progress::EngineDownloadProgress;
pub use runtime::{EngineInstallation, EngineRuntime};
pub use ungoogled_chromium::UngoogledChromiumAdapter;
pub use update_policy::{
    EngineUpdateArtifact, EngineUpdatePolicy, EngineUpdateService, UpdateMode,
};
