use std::collections::{BTreeMap, BTreeSet};

use browser_fingerprint::IdentityPreset;
use browser_network_policy::{DnsTabPayload, VpnProxyTabPayload};
use browser_sync_client::{BackupSnapshot, ConflictViewItem, SyncControlsModel};
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct IdentityStore {
    pub items: BTreeMap<String, IdentityPreset>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct NetworkStore {
    pub vpn_proxy: BTreeMap<String, VpnProxyTabPayload>,
    pub dns: BTreeMap<String, DnsTabPayload>,
    pub connection_templates: BTreeMap<String, ConnectionTemplate>,
    pub profile_template_selection: BTreeMap<String, String>,
    #[serde(default)]
    pub global_route_settings: NetworkGlobalRouteSettings,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkGlobalRouteSettings {
    pub global_vpn_enabled: bool,
    pub block_without_vpn: bool,
    pub default_template_id: Option<String>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct SyncStore {
    pub controls: BTreeMap<String, SyncControlsModel>,
    pub conflicts: BTreeMap<String, Vec<ConflictViewItem>>,
    pub snapshots: BTreeMap<String, Vec<BackupSnapshot>>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkRoutingStore {
    pub global_profile_id: Option<String>,
    #[serde(default)]
    pub type_bindings: BTreeMap<String, String>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionLibraryStore {
    #[serde(default)]
    pub auto_update_enabled: bool,
    pub items: BTreeMap<String, ExtensionLibraryItem>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct HiddenDefaultProfilesStore {
    #[serde(default)]
    pub names: BTreeSet<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionNode {
    pub id: String,
    pub connection_type: String,
    pub protocol: String,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub bridges: Option<String>,
    #[serde(default)]
    pub settings: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionTemplate {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub nodes: Vec<ConnectionNode>,
    #[serde(default)]
    pub connection_type: String,
    #[serde(default)]
    pub protocol: String,
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub bridges: Option<String>,
    pub updated_at_epoch_ms: u128,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionPackageVariant {
    pub engine_scope: String,
    pub version: String,
    pub source_kind: String,
    pub source_value: String,
    pub logo_url: Option<String>,
    pub store_url: Option<String>,
    #[serde(default)]
    pub package_path: Option<String>,
    #[serde(default)]
    pub package_file_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionLibraryItem {
    pub id: String,
    pub display_name: String,
    pub version: String,
    pub engine_scope: String,
    pub source_kind: String,
    pub source_value: String,
    pub logo_url: Option<String>,
    pub store_url: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub assigned_profile_ids: Vec<String>,
    #[serde(default)]
    pub auto_update_enabled: bool,
    #[serde(default)]
    pub preserve_on_panic_wipe: bool,
    #[serde(default)]
    pub protect_data_from_panic_wipe: bool,
    #[serde(default)]
    pub package_path: Option<String>,
    #[serde(default)]
    pub package_file_name: Option<String>,
    #[serde(default)]
    pub package_variants: Vec<ExtensionPackageVariant>,
}
