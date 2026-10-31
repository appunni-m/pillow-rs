//! Operation registration, split into focused initializer modules.
//!
//! CPU callbacks prepare operation-owned mode and architecture strategies.
//! Keep each existing operation key paired with its GPU shader metadata.
use super::OpEntry;
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
        }
    };
}
#[cfg(not(feature = "gpu"))]
macro_rules! gpu_entry {
    ($f:expr, $shader:literal) => {
        $crate::compute::registry::OpEntry::cpu_only($f as $crate::compute::registry::CpuOpFn)
    };
}

mod chops;
mod color;
mod composite;
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
mod transform;

pub(super) fn register_all(entries: &mut Registry) -> Result<(), PilError> {
    geometry::register(entries)?;
    color::register(entries)?;
    filter::register(entries)?;
    imageops::register(entries)?;
    chops::register(entries)?;
    enhance::register(entries)?;
    effects::register(entries)?;
    composite::register(entries)?;
    module_fns::register(entries)?;
    transform::register(entries)?;
    mutating::register(entries)?;
    extract_band::register(entries)?;
    draw::register(entries)?;
    Ok(())
}
