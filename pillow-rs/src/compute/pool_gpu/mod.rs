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
use std::sync::{Arc, Mutex};
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
mod readback;
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
mod batch_output;
mod batch_pipeline;
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

// Add/Subtract currently dispatch only the exact unit-divisor/integral-offset
// subset. Other valid public parameters are routed to CPU until the shader
// carries the full f64 contract without rounding differences.

/// `Maintain::Wait` can wait forever when a native device or driver wedges.
/// Poll with a short bounded backoff so the library remains responsive while
/// retaining a finite failure path for a wedged native device or driver.
const GPU_READBACK_TIMEOUT: Duration = Duration::from_secs(30);
/// Even large transfers can finish before the native scheduler's 1 ms timer
/// granularity. Give every read a bounded initial polling window; long jobs
/// return to the conservative backoff after a fixed retry count. Sixty-four
/// retries avoid ending this window just before a short compute frame
/// completes and paying a whole additional millisecond of scheduler delay.
const GPU_POLL_FAST_BACKOFF: Duration = Duration::from_micros(50);
const GPU_POLL_FAST_RETRIES: usize = 64;
const GPU_POLL_BACKOFF: Duration = Duration::from_millis(1);
/// Pillow's `ImagingLineBoxBlur{8,32}` in `src/libImaging/BoxBlur.c` uses a
/// normalized replicated-edge average, so a constant image remains constant
/// through every box pass (and therefore Pillow's three-pass Gaussian
/// lowering). Scanning larger frames can cost more than the six GPU passes it
/// replaces, so keep this exact lowering bounded to the benchmark-sized
/// working set where the launch savings are measurable.
const GPU_UNIFORM_BLUR_MAX_PIXELS: usize = 1024 * 1024;

fn readback_poll_backoff(
    fast_polling: bool,
    empty_polls: usize,
    now: Instant,
    deadline: Instant,
) -> Option<Duration> {
    let remaining = deadline.saturating_duration_since(now);
    (!remaining.is_zero()).then(|| {
        let backoff = if fast_polling && empty_polls <= GPU_POLL_FAST_RETRIES {
            GPU_POLL_FAST_BACKOFF
        } else {
            GPU_POLL_BACKOFF
        };
        backoff.min(remaining)
    })
}

fn gpu_uniform_blur_can_copy(ops: &[PipelineOp], image: &DynamicImage) -> bool {
    if ops.len() != 1
        || !matches!(
            ops[0],
            PipelineOp::BoxBlur { .. }
                | PipelineOp::BoxBlurXY { .. }
                | PipelineOp::GaussianBlur { .. }
        )
        || !gpu_operation_is_safe(&ops[0])
        || !gpu_image_layout_is_supported(image)
    {
        return false;
    }

    let Ok(dims) = CheckedDims::new(image.width(), image.height(), 1) else {
        return false;
    };
    let pixel_count = dims.total_pixels();
    if pixel_count == 0 || pixel_count > GPU_UNIFORM_BLUR_MAX_PIXELS {
        return false;
    }

    let channels = match image {
        DynamicImage::ImageLuma8(_) => 1,
        DynamicImage::ImageLumaA8(_) => 2,
        DynamicImage::ImageRgb8(_) => 3,
        DynamicImage::ImageRgba8(_) => 4,
        _ => return false,
    };
    let bytes = image.as_bytes();
    let Some(first) = bytes.get(..channels) else {
        return false;
    };
    bytes
        .chunks_exact(channels)
        .take(pixel_count)
        .all(|pixel| pixel == first)
}

fn gpu_working_set_bytes(capacity: u32) -> u64 {
    // The two full-size ping-pong buffers dominate this working set. The
    // optional-input fallbacks contain one word each and the LUT contains 256
    // words.
    u64::from(capacity)
        .saturating_mul(2)
        .saturating_mul(std::mem::size_of::<u32>() as u64)
        .saturating_add(2 * std::mem::size_of::<u32>() as u64)
        .saturating_add(256 * std::mem::size_of::<u32>() as u64)
        .saturating_add(GPU_HISTOGRAM_BYTES as u64)
}

fn compact_luma8_transfer_bytes(width: u32, height: u32) -> Result<u64, PilError> {
    let pixel_count = CheckedDims::new(width, height, 1)?.total_pixels();
    let transfer_bytes = pixel_count
        .div_ceil(std::mem::size_of::<u32>())
        .checked_mul(std::mem::size_of::<u32>())
        .ok_or_else(|| PilError::ValueError("GPU luma readback is too large".into()))?;
    u64::try_from(transfer_bytes)
        .map_err(|_| PilError::ValueError("GPU luma readback is too large".into()))
}

fn compact_native_channel_transfer_bytes(
    width: u32,
    height: u32,
    channels: u8,
) -> Result<u64, PilError> {
    let native_bytes = CheckedDims::new(width, height, channels)?.total_bytes();
    let transfer_bytes = native_bytes
        .div_ceil(std::mem::size_of::<u32>())
        .checked_mul(std::mem::size_of::<u32>())
        .ok_or_else(|| PilError::ValueError("GPU native channel transfer is too large".into()))?;
    u64::try_from(transfer_bytes)
        .map_err(|_| PilError::ValueError("GPU native channel transfer is too large".into()))
}

fn unpack_native_cover_packed_rows(
    width: u32,
    height: u32,
    channels: u8,
    dispatch: NativeCoverPackedOutputDispatch,
    packed: &[u8],
) -> Result<Vec<u8>, PilError> {
    let pixel_count = CheckedDims::new(width, height, channels)?.total_bytes();
    let row_words = usize::try_from(dispatch.words_per_row)
        .map_err(|_| PilError::ValueError("GPU Cover output row is too large".into()))?;
    let row_bytes = row_words
        .checked_mul(std::mem::size_of::<u32>())
        .ok_or_else(|| PilError::ValueError("GPU Cover output row is too large".into()))?;
    let expected_words = row_words
        .checked_mul(
            usize::try_from(height)
                .map_err(|_| PilError::ValueError("GPU Cover output height is too large".into()))?,
        )
        .ok_or_else(|| PilError::ValueError("GPU Cover output is too large".into()))?;
    let expected_transfer = expected_words
        .checked_mul(std::mem::size_of::<u32>())
        .ok_or_else(|| PilError::ValueError("GPU Cover output is too large".into()))?;
    let native_row_bytes = usize::try_from(width)
        .ok()
        .and_then(|width| width.checked_mul(usize::from(channels)))
        .ok_or_else(|| PilError::ValueError("GPU Cover output row is too large".into()))?;
    if dispatch.channels != channels
        || expected_words != dispatch.word_count as usize
        || expected_transfer as u64 != dispatch.transfer_bytes
        || packed.len() != expected_transfer
        || row_bytes < native_row_bytes
    {
        return Err(PilError::InternalError(
            "GPU packed Cover readback does not match its row layout".into(),
        ));
    }
    let mut samples = Vec::with_capacity(pixel_count);
    for row in 0..height as usize {
        let row_start = row
            .checked_mul(row_bytes)
            .ok_or_else(|| PilError::ValueError("GPU Cover output offset overflow".into()))?;
        let row_end = row_start
            .checked_add(native_row_bytes)
            .ok_or_else(|| PilError::ValueError("GPU Cover output offset overflow".into()))?;
        let row_pixels = packed.get(row_start..row_end).ok_or_else(|| {
            PilError::InternalError("GPU packed Cover readback ended mid-row".into())
        })?;
        samples.extend_from_slice(row_pixels);
    }
    Ok(samples)
}

const PACKED_LUMA_WORKGROUP_SIZE: u64 = 256;
const PACKED_LA_WORKGROUP_SIZE: u64 = 256;

/// Plan a one-dimensional dispatch over packed L samples. Each invocation
/// handles four adjacent bytes in one storage word, so the grid follows the
/// compact transfer rather than four-byte RGBA pixels.
fn plan_packed_luma_dispatch(
    width: u32,
    height: u32,
    max_workgroups_per_dimension: u32,
) -> Result<(u32, u32), PilError> {
    let pixels = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or_else(|| PilError::ValueError("GPU packed-luma image is too large".into()))?;
    let words = pixels.div_ceil(4);
    if pixels == 0 || words > u64::from(u32::MAX) {
        return Err(PilError::ValueError(
            "GPU packed-luma dimensions exceed shader indexing limits".into(),
        ));
    }
    if max_workgroups_per_dimension == 0 {
        return Err(PilError::ValueError(
            "GPU adapter reports no compute workgroups per dimension".into(),
        ));
    }

    let groups = words.div_ceil(PACKED_LUMA_WORKGROUP_SIZE).max(1);
    if groups > u64::from(max_workgroups_per_dimension) {
        return Err(PilError::ValueError(
            "GPU packed-luma dispatch exceeds adapter workgroup limits".into(),
        ));
    }
    Ok((
        u32::try_from(groups)
            .map_err(|_| PilError::ValueError("GPU packed-luma dispatch is too wide".into()))?,
        1,
    ))
}

/// Plan dispatch for native-LA words. Each little-endian u32 stores two
/// adjacent interleaved LA pixels, so its compact word count is ceil(pixels/2).
fn plan_packed_la_dispatch(
    width: u32,
    height: u32,
    max_workgroups_per_dimension: u32,
) -> Result<(u32, u32), PilError> {
    let pixels = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or_else(|| PilError::ValueError("GPU packed-LA image is too large".into()))?;
    let words = pixels.div_ceil(2);
    if pixels == 0 || pixels > u64::from(u32::MAX) || words > u64::from(u32::MAX) {
        return Err(PilError::ValueError(
            "GPU packed-LA dimensions exceed shader indexing limits".into(),
        ));
    }
    if max_workgroups_per_dimension == 0 {
        return Err(PilError::ValueError(
            "GPU adapter reports no compute workgroups per dimension".into(),
        ));
    }

    let groups = words.div_ceil(PACKED_LA_WORKGROUP_SIZE).max(1);
    if groups > u64::from(max_workgroups_per_dimension) {
        return Err(PilError::ValueError(
            "GPU packed-LA dispatch exceeds adapter workgroup limits".into(),
        ));
    }
    Ok((
        u32::try_from(groups)
            .map_err(|_| PilError::ValueError("GPU packed-LA dispatch is too wide".into()))?,
        1,
    ))
}

/// Plan the row-packed resize kernels used by native-L Pad. Unlike the
/// flattened point kernels, the resize shaders use one 16x16 invocation grid
/// over packed output words per row; the full word range must fit WGSL u32
/// indexing and both adapter dispatch axes.
fn plan_packed_luma_resize_dispatch(
    width: u32,
    height: u32,
    max_workgroups_per_dimension: u32,
) -> Result<(u32, u32), PilError> {
    let pixels = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or_else(|| PilError::ValueError("GPU packed-L resize is too large".into()))?;
    let words_per_row = width.div_ceil(4);
    let words = u64::from(words_per_row)
        .checked_mul(u64::from(height))
        .ok_or_else(|| PilError::ValueError("GPU packed-L resize is too large".into()))?;
    if width == 0 || height == 0 || pixels > u64::from(u32::MAX) || words > u64::from(u32::MAX) {
        return Err(PilError::ValueError(
            "GPU packed-L resize dimensions exceed shader indexing limits".into(),
        ));
    }
    if max_workgroups_per_dimension == 0 {
        return Err(PilError::ValueError(
            "GPU adapter reports no compute workgroups per dimension".into(),
        ));
    }
    let groups_x = words_per_row.div_ceil(16);
    let groups_y = height.div_ceil(16);
    if groups_x > max_workgroups_per_dimension || groups_y > max_workgroups_per_dimension {
        return Err(PilError::ValueError(
            "GPU packed-L resize exceeds adapter workgroup limits".into(),
        ));
    }
    Ok((groups_x, groups_y))
}

fn plan_packed_point_luma_dispatch(
    width: u32,
    height: u32,
    max_workgroups_per_dimension: u32,
) -> Result<(u32, u32), PilError> {
    plan_packed_luma_dispatch(width, height, max_workgroups_per_dimension)
}

/// Plan compact one-channel output dispatches such as ExtractBand and
/// Grayscale as a near-square 2D grid. One shader invocation writes one packed
/// word containing four output pixels. The flattened word index uses `u32`, so
/// reject dimensions and padded grids outside that index range.
const GRAYSCALE_WORKGROUP_SIZE: u64 = 256;

/// Plan one invocation per four-pixel output word across a bounded 2D grid.
fn plan_packed_output_dispatch(
    width: u32,
    height: u32,
    max_workgroups_per_dimension: u32,
    workgroup_size: u64,
    operation: &'static str,
) -> Result<(u32, u32), PilError> {
    let pixel_count = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or_else(|| PilError::ValueError(format!("{operation} image is too large")))?;
    if pixel_count == 0 || pixel_count > u64::from(u32::MAX) {
        return Err(PilError::ValueError(format!(
            "{operation} dimensions exceed shader indexing limits"
        )));
    }
    if max_workgroups_per_dimension == 0 {
        return Err(PilError::ValueError(
            "GPU adapter reports no compute workgroups per dimension".into(),
        ));
    }
    if workgroup_size == 0 {
        return Err(PilError::ValueError(
            "GPU packed-output workgroup size must be nonzero".into(),
        ));
    }

    let output_words = pixel_count.div_ceil(4);
    let required_workgroups = output_words.div_ceil(workgroup_size).max(1);
    let mut groups_x = required_workgroups.isqrt();
    let groups_x_square = groups_x
        .checked_mul(groups_x)
        .ok_or_else(|| PilError::ValueError(format!("{operation} dispatch overflows")))?;
    if groups_x_square < required_workgroups {
        groups_x = groups_x
            .checked_add(1)
            .ok_or_else(|| PilError::ValueError(format!("{operation} dispatch overflows")))?;
    }
    groups_x = groups_x.min(u64::from(max_workgroups_per_dimension));
    let groups_y = required_workgroups.div_ceil(groups_x);
    if groups_y > u64::from(max_workgroups_per_dimension) {
        return Err(PilError::ValueError(format!(
            "{operation} exceeds adapter workgroup limits"
        )));
    }

    let padded_word_count = groups_x
        .checked_mul(groups_y)
        .and_then(|groups| groups.checked_mul(workgroup_size))
        .ok_or_else(|| PilError::ValueError(format!("{operation} dispatch overflows")))?;
    let Some(max_output_word) = padded_word_count.checked_sub(1) else {
        return Err(PilError::ValueError(format!(
            "{operation} dispatch exceeds shader indexing limits"
        )));
    };
    if max_output_word > u64::from(u32::MAX) {
        return Err(PilError::ValueError(format!(
            "{operation} dispatch exceeds shader indexing limits"
        )));
    }

    Ok((
        u32::try_from(groups_x)
            .map_err(|_| PilError::ValueError(format!("{operation} dispatch is too wide")))?,
        u32::try_from(groups_y)
            .map_err(|_| PilError::ValueError(format!("{operation} dispatch is too tall")))?,
    ))
}

fn plan_extract_band_dispatch(
    width: u32,
    height: u32,
    max_workgroups_per_dimension: u32,
) -> Result<(u32, u32), PilError> {
    plan_packed_output_dispatch(
        width,
        height,
        max_workgroups_per_dimension,
        64,
        "GPU channel extraction",
    )
}

const SHARPNESS_L_WORKGROUP_WIDTH: u64 = 16;
const SHARPNESS_L_WORKGROUP_HEIGHT: u64 = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SharpnessLDispatch {
    groups_x: u32,
    groups_y: u32,
    row_aligned: bool,
}

/// Plan one invocation per four-pixel output word. Word-aligned widths map
/// directly to image rows; other dimensions use a bounded flattened 2D grid.
/// The shader's fourth uniform word selects the same layout returned here.
fn plan_sharpness_l_dispatch(
    width: u32,
    height: u32,
    max_workgroups_per_dimension: u32,
) -> Result<SharpnessLDispatch, PilError> {
    let pixel_count = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or_else(|| PilError::ValueError("GPU Sharpness image is too large".into()))?;
    if pixel_count == 0 || pixel_count > u64::from(u32::MAX) {
        return Err(PilError::ValueError(
            "GPU Sharpness dimensions exceed shader indexing limits".into(),
        ));
    }
    if max_workgroups_per_dimension == 0 {
        return Err(PilError::ValueError(
            "GPU adapter reports no compute workgroups per dimension".into(),
        ));
    }

    let max_groups = u64::from(max_workgroups_per_dimension);
    if width % 4 == 0 {
        let words_per_row = u64::from(width) / 4;
        let groups_x = words_per_row.div_ceil(SHARPNESS_L_WORKGROUP_WIDTH);
        let groups_y = u64::from(height).div_ceil(SHARPNESS_L_WORKGROUP_HEIGHT);
        if groups_x <= max_groups && groups_y <= max_groups {
            return Ok(SharpnessLDispatch {
                groups_x: u32::try_from(groups_x).map_err(|_| {
                    PilError::ValueError("GPU Sharpness dispatch is too wide".into())
                })?,
                groups_y: u32::try_from(groups_y).map_err(|_| {
                    PilError::ValueError("GPU Sharpness dispatch is too tall".into())
                })?,
                row_aligned: true,
            });
        }
    }

    let output_words = pixel_count.div_ceil(4);
    let invocations_per_group = SHARPNESS_L_WORKGROUP_WIDTH * SHARPNESS_L_WORKGROUP_HEIGHT;
    let required_groups = output_words.div_ceil(invocations_per_group).max(1);
    let mut groups_x = required_groups.isqrt();
    if groups_x * groups_x < required_groups {
        groups_x = groups_x
            .checked_add(1)
            .ok_or_else(|| PilError::ValueError("GPU Sharpness dispatch overflows".into()))?;
    }
    groups_x = groups_x.min(max_groups);
    let groups_y = required_groups.div_ceil(groups_x);
    if groups_y > max_groups {
        return Err(PilError::ValueError(
            "GPU Sharpness dispatch exceeds adapter workgroup limits".into(),
        ));
    }
    let padded_words = groups_x
        .checked_mul(groups_y)
        .and_then(|groups| groups.checked_mul(invocations_per_group))
        .ok_or_else(|| PilError::ValueError("GPU Sharpness dispatch overflows".into()))?;
    if padded_words.saturating_sub(1) > u64::from(u32::MAX) {
        return Err(PilError::ValueError(
            "GPU Sharpness dispatch exceeds shader indexing limits".into(),
        ));
    }

    Ok(SharpnessLDispatch {
        groups_x: u32::try_from(groups_x)
            .map_err(|_| PilError::ValueError("GPU Sharpness dispatch is too wide".into()))?,
        groups_y: u32::try_from(groups_y)
            .map_err(|_| PilError::ValueError("GPU Sharpness dispatch is too tall".into()))?,
        row_aligned: false,
    })
}

const BLUR_WORKGROUP_SIZE: u32 = 16;

/// Plan bounded 2D blur workgroups. Each invocation computes one output pixel;
/// shader grid-stride loops cover dimensions larger than the device grid.
fn plan_blur_dispatch(
    width: u32,
    height: u32,
    max_workgroups_per_dimension: u32,
) -> Result<(u32, u32), PilError> {
    if max_workgroups_per_dimension == 0 {
        return Err(PilError::ValueError(
            "GPU adapter reports no compute workgroups per dimension".into(),
        ));
    }
    let max_groups = max_workgroups_per_dimension;
    let groups = |dimension: u32| {
        dimension
            .div_ceil(BLUR_WORKGROUP_SIZE)
            .max(1)
            .min(max_groups)
    };
    Ok((groups(width), groups(height)))
}

fn gpu_buffer_reuse_allowed(capacity: u32, minimum_capacity: u32) -> bool {
    capacity >= minimum_capacity
        && (minimum_capacity == 0
            || capacity <= minimum_capacity.saturating_mul(MAX_GPU_BUFFER_REUSE_RATIO))
}

#[cfg(target_endian = "little")]
fn expand_rgb_into_rgba(rgb: &[u8], rgba: &mut [u8]) -> Result<(), PilError> {
    if !rgb.len().is_multiple_of(3) {
        return Err(PilError::InternalError(
            "GPU RGB source has a partial pixel".into(),
        ));
    }
    let expected = rgb
        .len()
        .checked_div(3)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| PilError::InternalError("GPU RGB upload size overflow".into()))?;
    if rgba.len() != expected {
        return Err(PilError::InternalError(format!(
            "GPU RGB staging length {} does not match expected {expected}",
            rgba.len()
        )));
    }

    for (source, target) in rgb.chunks_exact(3).zip(rgba.chunks_exact_mut(4)) {
        target.copy_from_slice(&[source[0], source[1], source[2], u8::MAX]);
    }
    Ok(())
}

/// Extract native RGB bytes directly from the mapped packed-word transport.
/// Alpha is discarded exactly as RGBA-to-RGB mode preservation does; no
/// channel arithmetic or logical color conversion belongs at this boundary.
fn rgb_from_packed_readback(w: u32, h: u32, packed: &[u8]) -> Result<DynamicImage, PilError> {
    let packed_dims = CheckedDims::new(w, h, 4)?;
    if packed.len() != packed_dims.total_bytes() {
        return Err(PilError::ValueError(format!(
            "GPU readback byte length {} does not match image size {}",
            packed.len(),
            packed_dims.total_bytes()
        )));
    }
    let rgb_dims = CheckedDims::new(w, h, 3)?;
    let mut rgb = rgb_dims.alloc_buffer();
    let unpack_row = |row: &mut [u8], y: usize| {
        let start = y * packed_dims.row_stride();
        let source = &packed[start..start + packed_dims.row_stride()];
        for (source, destination) in source.chunks_exact(4).zip(row.chunks_exact_mut(3)) {
            #[cfg(target_endian = "little")]
            destination.copy_from_slice(&source[..3]);
            #[cfg(target_endian = "big")]
            {
                // Match readback_to_image's native-word decoding before
                // removing alpha on hosts whose byte order differs.
                let pixel = u32::from_ne_bytes([source[0], source[1], source[2], source[3]]);
                destination.copy_from_slice(&[
                    (pixel & 0xff) as u8,
                    ((pixel >> 8) & 0xff) as u8,
                    ((pixel >> 16) & 0xff) as u8,
                ]);
            }
        }
    };
    // Keep GPU result materialization independent of the Parallel CPU
    // feature; the GPU path must not schedule host Rayon work.
    for (y, row) in rgb.chunks_exact_mut(rgb_dims.row_stride()).enumerate() {
        unpack_row(row, y);
    }
    crate::raster::RgbImage::from_raw(w, h, rgb)
        .map(DynamicImage::ImageRgb8)
        .ok_or_else(|| PilError::InternalError("GPU RGB readback shape mismatch".into()))
}

/// Narrow packed RGBA transport to LA without first constructing an RGBA
/// image. Native-LA Sharpness emits its filtered luma in R/G/B and preserves
/// alpha in A, matching the generic transport's mode-preserving boundary.
#[cfg(target_endian = "little")]
fn la_from_rgba_transport(w: u32, h: u32, packed: &[u8]) -> Result<DynamicImage, PilError> {
    let packed_dims = CheckedDims::new(w, h, 4)?;
    if packed.len() != packed_dims.total_bytes() {
        return Err(PilError::ValueError(format!(
            "GPU readback byte length {} does not match image size {}",
            packed.len(),
            packed_dims.total_bytes()
        )));
    }
    let la_dims = CheckedDims::new(w, h, 2)?;
    let mut la = la_dims.alloc_buffer();
    for (source, destination) in packed.chunks_exact(4).zip(la.chunks_exact_mut(2)) {
        destination.copy_from_slice(&[source[0], source[3]]);
    }
    crate::raster::GrayAlphaImage::from_raw(w, h, la)
        .map(DynamicImage::ImageLumaA8)
        .ok_or_else(|| PilError::InternalError("GPU LA readback shape mismatch".into()))
}

/// Pack an RGBA image into the storage representation used by the shaders.
fn pack_rgba(rgba: &RgbaImage, capacity: u32) -> Result<Vec<u32>, PilError> {
    let (w, h) = rgba.dimensions();
    let n = CheckedDims::new(w, h, 1)?.total_pixels();
    if n > capacity as usize {
        return Err(PilError::ValueError(format!(
            "GPU buffer capacity {} < image size {}",
            capacity, n
        )));
    }
    Ok(rgba
        .pixels()
        .map(|px| {
            (px[0] as u32) | ((px[1] as u32) << 8) | ((px[2] as u32) << 16) | ((px[3] as u32) << 24)
        })
        .collect())
}

/// Pack an auxiliary image for the GPU without cloning one that is already
/// RGBA. Other formats retain the existing RGBA transport semantics.
fn pack_dynamic_rgba(image: &DynamicImage, capacity: u32) -> Result<Vec<u32>, PilError> {
    match image {
        DynamicImage::ImageRgba8(rgba) => pack_rgba(rgba, capacity),
        _ => {
            let rgba = image.to_rgba8();
            pack_rgba(&rgba, capacity)
        }
    }
}

/// Append an RGBA auxiliary image directly to the reusable GPU upload arena.
/// On little-endian targets its byte layout is already the shader's u32 layout,
/// so this avoids both a temporary RGBA conversion and a temporary word vector.
fn append_dynamic_rgba(
    arena: &mut Vec<u32>,
    image: &DynamicImage,
    capacity: u32,
    alignment_bytes: usize,
) -> Result<BufferRange, PilError> {
    match image {
        DynamicImage::ImageRgba8(rgba) => {
            append_rgba_to_arena(arena, rgba, capacity, alignment_bytes)
        }
        _ => {
            let rgba = image.to_rgba8();
            append_rgba_to_arena(arena, &rgba, capacity, alignment_bytes)
        }
    }
}

fn append_rgba_to_arena(
    arena: &mut Vec<u32>,
    rgba: &RgbaImage,
    capacity: u32,
    alignment_bytes: usize,
) -> Result<BufferRange, PilError> {
    let (w, h) = rgba.dimensions();
    let pixel_count = CheckedDims::new(w, h, 1)?.total_pixels();
    if pixel_count > capacity as usize {
        return Err(PilError::ValueError(format!(
            "GPU buffer capacity {} < image size {}",
            capacity, pixel_count
        )));
    }

    let alignment_words = alignment_bytes.max(4).div_ceil(4);
    let offset_words = align_up(arena.len(), alignment_words);
    let size_words = align_up(pixel_count.max(1), alignment_words);
    let end_words = offset_words
        .checked_add(size_words)
        .ok_or_else(|| PilError::ValueError("GPU RGBA auxiliary size is too large".into()))?;
    arena.resize(end_words, 0);

    #[cfg(target_endian = "little")]
    {
        let packed_bytes = bytemuck::cast_slice_mut::<u32, u8>(
            &mut arena[offset_words..offset_words + pixel_count],
        );
        if packed_bytes.len() != rgba.as_raw().len() {
            return Err(PilError::InternalError(
                "GPU RGBA auxiliary byte length does not match image dimensions".into(),
            ));
        }
        packed_bytes.copy_from_slice(rgba.as_raw());
    }
    #[cfg(target_endian = "big")]
    {
        for (packed, pixel) in arena[offset_words..offset_words + pixel_count]
            .iter_mut()
            .zip(rgba.pixels())
        {
            *packed = (pixel[0] as u32)
                | ((pixel[1] as u32) << 8)
                | ((pixel[2] as u32) << 16)
                | ((pixel[3] as u32) << 24);
        }
    }

    Ok(BufferRange {
        offset: (offset_words * std::mem::size_of::<u32>()) as u64,
        size: (size_words * std::mem::size_of::<u32>()) as u64,
    })
}

/// Pack typed `I;16*` samples as numeric little-endian u16 values in the low
/// half of one storage word per pixel. This is distinct from the opaque byte
/// packing used by I;16 geometry: Paste blends the decoded unsigned samples,
/// so narrowing through RGBA8 would discard the high byte.
fn pack_luma16_numeric(
    image: &ImageBuffer<Luma<u16>, Vec<u16>>,
    capacity: u32,
) -> Result<Vec<u32>, PilError> {
    let (w, h) = image.dimensions();
    let pixel_count = CheckedDims::new(w, h, 1)?.total_pixels();
    if pixel_count > capacity as usize {
        return Err(PilError::ValueError(format!(
            "GPU buffer capacity {} < image size {}",
            capacity, pixel_count
        )));
    }
    Ok(image
        .as_raw()
        .iter()
        .map(|&sample| u32::from(sample))
        .collect())
}

/// Pack logical `putdata` samples into the auxiliary storage representation.
///
/// LA/PA place alpha in packed byte 3, matching the GPU's RGBA transport;
/// all other modes retain their raw channel order. The shader uses the
/// original byte length to preserve every untouched or partial pixel.
fn pack_put_data(data: &[u8], mode: PixelMode, capacity: u32) -> Result<Vec<u32>, PilError> {
    let channels = mode.channels();
    let pixel_count = data.len().div_ceil(channels);
    if pixel_count > capacity as usize {
        return Err(PilError::ValueError(format!(
            "GPU buffer capacity {} < putdata image size {}",
            capacity, pixel_count
        )));
    }
    let mut packed = Vec::with_capacity(pixel_count);
    for pixel_index in 0..pixel_count {
        let start = pixel_index * channels;
        let samples = &data[start..data.len().min(start + channels)];
        let pixel = match mode {
            PixelMode::L | PixelMode::P | PixelMode::Mode1 => {
                samples.first().copied().unwrap_or(0) as u32
            }
            PixelMode::LA | PixelMode::PA => {
                let luma = samples.first().copied().unwrap_or(0) as u32;
                let alpha = samples.get(1).copied().unwrap_or(0) as u32;
                luma | (alpha << 24)
            }
            PixelMode::RGB | PixelMode::YCbCr | PixelMode::HSV => {
                let r = samples.first().copied().unwrap_or(0) as u32;
                let g = samples.get(1).copied().unwrap_or(0) as u32;
                let b = samples.get(2).copied().unwrap_or(0) as u32;
                r | (g << 8) | (b << 16)
            }
            PixelMode::RGBA | PixelMode::CMYK | PixelMode::I | PixelMode::F => {
                let first = samples.first().copied().unwrap_or(0) as u32;
                let second = samples.get(1).copied().unwrap_or(0) as u32;
                let third = samples.get(2).copied().unwrap_or(0) as u32;
                let fourth = samples.get(3).copied().unwrap_or(0) as u32;
                first | (second << 8) | (third << 16) | (fourth << 24)
            }
        };
        packed.push(pixel);
    }
    Ok(packed)
}

/// Pack native-L PutData bytes four samples per storage word. This is the
/// auxiliary layout consumed by the compact L shader on little-endian hosts.
fn pack_luma8_putdata(data: &[u8], capacity: u32) -> Result<Vec<u32>, PilError> {
    let word_count = data.len().div_ceil(std::mem::size_of::<u32>());
    if word_count > capacity as usize {
        return Err(PilError::ValueError(format!(
            "GPU buffer capacity {} < packed-luma putdata size {}",
            capacity, word_count
        )));
    }
    let mut packed = Vec::with_capacity(word_count);
    for chunk in data.chunks(std::mem::size_of::<u32>()) {
        let mut bytes = [0u8; std::mem::size_of::<u32>()];
        bytes[..chunk.len()].copy_from_slice(chunk);
        packed.push(u32::from_le_bytes(bytes));
    }
    Ok(packed)
}

/// Prepare Color3DLut table entries in the same signed 10.6 representation
/// used by the CPU interpolation path. The returned i16 values are serialized
/// as one little-endian u32 word per table component for a read-only shader
/// buffer; keeping this conversion shared prevents f32/table rounding drift.
fn color3dlut_table_words(table: &[f64]) -> Result<Vec<u32>, PilError> {
    const PRECISION_BITS: i32 = 6;
    let mut words = Vec::with_capacity(table.len());
    for &value in table {
        if !value.is_finite() {
            return Err(PilError::ValueError(
                "GPU Color3DLut table contains a non-finite value".into(),
            ));
        }
        let item = value as f32;
        let scaled = item * ((255 << PRECISION_BITS) as f32);
        let prepared = if scaled >= i16::MAX as f32 - 0.5 {
            i16::MAX
        } else if scaled <= i16::MIN as f32 + 0.5 {
            i16::MIN
        } else if item < 0.0 {
            (scaled - 0.5) as i16
        } else {
            (scaled + 0.5) as i16
        };
        words.push(u32::from(u16::from_ne_bytes(prepared.to_ne_bytes())));
    }
    Ok(words)
}

fn align_up(value: usize, alignment: usize) -> usize {
    if alignment <= 1 {
        value
    } else {
        let remainder = value % alignment;
        if remainder == 0 {
            value
        } else {
            value + alignment - remainder
        }
    }
}

fn aligned_bytes(bytes: usize, alignment: usize) -> usize {
    align_up(bytes.max(1), alignment.max(4))
}

fn putdata_auxiliary_words(data_len: usize, channels: usize, packed_luma: bool) -> usize {
    if packed_luma {
        data_len.div_ceil(std::mem::size_of::<u32>())
    } else {
        data_len.div_ceil(channels)
    }
}

fn select_gpu_chunk_end(
    chunk_start: usize,
    resource_bytes: &[usize],
    shader_work_items: &[u64],
) -> Option<usize> {
    if resource_bytes.len() != shader_work_items.len() {
        return None;
    }
    let mut chunk_end = chunk_start;
    let mut total_bytes = 0usize;
    let mut total_work = 0u64;
    while chunk_end < resource_bytes.len() && chunk_end - chunk_start < MAX_GPU_OPS_PER_SUBMISSION {
        let op_bytes = resource_bytes[chunk_end];
        let op_work = shader_work_items[chunk_end];
        if chunk_end > chunk_start
            && (total_bytes.saturating_add(op_bytes) > MAX_GPU_RESOURCE_BYTES_PER_SUBMISSION
                || total_work.saturating_add(op_work) > MAX_GPU_SHADER_WORK_ITEMS)
        {
            break;
        }
        total_bytes = total_bytes.saturating_add(op_bytes);
        total_work = total_work.saturating_add(op_work);
        chunk_end += 1;
    }
    (chunk_end > chunk_start).then_some(chunk_end)
}

/// Append one aligned slice to a u32 arena and return its byte range.
fn append_arena_slice(arena: &mut Vec<u32>, values: &[u32], alignment_bytes: usize) -> BufferRange {
    let alignment_words = alignment_bytes.max(4).div_ceil(4);
    let offset_words = align_up(arena.len(), alignment_words);
    let size_words = align_up(values.len().max(1), alignment_words);
    arena.resize(offset_words + size_words, 0);
    arena[offset_words..offset_words + values.len()].copy_from_slice(values);
    BufferRange {
        offset: (offset_words * std::mem::size_of::<u32>()) as u64,
        size: (size_words * std::mem::size_of::<u32>()) as u64,
    }
}

fn create_sized_buffer(
    device: &wgpu::Device,
    label: &'static str,
    usage: wgpu::BufferUsages,
    size_bytes: usize,
    alignment_bytes: usize,
) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: aligned_bytes(size_bytes, alignment_bytes) as u64,
        usage,
        mapped_at_creation: false,
    })
}

struct GpuBatchResources<'a> {
    buf_a: &'a wgpu::Buffer,
    buf_b: &'a wgpu::Buffer,
    initial_input: Option<&'a wgpu::Buffer>,
    fallback_img2: &'a wgpu::Buffer,
    fallback_img3: &'a wgpu::Buffer,
    fallback_lut: &'a wgpu::Buffer,
    histogram: &'a wgpu::Buffer,
    params: &'a wgpu::Buffer,
    params_ranges: Vec<BufferRange>,
    img2: Option<&'a wgpu::Buffer>,
    img2_ranges: Vec<Option<BufferRange>>,
    resize_coeff_ranges: Vec<Option<(BufferRange, BufferRange)>>,
    img3: Option<&'a wgpu::Buffer>,
    img3_ranges: Vec<Option<BufferRange>>,
    lut: Option<&'a wgpu::Buffer>,
    lut_ranges: Vec<Option<BufferRange>>,
    native_expand_output: Option<NativeExpandOutputDispatch>,
    native_luma_transform_output: Option<NativeExpandOutputDispatch>,
    native_rgb_packed_output: Option<NativePackedChannelOutputDispatch>,
    native_la_transform_output: Option<NativePackedChannelOutputDispatch>,
    native_cover_packed_output: Option<NativeCoverPackedOutputDispatch>,
}

struct PreparedGpuBatch<'a> {
    resources: GpuBatchResources<'a>,
    input_dims: Vec<(u32, u32)>,
    output_dims: Vec<(u32, u32)>,
    /// Source row range represented by a boxed resize's compact horizontal
    /// intermediate. Other operations retain their complete input height.
    resize_source_rows: Vec<Option<(u32, u32)>>,
    /// Contain dimensions used by a public Pad operation before its final
    /// placement pass.  The resize and placement remain in one command
    /// buffer, so this is planner metadata rather than a host-side image
    /// handoff.
    pad_resize_dims: Vec<Option<(u32, u32)>>,
    final_dims: (u32, u32),
    resource_telemetry: PipelineResourceTelemetry,
}

fn ranged_binding(
    buffer: &wgpu::Buffer,
    range: Option<BufferRange>,
) -> Result<wgpu::BindingResource<'_>, PilError> {
    match range {
        Some(range) => {
            let size = NonZeroU64::new(range.size).ok_or_else(|| {
                PilError::InternalError("GPU buffer binding range cannot be empty".into())
            })?;
            Ok(wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer,
                offset: range.offset,
                size: Some(size),
            }))
        }
        None => Ok(buffer.as_entire_binding()),
    }
}

fn auxiliary_binding<'a>(
    arena: Option<&'a wgpu::Buffer>,
    fallback: &'a wgpu::Buffer,
    range: Option<BufferRange>,
) -> Result<wgpu::BindingResource<'a>, PilError> {
    match range {
        Some(range) => {
            let buffer = arena.ok_or_else(|| {
                PilError::InternalError("GPU auxiliary range has no backing buffer".into())
            })?;
            ranged_binding(buffer, Some(range))
        }
        None => ranged_binding(fallback, None),
    }
}

enum ResolvedPipeline {
    Single(Arc<CachedPipeline>),
    /// The second public operation of a private fused dispatch.  It remains
    /// in the prepared operation/resource arrays so the public operation
    /// count, auxiliary ownership, and parameter indexing stay unchanged.
    Skip,
    Blur {
        horizontal: Arc<CachedPipeline>,
        vertical: Arc<CachedPipeline>,
        pass_count: usize,
    },
    /// Non-nearest resize is the same exact two-pass fixed-point algorithm
    /// used by the CPU/SIMD paths.  Coefficients live in an auxiliary storage
    /// range, so the two dispatches stay on-device with no host materialize.
    Resize {
        horizontal: Arc<CachedPipeline>,
        vertical: Arc<CachedPipeline>,
    },
    /// Pad is an exact contain resize followed by a device-side fill/copy
    /// placement.  Keeping both stages inside one compute pass avoids a host
    /// materialization while preserving Pillow's separable resize contract.
    Pad {
        horizontal: Arc<CachedPipeline>,
        vertical: Arc<CachedPipeline>,
        place: Arc<CachedPipeline>,
        /// RGBX resize variant that writes the vertical result into the final
        /// canvas, including its fill rows, instead of materializing a resized
        /// frame for the placement pass.
        vertical_pad_rgbx: Option<Arc<CachedPipeline>>,
    },
    /// Histogram-driven ImageOps keep all control/data passes on the device:
    /// clear the reusable histogram, accumulate the current image, derive a
    /// packed 256-entry LUT, and apply that LUT through the vector remap.
    Histogram {
        clear: Arc<CachedPipeline>,
        histogram: Arc<CachedPipeline>,
        derive: Arc<CachedPipeline>,
        remap: Arc<CachedPipeline>,
    },
}

#[derive(Default)]
struct GpuDeviceState {
    lost: Option<String>,
    uncaptured_error: Option<String>,
}

// ─── GpuInner (lazy-initialized GPU engine) ────────────────────────────────

/// Internal GPU engine. Initialized once and stored in a static OnceLock.
struct GpuInner {
    device: wgpu::Device,
    queue: wgpu::Queue,
    /// Pipelines are compiled on first use. Values are shared so a resolved
    /// batch can keep every pipeline alive for the full compute-pass lifetime
    /// without holding the cache lock while commands are encoded.
    pipelines: Mutex<HashMap<&'static str, Arc<CachedPipeline>>>,
    device_state: Arc<Mutex<GpuDeviceState>>,
    available_buffers: Mutex<Vec<BufferPool>>,
    available_staging: Mutex<Vec<StagingBuffer>>,
    direct_primary_readback: bool,
}

fn direct_primary_readback_supported(
    backend: wgpu::Backend,
    device_type: wgpu::DeviceType,
    features: wgpu::Features,
) -> bool {
    // Limit the first transport implementation to a native backend whose
    // integrated adapter classification denotes shared CPU/GPU memory.
    cfg!(not(target_arch = "wasm32"))
        && backend == wgpu::Backend::Metal
        && device_type == wgpu::DeviceType::IntegratedGpu
        && features.contains(wgpu::Features::MAPPABLE_PRIMARY_BUFFERS)
}

#[cfg(not(target_arch = "wasm32"))]
fn format_adapter_inventory(adapters: &[wgpu::Adapter]) -> String {
    let infos = adapters
        .iter()
        .map(wgpu::Adapter::get_info)
        .collect::<Vec<_>>();
    format!("enumerated={} adapters={infos:?}", adapters.len())
}

impl GpuInner {
    fn new() -> Result<Self, PilError> {
        // `Instance::default()` does not apply WGPU_BACKEND. Use the explicit
        // descriptor so backend selection is deterministic and debuggable in
        // the Python extension as well as in standalone Rust binaries.
        let instance_descriptor = wgpu::InstanceDescriptor::from_env_or_default();
        let instance = wgpu::Instance::new(&instance_descriptor);
        let request_options = wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
        };

        #[cfg(not(target_arch = "wasm32"))]
        let enumerated_adapters = instance.enumerate_adapters(instance_descriptor.backends);
        #[cfg(not(target_arch = "wasm32"))]
        let adapter_inventory = format_adapter_inventory(&enumerated_adapters);

        // Prefer wgpu's normal selection because it applies power preference
        // and compatibility rules. Some embedded/native-library contexts have
        // been observed to return None here even though direct enumeration
        // succeeds; use that already-enumerated adapter instead of rejecting
        // a usable device in that case.
        let adapter =
            pollster::block_on(instance.request_adapter(&request_options)).or_else(|| {
                #[cfg(not(target_arch = "wasm32"))]
                {
                    enumerated_adapters.into_iter().next()
                }
                #[cfg(target_arch = "wasm32")]
                {
                    None
                }
            });
        let adapter = adapter.ok_or_else(|| {
            #[cfg(not(target_arch = "wasm32"))]
            {
                return PilError::ValueError(format!(
                    "GPU adapter not available: requested={:?}, enabled={:?}, {}",
                    instance_descriptor.backends,
                    wgpu::Instance::enabled_backend_features(),
                    adapter_inventory,
                ));
            }
            #[cfg(target_arch = "wasm32")]
            {
                PilError::ValueError(format!(
                    "GPU adapter not available: requested={:?}",
                    instance_descriptor.backends,
                ))
            }
        })?;
        let adapter_info = adapter.get_info();
        gpu_log!("[GPU] adapter selected: {:?}", adapter_info);
        let direct_primary_readback = direct_primary_readback_supported(
            adapter_info.backend,
            adapter_info.device_type,
            adapter.features(),
        );
        let required_features = if direct_primary_readback {
            wgpu::Features::MAPPABLE_PRIMARY_BUFFERS
        } else {
            wgpu::Features::empty()
        };
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("pillow-rs-gpu"),
                required_features,
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::Performance,
            },
            None,
        ))
        .map_err(|error| {
            PilError::ValueError(format!("GPU device initialization failed: {error}"))
        })?;

        let device_state = Arc::new(Mutex::new(GpuDeviceState::default()));
        let lost_state = Arc::clone(&device_state);
        device.set_device_lost_callback(move |reason, message| {
            let detail = format!("{reason:?}: {message}");
            if let Ok(mut state) = lost_state.lock() {
                state.lost = Some(detail.clone());
            }
            log::error!(target: "compute::gpu", "[GPU] device lost: {detail}");
        });
        let error_state = Arc::clone(&device_state);
        device.on_uncaptured_error(Box::new(move |error| {
            let detail = error.to_string();
            if let Ok(mut state) = error_state.lock() {
                state.uncaptured_error = Some(detail.clone());
            }
            log::error!(target: "compute::gpu", "[GPU] uncaptured device error: {detail}");
        }));

        Ok(GpuInner {
            device,
            queue,
            pipelines: Mutex::new(HashMap::new()),
            device_state,
            available_buffers: Mutex::new(Vec::new()),
            available_staging: Mutex::new(Vec::new()),
            direct_primary_readback,
        })
    }

    fn acquire_buffers(&self, minimum_capacity: u32) -> Result<BufferPool, PilError> {
        let mut available = self.available_buffers.lock().map_err(|_| {
            PilError::InternalError("GPU working-buffer pool lock is poisoned".into())
        })?;
        let candidate = available
            .iter()
            .enumerate()
            .filter(|(_, buffers)| gpu_buffer_reuse_allowed(buffers.capacity, minimum_capacity))
            .min_by_key(|(_, buffers)| buffers.capacity)
            .map(|(index, _)| index);
        Ok(match candidate {
            Some(index) => available.swap_remove(index),
            None => BufferPool::new(&self.device, minimum_capacity, self.direct_primary_readback),
        })
    }

    fn recycle_buffers(&self, buffers: BufferPool) {
        self.recycle_buffers_with_limits(
            buffers,
            MAX_RETAINED_GPU_WORKING_SETS,
            MAX_RETAINED_GPU_WORKING_BYTES,
        );
    }

    fn recycle_buffers_with_limits(&self, buffers: BufferPool, max_sets: usize, max_bytes: u64) {
        if self.failure_detail().is_some() || buffers.retained_bytes() > max_bytes {
            return;
        }
        let Ok(mut available) = self.available_buffers.lock() else {
            return;
        };
        available.push(buffers);
        available.sort_unstable_by_key(|candidate| candidate.capacity);
        while available.len() > max_sets
            || available.iter().fold(0u64, |total, candidate| {
                total.saturating_add(candidate.retained_bytes())
            }) > max_bytes
        {
            let _ = available.pop();
        }
    }

    fn acquire_staging(&self, minimum_bytes: u64) -> Result<StagingBuffer, PilError> {
        let mut available = self.available_staging.lock().map_err(|_| {
            PilError::InternalError("GPU staging-buffer pool lock is poisoned".into())
        })?;
        let candidate = available
            .iter()
            .enumerate()
            .filter(|(_, staging)| staging.capacity_bytes >= minimum_bytes)
            .min_by_key(|(_, staging)| staging.capacity_bytes)
            .map(|(index, _)| index);
        Ok(match candidate {
            Some(index) => available.swap_remove(index),
            None => StagingBuffer::new(&self.device, minimum_bytes),
        })
    }

    fn prepare_readback(
        &self,
        source: &wgpu::Buffer,
        size: u64,
    ) -> Result<ReadbackTarget, PilError> {
        // Use the resource's actual usage so a staged working set remains
        // staged even when this device supports directly mapped storage.
        if source.usage().contains(wgpu::BufferUsages::MAP_READ) {
            Ok(ReadbackTarget::Primary)
        } else {
            self.acquire_staging(size).map(ReadbackTarget::Staging)
        }
    }

    fn recycle_staging(&self, staging: StagingBuffer) {
        if self.failure_detail().is_some()
            || staging.capacity_bytes > MAX_RETAINED_GPU_STAGING_BYTES
        {
            return;
        }
        let Ok(mut available) = self.available_staging.lock() else {
            return;
        };
        available.push(staging);
        available.sort_unstable_by_key(|candidate| candidate.capacity_bytes);
        while available.len() > MAX_RETAINED_GPU_STAGING_BUFFERS
            || available.iter().fold(0u64, |total, candidate| {
                total.saturating_add(candidate.capacity_bytes)
            }) > MAX_RETAINED_GPU_STAGING_BYTES
        {
            let _ = available.pop();
        }
    }

    fn invalidate_resource_pools(&self) {
        if let Ok(mut pipelines) = self.pipelines.lock() {
            pipelines.clear();
        }
        if let Ok(mut available) = self.available_buffers.lock() {
            available.clear();
        }
        if let Ok(mut available) = self.available_staging.lock() {
            available.clear();
        }
    }

    fn failure_detail(&self) -> Option<String> {
        self.device_state.lock().ok().and_then(|state| {
            state
                .lost
                .clone()
                .or_else(|| state.uncaptured_error.clone())
        })
    }

    fn ensure_healthy(&self, stage: &str) -> Result<(), PilError> {
        if let Some(detail) = self.failure_detail() {
            self.invalidate_resource_pools();
            return Err(PilError::ValueError(format!(
                "GPU device unavailable during {stage}: {detail}"
            )));
        }
        Ok(())
    }

    fn mark_failed(&self, detail: String) {
        if let Ok(mut state) = self.device_state.lock() {
            if state.lost.is_none() {
                state.lost = Some(detail);
            }
        }
        self.invalidate_resource_pools();
    }

    fn poll_device(&self, stage: &str) -> Result<(), PilError> {
        self.ensure_healthy(stage)?;
        self.device.poll(wgpu::Maintain::Poll);
        self.ensure_healthy(stage)
    }

    fn validate_output_dims(&self, buffers: &BufferPool, w: u32, h: u32) -> Result<(), PilError> {
        let pixels = CheckedDims::new(w, h, 1)?.total_pixels();
        if pixels > buffers.capacity as usize {
            return Err(PilError::ValueError(format!(
                "GPU buffer capacity {} < output image size {}",
                buffers.capacity, pixels
            )));
        }
        Ok(())
    }

    fn upload_auxiliary_cache(
        &self,
        cache: &GpuAuxiliaryCache,
        buffers: &mut BufferPool,
        storage_alignment: usize,
    ) {
        if !cache.img2_values.is_empty() {
            buffers.img2_arena.ensure_capacity(
                &self.device,
                "gpu_batch_img2",
                wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                cache.img2_values.len() * std::mem::size_of::<u32>(),
                storage_alignment,
            );
            self.queue.write_buffer(
                &buffers.img2_arena.buffer,
                0,
                bytemuck::cast_slice(&cache.img2_values),
            );
        }
        if !cache.img3_values.is_empty() {
            buffers.img3_arena.ensure_capacity(
                &self.device,
                "gpu_batch_img3",
                wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                cache.img3_values.len() * std::mem::size_of::<u32>(),
                storage_alignment,
            );
            self.queue.write_buffer(
                &buffers.img3_arena.buffer,
                0,
                bytemuck::cast_slice(&cache.img3_values),
            );
        }
        if !cache.lut_values.is_empty() {
            buffers.lut_arena.ensure_capacity(
                &self.device,
                "gpu_batch_lut",
                wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                cache.lut_values.len() * std::mem::size_of::<u32>(),
                storage_alignment,
            );
            self.queue.write_buffer(
                &buffers.lut_arena.buffer,
                0,
                bytemuck::cast_slice(&cache.lut_values),
            );
        }
    }

    fn append_resize_coeff_ranges(
        &self,
        img2_arena: &mut Vec<u32>,
        auxiliary_cache: &GpuAuxiliaryCache,
        horizontal: &FilterCoeffs,
        vertical: &FilterCoeffs,
        storage_alignment: usize,
    ) -> Result<(BufferRange, BufferRange), PilError> {
        let horizontal_words = encode_resize_coeffs(horizontal)?;
        let vertical_words = encode_resize_coeffs(vertical)?;
        let base_offset = (auxiliary_cache.img2_values.len() * 4) as u64;
        let mut horizontal_range =
            append_arena_slice(img2_arena, &horizontal_words, storage_alignment);
        horizontal_range.offset += base_offset;
        let mut vertical_range = append_arena_slice(img2_arena, &vertical_words, storage_alignment);
        vertical_range.offset += base_offset;
        Ok((horizontal_range, vertical_range))
    }

    fn append_resize_f64_coeff_ranges(
        &self,
        img2_arena: &mut Vec<u32>,
        auxiliary_cache: &GpuAuxiliaryCache,
        horizontal: &FilterCoeffsF64,
        vertical: &FilterCoeffsF64,
        storage_alignment: usize,
    ) -> Result<(BufferRange, BufferRange), PilError> {
        let horizontal_words = encode_resize_coeffs_f64(horizontal)?;
        let vertical_words = encode_resize_coeffs_f64(vertical)?;
        self.append_resize_coeff_word_ranges(
            img2_arena,
            auxiliary_cache,
            &horizontal_words,
            &vertical_words,
            storage_alignment,
        )
    }

    fn append_resize_coeff_word_ranges(
        &self,
        img2_arena: &mut Vec<u32>,
        auxiliary_cache: &GpuAuxiliaryCache,
        horizontal_words: &[u32],
        vertical_words: &[u32],
        storage_alignment: usize,
    ) -> Result<(BufferRange, BufferRange), PilError> {
        let base_offset = (auxiliary_cache.img2_values.len() * 4) as u64;
        let mut horizontal_range =
            append_arena_slice(img2_arena, horizontal_words, storage_alignment);
        horizontal_range.offset += base_offset;
        let mut vertical_range = append_arena_slice(img2_arena, vertical_words, storage_alignment);
        vertical_range.offset += base_offset;
        Ok((horizontal_range, vertical_range))
    }

    fn prepare_batch<'a>(
        &self,
        ops: &[PipelineOp],
        auxiliary_images: &[AuxiliaryImages],
        w: u32,
        h: u32,
        mode: u32,
        logical_mode: Option<&str>,
        contrast_mean: Option<u8>,
        f_resize_constant_bits: Option<u32>,
        f_resize_box_copy_is_exact: bool,
        f_resize_identity_is_exact: bool,
        f_resize_box_average_is_exact: bool,
        f_resize_dyadic_is_exact: bool,
        f_resize_f64_is_exact: bool,
        f_resize_f64_ordered_is_exact: bool,
        packed_luma_colorize: bool,
        packed_luma_convert: bool,
        packed_luma_transform_input: bool,
        packed_luma_transform_output: bool,
        native_rgb_compact_input: bool,
        native_rgb_packed_output: bool,
        native_la_transform_output: bool,
        packed_luma_point: bool,
        packed_luma_putdata: bool,
        packed_luma_order_statistic: bool,
        packed_native_byte_filter: bool,
        packed_luma_pad: bool,
        native_extract_band: bool,
        native_grayscale_rgb_input: bool,
        native_sharpness_l_input: bool,
        native_sharpness_la_input: bool,
        native_sharpness_rgb_input: bool,
        native_reduce_rgb_input: bool,
        native_expand_channels: Option<u8>,
        native_cover_input_channels: Option<u8>,
        native_cover_packed_output: Option<NativeCoverPackedOutputDispatch>,
        buffers: &'a mut BufferPool,
        auxiliary_cache: &GpuAuxiliaryCache,
    ) -> Result<PreparedGpuBatch<'a>, PilError> {
        if ops.len() != auxiliary_images.len() {
            return Err(PilError::InternalError(
                "GPU operation and auxiliary-image counts differ".into(),
            ));
        }

        // A full-frame RGBA composite has one already-packed auxiliary image.
        // Put its bytes directly into the reusable device arena instead of
        // first duplicating them into the temporary host-side u32 arena. The
        // specialized pair is deliberately narrow; all other image formats
        // and operation graphs retain the general pack-and-arena path.
        #[cfg(target_endian = "little")]
        let direct_alpha_composite_rgba = if can_fuse_gpu_alpha_composite_mirror(
            ops,
            0,
            logical_mode,
            (w, h),
        ) && w != 0
            && h != 0
        {
            match auxiliary_images
                .first()
                .and_then(|auxiliary| auxiliary.second.as_deref())
            {
                Some(DynamicImage::ImageRgba8(rgba)) => {
                    let expected_bytes = CheckedDims::new(w, h, 4)?.total_bytes();
                    (rgba.as_raw().len() == expected_bytes).then_some(rgba.as_raw().as_slice())
                }
                _ => None,
            }
        } else {
            None
        };
        #[cfg(target_endian = "big")]
        let direct_alpha_composite_rgba: Option<&[u8]> = None;

        let limits = self.device.limits();
        let uniform_alignment = limits.min_uniform_buffer_offset_alignment as usize;
        let storage_alignment = limits.min_storage_buffer_offset_alignment as usize;
        let mut scratch = BatchPreparationScratch::new(ops.len());
        let packed_output_plans = self.prepare_batch_packed_output_plans(
            BatchPackedOutputPlanningContext {
                ops,
                source_dimensions: (w, h),
                native_rgb_packed_output,
                native_la_transform_output,
                max_workgroups_per_dimension: limits.max_compute_workgroups_per_dimension,
                max_storage_buffer_binding_size: limits.max_storage_buffer_binding_size,
                max_buffer_size: limits.max_buffer_size,
            },
            buffers.capacity,
        )?;
        let native_rgb_packed_output = packed_output_plans.native_rgb;
        let native_la_transform_output = packed_output_plans.native_la;
        let pipeline_options = BatchPreparationPipelineOptions {
            logical_mode,
            packed_native_byte_filter,
            native_rgb_packed_output: native_rgb_packed_output.is_some(),
            native_la_transform_output: native_la_transform_output.is_some(),
            packed_luma_transform_output,
            packed_luma_point,
            packed_luma_order_statistic,
            native_sharpness_l_input,
            native_sharpness_la_input,
            native_sharpness_rgb_input,
        };
        let parameter_layouts = BatchParameterLayouts {
            packed_luma_colorize,
            packed_luma_convert,
            packed_luma_point,
            packed_luma_putdata,
            packed_luma_pad,
            packed_luma_transform_input,
            packed_luma_transform_output,
            native_extract_band,
            native_grayscale_rgb_input,
            native_sharpness_l_input,
            native_reduce_rgb_input,
            native_expand_channels,
            native_cover_input: native_cover_input_channels.is_some(),
            native_rgb_compact_input,
            native_rgb_packed_output,
            native_la_transform_output,
            native_expand_output: None,
            native_cover_packed_output,
        };
        let final_dims = self.prepare_batch_operations(
            BatchOperationPreparationOptions {
                ops,
                auxiliary_images,
                logical_mode,
                contrast_mean,
                resize_exactness: BatchResizeParameterExactness {
                    constant_bits: f_resize_constant_bits,
                    box_copy_is_exact: f_resize_box_copy_is_exact,
                    identity_is_exact: f_resize_identity_is_exact,
                    box_average_is_exact: f_resize_box_average_is_exact,
                    dyadic_is_exact: f_resize_dyadic_is_exact,
                    f64_is_exact: f_resize_f64_is_exact,
                    f64_ordered_is_exact: f_resize_f64_ordered_is_exact,
                },
                layouts: parameter_layouts,
                pipeline_options,
                packed_luma_putdata,
                direct_alpha_composite_rgba,
                uniform_alignment,
                storage_alignment,
                max_workgroups_per_dimension: limits.max_compute_workgroups_per_dimension,
                max_storage_buffer_binding_size: limits.max_storage_buffer_binding_size,
                max_buffer_size: limits.max_buffer_size,
                auxiliary_cache,
            },
            buffers,
            &mut scratch,
            (w, h, mode),
        )?;

        self.finalize_prepared_batch(
            buffers,
            auxiliary_cache,
            scratch.into_output(BatchPreparationOutputOptions {
                uniform_alignment,
                storage_alignment,
                direct_img2_bytes: direct_alpha_composite_rgba.unwrap_or_default(),
                native_rgb_packed_output,
                native_la_transform_output,
                native_cover_packed_output,
                final_dims,
            }),
        )
    }

    fn blur_pass_count(op: &PipelineOp) -> Option<usize> {
        match op {
            PipelineOp::BoxBlur { .. } => Some(1),
            PipelineOp::BoxBlurXY { passes, .. } => Some((*passes).max(1) as usize),
            PipelineOp::GaussianBlur { .. } => Some(3),
            _ => None,
        }
    }

    #[cfg(target_endian = "little")]
    fn execute_packed_rgb_transpose(
        &self,
        op: &PipelineOp,
        image: &RgbImage,
        layout: &PackedRgbTransposeLayout,
        buffers: &mut BufferPool,
    ) -> Result<DynamicImage, PilError> {
        self.execute_packed_rgb_transpose_with_upload(
            op,
            image,
            layout,
            buffers,
            self.direct_primary_readback,
        )
    }

    #[cfg(target_endian = "little")]
    fn upload_packed_rgb_mapped(
        &self,
        image: &RgbImage,
        layout: &PackedRgbTransposeLayout,
        buffers: &mut BufferPool,
    ) -> Result<(), PilError> {
        self.upload_native_bytes_mapped(
            image.as_raw(),
            layout.native_bytes,
            layout.transfer_bytes,
            buffers,
        )
    }

    #[cfg(target_endian = "little")]
    fn upload_packed_luma8_mapped(
        &self,
        image: &crate::raster::GrayImage,
        buffers: &mut BufferPool,
    ) -> Result<(), PilError> {
        let (width, height) = image.dimensions();
        let native_bytes = CheckedDims::new(width, height, 1)?.total_bytes();
        let transfer_bytes = compact_luma8_transfer_bytes(width, height)?;
        self.upload_native_bytes_mapped(image.as_raw(), native_bytes, transfer_bytes, buffers)
    }

    #[cfg(target_endian = "little")]
    fn upload_native_bytes_mapped(
        &self,
        bytes: &[u8],
        native_bytes: usize,
        transfer_bytes: u64,
        buffers: &mut BufferPool,
    ) -> Result<(), PilError> {
        if !self.direct_primary_readback {
            return Err(PilError::InternalError(
                "GPU mapped native upload requires supported shared Metal storage".into(),
            ));
        }
        if bytes.len() != native_bytes
            || transfer_bytes > u64::from(buffers.capacity) * 4
            || (native_bytes as u64).checked_add(3).map(|bytes| bytes & !3) != Some(transfer_bytes)
        {
            return Err(PilError::InternalError(
                "GPU mapped native upload does not fit its checked working set".into(),
            ));
        }
        let size = usize::try_from(transfer_bytes)
            .map_err(|_| PilError::InternalError("GPU mapped input size overflow".into()))?;
        let usage = wgpu::BufferUsages::MAP_WRITE | wgpu::BufferUsages::STORAGE;
        let input = buffers.mapped_native_input.get_or_insert_with(|| {
            ReusableGpuBuffer::new(&self.device, "gpu_mapped_native_input", usage, size, 4)
        });
        input.ensure_capacity(&self.device, "gpu_mapped_native_input", usage, size, 4);
        self.upload_bytes_to_mapped_buffer(
            &input.buffer,
            bytes,
            native_bytes,
            transfer_bytes,
            "GPU mapped native upload",
        )
    }

    #[cfg(target_endian = "little")]
    fn upload_bytes_to_mapped_buffer(
        &self,
        buffer: &wgpu::Buffer,
        bytes: &[u8],
        native_bytes: usize,
        transfer_bytes: u64,
        stage: &str,
    ) -> Result<(), PilError> {
        if !self.direct_primary_readback {
            return Err(PilError::InternalError(
                "GPU mapped native upload requires supported shared Metal storage".into(),
            ));
        }
        if bytes.len() != native_bytes
            || transfer_bytes > buffer.size()
            || (native_bytes as u64).checked_add(3).map(|bytes| bytes & !3) != Some(transfer_bytes)
        {
            return Err(PilError::InternalError(format!(
                "{stage} does not fit its checked mapped buffer"
            )));
        }
        let size = usize::try_from(transfer_bytes)
            .map_err(|_| PilError::InternalError("GPU mapped input size overflow".into()))?;
        // The batch owns this buffer exclusively. Recycled batches reached
        // output map completion, so their input has no outstanding GPU use.
        self.wait_for_buffer_mapping(transfer_bytes, buffer, wgpu::MapMode::Write, stage)?;
        let result = {
            let mut view = buffer.slice(..transfer_bytes).get_mapped_range_mut();
            // BufferViewMut's read dereference logs a slow-read warning even
            // for len(). We only write: obtain the writable slice explicitly
            // while preserving the checked mapping length and view lifetime.
            let mapped = view.as_mut();
            if mapped.len() != size {
                Err(PilError::InternalError(format!(
                    "{stage} has an unexpected mapped byte length"
                )))
            } else {
                mapped[..native_bytes].copy_from_slice(bytes);
                mapped[native_bytes..].fill(0);
                Ok(())
            }
        };
        // Drop the host view and unmap before any shader can reference it,
        // including the defensive error path above.
        buffer.unmap();
        result
    }
}

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

    fn execute_exact_host_result(
        &self,
        ops: &[PipelineOp],
        img: &DynamicImage,
        mode: Option<&str>,
        reason: &str,
    ) -> Result<DynamicImage, PilError> {
        // Keep receipts honest: this path is an exact semantic bridge, not a
        // native GPU implementation.  The marker is consumed by the outer
        // execution boundary and therefore reports the pixels as CPU-owned
        // even when the packed result is subsequently copied through a real
        // GPU dispatch for transport/parity coverage.
        crate::compute::record_pipeline_backend_fallback(&format!(
            "exact host semantic control: {reason}"
        ));
        let exact = crate::compute::CpuPool.execute_batch(ops, img, mode)?;

        // Keep the GPU parity lane honest when the result can use the
        // ordinary packed transport: the host implementation determines the
        // public pixels, then a real GPU Duplicate dispatch carries those
        // bytes through the selected device backend. Typed/empty results are
        // returned directly because the packed copy would narrow their
        // sample representation or require a zero-sized storage binding.
        if exact.width() == 0 || exact.height() == 0 || !gpu_image_layout_is_supported(&exact) {
            return Ok(exact);
        }
        let copied = self
            .execute_batch_with_policy(&[PipelineOp::Duplicate], &exact, None, false)
            .unwrap_or_else(|_| exact.clone());
        Ok(crate::image::preserve_mode(&exact, copied))
    }

    fn preflight_failure(
        &self,
        ops: &[PipelineOp],
        img: &DynamicImage,
        mode: Option<&str>,
        allow_cpu_fallback: bool,
        reason: &str,
    ) -> Result<DynamicImage, PilError> {
        if allow_cpu_fallback {
            crate::compute::record_pipeline_backend_fallback(reason);
            let cpu = crate::compute::CpuPool;
            return cpu.execute_batch(ops, img, mode);
        }

        // An explicitly locked GPU is a parity lane, not a permission to
        // manufacture a public NotImplementedError for a valid Pillow
        // operation.  The preflight guards below describe GPU storage and
        // shader limits; they do not describe the public operation's error
        // contract.  Run the same exact Rust operation here so invalid
        // inputs produce Pillow-compatible errors and valid inputs retain
        // their pixels while the native GPU implementation is completed.
        // Automatic routing still takes the normal CPU fallback branch above.
        self.execute_exact_host_result(ops, img, mode, reason)
    }

    fn execute_batch_with_policy(
        &self,
        ops: &[PipelineOp],
        img: &DynamicImage,
        mode: Option<&str>,
        allow_cpu_fallback: bool,
    ) -> Result<DynamicImage, PilError> {
        if ops.is_empty() {
            return Ok(img.clone());
        }
        let normalized = match self.normalize_batch_operations(ops, img, mode)? {
            BatchNormalization::Continue(normalized) => normalized,
            BatchNormalization::Return(image) => return Ok(image),
        };
        let NormalizedGpuBatch {
            native_cover_input_channels,
            native_sharpness_la_patch,
            input_layouts,
            dispatch_ops,
        } = normalized;
        let ops = dispatch_ops.as_slice();
        let BatchInputLayoutCandidates {
            packed_luma_colorize,
            packed_luma_convert,
            packed_luma_transform_input,
            native_rgb_compact_input: native_rgb_compact_input_candidate,
            native_la_transform_input,
            packed_luma_point,
            packed_luma_putdata,
            packed_luma_order_statistic,
            packed_native_byte_filter,
            packed_luma_pad,
            packed_la_pad,
        } = input_layouts;
        if let Some(result) =
            self.execute_batch_segments_if_needed(ops, img, mode, allow_cpu_fallback)?
        {
            return Ok(result);
        }

        let GpuBatchResizeProofs {
            constant_bits: f_resize_constant_bits,
            box_copy_is_exact: f_resize_box_copy_is_exact,
            identity_is_exact: f_resize_identity_is_exact,
            box_average_is_exact: f_resize_box_average_is_exact,
            pad_f64_is_exact: f_pad_f64_is_exact,
            dyadic_is_exact: f_resize_dyadic_is_exact,
            f64_is_exact: f_resize_f64_is_exact,
            f64_ordered_is_exact: f_resize_f64_ordered_is_exact,
        } = gpu_batch_resize_proofs(&dispatch_ops, img, mode);

        // Pillow defines both histogram operations as identity operations for
        // an empty image. There is no valid storage-buffer invocation to
        // issue for a zero-area image, so complete this already-validated
        // no-op without entering the device path or manufacturing a CPU
        // fallback receipt.
        if (img.width() == 0 || img.height() == 0)
            && ops.iter().all(|op| {
                matches!(
                    op,
                    PipelineOp::Autocontrast { .. }
                        | PipelineOp::Equalize
                        | PipelineOp::EqualizeMasked { .. }
                )
            })
        {
            crate::compute::record_pipeline_dispatch_count(0);
            return Ok(img.clone());
        }

        // Geometry operations whose public contract includes a reducing-gap,
        // fractional crop, or typed-sample rule must publish the exact host
        // result.  The helper still performs a real GPU copy for packed byte
        // results, so this is a controlled host/GPU boundary rather than an
        // operation-level capability outcome.
        if let Some(reason) = gpu_geometry_host_control_reason(
            ops,
            img,
            mode,
            f_resize_constant_bits,
            f_resize_box_copy_is_exact,
            f_resize_identity_is_exact,
            f_resize_box_average_is_exact,
            f_resize_dyadic_is_exact,
            f_resize_f64_is_exact || f_resize_f64_ordered_is_exact,
        ) {
            return self.execute_exact_host_result(ops, img, mode, reason);
        }

        let empty_two_input_noop = match self.preflight_batch_contract(
            ops,
            img,
            mode,
            allow_cpu_fallback,
            f_resize_constant_bits,
            f_pad_f64_is_exact,
        )? {
            BatchContractPreflight::Continue {
                empty_two_input_noop,
            } => empty_two_input_noop,
            BatchContractPreflight::Return(image) => return Ok(image),
        };

        // Distance one makes every EffectSpread displacement exactly zero.
        // Preserve its two RNG draws per pixel, but skip generating/uploading
        // the identity map and submitting a shader that only copies pixels.
        // Keep this after the ordinary GPU capability and dimension checks so
        // strict backend selection retains the same input contract.
        if matches!(ops, [PipelineOp::EffectSpread { distance: 1 }])
            && img.width() != 0
            && img.height() != 0
        {
            crate::compute::pool_cpu::ops::effects::effect_spread_advance_identity_rng(
                img.width(),
                img.height(),
            )?;
            crate::compute::record_pipeline_operation_path("native-identity");
            crate::compute::record_pipeline_dispatch_count(0);
            return Ok(img.clone());
        }

        // Resolve nested images before GPU work; a locked nested pipeline may
        // itself need the device and must surface its materialization errors.
        let auxiliary_images = resolve_batch_auxiliary_images(ops, img, mode)?;

        let contrast_mean = match self.preflight_resolved_batch_inputs(
            ops,
            img,
            &auxiliary_images,
            mode,
            allow_cpu_fallback,
            empty_two_input_noop,
        )? {
            BatchResolvedInputPreflight::Continue { contrast_mean } => contrast_mean,
            BatchResolvedInputPreflight::Return(image) => return Ok(image),
        };

        let gpu = Self::ensure_init()?;
        gpu.ensure_healthy("GPU batch start")?;
        let max_workgroups_per_dimension = gpu.device.limits().max_compute_workgroups_per_dimension;
        let packed_dispatch_candidates = GpuPackedDispatchCandidates {
            packed_luma_point,
            packed_luma_putdata,
            packed_luma_order_statistic,
            packed_native_byte_filter,
            packed_luma_pad,
            packed_la_pad,
        };
        let packed_dispatch_plan = plan_gpu_packed_dispatches(
            ops,
            img,
            mode,
            max_workgroups_per_dimension,
            packed_dispatch_candidates,
        );
        let packed_dispatch_plan = match self.preflight_packed_dispatches(
            ops,
            img,
            mode,
            allow_cpu_fallback,
            max_workgroups_per_dimension,
            packed_dispatch_plan,
        )? {
            BatchPackedDispatchPreflight::Continue(plan) => plan,
            BatchPackedDispatchPreflight::Return(image) => return Ok(image),
        };
        let GpuPackedDispatchPlan {
            packed_luma_point,
            packed_luma_putdata,
            packed_luma_order_statistic,
            packed_native_byte_filter,
            packed_luma_pad,
            packed_la_pad,
            ..
        } = packed_dispatch_plan;
        let image_pixels = CheckedDims::new(img.width(), img.height(), 1)?.total_pixels();
        let full_luma_putdata = packed_luma_putdata
            && matches!(ops, [PipelineOp::PutData { data, mode: PixelMode::L }]
                if data.len() == image_pixels);
        let limits = gpu.device.limits();
        #[cfg(target_endian = "little")]
        if matches!(mode, None | Some("RGB"))
            && matches!(img, DynamicImage::ImageRgb8(_))
            && let [
                PipelineOp::Crop {
                    left,
                    top,
                    right,
                    bottom,
                },
            ] = ops
            && let Some(dispatch) = plan_gpu_native_rgb_crop(
                img.width(),
                img.height(),
                *left,
                *top,
                *right,
                *bottom,
                limits.max_compute_workgroups_per_dimension,
                limits.max_storage_buffer_binding_size,
                limits.max_buffer_size,
            )
        {
            let mut buffers = gpu.acquire_buffers(dispatch.buffer_words)?;
            let result = gpu.execute_native_rgb_crop(img, *left, *top, dispatch, &mut buffers)?;
            gpu.recycle_buffers(buffers);
            return Ok(result);
        }
        #[cfg(target_endian = "little")]
        if let Some(bands) = gpu_native_rgb_luma_merge_bands(ops, img, mode)
            && let Some(dispatch) = plan_gpu_native_rgb_merge(
                img.width(),
                img.height(),
                limits.max_compute_workgroups_per_dimension,
                limits.max_storage_buffer_binding_size,
                limits.max_buffer_size,
            )
        {
            let mut buffers = gpu.acquire_buffers(dispatch.buffer_words)?;
            let result = gpu.execute_native_rgb_luma_merge(img, bands, dispatch, &mut buffers)?;
            gpu.recycle_buffers(buffers);
            return Ok(result);
        }
        // A compact native route can fit limits that the generic RGBA buffer
        // and 16x16 dispatch checks reject. Its planner must prove every
        // adapter and global image limit before it bypasses those checks.
        #[cfg(target_endian = "little")]
        let native_masked_byte_paste = (|| {
            let ([op], [auxiliary]) = (ops, auxiliary_images.as_slice()) else {
                return None;
            };
            let (Some(source_pixels), Some(mask_pixels)) =
                (auxiliary.second.as_deref(), auxiliary.third.as_deref())
            else {
                return None;
            };
            let (offset, bytes_per_pixel, logical_mode) =
                gpu_native_masked_byte_paste_layout(op, img, source_pixels, mask_pixels, mode)?;
            let dispatch = plan_gpu_native_masked_byte_paste(
                img.width(),
                img.height(),
                source_pixels.width(),
                source_pixels.height(),
                mask_pixels.width(),
                mask_pixels.height(),
                bytes_per_pixel,
                limits.max_compute_workgroups_per_dimension,
                limits.max_storage_buffer_binding_size,
                limits.max_buffer_size,
            )?;
            Some((source_pixels, mask_pixels, offset, logical_mode, dispatch))
        })();
        #[cfg(not(target_endian = "little"))]
        let native_masked_byte_paste: Option<(
            &DynamicImage,
            &DynamicImage,
            (i32, i32),
            &'static str,
            NativeMaskedBytePasteDispatch,
        )> = None;

        #[cfg(target_endian = "little")]
        let native_hsv_pad = plan_native_hsv_pad_dispatch(
            ops,
            img,
            mode,
            limits.max_compute_workgroups_per_dimension,
            limits.max_storage_buffer_binding_size,
            limits.max_buffer_size,
        );
        #[cfg(not(target_endian = "little"))]
        let native_hsv_pad: Option<()> = None;

        if native_masked_byte_paste.is_none()
            && native_hsv_pad.is_none()
            && !packed_luma_order_statistic
            && gpu_dispatch_dimensions_require_cpu(
                ops,
                img.dimensions(),
                limits.max_compute_workgroups_per_dimension,
                mode,
                packed_luma_pad,
            )
        {
            gpu_log!("[GPU] dispatch preflight routed batch to CPU: adapter workgroup limit");
            return self.preflight_failure(
                ops,
                img,
                mode,
                allow_cpu_fallback,
                "adapter workgroup limit",
            );
        }
        if let Some((source_pixels, mask_pixels, offset, logical_mode, dispatch)) =
            native_masked_byte_paste
        {
            let mut buffers = gpu.acquire_buffers(dispatch.word_count)?;
            let result = gpu.execute_native_masked_byte_paste(
                img,
                source_pixels,
                mask_pixels,
                logical_mode,
                offset,
                dispatch,
                &mut buffers,
            )?;
            gpu.recycle_buffers(buffers);
            return Ok(result);
        }
        #[cfg(target_endian = "little")]
        if let Some(dispatch) = native_hsv_pad {
            let mut buffers = gpu.acquire_buffers(dispatch.output.output_word_count)?;
            let result = gpu.execute_native_hsv_pad(img, dispatch, &mut buffers)?;
            gpu.recycle_buffers(buffers);
            return Ok(result);
        }

        let capacity = gpu_batch_capacity(ops, img, &auxiliary_images, mode)?;
        let packed_luma_transform_output = packed_luma_transform_input
            && plan_native_luma_transform_output(
                ops,
                limits.max_compute_workgroups_per_dimension,
                limits.max_storage_buffer_binding_size,
                limits.max_buffer_size,
                capacity,
            )
            .is_some();
        let native_rgb_output_plan = if matches!(
            ops,
            [PipelineOp::Eval { .. }
                | PipelineOp::Equalize
                | PipelineOp::MedianFilter { size: 3 }
                | PipelineOp::MinFilter { size: 3 }
                | PipelineOp::MaxFilter { size: 3 }
                | PipelineOp::BoxBlur { radius: 1 }]
        ) {
            plan_native_rgb_point_output(
                img.width(),
                img.height(),
                limits.max_compute_workgroups_per_dimension,
                limits.max_storage_buffer_binding_size,
                limits.max_buffer_size,
                capacity,
            )
        } else {
            plan_native_rgb_transform_output(
                ops,
                limits.max_compute_workgroups_per_dimension,
                limits.max_storage_buffer_binding_size,
                limits.max_buffer_size,
                capacity,
            )
        };
        let native_rgb_packed_output =
            native_rgb_compact_input_candidate && native_rgb_output_plan.is_some();
        // Native RGB point, equalize, and 3x3 order filters require the
        // compact output plan as well as compact input. Otherwise keep both
        // transfer boundaries on the standard RGBA contract.
        let native_rgb_compact_input = native_rgb_compact_input_candidate
            && (!matches!(
                ops,
                [PipelineOp::Eval { .. }
                    | PipelineOp::Equalize
                    | PipelineOp::MedianFilter { size: 3 }
                    | PipelineOp::MinFilter { size: 3 }
                    | PipelineOp::MaxFilter { size: 3 }
                    | PipelineOp::BoxBlur { radius: 1 }]
            ) || native_rgb_packed_output);
        let native_la_transform_output = native_la_transform_input
            && plan_native_la_transform_output(
                ops,
                limits.max_compute_workgroups_per_dimension,
                limits.max_storage_buffer_binding_size,
                limits.max_buffer_size,
                capacity,
            )
            .is_some();
        let native_cover_packed_output = gpu_native_cover_packed_output_dispatch(
            ops,
            native_cover_input_channels,
            mode,
            img.height(),
            limits.max_compute_workgroups_per_dimension,
            limits.max_storage_buffer_binding_size,
            limits.max_buffer_size,
            capacity,
        );
        if gpu_buffer_capacity_exceeds_limits(
            capacity,
            limits.max_storage_buffer_binding_size,
            limits.max_buffer_size,
        ) {
            gpu_log!(
                "[GPU] dispatch preflight routed batch to CPU: image buffer exceeds adapter limits"
            );
            return self.preflight_failure(
                ops,
                img,
                mode,
                allow_cpu_fallback,
                "image buffer exceeds adapter limits",
            );
        }
        #[cfg(target_endian = "little")]
        if let Some(result) =
            gpu.try_execute_native_batch_routes(ops, img, mode, &auxiliary_images, &limits)?
        {
            return Ok(result);
        }

        #[cfg(target_endian = "little")]
        match gpu.try_execute_native_flip(ops, img, mode, &limits, capacity)? {
            NativeGeometryDispatchOutcome::NotApplicable => {}
            NativeGeometryDispatchOutcome::ExceedsLimits(reason) => {
                return self.preflight_failure(ops, img, mode, allow_cpu_fallback, reason);
            }
            NativeGeometryDispatchOutcome::Executed(result) => return Ok(result),
        }

        #[cfg(target_endian = "little")]
        match gpu.try_execute_native_transpose_paths(ops, img, mode, &limits, capacity)? {
            NativeGeometryDispatchOutcome::NotApplicable => {}
            NativeGeometryDispatchOutcome::ExceedsLimits(reason) => {
                return self.preflight_failure(ops, img, mode, allow_cpu_fallback, reason);
            }
            NativeGeometryDispatchOutcome::Executed(result) => return Ok(result),
        }

        let mut buffers = gpu.acquire_buffers(capacity)?;
        let native_luma16 = gpu_luma16_geometry_is_supported(ops, img, mode);
        let native_luma16_convert = gpu_luma16_convert_is_supported(ops, img);
        let native_luma16_paste = gpu_luma16_paste_is_supported(ops, img);
        let (w, h) = img.dimensions();
        let mcode = execution_mode_code(img, mode);
        let op_keys: Vec<&str> = ops.iter().map(|op| registry::variant_key(op)).collect();
        log::debug!(
            "[GPU] {} op(s) {}x{} mode={}: {:?}",
            ops.len(),
            w,
            h,
            mcode,
            op_keys
        );
        let native_extract_band_channels = gpu_native_extract_band_channels(ops, img, mode);
        let native_extract_band = native_extract_band_channels.is_some();
        let output_only_constant = matches!(ops, [PipelineOp::Constant { .. }]);
        let native_grayscale_rgb_input = gpu_native_grayscale_rgb_input(ops, img, mode);
        let native_grayscale_cmyk_input = gpu_native_grayscale_cmyk_input(ops, img, mode);
        let native_sharpness_l_input = gpu_native_sharpness_l_input(ops, img, mode);
        let native_sharpness_la_input = gpu_native_sharpness_la_input(ops, img, mode);
        let native_sharpness_rgb_input = gpu_native_sharpness_rgb_input(ops, img, mode);
        let native_reduce_rgb_input = gpu_native_reduce_rgb_input(ops, img, mode);
        let native_expand_channels = gpu_native_expand_channels(ops, img, mode);
        #[cfg(target_endian = "little")]
        let mapped_native_box_blur_input = gpu.direct_primary_readback
            && matches!(ops, [PipelineOp::BoxBlur { radius: 1 }])
            && ((packed_native_byte_filter
                && matches!(mode, None | Some("L"))
                && matches!(img, DynamicImage::ImageLuma8(image)
                    if image.as_raw().len() == (w as usize).saturating_mul(h as usize)))
                || (native_rgb_compact_input
                    && matches!(mode, None | Some("RGB"))
                    && matches!(img, DynamicImage::ImageRgb8(image)
                        if image.as_raw().len() == (w as usize).saturating_mul(h as usize).saturating_mul(3))));
        #[cfg(not(target_endian = "little"))]
        let mapped_native_box_blur_input = false;
        #[cfg(target_endian = "little")]
        let mapped_native_grayscale_input = gpu.direct_primary_readback
            && (native_grayscale_rgb_input || native_grayscale_cmyk_input);
        #[cfg(not(target_endian = "little"))]
        let mapped_native_grayscale_input = false;
        gpu_log!(
            "[GPU] step=upload start output_only_constant={output_only_constant} native_luma16={native_luma16} native_luma16_convert={native_luma16_convert} native_luma16_paste={native_luma16_paste}"
        );
        gpu.upload_batch_input(
            img,
            &mut buffers,
            GpuBatchUploadOptions {
                mode,
                native_luma16,
                native_luma16_convert,
                native_luma16_paste,
                full_luma_putdata,
                output_only_constant,
                packed_la_pad,
                packed_native_byte_filter,
                packed_luma_colorize,
                packed_luma_convert,
                packed_luma_transform_input,
                packed_luma_point,
                packed_luma_putdata,
                packed_luma_order_statistic,
                packed_luma_pad,
                native_la_transform_output,
                native_rgb_compact_input,
                native_extract_band_channels,
                native_grayscale_rgb_input,
                native_grayscale_cmyk_input,
                native_sharpness_l_input,
                native_sharpness_la_input,
                native_sharpness_la_patch,
                native_sharpness_rgb_input,
                native_reduce_rgb_input,
                native_expand_channels,
                native_cover_input_channels,
                mapped_native_box_blur_input,
                mapped_native_grayscale_input,
            },
        )?;
        gpu_log!(
            "[GPU] step=upload done output_only_constant={output_only_constant} native_luma16={native_luma16}"
        );
        gpu_log!("[GPU] step=execute_batch_impl start");
        let mapped_native_input = if mapped_native_box_blur_input || mapped_native_grayscale_input {
            Some(buffers.mapped_native_input.take().ok_or_else(|| {
                PilError::InternalError(
                    "mapped native upload completed without retaining its input buffer".into(),
                )
            })?)
        } else {
            None
        };
        let initial_input = mapped_native_input.as_ref().map(|input| &input.buffer);
        let (
            final_is_a,
            final_w,
            final_h,
            readback,
            mut resource_telemetry,
            dispatch_count,
            native_expand_output,
        ) = gpu.execute_batch_impl(
            ops,
            &auxiliary_images,
            w,
            h,
            mcode,
            packed_luma_colorize,
            packed_luma_convert,
            packed_luma_transform_input,
            packed_luma_transform_output,
            native_rgb_compact_input,
            native_rgb_packed_output,
            native_la_transform_output,
            packed_luma_point,
            packed_luma_putdata,
            packed_luma_order_statistic,
            packed_native_byte_filter,
            packed_luma_pad,
            packed_la_pad,
            native_extract_band,
            native_grayscale_rgb_input,
            native_sharpness_l_input,
            native_sharpness_la_input,
            native_sharpness_rgb_input,
            native_reduce_rgb_input,
            native_expand_channels,
            native_cover_input_channels,
            native_cover_packed_output,
            mode,
            contrast_mean,
            f_resize_constant_bits,
            f_resize_box_copy_is_exact,
            f_resize_identity_is_exact,
            f_resize_box_average_is_exact,
            f_resize_dyadic_is_exact,
            f_resize_f64_is_exact,
            f_resize_f64_ordered_is_exact,
            initial_input,
            &mut buffers,
        )?;
        // The ordinary image route shares the same fused shader resolver as
        // explicit GPU batches, but its per-operation telemetry was not
        // carrying the fusion count. Keep the receipt aligned with the
        // admitted dispatch plan without counting an unrelated CPU fallback.
        resource_telemetry.fused_operation_count = resource_telemetry
            .fused_operation_count
            .max(gpu_fused_operation_count(ops, mode, (w, h)));
        if let Some(input) = mapped_native_input {
            buffers.mapped_native_input = Some(input);
        }
        gpu_log!(
            "[GPU] step=execute_batch_impl done final=({},{}) is_a={}",
            final_w,
            final_h,
            final_is_a
        );
        let output_layout = GpuBatchOutputLayout::from_operations(ops, img.color());
        // All command buffers are submitted before registering the mapping. The
        // batch owns A/B exclusively until the mapped view is dropped and the
        // selected primary or staging buffer has been unmapped.
        gpu_log!("[GPU] step=readback start");
        let readback_buffer = readback.buffer(&buffers, final_is_a);
        let result = if let Some(dispatch) = native_cover_packed_output {
            gpu.readback_to_cover_native_bytes(final_w, final_h, dispatch, readback_buffer)?
        } else if native_rgb_packed_output {
            gpu.readback_to_native_channels(final_w, final_h, 3, readback_buffer)?
        } else if packed_la_pad {
            gpu.readback_to_native_channels(final_w, final_h, 2, readback_buffer)?
        } else if native_la_transform_output {
            gpu.readback_to_native_channels(final_w, final_h, 2, readback_buffer)?
        } else if packed_native_byte_filter && matches!(mode, Some("LA")) {
            gpu.readback_to_native_channels(final_w, final_h, 2, readback_buffer)?
        } else if native_luma16 {
            gpu.readback_to_luma16(final_w, final_h, readback_buffer, mode)?
        } else if native_luma16_paste {
            gpu.readback_to_luma16_numeric(final_w, final_h, readback_buffer)?
        } else if output_layout.native_luma8_extract
            || output_layout.native_luma8_grayscale
            || output_layout.native_luma8_constant
            || packed_luma_transform_output
            || packed_luma_point
            || packed_luma_putdata
            || packed_luma_order_statistic
            || packed_native_byte_filter
            || packed_luma_pad
            || native_sharpness_l_input
        {
            gpu.readback_to_luma8(final_w, final_h, readback_buffer)?
        } else if native_sharpness_la_input {
            gpu.readback_to_luma_alpha_from_rgba_transport(final_w, final_h, readback_buffer)?
        } else {
            if native_expand_output {
                let channels = native_expand_channels.ok_or_else(|| {
                    PilError::InternalError(
                        "GPU native Expand output is missing its channel count".into(),
                    )
                })?;
                gpu.readback_to_native_channels(final_w, final_h, channels, readback_buffer)?
            } else {
                gpu.readback_to_image(final_w, final_h, readback_buffer, output_layout.native_rgb)?
            }
        };
        gpu_log!("[GPU] step=readback done");
        // map_async completion proves the command buffer no longer uses the
        // working images. Return successful working sets to the bounded pool;
        // every error path drops its buffers instead of risking reuse of an
        // in-flight or device-invalid resource.
        resource_telemetry = gpu_batch_resource_telemetry(
            resource_telemetry,
            &buffers,
            &readback,
            img,
            GpuBatchResourceTelemetryOptions {
                input_dimensions: (w, h),
                output_dimensions: (final_w, final_h),
                mode,
                output_layout: &output_layout,
                output_only_constant,
                full_luma_putdata,
                native_extract_band_channels,
                native_grayscale_rgb_input,
                native_rgb_compact_input,
                native_sharpness_l_input,
                packed_native_byte_filter,
                packed_la_pad,
                packed_luma_pad,
                native_sharpness_la_input,
                native_la_transform_output,
                native_sharpness_rgb_input,
                native_reduce_rgb_input,
                native_expand_channels,
                native_cover_input_channels,
                packed_luma_colorize,
                packed_luma_convert,
                packed_luma_transform_input,
                packed_luma_point,
                packed_luma_putdata,
                packed_luma_order_statistic,
                native_luma16_convert,
                native_luma16_paste,
                native_cover_packed_output,
                native_rgb_packed_output,
                packed_luma_transform_output,
                native_expand_output,
            },
        )?;
        crate::compute::record_pipeline_resource_telemetry(resource_telemetry);
        crate::compute::record_pipeline_dispatch_count(dispatch_count);
        if let ReadbackTarget::Staging(staging) = readback {
            gpu.recycle_staging(staging);
        }
        gpu.recycle_buffers(buffers);
        if output_layout.native_rgb
            || native_la_transform_output
            || native_sharpness_l_input
            || native_sharpness_la_input
            || native_cover_packed_output.is_some()
            || packed_luma_pad
            || packed_la_pad
        {
            // The final native layout was decoded directly from the mapping;
            // applying mode preservation again would reallocate it.
            return Ok(result);
        }
        if output_layout.native_luma8_extract
            || output_layout.native_luma8_grayscale
            || output_layout.native_luma8_constant
            || packed_luma_transform_output
            || packed_luma_point
            || packed_luma_putdata
            || packed_luma_order_statistic
            || packed_native_byte_filter
            || packed_luma_pad
            || packed_la_pad
        {
            return Ok(result);
        }
        if let Some(mode) = output_layout.put_alpha_mode {
            return put_alpha_output(result, mode);
        }
        if let Some(ct) = output_layout.output_color_type {
            // Bypass preserve_mode — use the override color type directly.
            return gpu_result_as_color_type(result, ct);
        } else {
            Ok(crate::image::preserve_mode(img, result))
        }
    }
}

#[cfg(test)]
mod tests;
