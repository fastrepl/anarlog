use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DataCollectionMode {
    Allow,
    Deny,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderSort {
    Price,
    Throughput,
    Latency,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderSortPartition {
    Model,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderSortConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub by: Option<ProviderSort>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub partition: Option<ProviderSortPartition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ProviderSortUnion {
    Simple(ProviderSort),
    Config(ProviderSortConfig),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Quantization {
    Int4,
    Int8,
    Fp4,
    Fp6,
    Fp8,
    Fp16,
    Bf16,
    Fp32,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PriceValue {
    Amount(f64),
    ModelRef(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaxPrice {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<PriceValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion: Option<PriceValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<PriceValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio: Option<PriceValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request: Option<PriceValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PercentileThresholds {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub p50: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub p75: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub p90: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub p99: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ThresholdValue {
    Number(f64),
    Percentiles(PercentileThresholds),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProviderPreferences {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_fallbacks: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub require_parameters: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_collection: Option<DataCollectionMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub only: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ignore: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort: Option<ProviderSortUnion>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quantizations: Option<Vec<Quantization>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_price: Option<MaxPrice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preferred_min_throughput: Option<ThresholdValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preferred_max_latency: Option<ThresholdValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zdr: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enforce_distillable_text: Option<bool>,
}
