use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum VmStatus {
    Running,
    Stopped,
    Starting,
    Stopping,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vm {
    #[serde(rename = "vm_name")]
    pub name: String,
    pub image: String,
    pub status: VmStatus,
    #[serde(default)]
    pub region: String,
    #[serde(default)]
    pub region_display: String,
    #[serde(default)]
    pub https_url: Option<String>,
    #[serde(default)]
    pub shelley_url: Option<String>,
    #[serde(default)]
    pub ssh_dest: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct VmList {
    #[serde(default)]
    pub vms: Vec<Vm>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshKey {
    pub public_key: String,
    pub fingerprint: String,
    pub name: String,
    #[serde(default)]
    pub added_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub current: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct SshKeyList {
    #[serde(default)]
    pub ssh_keys: Vec<SshKey>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhoAmI {
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub region: String,
    #[serde(default)]
    pub region_display: String,
    #[serde(default)]
    pub ssh_keys: Vec<SshKey>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmStat {
    #[serde(default)]
    pub disk_used_gb: Option<f64>,
    #[serde(default)]
    pub disk_total_gb: Option<f64>,
    #[serde(default)]
    pub bandwidth_gb: Option<f64>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedApiKey {
    #[serde(default)]
    pub token: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub vm: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}
