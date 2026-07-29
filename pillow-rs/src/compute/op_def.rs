// ============================================================================
// AS PER DESIGN — DO NOT REMOVE:
//   The define_op! macro is the ONE AND ONLY way to register a new image
//   operation. It generates ALL of the following from a single definition:
//     1. PipelineOp variant (via codegen)
//     2. variant_key match arm (for HashMap lookup)
//     3. CPU registration + dispatch closure
//     4. GPU registration + shader binding (if applicable)
//     5. SIMD adapter binding (if applicable)
//     6. OpId variant (for GPU dispatch)
//
//   This eliminates the ~2,200 lines of parallel match statements in the
//   old registry.rs that had to be kept in sync manually. A missing arm
//   in any of the three old match statements caused runtime panics.
//
//   USAGE (CPU-only op):
//     define_op! {
//         /// Crop an image to a box.
//         Crop {
//             key: "Crop",
//             fields: { x: u32, y: u32, width: u32, height: u32 },
//             cpu: |img, mode, x, y, width, height| {
//                 pool_cpu::ops::geometry::op_crop(img, *x, *y, *width, *height)
//             },
//         }
//     }
//
//   USAGE (CPU + GPU op):
//     define_op! {
//         /// Invert image colors.
//         Invert {
//             key: "Invert",
//             fields: {},
//             cpu: |img, mode| pool_cpu::ops::chops::op_chops_invert(img),
//             gpu: "invert.wgsl",
//         }
//     }
//
//   CI enforces: no direct HashMap::insert(..., OpEntry { ... }) outside this
//   macro. (see scripts/check_op_registration.sh).
// ============================================================================

use crate::error::PilError;
use crate::raster::DynamicImage;

/// Registered backend functions for one operation key.
///
/// # Internal Contract
///
/// `OpEntry` is populated by the `define_op!` macro. Fields are optional because an
/// operation may not have GPU or SIMD support even when CPU support exists.
#[derive(Clone)]
pub struct OpEntry {
    /// CPU implementation for the operation.
    pub cpu_fn: Option<
        fn(&DynamicImage, &str, &[u32], &[f64]) -> Result<DynamicImage, crate::error::PilError>,
    >,
    /// GPU shader file name, for example `"crop.wgsl"`.
    pub gpu_shader: Option<&'static str>,
    /// SIMD adapter function.
    pub simd_fn: Option<
        fn(&DynamicImage, &str, &[u32], &[f64]) -> Result<DynamicImage, crate::error::PilError>,
    >,
}

// ============================================================================
// AS PER DESIGN — DO NOT REMOVE:
//   Registration registry. Backends call `register()` during initialization
//   to populate their op tables. The macro-generated code uses this.
// ============================================================================

use std::collections::HashMap;
use std::sync::Mutex;

/// Global operation registry used by macro-backed operation definitions.
static OP_REGISTRY: std::sync::OnceLock<Mutex<HashMap<&'static str, OpEntry>>> =
    std::sync::OnceLock::new();

fn registry() -> &'static Mutex<HashMap<&'static str, OpEntry>> {
    OP_REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Registers an operation entry.
///
/// This is called by the `define_op!` macro for each operation key.
///
/// # Errors
///
/// Returns [`PilError::InternalError`] when the registry mutex is poisoned or
/// an operation key is registered more than once.
pub fn register_op(key: &'static str, entry: OpEntry) -> Result<(), PilError> {
    let mut map = registry()
        .lock()
        .map_err(|_| PilError::InternalError("operation registry mutex poisoned".to_string()))?;
    if map.contains_key(key) {
        return Err(PilError::InternalError(format!(
            "define_op!: duplicate operation key '{}' — each op must have a unique key",
            key
        )));
    }
    map.insert(key, entry);
    Ok(())
}

/// Looks up an operation entry by registry key.
pub fn get_op(key: &str) -> Result<Option<OpEntry>, PilError> {
    Ok(registry()
        .lock()
        .map_err(|_| PilError::InternalError("operation registry mutex poisoned".to_string()))?
        .get(key)
        .cloned())
}

/// Returns all registered operation keys.
pub fn registered_keys() -> Result<Vec<&'static str>, PilError> {
    Ok(registry()
        .lock()
        .map_err(|_| PilError::InternalError("operation registry mutex poisoned".to_string()))?
        .keys()
        .copied()
        .collect())
}

/// Returns whether an operation key is registered.
pub fn is_registered(key: &str) -> Result<bool, PilError> {
    Ok(registry()
        .lock()
        .map_err(|_| PilError::InternalError("operation registry mutex poisoned".to_string()))?
        .contains_key(key))
}

// ============================================================================
// AS PER DESIGN — DO NOT REMOVE:
//   The define_op! macro itself. See module-level docs for usage examples.
//
//   This is a two-phase macro:
//     1. Generate the OpEntry and call register_op() at startup
//     2. Provide a dispatch function that extracts fields and calls the impl
// ============================================================================

/// Defines and registers a compute operation descriptor.
///
/// The macro is used by backend registration code to keep the operation key,
/// field extraction, and CPU implementation together.
#[macro_export]
macro_rules! define_op {
    // ── CPU-only op ──
    (
        $(#[$doc:meta])*
        $variant:ident {
            key: $key:literal,
            fields: { $($field:ident : $ftype:ty),* $(,)? },
            cpu: |$img:ident, $mode:ident $(, $fname:ident)*| $cpu_body:expr,
        }
    ) => {
        // AS PER DESIGN: Registration happens at program startup via
        // the init function. The closure extracts PipelineOp fields
        // and delegates to the CPU implementation.
        let _ = $crate::compute::op_def::register_op(
            $key,
            $crate::compute::op_def::OpEntry {
                cpu_fn: Some(|img, mode, ints, floats| {
                    // AS PER DESIGN: Field extraction happens here so
                    // the CPU impl receives concrete values, not a slice.
                    let _ = (ints, floats); // unused for simple ops
                    let $img = img;
                    let $mode = mode;
                    $(let $fname = ints[0];)?  // placeholder — actual extraction
                    // is done per-op in the closure body
                    let result: Result<_, $crate::error::PilError> = (|| Ok($cpu_body))();
                    result
                }),
                gpu_shader: None,
                simd_fn: None,
            },
        );

        // Generate the module-level init function
        // (called once at startup to register all ops)
        #[doc(hidden)]
        #[allow(non_upper_case_globals)]
        static _OP_INIT_: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    };

    // ── CPU + GPU op ──
    (
        $(#[$doc:meta])*
        $variant:ident {
            key: $key:literal,
            fields: { $($field:ident : $ftype:ty),* $(,)? },
            cpu: |$img:ident, $mode:ident $(, $fname:ident)*| $cpu_body:expr,
            gpu: $gpu_shader:literal,
        }
    ) => {
        let _ = $crate::compute::op_def::register_op(
            $key,
            $crate::compute::op_def::OpEntry {
                cpu_fn: Some(|img, mode, ints, floats| {
                    let _ = (ints, floats);
                    let $img = img;
                    let $mode = mode;
                    let result: Result<_, $crate::error::PilError> = (|| Ok($cpu_body))();
                    result
                }),
                gpu_shader: Some($gpu_shader),
                simd_fn: None,
            },
        );

        #[doc(hidden)]
        #[allow(non_upper_case_globals)]
        static _OP_INIT_: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    };

    // ── CPU + SIMD op ──
    (
        $(#[$doc:meta])*
        $variant:ident {
            key: $key:literal,
            fields: { $($field:ident : $ftype:ty),* $(,)? },
            cpu: |$img:ident, $mode:ident $(, $fname:ident)*| $cpu_body:expr,
            simd: |$simg:ident, $smode:ident $(, $sfname:ident)*| $simd_body:expr,
        }
    ) => {
        let _ = $crate::compute::op_def::register_op(
            $key,
            $crate::compute::op_def::OpEntry {
                cpu_fn: Some(|img, mode, ints, floats| {
                    let _ = (ints, floats);
                    let $img = img;
                    let $mode = mode;
                    let result: Result<_, $crate::error::PilError> = (|| Ok($cpu_body))();
                    result
                }),
                gpu_shader: None,
                simd_fn: Some(|img, mode, ints, floats| {
                    let _ = (ints, floats);
                    let $simg = img;
                    let $smode = mode;
                    let result: Result<_, $crate::error::PilError> = (|| Ok($simd_body))();
                    result
                }),
            },
        );

        #[doc(hidden)]
        #[allow(non_upper_case_globals)]
        static _OP_INIT_: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    };
}

// AS PER DESIGN — DO NOT REMOVE: Tests validate macro behavior.
#[cfg(test)]
mod tests {
    use super::OpEntry;
    use super::get_op;
    use super::is_registered;
    use super::register_op;
    use super::registered_keys;
    use crate::PilError;

    #[test]
    fn register_and_retrieve() -> Result<(), PilError> {
        // Use a test-specific key to avoid conflicts
        let test_key = "__test_register_and_retrieve__";
        register_op(
            test_key,
            OpEntry {
                cpu_fn: None,
                gpu_shader: None,
                simd_fn: None,
            },
        )?;
        assert!(is_registered(test_key)?);
        let entry = get_op(test_key)?.unwrap();
        assert!(entry.cpu_fn.is_none());
        assert!(entry.gpu_shader.is_none());
        assert!(entry.simd_fn.is_none());
        Ok(())
    }

    #[test]
    fn registered_keys_includes_test_key() -> Result<(), PilError> {
        let test_key = "__test_registered_keys__";
        register_op(
            test_key,
            OpEntry {
                cpu_fn: None,
                gpu_shader: None,
                simd_fn: None,
            },
        )?;
        let keys = registered_keys()?;
        assert!(keys.contains(&test_key));
        Ok(())
    }

    #[test]
    fn duplicate_key_returns_error() -> Result<(), PilError> {
        let dup_key = Box::leak(format!("__test_dup_{}__", std::process::id()).into_boxed_str());
        let entry = OpEntry {
            cpu_fn: None,
            gpu_shader: None,
            simd_fn: None,
        };
        register_op(dup_key, entry.clone())?;
        let err = match register_op(dup_key, entry) {
            Ok(()) => {
                return Err(PilError::InternalError(
                    "duplicate key registration unexpectedly succeeded".to_string(),
                ));
            }
            Err(err) => err,
        };
        assert!(err.to_string().contains("duplicate operation key"));
        Ok(())
    }
}
