//! GPU worker pool — implements BackendImpl for GPU compute backend.

macro_rules! gpu_log {
    ($($arg:tt)*) => {
        log::debug!(target: "compute::gpu", $($arg)*)
    };
}

// Uses wgpu for compute shader dispatch with packed u32 RGBA buffers
// (R|G<<8|B<<16|A<<24) and 16x16 workgroups.
// GPU init is lazy — happens on first `execute_batch` call.

use crate::checked_dims::CheckedDims;
use crate::compute::registry;
use crate::compute::{Backend, BackendImpl, PipelineResourceTelemetry};
use crate::error::PilError;
use crate::ops::pil_resize::{
    FilterCoeffs, FilterCoeffsF64, compact_resize_coeffs_to_source_span, filter_from_resample,
    i32_samples_from_le_bytes, luma16_resample_big_endian, luma16_resample_read,
    luma16_resample_write, precompute_coeffs, precompute_coeffs_boxed_for_filter,
    precompute_coeffs_f64, round_up,
};
use crate::pipeline::{
    ColorMode, PipelineOp, PixelMode, ResampleFilter, TransformMethod, TransposeMethod,
};
#[cfg(target_endian = "little")]
use crate::raster::RgbImage;
use crate::raster::{DynamicImage, GenericImageView, ImageBuffer, Luma, RgbaImage};
use std::collections::HashMap;
use std::num::NonZeroU64;
use std::sync::Arc;
use std::time::{Duration, Instant};
use wide::u8x16;

mod native_dispatch;
mod projection;
/// Explicit bounded GPU scheduling; ordinary image routing stays independent.
pub(crate) mod stream;
use native_dispatch::*;
mod resource_pool;
use resource_pool::*;
mod pipeline_cache;
use pipeline_cache::*;
mod device;
use device::GpuInner;
mod readback;
#[cfg(test)]
use readback::{
    GPU_POLL_BACKOFF, GPU_POLL_FAST_BACKOFF, GPU_POLL_FAST_RETRIES, la_from_rgba_transport,
    readback_poll_backoff, rgb_from_packed_readback, unpack_native_cover_packed_rows,
};
mod resize;
use resize::*;
mod geometry;
use geometry::*;
mod layout_admission;
use layout_admission::*;
mod native_input;
use native_input::*;
#[cfg(target_endian = "little")]
mod native_hsv_pad;
#[cfg(target_endian = "little")]
use native_hsv_pad::*;
mod mode;
use mode::*;
mod auxiliary_images;
use auxiliary_images::*;
mod point_ops;
use point_ops::*;
mod fusion;
use fusion::*;
mod native_paste;
use native_paste::*;
mod operation_support;
use operation_support::*;
mod operation_admission;
use operation_admission::*;
mod batch_encoding;
mod batch_execution;
use batch_execution::*;
mod batch_output;
mod batch_pipeline;
use batch_pipeline::ResolvedPipeline;
mod batch_segmentation;
use batch_output::*;
mod batch_upload;
use batch_upload::*;
mod batch_arena_upload;
use batch_arena_upload::*;
mod batch_resize_coefficients;
use batch_resize_coefficients::*;
mod batch_auxiliary_ranges;
use batch_auxiliary_ranges::*;
mod batch_operation_planning;
use batch_operation_planning::*;
mod batch_operations;
use batch_operations::*;
mod batch_parameters;
use batch_parameters::*;
mod batch_telemetry;
use batch_telemetry::*;
mod batch_preparation;
use batch_preparation::*;
mod batch_estimates;
use batch_estimates::*;
mod batch_preflight;
mod logical_mode_admission;
use batch_preflight::*;
mod batch_normalization;
use batch_normalization::*;
#[cfg(target_endian = "little")]
mod batch_native_routes;
#[cfg(target_endian = "little")]
mod native_composite;
#[cfg(target_endian = "little")]
mod rgb_merge;
#[cfg(target_endian = "little")]
use rgb_merge::*;
mod thumbnail;
use thumbnail::*;
mod transform;
use transform::*;

// ─── GpuPool ───────────────────────────────────────────────────────────────

/// GPU compute pool — wgpu-based compute shader dispatch.
///
/// Uses packed u32 RGBA and 16x16 workgroups. GPU is lazily initialized on the
/// first GPU capability query or execution. If wgpu cannot provide an adapter,
/// the capability query reports no support so automatic routing can select the
/// next host backend before any device buffer is created.
pub struct GpuPool;

/// Keep command submission bounded for very long lazy pipelines. A batch may
/// contain more operations; it is split into sequential submissions without
/// reading the image back between chunks.
const MAX_GPU_OPS_PER_SUBMISSION: usize = 256;

/// Bound transient per-submission auxiliary/parameter arenas. A single
/// operation larger than this limit is still allowed so valid large images are
/// not rejected merely because they need one large upload.
const MAX_GPU_RESOURCE_BYTES_PER_SUBMISSION: usize = 64 * 1024 * 1024;
// Four concurrent materializations should reuse their completed working sets
// on the next wave. A two-set cache repeatedly reallocated the other two;
// the independent byte cap still bounds retained device memory.
const MAX_RETAINED_GPU_WORKING_SETS: usize = 4;
const MAX_RETAINED_GPU_WORKING_BYTES: u64 = 128 * 1024 * 1024;
// Reusing a much larger working set for a tiny image materially increases
// Metal's command-buffer/readback cost. Keep a bounded amount of
// over-allocation while allowing the pool to reuse neighboring image sizes.
const MAX_GPU_BUFFER_REUSE_RATIO: u32 = 4;
const MAX_RETAINED_GPU_STAGING_BUFFERS: usize = 2;
const MAX_RETAINED_GPU_STAGING_BYTES: u64 = 64 * 1024 * 1024;
const MAX_GPU_AUXILIARY_CACHE_BYTES: usize = 64 * 1024 * 1024;
/// The histogram control passes use three 256-bin channels and retain one
/// extra 256-word tail so the existing clear shader can reset the complete
/// storage allocation without a host write between operations.
const GPU_HISTOGRAM_WORDS: usize = 1024;
const GPU_HISTOGRAM_BYTES: usize = GPU_HISTOGRAM_WORDS * std::mem::size_of::<u32>();

/// Dynamic shader loops must have a small, explicit upper bound. These limits
/// are deliberately stricter than the CPU implementation: an unsupported or
/// unusually large request is routed to CPU rather than being allowed to
/// monopolize a native GPU queue.
const MAX_GPU_BLUR_RADIUS: u32 = 64;
const MAX_GPU_FILTER_SIZE: u32 = 15;
const MAX_GPU_REDUCE_FACTOR: u32 = 64;
// Order-statistic filters use a bounded local insertion sort.  The estimate
// below is intentionally large enough for the reviewed 9x9/256x256 material
// workloads and the 3x3/1024x768 matrix case, while still rejecting unbounded
// large windows before a single device submission can monopolize the queue.
const MAX_GPU_SHADER_WORK_ITEMS: u64 = 2 * 1024 * 1024 * 1024;
const GPU_BUFFER_CAPACITY: u32 = 4096 * 4096;
#[cfg(target_endian = "little")]
const GPU_NATIVE_RGB_LUMA_MERGE_MODE: u32 = 0x4d524742;
// F affine admission compares the scalar source selection with the packed
// 16.16 walk once per destination pixel. Keep that proof bounded even when a
// caller constructs a large transform that the normal dimension checks would
// route to host semantic control.
const GPU_F_AFFINE_PROOF_MAX_PIXELS: usize = 1024 * 1024;
const MAX_GPU_SCALE_FIXED_POINT: f64 = u32::MAX as f64;

static GPU: std::sync::OnceLock<Result<GpuInner, PilError>> = std::sync::OnceLock::new();

impl GpuPool {
    fn ensure_init() -> Result<&'static GpuInner, PilError> {
        match GPU.get_or_init(GpuInner::new) {
            Ok(gpu) => Ok(gpu),
            Err(error) => Err(error.clone()),
        }
    }
}

// ─── BackendImpl ───────────────────────────────────────────────────────────

impl BackendImpl for GpuPool {
    fn name(&self) -> Backend {
        Backend::Gpu
    }

    fn priority(&self) -> u8 {
        100
    }

    fn supports(&self, op: &PipelineOp) -> Result<bool, PilError> {
        let healthy = match GPU.get() {
            Some(Ok(gpu)) => gpu.failure_detail().is_none(),
            Some(Err(_)) => false,
            None => Self::ensure_init().is_ok(),
        };
        Ok(healthy && Self::descriptor_supports(op)?)
    }

    fn execute_batch(
        &self,
        ops: &[PipelineOp],
        img: &DynamicImage,
        mode: Option<&str>,
    ) -> Result<DynamicImage, PilError> {
        self.execute_batch_with_policy(ops, img, mode, true)
    }

    fn execute_batch_strict(
        &self,
        ops: &[PipelineOp],
        img: &DynamicImage,
        mode: Option<&str>,
    ) -> Result<DynamicImage, PilError> {
        self.execute_batch_with_policy(ops, img, mode, false)
    }
}

impl GpuPool {
    /// Check operation parameters without initializing a device or executing
    /// pixels. Image-dependent proof checks still belong to the dispatch plan.
    pub(crate) fn descriptor_supports(op: &PipelineOp) -> Result<bool, PilError> {
        Ok(
            (gpu_operation_is_safe(op) || gpu_operation_requires_image_context(op))
                && registry::gpu_supports(op)?,
        )
    }
}

#[cfg(test)]
mod tests;
