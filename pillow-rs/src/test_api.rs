//! Test-build-only controls and reports for deterministic target contracts.

use crate::error::PilError;
use crate::pipeline::PipelineOp;
use std::collections::BTreeMap;

/// Arm the allow-listed deterministic GPU batch fault for the next executor.
pub fn arm_fault(point: &str) -> Result<(), String> {
    match point {
        "gpu_batch.admission.device_failure" => {
            #[cfg(feature = "gpu")]
            {
                crate::compute::arm_gpu_batch_admission_failure();
                Ok(())
            }
            #[cfg(not(feature = "gpu"))]
            {
                Err("GPU batch fault requires the gpu feature".into())
            }
        }
        _ => Err(format!("unknown migration parity fault point: {point}")),
    }
}

/// Clear an armed fault before the next fixture-driven workflow.
pub fn clear_fault() {
    #[cfg(feature = "gpu")]
    crate::compute::clear_gpu_batch_test_fault();
}

/// Public executor capability summary for one registered operation.
#[cfg(feature = "test-api")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackendSupportEntry {
    /// CPU mode/architecture strategy entry marker.
    pub cpu_fn: Option<()>,
    /// GPU shader name when the operation has a shader.
    pub gpu_shader: Option<&'static str>,
    /// GPU shader source marker.
    pub gpu_source: Option<()>,
}

/// Returns the backend capability registry as explicit public summary data.
#[cfg(feature = "test-api")]
pub fn backend_support_registry() -> Result<BTreeMap<&'static str, BackendSupportEntry>, PilError> {
    Ok(crate::compute::registry::registry()?
        .iter()
        .map(|(name, entry)| {
            (
                *name,
                BackendSupportEntry {
                    cpu_fn: entry.cpu_fn.map(|_| ()),
                    gpu_shader: {
                        #[cfg(feature = "gpu")]
                        {
                            entry.gpu_shader
                        }
                        #[cfg(not(feature = "gpu"))]
                        {
                            None
                        }
                    },
                    gpu_source: {
                        #[cfg(feature = "gpu")]
                        {
                            entry.gpu_source.map(|_| ())
                        }
                        #[cfg(not(feature = "gpu"))]
                        {
                            None
                        }
                    },
                },
            )
        })
        .collect())
}

/// Returns whether CPU declares support for a pipeline operation.
#[cfg(feature = "test-api")]
pub fn backend_cpu_supports(op: &PipelineOp) -> Result<bool, PilError> {
    crate::compute::registry::cpu_supports(op)
}

/// Returns whether the legacy SIMD request alias has a CPU operation entry.
#[cfg(feature = "test-api")]
pub fn backend_simd_supports(op: &PipelineOp) -> Result<bool, PilError> {
    crate::compute::registry::cpu_supports(op)
}

/// Returns whether GPU declares support for a pipeline operation.
#[cfg(feature = "test-api")]
pub fn backend_gpu_supports(op: &PipelineOp) -> Result<bool, PilError> {
    #[cfg(feature = "gpu")]
    {
        crate::compute::registry::gpu_supports(op)
    }
    #[cfg(not(feature = "gpu"))]
    {
        let _ = op;
        Ok(false)
    }
}

/// Returns the backend support matrix as deterministic pretty JSON.
#[cfg(feature = "test-api")]
pub fn backend_support_matrix_json() -> Result<String, PilError> {
    let registry = backend_support_registry()?;
    let operations = registry
        .iter()
        .map(|(name, entry)| {
            format!(
                "{{\"operation\":\"{name}\",\"cpu\":{},\"gpu_shader\":{}}}",
                entry.cpu_fn.is_some(),
                entry.gpu_shader.is_some()
            )
        })
        .collect::<Vec<_>>();
    let counts = backend_support_matrix_counts(&registry);

    let operations_json = operations
        .iter()
        .map(|entry| entry.to_string())
        .collect::<Vec<_>>()
        .join(",\n    ");

    Ok(format!(
        concat!(
            "{{\n",
            "  \"totals\": {{\n",
            "    \"operations\": {operations},\n",
            "    \"cpu\": {cpu},\n",
            "    \"gpu_shader\": {gpu_shader}\n",
            "  }},\n",
            "  \"operations\": [\n",
            "    {operations_json}\n",
            "  ]\n",
            "}}"
        ),
        operations = registry.len(),
        cpu = counts.cpu,
        gpu_shader = counts.gpu_shader,
        operations_json = operations_json
    ))
}

#[cfg(feature = "test-api")]
struct BackendSupportMatrixCounts {
    cpu: usize,
    gpu_shader: usize,
}

#[cfg(feature = "test-api")]
fn backend_support_matrix_counts(
    registry: &BTreeMap<&'static str, BackendSupportEntry>,
) -> BackendSupportMatrixCounts {
    let cpu = registry
        .values()
        .filter(|entry| entry.cpu_fn.is_some())
        .count();
    let gpu_shader = registry
        .values()
        .filter(|entry| entry.gpu_shader.is_some())
        .count();

    BackendSupportMatrixCounts {
        cpu,
        gpu_shader,
    }
}
