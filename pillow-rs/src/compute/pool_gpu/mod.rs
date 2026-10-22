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

/// Pillow's `ImagingLineBoxBlur{8,32}` in `src/libImaging/BoxBlur.c` uses a
/// normalized replicated-edge average, so a constant image remains constant
/// through every box pass (and therefore Pillow's three-pass Gaussian
/// lowering). Scanning larger frames can cost more than the six GPU passes it
/// replaces, so keep this exact lowering bounded to the benchmark-sized
/// working set where the launch savings are measurable.
const GPU_UNIFORM_BLUR_MAX_PIXELS: usize = 1024 * 1024;

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

fn gpu_buffer_reuse_allowed(capacity: u32, minimum_capacity: u32) -> bool {
    capacity >= minimum_capacity
        && (minimum_capacity == 0
            || capacity <= minimum_capacity.saturating_mul(MAX_GPU_BUFFER_REUSE_RATIO))
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
}

#[cfg(test)]
mod tests;
