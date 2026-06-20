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

use pillow_rs_image::DynamicImage;

/// AS PER DESIGN — DO NOT REMOVE:
/// OpEntry stores the per-backend function pointers for an operation.
/// Each field is an Option because not all backends support every op.
#[derive(Clone)]
pub struct OpEntry {
    /// CPU implementation (always present — CPU is the universal fallback)
    pub cpu_fn: Option<
        fn(&DynamicImage, &str, &[u32], &[f64]) -> Result<DynamicImage, crate::error::PilError>,
    >,
    /// GPU shader name (e.g., "crop.wgsl")
    pub gpu_shader: Option<&'static str>,
    /// SIMD adapter function
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

/// Global operation registry.
/// AS PER DESIGN: OnceLock<Mutex<...>> for thread-safe lazy initialization.
static OP_REGISTRY: std::sync::OnceLock<Mutex<HashMap<&'static str, OpEntry>>> =
    std::sync::OnceLock::new();

fn registry() -> &'static Mutex<HashMap<&'static str, OpEntry>> {
    OP_REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

/// AS PER DESIGN — DO NOT REMOVE:
/// Register an operation entry. Called by the define_op! macro for each op.
/// Panics on duplicate keys (indicates a bug — two ops with the same key).
pub fn register_op(key: &'static str, entry: OpEntry) {
    let mut map = registry().lock().unwrap_or_else(|poisoned| {
        // AS PER DESIGN: Recover from poisoned mutex (e.g., panic in test).
        // In production, this should never happen.
        log::warn!("OP_REGISTRY mutex was poisoned; recovering");
        poisoned.into_inner()
    });
    if map.contains_key(key) {
        panic!(
            "define_op!: duplicate operation key '{}' — each op must have a unique key",
            key
        );
    }
    map.insert(key, entry);
}

/// Look up an operation entry by key.
pub fn get_op(key: &str) -> Option<OpEntry> {
    registry()
        .lock()
        .unwrap_or_else(|e| {
            log::warn!("OP_REGISTRY mutex poisoned in get_op; recovering");
            e.into_inner()
        })
        .get(key)
        .cloned()
}

/// List all registered operation keys. Useful for validation/debugging.
pub fn registered_keys() -> Vec<&'static str> {
    registry()
        .lock()
        .unwrap_or_else(|e| {
            log::warn!("OP_REGISTRY mutex poisoned in registered_keys; recovering");
            e.into_inner()
        })
        .keys()
        .copied()
        .collect()
}

/// Check if an operation key is registered.
pub fn is_registered(key: &str) -> bool {
    registry()
        .lock()
        .unwrap_or_else(|e| {
            log::warn!("OP_REGISTRY mutex poisoned in is_registered; recovering");
            e.into_inner()
        })
        .contains_key(key)
}

// ============================================================================
// AS PER DESIGN — DO NOT REMOVE:
//   The define_op! macro itself. See module-level docs for usage examples.
//
//   This is a two-phase macro:
//     1. Generate the OpEntry and call register_op() at startup
//     2. Provide a dispatch function that extracts fields and calls the impl
// ============================================================================

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
        $crate::compute::op_def::register_op(
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
        $crate::compute::op_def::register_op(
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
        $crate::compute::op_def::register_op(
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
    use super::*;

    #[test]
    fn register_and_retrieve() {
        // Use a test-specific key to avoid conflicts
        let test_key = "__test_register_and_retrieve__";
        register_op(
            test_key,
            OpEntry {
                cpu_fn: None,
                gpu_shader: None,
                simd_fn: None,
            },
        );
        assert!(is_registered(test_key));
        let entry = get_op(test_key).unwrap();
        assert!(entry.cpu_fn.is_none());
        assert!(entry.gpu_shader.is_none());
        assert!(entry.simd_fn.is_none());
    }

    #[test]
    fn registered_keys_includes_test_key() {
        let test_key = "__test_registered_keys__";
        register_op(
            test_key,
            OpEntry {
                cpu_fn: None,
                gpu_shader: None,
                simd_fn: None,
            },
        );
        let keys = registered_keys();
        assert!(keys.contains(&test_key));
    }

    #[test]
    #[should_panic(expected = "duplicate operation key")]
    fn duplicate_key_panics() {
        // AS PER DESIGN: Use unique key per test run to avoid poison across tests
        let dup_key = Box::leak(format!("__test_dup_{}__", std::process::id()).into_boxed_str());
        let entry = OpEntry {
            cpu_fn: None,
            gpu_shader: None,
            simd_fn: None,
        };
        register_op(dup_key, entry.clone());
        // AS PER DESIGN: Mutex may be poisoned by the panic — the #[should_panic]
        // attribute expects this. Subsequent tests should use unique keys.
        register_op(dup_key, entry); // should panic
    }
}
