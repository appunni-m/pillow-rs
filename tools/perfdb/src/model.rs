use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, Default)]
pub struct Observation {
    pub source_kind: String,
    pub record_kind: String,
    pub raw: Value,
    pub workload: Option<String>,
    pub operation: Option<String>,
    pub scope: Option<String>,
    pub operation_class: Option<String>,
    pub subject: Option<String>,
    pub profile: Option<String>,
    pub baseline: Option<String>,
    pub comparison_group: Option<String>,
    pub run_id: Option<String>,
    pub revision: Option<String>,
    pub measured_at: Option<String>,
    pub runner_id: Option<String>,
    pub runner_signature: Option<String>,
    pub workload_signature: Option<String>,
    pub mode: Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub chain_length: Option<i64>,
    pub boundary: Option<String>,
    pub sample_unit: Option<String>,
    pub sample_count: Option<i64>,
    pub median_us: Option<f64>,
    pub p95_us: Option<f64>,
    pub throughput_ops_s: Option<f64>,
    pub verified_speedup_vs_pillow: Option<f64>,
    pub verified_speedup_vs_pillow_simd: Option<f64>,
    pub verified_gpu_latency_speedup_vs_simd: Option<f64>,
    pub gpu_sustained_throughput_status: Option<String>,
    pub status: Option<String>,
    pub correctness: Option<String>,
    pub evidence_status: Option<String>,
    pub requested_backend: Option<String>,
    pub actual_backend: Option<String>,
    pub terminal_complete: Option<bool>,
    pub fallback_reasons: Option<String>,
    pub policy: Option<Value>,
    pub context: Option<Value>,
    pub resource: Option<Value>,
}

impl Observation {
    pub fn observation_id(&self) -> anyhow::Result<String> {
        let mut hasher = Sha256::new();
        hasher.update(self.source_kind.as_bytes());
        hasher.update([0]);
        hasher.update(self.record_kind.as_bytes());
        hasher.update([0]);
        hasher.update(serde_json::to_vec(&self.raw)?);
        Ok(format!("{:x}", hasher.finalize()))
    }

    pub fn row_json(&self) -> anyhow::Result<String> {
        Ok(serde_json::to_string(&self.raw)?)
    }
}

pub fn canonical_object(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let sorted: Map<String, Value> = map
                .iter()
                .map(|(key, value)| (key.clone(), canonical_object(value)))
                .collect();
            Value::Object(sorted)
        }
        Value::Array(values) => Value::Array(values.iter().map(canonical_object).collect()),
        value => value.clone(),
    }
}

pub fn signature(value: &Value) -> String {
    let canonical = canonical_object(value);
    let bytes = serde_json::to_vec(&canonical).expect("JSON values always serialize");
    format!("{:x}", Sha256::digest(bytes))
}

pub fn string(value: &Value, key: &str) -> Option<String> {
    let item = value.get(key)?;
    if let Some(text) = item.as_str() {
        (!text.is_empty()).then(|| text.to_owned())
    } else if item.is_number() || item.is_boolean() {
        Some(item.to_string())
    } else {
        None
    }
}

pub fn parse_i64(value: &Value, key: &str) -> Option<i64> {
    let item = value.get(key)?;
    item.as_i64().or_else(|| item.as_str()?.parse().ok())
}

pub fn parse_f64(value: &Value, key: &str) -> Option<f64> {
    let item = value.get(key)?;
    item.as_f64().or_else(|| item.as_str()?.parse().ok())
}

pub fn parse_bool(value: &Value, key: &str) -> Option<bool> {
    let item = value.get(key)?;
    item.as_bool().or_else(|| match item.as_str()? {
        "true" | "True" | "1" => Some(true),
        "false" | "False" | "0" => Some(false),
        _ => None,
    })
}

pub fn array_size(value: &Value, key: &str) -> (Option<i64>, Option<i64>) {
    let Some(array) = value.get(key).and_then(Value::as_array) else {
        return (None, None);
    };
    let width = array.first().and_then(Value::as_i64);
    let height = array.get(1).and_then(Value::as_i64);
    (width, height)
}

pub fn split_context(value: &Value) -> (Option<String>, Option<i64>, Option<i64>, Option<i64>) {
    let (width, height) = array_size(value, "size");
    (
        string(value, "mode"),
        width,
        height,
        parse_i64(value, "chain_length"),
    )
}

pub struct WorkloadDefinition<'a> {
    pub workload: &'a Option<String>,
    pub mode: &'a Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub chain_length: Option<i64>,
    pub boundary: &'a Option<String>,
    pub sample_unit: &'a Option<String>,
    pub policy: &'a Option<Value>,
    pub context: &'a Option<Value>,
    pub input_hashes: Option<&'a Value>,
}

pub fn workload_signature(definition: &WorkloadDefinition<'_>) -> Option<String> {
    let workload = definition.workload.as_ref()?;
    let mut signature_value = json!({
        "workload": workload,
        "mode": definition.mode,
        "width": definition.width,
        "height": definition.height,
        "chain_length": definition.chain_length,
        "boundary": definition.boundary,
        "sample_unit": definition.sample_unit,
        "policy": definition.policy,
        "context": definition.context,
    });
    if let Some(input_hashes) = definition.input_hashes {
        signature_value["input_hashes"] = canonical_object(input_hashes);
    }
    Some(signature(&signature_value))
}

pub fn clean_optional_text(value: Option<String>) -> Option<String> {
    value.filter(|text| !text.trim().is_empty())
}
