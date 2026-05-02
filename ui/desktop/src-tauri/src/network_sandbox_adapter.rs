use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::network_sandbox::{
    ResolvedNetworkSandboxMode, ResolvedNetworkSandboxStrategy,
};
use crate::network_sandbox_container::{probe_container_runtime, ContainerSandboxRuntimeProbe};
use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkSandboxAdapterPlan {
    pub adapter_kind: String,
    pub runtime_kind: String,
    pub available: bool,
    pub requires_system_network_access: bool,
    pub max_helper_processes: u8,
    pub estimated_memory_mb: u16,
    pub active_sandboxes: u8,
    pub max_active_sandboxes: u8,
    pub supports_native_isolation: bool,
    pub reason: String,
}

pub fn resolve_adapter_plan_for_profile(
    state: &AppState,
    profile_id: Option<Uuid>,
    strategy: &ResolvedNetworkSandboxStrategy,
) -> NetworkSandboxAdapterPlan {
    let max_active_sandboxes = state
        .network_sandbox_store
        .lock()
        .ok()
        .map(|store| store.global.max_active_sandboxes.max(1))
        .unwrap_or(2);
    let probe = (strategy.mode == ResolvedNetworkSandboxMode::Container)
        .then(|| probe_container_runtime(state, profile_id));
    resolve_adapter_plan(strategy, probe.as_ref(), max_active_sandboxes)
}

pub fn resolve_adapter_plan(
    strategy: &ResolvedNetworkSandboxStrategy,
    container_probe: Option<&ContainerSandboxRuntimeProbe>,
    max_active_sandboxes: u8,
) -> NetworkSandboxAdapterPlan {
    match strategy.mode {
        ResolvedNetworkSandboxMode::IsolatedUserspace => NetworkSandboxAdapterPlan {
            adapter_kind: "userspace".to_string(),
            runtime_kind: "launcher-managed".to_string(),
            available: strategy.available,
            requires_system_network_access: false,
            max_helper_processes: 2,
            estimated_memory_mb: 96,
            active_sandboxes: 0,
            max_active_sandboxes,
            supports_native_isolation: false,
            reason: strategy.reason.clone(),
        },
        ResolvedNetworkSandboxMode::CompatibilityNative => NetworkSandboxAdapterPlan {
            adapter_kind: "compatibility-native".to_string(),
            runtime_kind: "windows-service".to_string(),
            available: strategy.available,
            requires_system_network_access: true,
            max_helper_processes: 1,
            estimated_memory_mb: 64,
            active_sandboxes: 0,
            max_active_sandboxes,
            supports_native_isolation: false,
            reason: strategy.reason.clone(),
        },
        ResolvedNetworkSandboxMode::Container => {
            let probe = container_probe.cloned().unwrap_or(ContainerSandboxRuntimeProbe {
                available: false,
                runtime_kind: "docker-desktop".to_string(),
                runtime_version: None,
                runtime_platform: None,
                active_sandboxes: 0,
                max_active_sandboxes,
                supports_native_isolation: true,
                reason: "Container sandbox runtime has not been probed yet".to_string(),
            });
            let available = strategy.available && probe.available;
            let reason = if !strategy.available {
                strategy.reason.clone()
            } else {
                probe.reason.clone()
            };
            NetworkSandboxAdapterPlan {
                adapter_kind: "container-vm".to_string(),
                runtime_kind: probe.runtime_kind,
                available,
                requires_system_network_access: false,
                max_helper_processes: 3,
                estimated_memory_mb: 160,
                active_sandboxes: probe.active_sandboxes,
                max_active_sandboxes: probe.max_active_sandboxes,
                supports_native_isolation: true,
                reason,
            }
        }
        ResolvedNetworkSandboxMode::Blocked => NetworkSandboxAdapterPlan {
            adapter_kind: "blocked".to_string(),
            runtime_kind: "policy-blocked".to_string(),
            available: false,
            requires_system_network_access: false,
            max_helper_processes: 0,
            estimated_memory_mb: 0,
            active_sandboxes: 0,
            max_active_sandboxes,
            supports_native_isolation: false,
            reason: strategy.reason.clone(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network_sandbox::{ResolvedNetworkSandboxMode, ResolvedNetworkSandboxStrategy};

    fn strategy(mode: ResolvedNetworkSandboxMode, available: bool) -> ResolvedNetworkSandboxStrategy {
        ResolvedNetworkSandboxStrategy {
            mode,
            requested_mode: "auto".to_string(),
            requires_native_backend: mode == ResolvedNetworkSandboxMode::CompatibilityNative,
            available,
            reason: "test".to_string(),
        }
    }

    #[test]
    fn userspace_adapter_budget_is_lightweight() {
        let plan = resolve_adapter_plan(&strategy(
            ResolvedNetworkSandboxMode::IsolatedUserspace,
            true,
        ), None, 2);
        assert_eq!(plan.adapter_kind, "userspace");
        assert!(plan.available);
        assert!(!plan.requires_system_network_access);
        assert!(plan.max_helper_processes <= 2);
    }

    #[test]
    fn container_adapter_reports_unavailable() {
        let plan = resolve_adapter_plan(
            &strategy(ResolvedNetworkSandboxMode::Container, false),
            None,
            2,
        );
        assert_eq!(plan.adapter_kind, "container-vm");
        assert!(!plan.available);
    }
}
