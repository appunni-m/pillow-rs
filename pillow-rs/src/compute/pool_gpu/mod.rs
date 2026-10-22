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
/// Explicit bounded GPU scheduling; ordinary image routing stays independent.
pub(crate) mod stream;
use native_dispatch::*;
mod resource_pool;
use resource_pool::*;
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

fn projection_source_is_zero(source: &[u8]) -> bool {
    let zero = u8x16::splat(0);
    let mut blocks = source.chunks_exact(64);
    for block in blocks.by_ref() {
        let mut combined = zero;
        for samples in block.chunks_exact(16) {
            let lanes: [u8; 16] = samples
                .try_into()
                .expect("projection SIMD zero check has sixteen-byte lanes");
            combined |= u8x16::new(lanes);
        }
        if combined != zero {
            return false;
        }
    }

    let remainder = blocks.remainder();
    let mut tail = remainder.chunks_exact(16);
    let mut combined = zero;
    for samples in tail.by_ref() {
        let lanes: [u8; 16] = samples
            .try_into()
            .expect("projection SIMD zero check has sixteen-byte lanes");
        combined |= u8x16::new(lanes);
    }
    combined == zero && tail.remainder().iter().all(|&byte| byte == 0)
}
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

const NATIVE_EXPAND_WORKGROUP_WIDTH: u64 = 16;
const NATIVE_EXPAND_WORKGROUP_HEIGHT: u64 = 16;
const NATIVE_COVER_PACKED_OUTPUT_MARKER: u32 = u32::MAX - 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct NativeCoverPackedOutputDispatch {
    channels: u8,
    groups_x: u32,
    groups_y: u32,
    words_per_row: u32,
    word_count: u32,
    transfer_bytes: u64,
    horizontal_groups_x: u32,
    horizontal_groups_y: u32,
    horizontal_words_per_row: u32,
    horizontal_word_count: u32,
    horizontal_transfer_bytes: u64,
}

/// Plan one vertical-resize invocation per row-packed group of four native
/// bytes. L stores four pixels per word; LA stores two pixels per word. Every
/// row has its own four-byte alignment, so the final partial word is stripped
/// when the mapped rows are decoded.
fn plan_native_cover_packed_output_dispatch(
    width: u32,
    height: u32,
    source_height: u32,
    channels: u8,
    max_workgroups_per_dimension: u32,
    max_storage_buffer_binding_size: u32,
    max_buffer_size: u64,
    output_buffer_bytes: u64,
) -> Option<NativeCoverPackedOutputDispatch> {
    if width == 0
        || height == 0
        || source_height == 0
        || !matches!(channels, 1 | 2)
        || max_workgroups_per_dimension == 0
    {
        return None;
    }
    let pixels = u64::from(width).checked_mul(u64::from(height))?;
    if pixels > u64::from(u32::MAX) {
        return None;
    }
    let pixels_per_word = 4 / u32::from(channels);
    let words_per_row = width.div_ceil(pixels_per_word);
    let word_count = u64::from(words_per_row).checked_mul(u64::from(height))?;
    if word_count == 0 || word_count > u64::from(u32::MAX) {
        return None;
    }
    let transfer_bytes = word_count.checked_mul(std::mem::size_of::<u32>() as u64)?;
    let horizontal_word_count = u64::from(words_per_row).checked_mul(u64::from(source_height))?;
    if horizontal_word_count == 0 || horizontal_word_count > u64::from(u32::MAX) {
        return None;
    }
    let horizontal_transfer_bytes =
        horizontal_word_count.checked_mul(std::mem::size_of::<u32>() as u64)?;
    if transfer_bytes > u64::from(max_storage_buffer_binding_size)
        || transfer_bytes > max_buffer_size
        || transfer_bytes > output_buffer_bytes
        || horizontal_transfer_bytes > u64::from(max_storage_buffer_binding_size)
        || horizontal_transfer_bytes > max_buffer_size
        || horizontal_transfer_bytes > output_buffer_bytes
        || usize::try_from(transfer_bytes).is_err()
        || usize::try_from(horizontal_transfer_bytes).is_err()
    {
        return None;
    }
    let groups_x = words_per_row.div_ceil(16);
    let groups_y = height.div_ceil(16);
    let horizontal_groups_x = words_per_row.div_ceil(16);
    let horizontal_groups_y = source_height.div_ceil(16);
    if groups_x > max_workgroups_per_dimension
        || groups_y > max_workgroups_per_dimension
        || horizontal_groups_x > max_workgroups_per_dimension
        || horizontal_groups_y > max_workgroups_per_dimension
    {
        return None;
    }
    Some(NativeCoverPackedOutputDispatch {
        channels,
        groups_x,
        groups_y,
        words_per_row,
        word_count: u32::try_from(word_count).ok()?,
        transfer_bytes,
        horizontal_groups_x,
        horizontal_groups_y,
        horizontal_words_per_row: words_per_row,
        horizontal_word_count: u32::try_from(horizontal_word_count).ok()?,
        horizontal_transfer_bytes,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct NativeExpandOutputDispatch {
    groups_x: u32,
    groups_y: u32,
    words_per_row: u32,
    word_count: u32,
    transfer_bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct NativePackedChannelOutputDispatch {
    groups_x: u32,
    groups_y: u32,
    row_tiled: bool,
    output_word_count: u32,
    transfer_bytes: u64,
}

/// Plan one output invocation per packed word. Each 16×16 workgroup owns 256
/// distinct words; the flattened 2D grid never assigns two invocations to the
/// same word, including when the adapter's X limit requires a second row.
fn plan_native_expand_output_dispatch(
    output_bytes: u64,
    max_workgroups_per_dimension: u32,
) -> Option<NativeExpandOutputDispatch> {
    if output_bytes == 0 || max_workgroups_per_dimension == 0 {
        return None;
    }
    let word_count = output_bytes.checked_add(3)?.checked_div(4)?;
    let workgroup_items =
        NATIVE_EXPAND_WORKGROUP_WIDTH.checked_mul(NATIVE_EXPAND_WORKGROUP_HEIGHT)?;
    let required_workgroups = word_count.div_ceil(workgroup_items);
    let max_groups = u64::from(max_workgroups_per_dimension);
    let groups_x = required_workgroups.min(max_groups);
    let groups_y = required_workgroups.div_ceil(groups_x);
    if groups_y > max_groups {
        return None;
    }
    let words_per_row = groups_x.checked_mul(NATIVE_EXPAND_WORKGROUP_WIDTH)?;
    let transfer_bytes = word_count.checked_mul(4)?;
    Some(NativeExpandOutputDispatch {
        groups_x: u32::try_from(groups_x).ok()?,
        groups_y: u32::try_from(groups_y).ok()?,
        words_per_row: u32::try_from(words_per_row).ok()?,
        word_count: u32::try_from(word_count).ok()?,
        transfer_bytes,
    })
}

/// Plan a compact native-L affine-nearest output. The dedicated shader writes
/// four logical pixels to each storage word, so its bounds are the packed
/// readback size and its signed coordinate walk must fit i32.
fn plan_native_luma_transform_output(
    ops: &[PipelineOp],
    max_workgroups_per_dimension: u32,
    max_storage_buffer_binding_size: u32,
    max_buffer_size: u64,
    capacity_words: u32,
) -> Option<NativeExpandOutputDispatch> {
    let [
        PipelineOp::Transform {
            w,
            h,
            method: TransformMethod::Affine,
            data,
            filter: ResampleFilter::Nearest,
            ..
        },
    ] = ops
    else {
        return None;
    };
    if *w == 0 || *h == 0 || *w > i32::MAX as u32 || *h > i32::MAX as u32 {
        return None;
    }
    let affine: [f64; 6] = data.get(..6)?.try_into().ok()?;
    let [step_x, step_y, origin_x, step_x_y, step_y_y, origin_y] =
        crate::compute::pool_cpu::ops::effects::affine_nearest_fixed(affine, *w, *h)?;
    let max_x = i128::from(*w - 1);
    let max_y = i128::from(*h - 1);
    let coords_fit = |step_x: i64, step_y: i64, origin: i64| {
        let step_x = i128::from(step_x);
        let step_y = i128::from(step_y);
        let origin = i128::from(origin);
        [
            origin,
            origin + max_x * step_x,
            origin + max_y * step_y,
            origin + max_x * step_x + max_y * step_y,
        ]
        .into_iter()
        .all(|value| i32::try_from(value).is_ok())
            && max_x * step_x <= i128::from(i32::MAX)
            && max_x * step_x >= i128::from(i32::MIN)
            && max_y * step_y <= i128::from(i32::MAX)
            && max_y * step_y >= i128::from(i32::MIN)
    };
    if !coords_fit(step_x, step_y, origin_x) || !coords_fit(step_x_y, step_y_y, origin_y) {
        return None;
    }
    let pixels = u64::try_from(CheckedDims::new(*w, *h, 1).ok()?.total_pixels()).ok()?;
    // The shader forms the flat pixel count in u32 before deriving packed
    // output words. Keep that multiplication representable on device.
    if pixels > u64::from(u32::MAX) {
        return None;
    }
    let dispatch = plan_native_expand_output_dispatch(pixels, max_workgroups_per_dimension)?;
    let capacity_bytes = u64::from(capacity_words).checked_mul(4)?;
    (dispatch.transfer_bytes <= capacity_bytes
        && dispatch.transfer_bytes <= u64::from(max_storage_buffer_binding_size)
        && dispatch.transfer_bytes <= max_buffer_size)
        .then_some(dispatch)
}

/// Plan an affine-nearest transform that writes packed native byte channels.
/// One shader invocation samples four pixels and owns the words they occupy,
/// avoiding an RGBA-sized readback for RGB and LA.
fn plan_native_transform_output(
    ops: &[PipelineOp],
    output_channels: u32,
    max_workgroups_per_dimension: u32,
    max_storage_buffer_binding_size: u32,
    max_buffer_size: u64,
    capacity_words: u32,
) -> Option<NativePackedChannelOutputDispatch> {
    if !matches!(output_channels, 2 | 3) {
        return None;
    }
    let [
        PipelineOp::Transform {
            w,
            h,
            method: TransformMethod::Affine,
            data,
            filter: ResampleFilter::Nearest,
            ..
        },
    ] = ops
    else {
        return None;
    };
    if *w == 0 || *h == 0 || *w > i32::MAX as u32 || *h > i32::MAX as u32 {
        return None;
    }
    let affine: [f64; 6] = data.get(..6)?.try_into().ok()?;
    let [step_x, step_y, origin_x, step_x_y, step_y_y, origin_y] =
        crate::compute::pool_cpu::ops::effects::affine_nearest_fixed(affine, *w, *h)?;
    let max_x = i128::from(*w - 1);
    let max_y = i128::from(*h - 1);
    let coords_fit = |step_x: i64, step_y: i64, origin: i64| {
        let step_x = i128::from(step_x);
        let step_y = i128::from(step_y);
        let origin = i128::from(origin);
        let x_offsets = [0, max_x * step_x];
        let partials = x_offsets.map(|x| origin + x);
        partials
            .into_iter()
            .all(|value| i32::try_from(value).is_ok())
            && [
                origin,
                origin + max_x * step_x,
                origin + max_y * step_y,
                origin + max_x * step_x + max_y * step_y,
            ]
            .into_iter()
            .all(|value| i32::try_from(value).is_ok())
            && max_x * step_x <= i128::from(i32::MAX)
            && max_x * step_x >= i128::from(i32::MIN)
            && max_y * step_y <= i128::from(i32::MAX)
            && max_y * step_y >= i128::from(i32::MIN)
    };
    if !coords_fit(step_x, step_y, origin_x) || !coords_fit(step_x_y, step_y_y, origin_y) {
        return None;
    }

    let pixels = u64::try_from(CheckedDims::new(*w, *h, 1).ok()?.total_pixels()).ok()?;
    let output_channels = u8::try_from(output_channels).ok()?;
    let output_bytes = u64::try_from(
        CheckedDims::new(*w, *h, output_channels)
            .ok()?
            .total_bytes(),
    )
    .ok()?;
    if pixels > u64::from(u32::MAX) || output_bytes > u64::from(u32::MAX) {
        return None;
    }
    let transfer_bytes = output_bytes
        .checked_add(3)?
        .checked_div(4)?
        .checked_mul(4)?;
    let output_word_count = u32::try_from(transfer_bytes / 4).ok()?;
    let output_group_count = pixels.div_ceil(4);
    let workgroup_items =
        NATIVE_EXPAND_WORKGROUP_WIDTH.checked_mul(NATIVE_EXPAND_WORKGROUP_HEIGHT)?;
    let required_workgroups = output_group_count.div_ceil(workgroup_items);
    let max_groups = u64::from(max_workgroups_per_dimension);
    if max_groups == 0 {
        return None;
    }
    let row_groups_x =
        (*w % 4 == 0).then(|| u64::from(*w / 4).div_ceil(NATIVE_EXPAND_WORKGROUP_WIDTH));
    let row_groups_y = u64::from(*h).div_ceil(NATIVE_EXPAND_WORKGROUP_HEIGHT);
    let row_tiled_workgroups = row_groups_x.and_then(|groups_x| groups_x.checked_mul(row_groups_y));
    let row_tiled = row_groups_x.is_some_and(|groups_x| {
        groups_x <= max_groups
            && row_groups_y <= max_groups
            && row_tiled_workgroups.is_some_and(|count| count <= required_workgroups)
    });
    let (groups_x, groups_y) = if row_tiled {
        (row_groups_x?, row_groups_y)
    } else {
        let groups_x = required_workgroups.min(max_groups);
        let groups_y = required_workgroups.div_ceil(groups_x);
        if groups_y > max_groups {
            return None;
        }
        (groups_x, groups_y)
    };
    let capacity_bytes = u64::from(capacity_words).checked_mul(4)?;
    (transfer_bytes <= capacity_bytes
        && transfer_bytes <= u64::from(max_storage_buffer_binding_size)
        && transfer_bytes <= max_buffer_size)
        .then_some(NativePackedChannelOutputDispatch {
            groups_x: u32::try_from(groups_x).ok()?,
            groups_y: u32::try_from(groups_y).ok()?,
            row_tiled,
            output_word_count,
            transfer_bytes,
        })
}

fn plan_native_rgb_transform_output(
    ops: &[PipelineOp],
    max_workgroups_per_dimension: u32,
    max_storage_buffer_binding_size: u32,
    max_buffer_size: u64,
    capacity_words: u32,
) -> Option<NativePackedChannelOutputDispatch> {
    plan_native_transform_output(
        ops,
        3,
        max_workgroups_per_dimension,
        max_storage_buffer_binding_size,
        max_buffer_size,
        capacity_words,
    )
}

/// Plan the native RGB point/LUT path. Each invocation remaps four RGB
/// pixels and owns the three u32 words containing their 12 output bytes.
/// The packed output preserves RGB channel order while avoiding RGBA staging.
fn plan_native_rgb_point_output(
    width: u32,
    height: u32,
    max_workgroups_per_dimension: u32,
    max_storage_buffer_binding_size: u32,
    max_buffer_size: u64,
    capacity_words: u32,
) -> Option<NativePackedChannelOutputDispatch> {
    if width == 0 || height == 0 || max_workgroups_per_dimension == 0 {
        return None;
    }
    let pixels = u64::from(width).checked_mul(u64::from(height))?;
    if pixels > u64::from(u32::MAX) {
        return None;
    }
    let output_bytes =
        u64::try_from(CheckedDims::new(width, height, 3).ok()?.total_bytes()).ok()?;
    if output_bytes > u64::from(u32::MAX) {
        return None;
    }
    let transfer_bytes = output_bytes
        .checked_add(3)?
        .checked_div(4)?
        .checked_mul(4)?;
    let output_word_count = u32::try_from(transfer_bytes / 4).ok()?;
    let output_group_count = pixels.div_ceil(4);
    let workgroup_items =
        NATIVE_EXPAND_WORKGROUP_WIDTH.checked_mul(NATIVE_EXPAND_WORKGROUP_HEIGHT)?;
    let required_workgroups = output_group_count.div_ceil(workgroup_items);
    let max_groups = u64::from(max_workgroups_per_dimension);
    let row_groups_x =
        (width % 4 == 0).then(|| u64::from(width / 4).div_ceil(NATIVE_EXPAND_WORKGROUP_WIDTH));
    let row_groups_y = u64::from(height).div_ceil(NATIVE_EXPAND_WORKGROUP_HEIGHT);
    let row_tiled_workgroups = row_groups_x.and_then(|groups_x| groups_x.checked_mul(row_groups_y));
    let row_tiled = row_groups_x.is_some_and(|groups_x| {
        groups_x <= max_groups
            && row_groups_y <= max_groups
            && row_tiled_workgroups.is_some_and(|count| count <= required_workgroups)
    });
    let (groups_x, groups_y) = if row_tiled {
        (row_groups_x?, row_groups_y)
    } else {
        let groups_x = required_workgroups.min(max_groups);
        if groups_x == 0 {
            return None;
        }
        let groups_y = required_workgroups.div_ceil(groups_x);
        if groups_y > max_groups {
            return None;
        }
        (groups_x, groups_y)
    };
    let capacity_bytes = u64::from(capacity_words).checked_mul(4)?;
    (transfer_bytes <= capacity_bytes
        && transfer_bytes <= u64::from(max_storage_buffer_binding_size)
        && transfer_bytes <= max_buffer_size)
        .then_some(NativePackedChannelOutputDispatch {
            groups_x: u32::try_from(groups_x).ok()?,
            groups_y: u32::try_from(groups_y).ok()?,
            row_tiled,
            output_word_count,
            transfer_bytes,
        })
}

fn plan_native_la_transform_output(
    ops: &[PipelineOp],
    max_workgroups_per_dimension: u32,
    max_storage_buffer_binding_size: u32,
    max_buffer_size: u64,
    capacity_words: u32,
) -> Option<NativePackedChannelOutputDispatch> {
    plan_native_transform_output(
        ops,
        2,
        max_workgroups_per_dimension,
        max_storage_buffer_binding_size,
        max_buffer_size,
        capacity_words,
    )
}

/// Admit one native LA affine-nearest transform. The matching packed shader
/// reads and writes two-byte luma/alpha samples directly, without RGBA staging.
#[cfg(target_endian = "little")]
fn gpu_native_la_transform_input(
    ops: &[PipelineOp],
    image: &DynamicImage,
    logical_mode: Option<&str>,
) -> bool {
    if !matches!(
        ops,
        [PipelineOp::Transform {
            method: TransformMethod::Affine,
            filter: ResampleFilter::Nearest,
            ..
        }]
    ) || !matches!(logical_mode, None | Some("LA"))
    {
        return false;
    }
    let DynamicImage::ImageLumaA8(la) = image else {
        return false;
    };
    let Ok(layout) = CheckedDims::new(image.width(), image.height(), 2) else {
        return false;
    };
    let byte_count = layout.total_bytes();
    byte_count > 0 && byte_count <= u32::MAX as usize && la.as_raw().len() == byte_count
}

#[cfg(not(target_endian = "little"))]
fn gpu_native_la_transform_input(
    _ops: &[PipelineOp],
    _image: &DynamicImage,
    _logical_mode: Option<&str>,
) -> bool {
    false
}

#[cfg(target_endian = "little")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct NativeRgbPutAlphaDispatch {
    input_bytes: usize,
    input_transfer_bytes: u64,
    output_bytes: usize,
    output_transfer_bytes: u64,
    groups_x: u32,
    groups_y: u32,
}

/// Plan native three-byte RGB input and four-byte RGBA output for scalar
/// PutAlpha. Shader byte offsets and the linear pixel index are u32, while
/// both storage bindings and each workgroup axis must fit the active adapter.
#[cfg(target_endian = "little")]
fn plan_native_rgb_put_alpha_dispatch(
    width: u32,
    height: u32,
    max_workgroups_per_dimension: u32,
    max_storage_buffer_binding_size: u32,
    max_buffer_size: u64,
) -> Option<NativeRgbPutAlphaDispatch> {
    if width == 0 || height == 0 || max_workgroups_per_dimension == 0 {
        return None;
    }
    let pixels = u64::from(width).checked_mul(u64::from(height))?;
    let input_bytes = pixels.checked_mul(3)?;
    let output_bytes = pixels.checked_mul(4)?;
    if pixels > u64::from(u32::MAX) || input_bytes > u64::from(u32::MAX) {
        return None;
    }
    let input_transfer_bytes = input_bytes.checked_add(3)? & !3;
    let output_transfer_bytes = output_bytes.checked_add(3)? & !3;
    let storage_limit = u64::from(max_storage_buffer_binding_size);
    if input_transfer_bytes > storage_limit
        || output_transfer_bytes > storage_limit
        || input_transfer_bytes > max_buffer_size
        || output_transfer_bytes > max_buffer_size
    {
        return None;
    }
    let groups_x = width.div_ceil(16);
    let groups_y = height.div_ceil(16);
    if groups_x > max_workgroups_per_dimension || groups_y > max_workgroups_per_dimension {
        return None;
    }
    Some(NativeRgbPutAlphaDispatch {
        input_bytes: usize::try_from(input_bytes).ok()?,
        input_transfer_bytes,
        output_bytes: usize::try_from(output_bytes).ok()?,
        output_transfer_bytes,
        groups_x,
        groups_y,
    })
}

#[cfg(target_endian = "little")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct NativeRgbPutAlphaDataDispatch {
    rgb_bytes: usize,
    rgb_transfer_bytes: u64,
    mask_bytes: usize,
    mask_transfer_bytes: u64,
    output_bytes: usize,
    output_transfer_bytes: u64,
    groups_x: u32,
    groups_y: u32,
}

/// Plan native RGB and L-mask uploads for the required RGBA PutAlphaData
/// result. The shader gathers three packed source bytes per pixel and uses a
/// separately packed one-byte mask, so each binding has its own checked span.
#[cfg(target_endian = "little")]
fn plan_native_rgb_put_alpha_data_dispatch(
    width: u32,
    height: u32,
    max_workgroups_per_dimension: u32,
    max_storage_buffer_binding_size: u32,
    max_buffer_size: u64,
) -> Option<NativeRgbPutAlphaDataDispatch> {
    if width == 0 || height == 0 || max_workgroups_per_dimension == 0 {
        return None;
    }
    let pixels = u64::from(width).checked_mul(u64::from(height))?;
    let rgb_bytes = pixels.checked_mul(3)?;
    let output_bytes = pixels.checked_mul(4)?;
    if pixels > u64::from(u32::MAX) || rgb_bytes > u64::from(u32::MAX) {
        return None;
    }
    let padded = |bytes: u64| bytes.checked_add(3).map(|bytes| bytes & !3);
    let rgb_transfer_bytes = padded(rgb_bytes)?;
    let mask_transfer_bytes = padded(pixels)?;
    let output_transfer_bytes = padded(output_bytes)?;
    let storage_limit = u64::from(max_storage_buffer_binding_size);
    if [
        rgb_transfer_bytes,
        mask_transfer_bytes,
        output_transfer_bytes,
    ]
    .into_iter()
    .any(|bytes| bytes > storage_limit || bytes > max_buffer_size)
    {
        return None;
    }
    let groups_x = width.div_ceil(16);
    let groups_y = height.div_ceil(16);
    if groups_x > max_workgroups_per_dimension || groups_y > max_workgroups_per_dimension {
        return None;
    }
    Some(NativeRgbPutAlphaDataDispatch {
        rgb_bytes: usize::try_from(rgb_bytes).ok()?,
        rgb_transfer_bytes,
        mask_bytes: usize::try_from(pixels).ok()?,
        mask_transfer_bytes,
        output_bytes: usize::try_from(output_bytes).ok()?,
        output_transfer_bytes,
        groups_x,
        groups_y,
    })
}

#[cfg(target_endian = "little")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct NativeRgbaPutAlphaDataDispatch {
    rgba_bytes: usize,
    rgba_transfer_bytes: u64,
    mask_bytes: usize,
    mask_transfer_bytes: u64,
    output_transfer_bytes: u64,
    groups_x: u32,
    groups_y: u32,
}

/// Plan native four-byte RGBA and one-byte L-mask uploads for PutAlphaData.
/// Pixel and mask indexing use u32 in WGSL, and each binding must fit the
/// active adapter's storage-buffer and allocation limits.
#[cfg(target_endian = "little")]
fn plan_native_rgba_put_alpha_data_dispatch(
    width: u32,
    height: u32,
    max_workgroups_per_dimension: u32,
    max_storage_buffer_binding_size: u32,
    max_buffer_size: u64,
) -> Option<NativeRgbaPutAlphaDataDispatch> {
    if width == 0 || height == 0 || max_workgroups_per_dimension == 0 {
        return None;
    }
    let pixels = u64::from(width).checked_mul(u64::from(height))?;
    let rgba_bytes = pixels.checked_mul(4)?;
    if pixels > u64::from(u32::MAX) {
        return None;
    }
    let padded = |bytes: u64| bytes.checked_add(3).map(|bytes| bytes & !3);
    let rgba_transfer_bytes = padded(rgba_bytes)?;
    let mask_transfer_bytes = padded(pixels)?;
    let output_transfer_bytes = rgba_transfer_bytes;
    let storage_limit = u64::from(max_storage_buffer_binding_size);
    if [
        rgba_transfer_bytes,
        mask_transfer_bytes,
        output_transfer_bytes,
    ]
    .into_iter()
    .any(|bytes| bytes > storage_limit || bytes > max_buffer_size)
    {
        return None;
    }
    let groups_x = width.div_ceil(16);
    let groups_y = height.div_ceil(16);
    if groups_x > max_workgroups_per_dimension || groups_y > max_workgroups_per_dimension {
        return None;
    }
    Some(NativeRgbaPutAlphaDataDispatch {
        rgba_bytes: usize::try_from(rgba_bytes).ok()?,
        rgba_transfer_bytes,
        mask_bytes: usize::try_from(pixels).ok()?,
        mask_transfer_bytes,
        output_transfer_bytes,
        groups_x,
        groups_y,
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

/// Return whether a nearest resize uses the packed byte convolution plan.
///
/// The convolution plan receives host-generated one-tap tables so its source
/// index follows Pillow's cumulative f64 stepping exactly. I- and F-mode need
/// that plan too: their four-byte samples are copied opaquely by the typed
/// branch in the resize shaders, rather than being interpreted as four byte
/// channels. I;16 retains its established relocation shader until its own
/// nearest-coordinate receipts are expanded with the same proof.
fn gpu_resize_nearest_uses_coefficients(logical_mode: Option<&str>) -> bool {
    !matches!(
        logical_mode,
        Some("1" | "I;16" | "I;16L" | "I;16B" | "I;16N")
    )
}

/// Build a resize coefficient table for the exact GPU byte path.
///
/// Non-nearest filters reuse the shared Pillow fixed-point table. Nearest is
/// different: Pillow's affine implementation advances one f64 accumulator
/// per output sample, so computing `(x + 0.5) * scale` independently in WGSL
/// can select a neighboring source pixel at a representable boundary. A
/// one-tap table preserves that scalar control-plane decision while leaving
/// every pixel read/write on the device.
fn gpu_resize_coefficients(
    out_size: u32,
    in_size: u32,
    filter: ResampleFilter,
) -> Arc<FilterCoeffs> {
    if !matches!(filter, ResampleFilter::Nearest) {
        return precompute_coeffs(out_size, in_size, filter);
    }
    if out_size == 0 || in_size == 0 {
        return Arc::new(FilterCoeffs {
            xmin: Vec::new(),
            count: Vec::new(),
            offsets: Vec::new(),
            weights: Vec::new(),
        });
    }

    let scale = f64::from(in_size) / f64::from(out_size);
    let mut source_position = scale * 0.5;
    let mut xmin = Vec::with_capacity(out_size as usize);
    let mut count = Vec::with_capacity(out_size as usize);
    let mut offsets = Vec::with_capacity(out_size as usize);
    let mut weights = Vec::with_capacity(out_size as usize);
    for _ in 0..out_size {
        let source = if source_position >= f64::from(in_size) {
            in_size - 1
        } else {
            source_position as u32
        };
        xmin.push(i64::from(source));
        count.push(1);
        offsets.push(weights.len());
        weights.push(1i64 << 22);
        source_position += scale;
    }
    Arc::new(FilterCoeffs {
        xmin,
        count,
        offsets,
        weights,
    })
}

/// Validate and size the fixed-point coefficient table consumed by the
/// separable resize shaders.
///
/// `FilterCoeffs` is deliberately shared with the CPU/SIMD implementations so
/// the GPU receives the same Pillow-rounded coefficients.  The shader uses an
/// i32 accumulator; prove the complete byte-domain accumulation before a
/// resize is admitted rather than allowing device arithmetic to wrap.
fn resize_coeff_word_count(coeffs: &FilterCoeffs) -> Result<usize, PilError> {
    if coeffs.xmin.len() != coeffs.count.len()
        || coeffs.xmin.len() != coeffs.offsets.len()
        || coeffs
            .offsets
            .iter()
            .zip(&coeffs.count)
            .any(|(&offset, &count)| {
                offset
                    .checked_add(count)
                    .is_none_or(|end| end > coeffs.weights.len())
            })
    {
        return Err(PilError::InternalError(
            "GPU resize coefficient table shape is invalid".into(),
        ));
    }

    for ((&xmin, &count), &offset) in coeffs.xmin.iter().zip(&coeffs.count).zip(&coeffs.offsets) {
        if xmin < 0
            || u32::try_from(xmin).is_err()
            || u32::try_from(count).is_err()
            || u32::try_from(offset).is_err()
        {
            return Err(PilError::ValueError(
                "GPU resize coefficient metadata exceeds shader limits".into(),
            ));
        }
    }

    for &weight in &coeffs.weights {
        let weight = i32::try_from(weight).map_err(|_| {
            PilError::ValueError("GPU resize coefficient exceeds signed shader limits".into())
        })?;
        let abs_weight = i64::from(weight).checked_abs().ok_or_else(|| {
            PilError::ValueError("GPU resize coefficient magnitude overflow".into())
        })?;
        // Every intermediate partial sum is bounded by the absolute sum of
        // all taps.  The largest byte sample is 255 and the shader adds the
        // fixed-point half-unit before shifting.
        let _ = abs_weight;
    }
    for (&offset, &count) in coeffs.offsets.iter().zip(&coeffs.count) {
        let mut abs_sum = 0i64;
        for &weight in coeffs.weights.get(offset..offset + count).ok_or_else(|| {
            PilError::InternalError("GPU resize coefficient slice is invalid".into())
        })? {
            let abs_weight = weight.checked_abs().ok_or_else(|| {
                PilError::ValueError("GPU resize coefficient magnitude overflow".into())
            })?;
            abs_sum = abs_sum.checked_add(abs_weight).ok_or_else(|| {
                PilError::ValueError("GPU resize coefficient sum overflow".into())
            })?;
        }
        let max_accumulator = abs_sum
            .checked_mul(255)
            .and_then(|value| value.checked_add(1i64 << 21))
            .ok_or_else(|| PilError::ValueError("GPU resize accumulator overflow".into()))?;
        if max_accumulator > i64::from(i32::MAX) {
            return Err(PilError::ValueError(
                "GPU resize accumulator exceeds signed shader limits".into(),
            ));
        }
    }

    coeffs
        .xmin
        .len()
        .checked_mul(3)
        .and_then(|metadata| metadata.checked_add(coeffs.weights.len()))
        .ok_or_else(|| PilError::ValueError("GPU resize coefficient arena size overflow".into()))
}

/// Encode one Pillow-compatible coefficient table as `[xmin,count,offset]`
/// metadata followed by signed fixed-point weights.  The returned words are
/// uploaded as an auxiliary storage range; no image pixels are materialized on
/// the host for this control table.
fn encode_resize_coeffs(coeffs: &FilterCoeffs) -> Result<Vec<u32>, PilError> {
    let word_count = resize_coeff_word_count(coeffs)?;
    let mut words = Vec::with_capacity(word_count);
    for ((&xmin, &count), &offset) in coeffs.xmin.iter().zip(&coeffs.count).zip(&coeffs.offsets) {
        words.push(u32::try_from(xmin).map_err(|_| {
            PilError::ValueError("GPU resize coefficient xmin exceeds shader limits".into())
        })?);
        words.push(u32::try_from(count).map_err(|_| {
            PilError::ValueError("GPU resize coefficient count exceeds shader limits".into())
        })?);
        words.push(u32::try_from(offset).map_err(|_| {
            PilError::ValueError("GPU resize coefficient offset exceeds shader limits".into())
        })?);
    }
    words.extend(coeffs.weights.iter().map(|&weight| {
        // resize_coeff_word_count already checked this conversion.
        i32::try_from(weight).expect("validated GPU resize coefficient") as u32
    }));
    debug_assert_eq!(words.len(), word_count);
    Ok(words)
}

/// Validate and size an f64 coefficient table transported to marker-9's
/// integer reducer.  Each dyadic coefficient occupies four words: a 53-bit
/// mantissa, its binary exponent, and its sign.  Metadata offsets therefore
/// count groups of four rather than individual weights.
fn resize_coeff_word_count_f64(coeffs: &FilterCoeffsF64) -> Result<usize, PilError> {
    if coeffs.xmin.len() != coeffs.count.len()
        || coeffs.xmin.len() != coeffs.weights.len()
        || coeffs
            .count
            .iter()
            .zip(&coeffs.weights)
            .any(|(&count, weights)| count != weights.len())
    {
        return Err(PilError::InternalError(
            "GPU f64 resize coefficient table shape is invalid".into(),
        ));
    }
    for &xmin in &coeffs.xmin {
        if xmin < 0 || u32::try_from(xmin).is_err() {
            return Err(PilError::ValueError(
                "GPU resize coefficient xmin exceeds shader limits".into(),
            ));
        }
    }
    let mut weight_words = 0usize;
    for row in &coeffs.weights {
        for &weight in row {
            if gpu_f64_integer_parts(weight).is_none() {
                return Err(PilError::ValueError(
                    "GPU resize f64 coefficient is not a finite normal value".into(),
                ));
            }
        }
        weight_words = weight_words
            .checked_add(row.len().checked_mul(4).ok_or_else(|| {
                PilError::ValueError("GPU resize coefficient arena size overflow".into())
            })?)
            .ok_or_else(|| {
                PilError::ValueError("GPU resize coefficient arena size overflow".into())
            })?;
    }
    coeffs
        .xmin
        .len()
        .checked_mul(3)
        .and_then(|metadata| metadata.checked_add(weight_words))
        .ok_or_else(|| PilError::ValueError("GPU resize coefficient arena size overflow".into()))
}

/// Return whether one f64 coefficient table can be bound as one storage range.
///
/// The host proof may certify every row's arithmetic while the encoded table
/// still exceeds the device binding limit when multiple output rows are
/// present.  Keep this check next to the encoder's word-count contract so
/// marker 9's special-value prepass and marker 12's ordered reducer make the
/// same adapter-safe admission decision before any bind group is created.
fn gpu_f_resize_f64_coefficients_fit_binding(coeffs: &FilterCoeffsF64) -> bool {
    let Ok(word_count) = resize_coeff_word_count_f64(coeffs) else {
        return false;
    };
    let Some(bytes) = word_count.checked_mul(std::mem::size_of::<u32>()) else {
        return false;
    };
    aligned_bytes(bytes, GPU_F_RESIZE_COEFFICIENT_ALIGNMENT_BYTES)
        <= GPU_F_RESIZE_MAX_COEFFICIENT_BINDING_BYTES
}

fn encode_resize_coeffs_f64(coeffs: &FilterCoeffsF64) -> Result<Vec<u32>, PilError> {
    let word_count = resize_coeff_word_count_f64(coeffs)?;
    let mut words = Vec::with_capacity(word_count);
    let mut weight_offset = 0usize;
    for ((&xmin, &count), row) in coeffs.xmin.iter().zip(&coeffs.count).zip(&coeffs.weights) {
        words.push(u32::try_from(xmin).map_err(|_| {
            PilError::ValueError("GPU resize coefficient xmin exceeds shader limits".into())
        })?);
        words.push(u32::try_from(count).map_err(|_| {
            PilError::ValueError("GPU resize coefficient count exceeds shader limits".into())
        })?);
        words.push(u32::try_from(weight_offset).map_err(|_| {
            PilError::ValueError("GPU resize coefficient offset exceeds shader limits".into())
        })?);
        weight_offset = weight_offset
            .checked_add(row.len().checked_mul(4).ok_or_else(|| {
                PilError::ValueError("GPU resize coefficient arena size overflow".into())
            })?)
            .ok_or_else(|| {
                PilError::ValueError("GPU resize coefficient arena size overflow".into())
            })?;
    }
    for row in &coeffs.weights {
        for &weight in row {
            let parts = gpu_f64_integer_parts(weight).ok_or_else(|| {
                PilError::ValueError(
                    "GPU resize f64 coefficient is not a finite normal value".into(),
                )
            })?;
            words.push(parts.mantissa as u32);
            words.push((parts.mantissa >> 32) as u32);
            words.push(parts.exponent as u32);
            words.push(u32::from(parts.negative && parts.mantissa != 0));
        }
    }
    debug_assert_eq!(words.len(), word_count);
    Ok(words)
}

/// Encode compact Box rows whose normalized coefficient is repeated for every
/// tap.  This is used only for integer-ratio downscales beyond the full f64
/// coefficient binding envelope; ordinary rows continue to use the complete
/// per-tap table above.
fn encode_resize_compact_box_axis(
    source_size: u32,
    output_size: u32,
) -> Result<Vec<u32>, PilError> {
    if output_size == 0
        || source_size % output_size != 0
        || source_size / output_size <= GPU_F_RESIZE_ORDERED_MAX_TAPS as u32
    {
        return Err(PilError::InternalError(
            "GPU compact Box coefficient geometry is invalid".into(),
        ));
    }
    let tap_count = source_size / output_size;
    let coefficient = 1.0 / f64::from(tap_count);
    let parts = gpu_f64_integer_parts(coefficient).ok_or_else(|| {
        PilError::ValueError("GPU compact Box coefficient is not a finite normal value".into())
    })?;
    let output_count = usize::try_from(output_size)
        .map_err(|_| PilError::ValueError("GPU compact Box output size is too large".into()))?;
    let mut words = Vec::with_capacity(
        output_count
            .checked_mul(3)
            .and_then(|count| count.checked_add(4))
            .ok_or_else(|| PilError::ValueError("GPU compact Box arena size overflow".into()))?,
    );
    for output_index in 0..output_count {
        let xmin = u32::try_from(
            output_index
                .checked_mul(usize::try_from(tap_count).map_err(|_| {
                    PilError::ValueError("GPU compact Box tap count is too large".into())
                })?)
                .ok_or_else(|| PilError::ValueError("GPU compact Box offset overflow".into()))?,
        )
        .map_err(|_| PilError::ValueError("GPU compact Box offset exceeds shader limits".into()))?;
        words.extend([xmin, tap_count, 0]);
    }
    words.extend([
        parts.mantissa as u32,
        (parts.mantissa >> 32) as u32,
        parts.exponent as u32,
        u32::from(parts.negative && parts.mantissa != 0),
    ]);
    Ok(words)
}

fn gpu_resize_coefficients_are_safe(
    filter: ResampleFilter,
    source_dimensions: (u32, u32),
    output_dimensions: (u32, u32),
) -> bool {
    let (source_w, source_h) = source_dimensions;
    let (output_w, output_h) = output_dimensions;
    if source_w == 0 || source_h == 0 || output_w == 0 || output_h == 0 {
        return false;
    }
    let horizontal = gpu_resize_coefficients(output_w, source_w, filter);
    let vertical = gpu_resize_coefficients(output_h, source_h, filter);
    resize_coeff_word_count(&horizontal).is_ok() && resize_coeff_word_count(&vertical).is_ok()
}

/// Build the exact two-pass coefficient tables for ImageOps.fit's fractional
/// source box.  The boxed builder performs Pillow's required f32 boundary
/// conversion before calculating f64 centers and 22-bit weights; reusing it
/// here keeps the GPU's control table identical to `pil_resize_boxed` while
/// leaving all sample reads, accumulation, and writes on the device.
/// This follows Pillow's `libImaging/Resample.c::precompute_coeffs` contract:
/// `ImageOps.fit` passes its crop box through the float ABI before those
/// coefficients are calculated, so retaining the fractional box is essential
/// at pixel boundaries.
fn gpu_fit_coefficients(
    source_dimensions: (u32, u32),
    output_dimensions: (u32, u32),
    bleed: f64,
    centering: (f64, f64),
    filter: ResampleFilter,
) -> Option<(FilterCoeffs, FilterCoeffs)> {
    let (crop_left, crop_top, crop_w, crop_h) = gpu_fit_box(
        source_dimensions.0,
        source_dimensions.1,
        output_dimensions.0,
        output_dimensions.1,
        bleed,
        centering,
    )?;
    if matches!(filter, ResampleFilter::Nearest) {
        return Some((
            gpu_fit_nearest_coefficients(
                output_dimensions.0,
                source_dimensions.0,
                crop_left,
                crop_left + crop_w,
            ),
            gpu_fit_nearest_coefficients(
                output_dimensions.1,
                source_dimensions.1,
                crop_top,
                crop_top + crop_h,
            ),
        ));
    }
    Some((
        precompute_coeffs_boxed_for_filter(
            output_dimensions.0,
            source_dimensions.0,
            crop_left,
            crop_left + crop_w,
            filter,
        ),
        precompute_coeffs_boxed_for_filter(
            output_dimensions.1,
            source_dimensions.1,
            crop_top,
            crop_top + crop_h,
            filter,
        ),
    ))
}

/// Build one-tap tables for Pillow's boxed nearest affine mapping.
///
/// `Image.resize(box=...)` does not use the convolution coefficients for
/// `NEAREST`: `ImagingScaleAffine` evaluates the source coordinate from the
/// float box bounds for each output pixel.  Keep that mapping in the host
/// control table (after the same f32 boundary conversion) so the exact
/// resize kernels can still own every byte read and write.
fn gpu_fit_nearest_coefficients(
    output_size: u32,
    source_size: u32,
    box_start: f64,
    box_end: f64,
) -> FilterCoeffs {
    if output_size == 0 || source_size == 0 {
        return FilterCoeffs {
            xmin: Vec::new(),
            count: Vec::new(),
            offsets: Vec::new(),
            weights: Vec::new(),
        };
    }
    let box_start_f32 = box_start as f32;
    let box_end_f32 = box_end as f32;
    let box_start = box_start_f32 as f64;
    let scale = f64::from(box_end_f32 - box_start_f32) / f64::from(output_size);
    let last_source = i64::from(source_size - 1);
    let mut xmin = Vec::with_capacity(output_size as usize);
    let mut count = Vec::with_capacity(output_size as usize);
    let mut offsets = Vec::with_capacity(output_size as usize);
    let mut weights = Vec::with_capacity(output_size as usize);
    // ImagingScaleAffine initializes one center coordinate and advances it
    // cumulatively. Recomputing each `(index + 0.5) * scale` can cross an
    // upsample boundary differently after f64 rounding.
    let mut coordinate = box_start + scale * 0.5;
    for _ in 0..output_size {
        let source = (coordinate.floor() as i64).clamp(0, last_source);
        xmin.push(source);
        count.push(1);
        offsets.push(weights.len());
        weights.push(1i64 << 22);
        coordinate += scale;
    }
    FilterCoeffs {
        xmin,
        count,
        offsets,
        weights,
    }
}

/// Build the exact boxed coefficient pair consumed by the device resize
/// passes. Boxed nearest uses Pillow's cumulative affine coordinate walk;
/// filtered modes use the shared float32-boundary fixed-point table builder.
fn gpu_resize_boxed_coefficients(
    source_dimensions: (u32, u32),
    output_dimensions: (u32, u32),
    bounds: (f64, f64, f64, f64),
    filter: ResampleFilter,
) -> (FilterCoeffs, FilterCoeffs) {
    let (left, top, right, bottom) = (
        f64::from(bounds.0 as f32),
        f64::from(bounds.1 as f32),
        f64::from(bounds.2 as f32),
        f64::from(bounds.3 as f32),
    );
    if matches!(filter, ResampleFilter::Nearest) {
        return (
            gpu_fit_nearest_coefficients(output_dimensions.0, source_dimensions.0, left, right),
            gpu_fit_nearest_coefficients(output_dimensions.1, source_dimensions.1, top, bottom),
        );
    }
    (
        precompute_coeffs_boxed_for_filter(
            output_dimensions.0,
            source_dimensions.0,
            left,
            right,
            filter,
        ),
        precompute_coeffs_boxed_for_filter(
            output_dimensions.1,
            source_dimensions.1,
            top,
            bottom,
            filter,
        ),
    )
}

fn gpu_resize_boxed_coefficients_are_safe(
    source_dimensions: (u32, u32),
    output_dimensions: (u32, u32),
    bounds: (f64, f64, f64, f64),
    filter: ResampleFilter,
) -> bool {
    if source_dimensions.0 == 0
        || source_dimensions.1 == 0
        || output_dimensions.0 == 0
        || output_dimensions.1 == 0
    {
        return false;
    }
    let Some(source_pixels) = CheckedDims::new(source_dimensions.0, source_dimensions.1, 4)
        .ok()
        .map(|dims| dims.total_pixels())
    else {
        return false;
    };
    let Some(output_pixels) = CheckedDims::new(output_dimensions.0, output_dimensions.1, 4)
        .ok()
        .map(|dims| dims.total_pixels())
    else {
        return false;
    };
    if source_pixels > GPU_BUFFER_CAPACITY as usize || output_pixels > GPU_BUFFER_CAPACITY as usize
    {
        return false;
    }
    let (horizontal, vertical) =
        gpu_resize_boxed_coefficients(source_dimensions, output_dimensions, bounds, filter);
    resize_coeff_word_count(&horizontal).is_ok() && resize_coeff_word_count(&vertical).is_ok()
}

fn gpu_fit_coefficients_are_safe(
    source_dimensions: (u32, u32),
    output_dimensions: (u32, u32),
    bleed: f64,
    centering: (f64, f64),
    filter: ResampleFilter,
) -> bool {
    let Some((horizontal, vertical)) = gpu_fit_coefficients(
        source_dimensions,
        output_dimensions,
        bleed,
        centering,
        filter,
    ) else {
        return false;
    };
    resize_coeff_word_count(&horizontal).is_ok() && resize_coeff_word_count(&vertical).is_ok()
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

// ─── Count shader bindings ─────────────────────────────────────────────────

/// Count the number of `@binding(N)` entries in WGSL source.
/// Returns max binding index + 1, or 0 if no bindings found.
fn count_shader_bindings(source: &str) -> u32 {
    let mut max_binding: i32 = -1;
    for line in source.lines() {
        if let Some(pos) = line.find("@binding(") {
            let rest = &line[pos + 9..];
            if let Some(end) = rest.find(')') {
                if let Ok(n) = rest[..end].trim().parse::<u32>() {
                    max_binding = max_binding.max(n as i32);
                }
            }
        }
    }
    if max_binding >= 0 {
        max_binding as u32 + 1
    } else {
        0
    }
}

/// Detect if a 4-binding shader uses the LUT layout (Eval/PointOp).
/// In LUT layout, `@binding(1)` is `storage read_write` (output).
/// In dual-input layout, `@binding(1)` is `storage read` (input_b).
fn is_lut_shader(source: &str) -> bool {
    for line in source.lines() {
        if line.contains("@binding(1)") {
            return line.contains("read_write");
        }
    }
    false
}

/// Detect the two-binding generator layout used by kernels that do not read
/// the current pipeline image. Their binding 0 is the writable output and
/// binding 1 is the uniform parameter block, unlike ordinary two-binding
/// kernels whose binding 0 is a read-only input.
fn is_output_only_shader(source: &str) -> bool {
    source
        .lines()
        .any(|line| line.contains("@binding(0)") && line.contains("var<storage, read_write>"))
}

// ─── CachedPipeline ────────────────────────────────────────────────────────

struct CachedPipeline {
    pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    variant_name: &'static str,
    shader_file: &'static str,
    /// Number of bindings in this shader (2-5).
    num_bindings: u32,
    /// True if this is a 4-binding LUT shader (Eval/PointOp).
    is_lut: bool,
    /// True if this is a two-binding generator shader with no input image.
    is_output_only: bool,
    /// True if the shader reads an auxiliary source and updates the current
    /// destination buffer in place (AlphaComposite).
    is_in_place: bool,
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

    fn resolve_pipeline(
        &self,
        key: &'static str,
        shader_file: &'static str,
        source: &'static str,
    ) -> Result<Arc<CachedPipeline>, PilError> {
        let mut pipelines = self
            .pipelines
            .lock()
            .map_err(|_| PilError::InternalError("GPU pipeline-cache lock is poisoned".into()))?;
        if let Some(pipeline) = pipelines.get(key) {
            return Ok(Arc::clone(pipeline));
        }

        gpu_log!("[GPU] compiling shader on first use: {key}");
        let pipeline =
            Self::build_pipeline(&self.device, key, shader_file, source).ok_or_else(|| {
                PilError::ValueError(format!(
                    "GPU operation '{key}' has no validated executable pipeline"
                ))
            })?;
        gpu_log!(
            "[GPU] compiled shader on first use: {key} ({} bindings)",
            pipeline.num_bindings
        );
        let pipeline = Arc::new(pipeline);
        pipelines.insert(key, Arc::clone(&pipeline));
        Ok(pipeline)
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

    fn build_pipeline(
        device: &wgpu::Device,
        variant_name: &'static str,
        shader_file: &'static str,
        shader_source: &str,
    ) -> Option<CachedPipeline> {
        device.push_error_scope(wgpu::ErrorFilter::Validation);
        let cs_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(variant_name),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });
        if let Some(error) = pollster::block_on(device.pop_error_scope()) {
            gpu_log!(
                "[GPU] shader module validation failed: variant={variant_name} file={shader_file}: {error}"
            );
            return None;
        }

        let num_bindings = count_shader_bindings(shader_source);

        // Supported: 2-5 binding shaders. 0/1/>5 are invalid.
        if !(2..=5).contains(&num_bindings) {
            gpu_log!(
                "[GPU] shader rejected: variant={variant_name} file={shader_file}: bindings={num_bindings}"
            );
            return None;
        }

        // Detect if this is a LUT shader (Eval/PointOp) with 4 bindings.
        let is_lut = num_bindings == 4 && is_lut_shader(shader_source);
        let is_output_only = num_bindings == 2 && is_output_only_shader(shader_source);
        let is_in_place = variant_name == "AlphaComposite";

        // Build bind group layout matching shader declarations.
        // Layout depends on binding count and LUT variant:
        //   2: [input(read), output(read_write)]
        //   2 (generator): [output(read_write), params(uniform)]
        //   3: [input(read), output(read_write), params(uniform)]
        //   4 (dual-input): [input_a(read), input_b(read), output(read_write), params(uniform)]
        //   4 (LUT):        [input(read), output(read_write), params(uniform), lut(read)]
        //   5: [input_a(read), input_b(read), input_c(read), output(read_write), params(uniform)]
        let mut bindings = Vec::with_capacity(num_bindings as usize);

        if num_bindings == 5 {
            // 5-binding: 3 inputs + output + params (Composite/CompositeModule/Paste)
            for i in 0..3 {
                bindings.push(wgpu::BindGroupLayoutEntry {
                    binding: i,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                });
            }
            bindings.push(wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            });
            bindings.push(wgpu::BindGroupLayoutEntry {
                binding: 4,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            });
        } else if num_bindings == 4 {
            if is_lut {
                // LUT layout: [input(read), output(rw), params(uniform), lut(read)]
                bindings.push(wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                });
                bindings.push(wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                });
                bindings.push(wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                });
                bindings.push(wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                });
            } else {
                // Dual-input layout
                bindings.push(wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                });
                bindings.push(wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                });
                bindings.push(wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                });
                bindings.push(wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                });
            }
        } else if is_output_only {
            // Generator layout: [output(rw), params(uniform)].
            bindings.push(wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            });
            bindings.push(wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            });
        } else {
            // 2 or 3 binding (single-input) layout
            bindings.push(wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            });
            bindings.push(wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            });

            if num_bindings > 2 {
                bindings.push(wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                });
            }
        }

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some(variant_name),
            entries: &bindings,
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some(variant_name),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Use error scope to catch shader validation errors without panicking.
        device.push_error_scope(wgpu::ErrorFilter::Validation);
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some(variant_name),
            layout: Some(&pipeline_layout),
            module: &cs_module,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        // If validation failed, skip this shader — it won't be available on GPU.
        if let Some(error) = pollster::block_on(device.pop_error_scope()) {
            gpu_log!(
                "[GPU] shader validation failed: variant={variant_name} file={shader_file}: {error}"
            );
            return None;
        }

        Some(CachedPipeline {
            pipeline,
            bind_group_layout,
            variant_name,
            shader_file,
            num_bindings,
            is_lut,
            is_output_only,
            is_in_place,
        })
    }

    fn estimate_resource_bytes(
        &self,
        op: &PipelineOp,
        auxiliary: &AuxiliaryImages,
        buffers: &BufferPool,
        uniform_alignment: usize,
        storage_alignment: usize,
        source_dimensions: (u32, u32),
        mode: u32,
        packed_luma_putdata: bool,
        logical_mode: Option<&str>,
        f_resize_f64_is_exact: bool,
        f_resize_f64_ordered_is_exact: bool,
    ) -> Result<usize, PilError> {
        let op_param_words = if matches!(op, PipelineOp::Pad { .. }) {
            // Header + resize controls + placement controls. The final
            // output dimensions appended below are included separately.
            9
        } else if matches!(op, PipelineOp::Fit { .. } | PipelineOp::ResizeBoxed { .. }) {
            // Fit and ResizeBoxed are lowered to the exact separable resize
            // kernels. Their fractional bounds live in coefficient tables,
            // leaving the same four resize control words as public Resize.
            4
        } else if let PipelineOp::Transform { method, .. } = op {
            // extract_params contributes ten public words; the executor then
            // adds premultiply, method, two projective values, and (for one
            // mesh record) four remaining source-corner values.
            if matches!(method, TransformMethod::Mesh) {
                18
            } else {
                14
            }
        } else if matches!(op, PipelineOp::Color3DLut { .. }) {
            7
        } else {
            registry::extract_params(op).len()
        };
        let param_words = 4usize
            .checked_add(op_param_words)
            .and_then(|words| {
                words.checked_add(usize::from(matches!(op, PipelineOp::Contrast { .. })))
            })
            .and_then(|words| words.checked_add(2))
            .ok_or_else(|| PilError::ValueError("GPU parameter arena size overflow".into()))?;
        let mut total = aligned_bytes(
            param_words
                .checked_mul(std::mem::size_of::<u32>())
                .ok_or_else(|| PilError::ValueError("GPU parameter arena size overflow".into()))?,
            uniform_alignment,
        );

        let image_bytes = |image: &DynamicImage| -> Result<usize, PilError> {
            let (w, h) = image.dimensions();
            Ok(aligned_bytes(
                CheckedDims::new(w, h, 4)?.total_bytes(),
                storage_alignment,
            ))
        };

        if let PipelineOp::PutData { data, mode } = op {
            let words = putdata_auxiliary_words(
                data.len(),
                mode.channels(),
                packed_luma_putdata && *mode == PixelMode::L,
            );
            if words > buffers.capacity as usize {
                return Err(PilError::ValueError(format!(
                    "GPU buffer capacity {} < putdata auxiliary size {}",
                    buffers.capacity, words
                )));
            }
            if words > 0 {
                total = total
                    .checked_add(aligned_bytes(
                        words
                            .checked_mul(std::mem::size_of::<u32>())
                            .ok_or_else(|| {
                                PilError::ValueError("GPU putdata arena size overflow".into())
                            })?,
                        storage_alignment,
                    ))
                    .ok_or_else(|| {
                        PilError::ValueError("GPU auxiliary arena size overflow".into())
                    })?;
            }
        } else {
            if let Some(second) = auxiliary.second.as_ref() {
                total = total.checked_add(image_bytes(second)?).ok_or_else(|| {
                    PilError::ValueError("GPU auxiliary arena size overflow".into())
                })?;
            }
        }
        if let Some(third) = auxiliary.third.as_ref() {
            total = total
                .checked_add(image_bytes(third)?)
                .ok_or_else(|| PilError::ValueError("GPU auxiliary arena size overflow".into()))?;
        }

        if matches!(
            op,
            PipelineOp::Autocontrast { .. }
                | PipelineOp::Equalize
                | PipelineOp::EqualizeMasked { .. }
        ) {
            total = total
                .checked_add(GPU_HISTOGRAM_BYTES)
                .ok_or_else(|| PilError::ValueError("GPU histogram arena size overflow".into()))?;
        }

        let resize_coefficients = match op {
            PipelineOp::Resize { w, h, filter } => Some((*w, *h, *filter, None)),
            PipelineOp::ResizeBoxed {
                w,
                h,
                filter,
                box_coords,
            } => Some((*w, *h, *filter, Some(*box_coords))),
            PipelineOp::Pad { filter, .. } => {
                gpu_pad_geometry(op, source_dimensions.0, source_dimensions.1)
                    .map(|((resize_w, resize_h), _)| (resize_w, resize_h, *filter, None))
            }
            _ => None,
        };
        if let Some((resize_w, resize_h, filter, box_coords)) = resize_coefficients {
            let (source_w, source_h) = source_dimensions;
            let (horizontal_bytes, vertical_bytes) = if let Some(box_coords) = box_coords {
                let (horizontal, vertical) = gpu_resize_boxed_coefficients(
                    source_dimensions,
                    (resize_w, resize_h),
                    box_coords,
                    filter,
                );
                (
                    resize_coeff_word_count(&horizontal)?,
                    resize_coeff_word_count(&vertical)?,
                )
            } else if (f_resize_f64_is_exact || f_resize_f64_ordered_is_exact)
                && matches!(mode, 5 | 7 | 8)
                && !matches!(filter, ResampleFilter::Nearest)
                && matches!(op, PipelineOp::Resize { .. } | PipelineOp::Pad { .. })
            {
                let compact_box = f_resize_f64_ordered_is_exact
                    && mode == 8
                    && matches!(op, PipelineOp::Resize { .. })
                    && matches!(filter, ResampleFilter::Box)
                    && gpu_f_resize_compact_box_any_axis(
                        (source_w, source_h),
                        (resize_w, resize_h),
                    );
                let (kernel, support) = filter_from_resample(filter);
                let axis_words = |source_size: u32, output_size: u32| {
                    if gpu_f_resize_compact_box_axis(source_size, output_size) {
                        usize::try_from(output_size)
                            .ok()
                            .and_then(|count| count.checked_mul(3))
                            .and_then(|metadata| metadata.checked_add(4))
                            .ok_or_else(|| {
                                PilError::ValueError(
                                    "GPU compact Box coefficient arena size overflow".into(),
                                )
                            })
                    } else {
                        let coeffs =
                            precompute_coeffs_f64(output_size, source_size, kernel, support);
                        resize_coeff_word_count_f64(&coeffs)
                    }
                };
                let (horizontal, vertical) = if compact_box {
                    (
                        axis_words(source_w, resize_w)?,
                        axis_words(source_h, resize_h)?,
                    )
                } else {
                    let horizontal = precompute_coeffs_f64(resize_w, source_w, kernel, support);
                    let vertical = precompute_coeffs_f64(resize_h, source_h, kernel, support);
                    (
                        resize_coeff_word_count_f64(&horizontal)?,
                        resize_coeff_word_count_f64(&vertical)?,
                    )
                };
                (horizontal, vertical)
            } else {
                let horizontal = gpu_resize_coefficients(resize_w, source_w, filter);
                let vertical = gpu_resize_coefficients(resize_h, source_h, filter);
                (
                    resize_coeff_word_count(&horizontal)?,
                    resize_coeff_word_count(&vertical)?,
                )
            };
            let horizontal_bytes = horizontal_bytes
                .checked_mul(std::mem::size_of::<u32>())
                .ok_or_else(|| {
                    PilError::ValueError("GPU resize coefficient arena size overflow".into())
                })?;
            let vertical_bytes = vertical_bytes
                .checked_mul(std::mem::size_of::<u32>())
                .ok_or_else(|| {
                    PilError::ValueError("GPU resize coefficient arena size overflow".into())
                })?;
            total = total
                .checked_add(aligned_bytes(horizontal_bytes, storage_alignment))
                .and_then(|value| {
                    value.checked_add(aligned_bytes(vertical_bytes, storage_alignment))
                })
                .ok_or_else(|| {
                    PilError::ValueError("GPU resize coefficient arena size overflow".into())
                })?;
        }

        if let PipelineOp::Fit {
            w,
            h,
            bleed,
            centering,
            filter,
            ..
        } = op
        {
            let (horizontal, vertical) =
                gpu_fit_coefficients(source_dimensions, (*w, *h), *bleed, *centering, *filter)
                    .ok_or_else(|| {
                        PilError::ValueError("GPU Fit has no safe crop geometry".into())
                    })?;
            let horizontal_bytes = resize_coeff_word_count(&horizontal)?
                .checked_mul(std::mem::size_of::<u32>())
                .ok_or_else(|| {
                    PilError::ValueError("GPU Fit coefficient arena size overflow".into())
                })?;
            let vertical_bytes = resize_coeff_word_count(&vertical)?
                .checked_mul(std::mem::size_of::<u32>())
                .ok_or_else(|| {
                    PilError::ValueError("GPU Fit coefficient arena size overflow".into())
                })?;
            total = total
                .checked_add(aligned_bytes(horizontal_bytes, storage_alignment))
                .and_then(|value| {
                    value.checked_add(aligned_bytes(vertical_bytes, storage_alignment))
                })
                .ok_or_else(|| {
                    PilError::ValueError("GPU Fit coefficient arena size overflow".into())
                })?;
        }

        if gpu_transform_uses_geometry_table(op, logical_mode, source_dimensions) {
            let (w, h) =
                op_output_dims(op, source_dimensions.0, source_dimensions.1).ok_or_else(|| {
                    PilError::InternalError("GPU transform dimensions missing".into())
                })?;
            total = total
                .checked_add(aligned_bytes(
                    CheckedDims::new(w, h, (GPU_TRANSFORM_GEOMETRY_WORDS * 4) as u8)?.total_bytes(),
                    storage_alignment,
                ))
                .ok_or_else(|| {
                    PilError::ValueError("GPU transform geometry arena size overflow".into())
                })?;
        }

        if extract_lut(op, mode).is_some() {
            total = total
                .checked_add(aligned_bytes(
                    256 * std::mem::size_of::<u32>(),
                    storage_alignment,
                ))
                .ok_or_else(|| PilError::ValueError("GPU LUT arena size overflow".into()))?;
        }
        Ok(total)
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

    fn readback_bytes(&self, size: u64, staging: &wgpu::Buffer) -> Result<Vec<u8>, PilError> {
        self.readback_with(size, staging, |bytes| Ok(bytes.to_vec()))
    }

    fn wait_for_buffer_mapping(
        &self,
        size: u64,
        buffer: &wgpu::Buffer,
        mode: wgpu::MapMode,
        stage: &str,
    ) -> Result<(), PilError> {
        let slice = buffer.slice(..size);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(mode, move |r| {
            let _ = tx.send(r);
        });
        let deadline = Instant::now() + GPU_READBACK_TIMEOUT;
        let mut empty_polls = 0usize;
        loop {
            self.poll_device(stage)?;
            match rx.try_recv() {
                Ok(Ok(())) => break,
                Ok(Err(error)) => {
                    return Err(PilError::ValueError(format!(
                        "{stage} map_async failed: {error:?}"
                    )));
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    return Err(PilError::ValueError(format!(
                        "{stage} channel closed before completion"
                    )));
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    empty_polls = empty_polls.saturating_add(1);
                    if let Some(backoff) =
                        readback_poll_backoff(true, empty_polls, Instant::now(), deadline)
                    {
                        std::thread::sleep(backoff);
                        continue;
                    }
                    let detail = self
                        .failure_detail()
                        .unwrap_or_else(|| "device did not complete the submission".into());
                    let message = format!(
                        "{stage} timed out after {}s: {detail}",
                        GPU_READBACK_TIMEOUT.as_secs()
                    );
                    self.mark_failed(message.clone());
                    // Do not leave a wedged native queue available to a later
                    // call in this process. The process-level parity runner
                    // also terminates the isolated child on its own deadline.
                    self.device.destroy();
                    return Err(PilError::ValueError(message));
                }
            }
        }
        Ok(())
    }

    /// Decode while the readback buffer is mapped, then unmap even when
    /// decoding fails. This accepts either staging or batch-owned primary
    /// storage; no later submission may use it until this function returns.
    /// The returned value cannot borrow the temporary mapping, so recycling
    /// remains safe after this function returns successfully.
    fn readback_with<T>(
        &self,
        size: u64,
        staging: &wgpu::Buffer,
        decode: impl FnOnce(&[u8]) -> Result<T, PilError>,
    ) -> Result<T, PilError> {
        self.wait_for_buffer_mapping(size, staging, wgpu::MapMode::Read, "GPU readback")?;
        let slice = staging.slice(..size);
        let data = {
            let mapped = slice.get_mapped_range();
            decode(&mapped)
        };
        let _ = slice;
        staging.unmap();
        data
    }

    fn readback_to_image(
        &self,
        w: u32,
        h: u32,
        staging: &wgpu::Buffer,
        native_rgb: bool,
    ) -> Result<DynamicImage, PilError> {
        let size = CheckedDims::new(w, h, 4)?.total_bytes() as u64;
        if native_rgb {
            return self
                .readback_with(size, staging, |bytes| rgb_from_packed_readback(w, h, bytes));
        }
        let data = self.readback_bytes(size, staging)?;

        let n = CheckedDims::new(w, h, 1)?.total_pixels();
        #[cfg(target_endian = "little")]
        {
            let expected = n * std::mem::size_of::<u32>();
            if data.len() != expected {
                return Err(PilError::ValueError(format!(
                    "GPU readback byte length {} does not match image size {expected}",
                    data.len()
                )));
            }
            return RgbaImage::from_raw(w, h, data)
                .map(DynamicImage::ImageRgba8)
                .ok_or_else(|| PilError::ValueError("bad readback buffer".into()));
        }

        #[cfg(target_endian = "big")]
        let mut rgba_bytes = Vec::with_capacity(n * 4);
        #[cfg(target_endian = "big")]
        let mut pixels = data.chunks_exact(std::mem::size_of::<u32>());
        #[cfg(target_endian = "big")]
        for _ in 0..n {
            let bytes = pixels.next().ok_or_else(|| {
                PilError::ValueError("GPU readback buffer ended before image pixels".into())
            })?;
            let pixel = u32::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
            rgba_bytes.push((pixel & 0xff) as u8);
            rgba_bytes.push(((pixel >> 8) & 0xff) as u8);
            rgba_bytes.push(((pixel >> 16) & 0xff) as u8);
            rgba_bytes.push(((pixel >> 24) & 0xff) as u8);
        }
        #[cfg(target_endian = "big")]
        if !pixels.remainder().is_empty() {
            return Err(PilError::ValueError(
                "GPU readback buffer has a partial pixel".into(),
            ));
        }

        #[cfg(target_endian = "big")]
        let img = RgbaImage::from_raw(w, h, rgba_bytes)
            .ok_or_else(|| PilError::ValueError("bad readback buffer".into()))?;
        #[cfg(target_endian = "big")]
        Ok(DynamicImage::ImageRgba8(img))
    }

    #[cfg(target_endian = "little")]
    fn readback_to_luma_alpha_from_rgba_transport(
        &self,
        w: u32,
        h: u32,
        staging: &wgpu::Buffer,
    ) -> Result<DynamicImage, PilError> {
        let size = CheckedDims::new(w, h, 4)?.total_bytes() as u64;
        self.readback_with(size, staging, |packed| la_from_rgba_transport(w, h, packed))
    }

    fn readback_to_native_channels(
        &self,
        w: u32,
        h: u32,
        channels: u8,
        staging: &wgpu::Buffer,
    ) -> Result<DynamicImage, PilError> {
        let dimensions = CheckedDims::new(w, h, channels)?;
        let native_bytes = dimensions.total_bytes();
        let expected_transfer_bytes = native_bytes
            .div_ceil(std::mem::size_of::<u32>())
            .checked_mul(std::mem::size_of::<u32>())
            .and_then(|bytes| u64::try_from(bytes).ok())
            .ok_or_else(|| {
                PilError::ValueError("GPU native image readback size overflow".into())
            })?;
        self.readback_with(expected_transfer_bytes, staging, |mapped| {
            if mapped.len() != expected_transfer_bytes as usize {
                return Err(PilError::InternalError(
                    "GPU native image readback has an unexpected byte length".into(),
                ));
            }
            let pixels = mapped[..native_bytes].to_vec();
            crate::compute::record_pipeline_allocation(pixels.len());
            crate::image_utils::raw_bytes_to_image(w, h, pixels, usize::from(channels))
        })
    }

    fn readback_to_luma8(
        &self,
        w: u32,
        h: u32,
        staging: &wgpu::Buffer,
    ) -> Result<DynamicImage, PilError> {
        let pixel_count = CheckedDims::new(w, h, 1)?.total_pixels();
        let transfer_bytes = compact_luma8_transfer_bytes(w, h)?;
        let transfer_len = usize::try_from(transfer_bytes)
            .map_err(|_| PilError::ValueError("GPU luma readback is too large".into()))?;
        self.readback_with(transfer_bytes, staging, |bytes| {
            if bytes.len() != transfer_len || bytes.len() < pixel_count {
                return Err(PilError::ValueError(format!(
                    "GPU packed-luma readback byte length {} does not match expected size {}",
                    bytes.len(),
                    transfer_len
                )));
            }
            let samples = bytes[..pixel_count].to_vec();
            crate::compute::record_pipeline_allocation(samples.len());
            crate::raster::GrayImage::from_raw(w, h, samples)
                .map(DynamicImage::ImageLuma8)
                .ok_or_else(|| PilError::ValueError("bad luma readback buffer".into()))
        })
    }

    fn readback_to_cover_native_bytes(
        &self,
        w: u32,
        h: u32,
        dispatch: NativeCoverPackedOutputDispatch,
        staging: &wgpu::Buffer,
    ) -> Result<DynamicImage, PilError> {
        let transfer_len = usize::try_from(dispatch.transfer_bytes)
            .map_err(|_| PilError::ValueError("GPU Cover readback is too large".into()))?;
        self.readback_with(dispatch.transfer_bytes, staging, |bytes| {
            if bytes.len() != transfer_len {
                return Err(PilError::InternalError(
                    "GPU Cover packed readback length mismatch".into(),
                ));
            }
            let samples =
                unpack_native_cover_packed_rows(w, h, dispatch.channels, dispatch, bytes)?;
            crate::compute::record_pipeline_allocation(samples.len());
            crate::image_utils::raw_bytes_to_image(w, h, samples, usize::from(dispatch.channels))
        })
    }

    fn readback_to_luma16(
        &self,
        w: u32,
        h: u32,
        staging: &wgpu::Buffer,
        mode: Option<&str>,
    ) -> Result<DynamicImage, PilError> {
        let size = CheckedDims::new(w, h, 4)?.total_bytes() as u64;
        let data = self.readback_bytes(size, staging)?;
        let n = CheckedDims::new(w, h, 1)?.total_pixels();
        let expected = n * std::mem::size_of::<u32>();
        if data.len() != expected {
            return Err(PilError::ValueError(format!(
                "GPU readback byte length {} does not match image size {expected}",
                data.len()
            )));
        }
        let mut pixels = Vec::with_capacity(n);
        for sample in data.chunks_exact(std::mem::size_of::<u32>()) {
            let bytes = [sample[0], sample[1]];
            pixels.push(if matches!(mode, Some("I;16B" | "I;16N")) {
                u16::from_be_bytes(bytes)
            } else {
                u16::from_ne_bytes(bytes)
            });
        }
        if !data
            .chunks_exact(std::mem::size_of::<u32>())
            .remainder()
            .is_empty()
        {
            return Err(PilError::ValueError(
                "GPU readback buffer has a partial typed sample".into(),
            ));
        }
        let image = ImageBuffer::<Luma<u16>, Vec<u16>>::from_raw(w, h, pixels)
            .ok_or_else(|| PilError::ValueError("bad typed readback buffer".into()))?;
        Ok(DynamicImage::ImageLuma16(image))
    }

    /// Read back the numeric-u16 representation used by typed Paste. Unlike
    /// I;16 geometry, this path deliberately ignores the public byte-order
    /// label: the shader blended decoded unsigned samples and wrote the
    /// result as a little-endian numeric word. The returned host image keeps
    /// those samples ready for the existing mode-preserving boundary.
    fn readback_to_luma16_numeric(
        &self,
        w: u32,
        h: u32,
        staging: &wgpu::Buffer,
    ) -> Result<DynamicImage, PilError> {
        let size = CheckedDims::new(w, h, 4)?.total_bytes() as u64;
        let data = self.readback_bytes(size, staging)?;
        let n = CheckedDims::new(w, h, 1)?.total_pixels();
        let expected = n * std::mem::size_of::<u32>();
        if data.len() != expected {
            return Err(PilError::ValueError(format!(
                "GPU readback byte length {} does not match image size {expected}",
                data.len()
            )));
        }
        let pixels = data
            .chunks_exact(std::mem::size_of::<u32>())
            .map(|sample| u16::from_le_bytes([sample[0], sample[1]]))
            .collect::<Vec<_>>();
        if !data
            .chunks_exact(std::mem::size_of::<u32>())
            .remainder()
            .is_empty()
        {
            return Err(PilError::ValueError(
                "GPU readback buffer has a partial typed sample".into(),
            ));
        }
        let image = ImageBuffer::<Luma<u16>, Vec<u16>>::from_raw(w, h, pixels)
            .ok_or_else(|| PilError::ValueError("bad typed readback buffer".into()))?;
        Ok(DynamicImage::ImageLuma16(image))
    }

    /// Stream coefficient tiles while the ordered binary64 state stays on the
    /// device. Resample.c stores FLOAT32 only after the complete axis, never at
    /// a transport boundary; marker 15 performs that one final conversion.
    fn execute_f_resize_tiled(
        &self,
        source: (u32, u32),
        destination: (u32, u32),
        horizontal: &FilterCoeffsF64,
        vertical: &FilterCoeffsF64,
        buffers: &BufferPool,
    ) -> Result<
        (
            bool,
            u32,
            u32,
            ReadbackTarget,
            PipelineResourceTelemetry,
            u64,
        ),
        PilError,
    > {
        let cached = self.resolve_pipeline(
            "__internal_resize_h",
            "resize_convolution_h.wgsl",
            include_str!("shaders/resize_convolution_h.wgsl"),
        )?;
        let storage = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST;
        let states = create_sized_buffer(
            &self.device,
            "gpu_f_resize_tile_states",
            storage,
            GPU_F_RESIZE_TILE_PIXELS * 6 * 4,
            GPU_F_RESIZE_COEFFICIENT_ALIGNMENT_BYTES,
        );
        let coefficients = create_sized_buffer(
            &self.device,
            "gpu_f_resize_tile_coefficients",
            storage,
            (6 + GPU_F_RESIZE_TILE_ROWS * (4 + GPU_F_RESIZE_TILE_TAPS * 4)) * 4,
            GPU_F_RESIZE_COEFFICIENT_ALIGNMENT_BYTES,
        );
        let parameters = create_sized_buffer(
            &self.device,
            "gpu_f_resize_tile_parameters",
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            8 * 4,
            256,
        );
        let mut current_is_a = true;
        let mut dimensions = source;
        let mut telemetry = PipelineResourceTelemetry::default();
        let mut dispatches = 0u64;
        for (axis, table, target) in [
            (0u32, horizontal, destination.0),
            (1, vertical, destination.1),
        ] {
            if target
                == if axis == 0 {
                    dimensions.0
                } else {
                    dimensions.1
                }
            {
                continue;
            }
            let output_dims = if axis == 0 {
                (target, dimensions.1)
            } else {
                (dimensions.0, target)
            };
            let line_count = if axis == 0 {
                dimensions.1
            } else {
                dimensions.0
            } as usize;
            let (source_buffer, destination_buffer) = if current_is_a {
                (&buffers.buf_a, &buffers.buf_b)
            } else {
                (&buffers.buf_b, &buffers.buf_a)
            };
            for row_start in (0..table.count.len()).step_by(GPU_F_RESIZE_TILE_ROWS) {
                let rows = GPU_F_RESIZE_TILE_ROWS.min(table.count.len() - row_start);
                let lines_per_tile = GPU_F_RESIZE_TILE_PIXELS / rows;
                let max_taps = table.count[row_start..row_start + rows]
                    .iter()
                    .copied()
                    .max()
                    .unwrap_or(0);
                for line_start in (0..line_count).step_by(lines_per_tile) {
                    let lines = lines_per_tile.min(line_count - line_start);
                    let header = [
                        axis,
                        row_start as u32,
                        line_start as u32,
                        rows as u32,
                        lines as u32,
                        0,
                    ];
                    for tap_start in (0..max_taps.max(1)).step_by(GPU_F_RESIZE_TILE_TAPS) {
                        let mut words = header.to_vec();
                        words[5] = tap_start as u32;
                        words.resize(6 + rows * 4, 0);
                        for row in 0..rows {
                            let index = row_start + row;
                            let total = table.count[index];
                            let end = total.min(tap_start.saturating_add(GPU_F_RESIZE_TILE_TAPS));
                            let start = total.min(tap_start);
                            let offset = words.len() as u32;
                            words[6 + row * 4..6 + (row + 1) * 4].copy_from_slice(&[
                                table.xmin[index] as u32,
                                total as u32,
                                (end - start) as u32,
                                offset,
                            ]);
                            for &weight in &table.weights[index][start..end] {
                                let parts = gpu_f64_integer_parts(weight).ok_or_else(|| {
                                    PilError::InternalError(
                                        "GPU tiled F coefficient is invalid".into(),
                                    )
                                })?;
                                words.extend([
                                    parts.mantissa as u32,
                                    (parts.mantissa >> 32) as u32,
                                    parts.exponent as u32,
                                    u32::from(parts.negative),
                                ]);
                            }
                        }
                        self.queue
                            .write_buffer(&coefficients, 0, bytemuck::cast_slice(&words));
                        telemetry.auxiliary_bytes += (words.len() * 4) as u64;
                        let params = [
                            dimensions.0,
                            dimensions.1,
                            8,
                            0,
                            output_dims.0,
                            output_dims.1,
                            1,
                            14,
                        ];
                        self.queue
                            .write_buffer(&parameters, 0, bytemuck::cast_slice(&params));
                        telemetry.parameter_bytes += 32;
                        self.submit_f_resize_tile(
                            &cached,
                            source_buffer,
                            &states,
                            &parameters,
                            &coefficients,
                            rows as u32,
                            lines as u32,
                        )?;
                        dispatches += 1;
                        if dispatches % 8 == 0 {
                            self.device.poll(wgpu::Maintain::Wait);
                            self.ensure_healthy("GPU tiled F resize bounded queue")?;
                        }
                    }
                    let params = [
                        dimensions.0,
                        dimensions.1,
                        8,
                        0,
                        output_dims.0,
                        output_dims.1,
                        1,
                        15,
                    ];
                    self.queue
                        .write_buffer(&parameters, 0, bytemuck::cast_slice(&params));
                    telemetry.parameter_bytes += 32;
                    self.submit_f_resize_tile(
                        &cached,
                        &states,
                        destination_buffer,
                        &parameters,
                        &coefficients,
                        rows as u32,
                        lines as u32,
                    )?;
                    dispatches += 1;
                }
            }
            current_is_a = !current_is_a;
            dimensions = output_dims;
        }
        let size = CheckedDims::new(destination.0, destination.1, 4)?.total_bytes() as u64;
        let result_buffer = if current_is_a {
            &buffers.buf_a
        } else {
            &buffers.buf_b
        };
        let readback = self.prepare_readback(result_buffer, size)?;
        if let ReadbackTarget::Staging(staging) = &readback {
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("gpu_f_resize_tile_readback"),
                });
            encoder.copy_buffer_to_buffer(result_buffer, 0, &staging.buffer, 0, size);
            self.queue.submit(Some(encoder.finish()));
            self.poll_device("GPU tiled F resize readback")?;
        } else if dispatches == 0 {
            // An unchanged shape may leave only a pending queue upload. Flush
            // it before mapping even when there was no compute submission.
            self.queue.submit(std::iter::empty());
            self.poll_device("GPU tiled F resize identity upload")?;
        }
        Ok((
            current_is_a,
            destination.0,
            destination.1,
            readback,
            telemetry,
            dispatches,
        ))
    }

    fn submit_f_resize_tile(
        &self,
        cached: &CachedPipeline,
        input: &wgpu::Buffer,
        output: &wgpu::Buffer,
        parameters: &wgpu::Buffer,
        coefficients: &wgpu::Buffer,
        rows: u32,
        lines: u32,
    ) -> Result<(), PilError> {
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gpu_f_resize_tile"),
            layout: &cached.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: input.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: output.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: parameters.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: coefficients.as_entire_binding(),
                },
            ],
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("gpu_f_resize_tile"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("gpu_f_resize_tile"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&cached.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(rows.div_ceil(16), lines.div_ceil(16), 1);
        }
        crate::compute::record_gpu_shader_dispatch(
            cached.variant_name,
            cached.shader_file,
            u64::from(rows.div_ceil(16)) * u64::from(lines.div_ceil(16)),
        );
        // Queue order protects both the state and the reused coefficient arena.
        // Poll between submissions so device errors surface at the owning tile.
        self.queue.submit(Some(encoder.finish()));
        self.poll_device("GPU tiled F resize submission")
    }

    fn execute_batch_impl(
        &self,
        ops: &[PipelineOp],
        auxiliary_images: &[AuxiliaryImages],
        w: u32,
        h: u32,
        mode: u32,
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
        packed_la_pad: bool,
        native_extract_band: bool,
        native_grayscale_rgb_input: bool,
        native_sharpness_l_input: bool,
        native_sharpness_la_input: bool,
        native_sharpness_rgb_input: bool,
        native_reduce_rgb_input: bool,
        native_expand_channels: Option<u8>,
        native_cover_input_channels: Option<u8>,
        native_cover_packed_output: Option<NativeCoverPackedOutputDispatch>,
        logical_mode: Option<&str>,
        contrast_mean: Option<u8>,
        f_resize_constant_bits: Option<u32>,
        f_resize_box_copy_is_exact: bool,
        f_resize_identity_is_exact: bool,
        f_resize_box_average_is_exact: bool,
        f_resize_dyadic_is_exact: bool,
        f_resize_f64_is_exact: bool,
        f_resize_f64_ordered_is_exact: bool,
        initial_input: Option<&wgpu::Buffer>,
        buffers: &mut BufferPool,
    ) -> Result<
        (
            bool,
            u32,
            u32,
            ReadbackTarget,
            PipelineResourceTelemetry,
            u64,
            bool,
        ),
        PilError,
    > {
        if logical_mode == Some("F") && (f_resize_f64_is_exact || f_resize_f64_ordered_is_exact) {
            if let [
                PipelineOp::Resize {
                    w: out_w,
                    h: out_h,
                    filter,
                },
            ] = ops
            {
                if !matches!(filter, ResampleFilter::Nearest)
                    && !(matches!(filter, ResampleFilter::Box)
                        && gpu_f_resize_compact_box_any_axis((w, h), (*out_w, *out_h)))
                {
                    let (kernel, support) = filter_from_resample(*filter);
                    let horizontal = precompute_coeffs_f64(*out_w, w, kernel, support);
                    let vertical = precompute_coeffs_f64(*out_h, h, kernel, support);
                    if gpu_f_resize_coefficients_need_tiles(&horizontal)
                        || gpu_f_resize_coefficients_need_tiles(&vertical)
                    {
                        return self
                            .execute_f_resize_tiled(
                                (w, h),
                                (*out_w, *out_h),
                                &horizontal,
                                &vertical,
                                buffers,
                            )
                            .map(
                                |(current_is_a, width, height, readback, telemetry, dispatches)| {
                                    (
                                        current_is_a,
                                        width,
                                        height,
                                        readback,
                                        telemetry,
                                        dispatches,
                                        false,
                                    )
                                },
                            );
                    }
                }
            }
        }
        let mut current_is_a = true;
        let mut cur_w = w;
        let mut cur_h = h;
        let mut dispatch_count = 0u64;
        let mut native_expand_output = false;
        gpu_log!(
            "[GPU] batch_impl: {} ops, start dims {}x{}",
            ops.len(),
            cur_w,
            cur_h
        );
        if ops.len() != auxiliary_images.len() {
            return Err(PilError::InternalError(
                "GPU operation and auxiliary-image counts differ".into(),
            ));
        }

        let limits = self.device.limits();
        let uniform_alignment = limits.min_uniform_buffer_offset_alignment as usize;
        let storage_alignment = limits.min_storage_buffer_offset_alignment as usize;
        let BatchExecutionEstimates {
            resource_bytes,
            shader_work_items,
        } = self.estimate_batch_resources(
            ops,
            auxiliary_images,
            (w, h),
            mode,
            BatchEstimateOptions {
                uniform_alignment,
                storage_alignment,
                packed_luma_putdata,
                logical_mode,
                f_resize_f64_is_exact,
                f_resize_f64_ordered_is_exact,
            },
            buffers,
        )?;
        let auxiliary_cache = GpuAuxiliaryCache::from_batch(
            ops,
            auxiliary_images,
            mode,
            buffers.capacity,
            storage_alignment,
        )?;
        self.upload_auxiliary_cache(&auxiliary_cache, buffers, storage_alignment);
        let mut chunk_start = 0usize;
        let mut submission_index = 0usize;
        let mut readback = None;
        let mut resource_telemetry = PipelineResourceTelemetry {
            auxiliary_bytes: auxiliary_cache.total_bytes() as u64,
            ..PipelineResourceTelemetry::default()
        };
        let mut current_mode = mode;
        while chunk_start < ops.len() {
            self.ensure_healthy("GPU batch submission")?;
            let chunk_end = select_gpu_chunk_end(chunk_start, &resource_bytes, &shader_work_items)
                .ok_or_else(|| {
                    PilError::InternalError(
                    "GPU chunk scheduling made no progress because its estimates are inconsistent"
                        .into(),
                )
                })?;
            let chunk_ops = &ops[chunk_start..chunk_end];
            let chunk_dispatch_count = if (packed_native_byte_filter
                && matches!(logical_mode, None | Some("L"))
                || native_rgb_packed_output && matches!(logical_mode, None | Some("RGB")))
                && matches!(chunk_ops, [PipelineOp::BoxBlur { radius: 1 }])
            {
                1
            } else {
                gpu_dispatch_count(chunk_ops, logical_mode, (cur_w, cur_h))
            };
            dispatch_count = dispatch_count.saturating_add(chunk_dispatch_count);
            resource_telemetry.fused_operation_count = resource_telemetry
                .fused_operation_count
                .saturating_add(gpu_fused_operation_count(
                    chunk_ops,
                    logical_mode,
                    (cur_w, cur_h),
                ));
            let estimated_bytes = resource_bytes[chunk_start..chunk_end]
                .iter()
                .fold(0usize, |total, bytes| total.saturating_add(*bytes));
            let estimated_work = shader_work_items[chunk_start..chunk_end]
                .iter()
                .fold(0u64, |total, work| total.saturating_add(*work));

            let mut prepared = self.prepare_batch(
                &ops[chunk_start..chunk_end],
                &auxiliary_images[chunk_start..chunk_end],
                cur_w,
                cur_h,
                current_mode,
                logical_mode,
                contrast_mean,
                f_resize_constant_bits,
                f_resize_box_copy_is_exact,
                f_resize_identity_is_exact,
                f_resize_box_average_is_exact,
                f_resize_dyadic_is_exact,
                f_resize_f64_is_exact,
                f_resize_f64_ordered_is_exact,
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
                native_extract_band,
                native_grayscale_rgb_input,
                native_sharpness_l_input,
                native_sharpness_la_input,
                native_sharpness_rgb_input,
                native_reduce_rgb_input,
                native_expand_channels,
                native_cover_input_channels,
                native_cover_packed_output,
                buffers,
                &auxiliary_cache,
            )?;
            if chunk_start == 0 {
                prepared.resources.initial_input = initial_input;
            }
            native_expand_output |= prepared.resources.native_expand_output.is_some();
            resource_telemetry.parameter_bytes = resource_telemetry
                .parameter_bytes
                .saturating_add(prepared.resource_telemetry.parameter_bytes);
            resource_telemetry.auxiliary_bytes = resource_telemetry
                .auxiliary_bytes
                .saturating_add(prepared.resource_telemetry.auxiliary_bytes);
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("gpu_batch_encoder"),
                });
            current_is_a = self.encode_batch(
                &mut encoder,
                &ops[chunk_start..chunk_end],
                &prepared,
                current_is_a,
                logical_mode,
                packed_luma_point,
                packed_luma_putdata,
                packed_luma_order_statistic,
                packed_native_byte_filter,
                packed_luma_pad,
                packed_la_pad,
                packed_luma_transform_output,
                native_sharpness_l_input,
                native_sharpness_la_input,
                native_sharpness_rgb_input,
            )?;

            let final_dims = prepared.final_dims;
            if chunk_end == ops.len() {
                let size = if let Some(dispatch) = prepared.resources.native_cover_packed_output {
                    dispatch.transfer_bytes
                } else if let Some(dispatch) = prepared.resources.native_expand_output {
                    dispatch.transfer_bytes
                } else if let Some(dispatch) = prepared.resources.native_rgb_packed_output {
                    dispatch.transfer_bytes
                } else if let Some(dispatch) = prepared.resources.native_la_transform_output {
                    dispatch.transfer_bytes
                } else if packed_luma_transform_output {
                    compact_luma8_transfer_bytes(final_dims.0, final_dims.1)?
                } else if packed_la_pad {
                    compact_native_channel_transfer_bytes(final_dims.0, final_dims.1, 2)?
                } else if packed_luma_point
                    || packed_luma_putdata
                    || packed_luma_order_statistic
                    || packed_native_byte_filter
                    || packed_luma_pad
                    || native_sharpness_l_input
                    || matches!(
                        ops.last(),
                        Some(PipelineOp::ExtractBand { .. } | PipelineOp::Grayscale)
                    )
                {
                    compact_luma8_transfer_bytes(final_dims.0, final_dims.1)?
                } else {
                    CheckedDims::new(final_dims.0, final_dims.1, 4)?.total_bytes() as u64
                };
                let src = if current_is_a {
                    prepared.resources.buf_a
                } else {
                    prepared.resources.buf_b
                };
                let target = self.prepare_readback(src, size)?;
                if let ReadbackTarget::Staging(staging) = &target {
                    // Keep the staged copy after compute in the same command
                    // buffer. Direct mapping instead waits on this submission's
                    // final primary-buffer use, without an extra device copy.
                    encoder.copy_buffer_to_buffer(src, 0, &staging.buffer, 0, size);
                }
                readback = Some(target);
            }
            self.queue.submit(Some(encoder.finish()));
            // The queue preserves submission order: the next chunk's writes
            // to these arenas are ordered after this chunk's reads. Release
            // the borrow only after the command buffer has captured it.
            drop(prepared);
            self.poll_device("GPU batch submission")?;
            submission_index += 1;
            gpu_log!(
                "[GPU] submitted chunk={} ops={}..{} resources={} bytes work={}",
                submission_index,
                chunk_start,
                chunk_end,
                estimated_bytes,
                estimated_work
            );
            (cur_w, cur_h) = final_dims;
            current_mode = gpu_mode_after_ops(current_mode, &ops[chunk_start..chunk_end]);
            chunk_start = chunk_end;
        }
        // After all chunks, current_is_a tracks where the latest result lives:
        //   true → buf_a has the final result, false → buf_b. Queue ordering
        // keeps chunk submissions dependent without a blocking poll between them.
        let readback = readback.ok_or_else(|| {
            PilError::InternalError("GPU batch produced no readback target".into())
        })?;
        Ok((
            current_is_a,
            cur_w,
            cur_h,
            readback,
            resource_telemetry,
            dispatch_count,
            native_expand_output,
        ))
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

    pub(crate) fn cmyk_projection_terminal_supported(
        image: &DynamicImage,
        ops: &[PipelineOp],
    ) -> bool {
        !ops.is_empty() && Self::projection_terminal_supported(image, ops, 4)
    }

    pub(crate) fn rgb_projection_supported(image: &DynamicImage) -> bool {
        Self::projection_terminal_supported(image, &[], 3)
    }

    pub(crate) fn rgb_projection_terminal_supported(
        image: &DynamicImage,
        ops: &[PipelineOp],
    ) -> bool {
        !ops.is_empty() && Self::projection_terminal_supported(image, ops, 3)
    }

    fn projection_terminal_supported(
        image: &DynamicImage,
        ops: &[PipelineOp],
        channels: u8,
    ) -> bool {
        let source = match (channels, image) {
            (3, DynamicImage::ImageRgb8(pixels)) => pixels.as_raw().as_slice(),
            (4, DynamicImage::ImageRgba8(pixels)) => pixels.as_raw().as_slice(),
            _ => return false,
        };
        let Ok(dimensions) = CheckedDims::new(image.width(), image.height(), channels) else {
            return false;
        };
        if image.width() == 0
            || image.height() == 0
            || ops.len() > 128
            || dimensions.total_bytes() % 4 != 0
            || !ops
                .iter()
                .all(|op| matches!(op, PipelineOp::PutPixel { .. }))
        {
            return false;
        }
        if dimensions.total_bytes() != source.len() {
            return false;
        }
        let Ok(gpu) = Self::ensure_init() else {
            return false;
        };
        let limits = gpu.device.limits();
        let width = u64::from(image.width());
        let height = u64::from(image.height());
        let pixels = dimensions.total_pixels() as u64;
        let output_words = u64::from(image.width()).saturating_add(u64::from(image.height()));
        let Some(output_bytes) = output_words.checked_mul(4) else {
            return false;
        };
        let capacity_words = pixels.max(output_words).max(1);
        let Some(capacity_bytes) = capacity_words.checked_mul(4) else {
            return false;
        };
        let groups_x = width.div_ceil(16);
        let groups_y = height
            .div_ceil(16)
            .checked_add(u64::from(ops.iter().any(|op| match op {
                PipelineOp::PutPixel { color, .. } => {
                    color.0 != 0 || color.1 != 0 || color.2 != 0 || (channels == 4 && color.3 != 0)
                }
                _ => false,
            })))
            .unwrap_or(u64::MAX);
        if pixels > u64::from(GPU_BUFFER_CAPACITY)
            || capacity_words > u64::from(GPU_BUFFER_CAPACITY)
            || capacity_bytes > limits.max_buffer_size
            || capacity_bytes > u64::from(limits.max_storage_buffer_binding_size)
            || output_bytes > u64::from(limits.max_storage_buffer_binding_size)
            || groups_x > u64::from(limits.max_compute_workgroups_per_dimension)
            || groups_y > u64::from(limits.max_compute_workgroups_per_dimension)
        {
            return false;
        }
        ops.iter().all(|op| match op {
            PipelineOp::PutPixel { x, y, .. } => *x < image.width() && *y < image.height(),
            _ => false,
        })
    }

    pub(crate) fn execute_cmyk_projection_terminal(
        &self,
        image: &DynamicImage,
        ops: &[PipelineOp],
        source_is_known_zero: bool,
    ) -> Result<(Vec<u32>, Vec<u32>, PipelineResourceTelemetry, u64), PilError> {
        if !Self::cmyk_projection_terminal_supported(image, ops) {
            return Err(PilError::NotImplementedError(
                "GPU does not support this CMYK projection terminal".into(),
            ));
        }
        Self::ensure_init()?.execute_projection_terminal(image, ops, 4, source_is_known_zero)
    }

    pub(crate) fn execute_rgb_projection(
        &self,
        image: &DynamicImage,
    ) -> Result<(Vec<u32>, Vec<u32>, PipelineResourceTelemetry, u64), PilError> {
        self.execute_rgb_projection_terminal(image, &[], false)
    }

    pub(crate) fn execute_rgb_projection_terminal(
        &self,
        image: &DynamicImage,
        ops: &[PipelineOp],
        source_is_known_zero: bool,
    ) -> Result<(Vec<u32>, Vec<u32>, PipelineResourceTelemetry, u64), PilError> {
        let supported = if ops.is_empty() {
            Self::rgb_projection_supported(image)
        } else {
            Self::rgb_projection_terminal_supported(image, ops)
        };
        if !supported {
            return Err(PilError::NotImplementedError(
                "GPU does not support this RGB projection terminal".into(),
            ));
        }
        Self::ensure_init()?.execute_projection_terminal(image, ops, 3, source_is_known_zero)
    }
}

impl GpuInner {
    fn create_projection_bind_group(
        &self,
        layout: &wgpu::BindGroupLayout,
        source: &wgpu::Buffer,
        source_bytes: u64,
        output: &wgpu::Buffer,
        output_bytes: u64,
        params: &wgpu::Buffer,
        overrides: &wgpu::Buffer,
        override_bytes: u64,
    ) -> Result<wgpu::BindGroup, PilError> {
        Ok(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gpu_getprojection"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: ranged_binding(
                        source,
                        Some(BufferRange {
                            offset: 0,
                            size: source_bytes,
                        }),
                    )?,
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: ranged_binding(
                        output,
                        Some(BufferRange {
                            offset: 0,
                            size: output_bytes,
                        }),
                    )?,
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: ranged_binding(
                        params,
                        Some(BufferRange {
                            offset: 0,
                            size: 32,
                        }),
                    )?,
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: ranged_binding(
                        overrides,
                        Some(BufferRange {
                            offset: 0,
                            size: override_bytes,
                        }),
                    )?,
                },
            ],
        }))
    }

    fn execute_projection_terminal(
        &self,
        image: &DynamicImage,
        ops: &[PipelineOp],
        channels: u8,
        source_is_known_zero: bool,
    ) -> Result<(Vec<u32>, Vec<u32>, PipelineResourceTelemetry, u64), PilError> {
        let source: &[u8] = match (channels, image) {
            (3, DynamicImage::ImageRgb8(pixels)) => pixels.as_raw(),
            (4, DynamicImage::ImageRgba8(pixels)) => pixels.as_raw(),
            _ => {
                return Err(PilError::InternalError(
                    "GPU projection received an unsupported native carrier".into(),
                ));
            }
        };
        let (width, height) = image.dimensions();
        let dimensions = CheckedDims::new(width, height, channels)?;
        let source_bytes = dimensions.total_bytes();
        if source.len() != source_bytes {
            return Err(PilError::InternalError(
                "GPU projection source is not tightly packed".into(),
            ));
        }
        // A zero-filled source contributes no projected pixels. The sparse
        // override kernel can handle this case without transferring or
        // dispatching across the full frame.
        // A constructor-proven zero fill avoids the CPU preflight scan. Other
        // sources still require an exact scan before the zero-upload shader
        // route can be admitted.
        let source_is_zero =
            !ops.is_empty() && (source_is_known_zero || projection_source_is_zero(source));

        let mut pending = Vec::with_capacity(ops.len());
        for (order, op) in ops.iter().enumerate() {
            let PipelineOp::PutPixel { x, y, color, .. } = op else {
                return Err(PilError::InternalError(
                    "GPU projection received a non-pixel operation".into(),
                ));
            };
            if *x >= width || *y >= height {
                return Err(PilError::IndexError("image index out of range".into()));
            }
            let index = y
                .checked_mul(width)
                .and_then(|row| row.checked_add(*x))
                .ok_or_else(|| PilError::ValueError("GPU projection index overflow".into()))?;
            let nonzero = u32::from(
                color.0 != 0 || color.1 != 0 || color.2 != 0 || (channels == 4 && color.3 != 0),
            );
            pending.push((index, order, nonzero));
        }
        pending.sort_unstable_by_key(|(index, order, _)| (*index, *order));
        let mut overrides: Vec<(u32, u32)> = Vec::with_capacity(pending.len());
        for (index, _, nonzero) in pending {
            if let Some((previous_index, previous_nonzero)) = overrides.last_mut()
                && *previous_index == index
            {
                *previous_nonzero = nonzero;
            } else {
                overrides.push((index, nonzero));
            }
        }
        let has_nonzero_override = overrides.iter().any(|(_, nonzero)| *nonzero != 0);

        let output_words = width
            .checked_add(height)
            .ok_or_else(|| PilError::ValueError("GPU projection size overflow".into()))?;
        let output_bytes = usize::try_from(output_words)
            .ok()
            .and_then(|words| words.checked_mul(std::mem::size_of::<u32>()))
            .ok_or_else(|| PilError::ValueError("GPU projection is too large".into()))?;
        // A known-zero source contributes only the sparse overrides. Pack the
        // two axis vectors into bitsets while they are on the GPU, then expand
        // the public Vec<u32> results after the smaller readback completes.
        let packed_output = source_is_zero;
        let packed_column_words = width.div_ceil(32) as usize;
        let packed_row_words = height.div_ceil(32) as usize;
        let transfer_bytes_len = if packed_output {
            packed_column_words
                .checked_add(packed_row_words)
                .and_then(|words| words.checked_mul(std::mem::size_of::<u32>()))
                .ok_or_else(|| PilError::ValueError("GPU projection is too large".into()))?
        } else {
            output_bytes
        };
        let transfer_bytes = u64::try_from(transfer_bytes_len)
            .map_err(|_| PilError::ValueError("GPU projection is too large".into()))?;
        // The zero-source shader branch never reads the frame binding. Give
        // it one valid storage word and size the working set for the packed
        // result instead of allocating two full-frame buffers.
        let capacity_words = u32::try_from(if packed_output {
            (transfer_bytes_len / std::mem::size_of::<u32>()).max(1)
        } else {
            dimensions.total_pixels().max(output_words as usize).max(1)
        })
        .map_err(|_| PilError::ValueError("GPU projection working set is too large".into()))?;
        let mut buffers = self.acquire_buffers(capacity_words)?;

        let uniform_alignment = self.device.limits().min_uniform_buffer_offset_alignment as usize;
        buffers.params_arena.ensure_capacity(
            &self.device,
            "gpu_getprojection_params",
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            32,
            uniform_alignment,
        );
        let mut params = [0u8; 32];
        params[0..4].copy_from_slice(&width.to_le_bytes());
        params[4..8].copy_from_slice(&height.to_le_bytes());
        params[8..12].copy_from_slice(&(overrides.len() as u32).to_le_bytes());
        let has_zero_override = overrides.iter().any(|(_, nonzero)| *nonzero == 0);
        params[12..16].copy_from_slice(&u32::from(has_zero_override).to_le_bytes());
        params[16..20].copy_from_slice(&(channels as u32).to_le_bytes());
        params[20..24].copy_from_slice(&u32::from(source_is_zero).to_le_bytes());
        params[24..28].copy_from_slice(&u32::from(packed_output).to_le_bytes());

        let mut override_bytes = Vec::with_capacity(overrides.len() * 8);
        for (index, nonzero) in &overrides {
            override_bytes.extend_from_slice(&index.to_le_bytes());
            override_bytes.extend_from_slice(&nonzero.to_le_bytes());
        }
        #[cfg(target_endian = "little")]
        if !source_is_zero {
            if self.direct_primary_readback {
                let source_transfer_bytes = u64::try_from(source_bytes).map_err(|_| {
                    PilError::ValueError("GPU projection source is too large".into())
                })?;
                self.upload_native_bytes_mapped(
                    source,
                    source_bytes,
                    source_transfer_bytes,
                    &mut buffers,
                )?;
            } else {
                self.queue.write_buffer(&buffers.buf_a, 0, source);
            }
        }
        #[cfg(not(target_endian = "little"))]
        if !source_is_zero {
            self.queue.write_buffer(&buffers.buf_a, 0, source);
        }
        self.queue
            .write_buffer(&buffers.params_arena.buffer, 0, &params);
        if !override_bytes.is_empty() {
            self.queue
                .write_buffer(&buffers.lut_buf, 0, &override_bytes);
        }

        let cached = self.resolve_pipeline(
            "__internal_getprojection_atomic",
            "getprojection_atomic.wgsl",
            include_str!("shaders/getprojection_atomic.wgsl"),
        )?;
        let transient_bind_group;
        let bind_group = if source_is_zero {
            let params_capacity_bytes = buffers.params_arena.capacity_bytes;
            if buffers
                .projection_zero_bind_group
                .as_ref()
                .is_none_or(|(capacity, _)| *capacity != params_capacity_bytes)
            {
                let output_binding_bytes = buffers.buf_b.size().min(u64::from(
                    self.device.limits().max_storage_buffer_binding_size,
                ));
                if output_binding_bytes < transfer_bytes {
                    return Err(PilError::InternalError(
                        "GPU projection pooled output binding is too small".into(),
                    ));
                }
                let bind_group = self.create_projection_bind_group(
                    &cached.bind_group_layout,
                    &buffers.buf_a,
                    std::mem::size_of::<u32>() as u64,
                    &buffers.buf_b,
                    output_binding_bytes,
                    &buffers.params_arena.buffer,
                    &buffers.lut_buf,
                    buffers.lut_buf.size(),
                )?;
                buffers.projection_zero_bind_group = Some((params_capacity_bytes, bind_group));
            }
            buffers
                .projection_zero_bind_group
                .as_ref()
                .map(|(_, bind_group)| bind_group)
                .expect("zero-source projection bind group was created")
        } else {
            #[cfg(target_endian = "little")]
            let source_buffer = if self.direct_primary_readback {
                &buffers
                    .mapped_native_input
                    .as_ref()
                    .expect("successful mapped projection upload retains its source")
                    .buffer
            } else {
                &buffers.buf_a
            };
            #[cfg(not(target_endian = "little"))]
            let source_buffer = &buffers.buf_a;
            transient_bind_group = self.create_projection_bind_group(
                &cached.bind_group_layout,
                source_buffer,
                source_bytes as u64,
                &buffers.buf_b,
                transfer_bytes,
                &buffers.params_arena.buffer,
                &buffers.lut_buf,
                override_bytes.len().max(8) as u64,
            )?;
            &transient_bind_group
        };
        let readback = self.prepare_readback(&buffers.buf_b, transfer_bytes)?;
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("gpu_getprojection"),
            });
        encoder.clear_buffer(&buffers.buf_b, 0, Some(transfer_bytes));
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("gpu_getprojection"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&cached.pipeline);
            pass.set_bind_group(0, bind_group, &[]);
            let (groups_x, groups_y) = if source_is_zero {
                (1, 1)
            } else {
                (
                    width.div_ceil(16),
                    height.div_ceil(16) + u32::from(has_nonzero_override),
                )
            };
            pass.dispatch_workgroups(groups_x, groups_y, 1);
            crate::compute::record_gpu_shader_dispatch(
                cached.variant_name,
                cached.shader_file,
                u64::from(groups_x) * u64::from(groups_y),
            );
        }
        if let ReadbackTarget::Staging(staging) = &readback {
            encoder.copy_buffer_to_buffer(&buffers.buf_b, 0, &staging.buffer, 0, transfer_bytes);
        }
        self.queue.submit(Some(encoder.finish()));
        self.poll_device("GPU projection submission")?;

        // Mapping the result polls the submitted work until readback is ready.
        let (horizontal, vertical) =
            self.readback_with(transfer_bytes, readback.buffer(&buffers, false), |mapped| {
                if mapped.len() != transfer_bytes_len {
                    return Err(PilError::InternalError(
                        "GPU projection readback length mismatch".into(),
                    ));
                }
                let (horizontal, vertical) = if packed_output {
                    let packed = |index: usize| {
                        let start = index * std::mem::size_of::<u32>();
                        u32::from_le_bytes([
                            mapped[start],
                            mapped[start + 1],
                            mapped[start + 2],
                            mapped[start + 3],
                        ])
                    };
                    let horizontal = (0..width as usize)
                        .map(|index| (packed(index / 32) >> (index % 32)) & 1)
                        .collect::<Vec<_>>();
                    let vertical = (0..height as usize)
                        .map(|index| (packed(packed_column_words + index / 32) >> (index % 32)) & 1)
                        .collect::<Vec<_>>();
                    (horizontal, vertical)
                } else {
                    let horizontal = (0..width as usize)
                        .map(|index| {
                            let word_offset = index * 4;
                            u32::from_le_bytes([
                                mapped[word_offset],
                                mapped[word_offset + 1],
                                mapped[word_offset + 2],
                                mapped[word_offset + 3],
                            ])
                        })
                        .collect::<Vec<_>>();
                    let vertical_base = width as usize * 4;
                    let vertical = (0..height as usize)
                        .map(|index| {
                            let word_offset = vertical_base + index * 4;
                            u32::from_le_bytes([
                                mapped[word_offset],
                                mapped[word_offset + 1],
                                mapped[word_offset + 2],
                                mapped[word_offset + 3],
                            ])
                        })
                        .collect::<Vec<_>>();
                    (horizontal, vertical)
                };
                crate::compute::record_pipeline_allocation(width as usize * 4);
                crate::compute::record_pipeline_allocation(height as usize * 4);
                Ok((horizontal, vertical))
            })?;
        let dispatches = 1;
        let resource = PipelineResourceTelemetry {
            upload_bytes: if source_is_zero {
                0
            } else {
                source_bytes as u64
            },
            readback_bytes: transfer_bytes,
            auxiliary_bytes: override_bytes.len() as u64,
            parameter_bytes: params.len() as u64,
            retained_cache_bytes: buffers.retained_bytes(),
            full_frame_copy_count: u64::from(!source_is_zero)
                + u64::from(
                    transfer_bytes >= source_bytes as u64
                        && matches!(&readback, ReadbackTarget::Staging(_)),
                ),
            fused_operation_count: ops.len() as u64,
            ..PipelineResourceTelemetry::default()
        };
        crate::compute::record_pipeline_dispatch_count(dispatches);
        if let ReadbackTarget::Staging(staging) = readback {
            self.recycle_staging(staging);
        }
        self.recycle_buffers(buffers);
        Ok((horizontal, vertical, resource, dispatches))
    }
}

/// Shared logical-mode admission for queued submission and device execution.
/// Inspecting an already-loaded source never executes an image operation.
fn gpu_logical_mode_is_supported(
    ops: &[PipelineOp],
    img: Option<&DynamicImage>,
    mode: Option<&str>,
    f_resize_constant_bits: Option<u32>,
    f_pad_f64_is_exact: bool,
    inspect_auxiliary_modes: bool,
) -> bool {
    mode.is_none_or(|logical_mode| {
        matches!(logical_mode, "L" | "LA" | "RGB" | "RGBA")
                || (logical_mode == "1"
                    && img.is_none_or(|img| matches!(img, DynamicImage::ImageLuma8(_)))
                    && matches!(ops, [PipelineOp::Flip]))
                // YCbCr shares RGB8 transport but grayscale reads only its Y
                // sample; admit exactly the singleton kernel that carries
                // this native-mode interpretation in its fourth uniform.
                || (logical_mode == "YCbCr"
                    && matches!(ops, [PipelineOp::Grayscale])
                    && img.is_none_or(|img| gpu_native_grayscale_rgb_input(ops, img, mode)))
                // On little-endian targets, HSV uses the same packed
                // three-byte storage as RGB. A singleton transpose only
                // relocates complete triplets; it does not interpret or
                // convert H/S/V samples. Keep other targets on the proven
                // semantic-control path until their native upload is covered.
                || (cfg!(target_endian = "little")
                    && logical_mode == "HSV"
                    && matches!(ops, [PipelineOp::Transpose { .. }])
                    && img.is_none_or(|img| matches!(img, DynamicImage::ImageRgb8(_))))
                // LAB shares RGB8 storage. The core has already encoded its
                // public signed A/B samples into the stored +128 byte domain,
                // so the raw three-channel PutPixel shader is exact here.
                || (logical_mode == "LAB"
                    && ops.iter().all(|op| {
                        matches!(
                            op,
                            PipelineOp::PutPixel {
                                palette_index: false,
                                ..
                            }
                        )
                    }))
                || (cfg!(target_endian = "little")
                    && matches!(ops, [PipelineOp::Multiply { .. }])
                    && matches!(logical_mode, "1" | "L" | "P" | "LA" | "La" | "PA" | "RGB" | "HSV" | "YCbCr" | "RGBA" | "RGBa" | "RGBX" | "CMYK")
                    && img.is_none_or(|img| gpu_native_multiply_channels(img, mode).is_some()))
                || (logical_mode == "La" && ops.iter().all(gpu_transform_is_projective))
                // ImageDraw's geometry is scan-converted by the exact host
                // canvas before the packed draw shader copies the complete
                // result.  The data-plane therefore preserves raw indexed,
                // typed-word, and native color bytes just like the ordinary
                // byte modes; no shader arithmetic interprets those samples.
                || (matches!(
                    logical_mode,
                    "1" | "P" | "PA" | "RGBX" | "RGBa" | "CMYK" | "HSV" | "YCbCr" | "I" | "F"
                ) && !ops.is_empty()
                    && ops
                        .iter()
                        .all(crate::compute::pool_cpu::ops::draw::is_draw_op))
                || (matches!(logical_mode, "P" | "PA")
                    && ops.iter().all(|op| {
                        matches!(
                            op,
                                PipelineOp::Add { .. }
                                | PipelineOp::Subtract { .. }
                                | PipelineOp::Multiply { .. }
                                | PipelineOp::Screen { .. }
                                | PipelineOp::Darker { .. }
                                | PipelineOp::Lighter { .. }
                                | PipelineOp::Difference { .. }
                                | PipelineOp::Overlay { .. }
                                | PipelineOp::HardLight { .. }
                                | PipelineOp::SoftLight { .. }
                                | PipelineOp::AddModulo { .. }
                                | PipelineOp::SubtractModulo { .. }
                                | PipelineOp::PutAlpha { .. }
                                | PipelineOp::PutAlphaData { .. }
                                | PipelineOp::PutData { .. }
                                | PipelineOp::ExtractBand { .. }
                                | PipelineOp::Eval { .. }
                                | PipelineOp::PutPixel { .. }
                                | PipelineOp::EffectSpread { .. }
                                | PipelineOp::CompositeModule { .. }
                                | PipelineOp::Filter3x3 { .. }
                                | PipelineOp::Filter5x5 { .. }
                                | PipelineOp::RemapPalette { .. }
                                | PipelineOp::InvertChops
                                | PipelineOp::Offset { .. }
                                | PipelineOp::Mirror
                                | PipelineOp::Transpose { .. }
                                | PipelineOp::Crop { .. }
                                | PipelineOp::CropBorder { .. }
                                | PipelineOp::Expand { .. }
                                | PipelineOp::Duplicate
                                | PipelineOp::Flip
                                | PipelineOp::Reduce { .. }
                                | PipelineOp::Paste { .. }
                                | PipelineOp::Scale { .. }
                                | PipelineOp::Contain { .. }
                                | PipelineOp::Cover { .. }
                                | PipelineOp::Pad { .. }
                                | PipelineOp::Fit {
                                    filter: ResampleFilter::Nearest,
                                    ..
                                }
                                | PipelineOp::Transform { .. }
                                | PipelineOp::Resize {
                                    ..
                                }
                                | PipelineOp::Equalize | PipelineOp::EqualizeMasked { .. }
                        )
                    }))
                || (matches!(ops, [PipelineOp::Merge { mode: ColorMode::RGB, .. }])
                    && logical_mode == "P"
                    && (!inspect_auxiliary_modes || gpu_palette_first_rgb_merge_is_supported(ops, mode)))
                || (logical_mode == "1"
                    && ops
                        .iter()
                        .all(|op| {
                            matches!(
                                op,
                                PipelineOp::PutData { .. }
                                    | PipelineOp::PutPixel { .. }
                                    | PipelineOp::EffectSpread { .. }
                                    | PipelineOp::CompositeModule { .. }
                                    | PipelineOp::Filter3x3 { .. }
                                | PipelineOp::Filter5x5 { .. }
                                    | PipelineOp::Eval { .. }
                                    | PipelineOp::LogicalAnd { .. }
                                    | PipelineOp::LogicalOr { .. }
                                    | PipelineOp::LogicalXor { .. }
                                    | PipelineOp::Offset { .. }
                                    | PipelineOp::Transpose { .. }
                                    | PipelineOp::Paste { .. }
                                    | PipelineOp::Duplicate
                                    | PipelineOp::Contain {
                                        filter: crate::pipeline::ResampleFilter::Nearest,
                                        ..
                                    }
                                    | PipelineOp::Cover {
                                        filter: crate::pipeline::ResampleFilter::Nearest,
                                        ..
                                    }
                                    | PipelineOp::Pad {
                                        filter: crate::pipeline::ResampleFilter::Nearest,
                                        ..
                                    }
                                    | PipelineOp::Transform { .. }
                                    | PipelineOp::Resize {
                                        filter: crate::pipeline::ResampleFilter::Nearest,
                                        ..
                                    }
                            )
                        }))
                // RGBX shares RGBA's four-byte storage, but its fourth byte
                // is padding rather than alpha. Transpose is a pure native
                // byte relocation, so the RGBA transport can preserve all
                // four bytes without applying alpha semantics. PutPixel is
                // likewise a raw four-byte write in this logical mode.
                // Sharpness has a dedicated mode-6 shader branch that filters
                // all four stored bytes, matching Pillow's RGBX behavior.
                || (logical_mode == "RGBX"
                    && ops.iter().all(|op| {
                        matches!(
                            op,
                            PipelineOp::Sharpness { .. }
                                | PipelineOp::PutPixel { .. }
                                | PipelineOp::EffectSpread { .. }
                                | PipelineOp::Transpose { .. }
                                | PipelineOp::Paste { mask: None, .. }
                                | PipelineOp::Scale { .. }
                                | PipelineOp::Contain { .. }
                                | PipelineOp::Cover { .. }
                                | PipelineOp::Pad { .. }
                                | PipelineOp::Transform { .. }
                                | PipelineOp::Resize {
                                    ..
                                }
                                | PipelineOp::Convert {
                                    mode: ColorMode::L
                                        | ColorMode::LA
                                        | ColorMode::RGB
                                        | ColorMode::RGBA
                                        | ColorMode::CMYK
                                        | ColorMode::YCbCr
                                        | ColorMode::HSV
                                        | ColorMode::I
                                        | ColorMode::F,
                                    matrix: None,
                                    dither: None,
                                }
                        )
                    }))
                // RGBa uses the same four-byte storage as RGBA. Pillow's
                // Image.resize leaves RGBa in its already-premultiplied
                // representation (PIL/Image.py resize mode dispatch), so
                // boxed Fit can use the same coefficient kernels with
                // `premultiply = 0`; ImageChops likewise operates on these
                // stored bytes directly. Keep this whitelist limited to
                // raw-channel operations and geometry that preserves that
                // four-byte sample contract.
                || (logical_mode == "RGBa"
                    && ops.iter().all(|op| {
                        matches!(
                            op,
                            PipelineOp::PutPixel { .. }
                                | PipelineOp::EffectSpread { .. }
                                | PipelineOp::Add { .. }
                                | PipelineOp::Subtract { .. }
                                | PipelineOp::Multiply { .. }
                                | PipelineOp::Screen { .. }
                                | PipelineOp::Darker { .. }
                                | PipelineOp::Lighter { .. }
                                | PipelineOp::Difference { .. }
                                | PipelineOp::Overlay { .. }
                                | PipelineOp::HardLight { .. }
                                | PipelineOp::SoftLight { .. }
                                | PipelineOp::AddModulo { .. }
                                | PipelineOp::SubtractModulo { .. }
                                | PipelineOp::InvertChops
                                | PipelineOp::Paste { mask: None, .. }
                                | PipelineOp::Scale { .. }
                                | PipelineOp::Contain { .. }
                                | PipelineOp::Cover { .. }
                                | PipelineOp::Pad { .. }
                                | PipelineOp::Transform { .. }
                                | PipelineOp::Fit { .. }
                                | PipelineOp::Resize {
                                    ..
                                }
                        )
                    }))
                || (matches!(logical_mode, "HSV" | "YCbCr")
                    && ops.iter().all(|op| {
                        matches!(
                            op,
                            PipelineOp::Add { .. }
                                | PipelineOp::Subtract { .. }
                                | PipelineOp::Multiply { .. }
                                | PipelineOp::Screen { .. }
                                | PipelineOp::Darker { .. }
                                | PipelineOp::Lighter { .. }
                                | PipelineOp::Difference { .. }
                                | PipelineOp::Overlay { .. }
                                | PipelineOp::HardLight { .. }
                                | PipelineOp::SoftLight { .. }
                                | PipelineOp::AddModulo { .. }
                                | PipelineOp::SubtractModulo { .. }
                                | PipelineOp::Brightness { .. }
                                | PipelineOp::Filter3x3 { .. }
                                | PipelineOp::Filter5x5 { .. }
                                    | PipelineOp::Reduce { .. }
                                    | PipelineOp::PutData { .. }
                                    | PipelineOp::Eval { .. }
                                | PipelineOp::PutPixel { .. }
                                | PipelineOp::EffectSpread { .. }
                                | PipelineOp::Paste { .. }
                                | PipelineOp::Crop { .. }
                                | PipelineOp::Scale { .. }
                                | PipelineOp::Contain { .. }
                                | PipelineOp::Cover { .. }
                                | PipelineOp::Pad { .. }
                                | PipelineOp::Transform { .. }
                                | PipelineOp::Resize {
                                    ..
                                }
                        )
                    }))
                // Singleton HSV Expand reads the image's three stored
                // samples as-is. Keep this tied to the native-layout gate so
                // the shader cannot admit a logical HSV image with a
                // mismatched physical representation.
                || (logical_mode == "HSV"
                    && matches!(ops, [PipelineOp::Expand { .. }])
                    && img.is_none_or(|img| gpu_native_expand_channels(ops, img, mode).is_some()))
                // CMYK, HSV, and YCbCr retain their native channel order in
                // the packed RGBA/RGB transport.  ExtractBand only copies
                // one requested byte and then publishes an L8 result, so it
                // does not reinterpret the samples as RGB or alpha.  Keep
                // PutPixel here as well: the maintained getchannel batches
                // often write one source pixel before extracting its band.
                || (matches!(logical_mode, "CMYK" | "HSV" | "YCbCr")
                    && ops.iter().all(|op| {
                        matches!(
                            op,
                            PipelineOp::ExtractBand { .. }
                                | PipelineOp::PutPixel { .. }
                                | PipelineOp::EffectSpread { .. }
                        )
                    }))
                // CMYK putalpha is a terminal promotion through the exact
                // integer CMYK->RGB conversion in put_alpha.wgsl.  Keep this
                // whitelist terminal-only: after promotion the public mode
                // is RGBA, and a following operation needs a segmented batch
                // with the updated logical layout rather than CMYK metadata.
                || (logical_mode == "CMYK"
                    && ops.len() == 1
                    && matches!(
                        ops[0],
                        PipelineOp::PutAlpha {
                            mode: PixelMode::CMYK,
                            ..
                        } | PipelineOp::PutAlphaData {
                            mode: PixelMode::CMYK,
                            ..
                        }
                    ))
                // Keep CMYK→RGB as one terminal native conversion. The
                // convert shader interprets the four bytes as C/M/Y/K and
                // applies Pillow's exact integer inverse before RGB readback.
                || (logical_mode == "CMYK"
                    && matches!(ops, [PipelineOp::Convert {
                        mode: ColorMode::RGB,
                        matrix: None,
                        dither: None,
                    }])
                    && img.is_none_or(|img| matches!(img, DynamicImage::ImageRgba8(_))))
                || (matches!(logical_mode, "RGB" | "RGBA" | "CMYK")
                    && ops
                        .iter()
                        .all(|op| matches!(op, PipelineOp::Color3DLut { .. })))
                // F stores one finite f32 sample in each four-byte word.
                // Its order-statistic shaders compare the decoded samples;
                // the ordinary byte filters would sort IEEE-754 bytes and
                // produce a numerically unrelated result. Mirror remains in
                // this clause because it only relocates complete words.
                || (logical_mode == "F" && img.is_none_or(|img| gpu_float_filter_is_supported(ops, img)))
                // A constant F Pad uses the exact scalar resize marker for
                // its contain step and a raw-word placement shader for the
                // final canvas. The source proof is deliberately limited to
                // a single non-nearest Pad; mixed batches still need the
                // host semantic path until their intermediate contract is
                // proven.
                || (logical_mode == "F"
                    && f_resize_constant_bits.is_some()
                    && ops.len() == 1
                    && matches!(ops[0], PipelineOp::Pad { .. }))
                // A heterogeneous F Pad uses marker 9 for its contain
                // resize and then copies complete words through placement.
                // The admission proof is limited to one changed-axis Pad
                // with an optional PutData(F)-only prefix; nearest, same-size,
                // and unrelated prefixes retain their existing paths.
                || (logical_mode == "F"
                    && f_pad_f64_is_exact)
                // A nearest F Fit is a two-axis one-tap word relocation. Its
                // boxed coefficients are generated on the host after the
                // same f32 crop-boundary conversion as Pillow's affine
                // nearest path; keep it separate from the vertical pass so
                // Metal cannot observe a stale horizontal intermediate.
                || (logical_mode == "F"
                    && ops.len() == 1
                    && matches!(
                        ops[0],
                        PipelineOp::Fit {
                            filter: ResampleFilter::Nearest,
                            ..
                        }
                    ))
                // I-mode nearest Pad carries signed int32 words through a
                // nearest contain resize and a raw-word placement pass. A
                // filtered Pad would need the typed INT32 accumulator and is
                // intentionally kept on exact host semantic control.
                || (logical_mode == "I"
                    && ops.iter().all(|op| {
                        matches!(
                            op,
                            PipelineOp::Pad {
                                filter: ResampleFilter::Nearest,
                                ..
                            }
                                | PipelineOp::Resize {
                                    filter: ResampleFilter::Nearest,
                                    ..
                                }
                        )
                    }))
                || (logical_mode == "I"
                    && img.is_none_or(|img| gpu_int_filter_is_supported(ops, img)
                        || gpu_int_filter_resize_chain_is_supported(ops, img)))
                // I/F samples are four raw bytes per pixel at this executor
                // boundary.  These operations only relocate or duplicate
                // the complete sample and therefore do not need to decode it
                // as an integer, float, or color.  Keep arithmetic and fill
                // operations out of this clause: their shader contracts need
                // a native typed buffer rather than packed RGBA semantics.
                || (matches!(logical_mode, "I" | "F")
                    && ops.iter().all(|op| {
                        matches!(
                            op,
                            PipelineOp::PutData { .. }
                                | PipelineOp::Offset { .. }
                                | PipelineOp::EffectSpread { .. }
                                | PipelineOp::Flip
                                | PipelineOp::Mirror
                                | PipelineOp::Transpose { .. }
                                | PipelineOp::Crop { .. }
                                | PipelineOp::CropBorder { .. }
                                | PipelineOp::Duplicate
                                | PipelineOp::Paste { mask: None, .. }
                                | PipelineOp::Scale {
                                    ..
                                }
                                | PipelineOp::Contain {
                                    ..
                                }
                                | PipelineOp::Cover {
                                    ..
                                }
                                | PipelineOp::Resize {
                                    ..
                                }
                                | PipelineOp::Transform { .. }
                        )
                    }))
                // Native I;16* images use one typed u16 sample per pixel.
                // The packed GPU word is only an opaque transport for these
                // relocation operations; it is never narrowed to an 8-bit
                // luma value or interpreted as RGBA.
                || (matches!(logical_mode, "I;16" | "I;16L" | "I;16B" | "I;16N")
                    && img.is_none_or(|img| gpu_luma16_geometry_is_supported(ops, img, Some(logical_mode))
                        || gpu_luma16_convert_is_supported(ops, img)
                        || gpu_luma16_paste_is_supported(ops, img)))
                || (logical_mode == "CMYK"
                    && ops
                        .iter()
                        .all(|op| {
                            matches!(
                                op,
                                PipelineOp::Grayscale
                                    | PipelineOp::Brightness { .. }
                                    | PipelineOp::Contrast { .. }
                                    | PipelineOp::ColorSaturation { .. }
                                    | PipelineOp::Sharpness { .. }
                                    | PipelineOp::InvertChops
                                    | PipelineOp::Add { .. }
                                    | PipelineOp::Subtract { .. }
                                    | PipelineOp::Multiply { .. }
                                    | PipelineOp::Screen { .. }
                                    | PipelineOp::Darker { .. }
                                    | PipelineOp::Lighter { .. }
                                    | PipelineOp::Difference { .. }
                                    | PipelineOp::Overlay { .. }
                                    | PipelineOp::HardLight { .. }
                                    | PipelineOp::SoftLight { .. }
                                    | PipelineOp::AddModulo { .. }
                                    | PipelineOp::SubtractModulo { .. }
                                    | PipelineOp::LogicalAnd { .. }
                                    | PipelineOp::LogicalOr { .. }
                                    | PipelineOp::LogicalXor { .. }
                                    | PipelineOp::Filter3x3 { .. }
                                    | PipelineOp::Filter5x5 { .. }
                                    | PipelineOp::BlendModule { .. }
                                    | PipelineOp::Offset { .. }
                                    | PipelineOp::Mirror
                                    | PipelineOp::Transpose { .. }
                                    | PipelineOp::Crop { .. }
                                    | PipelineOp::CropBorder { .. }
                                    | PipelineOp::Expand { .. }
                                    | PipelineOp::Duplicate
                                    | PipelineOp::Flip
                            | PipelineOp::Reduce { .. }
                                | PipelineOp::CompositeModule { .. }
                                | PipelineOp::PutData { .. }
                                | PipelineOp::Eval { .. }
                                | PipelineOp::PutPixel { .. }
                                | PipelineOp::EffectSpread { .. }
                                | PipelineOp::Paste { .. }
                                | PipelineOp::Scale { .. }
                                | PipelineOp::Contain { .. }
                            | PipelineOp::Cover { .. }
                            | PipelineOp::Pad { .. }
                            | PipelineOp::Transform { .. }
                            | PipelineOp::Resize {
                                    ..
                                }
                            )
                        }))
    })
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
