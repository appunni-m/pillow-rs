//! Operation registration, split into focused initializer modules.

use super::{OpEntry, SimdOpFn};
use crate::error::PilError;
use std::collections::HashMap;

type Registry = HashMap<&'static str, OpEntry>;

#[cfg(feature = "gpu")]
macro_rules! gpu_entry {
    ($f:expr, $shader:literal) => {
        $crate::compute::registry::OpEntry {
            cpu_fn: Some($f as $crate::compute::registry::CpuOpFn),
            gpu_shader: Some($shader),
            gpu_source: Some(include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/compute/pool_gpu/shaders/",
                $shader
            ))),
            simd_fn: None,
        }
    };
}
#[cfg(not(feature = "gpu"))]
macro_rules! gpu_entry {
    ($f:expr, $shader:literal) => {
        $crate::compute::registry::OpEntry::cpu_only($f as $crate::compute::registry::CpuOpFn)
    };
}

macro_rules! draw_entry {
    ($entries:ident, $key:expr, $func:expr) => {
        $entries.insert($key, gpu_entry!($func, "draw.wgsl"));
    };
}

fn simd_set(entries: &mut Registry, key: &'static str, f: SimdOpFn) -> Result<(), PilError> {
    let entry = entries
        .get_mut(key)
        .ok_or_else(|| PilError::InternalError(format!("SIMD key not registered: {key}")))?;
    entry.simd_fn = Some(f);
    Ok(())
}

mod chops;
mod color;
mod draw;
mod effects;
mod enhance;
mod extract_band;
mod filter;
mod geometry;
mod imageops;
mod module_fns;
mod mutating;
mod prelude;
mod simd;
mod transform;

pub(super) fn register_all(entries: &mut Registry) -> Result<(), PilError> {
    geometry::register(entries)?;
    color::register(entries)?;
    filter::register(entries)?;
    imageops::register(entries)?;
    chops::register(entries)?;
    enhance::register(entries)?;
    effects::register(entries)?;
    module_fns::register(entries)?;
    transform::register(entries)?;
    mutating::register(entries)?;
    extract_band::register(entries)?;
    draw::register(entries)?;
    simd::register(entries)?;
    Ok(())
}
