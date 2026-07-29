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

use crate::raster::DynamicImage;

use crate::error::PilError;

/// Borrowed parameter data for one backend operation execution.
///
/// # Internal Contract
///
/// Parameters are borrowed slices so hot-path dispatch does not allocate while
/// routing operation metadata to CPU, SIMD, or GPU adapters.
#[derive(Debug, Clone)]
pub struct OpParams<'a> {
    /// Integer parameters such as dimensions, counts, and mode codes.
    pub ints: &'a [u32],
    /// Floating-point parameters such as scale factors and rotation angles.
    pub floats: &'a [f64],
}

impl<'a> OpParams<'a> {
    /// Empty parameter set for operations with no parameter block.
    pub const EMPTY: Self = Self {
        ints: &[],
        floats: &[],
    };

    /// Creates parameter data from integer values only.
    pub fn from_ints(ints: &'a [u32]) -> Self {
        Self { ints, floats: &[] }
    }
}

/// Capability and execution interface implemented by each compute backend.
///
/// # Internal Contract
///
/// Each backend owns its operation metadata and determines its own capabilities.
/// `supports` must check live capability, not only static registration, because
/// GPU pipelines and SIMD feature availability can vary at runtime.
pub trait BackendOp: Send + Sync {
    /// Returns a lowercase backend name such as `"cpu"`, `"gpu"`, or `"simd"`.
    fn backend_name(&self) -> &'static str;

    /// Returns routing priority; higher values are preferred.
    fn priority(&self) -> u8;

    /// Returns whether this backend can execute `op_key`.
    ///
    /// CPU backends normally return true for registered operations. GPU and
    /// SIMD backends should include runtime capability checks.
    fn supports(&self, op_key: &str) -> bool;

    /// Executes one operation on this backend.
    ///
    /// `explicit_mode` carries Pillow mode tags when [`DynamicImage`] cannot
    /// represent the logical mode directly.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when the operation is unsupported or execution
    /// fails.
    fn execute(
        &self,
        op_key: &str,
        img: &DynamicImage,
        params: &OpParams<'_>,
        explicit_mode: Option<&str>,
    ) -> Result<DynamicImage, PilError>;

    /// Returns whether this backend can execute an operation batch efficiently.
    ///
    /// The default requires every operation key to be supported. Backends with
    /// batch-level constraints can override this, for example to keep small
    /// images on CPU when transfer overhead dominates.
    fn supports_batch(&self, op_keys: &[&str], _pixel_count: u64) -> bool {
        if op_keys.is_empty() {
            return true;
        }
        // AS PER DESIGN: GPU backend overrides this to add a size threshold.
        // Small images run faster on CPU due to GPU upload/download overhead.
        op_keys.iter().all(|k| self.supports(k))
    }
}
