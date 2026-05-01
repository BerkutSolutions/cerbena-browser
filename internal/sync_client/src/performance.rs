use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceBudget {
    pub startup_ms_max: u64,
    pub profile_launch_ms_max: u64,
    pub memory_per_profile_mb_max: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMeasurement {
    pub startup_ms: u64,
    pub profile_launch_ms: u64,
    pub memory_per_profile_mb: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RegressionCheck {
    pub startup_regression: bool,
    pub launch_regression: bool,
    pub memory_regression: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationPlan {
    pub hotspots: Vec<String>,
    pub actions: Vec<String>,
    pub projected_gain_percent: u8,
    pub security_preserved: bool,
}

#[derive(Debug, Default, Clone)]
pub struct PerformanceProfiler;

impl PerformanceProfiler {
    pub fn check_budget(
        &self,
        budget: &PerformanceBudget,
        m: &PerformanceMeasurement,
    ) -> RegressionCheck {
        RegressionCheck {
            startup_regression: m.startup_ms > budget.startup_ms_max,
            launch_regression: m.profile_launch_ms > budget.profile_launch_ms_max,
            memory_regression: m.memory_per_profile_mb > budget.memory_per_profile_mb_max,
        }
    }

    pub fn build_optimization_plan(
        &self,
        baseline: &PerformanceMeasurement,
        current: &PerformanceMeasurement,
    ) -> OptimizationPlan {
        let mut hotspots = Vec::new();
        let mut actions = Vec::new();
        if current.startup_ms > baseline.startup_ms {
            hotspots.push("startup_path".to_string());
            actions.push("reduce startup I/O and lazy-load non-critical modules".to_string());
        }
        if current.profile_launch_ms > baseline.profile_launch_ms {
            hotspots.push("profile_launch".to_string());
            actions.push(
                "cache resolved launch arguments and pre-validate profile policies".to_string(),
            );
        }
        if current.memory_per_profile_mb > baseline.memory_per_profile_mb {
            hotspots.push("memory_pressure".to_string());
            actions.push("shrink transient allocations and reuse buffers".to_string());
        }
        let projected_gain_percent = if hotspots.is_empty() { 0 } else { 15 };
        OptimizationPlan {
            hotspots,
            actions,
            projected_gain_percent,
            security_preserved: true,
        }
    }
}
