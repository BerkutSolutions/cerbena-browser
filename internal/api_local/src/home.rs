use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HomeMetric {
    pub key: String,
    pub value: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HomeAction {
    pub action_id: String,
    pub label_key: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HomeDashboardModel {
    pub profile_id: Uuid,
    pub metrics: Vec<HomeMetric>,
    pub quick_actions: Vec<HomeAction>,
}

#[derive(Debug, Default, Clone)]
pub struct HomePageService;

impl HomePageService {
    pub fn build_dashboard(
        &self,
        profile_id: Uuid,
        dns_blocked: u64,
        tracker_blocked: u64,
        service_blocked: u64,
        profile_running: bool,
    ) -> HomeDashboardModel {
        HomeDashboardModel {
            profile_id,
            metrics: vec![
                HomeMetric {
                    key: "home.metric.dns_blocked".to_string(),
                    value: dns_blocked,
                },
                HomeMetric {
                    key: "home.metric.tracker_blocked".to_string(),
                    value: tracker_blocked,
                },
                HomeMetric {
                    key: "home.metric.service_blocked".to_string(),
                    value: service_blocked,
                },
            ],
            quick_actions: vec![
                HomeAction {
                    action_id: "profile.launch".to_string(),
                    label_key: "home.action.launch".to_string(),
                    enabled: !profile_running,
                },
                HomeAction {
                    action_id: "profile.stop".to_string(),
                    label_key: "home.action.stop".to_string(),
                    enabled: profile_running,
                },
                HomeAction {
                    action_id: "profile.panic_wipe".to_string(),
                    label_key: "home.action.panic_wipe".to_string(),
                    enabled: true,
                },
            ],
        }
    }
}
