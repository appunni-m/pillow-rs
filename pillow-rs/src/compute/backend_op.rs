// ============================================================================
// AS PER DESIGN — DO NOT REMOVE:
//   The BackendOp trait defines how each compute backend (CPU, GPU, SIMD)
//   provides operation implementations. This replaces the old design where
//   OpEntry carried gpu_shader/gpu_source fields for every op (even CPU-only
//   ones), and where supports() checked static registration rather than
//   live capability.
//
//   Each backend:
//   1. Implements BackendOp with its own metadata store
//   2. Has its own supports() that checks LIVE capability (compiled shaders,
//      available SIMD features, etc.)
//   3. Registers ops via the `define_op!` macro, not manual match statements
//
//   CI enforces: no gpu_shader/gpu_source fields on shared OpEntry.
//   (see scripts/check_backend_leaks.sh)
// ============================================================================

use pillow_rs_image::DynamicImage;

use crate::error::PilError;

/// Optional parameter data for an operation execution.
/// AS PER DESIGN: This is a slice, not a Vec — no allocation on every call.
#[derive(Debug, Clone)]
pub struct OpParams<'a> {
    /// Integer parameters (dimensions, counts, mode codes)
    pub ints: &'a [u32],
    /// Floating-point parameters (scale factors, rotation angles)
    pub floats: &'a [f64],
}

impl<'a> OpParams<'a> {
    /// Create empty params for ops with no parameters.
    pub const EMPTY: Self = Self {
        ints: &[],
        floats: &[],
    };

    /// Create params from integer slice only.
    pub fn from_ints(ints: &'a [u32]) -> Self {
        Self { ints, floats: &[] }
    }
}

/// AS PER DESIGN — DO NOT REMOVE:
/// Trait implemented by each compute backend (CpuBackend, GpuBackend, SimdBackend).
///
/// Each backend owns its operation metadata and determines its own capabilities.
/// Backends DO NOT share a single OpEntry struct — that was the old design that
/// leaked GPU details into CPU-only ops.
pub trait BackendOp: Send + Sync {
    /// Human-readable backend name, e.g., "cpu", "gpu", "simd".
    /// AS PER DESIGN: Must match the Backend enum variant name.
    fn backend_name(&self) -> &'static str;

    /// Priority: higher = preferred. CPU=0, SIMD=50, GPU=100.
    /// AS PER DESIGN: route() uses this to pick the best available backend.
    fn priority(&self) -> u8;

    /// Does this backend support the given operation?
    /// AS PER DESIGN: Must check LIVE capability, not static registration.
    ///   - CPU: always true (universal fallback)
    ///   - GPU: checks compiled pipeline cache, not shader source existence
    ///   - SIMD: checks CPU feature flags + adapter availability
    fn supports(&self, op_key: &str) -> bool;

    /// Execute one operation on this backend.
    ///
    /// # Arguments
    /// - `op_key`: operation key string (matches PipelineOp variant_key)
    /// - `img`: input image (may be mutated or replaced)
    /// - `params`: operation parameters
    /// - `mode`: optional explicit color mode override
    ///
    /// # Errors
    /// - `NotImplementedError` if the backend doesn't support this op
    fn execute(
        &self,
        op_key: &str,
        img: &DynamicImage,
        params: &OpParams<'_>,
        explicit_mode: Option<&str>,
    ) -> Result<DynamicImage, PilError>;

    /// AS PER DESIGN: Check whether this backend can accelerate a batch of
    /// operations. Default: true if all ops are individually supported.
    /// Override for backends that have batch-level constraints (e.g., GPU
    /// has overhead — skip for small images).
    fn supports_batch(&self, op_keys: &[&str], _pixel_count: u64) -> bool {
        if op_keys.is_empty() {
            return true;
        }
        // AS PER DESIGN: GPU backend overrides this to add a size threshold.
        // Small images run faster on CPU due to GPU upload/download overhead.
        op_keys.iter().all(|k| self.supports(k))
    }
}
