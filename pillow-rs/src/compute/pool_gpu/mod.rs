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
mod batch_pipeline;
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct NativeHsvPadDispatch {
    output: NativePackedChannelOutputDispatch,
    input_bytes: usize,
    input_transfer_bytes: u64,
    output_width: u32,
    output_height: u32,
    offset_y: u32,
    fill_pixel: u32,
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
        let mut params_arena = Vec::new();
        let mut params_ranges = Vec::with_capacity(ops.len());
        let mut img2_arena = Vec::new();
        let mut img2_ranges = Vec::with_capacity(ops.len());
        let mut img3_arena = Vec::new();
        let mut img3_ranges = Vec::with_capacity(ops.len());
        let mut lut_arena = Vec::new();
        let mut lut_ranges = Vec::with_capacity(ops.len());
        let mut second_cache: HashMap<usize, BufferRange> = HashMap::new();
        let mut third_cache: HashMap<usize, BufferRange> = HashMap::new();
        let mut lut_cache: HashMap<[u32; 256], BufferRange> = HashMap::new();
        let mut resize_coeff_ranges = Vec::with_capacity(ops.len());
        let mut input_dims = Vec::with_capacity(ops.len());
        let mut output_dims = Vec::with_capacity(ops.len());
        let mut resize_source_rows = Vec::with_capacity(ops.len());
        let mut pad_resize_dims = Vec::with_capacity(ops.len());
        let mut native_expand_output = None;
        let mut native_luma_transform_output = None;
        let native_rgb_packed_output = if native_rgb_packed_output {
            let output_plan = if matches!(
                ops,
                [PipelineOp::Eval { .. }
                    | PipelineOp::Equalize
                    | PipelineOp::MedianFilter { size: 3 }
                    | PipelineOp::MinFilter { size: 3 }
                    | PipelineOp::MaxFilter { size: 3 }
                    | PipelineOp::BoxBlur { radius: 1 }]
            ) {
                plan_native_rgb_point_output(
                    w,
                    h,
                    limits.max_compute_workgroups_per_dimension,
                    limits.max_storage_buffer_binding_size,
                    limits.max_buffer_size,
                    buffers.capacity,
                )
            } else {
                plan_native_rgb_transform_output(
                    ops,
                    limits.max_compute_workgroups_per_dimension,
                    limits.max_storage_buffer_binding_size,
                    limits.max_buffer_size,
                    buffers.capacity,
                )
            };
            Some(output_plan.ok_or_else(|| {
                PilError::InternalError(
                    "GPU packed-RGB operation lost its checked output plan".into(),
                )
            })?)
        } else {
            None
        };
        let native_la_transform_output = if native_la_transform_output {
            Some(
                plan_native_la_transform_output(
                    ops,
                    limits.max_compute_workgroups_per_dimension,
                    limits.max_storage_buffer_binding_size,
                    limits.max_buffer_size,
                    buffers.capacity,
                )
                .ok_or_else(|| {
                    PilError::InternalError(
                        "GPU packed-LA transform lost its checked output plan".into(),
                    )
                })?,
            )
        } else {
            None
        };
        let mut cur_w = w;
        let mut cur_h = h;
        let mut current_mode = mode;

        for (index, op) in ops.iter().enumerate() {
            let cached = if can_fuse_gpu_multiply_screen(ops, index) {
                self.resolve_pipeline(
                    "__internal_multiply_screen",
                    "multiply_screen.wgsl",
                    include_str!("shaders/multiply_screen.wgsl"),
                )?
            } else if can_fuse_gpu_alpha_composite_mirror(ops, index, logical_mode, (cur_w, cur_h))
            {
                self.resolve_pipeline(
                    "__internal_alpha_composite_mirror",
                    "alpha_composite_mirror.wgsl",
                    include_str!("shaders/alpha_composite_mirror.wgsl"),
                )?
            } else if matches!(
                &ops[index],
                PipelineOp::Autocontrast { .. }
                    | PipelineOp::Equalize
                    | PipelineOp::EqualizeMasked { .. }
            ) {
                // Histogram-driven operations resolve to a multi-pass plan
                // during encoding. Use the clear pass here because it has
                // the same parameter contract as the public operation, while
                // the operation-specific cutoff/CDF parameters are appended
                // below.
                self.resolve_pipeline(
                    "__internal_histogram_clear",
                    "histogram_clear.wgsl",
                    include_str!("shaders/histogram_clear.wgsl"),
                )?
            } else if packed_native_byte_filter
                && matches!(logical_mode, None | Some("L"))
                && matches!(op, PipelineOp::Filter5x5 { .. })
            {
                self.resolve_pipeline(
                    "__internal_filter_5x5_luma_packed",
                    "filter_5x5_luma_packed.wgsl",
                    include_str!("shaders/filter_5x5_luma_packed.wgsl"),
                )?
            } else if packed_native_byte_filter
                && matches!(logical_mode, Some("LA"))
                && matches!(op, PipelineOp::Filter3x3 { .. })
            {
                self.resolve_pipeline(
                    "__internal_filter_3x3_la_binomial_packed",
                    "filter_3x3_la_binomial_packed.wgsl",
                    include_str!("shaders/filter_3x3_la_binomial_packed.wgsl"),
                )?
            } else if packed_native_byte_filter
                && matches!(logical_mode, Some("LA"))
                && matches!(op, PipelineOp::Filter5x5 { .. })
            {
                self.resolve_pipeline(
                    "__internal_filter_5x5_la_packed",
                    "filter_5x5_la_packed.wgsl",
                    include_str!("shaders/filter_5x5_la_packed.wgsl"),
                )?
            } else if packed_native_byte_filter
                && matches!(logical_mode, Some("LA"))
                && matches!(op, PipelineOp::MedianFilter { size: 3 })
            {
                self.resolve_pipeline(
                    "__internal_median_filter_3x3_la_packed",
                    "median_filter_3x3_la_packed.wgsl",
                    include_str!("shaders/median_filter_3x3_la_packed.wgsl"),
                )?
            } else if packed_native_byte_filter
                && matches!(logical_mode, Some("LA"))
                && matches!(op, PipelineOp::RankFilter { size: 3, rank } if *rank < 9)
            {
                self.resolve_pipeline(
                    "__internal_rank_filter_3x3_la_packed",
                    "rank_filter_3x3_la_packed.wgsl",
                    include_str!("shaders/rank_filter_3x3_la_packed.wgsl"),
                )?
            } else if packed_native_byte_filter
                && matches!(logical_mode, Some("LA"))
                && matches!(op, PipelineOp::MaxFilter { size: 3 })
            {
                self.resolve_pipeline(
                    "__internal_max_filter_3x3_la_packed",
                    "max_filter_3x3_la_packed.wgsl",
                    include_str!("shaders/max_filter_3x3_la_packed.wgsl"),
                )?
            } else if packed_native_byte_filter
                && matches!(logical_mode, Some("LA"))
                && matches!(op, PipelineOp::MinFilter { size: 3 })
            {
                self.resolve_pipeline(
                    "__internal_min_filter_3x3_la_packed",
                    "min_filter_3x3_la_packed.wgsl",
                    include_str!("shaders/min_filter_3x3_la_packed.wgsl"),
                )?
            } else if native_rgb_packed_output.is_some()
                && matches!(op, PipelineOp::MedianFilter { size: 3 })
            {
                self.resolve_pipeline(
                    "__internal_median_filter_3x3_rgb_packed",
                    "median_filter_3x3_rgb_packed.wgsl",
                    include_str!("shaders/median_filter_3x3_rgb_packed.wgsl"),
                )?
            } else if native_rgb_packed_output.is_some()
                && matches!(op, PipelineOp::MaxFilter { size: 3 })
            {
                self.resolve_pipeline(
                    "__internal_max_filter_3x3_rgb_packed",
                    "max_filter_3x3_rgb_packed.wgsl",
                    include_str!("shaders/max_filter_3x3_rgb_packed.wgsl"),
                )?
            } else if native_rgb_packed_output.is_some()
                && matches!(op, PipelineOp::MinFilter { size: 3 })
            {
                self.resolve_pipeline(
                    "__internal_min_filter_3x3_rgb_packed",
                    "min_filter_3x3_rgb_packed.wgsl",
                    include_str!("shaders/min_filter_3x3_rgb_packed.wgsl"),
                )?
            } else if native_rgb_packed_output.is_some()
                && matches!(logical_mode, None | Some("RGB"))
                && matches!(op, PipelineOp::BoxBlur { radius: 1 })
            {
                self.resolve_pipeline(
                    "__internal_blur_h_rgb_packed",
                    "box_blur_h_rgb_packed.wgsl",
                    include_str!("shaders/box_blur_h_rgb_packed.wgsl"),
                )?
            } else if packed_native_byte_filter
                && matches!(
                    op,
                    PipelineOp::GaussianBlur { .. } | PipelineOp::BoxBlur { radius: 1 }
                )
            {
                if matches!(logical_mode, Some("LA")) {
                    self.resolve_pipeline(
                        "__internal_blur_h_la_packed",
                        "box_blur_h_la_packed.wgsl",
                        include_str!("shaders/box_blur_h_la_packed.wgsl"),
                    )?
                } else {
                    self.resolve_pipeline(
                        "__internal_blur_h_luma_packed",
                        "box_blur_h_luma_packed.wgsl",
                        include_str!("shaders/box_blur_h_luma_packed.wgsl"),
                    )?
                }
            } else if Self::blur_pass_count(op).is_some() {
                self.resolve_pipeline(
                    "__internal_blur_h",
                    "box_blur_h.wgsl",
                    include_str!("shaders/box_blur_h.wgsl"),
                )?
            } else if matches!(op, PipelineOp::Pad { .. }) {
                // Pad's first device pass is the exact separable resize.  The
                // final placement shader is resolved alongside it during
                // encode_batch; using the resize variant here makes the
                // coefficient range and uniform preparation follow the same
                // binding contract as public Resize.
                self.resolve_pipeline(
                    "__internal_resize_h",
                    "resize_convolution_h.wgsl",
                    include_str!("shaders/resize_convolution_h.wgsl"),
                )?
            } else if matches!(op, PipelineOp::Fit { .. }) {
                // Fit's public crop box is fractional, but its exact byte
                // implementation is the same boxed two-pass convolution as
                // Resize once the host has supplied the coefficient tables.
                self.resolve_pipeline(
                    "__internal_resize_h",
                    "resize_convolution_h.wgsl",
                    include_str!("shaders/resize_convolution_h.wgsl"),
                )?
            } else if matches!(op, PipelineOp::Resize { filter, .. }
                if !matches!(filter, ResampleFilter::Nearest)
                    || gpu_resize_nearest_uses_coefficients(logical_mode))
            {
                self.resolve_pipeline(
                    "__internal_resize_h",
                    "resize_convolution_h.wgsl",
                    include_str!("shaders/resize_convolution_h.wgsl"),
                )?
            } else if native_rgb_packed_output.is_some() && matches!(op, PipelineOp::Eval { .. }) {
                self.resolve_pipeline(
                    "__internal_point_rgb_packed",
                    "point_rgb_packed.wgsl",
                    include_str!("shaders/point_rgb_packed.wgsl"),
                )?
            } else if native_rgb_packed_output.is_some()
                && matches!(op, PipelineOp::Transform { .. })
            {
                self.resolve_pipeline(
                    "__internal_transform_rgb_nearest_packed",
                    "transform_rgb_nearest_packed.wgsl",
                    include_str!("shaders/transform_rgb_nearest_packed.wgsl"),
                )?
            } else if native_la_transform_output.is_some()
                && matches!(op, PipelineOp::Transform { .. })
            {
                self.resolve_pipeline(
                    "__internal_transform_la_nearest_packed",
                    "transform_la_nearest_packed.wgsl",
                    include_str!("shaders/transform_la_nearest_packed.wgsl"),
                )?
            } else if packed_luma_transform_output && matches!(op, PipelineOp::Transform { .. }) {
                self.resolve_pipeline(
                    "__internal_transform_l_nearest_packed",
                    "transform_l_nearest_packed.wgsl",
                    include_str!("shaders/transform_l_nearest_packed.wgsl"),
                )?
            } else if gpu_transform_uses_geometry_table(op, logical_mode, (cur_w, cur_h)) {
                self.resolve_pipeline(
                    "__internal_transform_geometry",
                    "transform_geometry.wgsl",
                    include_str!("shaders/transform_geometry.wgsl"),
                )?
            } else if packed_luma_point && matches!(op, PipelineOp::Eval { .. }) {
                self.resolve_pipeline(
                    "__internal_point_luma_packed",
                    "point_luma_packed.wgsl",
                    include_str!("shaders/point_luma_packed.wgsl"),
                )?
            } else if packed_luma_order_statistic
                && matches!(op, PipelineOp::MedianFilter { size: 3 })
            {
                self.resolve_pipeline(
                    "__internal_median_filter_3x3_luma_packed",
                    "median_filter_3x3_luma_packed.wgsl",
                    include_str!("shaders/median_filter_3x3_luma_packed.wgsl"),
                )?
            } else if packed_luma_order_statistic && matches!(op, PipelineOp::MaxFilter { size: 3 })
            {
                self.resolve_pipeline(
                    "__internal_max_filter_3x3_luma_packed",
                    "max_filter_3x3_luma_packed.wgsl",
                    include_str!("shaders/max_filter_3x3_luma_packed.wgsl"),
                )?
            } else if packed_luma_order_statistic && matches!(op, PipelineOp::MinFilter { size: 3 })
            {
                self.resolve_pipeline(
                    "__internal_min_filter_3x3_luma_packed",
                    "min_filter_3x3_luma_packed.wgsl",
                    include_str!("shaders/min_filter_3x3_luma_packed.wgsl"),
                )?
            } else if packed_luma_order_statistic
                && matches!(op, PipelineOp::RankFilter { size: 3, rank: 1 })
            {
                self.resolve_pipeline(
                    "__internal_rank_filter_3x3_luma_packed",
                    "rank_filter_3x3_luma_packed.wgsl",
                    include_str!("shaders/rank_filter_3x3_luma_packed.wgsl"),
                )?
            } else if packed_luma_order_statistic
                && matches!(op, PipelineOp::RankFilter { size: 9, .. })
                && cur_w % 4 == 0
            {
                self.resolve_pipeline(
                    "__internal_rank_filter_9x9_luma_row_packed",
                    "rank_filter_9x9_luma_row_packed.wgsl",
                    include_str!("shaders/rank_filter_9x9_luma_row_packed.wgsl"),
                )?
            } else if packed_luma_order_statistic
                && matches!(op, PipelineOp::RankFilter { size: 9, .. })
            {
                self.resolve_pipeline(
                    "__internal_rank_filter_9x9_luma_packed",
                    "rank_filter_9x9_luma_packed.wgsl",
                    include_str!("shaders/rank_filter_9x9_luma_packed.wgsl"),
                )?
            } else if native_sharpness_l_input && matches!(op, PipelineOp::Sharpness { .. }) {
                self.resolve_pipeline(
                    "__internal_sharpness_l_native",
                    "sharpness_l_native.wgsl",
                    include_str!("shaders/sharpness_l_native.wgsl"),
                )?
            } else if native_sharpness_la_input && matches!(op, PipelineOp::Sharpness { .. }) {
                self.resolve_pipeline(
                    "__internal_sharpness_la_native",
                    "sharpness_la_native.wgsl",
                    include_str!("shaders/sharpness_la_native.wgsl"),
                )?
            } else if native_sharpness_rgb_input && matches!(op, PipelineOp::Sharpness { .. }) {
                self.resolve_pipeline(
                    "__internal_sharpness_rgb_native",
                    "sharpness_rgb_native.wgsl",
                    include_str!("shaders/sharpness_rgb_native.wgsl"),
                )?
            } else {
                let base_key = registry::variant_key(op);
                let entry = registry::registry()?.get(base_key).ok_or_else(|| {
                    PilError::ValueError(format!(
                        "GPU operation '{base_key}' has no registered shader entry"
                    ))
                })?;
                let source = entry.gpu_source.ok_or_else(|| {
                    PilError::ValueError(format!(
                        "GPU operation '{base_key}' has no registered shader source"
                    ))
                })?;
                let shader_file = entry.gpu_shader.ok_or_else(|| {
                    PilError::ValueError(format!(
                        "GPU operation '{base_key}' has no registered shader name"
                    ))
                })?;
                self.resolve_pipeline(base_key, shader_file, source)?
            };
            let (out_w, out_h) = op_output_dims(op, cur_w, cur_h).unwrap_or((cur_w, cur_h));
            self.validate_output_dims(buffers, out_w, out_h)?;
            let native_expand_output_dispatch = if let Some(channels) = native_expand_channels
                && ops.len() == 1
                && matches!(op, PipelineOp::Expand { .. })
                // CMYK already occupies the generic four-byte GPU transport.
                // Its compact word loop would add per-byte index arithmetic
                // without reducing input or output transfer size.
                && logical_mode != Some("CMYK")
            {
                let output_bytes = CheckedDims::new(out_w, out_h, channels)?.total_bytes();
                plan_native_expand_output_dispatch(
                    u64::try_from(output_bytes).map_err(|_| {
                        PilError::ValueError("GPU Expand output is too large".into())
                    })?,
                    limits.max_compute_workgroups_per_dimension,
                )
            } else {
                None
            };
            let native_luma_transform_output_dispatch = if packed_luma_transform_output
                && ops.len() == 1
                && matches!(op, PipelineOp::Transform { .. })
            {
                Some(
                    plan_native_luma_transform_output(
                        std::slice::from_ref(op),
                        limits.max_compute_workgroups_per_dimension,
                        limits.max_storage_buffer_binding_size,
                        limits.max_buffer_size,
                        buffers.capacity,
                    )
                    .ok_or_else(|| {
                        PilError::InternalError(
                            "GPU packed-L transform lost its checked output plan".into(),
                        )
                    })?,
                )
            } else {
                None
            };
            if native_expand_output_dispatch.is_some() {
                native_expand_output = native_expand_output_dispatch;
            }
            if native_luma_transform_output_dispatch.is_some() {
                native_luma_transform_output = native_luma_transform_output_dispatch;
            }
            let mut boxed_resize_coefficients = None;
            let resize_source_row_range =
                if let PipelineOp::ResizeBoxed {
                    filter, box_coords, ..
                } = op
                {
                    let (horizontal, mut vertical) = gpu_resize_boxed_coefficients(
                        (cur_w, cur_h),
                        (out_w, out_h),
                        *box_coords,
                        *filter,
                    );
                    let range = compact_resize_coeffs_to_source_span(&mut vertical, cur_h)
                        .unwrap_or((0, cur_h));
                    boxed_resize_coefficients = Some((horizontal, vertical));
                    Some(range)
                } else {
                    None
                };
            input_dims.push((cur_w, cur_h));
            output_dims.push((out_w, out_h));
            resize_source_rows.push(resize_source_row_range);

            // Transpose's swap variants and output-only generators describe
            // their dispatch dimensions in the first uniform words. Ordinary
            // kernels use the current source dimensions there and consume the
            // appended output dimensions only when their shader declares them.
            let (shader_w, shader_h) = if cached.is_output_only
                || matches!(
                    op,
                    PipelineOp::Transpose {
                        method: TransposeMethod::Rotate90
                            | TransposeMethod::Rotate270
                            | TransposeMethod::Transpose
                            | TransposeMethod::Transverse
                    }
                ) {
                (out_w, out_h)
            } else {
                (cur_w, cur_h)
            };
            // Reduce's mode word distinguishes ordinary four-byte samples
            // from RGBA/LA alpha layouts. RGBa and RGBX share the physical
            // RGBA transport, but Pillow averages all four stored channels
            // directly. PA and La likewise average raw bands; their packed
            // luma copies and alpha can use the raw-four-channel encoding.
            // Retain the source mode code for the rest of the batch.
            let op_mode = if matches!(op, PipelineOp::Reduce { .. })
                && matches!(logical_mode, Some("RGBa" | "RGBX" | "PA" | "La"))
            {
                4
            } else {
                current_mode
            };
            let source_row_base = resize_source_row_range.map_or(0, |(first, _)| first);
            let mut params = vec![shader_w, shader_h, op_mode, source_row_base];
            if let PipelineOp::Paste { mask, .. } = op {
                // Paste's fixed fourth uniform word carries mask and I;16
                // byte-order flags because some logical modes share the same
                // physical DynamicImage transport after materialization.
                if mask
                    .as_ref()
                    .is_some_and(|mask| mask.mode().ok().as_deref() == Some("RGBa"))
                {
                    params[3] |= 1;
                }
                if let Some(mask) = mask.as_ref() {
                    if mask.mode().ok().as_deref() == Some("1") {
                        params[3] |= 4;
                    }
                }
                if mask.is_some()
                    && (matches!(logical_mode, Some("I;16B"))
                        || (matches!(logical_mode, Some("I;16" | "I;16N") | None)
                            && cfg!(target_endian = "big")))
                {
                    params[3] |= 2;
                }
            }
            if native_extract_band && matches!(op, PipelineOp::ExtractBand { .. }) {
                // ExtractBand's fourth fixed uniform word is otherwise
                // unused. Mark the one-operation native-byte layout without
                // changing the generic packed-RGBA shader contract.
                params[3] = 1;
            }
            if native_grayscale_rgb_input && matches!(op, PipelineOp::Grayscale) {
                // Grayscale's fourth word selects the native triple source
                // semantics: RGB luma or direct YCbCr Y-byte extraction.
                params[3] = u32::from(logical_mode == Some("YCbCr")) + 1;
            }
            if native_sharpness_l_input && matches!(op, PipelineOp::Sharpness { .. }) {
                params[3] = u32::from(
                    plan_sharpness_l_dispatch(
                        cur_w,
                        cur_h,
                        limits.max_compute_workgroups_per_dimension,
                    )?
                    .row_aligned,
                );
            }
            if native_reduce_rgb_input && matches!(op, PipelineOp::Reduce { .. }) {
                // Reduce's fourth word selects packed native RGB byte input;
                // this is admitted only for a singleton RGB operation.
                params[3] = 1;
            }
            if native_expand_channels.is_some() && matches!(op, PipelineOp::Expand { .. }) {
                // Expand uses the same otherwise-unused fourth fixed uniform
                // word as flags for native-byte input and compact native output.
                // These are safe because admission requires a singleton
                // Expand batch with matching physical storage.
                params[3] = 1 | (u32::from(native_expand_output_dispatch.is_some()) << 1);
            }
            if native_cover_input_channels.is_some() && matches!(op, PipelineOp::Resize { .. }) {
                // Geometry lowering turns singleton Cover into this exact
                // Resize node. Its source-row word is otherwise unused for
                // ordinary Resize; reserve the sentinel for compact native
                // input byte addressing in the horizontal shader.
                params[3] = if native_cover_packed_output.is_some() {
                    NATIVE_COVER_PACKED_OUTPUT_MARKER
                } else {
                    u32::MAX
                };
            }
            if packed_luma_putdata && matches!(op, PipelineOp::PutData { .. }) {
                let pixel_count = CheckedDims::new(cur_w, cur_h, 1)?.total_pixels();
                params[0] = u32::try_from(pixel_count.div_ceil(4)).map_err(|_| {
                    PilError::ValueError(
                        "GPU PutData dimensions exceed packed shader indexing limits".into(),
                    )
                })?;
                params[1] = 1;
                params[2] = 0;
                params[3] = u32::try_from(pixel_count)
                    .map_err(|_| PilError::ValueError("GPU PutData image is too large".into()))?;
            } else if packed_luma_point && matches!(op, PipelineOp::Eval { .. }) {
                let pixel_count = u64::from(cur_w)
                    .checked_mul(u64::from(cur_h))
                    .ok_or_else(|| PilError::ValueError("GPU Point image is too large".into()))?;
                params[0] = u32::try_from(pixel_count.div_ceil(4)).map_err(|_| {
                    PilError::ValueError(
                        "GPU Point dimensions exceed shader indexing limits".into(),
                    )
                })?;
                params[1] = 1;
                params[2] = 0;
                params[3] = 0;
            } else if native_rgb_packed_output.is_some()
                && matches!(op, PipelineOp::Eval { .. } | PipelineOp::Equalize)
            {
                let plan = native_rgb_packed_output.ok_or_else(|| {
                    PilError::InternalError(
                        "packed-RGB point parameters have no checked output plan".into(),
                    )
                })?;
                params[0] = cur_w;
                params[1] = cur_h;
                if !matches!(op, PipelineOp::Equalize) {
                    params[2] = 0;
                }
                params[3] = if plan.row_tiled { 16 } else { 0 };
            } else if native_rgb_packed_output.is_some()
                && matches!(op, PipelineOp::MedianFilter { size: 3 })
            {
                let plan = native_rgb_packed_output.ok_or_else(|| {
                    PilError::InternalError(
                        "packed-RGB median parameters have no checked output plan".into(),
                    )
                })?;
                params[0] = cur_w;
                params[1] = cur_h;
                params[3] = if plan.row_tiled { 16 } else { 0 };
            } else if native_rgb_packed_output.is_some()
                && matches!(op, PipelineOp::MaxFilter { size: 3 })
            {
                let plan = native_rgb_packed_output.ok_or_else(|| {
                    PilError::InternalError(
                        "packed-RGB MaxFilter parameters have no checked output plan".into(),
                    )
                })?;
                params[0] = cur_w;
                params[1] = cur_h;
                params[3] = if plan.row_tiled { 16 } else { 0 };
            } else if native_rgb_packed_output.is_some()
                && matches!(op, PipelineOp::MinFilter { size: 3 })
            {
                let plan = native_rgb_packed_output.ok_or_else(|| {
                    PilError::InternalError(
                        "packed-RGB MinFilter parameters have no checked output plan".into(),
                    )
                })?;
                params[0] = cur_w;
                params[1] = cur_h;
                params[3] = if plan.row_tiled { 16 } else { 0 };
            } else if native_rgb_packed_output.is_some()
                && matches!(op, PipelineOp::BoxBlur { radius: 1 })
            {
                let plan = native_rgb_packed_output.ok_or_else(|| {
                    PilError::InternalError(
                        "packed-RGB BoxBlur parameters have no checked output plan".into(),
                    )
                })?;
                params[0] = cur_w;
                params[1] = cur_h;
                params[3] = if plan.row_tiled { 16 } else { 0 };
            } else if packed_luma_colorize && matches!(op, PipelineOp::Colorize { .. }) {
                // Colorize has no row-offset input; reuse that word to select
                // the packed four-luma-samples-per-u32 upload layout.
                params[3] = 1;
            } else if packed_luma_convert && matches!(op, PipelineOp::Convert { .. }) {
                // Convert reads four native L samples per input storage word.
                params[3] = 1;
            } else if native_rgb_compact_input && matches!(op, PipelineOp::Convert { .. }) {
                // Convert reads native RGB triples from the compact byte upload.
                params[3] = 2;
            }
            if matches!(
                op,
                PipelineOp::Resize {
                    filter: ResampleFilter::Nearest,
                    ..
                } | PipelineOp::ResizeBoxed {
                    filter: ResampleFilter::Nearest,
                    ..
                }
            ) {
                // F-mode nearest samples are opaque f32 words. Use the
                // coefficient path's host-generated one-tap table so its
                // source row/column follows Pillow's cumulative f64 affine
                // walk; the marker selects an exact word copy in WGSL and
                // preserves NaN, infinity, and signed zero.
                let nearest_mode = if logical_mode == Some("F") {
                    7
                } else {
                    u32::from(gpu_resize_should_premultiply(
                        op_mode,
                        logical_mode,
                        ResampleFilter::Nearest,
                    ))
                };
                params.extend([
                    out_w,
                    out_h,
                    gpu_resize_channel_count(op_mode),
                    nearest_mode,
                ]);
            } else if let Some(filter) = match op {
                PipelineOp::Resize { filter, .. } | PipelineOp::ResizeBoxed { filter, .. } => {
                    Some(filter)
                }
                _ => None,
            } {
                debug_assert!(!matches!(filter, ResampleFilter::Nearest));
                // F-mode constant images are lowered by the mode-8 shader as
                // an exact bit-pattern fill.  `channels` is unused by that
                // branch and `premultiply` is false for F, so carry the
                // scalar without changing the shared uniform ABI.
                let constant_bits = f_resize_constant_bits.filter(|_| {
                    logical_mode == Some("F") && !matches!(filter, ResampleFilter::Nearest)
                });
                let box_copy_is_exact = f_resize_box_copy_is_exact
                    && logical_mode == Some("F")
                    && matches!(filter, ResampleFilter::Box);
                let identity_is_exact = f_resize_identity_is_exact && logical_mode == Some("F");
                let box_average_is_exact = f_resize_box_average_is_exact
                    && logical_mode == Some("F")
                    && matches!(filter, ResampleFilter::Box);
                let dyadic_is_exact = f_resize_dyadic_is_exact
                    && logical_mode == Some("F")
                    && matches!(
                        filter,
                        ResampleFilter::Bilinear
                            | ResampleFilter::Bicubic
                            | ResampleFilter::Lanczos
                            | ResampleFilter::Hamming
                            | ResampleFilter::Box
                    );
                let f64_is_exact = f_resize_f64_is_exact
                    && logical_mode == Some("F")
                    && !matches!(filter, ResampleFilter::Nearest);
                let f64_ordered_is_exact = f_resize_f64_ordered_is_exact
                    && logical_mode == Some("F")
                    && !matches!(filter, ResampleFilter::Nearest);
                let compact_box_is_exact = f64_ordered_is_exact
                    && matches!(filter, ResampleFilter::Box)
                    && gpu_f_resize_compact_box_any_axis((cur_w, cur_h), (out_w, out_h));
                let i_f64_is_exact = f_resize_f64_is_exact
                    && logical_mode == Some("I")
                    && !matches!(filter, ResampleFilter::Nearest);
                let luma16_f64_is_exact = f_resize_f64_is_exact
                    && op_mode == 5
                    && logical_mode
                        .is_none_or(|mode| matches!(mode, "I;16" | "I;16L" | "I;16B" | "I;16N"))
                    && !matches!(filter, ResampleFilter::Nearest);
                let coefficient_channels =
                    constant_bits.unwrap_or_else(|| gpu_resize_channel_count(op_mode));
                params.extend([
                    out_w,
                    out_h,
                    coefficient_channels,
                    if constant_bits.is_some() {
                        2
                    } else if box_copy_is_exact {
                        3
                    } else if identity_is_exact {
                        5
                    } else if box_average_is_exact {
                        4
                    } else if compact_box_is_exact {
                        13
                    } else if f64_ordered_is_exact {
                        12
                    } else if f64_is_exact {
                        9
                    } else if i_f64_is_exact {
                        11
                    } else if luma16_f64_is_exact {
                        10
                    } else if dyadic_is_exact {
                        6
                    } else {
                        u32::from(gpu_resize_should_premultiply(
                            op_mode,
                            logical_mode,
                            *filter,
                        ))
                    },
                ]);
            } else if let PipelineOp::Pad { filter, .. } = op {
                let ((resize_w, resize_h), (offset_x, offset_y)) =
                    gpu_pad_geometry(op, cur_w, cur_h).ok_or_else(|| {
                        PilError::ValueError("GPU Pad has no safe geometry".into())
                    })?;
                let resize_filter = if logical_mode == Some("P") {
                    ResampleFilter::Nearest
                } else {
                    *filter
                };
                let fill = gpu_pad_fill(op, logical_mode, op_mode);
                let constant_bits = f_resize_constant_bits.filter(|_| {
                    logical_mode == Some("F") && !matches!(resize_filter, ResampleFilter::Nearest)
                });
                let f64_is_exact = f_resize_f64_is_exact
                    && logical_mode == Some("F")
                    && !matches!(resize_filter, ResampleFilter::Nearest);
                params.extend([
                    resize_w,
                    resize_h,
                    constant_bits.unwrap_or_else(|| gpu_resize_channel_count(op_mode)),
                    if constant_bits.is_some() {
                        2
                    } else if f64_is_exact {
                        9
                    } else {
                        u32::from(gpu_resize_should_premultiply(
                            op_mode,
                            logical_mode,
                            resize_filter,
                        ))
                    },
                    out_w,
                    out_h,
                    fill,
                    offset_x,
                    offset_y,
                ]);
                if packed_luma_pad {
                    // Reuse the exact row-packed L resize variants for both
                    // axes; `_pad` selects native byte addressing and packed
                    // output without changing Pad's public geometry fields.
                    params[3] = NATIVE_COVER_PACKED_OUTPUT_MARKER;
                }
            } else if matches!(op, PipelineOp::EqualizeMasked { .. }) {
                params[3] = u32::from(auxiliary_images[index].third.is_some());
            } else if let PipelineOp::Autocontrast { cutoff, .. } = op {
                let selected_pixels = gpu_autocontrast_selected_pixels(
                    cur_w,
                    cur_h,
                    auxiliary_images[index].third.as_deref(),
                )?;
                let (cutoff_low, cutoff_high) =
                    gpu_autocontrast_cutoff_indices(selected_pixels, *cutoff);
                params[3] = u32::from(auxiliary_images[index].third.is_some());
                // Keep the original cutoff bits for receipt/debugging
                // compatibility; the exact integer thresholds below avoid
                // f32 cutoff rounding inside the device control pass.
                params.extend([(*cutoff as f32).to_bits(), cutoff_low, cutoff_high]);
                params.push(selected_pixels.min(u32::MAX as usize) as u32);
            } else if let PipelineOp::Transform {
                filter,
                method,
                data,
                ..
            } = op
            {
                let mut transform_params = registry::extract_params(op);
                // SPECIAL filtered samples read/write one logical byte. The
                // packed transport has its own byte order. Carry the first
                // logical byte's position in the otherwise unused header word.
                let logical_big_endian = logical_mode == Some("I;16B")
                    || (cfg!(target_endian = "big")
                        && matches!(logical_mode, Some("I;16" | "I;16N")));
                let transport_big_endian =
                    matches!(logical_mode, Some("I;16B" | "I;16N")) || cfg!(target_endian = "big");
                params[3] = if native_rgb_compact_input {
                    // The singleton RGB transform reads native three-byte
                    // source triples. Bit 1 selects that layout; bit 0 stays
                    // reserved for the I;16 byte-order contract above.
                    2
                } else {
                    u32::from(logical_big_endian != transport_big_endian)
                };
                if packed_luma_transform_input {
                    // Singleton L transforms unpack four native samples per
                    // storage word. Bit 2 selects that source layout without
                    // changing Transform's packed RGBA output contract.
                    params[3] |= 4;
                }
                if packed_luma_transform_output {
                    // The dedicated affine-nearest L shader packs four
                    // logical output samples into each storage word.
                    params[3] |= 8;
                }
                if native_rgb_packed_output.is_some_and(|plan| plan.row_tiled) {
                    // The compact RGB shader maps workgroup rows directly to
                    // image rows, avoiding a flat group-to-row division.
                    params[3] |= 16;
                }
                if native_la_transform_output.is_some_and(|plan| plan.row_tiled) {
                    // The compact LA shader uses a row-local 4-pixel group
                    // when the checked dispatch can tile complete rows.
                    params[3] |= 32;
                }
                // Pillow's affine nearest kernel quantizes its coefficients
                // to signed 16.16 once, then advances integer coordinates
                // across each output row.  Carry those fixed-point values
                // in the six affine slots for the native GPU path instead
                // of reconstructing them from f32 values in the shader.  The
                // slots are still consumed as f32 for bilinear/projective
                // transforms, so this is an ABI-preserving mode-specific
                // encoding.
                if matches!(method, TransformMethod::Affine)
                    && gpu_transform_uses_nearest(logical_mode, *filter)
                {
                    let fixed = |value: f64| (value.mul_add(65_536.0, 0.5).floor() as i64) as u32;
                    let a = data.first().copied().unwrap_or(0.0);
                    let b = data.get(1).copied().unwrap_or(0.0);
                    let c = data.get(2).copied().unwrap_or(0.0);
                    let d = data.get(3).copied().unwrap_or(0.0);
                    let e = data.get(4).copied().unwrap_or(0.0);
                    let f = data.get(5).copied().unwrap_or(0.0);
                    let (origin_x, origin_y) = if logical_mode
                        .is_some_and(|mode| matches!(mode, "I;16" | "I;16L" | "I;16B" | "I;16N"))
                    {
                        // Geometry.c's native I;16 affine path evaluates the
                        // source coordinate at the integer destination pixel
                        // and then applies floor(value + 0.5).  Unlike the
                        // byte/I/F affine path it does not add half of each
                        // affine step before the fixed-point walk.
                        (fixed(c + 0.5), fixed(f + 0.5))
                    } else {
                        (fixed(c + a * 0.5 + b * 0.5), fixed(f + d * 0.5 + e * 0.5))
                    };
                    transform_params[2..8].copy_from_slice(&[
                        fixed(a),
                        fixed(b),
                        origin_x,
                        fixed(d),
                        fixed(e),
                        origin_y,
                    ]);
                }
                // `extract_params` deliberately contains only the public
                // operation values.  The GPU transport additionally needs
                // the scalar decision that mirrors Image.transform's
                // premultiplied-alpha round trip; keep that decision in the
                // control plane and let the shader own every pixel sample.
                transform_params[8] = gpu_transform_fill(op, logical_mode, op_mode);
                if gpu_transform_uses_nearest(logical_mode, *filter)
                    && !(logical_mode == Some("F") && gpu_transform_is_projective(op))
                {
                    transform_params[9] = 0;
                }
                params.extend(transform_params);
                params.push(u32::from(gpu_transform_should_premultiply(
                    op_mode,
                    logical_mode,
                    *filter,
                    method.clone(),
                )));
                // The affine shader historically used this final word as
                // padding. Reuse it as the method selector and append the
                // two projective coefficients/mesh tail values below so one
                // packed uniform contract covers every bounded transform
                // family without a second dispatch or auxiliary image.
                let method_code = match method {
                    TransformMethod::Affine => 0,
                    TransformMethod::Perspective => 1,
                    TransformMethod::Quad => 2,
                    TransformMethod::Mesh => 3,
                };
                params.push(method_code);
                params.push((data.get(6).copied().unwrap_or(0.0) as f32).to_bits());
                params.push((data.get(7).copied().unwrap_or(0.0) as f32).to_bits());
                if matches!(method, TransformMethod::Mesh) {
                    for value in data.get(8..12).unwrap_or(&[]) {
                        params.push((*value as f32).to_bits());
                    }
                }
            } else if let PipelineOp::Color3DLut {
                size,
                channels,
                source_mode,
                ..
            } = op
            {
                let scale = |extent: u32| {
                    ((f64::from(extent.saturating_sub(1)) / 255.0) * f64::from(1u32 << 18)) as u32
                };
                params.extend([
                    size.0,
                    size.1,
                    size.2,
                    *channels,
                    scale(size.0),
                    scale(size.1),
                    scale(size.2),
                ]);
                params[2] = gpu_pixel_mode_code(*source_mode).unwrap_or(op_mode);
            } else if let PipelineOp::Fit { filter, .. } = op {
                // Fit's fractional crop is represented exactly by its boxed
                // coefficient ranges.  The convolution shaders therefore
                // consume the same control words as Resize; no source pixels
                // are materialized on the host and no f32 crop mapping is
                // performed by a shader.
                let nearest_mode =
                    if logical_mode == Some("F") && matches!(filter, ResampleFilter::Nearest) {
                        // F-mode nearest Fit uses the host-generated one-tap
                        // table for the boxed affine mapping. Copy complete
                        // words so NaN, infinity, and signed zero survive.
                        7
                    } else {
                        u32::from(gpu_resize_should_premultiply(
                            op_mode,
                            logical_mode,
                            *filter,
                        ))
                    };
                params.extend([
                    out_w,
                    out_h,
                    gpu_resize_channel_count(op_mode),
                    nearest_mode,
                ]);
            } else if matches!(op, PipelineOp::Colorize { .. }) {
                // Colorize's exact 256-entry mapping is uploaded through the
                // dedicated LUT binding below. The shader needs only dimensions
                // and source mode, so avoid copying its six public parameters
                // into every aligned uniform record.
            } else {
                params.extend(registry::extract_params(op));
            }
            if matches!(op, PipelineOp::Contrast { .. }) {
                params.push(u32::from(contrast_mean.ok_or_else(|| {
                    PilError::ValueError("GPU Contrast is missing its scalar midpoint".into())
                })?));
            }
            // Shaders that declare dst_w/dst_h at the end of Params read these
            // words; shaders without them ignore the trailing words.
            if let Some(dispatch) = native_expand_output_dispatch {
                params.extend([dispatch.words_per_row, dispatch.word_count]);
            } else {
                params.extend([out_w, out_h]);
            }
            params_ranges.push(append_arena_slice(
                &mut params_arena,
                &params,
                uniform_alignment,
            ));

            let resize_coeff_range = match op {
                PipelineOp::Resize { filter, .. } => {
                    let uses_coefficients = !matches!(filter, ResampleFilter::Nearest)
                        || gpu_resize_nearest_uses_coefficients(logical_mode);
                    if !uses_coefficients {
                        None
                    } else {
                        let horizontal = gpu_resize_coefficients(out_w, cur_w, *filter);
                        let vertical = gpu_resize_coefficients(out_h, cur_h, *filter);
                        if (f_resize_f64_is_exact || f_resize_f64_ordered_is_exact)
                            && !matches!(filter, ResampleFilter::Nearest)
                            && (logical_mode == Some("F")
                                || (op_mode == 5
                                    && logical_mode.is_none_or(|mode| {
                                        matches!(mode, "I;16" | "I;16L" | "I;16B" | "I;16N")
                                    }))
                                || (op_mode == 7 && logical_mode == Some("I")))
                        {
                            let (kernel, support) = filter_from_resample(*filter);
                            let compact_box = f_resize_f64_ordered_is_exact
                                && logical_mode == Some("F")
                                && matches!(filter, ResampleFilter::Box)
                                && gpu_f_resize_compact_box_any_axis(
                                    (cur_w, cur_h),
                                    (out_w, out_h),
                                );
                            if compact_box {
                                let horizontal_words =
                                    if gpu_f_resize_compact_box_axis(cur_w, out_w) {
                                        encode_resize_compact_box_axis(cur_w, out_w)?
                                    } else {
                                        let coeffs =
                                            precompute_coeffs_f64(out_w, cur_w, kernel, support);
                                        encode_resize_coeffs_f64(&coeffs)?
                                    };
                                let vertical_words = if gpu_f_resize_compact_box_axis(cur_h, out_h)
                                {
                                    encode_resize_compact_box_axis(cur_h, out_h)?
                                } else {
                                    let coeffs =
                                        precompute_coeffs_f64(out_h, cur_h, kernel, support);
                                    encode_resize_coeffs_f64(&coeffs)?
                                };
                                Some(self.append_resize_coeff_word_ranges(
                                    &mut img2_arena,
                                    auxiliary_cache,
                                    &horizontal_words,
                                    &vertical_words,
                                    storage_alignment,
                                )?)
                            } else {
                                let horizontal_f64 =
                                    precompute_coeffs_f64(out_w, cur_w, kernel, support);
                                let vertical_f64 =
                                    precompute_coeffs_f64(out_h, cur_h, kernel, support);
                                Some(self.append_resize_f64_coeff_ranges(
                                    &mut img2_arena,
                                    auxiliary_cache,
                                    &horizontal_f64,
                                    &vertical_f64,
                                    storage_alignment,
                                )?)
                            }
                        } else {
                            Some(self.append_resize_coeff_ranges(
                                &mut img2_arena,
                                auxiliary_cache,
                                &horizontal,
                                &vertical,
                                storage_alignment,
                            )?)
                        }
                    }
                }
                PipelineOp::ResizeBoxed { .. } => {
                    let (horizontal, vertical) =
                        boxed_resize_coefficients.as_ref().ok_or_else(|| {
                            PilError::InternalError(
                                "GPU ResizeBoxed coefficients were not prepared".into(),
                            )
                        })?;
                    Some(self.append_resize_coeff_ranges(
                        &mut img2_arena,
                        auxiliary_cache,
                        horizontal,
                        vertical,
                        storage_alignment,
                    )?)
                }
                PipelineOp::Pad { filter, .. } => {
                    let ((resize_w, resize_h), _) =
                        gpu_pad_geometry(op, cur_w, cur_h).ok_or_else(|| {
                            PilError::ValueError("GPU Pad has no safe geometry".into())
                        })?;
                    // An identity contain resize is a byte-preserving copy in
                    // Pillow. The final placement shader reads the original
                    // image directly, so it needs no coefficient tables.
                    if (resize_w, resize_h) == (cur_w, cur_h) {
                        None
                    } else {
                        let resize_filter = if logical_mode == Some("P") {
                            ResampleFilter::Nearest
                        } else {
                            *filter
                        };
                        if f_resize_f64_is_exact
                            && logical_mode == Some("F")
                            && !matches!(resize_filter, ResampleFilter::Nearest)
                        {
                            let (kernel, support) = filter_from_resample(resize_filter);
                            let horizontal =
                                precompute_coeffs_f64(resize_w, cur_w, kernel, support);
                            let vertical = precompute_coeffs_f64(resize_h, cur_h, kernel, support);
                            Some(self.append_resize_f64_coeff_ranges(
                                &mut img2_arena,
                                auxiliary_cache,
                                &horizontal,
                                &vertical,
                                storage_alignment,
                            )?)
                        } else {
                            let horizontal =
                                gpu_resize_coefficients(resize_w, cur_w, resize_filter);
                            let vertical = gpu_resize_coefficients(resize_h, cur_h, resize_filter);
                            Some(self.append_resize_coeff_ranges(
                                &mut img2_arena,
                                auxiliary_cache,
                                &horizontal,
                                &vertical,
                                storage_alignment,
                            )?)
                        }
                    }
                }
                PipelineOp::Fit {
                    bleed,
                    centering,
                    filter,
                    ..
                } => {
                    let (horizontal, vertical) = gpu_fit_coefficients(
                        (cur_w, cur_h),
                        (out_w, out_h),
                        *bleed,
                        *centering,
                        *filter,
                    )
                    .ok_or_else(|| {
                        PilError::ValueError("GPU Fit has no safe crop geometry".into())
                    })?;
                    let ranges = self.append_resize_coeff_ranges(
                        &mut img2_arena,
                        auxiliary_cache,
                        &horizontal,
                        &vertical,
                        storage_alignment,
                    )?;
                    Some(ranges)
                }
                _ => None,
            };
            resize_coeff_ranges.push(resize_coeff_range);
            pad_resize_dims.push(
                matches!(op, PipelineOp::Pad { .. })
                    .then(|| gpu_pad_geometry(op, cur_w, cur_h).map(|(dims, _)| dims))
                    .flatten(),
            );

            let second_range =
                if gpu_transform_uses_geometry_table(op, logical_mode, (cur_w, cur_h)) {
                    let values = gpu_transform_geometry_words(op, (cur_w, cur_h), logical_mode)?;
                    let mut range = append_arena_slice(&mut img2_arena, &values, storage_alignment);
                    range.offset += (auxiliary_cache.img2_values.len() * 4) as u64;
                    Some(range)
                } else if let PipelineOp::PutData { data, mode } = op {
                    let second_values = if packed_luma_putdata {
                        pack_luma8_putdata(data, buffers.capacity)?
                    } else {
                        pack_put_data(data, *mode, buffers.capacity)?
                    };
                    if second_values.is_empty() {
                        None
                    } else {
                        let mut range =
                            append_arena_slice(&mut img2_arena, &second_values, storage_alignment);
                        range.offset += (auxiliary_cache.img2_values.len() * 4) as u64;
                        Some(range)
                    }
                } else if let Some(second) = auxiliary_images[index].second.as_ref() {
                    if matches!(op, PipelineOp::ConvertLab { .. }) {
                        let key = Arc::as_ptr(second) as usize;
                        if let Some(range) = auxiliary_cache.second_ranges.get(&key).copied() {
                            Some(range)
                        } else if let Some(range) = second_cache.get(&key).copied() {
                            Some(range)
                        } else {
                            let mut range = append_arena_slice(
                                &mut img2_arena,
                                crate::lab::gpu_table_words(),
                                storage_alignment,
                            );
                            range.offset += (auxiliary_cache.img2_values.len() * 4) as u64;
                            second_cache.insert(key, range);
                            Some(range)
                        }
                    } else if gpu_luma16_paste_source(op, op_mode, second) {
                        let DynamicImage::ImageLuma16(source) = second.as_ref() else {
                            return Err(PilError::InternalError(
                                "GPU typed Paste source was admitted without an ImageLuma16 buffer"
                                    .into(),
                            ));
                        };
                        let values = pack_luma16_numeric(source, buffers.capacity)?;
                        let mut range =
                            append_arena_slice(&mut img2_arena, &values, storage_alignment);
                        range.offset += (auxiliary_cache.img2_values.len() * 4) as u64;
                        Some(range)
                    } else {
                        let key = Arc::as_ptr(second) as usize;
                        if index == 0 && direct_alpha_composite_rgba.is_some() {
                            Some(BufferRange {
                                offset: 0,
                                size: direct_alpha_composite_rgba
                                    .expect("checked direct RGBA auxiliary")
                                    .len() as u64,
                            })
                        } else if let Some(range) = auxiliary_cache.second_ranges.get(&key).copied()
                        {
                            Some(range)
                        } else if let Some(range) = second_cache.get(&key).copied() {
                            Some(range)
                        } else {
                            let mut range = append_dynamic_rgba(
                                &mut img2_arena,
                                second.as_ref(),
                                buffers.capacity,
                                storage_alignment,
                            )?;
                            range.offset += (auxiliary_cache.img2_values.len() * 4) as u64;
                            second_cache.insert(key, range);
                            Some(range)
                        }
                    }
                } else {
                    None
                };
            img2_ranges.push(second_range);

            let third_range = if let Some(third) = auxiliary_images[index].third.as_ref() {
                let key = Arc::as_ptr(third) as usize;
                if let Some(range) = auxiliary_cache.third_ranges.get(&key).copied() {
                    Some(range)
                } else if let Some(range) = third_cache.get(&key).copied() {
                    Some(range)
                } else {
                    let mut range = append_dynamic_rgba(
                        &mut img3_arena,
                        third.as_ref(),
                        buffers.capacity,
                        storage_alignment,
                    )?;
                    range.offset += (auxiliary_cache.img3_values.len() * 4) as u64;
                    third_cache.insert(key, range);
                    Some(range)
                }
            } else {
                None
            };
            img3_ranges.push(third_range);

            let lut_values = if cached.is_lut
                && !matches!(
                    cached.variant_name,
                    "__internal_resize_h"
                        | "__internal_resize_v"
                        | "__internal_resize_v_pad_rgbx"
                        | "__internal_resize_h_pad_luma_packed"
                        | "__internal_resize_v_pad_luma_packed"
                ) {
                Some(extract_lut(op, op_mode).ok_or_else(|| {
                    PilError::ValueError(format!(
                        "GPU LUT length does not match source mode for '{}'",
                        registry::variant_key(op)
                    ))
                })?)
            } else {
                None
            };
            lut_ranges.push(lut_values.map(|lut| {
                if let Some(range) = auxiliary_cache.lut_ranges.get(&lut).copied() {
                    range
                } else if let Some(range) = lut_cache.get(&lut).copied() {
                    range
                } else {
                    let mut range = append_arena_slice(&mut lut_arena, &lut, storage_alignment);
                    range.offset += (auxiliary_cache.lut_values.len() * 4) as u64;
                    lut_cache.insert(lut, range);
                    range
                }
            }));

            cur_w = out_w;
            cur_h = out_h;
            current_mode = gpu_mode_after_op(current_mode, op);
        }

        buffers.params_arena.ensure_capacity(
            &self.device,
            "gpu_batch_params",
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            params_arena
                .len()
                .saturating_mul(std::mem::size_of::<u32>()),
            uniform_alignment,
        );
        self.queue.write_buffer(
            &buffers.params_arena.buffer,
            0,
            bytemuck::cast_slice(&params_arena),
        );

        let direct_img2_bytes = direct_alpha_composite_rgba.unwrap_or_default();
        let arena_img2_bytes = (auxiliary_cache.img2_values.len() + img2_arena.len())
            .saturating_mul(std::mem::size_of::<u32>());
        let required_img2_bytes = arena_img2_bytes.max(direct_img2_bytes.len());
        let img2 = if required_img2_bytes == 0 {
            None
        } else {
            let previous_capacity = buffers.img2_arena.capacity_bytes;
            buffers.img2_arena.ensure_capacity(
                &self.device,
                "gpu_batch_img2",
                wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                required_img2_bytes,
                storage_alignment,
            );
            if buffers.img2_arena.capacity_bytes != previous_capacity
                && !auxiliary_cache.img2_values.is_empty()
            {
                self.queue.write_buffer(
                    &buffers.img2_arena.buffer,
                    0,
                    bytemuck::cast_slice(&auxiliary_cache.img2_values),
                );
            }
            if !img2_arena.is_empty() {
                self.queue.write_buffer(
                    &buffers.img2_arena.buffer,
                    (auxiliary_cache.img2_values.len() * 4) as u64,
                    bytemuck::cast_slice(&img2_arena),
                );
            }
            if !direct_img2_bytes.is_empty() {
                self.queue
                    .write_buffer(&buffers.img2_arena.buffer, 0, direct_img2_bytes);
            }
            Some(&buffers.img2_arena.buffer)
        };
        let img3 = if auxiliary_cache.img3_values.is_empty() && img3_arena.is_empty() {
            None
        } else {
            let previous_capacity = buffers.img3_arena.capacity_bytes;
            buffers.img3_arena.ensure_capacity(
                &self.device,
                "gpu_batch_img3",
                wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                (auxiliary_cache.img3_values.len() + img3_arena.len())
                    .saturating_mul(std::mem::size_of::<u32>()),
                storage_alignment,
            );
            if buffers.img3_arena.capacity_bytes != previous_capacity
                && !auxiliary_cache.img3_values.is_empty()
            {
                self.queue.write_buffer(
                    &buffers.img3_arena.buffer,
                    0,
                    bytemuck::cast_slice(&auxiliary_cache.img3_values),
                );
            }
            if !img3_arena.is_empty() {
                self.queue.write_buffer(
                    &buffers.img3_arena.buffer,
                    (auxiliary_cache.img3_values.len() * 4) as u64,
                    bytemuck::cast_slice(&img3_arena),
                );
            }
            Some(&buffers.img3_arena.buffer)
        };
        let lut = if auxiliary_cache.lut_values.is_empty() && lut_arena.is_empty() {
            None
        } else {
            let previous_capacity = buffers.lut_arena.capacity_bytes;
            buffers.lut_arena.ensure_capacity(
                &self.device,
                "gpu_batch_lut",
                wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                (auxiliary_cache.lut_values.len() + lut_arena.len())
                    .saturating_mul(std::mem::size_of::<u32>()),
                storage_alignment,
            );
            if buffers.lut_arena.capacity_bytes != previous_capacity
                && !auxiliary_cache.lut_values.is_empty()
            {
                self.queue.write_buffer(
                    &buffers.lut_arena.buffer,
                    0,
                    bytemuck::cast_slice(&auxiliary_cache.lut_values),
                );
            }
            if !lut_arena.is_empty() {
                self.queue.write_buffer(
                    &buffers.lut_arena.buffer,
                    (auxiliary_cache.lut_values.len() * 4) as u64,
                    bytemuck::cast_slice(&lut_arena),
                );
            }
            Some(&buffers.lut_arena.buffer)
        };

        let resource_telemetry = PipelineResourceTelemetry {
            parameter_bytes: (params_arena.len() * std::mem::size_of::<u32>()) as u64,
            auxiliary_bytes: ((img2_arena.len() + img3_arena.len() + lut_arena.len())
                * std::mem::size_of::<u32>()) as u64
                + direct_img2_bytes.len() as u64,
            ..PipelineResourceTelemetry::default()
        };

        Ok(PreparedGpuBatch {
            resources: GpuBatchResources {
                buf_a: &buffers.buf_a,
                buf_b: &buffers.buf_b,
                initial_input: None,
                fallback_img2: &buffers.buf_img2,
                fallback_img3: &buffers.buf_img3,
                fallback_lut: &buffers.lut_buf,
                histogram: &buffers.histogram_buf,
                params: &buffers.params_arena.buffer,
                params_ranges,
                img2,
                img2_ranges,
                resize_coeff_ranges,
                img3,
                img3_ranges,
                lut,
                lut_ranges,
                native_expand_output,
                native_luma_transform_output,
                native_rgb_packed_output,
                native_la_transform_output,
                native_cover_packed_output,
            },
            input_dims,
            output_dims,
            resize_source_rows,
            pad_resize_dims,
            final_dims: (cur_w, cur_h),
            resource_telemetry,
        })
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

    #[cfg(target_endian = "little")]
    #[cfg(target_endian = "little")]
    #[cfg(target_endian = "little")]
    #[cfg(target_endian = "little")]

    /// Execute a full-size ImageChops composite using native byte buffers.
    /// The shader treats each packed word as four independent result bytes,
    /// so L/LA/RGB channels and an L/alpha mask stay compact from upload to
    /// readback instead of expanding every input to packed RGBA pixels.
    #[cfg(target_endian = "little")]
    fn execute_native_composite(
        &self,
        image: &DynamicImage,
        other: &DynamicImage,
        mask: &DynamicImage,
        channels: u8,
        mask_channels: u8,
        mask_channel: u8,
        buffers: &mut BufferPool,
    ) -> Result<DynamicImage, PilError> {
        let (width, height) = image.dimensions();
        let dimensions = CheckedDims::new(width, height, channels)?;
        let pixels = dimensions.total_pixels();
        let output_length = dimensions.total_bytes();
        let mask_length = pixels
            .checked_mul(usize::from(mask_channels))
            .ok_or_else(|| PilError::ValueError("GPU native composite is too large".into()))?;
        if image.as_bytes().len() != output_length
            || other.dimensions() != (width, height)
            || other.as_bytes().len() != output_length
            || mask.dimensions() != (width, height)
            || mask.as_bytes().len() != mask_length
        {
            return Err(PilError::InternalError(
                "GPU native composite layout mismatch".into(),
            ));
        }

        let transfer = |length: usize| -> Result<u64, PilError> {
            let length = u64::try_from(length)
                .map_err(|_| PilError::ValueError("GPU native composite is too large".into()))?;
            Ok(length
                .checked_add(3)
                .ok_or_else(|| PilError::ValueError("GPU native composite is too large".into()))?
                & !3)
        };
        let input_transfer = transfer(output_length)?;
        let output_transfer = input_transfer;
        let other_transfer = input_transfer;
        let mask_transfer = transfer(mask_length)?;
        if output_transfer > u64::from(buffers.capacity) * 4
            || mask_transfer > u64::from(self.device.limits().max_storage_buffer_binding_size)
        {
            return Err(PilError::ValueError(
                "GPU native composite exceeds adapter buffer limits".into(),
            ));
        }
        let word_count = u32::try_from(output_transfer / 4)
            .map_err(|_| PilError::ValueError("GPU native composite is too large".into()))?;
        let workgroups = word_count.div_ceil(64);
        let (groups_x, groups_y) = plan_native_composite_dispatch(
            word_count,
            self.device.limits().max_compute_workgroups_per_dimension,
        )
        .ok_or_else(|| {
            PilError::ValueError("GPU native composite exceeds adapter workgroup grid limit".into())
        })?;

        buffers.img2_arena.ensure_capacity(
            &self.device,
            "gpu_composite_native_img2",
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            other_transfer as usize,
            4,
        );
        buffers.img3_arena.ensure_capacity(
            &self.device,
            "gpu_composite_native_mask",
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mask_transfer as usize,
            4,
        );
        buffers.params_arena.ensure_capacity(
            &self.device,
            "gpu_composite_native_params",
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            32,
            self.device.limits().min_uniform_buffer_offset_alignment as usize,
        );

        let mut parameters = [0u32; 8];
        parameters[0] = u32::from(channels);
        parameters[1] = u32::from(mask_channels);
        parameters[2] = u32::from(mask_channel);
        parameters[3] = u32::try_from(output_length)
            .map_err(|_| PilError::ValueError("GPU native composite is too large".into()))?;
        parameters[4] = word_count;
        parameters[5] = groups_x;
        let write_bytes = |buffer: &wgpu::Buffer, bytes: &[u8], size: u64| {
            let size = NonZeroU64::new(size).ok_or_else(|| {
                PilError::InternalError("GPU native composite upload is empty".into())
            })?;
            let mut view = self
                .queue
                .write_buffer_with(buffer, 0, size)
                .ok_or_else(|| {
                    PilError::InternalError("GPU native composite staging allocation failed".into())
                })?;
            let mapped = view.as_mut();
            mapped[..bytes.len()].copy_from_slice(bytes);
            mapped[bytes.len()..].fill(0);
            Ok::<(), PilError>(())
        };
        write_bytes(&buffers.buf_a, image.as_bytes(), input_transfer)?;
        write_bytes(&buffers.img2_arena.buffer, other.as_bytes(), other_transfer)?;
        write_bytes(&buffers.img3_arena.buffer, mask.as_bytes(), mask_transfer)?;
        self.queue.write_buffer(
            &buffers.params_arena.buffer,
            0,
            bytemuck::cast_slice(&parameters),
        );
        let cached = self.resolve_pipeline(
            "__internal_composite_native",
            "composite_native.wgsl",
            include_str!("shaders/composite_native.wgsl"),
        )?;

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gpu_native_composite"),
            layout: &cached.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: ranged_binding(
                        &buffers.buf_a,
                        Some(BufferRange {
                            offset: 0,
                            size: input_transfer,
                        }),
                    )?,
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: ranged_binding(
                        &buffers.img2_arena.buffer,
                        Some(BufferRange {
                            offset: 0,
                            size: other_transfer,
                        }),
                    )?,
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: ranged_binding(
                        &buffers.img3_arena.buffer,
                        Some(BufferRange {
                            offset: 0,
                            size: mask_transfer,
                        }),
                    )?,
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: ranged_binding(
                        &buffers.buf_b,
                        Some(BufferRange {
                            offset: 0,
                            size: output_transfer,
                        }),
                    )?,
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: ranged_binding(
                        &buffers.params_arena.buffer,
                        Some(BufferRange {
                            offset: 0,
                            size: 32,
                        }),
                    )?,
                },
            ],
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("gpu_native_composite"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("gpu_native_composite"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&cached.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            crate::compute::record_gpu_shader_dispatch(
                cached.variant_name,
                cached.shader_file,
                u64::from(workgroups),
            );
            pass.dispatch_workgroups(groups_x, groups_y, 1);
        }
        let readback = self.prepare_readback(&buffers.buf_b, output_transfer)?;
        if let ReadbackTarget::Staging(staging) = &readback {
            encoder.copy_buffer_to_buffer(&buffers.buf_b, 0, &staging.buffer, 0, output_transfer);
        }
        self.queue.submit(Some(encoder.finish()));
        self.poll_device("GPU native composite submission")?;
        let result =
            self.readback_with(output_transfer, readback.buffer(buffers, false), |mapped| {
                if mapped.len() != output_transfer as usize {
                    return Err(PilError::InternalError(
                        "GPU native composite readback length mismatch".into(),
                    ));
                }
                let bytes = mapped[..output_length].to_vec();
                crate::compute::record_pipeline_allocation(bytes.len());
                crate::image_utils::raw_bytes_to_image(width, height, bytes, usize::from(channels))
            })?;
        crate::compute::record_pipeline_resource_telemetry(PipelineResourceTelemetry {
            upload_bytes: input_transfer + other_transfer + mask_transfer,
            auxiliary_bytes: other_transfer + mask_transfer,
            readback_bytes: output_transfer,
            parameter_bytes: 32,
            retained_cache_bytes: buffers.retained_bytes(),
            full_frame_copy_count: 3 + u64::from(matches!(&readback, ReadbackTarget::Staging(_))),
            mode_conversion_count: 0,
            ..PipelineResourceTelemetry::default()
        });
        crate::compute::record_pipeline_dispatch_count(1);
        if let ReadbackTarget::Staging(staging) = readback {
            self.recycle_staging(staging);
        }
        Ok(result)
    }

    /// Merge three native L planes into packed RGB bytes without RGBA staging.
    #[cfg(target_endian = "little")]
    fn upload_native_rgb_luma_merge_mapped(
        &self,
        red: &[u8],
        green: &[u8],
        blue: &[u8],
        dispatch: &NativeRgbMergeDispatch,
        auxiliary_offset: u64,
        transfer_bytes: u64,
        buffers: &mut BufferPool,
    ) -> Result<(), PilError> {
        if !self.direct_primary_readback
            || red.len() != dispatch.input_bytes
            || green.len().checked_add(blue.len()) != Some(dispatch.auxiliary_bytes)
            || auxiliary_offset < dispatch.input_transfer_bytes
            || auxiliary_offset.checked_add(dispatch.auxiliary_transfer_bytes)
                != Some(transfer_bytes)
        {
            return Err(PilError::InternalError(
                "GPU mapped RGB Merge input layout mismatch".into(),
            ));
        }

        let transfer_len = usize::try_from(transfer_bytes)
            .map_err(|_| PilError::ValueError("GPU mapped RGB Merge input is too large".into()))?;
        let auxiliary_start = usize::try_from(auxiliary_offset)
            .map_err(|_| PilError::ValueError("GPU mapped RGB Merge offset is too large".into()))?;
        let green_end = auxiliary_start
            .checked_add(green.len())
            .ok_or_else(|| PilError::ValueError("GPU mapped RGB Merge input overflow".into()))?;
        let blue_end = green_end
            .checked_add(blue.len())
            .ok_or_else(|| PilError::ValueError("GPU mapped RGB Merge input overflow".into()))?;
        if blue_end > transfer_len {
            return Err(PilError::InternalError(
                "GPU mapped RGB Merge bands exceed their checked buffer".into(),
            ));
        }

        let usage = wgpu::BufferUsages::MAP_WRITE | wgpu::BufferUsages::STORAGE;
        let input = buffers.mapped_native_input.get_or_insert_with(|| {
            ReusableGpuBuffer::new(
                &self.device,
                "gpu_mapped_native_rgb_merge_input",
                usage,
                transfer_len,
                4,
            )
        });
        input.ensure_capacity(
            &self.device,
            "gpu_mapped_native_rgb_merge_input",
            usage,
            transfer_len,
            4,
        );
        self.wait_for_buffer_mapping(
            transfer_bytes,
            &input.buffer,
            wgpu::MapMode::Write,
            "GPU mapped RGB Merge input",
        )?;
        let result = {
            let mut view = input.buffer.slice(..transfer_bytes).get_mapped_range_mut();
            let mapped = view.as_mut();
            if mapped.len() != transfer_len {
                Err(PilError::InternalError(
                    "GPU mapped RGB Merge input has an unexpected length".into(),
                ))
            } else {
                mapped.fill(0);
                mapped[..red.len()].copy_from_slice(red);
                mapped[auxiliary_start..auxiliary_start + green.len()].copy_from_slice(green);
                mapped[green_end..blue_end].copy_from_slice(blue);
                Ok(())
            }
        };
        input.buffer.unmap();
        result
    }

    #[cfg(target_endian = "little")]
    fn execute_native_rgb_luma_merge(
        &self,
        image: &DynamicImage,
        bands: &[crate::image::Image],
        dispatch: NativeRgbMergeDispatch,
        buffers: &mut BufferPool,
    ) -> Result<DynamicImage, PilError> {
        let DynamicImage::ImageLuma8(red) = image else {
            return Err(PilError::InternalError(
                "GPU native RGB Merge requires a native L first band".into(),
            ));
        };
        let dimensions = CheckedDims::new(dispatch.width, dispatch.height, 1)?;
        if bands.len() != 3
            || image.dimensions() != (dispatch.width, dispatch.height)
            || dimensions.total_pixels() != dispatch.pixels as usize
            || red.as_raw().len() != dispatch.input_bytes
            || CheckedDims::new(dispatch.width, dispatch.height, 3)?.total_bytes()
                != dispatch.output_bytes
            || dispatch.output_word_count as u64 * 4 != dispatch.output_transfer_bytes
        {
            return Err(PilError::InternalError(
                "GPU native RGB Merge input layout mismatch".into(),
            ));
        }

        let green_owner = bands[1].materialized_shared_for_ops()?;
        let blue_owner = bands[2].materialized_shared_for_ops()?;
        let (DynamicImage::ImageLuma8(green), DynamicImage::ImageLuma8(blue)) =
            (green_owner.as_ref(), blue_owner.as_ref())
        else {
            return Err(PilError::InternalError(
                "GPU native RGB Merge auxiliary bands must stay in L mode".into(),
            ));
        };
        if green.dimensions() != image.dimensions()
            || blue.dimensions() != image.dimensions()
            || green.as_raw().len().checked_add(blue.as_raw().len())
                != Some(dispatch.auxiliary_bytes)
        {
            return Err(PilError::InternalError(
                "GPU native RGB Merge auxiliary layout mismatch".into(),
            ));
        }

        let limits = self.device.limits();
        if dispatch.input_transfer_bytes > u64::from(buffers.capacity) * 4
            || dispatch.output_transfer_bytes > u64::from(buffers.capacity) * 4
            || dispatch.auxiliary_transfer_bytes > u64::from(limits.max_storage_buffer_binding_size)
            || [
                dispatch.input_transfer_bytes,
                dispatch.auxiliary_transfer_bytes,
                dispatch.output_transfer_bytes,
            ]
            .into_iter()
            .any(|bytes| {
                bytes > u64::from(limits.max_storage_buffer_binding_size)
                    || bytes > limits.max_buffer_size
            })
        {
            return Err(PilError::ValueError(
                "GPU native RGB Merge exceeds adapter buffer limits".into(),
            ));
        }

        let auxiliary_offset_alignment =
            u64::from(limits.min_storage_buffer_offset_alignment.max(1));
        let mapped_auxiliary_offset = dispatch
            .input_transfer_bytes
            .checked_add(auxiliary_offset_alignment - 1)
            .map(|aligned| aligned / auxiliary_offset_alignment * auxiliary_offset_alignment);
        let mapped_input_layout = mapped_auxiliary_offset
            .and_then(|offset| {
                offset
                    .checked_add(dispatch.auxiliary_transfer_bytes)
                    .map(|end| (offset, end))
            })
            .filter(|(_, total)| *total <= limits.max_buffer_size)
            .filter(|(_, total)| usize::try_from(*total).is_ok());
        let mapped_input = self.direct_primary_readback && mapped_input_layout.is_some();
        buffers.params_arena.ensure_capacity(
            &self.device,
            "gpu_merge_native_rgb_luma_params",
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            32,
            limits.min_uniform_buffer_offset_alignment as usize,
        );

        let mapped_auxiliary_offset = if mapped_input {
            let (offset, total) = mapped_input_layout.expect("mapped layout was admitted");
            self.upload_native_rgb_luma_merge_mapped(
                red.as_raw(),
                green.as_raw(),
                blue.as_raw(),
                &dispatch,
                offset,
                total,
                buffers,
            )?;
            Some(offset)
        } else {
            buffers.img2_arena.ensure_capacity(
                &self.device,
                "gpu_merge_native_rgb_luma_auxiliary",
                wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                dispatch.auxiliary_transfer_bytes as usize,
                limits.min_storage_buffer_offset_alignment as usize,
            );
            let write_luma = |buffer: &wgpu::Buffer,
                              transfer_bytes: u64,
                              copy_samples: &mut dyn FnMut(&mut [u8])|
             -> Result<(), PilError> {
                let size = NonZeroU64::new(transfer_bytes).ok_or_else(|| {
                    PilError::InternalError("GPU native RGB Merge upload is empty".into())
                })?;
                let mut upload =
                    self.queue
                        .write_buffer_with(buffer, 0, size)
                        .ok_or_else(|| {
                            PilError::InternalError(
                                "GPU native RGB Merge staging allocation failed".into(),
                            )
                        })?;
                let mapped = upload.as_mut();
                mapped.fill(0);
                copy_samples(mapped);
                Ok(())
            };
            write_luma(
                &buffers.buf_a,
                dispatch.input_transfer_bytes,
                &mut |mapped| {
                    mapped[..dispatch.input_bytes].copy_from_slice(red.as_raw());
                },
            )?;
            write_luma(
                &buffers.img2_arena.buffer,
                dispatch.auxiliary_transfer_bytes,
                &mut |mapped| {
                    let green_len = green.as_raw().len();
                    mapped[..green_len].copy_from_slice(green.as_raw());
                    mapped[green_len..green_len + blue.as_raw().len()]
                        .copy_from_slice(blue.as_raw());
                },
            )?;
            None
        };

        let parameters = [
            dispatch.invocation_row_width,
            dispatch.output_pixel_groups,
            GPU_NATIVE_RGB_LUMA_MERGE_MODE,
            dispatch.output_word_count,
            dispatch.pixels,
            0,
            0,
            0,
        ];
        let (input_buffer, auxiliary_buffer, input_range, auxiliary_range) =
            if let Some(auxiliary_offset) = mapped_auxiliary_offset {
                let input = &buffers
                    .mapped_native_input
                    .as_ref()
                    .expect("mapped RGB Merge upload owns its input buffer")
                    .buffer;
                (
                    input,
                    input,
                    BufferRange {
                        offset: 0,
                        size: dispatch.input_transfer_bytes,
                    },
                    BufferRange {
                        offset: auxiliary_offset,
                        size: dispatch.auxiliary_transfer_bytes,
                    },
                )
            } else {
                (
                    &buffers.buf_a,
                    &buffers.img2_arena.buffer,
                    BufferRange {
                        offset: 0,
                        size: dispatch.input_transfer_bytes,
                    },
                    BufferRange {
                        offset: 0,
                        size: dispatch.auxiliary_transfer_bytes,
                    },
                )
            };
        self.queue.write_buffer(
            &buffers.params_arena.buffer,
            0,
            bytemuck::cast_slice(&parameters),
        );
        let cached = self.resolve_pipeline(
            "__internal_merge_native_rgb_luma",
            "merge.wgsl",
            include_str!("shaders/merge.wgsl"),
        )?;
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gpu_native_rgb_luma_merge"),
            layout: &cached.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: ranged_binding(input_buffer, Some(input_range))?,
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: ranged_binding(auxiliary_buffer, Some(auxiliary_range))?,
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: ranged_binding(
                        &buffers.buf_b,
                        Some(BufferRange {
                            offset: 0,
                            size: dispatch.output_transfer_bytes,
                        }),
                    )?,
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: ranged_binding(
                        &buffers.params_arena.buffer,
                        Some(BufferRange {
                            offset: 0,
                            size: 32,
                        }),
                    )?,
                },
            ],
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("gpu_native_rgb_luma_merge"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("gpu_native_rgb_luma_merge"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&cached.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            crate::compute::record_gpu_shader_dispatch(
                cached.variant_name,
                cached.shader_file,
                u64::from(dispatch.groups_x) * u64::from(dispatch.groups_y),
            );
            pass.dispatch_workgroups(dispatch.groups_x, dispatch.groups_y, 1);
        }
        let readback = self.prepare_readback(&buffers.buf_b, dispatch.output_transfer_bytes)?;
        if let ReadbackTarget::Staging(staging) = &readback {
            encoder.copy_buffer_to_buffer(
                &buffers.buf_b,
                0,
                &staging.buffer,
                0,
                dispatch.output_transfer_bytes,
            );
        }
        self.queue.submit(Some(encoder.finish()));
        let result = self.readback_to_native_channels(
            dispatch.width,
            dispatch.height,
            3,
            readback.buffer(buffers, false),
        )?;
        crate::compute::record_pipeline_resource_telemetry(PipelineResourceTelemetry {
            upload_bytes: dispatch.input_transfer_bytes + dispatch.auxiliary_transfer_bytes,
            auxiliary_bytes: dispatch.auxiliary_transfer_bytes,
            readback_bytes: dispatch.output_transfer_bytes,
            parameter_bytes: 32,
            retained_cache_bytes: buffers.retained_bytes(),
            full_frame_copy_count: u64::from(!mapped_input) * 3
                + u64::from(matches!(&readback, ReadbackTarget::Staging(_))),
            mode_conversion_count: 0,
            ..PipelineResourceTelemetry::default()
        });
        crate::compute::record_pipeline_dispatch_count(1);
        if let ReadbackTarget::Staging(staging) = readback {
            self.recycle_staging(staging);
        }
        Ok(result)
    }

    /// Copy or clip one exact-layout unmasked L/LA/RGB Paste in native bytes.
    /// The shader packs each output word directly, avoiding RGBA staging and
    /// mode restoration; RGB bytes that cross word boundaries share one writer.
    #[cfg(target_endian = "little")]

    /// Paste native packed RGB bytes into an RGBA destination without
    /// allocating or uploading an expanded RGBA source image.
    #[cfg(target_endian = "little")]

    /// Execute exact masked L/LA/PA/RGB/HSV/RGBA Paste with compact native buffers.
    #[cfg(target_endian = "little")]

    /// Keep identity-contain HSV Pad in its native three-byte representation.
    #[cfg(target_endian = "little")]
    fn execute_native_hsv_pad(
        &self,
        image: &DynamicImage,
        dispatch: NativeHsvPadDispatch,
        buffers: &mut BufferPool,
    ) -> Result<DynamicImage, PilError> {
        let DynamicImage::ImageRgb8(rgb) = image else {
            return Err(PilError::InternalError(
                "GPU native HSV Pad requires ImageRgb8 storage".into(),
            ));
        };
        if rgb.as_raw().len() != dispatch.input_bytes
            || CheckedDims::new(image.width(), image.height(), 3)?.total_bytes()
                != dispatch.input_bytes
            || dispatch.output.transfer_bytes
                > u64::from(buffers.capacity)
                    .checked_mul(4)
                    .ok_or_else(|| PilError::ValueError("GPU HSV Pad buffer is too large".into()))?
            || dispatch.input_transfer_bytes > u64::from(buffers.capacity) * 4
        {
            return Err(PilError::InternalError(
                "GPU native HSV Pad layout exceeds its checked buffers".into(),
            ));
        }

        buffers.upload_native_channel_bytes(
            &self.queue,
            image.width(),
            image.height(),
            3,
            rgb.as_raw(),
        )?;
        buffers.params_arena.ensure_capacity(
            &self.device,
            "gpu_pad_hsv_native_rgb_params",
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            64,
            self.device.limits().min_uniform_buffer_offset_alignment as usize,
        );
        let parameters = [
            image.width(),
            image.height(),
            2,
            0,
            image.width(),
            image.height(),
            3,
            0,
            dispatch.output_width,
            dispatch.output_height,
            dispatch.fill_pixel,
            0,
            dispatch.offset_y,
            0,
            0,
            0,
        ];
        self.queue.write_buffer(
            &buffers.params_arena.buffer,
            0,
            bytemuck::cast_slice(&parameters),
        );
        let cached = self.resolve_pipeline(
            "__internal_pad_hsv_native_rgb",
            "pad_hsv_native_rgb.wgsl",
            include_str!("shaders/pad_hsv_native_rgb.wgsl"),
        )?;
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gpu_pad_hsv_native_rgb"),
            layout: &cached.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: ranged_binding(
                        &buffers.buf_a,
                        Some(BufferRange {
                            offset: 0,
                            size: dispatch.input_transfer_bytes,
                        }),
                    )?,
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: ranged_binding(
                        &buffers.buf_b,
                        Some(BufferRange {
                            offset: 0,
                            size: dispatch.output.transfer_bytes,
                        }),
                    )?,
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: ranged_binding(
                        &buffers.params_arena.buffer,
                        Some(BufferRange {
                            offset: 0,
                            size: 64,
                        }),
                    )?,
                },
            ],
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("gpu_pad_hsv_native_rgb_encoder"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("gpu_pad_hsv_native_rgb_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&cached.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            crate::compute::record_gpu_shader_dispatch(
                cached.variant_name,
                cached.shader_file,
                u64::from(dispatch.output.groups_x)
                    .saturating_mul(u64::from(dispatch.output.groups_y)),
            );
            pass.dispatch_workgroups(dispatch.output.groups_x, dispatch.output.groups_y, 1);
        }
        let readback = self.prepare_readback(&buffers.buf_b, dispatch.output.transfer_bytes)?;
        if let ReadbackTarget::Staging(staging) = &readback {
            encoder.copy_buffer_to_buffer(
                &buffers.buf_b,
                0,
                &staging.buffer,
                0,
                dispatch.output.transfer_bytes,
            );
        }
        self.queue.submit(Some(encoder.finish()));
        let result = self.readback_to_native_channels(
            dispatch.output_width,
            dispatch.output_height,
            3,
            readback.buffer(buffers, false),
        )?;
        crate::compute::record_pipeline_resource_telemetry(PipelineResourceTelemetry {
            upload_bytes: dispatch.input_transfer_bytes,
            readback_bytes: dispatch.output.transfer_bytes,
            parameter_bytes: 64,
            retained_cache_bytes: buffers.retained_bytes(),
            full_frame_copy_count: 2 + u64::from(matches!(&readback, ReadbackTarget::Staging(_))),
            mode_conversion_count: 0,
            ..PipelineResourceTelemetry::default()
        });
        crate::compute::record_pipeline_dispatch_count(1);
        if let ReadbackTarget::Staging(staging) = readback {
            self.recycle_staging(staging);
        }
        Ok(result)
    }

    /// Add scalar alpha to native RGB samples and write the required RGBA
    /// result without first expanding the host upload to four bytes per pixel.
    #[cfg(target_endian = "little")]
    fn execute_native_rgb_put_alpha(
        &self,
        image: &DynamicImage,
        alpha: u8,
        dispatch: NativeRgbPutAlphaDispatch,
        buffers: &mut BufferPool,
    ) -> Result<DynamicImage, PilError> {
        let DynamicImage::ImageRgb8(rgb) = image else {
            return Err(PilError::InternalError(
                "GPU native RGB PutAlpha requires ImageRgb8 storage".into(),
            ));
        };
        let (width, height) = rgb.dimensions();
        let dimensions = CheckedDims::new(width, height, 3)?;
        if dimensions.total_bytes() != dispatch.input_bytes
            || rgb.as_raw().len() != dispatch.input_bytes
            || CheckedDims::new(width, height, 4)?.total_bytes() != dispatch.output_bytes
            || dispatch.output_transfer_bytes > u64::from(buffers.capacity) * 4
        {
            return Err(PilError::InternalError(
                "GPU native RGB PutAlpha layout mismatch".into(),
            ));
        }

        buffers.upload_native_channel_bytes(&self.queue, width, height, 3, rgb.as_raw())?;
        buffers.params_arena.ensure_capacity(
            &self.device,
            "gpu_put_alpha_native_rgb_params",
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            16,
            self.device.limits().min_uniform_buffer_offset_alignment as usize,
        );
        let parameters = [
            width,
            height,
            u32::from(alpha),
            u32::try_from(dispatch.input_bytes).map_err(|_| {
                PilError::ValueError("GPU native RGB PutAlpha input is too large".into())
            })?,
        ];
        self.queue.write_buffer(
            &buffers.params_arena.buffer,
            0,
            bytemuck::cast_slice(&parameters),
        );
        let cached = self.resolve_pipeline(
            "PutAlphaNativeRGB",
            "put_alpha_native_rgb.wgsl",
            include_str!("shaders/put_alpha_native_rgb.wgsl"),
        )?;
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gpu_put_alpha_native_rgb"),
            layout: &cached.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: ranged_binding(
                        &buffers.buf_a,
                        Some(BufferRange {
                            offset: 0,
                            size: dispatch.input_transfer_bytes,
                        }),
                    )?,
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: ranged_binding(
                        &buffers.buf_b,
                        Some(BufferRange {
                            offset: 0,
                            size: dispatch.output_transfer_bytes,
                        }),
                    )?,
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: ranged_binding(
                        &buffers.params_arena.buffer,
                        Some(BufferRange {
                            offset: 0,
                            size: 16,
                        }),
                    )?,
                },
            ],
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("gpu_put_alpha_native_rgb"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("gpu_put_alpha_native_rgb"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&cached.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            crate::compute::record_gpu_shader_dispatch(
                cached.variant_name,
                cached.shader_file,
                u64::from(dispatch.groups_x) * u64::from(dispatch.groups_y),
            );
            pass.dispatch_workgroups(dispatch.groups_x, dispatch.groups_y, 1);
        }
        let readback = self.prepare_readback(&buffers.buf_b, dispatch.output_transfer_bytes)?;
        if let ReadbackTarget::Staging(staging) = &readback {
            encoder.copy_buffer_to_buffer(
                &buffers.buf_b,
                0,
                &staging.buffer,
                0,
                dispatch.output_transfer_bytes,
            );
        }
        self.queue.submit(Some(encoder.finish()));
        self.poll_device("GPU native RGB PutAlpha submission")?;
        let result = self.readback_with(
            dispatch.output_transfer_bytes,
            readback.buffer(buffers, false),
            |mapped| {
                if mapped.len() != dispatch.output_transfer_bytes as usize {
                    return Err(PilError::InternalError(
                        "GPU native RGB PutAlpha readback length mismatch".into(),
                    ));
                }
                let bytes = mapped[..dispatch.output_bytes].to_vec();
                crate::compute::record_pipeline_allocation(bytes.len());
                crate::image_utils::raw_bytes_to_image(width, height, bytes, 4)
            },
        )?;
        crate::compute::record_pipeline_resource_telemetry(PipelineResourceTelemetry {
            upload_bytes: dispatch.input_transfer_bytes,
            readback_bytes: dispatch.output_transfer_bytes,
            parameter_bytes: 16,
            retained_cache_bytes: buffers.retained_bytes(),
            full_frame_copy_count: 1 + u64::from(matches!(&readback, ReadbackTarget::Staging(_))),
            mode_conversion_count: 0,
            ..PipelineResourceTelemetry::default()
        });
        crate::compute::record_pipeline_dispatch_count(1);
        if let ReadbackTarget::Staging(staging) = readback {
            self.recycle_staging(staging);
        }
        Ok(result)
    }

    /// Add a mask-derived alpha to native RGB samples while writing the
    /// required RGBA result directly, without expanding either upload.
    #[cfg(target_endian = "little")]
    fn execute_native_rgb_put_alpha_data(
        &self,
        image: &DynamicImage,
        mask: &DynamicImage,
        dispatch: NativeRgbPutAlphaDataDispatch,
        buffers: &mut BufferPool,
    ) -> Result<DynamicImage, PilError> {
        let (DynamicImage::ImageRgb8(rgb), DynamicImage::ImageLuma8(alpha)) = (image, mask) else {
            return Err(PilError::InternalError(
                "GPU native RGB PutAlphaData requires RGB and L storage".into(),
            ));
        };
        let (width, height) = rgb.dimensions();
        let dimensions = CheckedDims::new(width, height, 3)?;
        let pixel_count = CheckedDims::new(width, height, 1)?.total_pixels();
        if rgb.dimensions() != alpha.dimensions()
            || dimensions.total_bytes() != dispatch.rgb_bytes
            || rgb.as_raw().len() != dispatch.rgb_bytes
            || alpha.as_raw().len() != dispatch.mask_bytes
            || pixel_count != dispatch.mask_bytes
            || CheckedDims::new(width, height, 4)?.total_bytes() != dispatch.output_bytes
            || dispatch.output_transfer_bytes > u64::from(buffers.capacity) * 4
        {
            return Err(PilError::InternalError(
                "GPU native RGB PutAlphaData layout mismatch".into(),
            ));
        }

        buffers.upload_native_channel_bytes(&self.queue, width, height, 3, rgb.as_raw())?;
        buffers.img3_arena.ensure_capacity(
            &self.device,
            "gpu_put_alpha_data_native_rgb_mask",
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            dispatch.mask_transfer_bytes as usize,
            4,
        );
        let mask_size = NonZeroU64::new(dispatch.mask_transfer_bytes).ok_or_else(|| {
            PilError::ValueError("GPU native RGB PutAlphaData mask is empty".into())
        })?;
        let mut mask_upload = self
            .queue
            .write_buffer_with(&buffers.img3_arena.buffer, 0, mask_size)
            .ok_or_else(|| {
                PilError::InternalError(
                    "GPU native RGB PutAlphaData mask staging allocation failed".into(),
                )
            })?;
        mask_upload.fill(0);
        mask_upload[..dispatch.mask_bytes].copy_from_slice(alpha.as_raw());
        drop(mask_upload);

        buffers.params_arena.ensure_capacity(
            &self.device,
            "gpu_put_alpha_data_native_rgb_params",
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            16,
            self.device.limits().min_uniform_buffer_offset_alignment as usize,
        );
        let parameters = [
            width,
            height,
            u32::try_from(dispatch.rgb_bytes).map_err(|_| {
                PilError::ValueError("GPU native RGB PutAlphaData input is too large".into())
            })?,
            u32::try_from(dispatch.mask_bytes).map_err(|_| {
                PilError::ValueError("GPU native RGB PutAlphaData mask is too large".into())
            })?,
        ];
        self.queue.write_buffer(
            &buffers.params_arena.buffer,
            0,
            bytemuck::cast_slice(&parameters),
        );
        let cached = self.resolve_pipeline(
            "PutAlphaDataNativeRGB",
            "put_alpha_data_native_rgb.wgsl",
            include_str!("shaders/put_alpha_data_native_rgb.wgsl"),
        )?;
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gpu_put_alpha_data_native_rgb"),
            layout: &cached.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: ranged_binding(
                        &buffers.buf_a,
                        Some(BufferRange {
                            offset: 0,
                            size: dispatch.rgb_transfer_bytes,
                        }),
                    )?,
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: ranged_binding(
                        &buffers.img3_arena.buffer,
                        Some(BufferRange {
                            offset: 0,
                            size: dispatch.mask_transfer_bytes,
                        }),
                    )?,
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: ranged_binding(
                        &buffers.buf_b,
                        Some(BufferRange {
                            offset: 0,
                            size: dispatch.output_transfer_bytes,
                        }),
                    )?,
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: ranged_binding(
                        &buffers.params_arena.buffer,
                        Some(BufferRange {
                            offset: 0,
                            size: 16,
                        }),
                    )?,
                },
            ],
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("gpu_put_alpha_data_native_rgb"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("gpu_put_alpha_data_native_rgb"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&cached.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            crate::compute::record_gpu_shader_dispatch(
                cached.variant_name,
                cached.shader_file,
                u64::from(dispatch.groups_x) * u64::from(dispatch.groups_y),
            );
            pass.dispatch_workgroups(dispatch.groups_x, dispatch.groups_y, 1);
        }
        let readback = self.prepare_readback(&buffers.buf_b, dispatch.output_transfer_bytes)?;
        if let ReadbackTarget::Staging(staging) = &readback {
            encoder.copy_buffer_to_buffer(
                &buffers.buf_b,
                0,
                &staging.buffer,
                0,
                dispatch.output_transfer_bytes,
            );
        }
        self.queue.submit(Some(encoder.finish()));
        self.poll_device("GPU native RGB PutAlphaData submission")?;
        let result = self.readback_with(
            dispatch.output_transfer_bytes,
            readback.buffer(buffers, false),
            |mapped| {
                if mapped.len() != dispatch.output_transfer_bytes as usize {
                    return Err(PilError::InternalError(
                        "GPU native RGB PutAlphaData readback length mismatch".into(),
                    ));
                }
                let bytes = mapped[..dispatch.output_bytes].to_vec();
                crate::compute::record_pipeline_allocation(bytes.len());
                crate::image_utils::raw_bytes_to_image(width, height, bytes, 4)
            },
        )?;
        crate::compute::record_pipeline_resource_telemetry(PipelineResourceTelemetry {
            upload_bytes: dispatch.rgb_transfer_bytes,
            auxiliary_bytes: dispatch.mask_transfer_bytes,
            readback_bytes: dispatch.output_transfer_bytes,
            parameter_bytes: 16,
            retained_cache_bytes: buffers.retained_bytes(),
            full_frame_copy_count: 2 + u64::from(matches!(&readback, ReadbackTarget::Staging(_))),
            mode_conversion_count: 0,
            ..PipelineResourceTelemetry::default()
        });
        crate::compute::record_pipeline_dispatch_count(1);
        if let ReadbackTarget::Staging(staging) = readback {
            self.recycle_staging(staging);
        }
        Ok(result)
    }

    /// Replace the alpha byte in native RGBA pixels from a packed L mask.
    /// Both inputs stay in their original host layout; the shader writes the
    /// required RGBA output without constructing a widened mask image.
    #[cfg(target_endian = "little")]
    fn execute_native_rgba_put_alpha_data(
        &self,
        image: &DynamicImage,
        mask: &DynamicImage,
        dispatch: NativeRgbaPutAlphaDataDispatch,
        buffers: &mut BufferPool,
    ) -> Result<DynamicImage, PilError> {
        let (DynamicImage::ImageRgba8(rgba), DynamicImage::ImageLuma8(alpha)) = (image, mask)
        else {
            return Err(PilError::InternalError(
                "GPU native RGBA PutAlphaData requires RGBA and L storage".into(),
            ));
        };
        let (width, height) = rgba.dimensions();
        let dimensions = CheckedDims::new(width, height, 4)?;
        let pixel_count = CheckedDims::new(width, height, 1)?.total_pixels();
        if rgba.dimensions() != alpha.dimensions()
            || dimensions.total_bytes() != dispatch.rgba_bytes
            || rgba.as_raw().len() != dispatch.rgba_bytes
            || alpha.as_raw().len() != dispatch.mask_bytes
            || pixel_count != dispatch.mask_bytes
            || dispatch.output_transfer_bytes > u64::from(buffers.capacity) * 4
        {
            return Err(PilError::InternalError(
                "GPU native RGBA PutAlphaData layout mismatch".into(),
            ));
        }

        buffers.upload_native_channel_bytes(&self.queue, width, height, 4, rgba.as_raw())?;
        buffers.img3_arena.ensure_capacity(
            &self.device,
            "gpu_put_alpha_data_native_rgba_mask",
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            dispatch.mask_transfer_bytes as usize,
            4,
        );
        let mask_size = NonZeroU64::new(dispatch.mask_transfer_bytes).ok_or_else(|| {
            PilError::ValueError("GPU native RGBA PutAlphaData mask is empty".into())
        })?;
        let mut mask_upload = self
            .queue
            .write_buffer_with(&buffers.img3_arena.buffer, 0, mask_size)
            .ok_or_else(|| {
                PilError::InternalError(
                    "GPU native RGBA PutAlphaData mask staging allocation failed".into(),
                )
            })?;
        mask_upload.fill(0);
        mask_upload[..dispatch.mask_bytes].copy_from_slice(alpha.as_raw());
        drop(mask_upload);

        buffers.params_arena.ensure_capacity(
            &self.device,
            "gpu_put_alpha_data_native_rgba_params",
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            16,
            self.device.limits().min_uniform_buffer_offset_alignment as usize,
        );
        let parameters = [
            width,
            height,
            u32::try_from(dispatch.mask_bytes).map_err(|_| {
                PilError::ValueError("GPU native RGBA PutAlphaData mask is too large".into())
            })?,
            0,
        ];
        self.queue.write_buffer(
            &buffers.params_arena.buffer,
            0,
            bytemuck::cast_slice(&parameters),
        );
        let cached = self.resolve_pipeline(
            "PutAlphaDataNativeRGBA",
            "put_alpha_data_native_rgba.wgsl",
            include_str!("shaders/put_alpha_data_native_rgba.wgsl"),
        )?;
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gpu_put_alpha_data_native_rgba"),
            layout: &cached.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: ranged_binding(
                        &buffers.buf_a,
                        Some(BufferRange {
                            offset: 0,
                            size: dispatch.rgba_transfer_bytes,
                        }),
                    )?,
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: ranged_binding(
                        &buffers.img3_arena.buffer,
                        Some(BufferRange {
                            offset: 0,
                            size: dispatch.mask_transfer_bytes,
                        }),
                    )?,
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: ranged_binding(
                        &buffers.buf_b,
                        Some(BufferRange {
                            offset: 0,
                            size: dispatch.output_transfer_bytes,
                        }),
                    )?,
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: ranged_binding(
                        &buffers.params_arena.buffer,
                        Some(BufferRange {
                            offset: 0,
                            size: 16,
                        }),
                    )?,
                },
            ],
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("gpu_put_alpha_data_native_rgba"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("gpu_put_alpha_data_native_rgba"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&cached.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            crate::compute::record_gpu_shader_dispatch(
                cached.variant_name,
                cached.shader_file,
                u64::from(dispatch.groups_x) * u64::from(dispatch.groups_y),
            );
            pass.dispatch_workgroups(dispatch.groups_x, dispatch.groups_y, 1);
        }
        let readback = self.prepare_readback(&buffers.buf_b, dispatch.output_transfer_bytes)?;
        if let ReadbackTarget::Staging(staging) = &readback {
            encoder.copy_buffer_to_buffer(
                &buffers.buf_b,
                0,
                &staging.buffer,
                0,
                dispatch.output_transfer_bytes,
            );
        }
        self.queue.submit(Some(encoder.finish()));
        self.poll_device("GPU native RGBA PutAlphaData submission")?;
        let result = self.readback_with(
            dispatch.output_transfer_bytes,
            readback.buffer(buffers, false),
            |mapped| {
                if mapped.len() != dispatch.output_transfer_bytes as usize {
                    return Err(PilError::InternalError(
                        "GPU native RGBA PutAlphaData readback length mismatch".into(),
                    ));
                }
                let bytes = mapped[..dispatch.rgba_bytes].to_vec();
                crate::compute::record_pipeline_allocation(bytes.len());
                crate::image_utils::raw_bytes_to_image(width, height, bytes, 4)
            },
        )?;
        crate::compute::record_pipeline_resource_telemetry(PipelineResourceTelemetry {
            upload_bytes: dispatch.rgba_transfer_bytes,
            auxiliary_bytes: dispatch.mask_transfer_bytes,
            readback_bytes: dispatch.output_transfer_bytes,
            parameter_bytes: 16,
            retained_cache_bytes: buffers.retained_bytes(),
            full_frame_copy_count: 2 + u64::from(matches!(&readback, ReadbackTarget::Staging(_))),
            mode_conversion_count: 0,
            ..PipelineResourceTelemetry::default()
        });
        crate::compute::record_pipeline_dispatch_count(1);
        if let ReadbackTarget::Staging(staging) = readback {
            self.recycle_staging(staging);
        }
        Ok(result)
    }

    /// Execute admitted byte operations without transport mode expansion.
    #[cfg(target_endian = "little")]

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
        let mut op_modes = Vec::with_capacity(ops.len());
        let mut current_mode = mode;
        for op in ops {
            op_modes.push(current_mode);
            current_mode = gpu_mode_after_op(current_mode, op);
        }
        let mut resource_source_dims = Vec::with_capacity(ops.len());
        let mut resource_w = w;
        let mut resource_h = h;
        for op in ops {
            resource_source_dims.push((resource_w, resource_h));
            let next = match op_output_dims(op, resource_w, resource_h) {
                Some(dimensions) => dimensions,
                None if op_has_explicit_output_dimensions(op) => {
                    return Err(PilError::ValueError(format!(
                        "GPU operation '{}' has no safe source dimensions",
                        registry::variant_key(op)
                    )));
                }
                None => (resource_w, resource_h),
            };
            (resource_w, resource_h) = next;
        }
        let resource_bytes = ops
            .iter()
            .zip(auxiliary_images.iter())
            .enumerate()
            .map(|(index, (op, auxiliary))| {
                self.estimate_resource_bytes(
                    op,
                    auxiliary,
                    buffers,
                    uniform_alignment,
                    storage_alignment,
                    resource_source_dims[index],
                    op_modes[index],
                    packed_luma_putdata,
                    logical_mode,
                    f_resize_f64_is_exact,
                    f_resize_f64_ordered_is_exact,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut work_w = w;
        let mut work_h = h;
        let shader_work_items = ops
            .iter()
            .map(|op| {
                let next = match op_output_dims(op, work_w, work_h) {
                    Some(dimensions) => dimensions,
                    None if op_has_explicit_output_dimensions(op) => {
                        return Err(PilError::ValueError(format!(
                            "GPU operation '{}' has no safe work dimensions",
                            registry::variant_key(op)
                        )));
                    }
                    None => (work_w, work_h),
                };
                let work = gpu_shader_work_items(op, (work_w, work_h), next, logical_mode)
                    .ok_or_else(|| {
                        PilError::ValueError(format!(
                            "GPU operation '{}' has no bounded shader work estimate",
                            registry::variant_key(op)
                        ))
                    })?;
                (work_w, work_h) = next;
                Ok(work)
            })
            .collect::<Result<Vec<_>, _>>()?;
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
        current_mode = mode;
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
        let source_is_luma_point_run = ops.iter().all(|op| matches!(op, PipelineOp::Eval { .. }))
            && matches!(img, DynamicImage::ImageLuma8(_))
            && matches!(mode, None | Some("L"));
        let native_cover_input_channels = gpu_native_cover_input_channels(ops, img, mode);

        // Keep the public operation list intact for routing and parity, but
        // execute contiguous exact point runs as one LUT dispatch when the
        // logical mode matches the source's native byte layout.  The helper
        // below excludes palette and typed modes whose public point contract
        // is not represented by the batch-wide GPU mode word.
        // For unmasked native L/RGB inputs, histogram selection is small
        // control-plane work compared with the packed full-frame GPU path.
        // Derive Pillow's exact LUT on the host, then keep the complete pixel
        // remap on the GPU as one native lookup dispatch. Masked and other
        // layouts keep the four-pass GPU histogram implementation.
        let host_autocontrast_lut = if let [PipelineOp::Autocontrast { cutoff, mask }] = ops {
            let native_layout = matches!(
                img,
                DynamicImage::ImageLuma8(_) | DynamicImage::ImageRgb8(_)
            ) && (mode.is_none()
                || (matches!(img, DynamicImage::ImageLuma8(_)) && mode == Some("L"))
                || (matches!(img, DynamicImage::ImageRgb8(_)) && mode == Some("RGB")));
            if mask.is_none()
                && cutoff.is_finite()
                && native_layout
                && img.width() != 0
                && img.height() != 0
            {
                Some(PipelineOp::Eval {
                    lut: crate::compute::pool_cpu::ops::imageops::autocontrast_lut(
                        img, *cutoff, None,
                    )?
                    .into(),
                })
            } else {
                None
            }
        } else {
            None
        };
        let host_autocontrast_luma_lut = host_autocontrast_lut.is_some()
            && matches!(img, DynamicImage::ImageLuma8(_))
            && matches!(mode, None | Some("L"));
        let host_autocontrast_rgb_lut = host_autocontrast_lut.is_some()
            && matches!(img, DynamicImage::ImageRgb8(_))
            && matches!(mode, None | Some("RGB"));
        let mut dispatch_ops: Vec<PipelineOp> = if let Some(op) = host_autocontrast_lut {
            vec![op]
        } else {
            ops.iter()
                .filter(|op| !gpu_reduce_is_identity(op) && !gpu_sharpness_is_identity(op))
                .cloned()
                .collect()
        };
        if dispatch_ops.is_empty() {
            // Exact identities have no pixel invocations. Return an
            // independent result without paying for upload, dispatch, or
            // readback. The selected GPU backend still owns completion and
            // reports an explicit zero-dispatch receipt.
            crate::compute::record_pipeline_dispatch_count(0);
            return Ok(img.clone());
        }
        dispatch_ops = if gpu_byte_point_mode_allowed(img, mode) {
            fuse_gpu_point_ops(&dispatch_ops, mode_code(img))
        } else {
            dispatch_ops
        };
        if gpu_image_layout_is_supported(img) {
            // Transpose only relocates native samples. Composing it does not
            // interpret palette indices or the packed I/F words, so the
            // existing logical-mode preflight below remains authoritative.
            dispatch_ops = fuse_gpu_transpose_ops(&dispatch_ops);
            if dispatch_ops.is_empty() {
                // A cancelled transpose run requires an independent result
                // but no device work, upload, or readback. Keep the public
                // operation count while explicitly reporting zero dispatches.
                crate::compute::record_pipeline_operation_path("native-copy");
                crate::compute::record_pipeline_dispatch_count(0);
                return Ok(img.clone());
            }
        }
        dispatch_ops = fuse_gpu_crop_border_ops(&dispatch_ops);
        // Normalize geometry wrappers before deriving operation-aligned
        // auxiliary inputs and preflight state.  Thumbnail can expand into a
        // Reduce + Resize pair, so postponing this step would leave those
        // vectors with different lengths.
        dispatch_ops = expand_gpu_geometry_ops(&dispatch_ops, img, img.dimensions(), mode);

        // A packed dispatch has one logical layout uniform. Operations such
        // as Grayscale, ExtractBand, and PutAlpha change that layout for the
        // following node, so they cannot share a dispatch with later work.
        // F resize chains likewise require materialized FLOAT32 words between
        // public resize calls: split the chain before deriving proof flags so
        // each segment inspects the preceding result rather than the original
        // source. Recursion preserves the same receipt aggregation below for
        // both mode changes and F resize boundaries.
        if gpu_uniform_blur_can_copy(&dispatch_ops, img) {
            // A normalized blur preserves every channel of a constant image,
            // including the edge samples. Replace only the GPU lowering with
            // an identity dispatch; the public operation remains represented
            // by the same exact result and the selected backend stays GPU.
            gpu_log!("[GPU] lowering constant blur to one identity dispatch");
            dispatch_ops = vec![PipelineOp::Duplicate];
        }
        // `putpixel` is queued lazily by Image. Fold one native LA pixel write
        // immediately before Sharpness into the already-required upload copy,
        // then let the dedicated shader process the resulting LA frame once.
        let native_sharpness_la_patch = gpu_native_sharpness_la_putpixel(&dispatch_ops, img, mode);
        if native_sharpness_la_patch.is_some() {
            dispatch_ops.remove(0);
        }
        let ops = dispatch_ops.as_slice();
        // Only a standalone L -> RGB Colorize can consume a four-samples-per-
        // word upload; preceding GPU kernels leave the ordinary RGBA transport.
        let packed_luma_colorize = matches!(ops, [PipelineOp::Colorize { .. }])
            && matches!(img, DynamicImage::ImageLuma8(_));
        // A standalone L -> RGBA conversion can read the native L bytes as a
        // packed four-samples-per-word source. Other targets and prefixes keep
        // the existing standard RGBA transport and shader contract.
        let packed_luma_convert = gpu_packed_luma_convert_input(ops, img, mode);
        let packed_luma_transform_input = gpu_packed_luma_transform_input(ops, img, mode);
        let native_rgb_compact_input_candidate =
            gpu_native_rgb_compact_input(ops, img, mode, host_autocontrast_rgb_lut);
        let native_la_transform_input = gpu_native_la_transform_input(ops, img, mode);
        let packed_luma_point = gpu_packed_luma_point_input(
            source_is_luma_point_run,
            host_autocontrast_luma_lut,
            ops,
            img,
            mode,
        );
        // Native L PutData can keep source and replacement bytes compact.
        // P/1 and mode-converted images retain their separate semantics.
        let packed_luma_putdata = cfg!(target_endian = "little")
            && matches!(mode, None | Some("L"))
            && img.width() != 0
            && img.height() != 0
            && matches!(
                ops,
                [PipelineOp::PutData {
                    data,
                    mode: PixelMode::L,
                }] if matches!(img, DynamicImage::ImageLuma8(source)
                    if data.len() <= source.as_raw().len())
            );
        let packed_luma_order_statistic = gpu_packed_luma_order_statistic_input(ops, img, mode);
        let packed_native_byte_filter = gpu_packed_native_byte_filter_input(ops, img, mode);
        let packed_luma_pad = gpu_packed_luma_pad_input(ops, img, mode);
        let packed_la_pad = gpu_packed_la_pad_input(ops, img, mode);
        let segment_boundary = gpu_first_nonterminal_mode_change(ops).or_else(|| {
            (mode == Some("F")
                && ops.len() > 1
                && ops.iter().all(|op| matches!(op, PipelineOp::Resize { .. })))
            .then_some(0)
        });
        if let Some(mode_index) = segment_boundary {
            let (prefix, suffix) = ops.split_at(mode_index + 1);
            let prefix_result =
                self.execute_batch_with_policy(prefix, img, mode, allow_cpu_fallback)?;
            // A recursive segment publishes its own receipt counters. Drain
            // them before executing the suffix so the outer benchmark sample
            // can retain one terminal aggregate rather than reporting only
            // the final segment's dispatch/resource counts.
            let prefix_override = crate::compute::take_pipeline_backend_override();
            let prefix_resource = crate::compute::take_pipeline_resource_telemetry();
            let prefix_dispatch = crate::compute::take_pipeline_dispatch_count().unwrap_or(0);
            if suffix.is_empty() {
                if let Some(prefix_override) = prefix_override {
                    crate::compute::restore_pipeline_backend_override(prefix_override);
                }
                if let Some(resource) = prefix_resource {
                    crate::compute::record_pipeline_resource_telemetry(resource);
                }
                crate::compute::record_pipeline_dispatch_count(prefix_dispatch);
                return Ok(prefix_result);
            }
            let next_mode = gpu_logical_mode_after_op(mode, &prefix[mode_index], &prefix_result);
            let suffix_result = self.execute_batch_with_policy(
                suffix,
                &prefix_result,
                next_mode.as_deref(),
                allow_cpu_fallback,
            )?;
            let suffix_override = crate::compute::take_pipeline_backend_override();
            let suffix_resource = crate::compute::take_pipeline_resource_telemetry();
            let suffix_dispatch = crate::compute::take_pipeline_dispatch_count().unwrap_or(0);
            // A host-controlled prefix is allowed to feed a native GPU
            // suffix, but its CPU override must not mislabel the terminal
            // executor. Preserve the diagnostic reason while reporting the
            // final segment's backend identity. If the suffix itself fell
            // back, its terminal CPU override wins unchanged.
            if let Some(suffix_override) = suffix_override {
                crate::compute::restore_pipeline_backend_override(suffix_override);
            } else if let Some((_, reason)) = prefix_override {
                crate::compute::restore_pipeline_backend_override((Backend::Gpu, reason));
            }
            let mut resource = None;
            crate::compute::merge_pipeline_resource_telemetry(&mut resource, prefix_resource);
            crate::compute::merge_pipeline_resource_telemetry(&mut resource, suffix_resource);
            if let Some(resource) = resource {
                crate::compute::record_pipeline_resource_telemetry(resource);
            }
            crate::compute::record_pipeline_dispatch_count(
                prefix_dispatch.saturating_add(suffix_dispatch),
            );
            return Ok(suffix_result);
        }

        let f_resize_constant_bits = gpu_f_resize_constant_bits(&dispatch_ops, img, mode);
        let f_resize_box_copy_is_exact = gpu_f_resize_box_copy_is_exact(&dispatch_ops, img, mode);
        let f_resize_identity_is_exact = gpu_f_resize_identity_is_exact(&dispatch_ops, img, mode);
        let f_resize_box_average_is_exact =
            gpu_f_resize_box_average_is_exact(&dispatch_ops, img, mode);
        let f_pad_f64_is_exact = gpu_f_pad_f64_is_exact(&dispatch_ops, img, mode);
        let f_resize_f64_ordered_shader_is_supported =
            gpu_f_resize_f64_ordered_shader_is_supported(&dispatch_ops, img, mode);
        // The static ordered-f64 contract covers every source bit pattern and
        // does not need the per-pixel marker-6 CPU simulation. Run that more
        // expensive input proof only when the ordered shader cannot accept
        // this geometry.
        let f_resize_dyadic_is_exact = !f_resize_f64_ordered_shader_is_supported
            && gpu_f_resize_dyadic_is_exact(&dispatch_ops, img, mode);
        // The bounded ordered-f64 shader implements Pillow's per-tap
        // accumulation for finite inputs, subnormals, and special values.
        // Use its static coefficient/geometry contract first; otherwise keep
        // the existing input-driven exact proofs for marker 9 and the
        // restricted marker-12 simulation.
        let f_resize_f64_is_exact = !f_resize_f64_ordered_shader_is_supported
            && (gpu_f_resize_f64_is_exact(&dispatch_ops, img, mode)
                || f_pad_f64_is_exact
                || gpu_luma16_resize_f64_is_exact(&dispatch_ops, img, mode)
                || gpu_i_resize_f64_is_exact(&dispatch_ops, img, mode));
        let f_resize_f64_ordered_proof = f_resize_f64_ordered_shader_is_supported
            || (!f_resize_f64_is_exact
                && gpu_f_resize_f64_ordered_is_exact(&dispatch_ops, img, mode));
        // Marker 12 is selected only when none of the earlier F proofs owns
        // the operation.  In particular, Box-average/dyadic markers consume
        // the fixed-point coefficient table, whereas marker 12 requires the
        // four-word f64 coefficient arena.
        let f_resize_f64_ordered_is_exact = f_resize_f64_ordered_proof
            && !f_resize_f64_is_exact
            && f_resize_constant_bits.is_none()
            && !f_resize_box_copy_is_exact
            && !f_resize_identity_is_exact
            && !f_resize_box_average_is_exact
            && !f_resize_dyadic_is_exact;

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

        // GPU shaders consume the packed L/LA/RGB/RGBA representation. P and
        // PA are safe for raw channel-wise Chops because the core exposes
        // their index/(index, alpha) samples as Luma8/LumaA8 buffers; no
        // palette expansion is involved. CMYK is additionally safe for a
        // brightness/color-saturation batch because the core stores its
        // C/M/Y/K bytes in the same four-byte transport and those shaders
        // explicitly process K. Grayscale has a dedicated shader branch that
        // fuses CMYK-to-RGB arithmetic with luma. EffectSpread is a separate
        // raw-byte relocation: its host-generated map is gathered as complete
        // packed words, so it is safe for every mode backed by the ordinary
        // byte transport, including the logical modes below.
        let logical_mode_supported = gpu_logical_mode_is_supported(
            ops,
            Some(img),
            mode,
            f_resize_constant_bits,
            f_pad_f64_is_exact,
            true,
        );
        if !logical_mode_supported {
            gpu_log!(
                "[GPU] dispatch preflight routed batch to exact host semantic control: logical layout"
            );
            return self.preflight_failure(
                ops,
                img,
                mode,
                allow_cpu_fallback,
                "GPU logical-mode operation contract is not proven",
            );
        }

        // A palette-index PutPixel has the same concrete Luma8 storage as an
        // ordinary L image, so the logical mode is the only distinction that
        // prevents a direct-core caller from accidentally treating the index
        // as a visible luma sample. PA uses the non-indexed flag and is
        // already represented by the LumaA8 layout check below.
        if ops.iter().any(|op| {
            matches!(
                op,
                PipelineOp::PutPixel {
                    palette_index: true,
                    ..
                }
            )
        }) && mode != Some("P")
        {
            return self.preflight_failure(
                ops,
                img,
                mode,
                allow_cpu_fallback,
                "palette index requires P logical mode",
            );
        }

        // The uniform mode word describes the source layout for the whole
        // dispatch batch. Convert, grayscale, and getchannel change that
        // layout; a later shader would otherwise interpret the new packed
        // pixels using the old source mode. Keep the pipeline lazy, but hand
        // mixed-mode batches to the universal sequential CPU executor until
        // GPU batch segmentation carries an updated mode between dispatches.
        if gpu_batch_has_nonterminal_mode_change(ops) {
            gpu_log!(
                "[GPU] dispatch preflight routed batch to CPU: mode-changing op is not terminal"
            );
            return self.preflight_failure(
                ops,
                img,
                mode,
                allow_cpu_fallback,
                "non-terminal mode change",
            );
        }

        if !gpu_image_layout_is_supported(img)
            && !gpu_luma16_geometry_is_supported(ops, img, mode)
            && !gpu_luma16_convert_is_supported(ops, img)
            && !gpu_luma16_paste_is_supported(ops, img)
        {
            gpu_log!(
                "[GPU] dispatch preflight routed batch to CPU: unsupported native pixel layout"
            );
            return self.preflight_failure(
                ops,
                img,
                mode,
                allow_cpu_fallback,
                "unsupported native pixel layout",
            );
        }

        for op in ops {
            let i_mode_filter = mode == Some("I")
                && matches!(
                    op,
                    PipelineOp::Filter3x3 { .. } | PipelineOp::Filter5x5 { .. }
                )
                && (gpu_int_filter_is_supported(ops, img)
                    || gpu_int_filter_resize_chain_is_supported(ops, img));
            if !registry::gpu_supports(op)? && !i_mode_filter {
                return self.preflight_failure(
                    ops,
                    img,
                    mode,
                    allow_cpu_fallback,
                    "no valid single-dispatch shader contract",
                );
            }
        }

        // Keep this guard at execution time as well as in `supports`: explicit
        // backend selection and future callers must not bypass shader bounds.
        if let Err(error) = validate_gpu_operations(ops, img, mode) {
            let reason = error.to_string();
            return self.preflight_failure(ops, img, mode, allow_cpu_fallback, &reason);
        }

        // Check the primary image and every declared output before resolving
        // nested images. An empty/oversized outer image must not initialize a
        // device merely because a later auxiliary pipeline is present; the
        // entire batch will be handled by the CPU fallback.
        let empty_two_input_noop = gpu_empty_two_input_batch_is_noop(ops, img);
        if gpu_dimensions_require_cpu(ops, img, mode) && !empty_two_input_noop {
            gpu_log!(
                "[GPU] dispatch preflight routed batch to CPU: unsafe primary image dimensions"
            );
            return self.preflight_failure(
                ops,
                img,
                mode,
                allow_cpu_fallback,
                "unsafe primary image dimensions",
            );
        }

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

        // Resolve every nested image before starting GPU work. A nested
        // explicitly locked pipeline may itself need the GPU pool, and Pillow
        // surfaces that materialization failure instead of dispatching the
        // outer shader with an empty auxiliary buffer.
        let auxiliary_images = {
            let mut dimensions = img.dimensions();
            let mut images = Vec::with_capacity(ops.len());
            // Non-drawing batches only need dimensions and auxiliary
            // handles. Borrow the source until drawing actually produces a
            // preview, avoiding a discarded full-frame clone on every batch.
            let mut draw_preview: Option<DynamicImage> = None;
            let mut draw_preview_valid = true;
            for op in ops {
                let rendered =
                    if draw_preview_valid && crate::compute::pool_cpu::ops::draw::is_draw_op(op) {
                        Some(crate::compute::pool_cpu::ops::draw::execute_draw_batch(
                            draw_preview.as_ref().unwrap_or(img),
                            std::slice::from_ref(op),
                            mode,
                        )?)
                    } else {
                        None
                    };
                images.push(extract_auxiliary_images(op, dimensions, rendered.as_ref())?);
                if let Some(rendered) = rendered {
                    draw_preview = Some(rendered);
                } else if !crate::compute::pool_cpu::ops::draw::is_draw_op(op) {
                    // Drawing after an arbitrary GPU operation needs the
                    // preceding native result as its geometry canvas. Keep
                    // the current bounded implementation explicit rather
                    // than rendering against stale source pixels.
                    draw_preview_valid = false;
                }
                if let Some(next) = op_output_dims(op, dimensions.0, dimensions.1) {
                    dimensions = next;
                }
            }
            images
        };

        if empty_two_input_noop {
            if !gpu_empty_two_input_inputs_are_safe(ops, img, &auxiliary_images) {
                gpu_log!(
                    "[GPU] dispatch preflight routed batch to CPU: unsafe or incomplete empty-image inputs"
                );
                return self.preflight_failure(
                    ops,
                    img,
                    mode,
                    allow_cpu_fallback,
                    "unsafe or incomplete empty-image inputs",
                );
            }
            // There are no invocations to submit. Record an explicit zero
            // dispatch so telemetry does not substitute the operation-count
            // estimate for this valid no-op path.
            crate::compute::record_pipeline_dispatch_count(0);
            return Ok(img.clone());
        }

        if gpu_pipeline_requires_cpu(ops, img, &auxiliary_images, mode) {
            gpu_log!(
                "[GPU] dispatch preflight routed batch to CPU: unsafe or incomplete image dimensions"
            );
            return self.preflight_failure(
                ops,
                img,
                mode,
                allow_cpu_fallback,
                "unsafe or incomplete image dimensions",
            );
        }

        // Contrast's midpoint belongs to the current source image. The
        // batch uniform block carries one midpoint, so compute it before
        // upload. A single exact PutPixel prefix can be mirrored without
        // publishing a second backend receipt; all other current-image
        // sequences retain the exact host-control path.
        let contrast_mean = if ops
            .iter()
            .any(|op| matches!(op, PipelineOp::Contrast { .. }))
        {
            match gpu_contrast_mean_after_exact_prefix(img, ops, mode) {
                Some(mean) => Some(mean),
                None => {
                    let reason = if ops
                        .iter()
                        .position(|op| matches!(op, PipelineOp::Contrast { .. }))
                        == Some(0)
                    {
                        "Contrast midpoint is unavailable for this image layout"
                    } else {
                        "Contrast midpoint requires the current host image"
                    };
                    return self.preflight_failure(ops, img, mode, allow_cpu_fallback, reason);
                }
            }
        } else {
            None
        };

        let gpu = Self::ensure_init()?;
        gpu.ensure_healthy("GPU batch start")?;
        let packed_luma_point = packed_luma_point
            && plan_packed_point_luma_dispatch(
                img.width(),
                img.height(),
                gpu.device.limits().max_compute_workgroups_per_dimension,
            )
            .is_ok();
        let packed_luma_putdata = packed_luma_putdata
            && plan_packed_luma_dispatch(
                img.width(),
                img.height(),
                gpu.device.limits().max_compute_workgroups_per_dimension,
            )
            .is_ok();
        let packed_luma_order_statistic_requested = packed_luma_order_statistic;
        let packed_luma_order_statistic = packed_luma_order_statistic_requested
            && plan_packed_luma_dispatch(
                img.width(),
                img.height(),
                gpu.device.limits().max_compute_workgroups_per_dimension,
            )
            .is_ok();
        let packed_native_byte_filter_requested = packed_native_byte_filter;
        let packed_native_byte_filter = packed_native_byte_filter_requested
            && if matches!(mode, Some("LA")) {
                plan_packed_la_dispatch(
                    img.width(),
                    img.height(),
                    gpu.device.limits().max_compute_workgroups_per_dimension,
                )
                .is_ok()
            } else {
                plan_packed_luma_dispatch(
                    img.width(),
                    img.height(),
                    gpu.device.limits().max_compute_workgroups_per_dimension,
                )
                .is_ok()
            };
        let packed_luma_pad_requested = packed_luma_pad;
        let packed_luma_pad = packed_luma_pad_requested
            && matches!(ops, [op @ PipelineOp::Pad { .. }]
                if gpu_packed_luma_pad_dispatch_supported(
                    op,
                    img.dimensions(),
                    gpu.device.limits().max_compute_workgroups_per_dimension,
                )
            );
        if packed_luma_pad_requested && !packed_luma_pad {
            gpu_log!(
                "[GPU] dispatch preflight routed packed-L Pad to CPU: adapter workgroup limit"
            );
            return self.preflight_failure(
                ops,
                img,
                mode,
                allow_cpu_fallback,
                "adapter workgroup limit",
            );
        }
        let packed_la_pad_requested = packed_la_pad;
        let packed_la_pad = packed_la_pad_requested
            && matches!(ops, [op @ PipelineOp::Pad { .. }]
                if gpu_packed_la_pad_dispatch_supported(
                    op,
                    img.dimensions(),
                    gpu.device.limits().max_compute_workgroups_per_dimension,
                )
            );
        if packed_la_pad_requested && !packed_la_pad {
            gpu_log!(
                "[GPU] dispatch preflight routed packed-LA Pad to CPU: adapter workgroup limit"
            );
            return self.preflight_failure(
                ops,
                img,
                mode,
                allow_cpu_fallback,
                "adapter workgroup limit",
            );
        }
        if packed_luma_order_statistic_requested && !packed_luma_order_statistic {
            gpu_log!(
                "[GPU] dispatch preflight routed packed L order statistic to CPU: adapter workgroup limit"
            );
            return self.preflight_failure(
                ops,
                img,
                mode,
                allow_cpu_fallback,
                "adapter workgroup limit",
            );
        }
        if packed_native_byte_filter_requested && !packed_native_byte_filter {
            gpu_log!(
                "[GPU] dispatch preflight routed packed native blur to CPU: adapter workgroup limit"
            );
            return self.preflight_failure(
                ops,
                img,
                mode,
                allow_cpu_fallback,
                "adapter workgroup limit",
            );
        }
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
        let native_hsv_pad: Option<NativeHsvPadDispatch> = None;

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
        if let ([op], [auxiliary]) = (ops, auxiliary_images.as_slice())
            && let Some(source_pixels) = auxiliary.second.as_deref()
            && let Some(offset) = gpu_native_rgb_to_rgba_paste_layout(op, img, source_pixels, mode)
            && let Some(dispatch) = plan_gpu_native_rgb_to_rgba_paste(
                img.width(),
                img.height(),
                source_pixels.width(),
                source_pixels.height(),
                limits.max_compute_workgroups_per_dimension,
                limits.max_storage_buffer_binding_size,
                limits.max_buffer_size,
            )
        {
            let mut buffers = gpu.acquire_buffers(dispatch.destination_pixels)?;
            let result = gpu.execute_native_rgb_to_rgba_paste(
                img,
                source_pixels,
                offset,
                dispatch,
                &mut buffers,
            )?;
            gpu.recycle_buffers(buffers);
            return Ok(result);
        }

        #[cfg(target_endian = "little")]
        if let ([op], [auxiliary]) = (ops, auxiliary_images.as_slice())
            && let Some(source_pixels) = auxiliary.second.as_deref()
            && let Some((offset, bytes_per_pixel)) =
                gpu_native_byte_paste_layout(op, img, source_pixels, mode)
            && let Some(dispatch) = plan_gpu_native_byte_paste(
                img.width(),
                img.height(),
                source_pixels.width(),
                source_pixels.height(),
                bytes_per_pixel,
                limits.max_compute_workgroups_per_dimension,
                limits.max_storage_buffer_binding_size,
                limits.max_buffer_size,
            )
        {
            let mut buffers = gpu.acquire_buffers(dispatch.word_count)?;
            let result =
                gpu.execute_native_byte_paste(img, source_pixels, offset, dispatch, &mut buffers)?;
            gpu.recycle_buffers(buffers);
            return Ok(result);
        }

        #[cfg(target_endian = "little")]
        if let (
            [
                PipelineOp::CompositeModule {
                    other,
                    mask,
                    mask_alpha,
                },
            ],
            [auxiliary],
        ) = (ops, auxiliary_images.as_slice())
            && let (Some(other_image), Some(mask_image)) =
                (auxiliary.second.as_deref(), auxiliary.third.as_deref())
            && let Some((channels, mask_channels, mask_channel)) = gpu_native_composite_layout(
                img,
                mode,
                other,
                other_image,
                mask,
                mask_image,
                *mask_alpha,
            )
        {
            let bytes = CheckedDims::new(img.width(), img.height(), channels)?.total_bytes();
            let words = u32::try_from(bytes.div_ceil(4))
                .map_err(|_| PilError::ValueError("GPU native composite is too large".into()))?;
            let mut buffers = gpu.acquire_buffers(words)?;
            let result = gpu.execute_native_composite(
                img,
                other_image,
                mask_image,
                channels,
                mask_channels,
                mask_channel,
                &mut buffers,
            )?;
            gpu.recycle_buffers(buffers);
            return Ok(result);
        }

        #[cfg(target_endian = "little")]
        if let (
            [
                PipelineOp::PutAlphaData {
                    mode: PixelMode::RGB,
                    ..
                },
            ],
            [auxiliary],
        ) = (ops, auxiliary_images.as_slice())
            && matches!(img, DynamicImage::ImageRgb8(_))
            && matches!(mode, None | Some("RGB"))
            && let Some(alpha_mask) = auxiliary.third.as_deref()
            && matches!(alpha_mask, DynamicImage::ImageLuma8(_))
            && alpha_mask.dimensions() == img.dimensions()
            && let Some(dispatch) = plan_native_rgb_put_alpha_data_dispatch(
                img.width(),
                img.height(),
                limits.max_compute_workgroups_per_dimension,
                limits.max_storage_buffer_binding_size,
                limits.max_buffer_size,
            )
        {
            let pixels = CheckedDims::new(img.width(), img.height(), 1)?.total_pixels();
            let capacity = u32::try_from(pixels).map_err(|_| {
                PilError::ValueError("GPU native RGB PutAlphaData image is too large".into())
            })?;
            let mut buffers = gpu.acquire_buffers(capacity)?;
            let result =
                gpu.execute_native_rgb_put_alpha_data(img, alpha_mask, dispatch, &mut buffers)?;
            gpu.recycle_buffers(buffers);
            return Ok(result);
        }

        #[cfg(target_endian = "little")]
        if let (
            [
                PipelineOp::PutAlphaData {
                    mode: PixelMode::RGBA,
                    ..
                },
            ],
            [auxiliary],
        ) = (ops, auxiliary_images.as_slice())
            && matches!(img, DynamicImage::ImageRgba8(_))
            && matches!(mode, None | Some("RGBA"))
            && let Some(alpha_mask) = auxiliary.third.as_deref()
            && matches!(alpha_mask, DynamicImage::ImageLuma8(_))
            && alpha_mask.dimensions() == img.dimensions()
            && let Some(dispatch) = plan_native_rgba_put_alpha_data_dispatch(
                img.width(),
                img.height(),
                limits.max_compute_workgroups_per_dimension,
                limits.max_storage_buffer_binding_size,
                limits.max_buffer_size,
            )
        {
            let pixels = CheckedDims::new(img.width(), img.height(), 1)?.total_pixels();
            let capacity = u32::try_from(pixels).map_err(|_| {
                PilError::ValueError("GPU native RGBA PutAlphaData image is too large".into())
            })?;
            let mut buffers = gpu.acquire_buffers(capacity)?;
            let result =
                gpu.execute_native_rgba_put_alpha_data(img, alpha_mask, dispatch, &mut buffers)?;
            gpu.recycle_buffers(buffers);
            return Ok(result);
        }

        #[cfg(target_endian = "little")]
        if let [
            PipelineOp::PutAlpha {
                alpha,
                mode: PixelMode::RGB,
            },
        ] = ops
            && matches!(img, DynamicImage::ImageRgb8(_))
            && matches!(mode, None | Some("RGB"))
            && let Some(dispatch) = plan_native_rgb_put_alpha_dispatch(
                img.width(),
                img.height(),
                limits.max_compute_workgroups_per_dimension,
                limits.max_storage_buffer_binding_size,
                limits.max_buffer_size,
            )
        {
            let pixels = CheckedDims::new(img.width(), img.height(), 1)?.total_pixels();
            let capacity = u32::try_from(pixels).map_err(|_| {
                PilError::ValueError("GPU native RGB PutAlpha image is too large".into())
            })?;
            let mut buffers = gpu.acquire_buffers(capacity)?;
            let result = gpu.execute_native_rgb_put_alpha(img, *alpha, dispatch, &mut buffers)?;
            gpu.recycle_buffers(buffers);
            return Ok(result);
        }

        #[cfg(target_endian = "little")]
        let native_byte_op = match (ops, auxiliary_images.as_slice()) {
            ([op], [auxiliary]) => Some((op, auxiliary, false)),
            ([op, _], [auxiliary, _]) if can_fuse_gpu_multiply_screen(ops, 0) => {
                Some((op, auxiliary, true))
            }
            _ => None,
        };
        #[cfg(target_endian = "little")]
        if let Some((op, auxiliary, fused_screen)) = native_byte_op
            && let Some(channels) = gpu_native_byte_op_channels(op, img, mode)
            && auxiliary.second.as_deref().is_none_or(|other| {
                img.color() == other.color() && img.dimensions() == other.dimensions()
            })
        {
            // Public/adapter guards have run. Keep native sample transport for
            // one admitted operation or the exact shared-secondary blend pair.
            let bytes = CheckedDims::new(img.width(), img.height(), channels)?.total_bytes();
            let words = u32::try_from(bytes.div_ceil(4)).map_err(|_| {
                PilError::ValueError("GPU native binary operation is too large".into())
            })?;
            let mut buffers = gpu.acquire_buffers(words)?;
            let result = gpu.execute_native_byte_op(
                op,
                img,
                auxiliary.second.as_deref(),
                channels,
                mode,
                fused_screen,
                &mut buffers,
            )?;
            gpu.recycle_buffers(buffers);
            return Ok(result);
        }

        #[cfg(target_endian = "little")]
        if let Some(format) = gpu_native_flip_format(ops, img, mode) {
            match format {
                NativeFlipFormat::Mode1 => {
                    let Some(dispatch) = plan_native_byte_flip_dispatch(
                        img.width(),
                        img.height(),
                        1,
                        limits.max_compute_workgroups_per_dimension,
                        limits.max_storage_buffer_binding_size,
                        limits.max_buffer_size,
                        GPU_BUFFER_CAPACITY,
                    ) else {
                        return self.preflight_failure(
                            ops,
                            img,
                            mode,
                            allow_cpu_fallback,
                            "native byte Flip exceeds adapter or shader limits",
                        );
                    };
                    let mut buffers = gpu.acquire_buffers(dispatch.word_count)?;
                    let result =
                        gpu.execute_native_byte_flip(img, format, dispatch, &mut buffers)?;
                    gpu.recycle_buffers(buffers);
                    return Ok(result);
                }
                NativeFlipFormat::L => {
                    let Some(dispatch) = plan_native_byte_flip_dispatch(
                        img.width(),
                        img.height(),
                        1,
                        limits.max_compute_workgroups_per_dimension,
                        limits.max_storage_buffer_binding_size,
                        limits.max_buffer_size,
                        GPU_BUFFER_CAPACITY,
                    ) else {
                        return self.preflight_failure(
                            ops,
                            img,
                            mode,
                            allow_cpu_fallback,
                            "native L Flip exceeds adapter or shader limits",
                        );
                    };
                    let mut buffers = gpu.acquire_buffers(dispatch.word_count)?;
                    let result =
                        gpu.execute_native_byte_flip(img, format, dispatch, &mut buffers)?;
                    gpu.recycle_buffers(buffers);
                    return Ok(result);
                }
                NativeFlipFormat::Palette => {
                    let Some(dispatch) = plan_native_byte_flip_dispatch(
                        img.width(),
                        img.height(),
                        1,
                        limits.max_compute_workgroups_per_dimension,
                        limits.max_storage_buffer_binding_size,
                        limits.max_buffer_size,
                        GPU_BUFFER_CAPACITY,
                    ) else {
                        return self.preflight_failure(
                            ops,
                            img,
                            mode,
                            allow_cpu_fallback,
                            "native palette-index Flip exceeds adapter or shader limits",
                        );
                    };
                    let mut buffers = gpu.acquire_buffers(dispatch.word_count)?;
                    let result =
                        gpu.execute_native_byte_flip(img, format, dispatch, &mut buffers)?;
                    gpu.recycle_buffers(buffers);
                    return Ok(result);
                }
                NativeFlipFormat::LA => {
                    let Some(dispatch) = plan_native_byte_flip_dispatch(
                        img.width(),
                        img.height(),
                        2,
                        limits.max_compute_workgroups_per_dimension,
                        limits.max_storage_buffer_binding_size,
                        limits.max_buffer_size,
                        GPU_BUFFER_CAPACITY,
                    ) else {
                        return self.preflight_failure(
                            ops,
                            img,
                            mode,
                            allow_cpu_fallback,
                            "native LA Flip exceeds adapter or shader limits",
                        );
                    };
                    let mut buffers = gpu.acquire_buffers(dispatch.word_count)?;
                    let result =
                        gpu.execute_native_byte_flip(img, format, dispatch, &mut buffers)?;
                    gpu.recycle_buffers(buffers);
                    return Ok(result);
                }
                NativeFlipFormat::PA => {
                    let Some(dispatch) = plan_native_byte_flip_dispatch(
                        img.width(),
                        img.height(),
                        2,
                        limits.max_compute_workgroups_per_dimension,
                        limits.max_storage_buffer_binding_size,
                        limits.max_buffer_size,
                        GPU_BUFFER_CAPACITY,
                    ) else {
                        return self.preflight_failure(
                            ops,
                            img,
                            mode,
                            allow_cpu_fallback,
                            "native PA Flip exceeds adapter or shader limits",
                        );
                    };
                    let mut buffers = gpu.acquire_buffers(dispatch.word_count)?;
                    let result =
                        gpu.execute_native_byte_flip(img, format, dispatch, &mut buffers)?;
                    gpu.recycle_buffers(buffers);
                    return Ok(result);
                }
                NativeFlipFormat::RGB => {
                    let DynamicImage::ImageRgb8(rgb) = img else {
                        return Err(PilError::InternalError(
                            "native RGB Flip admission did not match RGB storage".into(),
                        ));
                    };
                    let Some(layout) = packed_rgb_transpose_layout(
                        rgb.width(),
                        rgb.height(),
                        limits.max_compute_workgroups_per_dimension,
                    ) else {
                        return self.preflight_failure(
                            ops,
                            img,
                            mode,
                            allow_cpu_fallback,
                            "native RGB Flip exceeds adapter or shader limits",
                        );
                    };
                    let words = u32::try_from(layout.transfer_bytes / 4).map_err(|_| {
                        PilError::ValueError("GPU packed RGB Flip is too large".into())
                    })?;
                    let mut buffers = gpu.acquire_buffers(words)?;
                    let geometry_as_transpose = PipelineOp::Transpose {
                        method: TransposeMethod::FlipTopBottom,
                    };
                    let result = gpu.execute_packed_rgb_transpose(
                        &geometry_as_transpose,
                        rgb,
                        &layout,
                        &mut buffers,
                    )?;
                    gpu.recycle_buffers(buffers);
                    return Ok(result);
                }
                NativeFlipFormat::RGBA => {
                    let DynamicImage::ImageRgba8(rgba) = img else {
                        return Err(PilError::InternalError(
                            "native RGBA Flip admission did not match RGBA storage".into(),
                        ));
                    };
                    let dimensions = CheckedDims::new(img.width(), img.height(), 4)?;
                    let transfer_bytes = u64::try_from(dimensions.total_bytes()).map_err(|_| {
                        PilError::ValueError("GPU native RGBA Flip is too large".into())
                    })?;
                    let workgroup_count = u64::try_from(capacity)
                        .ok()
                        .and_then(|words| words.checked_mul(4))
                        .ok_or_else(|| {
                            PilError::ValueError("GPU native RGBA Flip is too large".into())
                        })?;
                    let groups_x = img.width().div_ceil(16);
                    let groups_y = img.height().div_ceil(16);
                    if transfer_bytes > u64::from(limits.max_storage_buffer_binding_size)
                        || transfer_bytes > limits.max_buffer_size
                        || transfer_bytes > workgroup_count
                        || groups_x > limits.max_compute_workgroups_per_dimension
                        || groups_y > limits.max_compute_workgroups_per_dimension
                    {
                        return self.preflight_failure(
                            ops,
                            img,
                            mode,
                            allow_cpu_fallback,
                            "native RGBA Flip exceeds adapter or shader limits",
                        );
                    }
                    let capacity_words = u32::try_from(capacity).map_err(|_| {
                        PilError::ValueError("GPU native RGBA Flip is too large".into())
                    })?;
                    let mut buffers = gpu.acquire_buffers(capacity_words)?;
                    let geometry_as_transpose = PipelineOp::Transpose {
                        method: TransposeMethod::FlipTopBottom,
                    };
                    let result = gpu.execute_rgba_transpose_with_upload(
                        &geometry_as_transpose,
                        rgba,
                        &mut buffers,
                        gpu.direct_primary_readback,
                    )?;
                    gpu.recycle_buffers(buffers);
                    return Ok(result);
                }
            }
        }

        #[cfg(target_endian = "little")]
        if matches!(mode, None | Some("RGB"))
            && matches!(ops, [PipelineOp::Mirror])
            && let DynamicImage::ImageRgb8(rgb) = img
            && let Some(layout) = packed_rgb_transpose_layout(
                rgb.width(),
                rgb.height(),
                limits.max_compute_workgroups_per_dimension,
            )
        {
            // Mirror is the horizontal RGB transpose method. Keep its packed
            // three-byte input and output instead of expanding RGB to RGBA.
            let geometry_as_transpose = PipelineOp::Transpose {
                method: TransposeMethod::FlipLeftRight,
            };
            let words = u32::try_from(layout.transfer_bytes / 4)
                .map_err(|_| PilError::ValueError("GPU packed RGB mirror is too large".into()))?;
            let mut buffers = gpu.acquire_buffers(words)?;
            let result = gpu.execute_packed_rgb_transpose(
                &geometry_as_transpose,
                rgb,
                &layout,
                &mut buffers,
            )?;
            gpu.recycle_buffers(buffers);
            return Ok(result);
        }

        #[cfg(target_endian = "little")]
        if matches!(mode, None | Some("RGB" | "HSV"))
            && let ([op @ PipelineOp::Transpose { .. }], DynamicImage::ImageRgb8(rgb)) = (ops, img)
            && let Some(layout) = packed_rgb_transpose_layout(
                rgb.width(),
                rgb.height(),
                limits.max_compute_workgroups_per_dimension,
            )
        {
            // Fusion and ordinary preflight have already validated the full
            // batch. RGB and HSV share physical RGB8 triplets here; the
            // singleton transpose only relocates bytes, while mixed
            // operations keep the ordinary packed RGBA path.
            let mut buffers = gpu.acquire_buffers(capacity)?;
            let result = gpu.execute_packed_rgb_transpose(op, rgb, &layout, &mut buffers)?;
            gpu.recycle_buffers(buffers);
            return Ok(result);
        }

        #[cfg(target_endian = "little")]
        if mapped_rgba_transpose_is_supported(ops, img, mode, gpu.direct_primary_readback)
            && let ([op], DynamicImage::ImageRgba8(rgba)) = (ops, img)
        {
            // Logical mode matters: RGBX, premultiplied samples, and packed
            // I/F storage also use four-byte containers, but keep their
            // existing mode-specific execution paths.
            let mut buffers = gpu.acquire_buffers(capacity)?;
            let result = gpu.execute_rgba_transpose_with_upload(op, rgba, &mut buffers, true)?;
            gpu.recycle_buffers(buffers);
            return Ok(result);
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
        if output_only_constant {
            // A standalone Constant generates every output sample from its
            // parameter; the shader does not consume the source pixels.
        } else if native_luma16 {
            let DynamicImage::ImageLuma16(image) = img else {
                return Err(PilError::InternalError(
                    "GPU typed layout was admitted without an ImageLuma16 source".into(),
                ));
            };
            buffers.upload_luma16(&gpu.queue, image, mode)?;
        } else if native_luma16_convert || native_luma16_paste {
            let DynamicImage::ImageLuma16(image) = img else {
                return Err(PilError::InternalError(
                    "GPU typed operation was admitted without an ImageLuma16 source".into(),
                ));
            };
            buffers.upload_luma16_numeric(&gpu.queue, image)?;
        } else if full_luma_putdata {
            // A complete replacement has no observable source pixels. The
            // packed shader initializes every valid destination sample.
        } else if packed_la_pad {
            buffers.upload_native_channel_bytes(&gpu.queue, w, h, 2, img.as_bytes())?;
        } else if packed_native_byte_filter && matches!(mode, Some("LA")) {
            buffers.upload_native_channel_bytes(&gpu.queue, w, h, 2, img.as_bytes())?;
        } else if packed_luma_colorize
            || packed_luma_convert
            || packed_luma_transform_input
            || packed_luma_point
            || packed_luma_putdata
            || packed_luma_order_statistic
            || packed_native_byte_filter
            || packed_luma_pad
        {
            let DynamicImage::ImageLuma8(image) = img else {
                return Err(PilError::InternalError(
                    "packed-luma GPU input requires native L samples".into(),
                ));
            };
            if mapped_native_box_blur_input {
                gpu.upload_packed_luma8_mapped(image, &mut buffers)?;
            } else {
                buffers.upload_packed_luma8(&gpu.queue, image)?;
            }
        } else if native_la_transform_output {
            buffers.upload_native_channel_bytes(&gpu.queue, w, h, 2, img.as_bytes())?;
        } else if native_rgb_compact_input {
            if mapped_native_box_blur_input {
                let transfer_bytes = compact_native_channel_transfer_bytes(w, h, 3)?;
                gpu.upload_native_bytes_mapped(
                    img.as_bytes(),
                    img.as_bytes().len(),
                    transfer_bytes,
                    &mut buffers,
                )?;
            } else {
                buffers.upload_native_channel_bytes(&gpu.queue, w, h, 3, img.as_bytes())?;
            }
        } else if let Some(channels) = native_extract_band_channels {
            buffers.upload_native_channel_bytes(&gpu.queue, w, h, channels, img.as_bytes())?;
        } else if native_grayscale_rgb_input {
            if mapped_native_grayscale_input {
                let transfer_bytes = compact_native_channel_transfer_bytes(w, h, 3)?;
                gpu.upload_native_bytes_mapped(
                    img.as_bytes(),
                    img.as_bytes().len(),
                    transfer_bytes,
                    &mut buffers,
                )?;
            } else {
                buffers.upload_native_channel_bytes(&gpu.queue, w, h, 3, img.as_bytes())?;
            }
        } else if native_grayscale_cmyk_input {
            if mapped_native_grayscale_input {
                let transfer_bytes = compact_native_channel_transfer_bytes(w, h, 4)?;
                gpu.upload_native_bytes_mapped(
                    img.as_bytes(),
                    img.as_bytes().len(),
                    transfer_bytes,
                    &mut buffers,
                )?;
            } else {
                buffers.upload_standard_image(&gpu.queue, img)?;
            }
        } else if native_sharpness_l_input {
            let DynamicImage::ImageLuma8(image) = img else {
                return Err(PilError::InternalError(
                    "native-L Sharpness input requires an ImageLuma8 source".into(),
                ));
            };
            buffers.upload_packed_luma8(&gpu.queue, image)?;
        } else if native_sharpness_la_input {
            buffers.upload_native_channel_bytes_with_la_patch(
                &gpu.queue,
                w,
                h,
                2,
                img.as_bytes(),
                native_sharpness_la_patch,
            )?;
        } else if native_sharpness_rgb_input {
            buffers.upload_native_channel_bytes(&gpu.queue, w, h, 3, img.as_bytes())?;
        } else if native_reduce_rgb_input {
            buffers.upload_native_channel_bytes(&gpu.queue, w, h, 3, img.as_bytes())?;
        } else if let Some(channels) = native_expand_channels {
            buffers.upload_native_channel_bytes(&gpu.queue, w, h, channels, img.as_bytes())?;
        } else if let Some(channels) = native_cover_input_channels {
            buffers.upload_native_channel_bytes(&gpu.queue, w, h, channels, img.as_bytes())?;
        } else {
            buffers.upload_standard_image(&gpu.queue, img)?;
        }
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
        // Track the last mode-changing operation. Geometry and other
        // mode-preserving operations after it do not undo the promotion.
        let mut put_alpha_mode = None;
        let mut out_mode = None;
        for op in ops {
            match op {
                PipelineOp::Grayscale | PipelineOp::ExtractBand { .. } => {
                    put_alpha_mode = None;
                    out_mode = Some(crate::raster::ColorType::L8);
                }
                PipelineOp::Constant { .. } => {
                    put_alpha_mode = None;
                    out_mode = Some(crate::raster::ColorType::L8);
                }
                PipelineOp::Convert { mode, .. } => {
                    put_alpha_mode = None;
                    out_mode = gpu_output_color_type(mode);
                }
                PipelineOp::ConvertLab { .. } => {
                    put_alpha_mode = None;
                    out_mode = Some(crate::raster::ColorType::Rgb8);
                }
                PipelineOp::Colorize { .. } => {
                    put_alpha_mode = None;
                    out_mode = Some(crate::raster::ColorType::Rgb8);
                }
                PipelineOp::Merge { mode, .. } => {
                    put_alpha_mode = None;
                    out_mode = gpu_output_color_type(mode);
                }
                PipelineOp::Color3DLut { target_mode, .. } => {
                    put_alpha_mode = None;
                    out_mode = gpu_pixel_mode_color_type(*target_mode);
                }
                PipelineOp::EffectNoise { .. } => {
                    put_alpha_mode = None;
                    out_mode = Some(crate::raster::ColorType::L8);
                }
                PipelineOp::PutAlpha { mode, .. } => {
                    put_alpha_mode = Some(*mode);
                    out_mode = None;
                }
                PipelineOp::PutAlphaData { mode, .. } => {
                    put_alpha_mode = Some(*mode);
                    out_mode = None;
                }
                _ => {}
            }
        }
        let native_rgb = put_alpha_mode.is_none()
            && out_mode.unwrap_or_else(|| img.color()) == crate::raster::ColorType::Rgb8;
        let native_luma8_extract = out_mode == Some(crate::raster::ColorType::L8)
            && matches!(ops.last(), Some(PipelineOp::ExtractBand { .. }));
        let native_luma8_grayscale = out_mode == Some(crate::raster::ColorType::L8)
            && matches!(ops.last(), Some(PipelineOp::Grayscale));
        let native_luma8_constant = out_mode == Some(crate::raster::ColorType::L8)
            && matches!(ops.last(), Some(PipelineOp::Constant { .. }));
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
        } else if native_luma8_extract
            || native_luma8_grayscale
            || native_luma8_constant
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
                gpu.readback_to_image(final_w, final_h, readback_buffer, native_rgb)?
            }
        };
        gpu_log!("[GPU] step=readback done");
        // map_async completion proves the command buffer no longer uses the
        // working images. Return successful working sets to the bounded pool;
        // every error path drops its buffers instead of risking reuse of an
        // in-flight or device-invalid resource.
        resource_telemetry.upload_bytes = if output_only_constant || full_luma_putdata {
            0
        } else if let Some(channels) = native_extract_band_channels {
            let raw_bytes = CheckedDims::new(w, h, channels)?.total_bytes();
            let transfer_bytes = raw_bytes
                .div_ceil(std::mem::size_of::<u32>())
                .checked_mul(std::mem::size_of::<u32>())
                .ok_or_else(|| PilError::ValueError("GPU channel input is too large".into()))?;
            u64::try_from(transfer_bytes)
                .map_err(|_| PilError::ValueError("GPU channel input is too large".into()))?
        } else if native_grayscale_rgb_input || native_rgb_compact_input {
            let raw_bytes = CheckedDims::new(w, h, 3)?.total_bytes();
            let transfer_bytes = raw_bytes
                .div_ceil(std::mem::size_of::<u32>())
                .checked_mul(std::mem::size_of::<u32>())
                .ok_or_else(|| PilError::ValueError("GPU native RGB input is too large".into()))?;
            u64::try_from(transfer_bytes)
                .map_err(|_| PilError::ValueError("GPU native RGB input is too large".into()))?
        } else if native_sharpness_l_input {
            compact_luma8_transfer_bytes(w, h)?
        } else if packed_native_byte_filter && matches!(mode, Some("LA")) {
            let raw_bytes = CheckedDims::new(w, h, 2)?.total_bytes();
            u64::try_from(
                raw_bytes
                    .div_ceil(std::mem::size_of::<u32>())
                    .checked_mul(std::mem::size_of::<u32>())
                    .ok_or_else(|| {
                        PilError::ValueError("GPU native LA input is too large".into())
                    })?,
            )
            .map_err(|_| PilError::ValueError("GPU native LA input is too large".into()))?
        } else if packed_native_byte_filter {
            compact_luma8_transfer_bytes(w, h)?
        } else if packed_la_pad {
            compact_native_channel_transfer_bytes(w, h, 2)?
        } else if packed_luma_pad {
            compact_luma8_transfer_bytes(w, h)?
        } else if native_sharpness_la_input {
            let raw_bytes = CheckedDims::new(w, h, 2)?.total_bytes();
            let transfer_bytes = raw_bytes
                .div_ceil(std::mem::size_of::<u32>())
                .checked_mul(std::mem::size_of::<u32>())
                .ok_or_else(|| PilError::ValueError("GPU native LA input is too large".into()))?;
            u64::try_from(transfer_bytes)
                .map_err(|_| PilError::ValueError("GPU native LA input is too large".into()))?
        } else if native_la_transform_output {
            let raw_bytes = CheckedDims::new(w, h, 2)?.total_bytes();
            let transfer_bytes = raw_bytes
                .div_ceil(std::mem::size_of::<u32>())
                .checked_mul(std::mem::size_of::<u32>())
                .ok_or_else(|| PilError::ValueError("GPU native LA input is too large".into()))?;
            u64::try_from(transfer_bytes)
                .map_err(|_| PilError::ValueError("GPU native LA input is too large".into()))?
        } else if native_sharpness_rgb_input || native_reduce_rgb_input {
            let raw_bytes = CheckedDims::new(w, h, 3)?.total_bytes();
            let transfer_bytes = raw_bytes
                .div_ceil(std::mem::size_of::<u32>())
                .checked_mul(std::mem::size_of::<u32>())
                .ok_or_else(|| PilError::ValueError("GPU native RGB input is too large".into()))?;
            u64::try_from(transfer_bytes)
                .map_err(|_| PilError::ValueError("GPU native RGB input is too large".into()))?
        } else if let Some(channels) = native_expand_channels {
            let raw_bytes = CheckedDims::new(w, h, channels)?.total_bytes();
            let transfer_bytes = raw_bytes
                .div_ceil(std::mem::size_of::<u32>())
                .checked_mul(std::mem::size_of::<u32>())
                .ok_or_else(|| PilError::ValueError("GPU Expand input is too large".into()))?;
            u64::try_from(transfer_bytes)
                .map_err(|_| PilError::ValueError("GPU Expand input is too large".into()))?
        } else if let Some(channels) = native_cover_input_channels {
            let raw_bytes = CheckedDims::new(w, h, channels)?.total_bytes();
            let transfer_bytes = raw_bytes
                .div_ceil(std::mem::size_of::<u32>())
                .checked_mul(std::mem::size_of::<u32>())
                .ok_or_else(|| PilError::ValueError("GPU Cover input is too large".into()))?;
            u64::try_from(transfer_bytes)
                .map_err(|_| PilError::ValueError("GPU Cover input is too large".into()))?
        } else if packed_luma_colorize
            || packed_luma_convert
            || packed_luma_transform_input
            || packed_luma_point
            || packed_luma_putdata
            || packed_luma_order_statistic
            || packed_native_byte_filter
            || packed_luma_pad
            || native_sharpness_l_input
        {
            compact_luma8_transfer_bytes(w, h)?
        } else {
            CheckedDims::new(w, h, 4)?.total_bytes() as u64
        };
        resource_telemetry.readback_bytes = if let Some(dispatch) = native_cover_packed_output {
            dispatch.transfer_bytes
        } else if native_rgb_packed_output {
            u64::try_from(CheckedDims::new(final_w, final_h, 3)?.total_bytes())
                .map_err(|_| PilError::ValueError("GPU native RGB readback is too large".into()))?
                .div_ceil(4)
                .checked_mul(4)
                .ok_or_else(|| {
                    PilError::ValueError("GPU native RGB readback size overflow".into())
                })?
        } else if packed_la_pad || native_la_transform_output {
            u64::try_from(CheckedDims::new(final_w, final_h, 2)?.total_bytes())
                .map_err(|_| PilError::ValueError("GPU native LA readback is too large".into()))?
                .div_ceil(4)
                .checked_mul(4)
                .ok_or_else(|| {
                    PilError::ValueError("GPU native LA readback size overflow".into())
                })?
        } else if packed_native_byte_filter && matches!(mode, Some("LA")) {
            let raw_bytes = CheckedDims::new(final_w, final_h, 2)?.total_bytes();
            u64::try_from(
                raw_bytes
                    .div_ceil(std::mem::size_of::<u32>())
                    .checked_mul(std::mem::size_of::<u32>())
                    .ok_or_else(|| {
                        PilError::ValueError("GPU native LA readback size overflow".into())
                    })?,
            )
            .map_err(|_| PilError::ValueError("GPU native LA readback size overflow".into()))?
        } else if native_expand_output {
            let channels = native_expand_channels.ok_or_else(|| {
                PilError::InternalError("GPU native Expand output is missing its channels".into())
            })?;
            let native_bytes = CheckedDims::new(final_w, final_h, channels)?.total_bytes();
            u64::try_from(
                native_bytes
                    .div_ceil(std::mem::size_of::<u32>())
                    .checked_mul(std::mem::size_of::<u32>())
                    .ok_or_else(|| {
                        PilError::ValueError("GPU native image readback size overflow".into())
                    })?,
            )
            .map_err(|_| PilError::ValueError("GPU native image readback size overflow".into()))?
        } else if native_luma8_extract
            || native_luma8_grayscale
            || native_luma8_constant
            || packed_luma_transform_output
            || packed_luma_point
            || packed_luma_putdata
            || packed_luma_order_statistic
            || packed_native_byte_filter
            || packed_luma_pad
            || native_sharpness_l_input
        {
            compact_luma8_transfer_bytes(final_w, final_h)?
        } else {
            CheckedDims::new(final_w, final_h, 4)?.total_bytes() as u64
        };
        resource_telemetry.retained_cache_bytes = buffers.retained_bytes();
        resource_telemetry.full_frame_copy_count =
            readback.full_frame_copy_count(!output_only_constant);
        resource_telemetry.mode_conversion_count = if output_only_constant
            || native_luma8_constant
            || packed_luma_convert
            || packed_luma_transform_input
            || packed_luma_order_statistic
            || packed_native_byte_filter
            || packed_luma_pad
            || packed_la_pad
            || native_rgb_compact_input
            || native_la_transform_output
            || native_grayscale_rgb_input
            || native_sharpness_l_input
            || native_sharpness_la_input
            || native_sharpness_rgb_input
            || native_reduce_rgb_input
            || native_expand_channels.is_some()
            || native_cover_input_channels.is_some()
        {
            // This counter measures source widening before backend execution.
            // Native upload avoids that conversion for admitted layouts;
            // RGBA readback narrowing is accounted at the output boundary.
            0
        } else {
            u64::from(
                !native_extract_band
                    && !native_grayscale_rgb_input
                    && !native_sharpness_l_input
                    && !native_sharpness_la_input
                    && !native_sharpness_rgb_input
                    && !native_reduce_rgb_input
                    && !packed_luma_point
                    && !packed_luma_putdata
                    && !packed_luma_order_statistic
                    && !packed_native_byte_filter
                    && !packed_luma_pad
                    && !packed_la_pad
                    && (native_luma16_convert
                        || native_luma16_paste
                        || !matches!(
                            img,
                            DynamicImage::ImageRgba8(_) | DynamicImage::ImageLuma16(_)
                        )),
            )
        };
        crate::compute::record_pipeline_resource_telemetry(resource_telemetry);
        crate::compute::record_pipeline_dispatch_count(dispatch_count);
        if let ReadbackTarget::Staging(staging) = readback {
            gpu.recycle_staging(staging);
        }
        gpu.recycle_buffers(buffers);
        if native_rgb
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
        if native_luma8_extract
            || native_luma8_grayscale
            || native_luma8_constant
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
        if let Some(mode) = put_alpha_mode {
            return put_alpha_output(result, mode);
        }
        if let Some(ct) = out_mode {
            // Bypass preserve_mode — use the override color type directly.
            return gpu_result_as_color_type(result, ct);
        } else {
            Ok(crate::image::preserve_mode(img, result))
        }
    }
}

#[cfg(test)]
mod tests;
