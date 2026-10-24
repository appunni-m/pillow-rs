use super::super::super::{aligned_bytes, gpu_working_set_bytes};
use super::{Stage, planned_channels};
use crate::Image;
use crate::checked_dims::CheckedDims;
use crate::error::PilError;

pub(super) struct ResourceUsage {
    pub(super) gpu_bytes: u64,
    pub(super) host_bytes: u64,
    pub(super) metadata_bytes: u64,
}

pub(super) fn account(
    image: &Image,
    encoded_bytes: u64,
    source_bytes: usize,
    stages: &[Stage<'_>],
    dimensions: (u32, u32),
    mode: &str,
    capacity: u32,
) -> Result<ResourceUsage, PilError> {
    // Exact main workspace plus conservative stage arenas. Encoders can
    // generate coefficient and LUT tables, so reserve them before allocation.
    let resource_bytes = stages.iter().try_fold(0u64, |total, stage| {
        total
            .checked_add(stage.arena_bytes)
            .ok_or_else(|| PilError::ValueError("GPU graph resource byte count overflow".into()))
    })?;
    let length =
        CheckedDims::new(dimensions.0, dimensions.1, planned_channels(mode)?)?.total_bytes();
    let staging = aligned_bytes(length, 8) as u64;
    let mut gpu_bytes =
        gpu_working_set_bytes(capacity) + source_bytes as u64 + resource_bytes * 2 + staging;
    let metadata_bytes = image.gpu_metadata_bytes();
    let mut host_bytes = source_bytes as u64 * 3
        + encoded_bytes
        + staging * 2
        + resource_bytes
        + metadata_bytes
        + stages.len() as u64 * std::mem::size_of::<Stage<'_>>() as u64;
    for child in stages.iter().flat_map(|stage| {
        [stage.secondary.as_deref(), stage.tertiary.as_deref()]
            .into_iter()
            .flatten()
    }) {
        gpu_bytes = gpu_bytes
            .checked_add(child.gpu_bytes)
            .ok_or_else(|| PilError::ValueError("GPU graph byte count overflow".into()))?;
        host_bytes = host_bytes
            .checked_add(child.host_bytes)
            .ok_or_else(|| PilError::ValueError("GPU graph byte count overflow".into()))?;
    }
    Ok(ResourceUsage {
        gpu_bytes,
        host_bytes,
        metadata_bytes,
    })
}
