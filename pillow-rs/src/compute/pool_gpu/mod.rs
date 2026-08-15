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
use crate::pipeline::{ColorMode, PipelineOp, PixelMode, TransformMethod, TransposeMethod};
use crate::raster::{DynamicImage, GenericImageView, RgbaImage};
use std::collections::HashMap;
use std::num::NonZeroU64;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Keep command submission bounded for very long lazy pipelines. A batch may
/// contain more operations; it is split into sequential submissions without
/// reading the image back between chunks.
const MAX_GPU_OPS_PER_SUBMISSION: usize = 256;

/// Bound transient per-submission auxiliary/parameter arenas. A single
/// operation larger than this limit is still allowed so valid large images are
/// not rejected merely because they need one large upload.
const MAX_GPU_RESOURCE_BYTES_PER_SUBMISSION: usize = 64 * 1024 * 1024;
const MAX_RETAINED_GPU_WORKING_SETS: usize = 2;
const MAX_RETAINED_GPU_WORKING_BYTES: u64 = 128 * 1024 * 1024;
const MAX_RETAINED_GPU_STAGING_BUFFERS: usize = 2;
const MAX_RETAINED_GPU_STAGING_BYTES: u64 = 64 * 1024 * 1024;
const MAX_GPU_AUXILIARY_CACHE_BYTES: usize = 64 * 1024 * 1024;

/// Dynamic shader loops must have a small, explicit upper bound. These limits
/// are deliberately stricter than the CPU implementation: an unsupported or
/// unusually large request is routed to CPU rather than being allowed to
/// monopolize a native GPU queue.
const MAX_GPU_BLUR_RADIUS: u32 = 16;
const MAX_GPU_FILTER_SIZE: u32 = 9;
const MAX_GPU_REDUCE_FACTOR: u32 = 64;
const MAX_GPU_MANDELBROT_ITERS: u32 = 10_000;
const MAX_GPU_SHADER_WORK_ITEMS: u64 = 128 * 1024 * 1024;
const GPU_BUFFER_CAPACITY: u32 = 4096 * 4096;
const MAX_GPU_SCALE_FIXED_POINT: f64 = u32::MAX as f64;
// Add/Subtract currently dispatch only the exact unit-divisor/integral-offset
// subset. Other valid public parameters are routed to CPU until the shader
// carries the full f64 contract without rounding differences.

/// `Maintain::Wait` can wait forever when a native device or driver wedges.
/// Poll in short intervals so the library has a bounded failure path.
const GPU_READBACK_TIMEOUT: Duration = Duration::from_secs(30);
const GPU_POLL_INTERVAL: Duration = Duration::from_millis(5);

#[derive(Clone, Copy)]
struct BufferRange {
    offset: u64,
    size: u64,
}

/// Immutable auxiliary resources shared by operations in one execution.
///
/// GPU submissions are split at a bounded operation/resource budget, but the
/// source graph may reuse the same secondary image, mask, or LUT on both sides
/// of that boundary. Keep only repeated resources in this execution-wide
/// cache; unique resources remain chunk-local. The aggregate cache is capped
/// so a long graph cannot turn deduplication into unbounded retention.
#[derive(Default)]
struct GpuAuxiliaryCache {
    second_ranges: HashMap<usize, BufferRange>,
    third_ranges: HashMap<usize, BufferRange>,
    lut_ranges: HashMap<[u32; 256], BufferRange>,
    img2_values: Vec<u32>,
    img3_values: Vec<u32>,
    lut_values: Vec<u32>,
}

impl GpuAuxiliaryCache {
    fn from_batch(
        ops: &[PipelineOp],
        auxiliary_images: &[AuxiliaryImages],
        mode: u32,
        capacity: u32,
        storage_alignment: usize,
    ) -> Result<Self, PilError> {
        let mut second_counts = HashMap::<usize, usize>::new();
        let mut third_counts = HashMap::<usize, usize>::new();
        let mut lut_counts = HashMap::<[u32; 256], usize>::new();
        for (op, auxiliary) in ops.iter().zip(auxiliary_images) {
            if !matches!(op, PipelineOp::PutData { .. }) {
                if let Some(second) = auxiliary.second.as_ref() {
                    *second_counts
                        .entry(Arc::as_ptr(second) as usize)
                        .or_default() += 1;
                }
            }
            if let Some(third) = auxiliary.third.as_ref() {
                *third_counts.entry(Arc::as_ptr(third) as usize).or_default() += 1;
            }
            if let Some(lut) = extract_lut(op, mode) {
                *lut_counts.entry(lut).or_default() += 1;
            }
        }

        let mut cache = Self::default();
        for (op, auxiliary) in ops.iter().zip(auxiliary_images) {
            if !matches!(op, PipelineOp::PutData { .. }) {
                if let Some(second) = auxiliary.second.as_ref() {
                    let key = Arc::as_ptr(second) as usize;
                    if second_counts.get(&key).copied().unwrap_or_default() > 1
                        && !cache.second_ranges.contains_key(&key)
                    {
                        let values = pack_rgba(&second.to_rgba8(), capacity)?;
                        if cache.total_bytes().saturating_add(values.len() * 4)
                            <= MAX_GPU_AUXILIARY_CACHE_BYTES
                        {
                            let range = append_arena_slice(
                                &mut cache.img2_values,
                                &values,
                                storage_alignment,
                            );
                            cache.second_ranges.insert(key, range);
                        }
                    }
                }
            }
            if let Some(third) = auxiliary.third.as_ref() {
                let key = Arc::as_ptr(third) as usize;
                if third_counts.get(&key).copied().unwrap_or_default() > 1
                    && !cache.third_ranges.contains_key(&key)
                {
                    let values = pack_rgba(&third.to_rgba8(), capacity)?;
                    if cache.total_bytes().saturating_add(values.len() * 4)
                        <= MAX_GPU_AUXILIARY_CACHE_BYTES
                    {
                        let range =
                            append_arena_slice(&mut cache.img3_values, &values, storage_alignment);
                        cache.third_ranges.insert(key, range);
                    }
                }
            }
            if let Some(lut) = extract_lut(op, mode) {
                if lut_counts.get(&lut).copied().unwrap_or_default() > 1
                    && !cache.lut_ranges.contains_key(&lut)
                {
                    if cache.total_bytes().saturating_add(lut.len() * 4)
                        <= MAX_GPU_AUXILIARY_CACHE_BYTES
                    {
                        let range =
                            append_arena_slice(&mut cache.lut_values, &lut, storage_alignment);
                        cache.lut_ranges.insert(lut, range);
                    }
                }
            }
        }
        Ok(cache)
    }

    fn total_bytes(&self) -> usize {
        (self.img2_values.len() + self.img3_values.len() + self.lut_values.len()) * 4
    }
}

/// A batch-owned arena allocation that grows to the largest plan seen by the
/// working-buffer pool and is reused by later plans. The buffer is never
/// shrunk: keeping the capacity with the already-pooled image buffers avoids
/// recreating parameter and auxiliary resources for every materialization.
struct ReusableGpuBuffer {
    buffer: wgpu::Buffer,
    capacity_bytes: u64,
}

impl ReusableGpuBuffer {
    fn new(
        device: &wgpu::Device,
        label: &'static str,
        usage: wgpu::BufferUsages,
        initial_bytes: usize,
        alignment_bytes: usize,
    ) -> Self {
        let capacity_bytes = aligned_bytes(initial_bytes, alignment_bytes) as u64;
        Self {
            buffer: create_sized_buffer(
                device,
                label,
                usage,
                capacity_bytes as usize,
                alignment_bytes,
            ),
            capacity_bytes,
        }
    }

    fn ensure_capacity(
        &mut self,
        device: &wgpu::Device,
        label: &'static str,
        usage: wgpu::BufferUsages,
        required_bytes: usize,
        alignment_bytes: usize,
    ) {
        let required_bytes = aligned_bytes(required_bytes, alignment_bytes) as u64;
        if required_bytes <= self.capacity_bytes {
            return;
        }
        self.buffer = create_sized_buffer(
            device,
            label,
            usage,
            required_bytes as usize,
            alignment_bytes,
        );
        self.capacity_bytes = required_bytes;
    }
}

// ─── BufferPool ────────────────────────────────────────────────────────────

/// GPU storage owned by one in-flight pipeline batch and returned to the
/// bounded working-set pool after readback completes.
///
/// The device, queue, and compiled pipelines are shared, but mutable image
/// storage must not be shared across batches. Keeping these buffers local to
/// an execution removes the need for a process-wide execution mutex. Arena
/// writes remain ordered with their corresponding queue submissions.
struct BufferPool {
    buf_a: wgpu::Buffer,
    buf_b: wgpu::Buffer,
    buf_img2: wgpu::Buffer, // Second image for dual-input ops
    buf_img3: wgpu::Buffer, // Third image for 3-input ops (Composite/Paste mask)
    lut_buf: wgpu::Buffer,  // LUT storage buffer for Eval/PointOp (1024 bytes)
    params_arena: ReusableGpuBuffer,
    img2_arena: ReusableGpuBuffer,
    img3_arena: ReusableGpuBuffer,
    lut_arena: ReusableGpuBuffer,
    capacity: u32,
}

/// One reusable map-readable destination for the final device-to-host copy.
///
/// A staging buffer is returned to the pool only after `map_async` completes
/// and the buffer has been unmapped, so no queued command or host mapping can
/// still own it when a later materialization acquires it.
struct StagingBuffer {
    buffer: wgpu::Buffer,
    capacity_bytes: u64,
}

impl StagingBuffer {
    fn new(device: &wgpu::Device, capacity_bytes: u64) -> Self {
        Self {
            buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("gpu_readback_staging"),
                size: capacity_bytes.max(4),
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            capacity_bytes: capacity_bytes.max(4),
        }
    }
}

impl BufferPool {
    fn new(device: &wgpu::Device, capacity: u32) -> Self {
        let size = (capacity.max(1) as u64) * 4;
        let buf_a = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu_buf_a"),
            size,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let buf_b = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu_buf_b"),
            size,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let buf_img2 = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu_buf_img2"),
            // Missing optional auxiliary inputs are never read by the
            // shader. A single storage element is sufficient as the
            // fallback binding and avoids allocating another full image.
            size: 4,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let buf_img3 = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu_buf_img3"),
            size: 4,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let lut_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu_lut"),
            size: 1024, // 256 entries * 4 bytes each
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let params_arena = ReusableGpuBuffer::new(
            device,
            "gpu_batch_params",
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            4,
            4,
        );
        let img2_arena = ReusableGpuBuffer::new(
            device,
            "gpu_batch_img2",
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            4,
            4,
        );
        let img3_arena = ReusableGpuBuffer::new(
            device,
            "gpu_batch_img3",
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            4,
            4,
        );
        let lut_arena = ReusableGpuBuffer::new(
            device,
            "gpu_batch_lut",
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            1024,
            4,
        );
        BufferPool {
            buf_a,
            buf_b,
            buf_img2,
            buf_img3,
            lut_buf,
            params_arena,
            img2_arena,
            img3_arena,
            lut_arena,
            capacity,
        }
    }

    fn upload_rgba(&self, queue: &wgpu::Queue, rgba: &RgbaImage) -> Result<(), PilError> {
        let (w, h) = rgba.dimensions();
        let pixel_count = CheckedDims::new(w, h, 1)?.total_pixels();
        if pixel_count > self.capacity as usize {
            return Err(PilError::ValueError(format!(
                "GPU buffer capacity {} < image size {}",
                self.capacity, pixel_count
            )));
        }

        // WGSL packs R, G, B, and A into the low-to-high bytes of one u32.
        // Little-endian RGBA storage already has exactly that byte layout, so
        // upload it directly instead of allocating and filling a second host
        // vector. Big-endian builds retain the explicit portable packer.
        #[cfg(target_endian = "little")]
        queue.write_buffer(&self.buf_a, 0, rgba.as_raw());
        #[cfg(target_endian = "big")]
        {
            let packed = pack_rgba(rgba, self.capacity)?;
            queue.write_buffer(&self.buf_a, 0, bytemuck::cast_slice(&packed));
        }

        // The first ordinary dispatch writes buf_b completely. Uploading the
        // source to both ping-pong buffers doubled host-to-device traffic and
        // did not provide input to any dispatch.
        Ok(())
    }

    fn retained_bytes(&self) -> u64 {
        gpu_working_set_bytes(self.capacity)
            .saturating_add(self.params_arena.capacity_bytes)
            .saturating_add(self.img2_arena.capacity_bytes)
            .saturating_add(self.img3_arena.capacity_bytes)
            .saturating_add(self.lut_arena.capacity_bytes)
    }
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
    fallback_img2: &'a wgpu::Buffer,
    fallback_img3: &'a wgpu::Buffer,
    fallback_lut: &'a wgpu::Buffer,
    params: &'a wgpu::Buffer,
    params_ranges: Vec<BufferRange>,
    img2: Option<&'a wgpu::Buffer>,
    img2_ranges: Vec<Option<BufferRange>>,
    img3: Option<&'a wgpu::Buffer>,
    img3_ranges: Vec<Option<BufferRange>>,
    lut: Option<&'a wgpu::Buffer>,
    lut_ranges: Vec<Option<BufferRange>>,
}

struct PreparedGpuBatch<'a> {
    resources: GpuBatchResources<'a>,
    output_dims: Vec<(u32, u32)>,
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
        gpu_log!("[GPU] adapter selected: {:?}", adapter.get_info());
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("pillow-rs-gpu"),
                required_features: wgpu::Features::empty(),
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
        })
    }

    fn acquire_buffers(&self, minimum_capacity: u32) -> Result<BufferPool, PilError> {
        let mut available = self.available_buffers.lock().map_err(|_| {
            PilError::InternalError("GPU working-buffer pool lock is poisoned".into())
        })?;
        let candidate = available
            .iter()
            .enumerate()
            .filter(|(_, buffers)| buffers.capacity >= minimum_capacity)
            .min_by_key(|(_, buffers)| buffers.capacity)
            .map(|(index, _)| index);
        Ok(match candidate {
            Some(index) => available.swap_remove(index),
            None => BufferPool::new(&self.device, minimum_capacity),
        })
    }

    fn recycle_buffers(&self, buffers: BufferPool) {
        if self.failure_detail().is_some()
            || buffers.retained_bytes() > MAX_RETAINED_GPU_WORKING_BYTES
        {
            return;
        }
        let Ok(mut available) = self.available_buffers.lock() else {
            return;
        };
        available.push(buffers);
        available.sort_unstable_by_key(|candidate| candidate.capacity);
        while available.len() > MAX_RETAINED_GPU_WORKING_SETS
            || available.iter().fold(0u64, |total, candidate| {
                total.saturating_add(candidate.retained_bytes())
            }) > MAX_RETAINED_GPU_WORKING_BYTES
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
        let pipeline = Self::build_pipeline(&self.device, key, source).ok_or_else(|| {
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

    fn resolve_batch_pipelines(
        &self,
        ops: &[PipelineOp],
    ) -> Result<Vec<ResolvedPipeline>, PilError> {
        let mut resolved = Vec::with_capacity(ops.len());
        let mut index = 0usize;
        while index < ops.len() {
            if index + 1 < ops.len() && can_fuse_gpu_multiply_screen(ops, index) {
                let fused = self.resolve_pipeline(
                    "__internal_multiply_screen",
                    include_str!("shaders/multiply_screen.wgsl"),
                )?;
                resolved.push(ResolvedPipeline::Single(fused));
                resolved.push(ResolvedPipeline::Skip);
                index += 2;
                continue;
            }

            let op = &ops[index];
            if let Some(pass_count) = Self::blur_pass_count(op) {
                // These internal variants expand one public blur operation
                // into horizontal and vertical dispatches without a host
                // materialization between them.
                let horizontal = self.resolve_pipeline(
                    "__internal_blur_h",
                    include_str!("shaders/box_blur_h.wgsl"),
                )?;
                let vertical = self.resolve_pipeline(
                    "__internal_blur_v",
                    include_str!("shaders/box_blur_v.wgsl"),
                )?;
                resolved.push(ResolvedPipeline::Blur {
                    horizontal,
                    vertical,
                    pass_count,
                });
            } else {
                let key = registry::variant_key(op);
                let source = registry::registry()?
                    .get(key)
                    .and_then(|entry| entry.gpu_source)
                    .ok_or_else(|| {
                        PilError::ValueError(format!(
                            "GPU operation '{key}' has no registered shader source"
                        ))
                    })?;
                resolved.push(
                    self.resolve_pipeline(key, source)
                        .map(ResolvedPipeline::Single)?,
                );
            }
            index += 1;
        }
        Ok(resolved)
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
        shader_source: &str,
    ) -> Option<CachedPipeline> {
        let cs_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(variant_name),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });

        let num_bindings = count_shader_bindings(shader_source);

        // Supported: 2-5 binding shaders. 0/1/>5 are invalid.
        if !(2..=5).contains(&num_bindings) {
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
        if pollster::block_on(device.pop_error_scope()).is_some() {
            return None;
        }

        Some(CachedPipeline {
            pipeline,
            bind_group_layout,
            variant_name,
            num_bindings,
            is_lut,
            is_output_only,
            is_in_place,
        })
    }

    fn make_bind_group(
        &self,
        cached: &CachedPipeline,
        input_buf: &wgpu::Buffer,
        output_buf: &wgpu::Buffer,
        resources: &GpuBatchResources,
        op_index: usize,
    ) -> Result<wgpu::BindGroup, PilError> {
        let params = ranged_binding(resources.params, Some(resources.params_ranges[op_index]))?;
        let second = auxiliary_binding(
            resources.img2,
            resources.fallback_img2,
            resources.img2_ranges[op_index],
        )?;
        let third = auxiliary_binding(
            resources.img3,
            resources.fallback_img3,
            resources.img3_ranges[op_index],
        )?;
        let lut = auxiliary_binding(
            resources.lut,
            resources.fallback_lut,
            resources.lut_ranges[op_index],
        )?;
        let mut entries = Vec::with_capacity(cached.num_bindings as usize);
        match (cached.num_bindings, cached.is_lut, cached.is_output_only) {
            (2, _, true) => {
                // Generator layout: [output(rw), params(uniform)].
                entries.push(wgpu::BindGroupEntry {
                    binding: 0,
                    resource: output_buf.as_entire_binding(),
                });
                entries.push(wgpu::BindGroupEntry {
                    binding: 1,
                    resource: params,
                });
            }
            (5, _, _) => {
                // 5-binding: [in_a(read), in_b(read), in_c(read), out(rw), params(uniform)]
                entries.push(wgpu::BindGroupEntry {
                    binding: 0,
                    resource: input_buf.as_entire_binding(),
                });
                // Storage read and read_write bindings may not alias within one
                // compute dispatch. Keep absent optional inputs on their
                // dedicated auxiliary buffers; shaders guard those reads with
                // their corresponding presence parameter.
                entries.push(wgpu::BindGroupEntry {
                    binding: 1,
                    resource: second,
                });
                entries.push(wgpu::BindGroupEntry {
                    binding: 2,
                    resource: third,
                });
                entries.push(wgpu::BindGroupEntry {
                    binding: 3,
                    resource: output_buf.as_entire_binding(),
                });
                entries.push(wgpu::BindGroupEntry {
                    binding: 4,
                    resource: params,
                });
            }
            (4, true, _) => {
                // LUT layout: [input(read), output(rw), params(uniform), lut(read)]
                entries.push(wgpu::BindGroupEntry {
                    binding: 0,
                    resource: input_buf.as_entire_binding(),
                });
                entries.push(wgpu::BindGroupEntry {
                    binding: 1,
                    resource: output_buf.as_entire_binding(),
                });
                entries.push(wgpu::BindGroupEntry {
                    binding: 2,
                    resource: params,
                });
                entries.push(wgpu::BindGroupEntry {
                    binding: 3,
                    resource: lut,
                });
            }
            (4, false, _) => {
                // Dual-input layout: [input_a(read), input_b(read), output(rw), params(uniform)]
                entries.push(wgpu::BindGroupEntry {
                    binding: 0,
                    resource: input_buf.as_entire_binding(),
                });
                entries.push(wgpu::BindGroupEntry {
                    binding: 1,
                    resource: second,
                });
                entries.push(wgpu::BindGroupEntry {
                    binding: 2,
                    resource: output_buf.as_entire_binding(),
                });
                entries.push(wgpu::BindGroupEntry {
                    binding: 3,
                    resource: params,
                });
            }
            (3, _, _) if cached.is_in_place => {
                // AlphaComposite: [source(read), destination(read_write), params].
                // The destination is the current ping-pong buffer, so the
                // dispatch updates it in place and encode_batch keeps the
                // current buffer selection unchanged.
                entries.push(wgpu::BindGroupEntry {
                    binding: 0,
                    resource: second,
                });
                entries.push(wgpu::BindGroupEntry {
                    binding: 1,
                    resource: input_buf.as_entire_binding(),
                });
                entries.push(wgpu::BindGroupEntry {
                    binding: 2,
                    resource: params,
                });
            }
            _ => {
                // 2 or 3 binding: [input(read), output(rw), ...params(uniform)]
                entries.push(wgpu::BindGroupEntry {
                    binding: 0,
                    resource: input_buf.as_entire_binding(),
                });
                entries.push(wgpu::BindGroupEntry {
                    binding: 1,
                    resource: output_buf.as_entire_binding(),
                });
                if cached.num_bindings > 2 {
                    entries.push(wgpu::BindGroupEntry {
                        binding: 2,
                        resource: params,
                    });
                }
            }
        }
        Ok(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(cached.variant_name),
            layout: &cached.bind_group_layout,
            entries: &entries,
        }))
    }

    fn estimate_resource_bytes(
        &self,
        op: &PipelineOp,
        auxiliary: &AuxiliaryImages,
        buffers: &BufferPool,
        uniform_alignment: usize,
        storage_alignment: usize,
        mode: u32,
    ) -> Result<usize, PilError> {
        let param_words = 4usize
            .checked_add(registry::extract_params(op).len())
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
            let pixels = data.len().div_ceil(mode.channels());
            if pixels > buffers.capacity as usize {
                return Err(PilError::ValueError(format!(
                    "GPU buffer capacity {} < putdata image size {}",
                    buffers.capacity, pixels
                )));
            }
            total = total
                .checked_add(aligned_bytes(
                    pixels
                        .checked_mul(std::mem::size_of::<u32>())
                        .ok_or_else(|| {
                            PilError::ValueError("GPU putdata arena size overflow".into())
                        })?,
                    storage_alignment,
                ))
                .ok_or_else(|| PilError::ValueError("GPU auxiliary arena size overflow".into()))?;
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

    fn prepare_batch<'a>(
        &self,
        ops: &[PipelineOp],
        auxiliary_images: &[AuxiliaryImages],
        w: u32,
        h: u32,
        mode: u32,
        buffers: &'a mut BufferPool,
        auxiliary_cache: &GpuAuxiliaryCache,
    ) -> Result<PreparedGpuBatch<'a>, PilError> {
        if ops.len() != auxiliary_images.len() {
            return Err(PilError::InternalError(
                "GPU operation and auxiliary-image counts differ".into(),
            ));
        }

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
        let mut output_dims = Vec::with_capacity(ops.len());
        let mut cur_w = w;
        let mut cur_h = h;

        for (index, op) in ops.iter().enumerate() {
            let cached = if can_fuse_gpu_multiply_screen(ops, index) {
                self.resolve_pipeline(
                    "__internal_multiply_screen",
                    include_str!("shaders/multiply_screen.wgsl"),
                )?
            } else if Self::blur_pass_count(op).is_some() {
                self.resolve_pipeline("__internal_blur_h", include_str!("shaders/box_blur_h.wgsl"))?
            } else {
                let base_key = registry::variant_key(op);
                let source = registry::registry()?
                    .get(base_key)
                    .and_then(|entry| entry.gpu_source)
                    .ok_or_else(|| {
                        PilError::ValueError(format!(
                            "GPU operation '{base_key}' has no registered shader source"
                        ))
                    })?;
                self.resolve_pipeline(base_key, source)?
            };
            let (out_w, out_h) = op_output_dims(op, cur_w, cur_h).unwrap_or((cur_w, cur_h));
            self.validate_output_dims(buffers, out_w, out_h)?;
            output_dims.push((out_w, out_h));

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
            let mut params = vec![shader_w, shader_h, mode, 0u32];
            params.extend(registry::extract_params(op));
            // Shaders that declare dst_w/dst_h at the end of Params read these
            // words; shaders without them ignore the trailing words.
            params.extend([out_w, out_h]);
            params_ranges.push(append_arena_slice(
                &mut params_arena,
                &params,
                uniform_alignment,
            ));

            let second_range = if let PipelineOp::PutData { data, mode } = op {
                let second_values = pack_put_data(data, *mode, buffers.capacity)?;
                if second_values.is_empty() {
                    None
                } else {
                    let mut range =
                        append_arena_slice(&mut img2_arena, &second_values, storage_alignment);
                    range.offset += (auxiliary_cache.img2_values.len() * 4) as u64;
                    Some(range)
                }
            } else if let Some(second) = auxiliary_images[index].second.as_ref() {
                let key = Arc::as_ptr(second) as usize;
                if let Some(range) = auxiliary_cache.second_ranges.get(&key).copied() {
                    Some(range)
                } else if let Some(range) = second_cache.get(&key).copied() {
                    Some(range)
                } else {
                    let values = pack_rgba(&second.to_rgba8(), buffers.capacity)?;
                    let mut range = append_arena_slice(&mut img2_arena, &values, storage_alignment);
                    range.offset += (auxiliary_cache.img2_values.len() * 4) as u64;
                    second_cache.insert(key, range);
                    Some(range)
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
                    let values = pack_rgba(&third.to_rgba8(), buffers.capacity)?;
                    let mut range = append_arena_slice(&mut img3_arena, &values, storage_alignment);
                    range.offset += (auxiliary_cache.img3_values.len() * 4) as u64;
                    third_cache.insert(key, range);
                    Some(range)
                }
            } else {
                None
            };
            img3_ranges.push(third_range);

            let lut_values = if cached.is_lut {
                Some(extract_lut(op, mode).ok_or_else(|| {
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

        let img2 = if auxiliary_cache.img2_values.is_empty() && img2_arena.is_empty() {
            None
        } else {
            let previous_capacity = buffers.img2_arena.capacity_bytes;
            buffers.img2_arena.ensure_capacity(
                &self.device,
                "gpu_batch_img2",
                wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                (auxiliary_cache.img2_values.len() + img2_arena.len())
                    .saturating_mul(std::mem::size_of::<u32>()),
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
                * std::mem::size_of::<u32>()) as u64,
            ..PipelineResourceTelemetry::default()
        };

        Ok(PreparedGpuBatch {
            resources: GpuBatchResources {
                buf_a: &buffers.buf_a,
                buf_b: &buffers.buf_b,
                fallback_img2: &buffers.buf_img2,
                fallback_img3: &buffers.buf_img3,
                fallback_lut: &buffers.lut_buf,
                params: &buffers.params_arena.buffer,
                params_ranges,
                img2,
                img2_ranges,
                img3,
                img3_ranges,
                lut,
                lut_ranges,
            },
            output_dims,
            final_dims: (cur_w, cur_h),
            resource_telemetry,
        })
    }

    fn encode_dispatch(
        &self,
        cpass: &mut wgpu::ComputePass<'_>,
        cached: &CachedPipeline,
        index: usize,
        current_is_a: bool,
        resources: &GpuBatchResources,
        output_dims: (u32, u32),
    ) -> Result<bool, PilError> {
        let (input_buf, output_buf) = if current_is_a {
            (resources.buf_a, resources.buf_b)
        } else {
            (resources.buf_b, resources.buf_a)
        };
        let bind_group = self.make_bind_group(cached, input_buf, output_buf, resources, index)?;
        cpass.set_pipeline(&cached.pipeline);
        cpass.set_bind_group(0, &bind_group, &[]);
        let (dispatch_w, dispatch_h) = match cached.variant_name {
            "__internal_blur_h" => (1, output_dims.1),
            "__internal_blur_v" => (output_dims.0, 1),
            _ => (output_dims.0.div_ceil(16), output_dims.1.div_ceil(16)),
        };
        cpass.dispatch_workgroups(dispatch_w, dispatch_h, 1);
        Ok(if cached.is_in_place {
            current_is_a
        } else {
            !current_is_a
        })
    }

    fn blur_pass_count(op: &PipelineOp) -> Option<usize> {
        match op {
            PipelineOp::BoxBlur { .. } => Some(1),
            PipelineOp::GaussianBlur { .. } => Some(3),
            _ => None,
        }
    }

    fn encode_batch(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        ops: &[PipelineOp],
        prepared: &PreparedGpuBatch,
        start_is_a: bool,
    ) -> Result<bool, PilError> {
        let resolved = self.resolve_batch_pipelines(ops)?;
        let mut current_is_a = start_is_a;
        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("gpu_batch_compute"),
            timestamp_writes: None,
        });
        for (index, pipeline) in resolved.iter().enumerate() {
            if matches!(pipeline, ResolvedPipeline::Skip) {
                continue;
            }
            if let ResolvedPipeline::Blur {
                horizontal,
                vertical,
                pass_count,
            } = pipeline
            {
                // Pillow's GaussianBlur is three horizontal box passes
                // followed by three vertical box passes. BoxBlur is one of
                // each. Keep all passes in this compute pass and ping-pong
                // between the two image buffers; no intermediate readback or
                // CPU serialization is needed.
                for _ in 0..*pass_count {
                    current_is_a = self.encode_dispatch(
                        &mut cpass,
                        horizontal,
                        index,
                        current_is_a,
                        &prepared.resources,
                        prepared.output_dims[index],
                    )?;
                }
                for _ in 0..*pass_count {
                    current_is_a = self.encode_dispatch(
                        &mut cpass,
                        vertical,
                        index,
                        current_is_a,
                        &prepared.resources,
                        prepared.output_dims[index],
                    )?;
                }
            } else {
                let ResolvedPipeline::Single(cached) = pipeline else {
                    unreachable!("resolved GPU pipeline variant changed during encoding")
                };
                current_is_a = self.encode_dispatch(
                    &mut cpass,
                    cached,
                    index,
                    current_is_a,
                    &prepared.resources,
                    prepared.output_dims[index],
                )?;
            }
        }
        drop(cpass);
        Ok(current_is_a)
    }

    fn readback_to_image(
        &self,
        w: u32,
        h: u32,
        staging: &wgpu::Buffer,
    ) -> Result<DynamicImage, PilError> {
        let size = CheckedDims::new(w, h, 4)?.total_bytes() as u64;

        let slice = staging.slice(..size);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        let deadline = Instant::now() + GPU_READBACK_TIMEOUT;
        loop {
            self.poll_device("GPU readback")?;
            match rx.recv_timeout(GPU_POLL_INTERVAL) {
                Ok(Ok(())) => break,
                Ok(Err(error)) => {
                    return Err(PilError::ValueError(format!(
                        "GPU readback map_async failed: {error:?}"
                    )));
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(PilError::ValueError(
                        "GPU readback channel closed before completion".into(),
                    ));
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) if Instant::now() < deadline => {}
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    let detail = self
                        .failure_detail()
                        .unwrap_or_else(|| "device did not complete the submission".into());
                    let message = format!(
                        "GPU readback timed out after {}s: {detail}",
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

        let data = slice.get_mapped_range().to_vec();
        let _ = slice;
        staging.unmap();

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

    fn execute_batch_impl(
        &self,
        ops: &[PipelineOp],
        auxiliary_images: &[AuxiliaryImages],
        w: u32,
        h: u32,
        mode: u32,
        buffers: &mut BufferPool,
    ) -> Result<
        (
            bool,
            u32,
            u32,
            StagingBuffer,
            PipelineResourceTelemetry,
            u64,
        ),
        PilError,
    > {
        let mut current_is_a = true;
        let mut cur_w = w;
        let mut cur_h = h;
        let dispatch_count = gpu_dispatch_count(ops);
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
        let resource_bytes = ops
            .iter()
            .zip(auxiliary_images.iter())
            .map(|(op, auxiliary)| {
                self.estimate_resource_bytes(
                    op,
                    auxiliary,
                    buffers,
                    uniform_alignment,
                    storage_alignment,
                    mode,
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
                let work = gpu_shader_work_items(op, (work_w, work_h), next).ok_or_else(|| {
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
        let mut staging = None;
        let mut resource_telemetry = PipelineResourceTelemetry {
            auxiliary_bytes: auxiliary_cache.total_bytes() as u64,
            ..PipelineResourceTelemetry::default()
        };
        while chunk_start < ops.len() {
            self.ensure_healthy("GPU batch submission")?;
            let chunk_end = select_gpu_chunk_end(chunk_start, &resource_bytes, &shader_work_items)
                .ok_or_else(|| {
                    PilError::InternalError(
                    "GPU chunk scheduling made no progress because its estimates are inconsistent"
                        .into(),
                )
                })?;
            let estimated_bytes = resource_bytes[chunk_start..chunk_end]
                .iter()
                .fold(0usize, |total, bytes| total.saturating_add(*bytes));
            let estimated_work = shader_work_items[chunk_start..chunk_end]
                .iter()
                .fold(0u64, |total, work| total.saturating_add(*work));

            let prepared = self.prepare_batch(
                &ops[chunk_start..chunk_end],
                &auxiliary_images[chunk_start..chunk_end],
                cur_w,
                cur_h,
                mode,
                buffers,
                &auxiliary_cache,
            )?;
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
            )?;

            let final_dims = prepared.final_dims;
            if chunk_end == ops.len() {
                let size = CheckedDims::new(final_dims.0, final_dims.1, 4)?.total_bytes() as u64;
                let readback = self.acquire_staging(size)?;
                let src = if current_is_a {
                    prepared.resources.buf_a
                } else {
                    prepared.resources.buf_b
                };
                // Record the copy after the compute pass in the same command
                // buffer. Queue ordering therefore covers compute and
                // readback together, avoiding a second command-buffer/submit
                // lifecycle at the point where the native driver previously
                // wedged during command recording.
                encoder.copy_buffer_to_buffer(src, 0, &readback.buffer, 0, size);
                staging = Some(readback);
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
            chunk_start = chunk_end;
        }
        // After all chunks, current_is_a tracks where the latest result lives:
        //   true → buf_a has the final result, false → buf_b. Queue ordering
        // keeps chunk submissions dependent without a blocking poll between them.
        let staging = staging.ok_or_else(|| {
            PilError::InternalError("GPU batch produced no readback staging buffer".into())
        })?;
        Ok((
            current_is_a,
            cur_w,
            cur_h,
            staging,
            resource_telemetry,
            dispatch_count,
        ))
    }
}

static GPU: std::sync::OnceLock<Result<GpuInner, PilError>> = std::sync::OnceLock::new();

// ─── Mode helpers ───────────────────────────────────────────────────────────

/// Map a DynamicImage variant to its mode code for the GPU uniform buffer.
/// 0 = L (1 channel), 1 = LA (2 channels), 2 = RGB (3 channels), 3 = RGBA (4 channels)
fn mode_code(img: &DynamicImage) -> u32 {
    match img {
        DynamicImage::ImageLuma8(_) => 0,
        DynamicImage::ImageLumaA8(_) => 1,
        DynamicImage::ImageRgb8(_) => 2,
        DynamicImage::ImageRgba8(_) => 3,
        _ => 3, // fallback: treat as RGBA
    }
}

/// Return the concrete byte-buffer color type produced by a supported GPU
/// mode-changing operation. The shader always writes packed RGBA8, but the
/// public result must expose the requested number of bands rather than the
/// source image's type.
fn gpu_output_color_type(mode: &ColorMode) -> Option<crate::raster::ColorType> {
    match mode {
        ColorMode::L => Some(crate::raster::ColorType::L8),
        ColorMode::LA => Some(crate::raster::ColorType::La8),
        ColorMode::RGB => Some(crate::raster::ColorType::Rgb8),
        ColorMode::RGBA => Some(crate::raster::ColorType::Rgba8),
        _ => None,
    }
}

/// Convert the packed GPU result to the requested standard byte mode without
/// applying a second luma conversion. GPU L/LA shaders intentionally keep the
/// luma sample in byte 0; calling `to_luma8()` on that RGBA transport would
/// weight the zeroed G/B bytes again and change every sample.
fn gpu_result_as_color_type(
    result: DynamicImage,
    color_type: crate::raster::ColorType,
) -> Result<DynamicImage, PilError> {
    let rgba = result.to_rgba8();
    let (w, h) = rgba.dimensions();
    match color_type {
        crate::raster::ColorType::L8 => {
            let luma = rgba.pixels().map(|pixel| pixel[0]).collect();
            crate::raster::GrayImage::from_raw(w, h, luma)
                .map(DynamicImage::ImageLuma8)
                .ok_or_else(|| PilError::InternalError("GPU L output shape mismatch".into()))
        }
        crate::raster::ColorType::La8 => {
            let samples = rgba
                .pixels()
                .flat_map(|pixel| [pixel[0], pixel[3]])
                .collect();
            crate::raster::GrayAlphaImage::from_raw(w, h, samples)
                .map(DynamicImage::ImageLumaA8)
                .ok_or_else(|| PilError::InternalError("GPU LA output shape mismatch".into()))
        }
        crate::raster::ColorType::Rgb8 => Ok(DynamicImage::ImageRgb8(
            DynamicImage::ImageRgba8(rgba).to_rgb8(),
        )),
        crate::raster::ColorType::Rgba8 => Ok(DynamicImage::ImageRgba8(rgba)),
        _ => Err(PilError::InternalError(
            "unsupported GPU output color type".into(),
        )),
    }
}

/// Return whether a batch contains a mode-changing operation followed by a
/// later shader. The current GPU uniform arena carries one source mode for a
/// dispatch, so such a batch must remain on the sequential CPU path.
fn gpu_batch_has_nonterminal_mode_change(ops: &[PipelineOp]) -> bool {
    ops.iter().enumerate().any(|(index, op)| {
        index + 1 < ops.len()
            && matches!(
                op,
                PipelineOp::Convert { .. }
                    | PipelineOp::Grayscale
                    | PipelineOp::ExtractBand { .. }
                    | PipelineOp::Constant { .. }
                    | PipelineOp::PutAlpha { .. }
                    | PipelineOp::PutAlphaData { .. }
            )
    })
}

fn put_alpha_output(result: DynamicImage, mode: PixelMode) -> Result<DynamicImage, PilError> {
    if matches!(
        mode,
        PixelMode::L | PixelMode::LA | PixelMode::P | PixelMode::PA
    ) {
        let rgba = result.to_rgba8();
        let (w, h) = rgba.dimensions();
        let samples = rgba
            .pixels()
            .flat_map(|pixel| [pixel[0], pixel[3]])
            .collect();
        crate::raster::GrayAlphaImage::from_raw(w, h, samples)
            .map(DynamicImage::ImageLumaA8)
            .ok_or_else(|| {
                PilError::InternalError("GPU putalpha buffer shape mismatch".to_string())
            })
    } else {
        Ok(DynamicImage::ImageRgba8(result.to_rgba8()))
    }
}

/// Extract the second (right-hand) image from a dual-input PipelineOp, if present.
/// Returns shared materialized pixels ready for GPU upload.
fn extract_second_image(op: &PipelineOp) -> Result<Option<Arc<DynamicImage>>, PilError> {
    if let PipelineOp::Paste { source, .. } = op {
        // Paste has already converted its source to the destination mode. Keep
        // P-mode sources as their one-byte indices here: expanding them through
        // the palette before packing the GPU upload would make the GPU lane
        // disagree with the CPU and SIMD paste implementations.
        return source.materialized_shared().map(Some);
    }
    if let PipelineOp::CompositeModule { other, .. } = op {
        if other.mode()? == "P" {
            // Image.composite blends P indices and gives the result image2's
            // palette. Upload image2's indices, not its visible RGB expansion.
            return other.materialized_shared().map(Some);
        }
    }

    let arc_img: Option<&std::sync::Arc<crate::image::Image>> = match op {
        PipelineOp::Add { other, .. }
        | PipelineOp::Subtract { other, .. }
        | PipelineOp::Multiply { other }
        | PipelineOp::Screen { other }
        | PipelineOp::Darker { other }
        | PipelineOp::Lighter { other }
        | PipelineOp::Difference { other }
        | PipelineOp::Overlay { other }
        | PipelineOp::HardLight { other }
        | PipelineOp::SoftLight { other }
        | PipelineOp::AddModulo { other }
        | PipelineOp::SubtractModulo { other }
        | PipelineOp::LogicalAnd { other }
        | PipelineOp::LogicalOr { other }
        | PipelineOp::LogicalXor { other }
        | PipelineOp::Blend { other, .. }
        | PipelineOp::Composite { other, .. }
        | PipelineOp::BlendModule { other, .. }
        | PipelineOp::CompositeModule { other, .. } => Some(other),
        PipelineOp::AlphaComposite { source, .. } => Some(source),
        _ => None,
    };
    arc_img
        .map(|image| image.materialized_shared().map(Some))
        .unwrap_or(Ok(None))
}

/// Extract the third image (mask) from a 3-input PipelineOp, if present.
/// Returns shared materialized pixels ready for GPU upload.
fn extract_third_image(op: &PipelineOp) -> Result<Option<Arc<DynamicImage>>, PilError> {
    let arc_img: Option<&std::sync::Arc<crate::image::Image>> = match op {
        PipelineOp::Composite { mask, .. } | PipelineOp::CompositeModule { mask, .. } => Some(mask),
        PipelineOp::Paste { mask, .. } => mask.as_ref(),
        _ => None,
    };
    arc_img
        .map(|image| image.materialized_shared().map(Some))
        .unwrap_or(Ok(None))
}

struct AuxiliaryImages {
    second: Option<Arc<DynamicImage>>,
    third: Option<Arc<DynamicImage>>,
}

fn extract_auxiliary_images(op: &PipelineOp) -> Result<AuxiliaryImages, PilError> {
    // Pillow resolves each operation's source before its mask, then advances
    // to the next operation. Preserve that observable error order instead of
    // collecting one auxiliary slot across the whole batch at a time.
    Ok(AuxiliaryImages {
        second: extract_second_image(op)?,
        third: extract_third_image(op)?,
    })
}

/// Extract and pack LUT data from a PipelineOp into [u32; 256] for GPU upload.
/// Each u32 packs RGBA channels for one LUT entry (R in byte 0, G byte 1, B byte 2, A byte 3).
fn extract_lut(op: &PipelineOp, mode: u32) -> Option<[u32; 256]> {
    if let PipelineOp::RemapPalette { dest_map } = op {
        let mut inverse = [0u8; 256];
        for (new_index, &old_index) in dest_map.iter().take(256).enumerate() {
            inverse[usize::from(old_index)] = new_index as u8;
        }
        let mut packed = [0u32; 256];
        for (entry, &value) in packed.iter_mut().zip(inverse.iter()) {
            let value = u32::from(value);
            *entry = value | (value << 8) | (value << 16) | (value << 24);
        }
        return Some(packed);
    }
    let lut_bytes: &[u8] = match op {
        PipelineOp::Eval { lut } | PipelineOp::PointOp { lut } => lut.as_ref(),
        _ => return None,
    };
    let channels = match mode {
        0 => 1usize,
        1 => 2,
        2 => 3,
        3 => 4,
        _ => return None,
    };
    if lut_bytes.len() != channels * 256 {
        return None;
    }
    let mut packed = [0u32; 256];
    // CPU Eval/PointOp stores one complete 256-entry table per logical band.
    // Pack those band-major tables into the per-index RGBA transport used by
    // the shader. Alpha is looked up for LA/RGBA; opaque modes use 255.
    for (i, p) in packed.iter_mut().enumerate() {
        let r = u32::from(lut_bytes[i]);
        let g = if channels >= 3 {
            u32::from(lut_bytes[256 + i])
        } else {
            r
        };
        let b = if channels >= 3 {
            u32::from(lut_bytes[512 + i])
        } else {
            r
        };
        let a = if channels == 2 {
            u32::from(lut_bytes[256 + i])
        } else if channels == 4 {
            u32::from(lut_bytes[768 + i])
        } else {
            255
        };
        *p = r | (g << 8) | (b << 16) | (a << 24);
    }
    Some(packed)
}

/// Build the byte LUT represented by a GPU-compatible point operation.
///
/// `Invert`, `InvertChops`, `Solarize`, and `Posterize` normally have their
/// own shaders.  For a contiguous run of point operations their per-channel
/// byte semantics are exactly representable by the generic LUT shader.  The
/// mode restrictions mirror the public ImageOps constructors: alpha-bearing
/// Solarize/Posterize operations stay on their existing shader path, while
/// explicit Eval/PointOp tables remain valid for every packed byte mode.
fn gpu_point_lut(op: &PipelineOp, mode: u32) -> Option<Vec<u8>> {
    let channels = match mode {
        0 => 1usize,
        1 => 2,
        2 => 3,
        3 => 4,
        _ => return None,
    };
    if let PipelineOp::Eval { lut } | PipelineOp::PointOp { lut } = op {
        return (lut.len() == channels * 256).then(|| lut.to_vec());
    }
    if matches!(
        op,
        PipelineOp::Solarize { .. } | PipelineOp::Posterize { .. }
    ) && matches!(mode, 1 | 3)
    {
        return None;
    }

    let map = |value: u8| -> Option<u8> {
        match op {
            PipelineOp::Invert | PipelineOp::InvertChops => Some(255 - value),
            PipelineOp::Solarize { threshold } => Some(if value >= *threshold {
                255 - value
            } else {
                value
            }),
            PipelineOp::Posterize { bits } if (1..=8).contains(bits) => {
                let mask = !((1u8 << (8 - bits)) - 1);
                Some(value & mask)
            }
            _ => None,
        }
    };
    let mut lut = Vec::with_capacity(channels * 256);
    for _ in 0..channels {
        for value in 0..=u8::MAX {
            lut.push(map(value)?);
        }
    }
    Some(lut)
}

/// Collapse adjacent exact point operations into one generic LUT dispatch.
///
/// The public operation count remains the original count in the outer
/// telemetry receipt; only the GPU execution plan is rewritten.  Non-point
/// operations terminate a run, so geometry, neighborhood, and multi-image
/// ordering are unchanged.
fn fuse_gpu_point_ops(ops: &[PipelineOp], mode: u32) -> Vec<PipelineOp> {
    let mut fused = Vec::with_capacity(ops.len());
    let mut index = 0usize;
    while index < ops.len() {
        let Some(first) = gpu_point_lut(&ops[index], mode) else {
            fused.push(ops[index].clone());
            index += 1;
            continue;
        };
        let mut composed = first;
        let mut consumed = 1usize;
        while index + consumed < ops.len() {
            let Some(next) = gpu_point_lut(&ops[index + consumed], mode) else {
                break;
            };
            for band in 0..(composed.len() / 256) {
                let offset = band * 256;
                for value in 0..256usize {
                    composed[offset + value] = next[offset + composed[offset + value] as usize];
                }
            }
            consumed += 1;
        }
        if consumed >= 2 {
            fused.push(PipelineOp::PointOp {
                lut: composed.into(),
            });
        } else {
            fused.push(ops[index].clone());
        }
        index += consumed;
    }
    fused
}

fn transpose_output_dimensions(method: &TransposeMethod, width: u32, height: u32) -> (u32, u32) {
    match method {
        TransposeMethod::Rotate90
        | TransposeMethod::Rotate270
        | TransposeMethod::Transpose
        | TransposeMethod::Transverse => (height, width),
        _ => (width, height),
    }
}

fn transpose_forward(
    method: &TransposeMethod,
    width: u32,
    height: u32,
    x: u32,
    y: u32,
) -> (u32, u32) {
    match method {
        TransposeMethod::FlipLeftRight => (width - 1 - x, y),
        TransposeMethod::FlipTopBottom => (x, height - 1 - y),
        TransposeMethod::Rotate90 => (y, width - 1 - x),
        TransposeMethod::Rotate180 => (width - 1 - x, height - 1 - y),
        TransposeMethod::Rotate270 => (height - 1 - y, x),
        TransposeMethod::Transpose => (y, x),
        TransposeMethod::Transverse => (height - 1 - y, width - 1 - x),
    }
}

/// Compose adjacent GPU transpose operations before resources and dispatches
/// are planned. The seven Pillow methods form a closed dihedral transform
/// set, so corner mapping is an exact composition check rather than a pixel
/// approximation.
fn compose_transpose_methods(
    first: &TransposeMethod,
    second: &TransposeMethod,
    width: u32,
    height: u32,
) -> Option<TransposeMethod> {
    if width == 0 || height == 0 {
        return None;
    }
    let middle_dimensions = transpose_output_dimensions(first, width, height);
    let output_dimensions =
        transpose_output_dimensions(second, middle_dimensions.0, middle_dimensions.1);
    let corners = [
        (0, 0),
        (width - 1, 0),
        (0, height - 1),
        (width - 1, height - 1),
    ];
    let candidates = [
        TransposeMethod::FlipLeftRight,
        TransposeMethod::FlipTopBottom,
        TransposeMethod::Rotate90,
        TransposeMethod::Rotate180,
        TransposeMethod::Rotate270,
        TransposeMethod::Transpose,
        TransposeMethod::Transverse,
    ];
    candidates.into_iter().find(|candidate| {
        if transpose_output_dimensions(candidate, width, height) != output_dimensions {
            return false;
        }
        corners.iter().all(|&(x, y)| {
            let middle = transpose_forward(first, width, height, x, y);
            let expected = transpose_forward(
                second,
                middle_dimensions.0,
                middle_dimensions.1,
                middle.0,
                middle.1,
            );
            transpose_forward(candidate, width, height, x, y) == expected
        })
    })
}

fn fuse_gpu_transpose_ops(ops: &[PipelineOp], width: u32, height: u32) -> Vec<PipelineOp> {
    let mut fused = Vec::with_capacity(ops.len());
    let mut index = 0usize;
    while index < ops.len() {
        let PipelineOp::Transpose { method } = &ops[index] else {
            fused.push(ops[index].clone());
            index += 1;
            continue;
        };
        let mut combined = method.clone();
        let mut consumed = 1usize;
        while index + consumed < ops.len() {
            let PipelineOp::Transpose { method: next } = &ops[index + consumed] else {
                break;
            };
            let Some(composed) = compose_transpose_methods(&combined, next, width, height) else {
                break;
            };
            combined = composed;
            consumed += 1;
        }
        fused.push(PipelineOp::Transpose { method: combined });
        index += consumed;
    }
    fused
}

/// Return whether two adjacent public Chops operations can share one exact
/// dual-input GPU traversal.  The source identity guard is important: the
/// fused shader consumes the same secondary image for both formulas, so two
/// equal-looking but independently constructed images must retain the normal
/// two-dispatch path.
fn can_fuse_gpu_multiply_screen(ops: &[PipelineOp], index: usize) -> bool {
    if index + 1 >= ops.len() {
        return false;
    }
    match (&ops[index], &ops[index + 1]) {
        (PipelineOp::Multiply { other: first }, PipelineOp::Screen { other: second }) => {
            first.shares_execution_source(second)
        }
        _ => false,
    }
}

fn gpu_dispatch_count(ops: &[PipelineOp]) -> u64 {
    let mut count = 0u64;
    let mut index = 0usize;
    while index < ops.len() {
        if can_fuse_gpu_multiply_screen(ops, index) {
            count += 1;
            index += 2;
            continue;
        }
        count += GpuInner::blur_pass_count(&ops[index])
            .map_or(1usize, |passes| passes.saturating_mul(2)) as u64;
        index += 1;
    }
    count
}

/// Compute output dimensions for a size-changing op given current input dimensions.
/// Returns `None` if the op does not change the image dimensions.
fn round_positive_ties_even(value: f64) -> f64 {
    if !value.is_finite() || value <= 0.0 {
        return 0.0;
    }
    let lower = value.floor();
    let fraction = value - lower;
    if fraction < 0.5 {
        lower
    } else if fraction > 0.5 || (lower as u64) % 2 == 1 {
        lower + 1.0
    } else {
        lower
    }
}

fn op_output_dims(op: &PipelineOp, cur_w: u32, cur_h: u32) -> Option<(u32, u32)> {
    match op {
        PipelineOp::Resize { w, h, .. } => Some((*w, *h)),
        PipelineOp::Pad { w, h, .. } => Some((*w, *h)),
        PipelineOp::Crop {
            left,
            top,
            right,
            bottom,
        } => Some((right.checked_sub(*left)?, bottom.checked_sub(*top)?)),
        PipelineOp::Expand { border, .. } => {
            let border = border.checked_mul(2)?;
            let new_w = cur_w.checked_add(border)?;
            let new_h = cur_h.checked_add(border)?;
            Some((new_w, new_h))
        }
        PipelineOp::CropBorder { border } => {
            let border = border.checked_mul(2)?;
            let new_w = cur_w.checked_sub(border)?;
            let new_h = cur_h.checked_sub(border)?;
            Some((new_w, new_h))
        }
        PipelineOp::Rotate { angle, expand, .. } => {
            if *expand {
                let (sw, sh) = (cur_w as f64, cur_h as f64);
                let rad = angle.to_radians();
                let (cos_a, sin_a) = (rad.cos(), rad.sin());
                let corners = [(0.0, 0.0), (sw, 0.0), (sw, sh), (0.0, sh)];
                let (mut min_x, mut min_y, mut max_x, mut max_y) =
                    (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
                for &(cx, cy) in &corners {
                    let rx = cx * cos_a - cy * sin_a;
                    let ry = cx * sin_a + cy * cos_a;
                    min_x = min_x.min(rx);
                    max_x = max_x.max(rx);
                    min_y = min_y.min(ry);
                    max_y = max_y.max(ry);
                }
                let dw = (max_x - min_x).ceil();
                let dh = (max_y - min_y).ceil();
                if !dw.is_finite()
                    || !dh.is_finite()
                    || dw < 0.0
                    || dh < 0.0
                    || dw > f64::from(u32::MAX)
                    || dh > f64::from(u32::MAX)
                {
                    return None;
                }
                Some((dw as u32, dh as u32))
            } else {
                Some((cur_w, cur_h))
            }
        }
        PipelineOp::Transpose { method } => {
            if matches!(
                method,
                TransposeMethod::Rotate90
                    | TransposeMethod::Rotate270
                    | TransposeMethod::Transpose
                    | TransposeMethod::Transverse
            ) {
                Some((cur_h, cur_w))
            } else {
                Some((cur_w, cur_h))
            }
        }
        PipelineOp::Reduce { x_factor, y_factor } => {
            let fx = (*x_factor).max(1);
            let fy = (*y_factor).max(1);
            Some((cur_w.div_ceil(fx), cur_h.div_ceil(fy)))
        }
        PipelineOp::Scale { factor, .. } => {
            // ImageOps.scale uses Python's round(width * factor), including
            // ties-to-even at half-pixel products.
            let new_w = round_positive_ties_even(f64::from(cur_w) * factor);
            let new_h = round_positive_ties_even(f64::from(cur_h) * factor);
            if !new_w.is_finite()
                || !new_h.is_finite()
                || new_w < 0.0
                || new_h < 0.0
                || new_w > f64::from(u32::MAX)
                || new_h > f64::from(u32::MAX)
            {
                return None;
            }
            let new_w = new_w.max(1.0) as u32;
            let new_h = new_h.max(1.0) as u32;
            Some((new_w, new_h))
        }
        PipelineOp::Transform { w, h, .. } => Some((*w, *h)),
        PipelineOp::LinearGradient { .. } | PipelineOp::RadialGradient { .. } => Some((256, 256)),
        PipelineOp::EffectMandelbrot { w, h, .. } => Some((*w, *h)),
        PipelineOp::CompositeModule { other, .. } => other.size().ok(),
        _ => None,
    }
}

/// Return true for operations whose output dimensions are not simply the
/// current input dimensions. `op_output_dims` returns `None` for an overflow or
/// an unavailable nested-image size, so callers must distinguish that from a
/// genuinely dimension-preserving operation before dispatching.
fn op_has_explicit_output_dimensions(op: &PipelineOp) -> bool {
    matches!(
        op,
        PipelineOp::Resize { .. }
            | PipelineOp::Pad { .. }
            | PipelineOp::Crop { .. }
            | PipelineOp::Expand { .. }
            | PipelineOp::CropBorder { .. }
            | PipelineOp::Rotate { .. }
            | PipelineOp::Transpose { .. }
            | PipelineOp::Thumbnail { .. }
            | PipelineOp::Contain { .. }
            | PipelineOp::Cover { .. }
            | PipelineOp::Fit { .. }
            | PipelineOp::Reduce { .. }
            | PipelineOp::Scale { .. }
            | PipelineOp::Transform { .. }
            | PipelineOp::CompositeModule { .. }
            | PipelineOp::LinearGradient { .. }
            | PipelineOp::RadialGradient { .. }
            | PipelineOp::EffectMandelbrot { .. }
    )
}

fn auxiliary_dimensions(image: Option<&DynamicImage>) -> Option<(u32, u32)> {
    image.map(DynamicImage::dimensions)
}

/// The GPU transport is packed RGBA8. Do not let `DynamicImage::to_rgba8`
/// silently narrow a 16-bit or floating-point source before a shader runs:
/// those formats have different Pillow sample semantics and must stay on the
/// CPU path until a native GPU representation exists.
fn gpu_image_layout_is_supported(image: &DynamicImage) -> bool {
    matches!(
        image,
        DynamicImage::ImageLuma8(_)
            | DynamicImage::ImageLumaA8(_)
            | DynamicImage::ImageRgb8(_)
            | DynamicImage::ImageRgba8(_)
    )
}

/// Validate the index space assumptions made by multi-input shaders. A
/// storage-array read is not bounds-checked by WGSL, so an image that is
/// smaller than the coordinates used by a shader must never be uploaded for a
/// GPU dispatch. Operations with intentionally different canvases (Paste and
/// CompositeModule) are checked against the dimensions encoded in their
/// parameter contract instead of being forced to match the destination.
fn gpu_auxiliary_shapes_are_safe(
    op: &PipelineOp,
    auxiliary: &AuxiliaryImages,
    cur_w: u32,
    cur_h: u32,
) -> bool {
    let second = auxiliary_dimensions(auxiliary.second.as_deref());
    let third = auxiliary_dimensions(auxiliary.third.as_deref());
    let current = (cur_w, cur_h);

    match op {
        PipelineOp::Paste { w, h, mask, .. } => {
            if *w <= 0 || *h <= 0 {
                return false;
            }
            let source = (*w as u32, *h as u32);
            second == Some(source)
                && match mask {
                    Some(_) => third == Some(source),
                    None => third.is_none(),
                }
        }
        PipelineOp::CompositeModule { .. } => third == Some(current),
        PipelineOp::Composite { .. } => second == Some(current) && third == Some(current),
        PipelineOp::PutAlphaData { .. } => third == Some(current),
        PipelineOp::AlphaComposite { .. } => second == Some(current),
        _ => match (second, third) {
            (None, None) => true,
            (second, third) => {
                second
                    .map(|dimensions| dimensions == current)
                    .unwrap_or(true)
                    && third
                        .map(|dimensions| dimensions == current)
                        .unwrap_or(true)
            }
        },
    }
}

/// Check dimensions before creating a device or uploading any image data.
/// Empty images and zero-sized GPU outputs are valid CPU/Pillow states, but
/// they are not valid storage-buffer dispatches: sampling kernels subtract one
/// from source dimensions and readback needs a non-empty copy range. Operations
/// whose host/shader output-size contract is incomplete are deliberately kept
/// on the CPU until that contract is explicit.
fn gpu_dimensions_require_cpu(ops: &[PipelineOp], image: &DynamicImage) -> bool {
    let dimensions_fit = |w: u32, h: u32| {
        CheckedDims::new(w, h, 1)
            .map(|dims| dims.total_pixels() <= GPU_BUFFER_CAPACITY as usize)
            .unwrap_or(false)
    };

    if !gpu_image_layout_is_supported(image) {
        return true;
    }
    let source_mode = mode_code(image);
    if ops.iter().any(|op| {
        matches!(op, PipelineOp::Eval { .. } | PipelineOp::PointOp { .. })
            && extract_lut(op, source_mode).is_none()
    }) {
        return true;
    }
    let (mut cur_w, mut cur_h) = image.dimensions();
    if cur_w == 0 || cur_h == 0 || !dimensions_fit(cur_w, cur_h) {
        return true;
    }
    for op in ops {
        if gpu_operation_mode_requires_cpu(op, image) {
            return true;
        }
        // CPU PutPixel reports IndexError for an out-of-range coordinate;
        // the shader would silently perform no write. Route that case to the
        // CPU before device initialization so error semantics are preserved.
        if let PipelineOp::PutPixel { x, y, .. } = op {
            if *x >= cur_w || *y >= cur_h {
                return true;
            }
        }
        let next = match op_output_dims(op, cur_w, cur_h) {
            Some(dimensions) => dimensions,
            None if op_has_explicit_output_dimensions(op) => return true,
            None => (cur_w, cur_h),
        };
        if next.0 == 0 || next.1 == 0 || !dimensions_fit(next.0, next.1) {
            return true;
        }
        if gpu_shader_work_requires_cpu(op, (cur_w, cur_h), next) {
            return true;
        }
        (cur_w, cur_h) = next;
    }
    false
}

/// Some shader math is layout-safe but still differs from Pillow for a
/// particular native mode. Keep this check beside the dimension preflight so
/// those cases fall back before any device or upload work begins.
fn gpu_operation_mode_requires_cpu(op: &PipelineOp, image: &DynamicImage) -> bool {
    match op {
        // AlphaComposite's public implementation is defined for LA/RGBA
        // canvases. On L/RGB, the CPU operation promotes to RGBA while the
        // packed GPU result would otherwise be fed through preserve_mode.
        PipelineOp::AlphaComposite { .. } => !matches!(
            image,
            DynamicImage::ImageLumaA8(_) | DynamicImage::ImageRgba8(_)
        ),
        // ImageChops.blend intentionally converts through RGB and restores
        // an opaque alpha for an alpha-bearing source. Image.blend (the
        // module operation) blends every stored channel, including alpha, so
        // its shader is exact only for the non-alpha byte modes.
        PipelineOp::BlendModule { .. } => matches!(
            image,
            DynamicImage::ImageLumaA8(_) | DynamicImage::ImageRgba8(_)
        ),
        // PutData and PutAlpha carry the logical source/target layout in the
        // operation. A direct core caller can construct a mismatched pair;
        // CPU converts according to that mode, while the packed shader would
        // otherwise reinterpret the existing storage in place.
        PipelineOp::PutData { mode, .. } | PipelineOp::PutAlpha { mode, .. } => {
            !pixel_mode_matches_image(*mode, image)
        }
        // The CPU ImageOps implementation runs Posterize/Solarize through
        // an RGB temporary and preserve_mode, which makes alpha opaque for
        // LA/RGBA. The packed shaders currently retain alpha, so keep those
        // native alpha cases on CPU until the public operation is normalized.
        PipelineOp::Posterize { .. } | PipelineOp::Solarize { .. } => matches!(
            image,
            DynamicImage::ImageLumaA8(_) | DynamicImage::ImageRgba8(_)
        ),
        // getchannel raises IndexError for a band that the source mode does
        // not have; a shader would read byte 3 or an unused packed byte.
        PipelineOp::ExtractBand { index } => {
            usize::from(*index) >= usize::from(image.color().channel_count())
        }
        _ => false,
    }
}

fn pixel_mode_matches_image(mode: PixelMode, image: &DynamicImage) -> bool {
    matches!(
        (mode, image),
        (PixelMode::L, DynamicImage::ImageLuma8(_))
            | (PixelMode::LA, DynamicImage::ImageLumaA8(_))
            | (PixelMode::RGB, DynamicImage::ImageRgb8(_))
            | (PixelMode::RGBA, DynamicImage::ImageRgba8(_))
    )
}

fn gpu_pipeline_requires_cpu(
    ops: &[PipelineOp],
    image: &DynamicImage,
    auxiliary_images: &[AuxiliaryImages],
) -> bool {
    if ops.len() != auxiliary_images.len() {
        return true;
    }
    if gpu_dimensions_require_cpu(ops, image) {
        return true;
    }
    let dimensions_fit = |w: u32, h: u32| {
        CheckedDims::new(w, h, 1)
            .map(|dims| dims.total_pixels() <= GPU_BUFFER_CAPACITY as usize)
            .unwrap_or(false)
    };
    if auxiliary_images.iter().any(|auxiliary| {
        auxiliary
            .second
            .iter()
            .chain(auxiliary.third.iter())
            .any(|image| {
                let (w, h) = image.dimensions();
                !gpu_image_layout_is_supported(image) || w == 0 || h == 0 || !dimensions_fit(w, h)
            })
    }) {
        return true;
    }
    let (mut cur_w, mut cur_h) = image.dimensions();
    for (index, op) in ops.iter().enumerate() {
        if !gpu_auxiliary_shapes_are_safe(op, &auxiliary_images[index], cur_w, cur_h) {
            return true;
        }
        if !gpu_auxiliary_modes_are_safe(op, image, &auxiliary_images[index]) {
            return true;
        }
        if let Some(next) = op_output_dims(op, cur_w, cur_h) {
            (cur_w, cur_h) = next;
        }
    }
    false
}

/// Multi-input shaders use the primary mode word for every packed sample.
/// Require an auxiliary source to have the same native byte layout; otherwise
/// a legal CPU operation such as RGB-vs-L would be interpreted as unrelated
/// channels on GPU. Paste masks have a separate luma/alpha contract and are
/// checked below against the channel the shader actually samples.
fn gpu_auxiliary_modes_are_safe(
    op: &PipelineOp,
    image: &DynamicImage,
    auxiliary: &AuxiliaryImages,
) -> bool {
    let requires_matching_source = matches!(
        op,
        PipelineOp::Add { .. }
            | PipelineOp::Subtract { .. }
            | PipelineOp::Multiply { .. }
            | PipelineOp::Screen { .. }
            | PipelineOp::Darker { .. }
            | PipelineOp::Lighter { .. }
            | PipelineOp::Difference { .. }
            | PipelineOp::AddModulo { .. }
            | PipelineOp::SubtractModulo { .. }
            | PipelineOp::LogicalAnd { .. }
            | PipelineOp::LogicalOr { .. }
            | PipelineOp::LogicalXor { .. }
            | PipelineOp::Blend { .. }
            | PipelineOp::BlendModule { .. }
            | PipelineOp::Paste { .. }
            | PipelineOp::AlphaComposite { .. }
    );
    if requires_matching_source
        && auxiliary
            .second
            .as_ref()
            .is_some_and(|second| second.color() != image.color())
    {
        return false;
    }
    if let PipelineOp::Paste {
        mask_alpha: false, ..
    } = op
    {
        // Paste's luma-mask path calls DynamicImage::to_luma8(), which uses
        // weighted RGB conversion for RGB/RGBA masks. The shader samples byte
        // 0 directly, so only native L/LA masks are exact.
        return !auxiliary.third.as_ref().is_some_and(|mask| {
            !matches!(
                mask.as_ref(),
                DynamicImage::ImageLuma8(_) | DynamicImage::ImageLumaA8(_)
            )
        });
    }
    true
}

/// Return the largest packed RGBA image that a batch needs. Image storage is
/// batch-owned, so allocating to the batch's actual high-water mark avoids
/// reserving the global maximum for every concurrent lazy pipeline.
fn gpu_batch_capacity(
    ops: &[PipelineOp],
    image: &DynamicImage,
    auxiliary_images: &[AuxiliaryImages],
) -> Result<u32, PilError> {
    if ops.len() != auxiliary_images.len() {
        return Err(PilError::InternalError(
            "GPU operation and auxiliary-image counts differ".into(),
        ));
    }

    let pixels = |(w, h): (u32, u32)| -> Result<usize, PilError> {
        Ok(CheckedDims::new(w, h, 1)?.total_pixels())
    };
    let mut high_water = pixels(image.dimensions())?;
    for auxiliary in auxiliary_images {
        for nested in auxiliary
            .second
            .as_ref()
            .into_iter()
            .chain(auxiliary.third.as_ref())
        {
            high_water = high_water.max(pixels(nested.dimensions())?);
        }
    }

    let (mut cur_w, mut cur_h) = image.dimensions();
    for op in ops {
        let (out_w, out_h) = match op_output_dims(op, cur_w, cur_h) {
            Some(dimensions) => dimensions,
            None if op_has_explicit_output_dimensions(op) => {
                return Err(PilError::ValueError(format!(
                    "GPU operation '{}' has no safe output dimensions",
                    registry::variant_key(op)
                )));
            }
            None => (cur_w, cur_h),
        };
        high_water = high_water.max(pixels((out_w, out_h))?);
        if let PipelineOp::PutData { data, mode } = op {
            high_water = high_water.max(data.len().div_ceil(mode.channels()));
        }
        (cur_w, cur_h) = (out_w, out_h);
    }

    if high_water == 0 || high_water > GPU_BUFFER_CAPACITY as usize {
        return Err(PilError::ValueError(format!(
            "GPU batch requires {} pixels; supported capacity is {}",
            high_water, GPU_BUFFER_CAPACITY
        )));
    }
    Ok(high_water as u32)
}

/// Check the size of each batch-owned image buffer against the selected
/// adapter's actual limits before calling `Device::create_buffer`.
///
/// The static pixel cap is intentionally conservative, but keeping this
/// check dynamic prevents a future cap change or an adapter with narrower
/// limits from turning a valid public image into a device validation error.
fn gpu_buffer_capacity_exceeds_limits(
    capacity: u32,
    max_storage_buffer_binding_size: u32,
    max_buffer_size: u64,
) -> bool {
    let Some(bytes) = u64::from(capacity).checked_mul(4) else {
        return true;
    };
    bytes > u64::from(max_storage_buffer_binding_size) || bytes > max_buffer_size
}

/// Validate the dispatch grid against the adapter limit. Pixel-count limits
/// alone are insufficient: a very wide, short image can fit in storage while
/// still requiring more workgroups in one dimension than the device accepts.
fn gpu_dispatch_dimensions_require_cpu(
    ops: &[PipelineOp],
    image_dimensions: (u32, u32),
    max_workgroups_per_dimension: u32,
) -> bool {
    if max_workgroups_per_dimension == 0 {
        return true;
    }
    let (mut cur_w, mut cur_h) = image_dimensions;
    if cur_w == 0 || cur_h == 0 {
        return true;
    }
    for op in ops {
        let next = match op_output_dims(op, cur_w, cur_h) {
            Some(dimensions) => dimensions,
            None if op_has_explicit_output_dimensions(op) => return true,
            None => (cur_w, cur_h),
        };
        // The rolling blur shaders are deliberately 1x1 workgroups: one
        // invocation owns a complete row or column. Their dispatch grid is
        // therefore (1, height) for the horizontal pass and (width, 1) for
        // the vertical pass, unlike the ordinary 16x16 kernels. Checking
        // only ceil(dim / 16) would admit a tall, narrow image and let the
        // later blur dispatch exceed the adapter's per-dimension limit.
        let dispatch_exceeds_limit = if matches!(
            op,
            PipelineOp::BoxBlur { .. } | PipelineOp::GaussianBlur { .. }
        ) {
            next.1 > max_workgroups_per_dimension || next.0 > max_workgroups_per_dimension
        } else {
            next.0.div_ceil(16) > max_workgroups_per_dimension
                || next.1.div_ceil(16) > max_workgroups_per_dimension
        };
        if next.0 == 0 || next.1 == 0 || dispatch_exceeds_limit {
            return true;
        }
        (cur_w, cur_h) = next;
    }
    false
}

/// Keep finite-but-expensive kernels below a conservative watchdog budget.
/// The estimate counts the inner loop body per output pixel; kernels without
/// dynamic inner work return false and remain eligible for GPU dispatch.
fn gpu_shader_work_items(
    op: &PipelineOp,
    source_dimensions: (u32, u32),
    output_dimensions: (u32, u32),
) -> Option<u64> {
    match op {
        // The convolution shaders intentionally load a complete interior
        // neighborhood. Small images are valid Pillow inputs, but have no
        // interior pixels; keep them on the scalar path for exact border
        // semantics and to avoid relying on unsigned underflow behavior.
        PipelineOp::Filter3x3 { .. } if source_dimensions.0 < 3 || source_dimensions.1 < 3 => {
            return None;
        }
        PipelineOp::Filter5x5 { .. } if source_dimensions.0 < 5 || source_dimensions.1 < 5 => {
            return None;
        }
        _ => {}
    }
    let output_pixels = u64::from(output_dimensions.0) * u64::from(output_dimensions.1);
    let (source_w, source_h) = source_dimensions;
    let inner_work = match op {
        PipelineOp::BoxBlur { radius } => {
            let _ = radius;
            // One remove/add update per pixel in each of the horizontal and
            // vertical rolling passes. Count four channels plus edge reads;
            // the radius-sized initialization is paid once per row/column.
            24
        }
        PipelineOp::GaussianBlur { sigma } => {
            let _ = sigma;
            // GaussianBlur expands to three horizontal and three vertical
            // rolling passes. The estimate is radius-independent; each
            // pass advances one window per output pixel and pays its
            // radius-sized initialization once per row/column.
            72
        }
        PipelineOp::MedianFilter { size } | PipelineOp::RankFilter { size, .. } => {
            // These shaders insertion-sort four channel arrays. The sort is
            // quadratic in the window area, so size² alone understates the
            // work by orders of magnitude and can admit watchdog-triggering
            // dispatches for the supported 9x9 maximum.
            let area = u64::from(*size).saturating_mul(u64::from(*size));
            area.saturating_mul(area).saturating_mul(4)
        }
        PipelineOp::MaxFilter { size } | PipelineOp::MinFilter { size } => u64::from(*size)
            .saturating_mul(u64::from(*size))
            .saturating_mul(4),
        PipelineOp::Filter3x3 { .. } => 9,
        PipelineOp::Filter5x5 { .. } => 25,
        PipelineOp::Reduce { x_factor, y_factor } => {
            let block_w = u64::from((*x_factor).max(1).min(source_w.max(1)));
            let block_h = u64::from((*y_factor).max(1).min(source_h.max(1)));
            block_w.saturating_mul(block_h)
        }
        PipelineOp::EffectMandelbrot { quality, .. } => u64::from(*quality),
        PipelineOp::PutData { mode, .. } => mode.channels() as u64,
        PipelineOp::Pad { .. } => 4,
        // Even a constant-work shader consumes one invocation per output
        // pixel. Count that work so a long point-operation chain is split
        // before its cumulative dispatch cost can monopolize one submission.
        _ => 1,
    };
    Some(output_pixels.saturating_mul(inner_work))
}

fn gpu_shader_work_requires_cpu(
    op: &PipelineOp,
    source_dimensions: (u32, u32),
    output_dimensions: (u32, u32),
) -> bool {
    gpu_shader_work_items(op, source_dimensions, output_dimensions)
        .is_none_or(|work| work > MAX_GPU_SHADER_WORK_ITEMS)
}

// ─── GpuPool ───────────────────────────────────────────────────────────────

/// GPU compute pool — wgpu-based compute shader dispatch.
///
/// Uses packed u32 RGBA and 16x16 workgroups. GPU is lazily initialized
/// on first execution. If wgpu is unavailable, execute_batch returns an error.
pub struct GpuPool;

fn gpu_operation_is_safe(op: &PipelineOp) -> bool {
    let finite_f32 = |value: f64| value.is_finite() && (value as f32).is_finite();
    match op {
        PipelineOp::Filter3x3 { kernel, scale, .. } => {
            let scale = f64::from(*scale);
            let denominator = if scale.abs() < 1e-10 { 1.0 } else { scale };
            scale.is_finite()
                && kernel
                    .iter()
                    .all(|coefficient| finite_f32(f64::from(*coefficient) / denominator))
        }
        PipelineOp::Filter5x5 { kernel, scale, .. } => {
            let scale = f64::from(*scale);
            let denominator = if scale.abs() < 1e-10 { 1.0 } else { scale };
            scale.is_finite()
                && kernel
                    .iter()
                    .all(|coefficient| finite_f32(f64::from(*coefficient) / denominator))
        }
        PipelineOp::GaussianBlur { sigma } => {
            if !sigma.is_finite() || *sigma < 0.0 {
                return false;
            }
            let radius = (*sigma * 3.0).ceil();
            radius.is_finite() && radius <= MAX_GPU_BLUR_RADIUS as f32
        }
        PipelineOp::BoxBlur { radius } => *radius <= MAX_GPU_BLUR_RADIUS,
        PipelineOp::MedianFilter { size }
        | PipelineOp::MaxFilter { size }
        | PipelineOp::MinFilter { size } => {
            *size >= 1 && *size <= MAX_GPU_FILTER_SIZE && *size % 2 == 1
        }
        PipelineOp::RankFilter { size, .. } => {
            *size >= 1 && *size <= MAX_GPU_FILTER_SIZE && *size % 2 == 1
        }
        PipelineOp::Reduce { x_factor, y_factor } => {
            *x_factor <= MAX_GPU_REDUCE_FACTOR && *y_factor <= MAX_GPU_REDUCE_FACTOR
        }
        // The CPU implementation shifts by `8 - bits`; zero is not a valid
        // direct PipelineOp value and would otherwise underflow that shift.
        // Public constructors already clamp to 1..=8, but keep the GPU
        // safety gate correct for callers that build the pipeline directly.
        PipelineOp::Posterize { bits } => (1..=8).contains(bits),
        PipelineOp::ExtractBand { index } => *index < 4,
        PipelineOp::Brightness { factor }
        | PipelineOp::Contrast { factor }
        | PipelineOp::ColorSaturation { factor } => {
            factor.is_finite() && (*factor == 0.0 || *factor == 1.0)
        }
        PipelineOp::Sharpness { factor } => factor.is_finite() && *factor == 1.0,
        PipelineOp::Add { scale, offset, .. } | PipelineOp::Subtract { scale, offset, .. } => {
            // The shader transports these values as f32, whereas Pillow's
            // public calculation is f64.  Restrict GPU dispatch to the exact
            // unit-divisor/integral-offset subset; all other valid requests
            // remain eligible for the CPU fallback.
            *scale == 1.0
                && finite_f32(*offset)
                && (*offset as f32) as f64 == *offset
                && offset.fract() == 0.0
        }
        PipelineOp::Blend { alpha, .. } | PipelineOp::BlendModule { alpha, .. } => {
            alpha.is_finite() && (*alpha == 0.0 || *alpha == 1.0)
        }
        PipelineOp::Scale { factor, .. } => {
            factor.is_finite()
                && *factor > 0.0
                && (*factor * 65536.0).is_finite()
                && *factor * 65536.0 >= 1.0
                && *factor * 65536.0 <= MAX_GPU_SCALE_FIXED_POINT
        }
        PipelineOp::Rotate {
            angle,
            center,
            translate,
            ..
        } => {
            angle.is_finite()
                && center
                    .map(|(x, y)| x.is_finite() && y.is_finite())
                    .unwrap_or(true)
                && translate
                    .map(|(x, y)| x.is_finite() && y.is_finite())
                    .unwrap_or(true)
        }
        PipelineOp::Transform {
            w, h, method, data, ..
        } => {
            *w > 0
                && *h > 0
                && matches!(method, TransformMethod::Affine)
                && data.len() >= 6
                && data[..6].iter().copied().all(finite_f32)
        }
        PipelineOp::AlphaComposite { dest, src, .. } => *dest == (0, 0) && *src == (0, 0),
        PipelineOp::Autocontrast { cutoff, .. } => finite_f32(*cutoff),
        PipelineOp::EffectMandelbrot {
            w,
            h,
            x0,
            y0,
            x1,
            y1,
            quality,
        } => {
            *w > 0
                && *h > 0
                && *quality >= 1
                && *quality <= MAX_GPU_MANDELBROT_ITERS
                && [*x0, *y0, *x1, *y1].into_iter().all(finite_f32)
        }
        _ => true,
    }
}

fn validate_gpu_operations(ops: &[PipelineOp]) -> Result<(), PilError> {
    for op in ops {
        if !gpu_operation_is_safe(op) {
            return Err(PilError::ValueError(format!(
                "GPU operation '{}' exceeds the bounded shader safety limits",
                registry::variant_key(op)
            )));
        }
    }
    Ok(())
}

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
            None => true,
        };
        Ok(healthy && gpu_operation_is_safe(op) && registry::gpu_supports(op)?)
    }

    fn execute_batch(
        &self,
        ops: &[PipelineOp],
        img: &DynamicImage,
        mode: Option<&str>,
    ) -> Result<DynamicImage, PilError> {
        if ops.is_empty() {
            return Ok(img.clone());
        }

        // Keep the public operation list intact for routing and parity, but
        // execute contiguous exact point runs as one LUT dispatch when the
        // source already has a packed native layout.  Explicit logical modes
        // retain their existing path because their byte interpretation is not
        // represented by the batch-wide GPU mode word.
        let mut dispatch_ops = if mode.is_none() && gpu_image_layout_is_supported(img) {
            fuse_gpu_point_ops(ops, mode_code(img))
        } else {
            ops.to_vec()
        };
        if mode.is_none() && gpu_image_layout_is_supported(img) {
            dispatch_ops = fuse_gpu_transpose_ops(&dispatch_ops, img.width(), img.height());
        }
        let ops = dispatch_ops.as_slice();

        // GPU shaders consume the packed L/LA/RGB/RGBA representation. An
        // explicit Pillow mode such as P, PA, I, F, CMYK, or 1 carries a
        // different sample contract even when the transport buffer happens
        // to be four bytes wide; use the CPU implementation rather than
        // silently interpreting those samples as RGBA.
        if mode.is_some_and(|mode| !matches!(mode, "L" | "LA" | "RGB" | "RGBA")) {
            gpu_log!("[GPU] dispatch preflight routed batch to CPU: unsupported logical mode");
            crate::compute::record_pipeline_backend_fallback(
                "GPU preflight: unsupported logical mode",
            );
            let cpu = crate::compute::CpuPool;
            return cpu.execute_batch(ops, img, mode);
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
            crate::compute::record_pipeline_backend_fallback(
                "GPU preflight: non-terminal mode change",
            );
            let cpu = crate::compute::CpuPool;
            return cpu.execute_batch(ops, img, mode);
        }

        if !gpu_image_layout_is_supported(img) {
            gpu_log!(
                "[GPU] dispatch preflight routed batch to CPU: unsupported native pixel layout"
            );
            crate::compute::record_pipeline_backend_fallback(
                "GPU preflight: unsupported native pixel layout",
            );
            let cpu = crate::compute::CpuPool;
            return cpu.execute_batch(ops, img, mode);
        }

        for op in ops {
            if !registry::gpu_supports(op)? {
                return Err(PilError::ValueError(format!(
                    "GPU operation '{}' has no valid single-dispatch shader contract",
                    registry::variant_key(op)
                )));
            }
        }

        // Keep this guard at execution time as well as in `supports`: explicit
        // backend selection and future callers must not bypass shader bounds.
        validate_gpu_operations(ops)?;

        // Check the primary image and every declared output before resolving
        // nested images. An empty/oversized outer image must not initialize a
        // device merely because a later auxiliary pipeline is present; the
        // entire batch will be handled by the CPU fallback.
        if gpu_dimensions_require_cpu(ops, img) {
            gpu_log!(
                "[GPU] dispatch preflight routed batch to CPU: unsafe primary image dimensions"
            );
            crate::compute::record_pipeline_backend_fallback(
                "GPU preflight: unsafe primary image dimensions",
            );
            let cpu = crate::compute::CpuPool;
            return cpu.execute_batch(ops, img, mode);
        }

        // Resolve every nested image before starting GPU work. A nested
        // explicitly locked pipeline may itself need the GPU pool, and Pillow
        // surfaces that materialization failure instead of dispatching the
        // outer shader with an empty auxiliary buffer.
        let auxiliary_images = ops
            .iter()
            .map(extract_auxiliary_images)
            .collect::<Result<Vec<_>, _>>()?;

        if gpu_pipeline_requires_cpu(ops, img, &auxiliary_images) {
            gpu_log!(
                "[GPU] dispatch preflight routed batch to CPU: unsafe or incomplete image dimensions"
            );
            crate::compute::record_pipeline_backend_fallback(
                "GPU preflight: unsafe or incomplete image dimensions",
            );
            let cpu = crate::compute::CpuPool;
            return cpu.execute_batch(ops, img, mode);
        }

        let gpu = Self::ensure_init()?;
        gpu.ensure_healthy("GPU batch start")?;
        if gpu_dispatch_dimensions_require_cpu(
            ops,
            img.dimensions(),
            gpu.device.limits().max_compute_workgroups_per_dimension,
        ) {
            gpu_log!("[GPU] dispatch preflight routed batch to CPU: adapter workgroup limit");
            crate::compute::record_pipeline_backend_fallback(
                "GPU preflight: adapter workgroup limit",
            );
            let cpu = crate::compute::CpuPool;
            return cpu.execute_batch(ops, img, mode);
        }
        let capacity = gpu_batch_capacity(ops, img, &auxiliary_images)?;
        let limits = gpu.device.limits();
        if gpu_buffer_capacity_exceeds_limits(
            capacity,
            limits.max_storage_buffer_binding_size,
            limits.max_buffer_size,
        ) {
            gpu_log!(
                "[GPU] dispatch preflight routed batch to CPU: image buffer exceeds adapter limits"
            );
            crate::compute::record_pipeline_backend_fallback(
                "GPU preflight: image buffer exceeds adapter limits",
            );
            let cpu = crate::compute::CpuPool;
            return cpu.execute_batch(ops, img, mode);
        }
        let mut buffers = gpu.acquire_buffers(capacity)?;
        let rgba = img.to_rgba8();
        let (w, h) = rgba.dimensions();
        let mcode = mode_code(img);
        let op_keys: Vec<&str> = ops.iter().map(|op| registry::variant_key(op)).collect();
        log::debug!(
            "[GPU] {} op(s) {}x{} mode={}: {:?}",
            ops.len(),
            w,
            h,
            mcode,
            op_keys
        );
        gpu_log!("[GPU] step=upload_rgba start");
        buffers.upload_rgba(&gpu.queue, &rgba)?;
        gpu_log!("[GPU] step=upload_rgba done");
        gpu_log!("[GPU] step=execute_batch_impl start");
        let (final_is_a, final_w, final_h, staging, mut resource_telemetry, dispatch_count) =
            gpu.execute_batch_impl(ops, &auxiliary_images, w, h, mcode, &mut buffers)?;
        gpu_log!(
            "[GPU] step=execute_batch_impl done final=({},{}) is_a={}",
            final_w,
            final_h,
            final_is_a
        );
        // The final copy is recorded in the final compute command buffer, so
        // the lazy pipeline performs one readback submission after all GPU
        // operations instead of creating a second command buffer/submit pair.
        gpu_log!("[GPU] step=readback start");
        let result = gpu.readback_to_image(final_w, final_h, &staging.buffer)?;
        gpu_log!("[GPU] step=readback done");
        // map_async completion proves the command buffer no longer uses the
        // working images. Return successful working sets to the bounded pool;
        // every error path drops its buffers instead of risking reuse of an
        // in-flight or device-invalid resource.
        resource_telemetry.upload_bytes = CheckedDims::new(w, h, 4)?.total_bytes() as u64;
        resource_telemetry.readback_bytes =
            CheckedDims::new(final_w, final_h, 4)?.total_bytes() as u64;
        resource_telemetry.retained_cache_bytes = buffers.retained_bytes();
        resource_telemetry.full_frame_copy_count = 2;
        resource_telemetry.mode_conversion_count =
            u64::from(!matches!(img, DynamicImage::ImageRgba8(_)));
        crate::compute::record_pipeline_resource_telemetry(resource_telemetry);
        crate::compute::record_pipeline_dispatch_count(dispatch_count);
        gpu.recycle_staging(staging);
        gpu.recycle_buffers(buffers);
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
mod tests {
    use super::{
        GpuPool, MAX_GPU_BLUR_RADIUS, MAX_GPU_FILTER_SIZE, MAX_GPU_MANDELBROT_ITERS,
        MAX_GPU_OPS_PER_SUBMISSION, MAX_GPU_REDUCE_FACTOR, MAX_GPU_RESOURCE_BYTES_PER_SUBMISSION,
        MAX_GPU_SHADER_WORK_ITEMS, MAX_RETAINED_GPU_WORKING_BYTES, PixelMode, append_arena_slice,
        gpu_auxiliary_modes_are_safe, gpu_batch_capacity, gpu_batch_has_nonterminal_mode_change,
        gpu_buffer_capacity_exceeds_limits, gpu_dispatch_dimensions_require_cpu,
        gpu_image_layout_is_supported, gpu_operation_is_safe, gpu_operation_mode_requires_cpu,
        gpu_output_color_type, gpu_pipeline_requires_cpu, gpu_result_as_color_type,
        gpu_shader_work_requires_cpu, gpu_working_set_bytes, is_output_only_shader, op_output_dims,
        pack_put_data, select_gpu_chunk_end,
    };
    use crate::compute::BackendImpl;
    use crate::pipeline::{
        ColorMode, PipelineOp, ResampleFilter, TransformMethod, TransposeMethod,
    };
    use crate::raster::{
        ColorType, DynamicImage, GenericImageView, GrayAlphaImage, RgbImage, Rgba, RgbaImage,
    };

    #[test]
    fn gpu_chunking_never_queues_one_submission_per_operation() {
        let resource_bytes = vec![1024; MAX_GPU_OPS_PER_SUBMISSION * 2 + 1];
        let shader_work = vec![1; resource_bytes.len()];
        assert_eq!(
            select_gpu_chunk_end(0, &resource_bytes, &shader_work),
            Some(MAX_GPU_OPS_PER_SUBMISSION)
        );
        assert_eq!(
            select_gpu_chunk_end(MAX_GPU_OPS_PER_SUBMISSION, &resource_bytes, &shader_work),
            Some(MAX_GPU_OPS_PER_SUBMISSION * 2)
        );
    }

    #[test]
    fn gpu_chunking_splits_on_resource_arena_limit() {
        let resource_bytes = vec![MAX_GPU_RESOURCE_BYTES_PER_SUBMISSION / 2 + 1; 3];
        let shader_work = vec![1; resource_bytes.len()];
        assert_eq!(
            select_gpu_chunk_end(0, &resource_bytes, &shader_work),
            Some(1)
        );
        assert_eq!(
            select_gpu_chunk_end(1, &resource_bytes, &shader_work),
            Some(2)
        );
    }

    #[test]
    fn gpu_chunking_splits_on_cumulative_shader_work() {
        let resource_bytes = vec![1024; 3];
        let shader_work = vec![MAX_GPU_SHADER_WORK_ITEMS / 2 + 1; 3];
        assert_eq!(
            select_gpu_chunk_end(0, &resource_bytes, &shader_work),
            Some(1)
        );
        assert_eq!(
            select_gpu_chunk_end(1, &resource_bytes, &shader_work),
            Some(2)
        );
    }

    #[test]
    fn gpu_chunking_rejects_mismatched_estimate_vectors() {
        assert_eq!(select_gpu_chunk_end(0, &[1024], &[]), None);
    }

    #[test]
    fn gpu_working_buffer_retention_has_a_hard_byte_bound() {
        assert!(gpu_working_set_bytes(1024 * 1024) < MAX_RETAINED_GPU_WORKING_BYTES);
        assert!(gpu_working_set_bytes(4096 * 4096) > MAX_RETAINED_GPU_WORKING_BYTES);
    }

    #[test]
    fn gpu_arenas_keep_each_operation_aligned_and_isolated() {
        let mut arena = Vec::new();
        let first = append_arena_slice(&mut arena, &[1, 2, 3], 256);
        let second = append_arena_slice(&mut arena, &[4, 5], 256);

        assert_eq!(first.offset % 256, 0);
        assert_eq!(second.offset % 256, 0);
        assert_eq!(first.size, 256);
        assert_eq!(second.size, 256);
        assert_eq!(&arena[0..3], &[1, 2, 3]);
        assert_eq!(&arena[64..66], &[4, 5]);
    }

    #[test]
    fn gpu_put_data_packing_rejects_capacity_overflow() {
        let error = pack_put_data(&[1, 2, 3, 4, 5, 6, 7, 8], PixelMode::RGBA, 1)
            .expect_err("two RGBA pixels must exceed one-pixel capacity");
        assert!(error.to_string().contains("putdata image size"));
    }

    #[test]
    fn gpu_dynamic_shader_parameters_are_bounded() {
        assert!(gpu_operation_is_safe(&PipelineOp::GaussianBlur {
            sigma: 2.0
        }));
        assert!(!gpu_operation_is_safe(&PipelineOp::GaussianBlur {
            sigma: f32::NAN
        }));
        assert!(!gpu_operation_is_safe(&PipelineOp::GaussianBlur {
            sigma: MAX_GPU_BLUR_RADIUS as f32,
        }));
        assert!(!gpu_operation_is_safe(&PipelineOp::BoxBlur {
            radius: MAX_GPU_BLUR_RADIUS + 1,
        }));
        assert!(gpu_operation_is_safe(&PipelineOp::MedianFilter {
            size: MAX_GPU_FILTER_SIZE,
        }));
        assert!(!gpu_operation_is_safe(&PipelineOp::MedianFilter {
            size: MAX_GPU_FILTER_SIZE + 2,
        }));
        assert!(!gpu_operation_is_safe(&PipelineOp::RankFilter {
            size: 8,
            rank: 0
        }));
        assert!(!gpu_operation_is_safe(&PipelineOp::Reduce {
            x_factor: MAX_GPU_REDUCE_FACTOR + 1,
            y_factor: 1,
        }));
        assert!(!gpu_operation_is_safe(&PipelineOp::EffectMandelbrot {
            w: 8,
            h: 8,
            x0: -2.0,
            y0: -1.0,
            x1: 1.0,
            y1: 1.0,
            quality: MAX_GPU_MANDELBROT_ITERS + 1,
        }));
        assert!(!gpu_operation_is_safe(&PipelineOp::Brightness {
            factor: 9000.0
        }));
        assert!(!gpu_operation_is_safe(&PipelineOp::ColorSaturation {
            factor: 9000.0
        }));
        assert!(!gpu_operation_is_safe(&PipelineOp::Sharpness {
            factor: -1.0
        }));
        assert!(!gpu_operation_is_safe(&PipelineOp::Add {
            other: std::sync::Arc::new(crate::image::Image::new_palette_index(1, 1, 0)),
            scale: f64::MAX,
            offset: 0.0,
        }));
        assert!(!gpu_operation_is_safe(&PipelineOp::Add {
            other: std::sync::Arc::new(crate::image::Image::new_palette_index(1, 1, 0)),
            scale: 0.0,
            offset: 0.0,
        }));
        assert!(!gpu_operation_is_safe(&PipelineOp::Subtract {
            other: std::sync::Arc::new(crate::image::Image::new_palette_index(1, 1, 0)),
            scale: 1.0e-15,
            offset: 0.0,
        }));
        assert!(!gpu_operation_is_safe(&PipelineOp::Add {
            other: std::sync::Arc::new(crate::image::Image::new_palette_index(1, 1, 0)),
            scale: 1.0,
            offset: 0.5,
        }));
        let exact_add_other = std::sync::Arc::new(crate::image::Image::new_palette_index(1, 1, 0));
        assert!(gpu_operation_is_safe(&PipelineOp::Add {
            other: exact_add_other.clone(),
            scale: 1.0,
            offset: -17.0,
        }));
        assert!(!gpu_operation_is_safe(&PipelineOp::Add {
            other: exact_add_other,
            scale: 2.0,
            offset: 0.0,
        }));
        assert!(!gpu_operation_is_safe(&PipelineOp::Posterize { bits: 9 }));
        assert!(!gpu_operation_is_safe(&PipelineOp::Posterize { bits: 0 }));
        assert!(!gpu_operation_is_safe(&PipelineOp::ExtractBand {
            index: 4
        }));
        assert!(!gpu_operation_is_safe(&PipelineOp::Scale {
            factor: 0.0,
            filter: ResampleFilter::Nearest,
        }));
        assert!(!gpu_operation_is_safe(&PipelineOp::Scale {
            factor: f64::NAN,
            filter: ResampleFilter::Nearest,
        }));
        assert!(!gpu_operation_is_safe(&PipelineOp::Scale {
            factor: f64::from(u32::MAX) / 65536.0 + 1.0,
            filter: ResampleFilter::Nearest,
        }));
        assert!(!gpu_operation_is_safe(&PipelineOp::Autocontrast {
            cutoff: f64::MAX,
            mask: None,
        }));
        assert!(!gpu_operation_is_safe(&PipelineOp::Transform {
            w: 8,
            h: 8,
            method: TransformMethod::Affine,
            data: vec![f64::MAX; 6].into(),
            filter: ResampleFilter::Nearest,
            fill: None,
            palette_fill: None,
        }));
        assert!(!gpu_operation_is_safe(&PipelineOp::EffectMandelbrot {
            w: 8,
            h: 8,
            x0: f64::MAX,
            y0: -1.0,
            x1: 1.0,
            y1: 1.0,
            quality: 10,
        }));
        assert!(gpu_operation_is_safe(&PipelineOp::Reduce {
            x_factor: 0,
            y_factor: 0,
        }));
    }

    #[test]
    fn generator_shader_bindings_are_classified_as_output_only() {
        for source in [
            include_str!("shaders/linear_gradient.wgsl"),
            include_str!("shaders/radial_gradient.wgsl"),
            include_str!("shaders/effect_mandelbrot.wgsl"),
        ] {
            assert!(is_output_only_shader(source));
        }
        assert!(!is_output_only_shader(include_str!("shaders/invert.wgsl")));
        assert!(include_str!("shaders/effect_mandelbrot.wgsl").contains("@workgroup_size(16, 16)"));
    }

    #[test]
    fn active_gpu_shaders_match_the_bounded_dispatch_contract() {
        let registry = super::registry::registry().expect("GPU registry must build");
        let active = registry
            .iter()
            .filter_map(|(key, entry)| entry.gpu_source.map(|source| (*key, source)))
            .collect::<Vec<_>>();

        assert_eq!(active.len(), 72, "active GPU shader denominator changed");
        for (key, source) in active {
            assert!(
                source.contains("@compute @workgroup_size(16, 16)"),
                "{key} must use the 16x16 dispatch shape assumed by GpuPool"
            );
            assert!(
                source.contains("gid.x") && source.contains("gid.y"),
                "{key} must guard its invocation coordinates before indexing"
            );
        }
    }

    #[test]
    fn active_gpu_shaders_parse_and_validate_without_an_adapter() {
        let registry = super::registry::registry().expect("GPU registry must build");
        for (key, entry) in registry {
            let Some(source) = entry.gpu_source else {
                continue;
            };
            let module = wgpu::naga::front::wgsl::parse_str(source)
                .unwrap_or_else(|error| panic!("{key} WGSL parse failed: {error:?}"));
            let mut validator = wgpu::naga::valid::Validator::new(
                wgpu::naga::valid::ValidationFlags::all(),
                wgpu::naga::valid::Capabilities::default(),
            );
            validator
                .validate(&module)
                .unwrap_or_else(|error| panic!("{key} WGSL validation failed: {error:?}"));
        }
        for (key, source) in [
            ("BlurH", include_str!("shaders/box_blur_h.wgsl")),
            ("BlurV", include_str!("shaders/box_blur_v.wgsl")),
        ] {
            let module = wgpu::naga::front::wgsl::parse_str(source)
                .unwrap_or_else(|error| panic!("{key} WGSL parse failed: {error:?}"));
            let mut validator = wgpu::naga::valid::Validator::new(
                wgpu::naga::valid::ValidationFlags::all(),
                wgpu::naga::valid::Capabilities::default(),
            );
            validator
                .validate(&module)
                .unwrap_or_else(|error| panic!("{key} WGSL validation failed: {error:?}"));
        }
    }

    #[test]
    fn active_dynamic_gpu_kernels_keep_explicit_loop_bounds() {
        let registry = super::registry::registry().expect("GPU registry must build");
        let expected_bounds = [
            ("BoxBlur", "min(params.radius, MAX_RADIUS)"),
            ("GaussianBlur", "MAX_RADIUS"),
            ("MedianFilter", "min(params.size, 9u)"),
            ("MaxFilter", "min(params.size, 9u)"),
            ("MinFilter", "min(params.size, 9u)"),
            ("RankFilter", "min(params.size, 9u)"),
            ("EffectMandelbrot", "min(params.max_iters, MAX_ITERS)"),
            ("PutData", "channel_count(params.data_mode)"),
            ("Pad", "c < 4u"),
            ("Reduce", "if count == 0u"),
        ];

        for (key, marker) in expected_bounds {
            let source = registry
                .get(key)
                .and_then(|entry| entry.gpu_source)
                .unwrap_or_else(|| panic!("{key} must have an active shader"));
            assert!(
                source.contains(marker),
                "{key} must retain its loop-safety marker: {marker}"
            );
        }
        let reduce_source = registry
            .get("Reduce")
            .and_then(|entry| entry.gpu_source)
            .expect("Reduce must have an active shader");
        assert!(reduce_source.contains("MAX_FACTOR"));

        let dynamic_keys = expected_bounds
            .iter()
            .map(|(key, _)| *key)
            .collect::<std::collections::HashSet<_>>();
        for (key, entry) in registry {
            let Some(source) = entry.gpu_source else {
                continue;
            };
            let has_dynamic_loop =
                source.contains("for (") || source.contains("while ") || source.contains("loop {");
            assert_eq!(
                has_dynamic_loop,
                dynamic_keys.contains(key),
                "unreviewed dynamic loop found in active shader {key}"
            );
        }
    }

    #[test]
    fn active_extract_band_shader_matches_its_uniform_contract() {
        let source = super::registry::gpu_shader_source_for_key("ExtractBand")
            .expect("registry lookup must succeed")
            .expect("ExtractBand must retain its shader source");
        assert!(source.contains("mode: u32"));
        assert!(source.contains("channel: u32"));
        assert!(source.contains("params.channel"));
        assert!(source.contains("params.mode"));
        assert!(source.contains("params.mode == 1u && params.channel == 1u"));
    }

    #[test]
    fn active_sharpness_identity_skips_the_convolution() {
        let source = super::registry::gpu_shader_source_for_key("Sharpness")
            .expect("registry lookup must succeed")
            .expect("Sharpness must retain its shader source");
        let identity = source
            .find("if params.factor == 1000u")
            .expect("Sharpness must return early for its supported identity factor");
        let first_neighborhood_load = source
            .find("let p0_0 = input[")
            .expect("Sharpness must retain its reviewed convolution");
        assert!(identity < first_neighborhood_load);
    }

    #[test]
    fn offset_params_preserve_full_signed_coordinates() {
        let params = super::registry::extract_params(&PipelineOp::Offset {
            x: -2_000_000_001,
            y: 131_073,
        });
        assert_eq!(params, vec![(-2_000_000_001i32) as u32, 131_073]);

        let source = super::registry::gpu_shader_source_for_key("Offset")
            .expect("registry lookup must succeed")
            .expect("Offset must retain its shader source");
        assert!(source.contains("bitcast<i32>(dx_bits) < 0i"));
        assert!(source.contains("0u - dx_bits"));
        assert!(source.contains("dx_magnitude = dx_magnitude % w"));
        assert!(source.contains("sx = w - (dx_magnitude - gid.x)"));
        assert!(source.contains("sx = gid.x + dx_magnitude"));
        assert!(source.contains("gid.x >= w - dx_magnitude"));
    }

    #[test]
    fn geometry_shaders_guard_unsigned_dimension_arithmetic() {
        for (key, markers) in [
            ("Crop", ["params.left > src_w", "dx >= src_w - params.left"]),
            (
                "Expand",
                [
                    "(0xffffffffu - params.width) / 2u",
                    "out_w = params.width + 2u * b",
                ],
            ),
            (
                "CropBorder",
                ["b > params.width / 2u", "out_w = params.width - 2u * b"],
            ),
        ] {
            let source = super::registry::gpu_shader_source_for_key(key)
                .expect("registry lookup must succeed")
                .expect("geometry shader must be registered");
            for marker in markers {
                assert!(
                    source.contains(marker),
                    "{key} missing guard marker {marker}"
                );
            }
        }
    }

    #[test]
    fn add_and_subtract_shaders_match_the_division_contract() {
        for key in ["Add", "Subtract"] {
            let source = super::registry::gpu_shader_source_for_key(key)
                .expect("registry lookup must succeed")
                .expect("arithmetic shader must be registered");
            assert!(
                source.contains("/ scale + offset"),
                "{key} must divide by scale"
            );
            assert!(
                !source.contains("* scale + offset"),
                "{key} must not multiply by scale"
            );
        }
    }

    #[test]
    fn imagechops_shaders_apply_binary_operations_to_alpha() {
        for key in [
            "Add",
            "Subtract",
            "Multiply",
            "Screen",
            "Darker",
            "Lighter",
            "Difference",
            "AddModulo",
            "SubtractModulo",
            "LogicalAnd",
            "LogicalOr",
            "LogicalXor",
        ] {
            let source = super::registry::gpu_shader_source_for_key(key)
                .expect("registry lookup must succeed")
                .expect("ImageChops shader must be registered");
            assert!(source.contains("let ba"), "{key} must load other alpha");
            assert!(source.contains("out_a_raw"), "{key} must compute alpha");
            assert!(
                source.contains("select(255u, out_a_raw, mode_has_a"),
                "{key} must preserve opaque-mode alpha semantics"
            );
        }
        for key in ["Invert", "InvertChops"] {
            let source = super::registry::gpu_shader_source_for_key(key)
                .expect("registry lookup must succeed")
                .expect("invert shader must be registered");
            assert!(source.contains("select(255u, 255u - a, mode_has_a"));
        }
    }

    #[test]
    fn gpu_luma_shaders_use_pillow_fixed_point_coefficients() {
        for key in ["Convert", "Grayscale", "ColorSaturation"] {
            let source = super::registry::gpu_shader_source_for_key(key)
                .expect("registry lookup must succeed")
                .expect("luma shader must be registered");
            assert!(
                source.contains("19595u * r + 38470u * g + 7471u * b + 32768u"),
                "{key} must use Pillow's fixed-point luma"
            );
        }
    }

    #[test]
    fn blend_shaders_match_the_public_direction_and_alpha_contracts() {
        let blend = super::registry::gpu_shader_source_for_key("Blend")
            .expect("registry lookup must succeed")
            .expect("Blend must retain its shader source");
        assert!(blend.contains("ar * inv_alpha + br * alpha"));
        assert!(blend.contains("let out_a = 255u;"));

        let blend_module = super::registry::gpu_shader_source_for_key("BlendModule")
            .expect("registry lookup must succeed")
            .expect("BlendModule must retain its shader source");
        assert!(blend_module.contains("ar * inv_alpha + br * alpha"));
        assert!(blend_module.contains("let out_a_raw"));

        let alpha_composite = super::registry::gpu_shader_source_for_key("AlphaComposite")
            .expect("registry lookup must succeed")
            .expect("AlphaComposite must retain its shader source");
        assert!(alpha_composite.contains("if out_a_val == 0u"));
        assert!(alpha_composite.contains("return dst_pixel;"));

        let sharpness = super::registry::gpu_shader_source_for_key("Sharpness")
            .expect("registry lookup must succeed")
            .expect("Sharpness must retain its shader source");
        assert!(sharpness.contains("let out_a = select(255u, orig_a, mode_has_a"));
        let brightness = super::registry::gpu_shader_source_for_key("Brightness")
            .expect("registry lookup must succeed")
            .expect("Brightness must retain its shader source");
        assert!(brightness.contains("let out_g = val_g;"));
        assert!(brightness.contains("let out_b = val_b;"));
        assert!(sharpness.contains("let out_g = blend_fixed(blur_g_u, orig_g"));
        assert!(sharpness.contains("let out_b = blend_fixed(blur_b_u, orig_b"));
    }

    #[test]
    fn scale_shader_uses_rounded_output_dimensions_for_nearest_mapping() {
        let source = super::registry::gpu_shader_source_for_key("Scale")
            .expect("registry lookup must succeed")
            .expect("Scale must retain its shader source");
        assert!(source.contains("f32(src_w) / f32(params.dst_w)"));
        assert!(source.contains("f32(src_h) / f32(params.dst_h)"));
        assert!(!source.contains("1.0 / factor"));
    }

    #[test]
    fn lut_packing_preserves_band_major_eval_and_point_tables() {
        let mut la = vec![0u8; 512];
        for i in 0..256 {
            la[i] = i as u8;
            la[256 + i] = 255u8.saturating_sub(i as u8);
        }
        let packed = super::extract_lut(&PipelineOp::Eval { lut: la.into() }, 1)
            .expect("LA LUT must pack for the LA source mode");
        assert_eq!(packed[17] & 0xff, 17);
        assert_eq!((packed[17] >> 24) & 0xff, 238);

        let rgb = vec![7u8; 768];
        let packed = super::extract_lut(&PipelineOp::PointOp { lut: rgb.into() }, 2)
            .expect("RGB LUT must pack for the RGB source mode");
        assert_eq!(packed[0], 0xff_07_07_07);

        let image = DynamicImage::ImageLumaA8(crate::raster::GrayAlphaImage::new(1, 1));
        assert!(super::gpu_dimensions_require_cpu(
            &[PipelineOp::Eval {
                lut: vec![0; 256].into()
            }],
            &image,
        ));
    }

    #[test]
    fn blur_dispatches_keep_gaussian_separable_without_readback() {
        assert_eq!(
            super::GpuInner::blur_pass_count(&PipelineOp::BoxBlur { radius: 2 }),
            Some(1)
        );
        assert_eq!(
            super::GpuInner::blur_pass_count(&PipelineOp::GaussianBlur { sigma: 2.0 }),
            Some(3)
        );
        assert_eq!(super::GpuInner::blur_pass_count(&PipelineOp::Invert), None);
    }

    #[test]
    fn gpu_geometry_contract_tracks_swapped_and_partial_outputs() {
        assert_eq!(
            op_output_dims(
                &PipelineOp::Transpose {
                    method: TransposeMethod::Rotate90,
                },
                5,
                3,
            ),
            Some((3, 5))
        );
        assert_eq!(
            op_output_dims(
                &PipelineOp::Reduce {
                    x_factor: 2,
                    y_factor: 3,
                },
                5,
                7,
            ),
            Some((3, 3))
        );
        assert_eq!(
            op_output_dims(
                &PipelineOp::Resize {
                    w: 0,
                    h: 5,
                    filter: ResampleFilter::Nearest,
                },
                5,
                7,
            ),
            Some((0, 5))
        );
    }

    #[test]
    fn empty_images_are_preflighted_to_cpu_before_gpu_initialization() {
        let image = DynamicImage::ImageRgb8(RgbImage::new(0, 2));
        let ops = [PipelineOp::Invert];
        let auxiliary = [super::AuxiliaryImages {
            second: None,
            third: None,
        }];
        assert!(gpu_pipeline_requires_cpu(&ops, &image, &auxiliary));
        let result = GpuPool
            .execute_batch(&ops, &image, None)
            .expect("empty-image fallback should not require an adapter");
        assert_eq!(result.dimensions(), (0, 2));
    }

    #[test]
    fn malformed_auxiliary_metadata_is_rejected_without_indexing_past_ops() {
        let image = DynamicImage::ImageRgb8(RgbImage::new(1, 1));
        let ops = [PipelineOp::Invert];
        assert!(gpu_pipeline_requires_cpu(&ops, &image, &[]));
    }

    #[test]
    fn gpu_registry_rejects_incomplete_single_dispatch_contracts() {
        use crate::pipeline::DitherMethod;

        assert!(!super::registry::gpu_supports(&PipelineOp::Autocontrast {
            cutoff: 0.0,
            mask: None,
        })
        .unwrap());
        assert!(!super::registry::gpu_supports(&PipelineOp::Equalize).unwrap());
        assert!(
            !super::registry::gpu_supports(&PipelineOp::Quantize {
                colors: 16,
                dither: false,
            })
            .unwrap()
        );
        assert!(
            !super::registry::gpu_supports(&PipelineOp::Colorize {
                black: (0, 0, 0),
                white: (255, 255, 255),
                mid: None,
                blackpoint: 0,
                midpoint: 127,
                whitepoint: 255,
            })
            .unwrap()
        );
        assert!(!super::registry::gpu_supports(&PipelineOp::Contrast { factor: 1.0 }).unwrap());
        assert!(
            !super::registry::gpu_supports(&PipelineOp::Filter3x3 {
                kernel: [0.0; 9],
                scale: 1.0,
                offset: 0,
            })
            .unwrap()
        );
        assert!(
            !super::registry::gpu_supports(&PipelineOp::Filter5x5 {
                kernel: [0.0; 25],
                scale: 1.0,
                offset: 0,
            })
            .unwrap()
        );
        assert!(!super::registry::gpu_supports(&PipelineOp::Brightness { factor: 1.25 }).unwrap());
        assert!(
            !super::registry::gpu_supports(&PipelineOp::Brightness { factor: 1.2345 }).unwrap()
        );
        assert!(
            !super::registry::gpu_supports(&PipelineOp::ColorSaturation { factor: 0.5 }).unwrap()
        );
        assert!(
            !super::registry::gpu_supports(&PipelineOp::ColorSaturation { factor: 0.5005 })
                .unwrap()
        );
        assert!(!super::registry::gpu_supports(&PipelineOp::Sharpness { factor: 2.0 }).unwrap());
        assert!(!super::registry::gpu_supports(&PipelineOp::Sharpness { factor: 0.0 }).unwrap());
        assert!(!super::registry::gpu_supports(&PipelineOp::Sharpness { factor: 2.0005 }).unwrap());
        assert!(super::registry::gpu_supports(&PipelineOp::Brightness { factor: 1.0 }).unwrap());
        assert!(
            super::registry::gpu_supports(&PipelineOp::ColorSaturation { factor: 0.0 }).unwrap()
        );
        assert!(super::registry::gpu_supports(&PipelineOp::Sharpness { factor: 1.0 }).unwrap());
        let blend_other = std::sync::Arc::new(crate::image::Image::new_palette_index(1, 1, 0));
        assert!(
            !super::registry::gpu_supports(&PipelineOp::Blend {
                other: blend_other.clone(),
                alpha: 0.5,
            })
            .unwrap()
        );
        assert!(
            !super::registry::gpu_supports(&PipelineOp::BlendModule {
                other: blend_other,
                alpha: 0.5,
            })
            .unwrap()
        );
        let exact_blend_other =
            std::sync::Arc::new(crate::image::Image::new_palette_index(1, 1, 0));
        assert!(
            !super::registry::gpu_supports(&PipelineOp::Blend {
                other: exact_blend_other.clone(),
                alpha: 127.0 / 255.0,
            })
            .unwrap()
        );
        assert!(
            !super::registry::gpu_supports(&PipelineOp::BlendModule {
                other: exact_blend_other.clone(),
                alpha: 127.0 / 255.0,
            })
            .unwrap()
        );
        assert!(
            super::registry::gpu_supports(&PipelineOp::Blend {
                other: exact_blend_other.clone(),
                alpha: 0.0,
            })
            .unwrap()
        );
        assert!(
            super::registry::gpu_supports(&PipelineOp::BlendModule {
                other: exact_blend_other,
                alpha: 1.0,
            })
            .unwrap()
        );
        let exact_add_other = std::sync::Arc::new(crate::image::Image::new_palette_index(1, 1, 0));
        assert!(
            super::registry::gpu_supports(&PipelineOp::Add {
                other: exact_add_other.clone(),
                scale: 1.0,
                offset: -3.0,
            })
            .unwrap()
        );
        assert!(
            !super::registry::gpu_supports(&PipelineOp::Add {
                other: exact_add_other,
                scale: 2.0,
                offset: 0.0,
            })
            .unwrap()
        );
        let overlay_other = std::sync::Arc::new(crate::image::Image::new_palette_index(1, 1, 0));
        assert!(
            !super::registry::gpu_supports(&PipelineOp::Overlay {
                other: overlay_other.clone(),
            })
            .unwrap()
        );
        assert!(
            !super::registry::gpu_supports(&PipelineOp::HardLight {
                other: overlay_other.clone(),
            })
            .unwrap()
        );
        assert!(
            !super::registry::gpu_supports(&PipelineOp::SoftLight {
                other: overlay_other,
            })
            .unwrap()
        );
        assert!(
            !super::registry::gpu_supports(&PipelineOp::Resize {
                w: 4,
                h: 4,
                filter: ResampleFilter::Nearest,
            })
            .unwrap()
        );
        assert!(
            !super::registry::gpu_supports(&PipelineOp::Scale {
                factor: 2.0,
                filter: ResampleFilter::Bilinear,
            })
            .unwrap()
        );
        assert!(
            !super::registry::gpu_supports(&PipelineOp::Convert {
                mode: ColorMode::RGB,
                matrix: None,
                dither: Some(DitherMethod::None),
            })
            .unwrap()
        );
        assert!(
            !super::registry::gpu_supports(&PipelineOp::LinearGradient { mode: ColorMode::L })
                .unwrap()
        );
        assert!(
            !super::registry::gpu_supports(&PipelineOp::RadialGradient {
                mode: ColorMode::RGB,
            })
            .unwrap()
        );
        assert!(
            !super::registry::gpu_supports(&PipelineOp::EffectMandelbrot {
                w: 8,
                h: 8,
                x0: -2.0,
                y0: -1.0,
                x1: 1.0,
                y1: 1.0,
                quality: 10,
            })
            .unwrap()
        );
        let other = std::sync::Arc::new(crate::image::Image::new_palette_index(1, 1, 0));
        let mask = std::sync::Arc::new(crate::image::Image::new_palette_index(1, 1, 0));
        assert!(
            !super::registry::gpu_supports(&PipelineOp::Composite {
                other: other.clone(),
                mask: mask.clone(),
            })
            .unwrap()
        );
        assert!(
            !super::registry::gpu_supports(&PipelineOp::CompositeModule {
                other,
                mask,
                mask_alpha: false,
            })
            .unwrap()
        );
        assert!(
            !super::registry::gpu_supports(&PipelineOp::Pad {
                w: 4,
                h: 4,
                filter: ResampleFilter::Bilinear,
                color: None,
                centering: (0.5, 0.5),
            })
            .unwrap()
        );
        assert!(
            !super::registry::gpu_supports(&PipelineOp::Rotate {
                angle: 17.0,
                expand: false,
                fill: None,
                center: None,
                translate: None,
                nearest: false,
            })
            .unwrap()
        );
        assert!(
            !super::registry::gpu_supports(&PipelineOp::Transform {
                w: 4,
                h: 4,
                method: TransformMethod::Affine,
                data: vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0].into(),
                filter: ResampleFilter::Nearest,
                fill: None,
                palette_fill: None,
            })
            .unwrap()
        );
        assert!(
            !super::registry::gpu_supports(&PipelineOp::RemapPalette {
                dest_map: vec![0; 256].into(),
            })
            .unwrap()
        );
        assert!(
            !super::registry::gpu_supports(&PipelineOp::Resize {
                w: 4,
                h: 4,
                filter: ResampleFilter::Bilinear,
            })
            .unwrap()
        );
        assert!(
            !super::registry::gpu_supports(&PipelineOp::Resize {
                w: 4,
                h: 4,
                filter: ResampleFilter::Nearest,
            })
            .unwrap()
        );
        assert!(
            !super::registry::gpu_supports(&PipelineOp::Reduce {
                x_factor: 2,
                y_factor: 2,
            })
            .unwrap()
        );
        assert!(
            !super::registry::gpu_supports(&PipelineOp::Scale {
                factor: 2.0,
                filter: ResampleFilter::Nearest,
            })
            .unwrap()
        );
    }

    #[test]
    fn mode_changing_gpu_ops_must_terminate_a_dispatch_batch() {
        let convert = PipelineOp::Convert {
            mode: ColorMode::L,
            matrix: None,
            dither: None,
        };
        assert!(!gpu_batch_has_nonterminal_mode_change(&[convert.clone()]));
        assert!(gpu_batch_has_nonterminal_mode_change(&[
            convert,
            PipelineOp::Invert,
        ]));
        assert!(gpu_batch_has_nonterminal_mode_change(&[
            PipelineOp::ExtractBand { index: 0 },
            PipelineOp::Invert,
        ]));
        assert!(gpu_batch_has_nonterminal_mode_change(&[
            PipelineOp::Constant { value: 4 },
            PipelineOp::Invert,
        ]));
        assert!(gpu_batch_has_nonterminal_mode_change(&[
            PipelineOp::PutAlpha {
                alpha: 128,
                mode: PixelMode::RGBA,
            },
            PipelineOp::Invert,
        ]));
        assert!(!gpu_batch_has_nonterminal_mode_change(&[
            PipelineOp::Invert,
            PipelineOp::Flip,
        ]));
    }

    #[test]
    fn alpha_blend_module_stays_cpu_for_native_alpha_modes() {
        let other = std::sync::Arc::new(crate::image::Image::new_palette_index(1, 1, 0));
        let rgba = DynamicImage::ImageRgba8(RgbaImage::new(1, 1));
        assert!(gpu_operation_mode_requires_cpu(
            &PipelineOp::BlendModule {
                other: other.clone(),
                alpha: 0.0,
            },
            &rgba,
        ));

        let rgb = DynamicImage::ImageRgb8(RgbImage::new(1, 1));
        assert!(!gpu_operation_mode_requires_cpu(
            &PipelineOp::BlendModule { other, alpha: 1.0 },
            &rgb,
        ));
    }

    #[test]
    fn gpu_mode_output_preserves_luma_bytes_without_reweighting() {
        assert_eq!(gpu_output_color_type(&ColorMode::L), Some(ColorType::L8));
        assert_eq!(gpu_output_color_type(&ColorMode::LA), Some(ColorType::La8));
        assert_eq!(
            gpu_output_color_type(&ColorMode::RGB),
            Some(ColorType::Rgb8)
        );
        assert_eq!(
            gpu_output_color_type(&ColorMode::RGBA),
            Some(ColorType::Rgba8)
        );
        assert_eq!(gpu_output_color_type(&ColorMode::I), None);

        let packed = DynamicImage::ImageRgba8(RgbaImage::from_pixel(1, 1, Rgba([100, 0, 0, 77])));
        let luma = gpu_result_as_color_type(packed.clone(), ColorType::L8).unwrap();
        assert_eq!(luma.to_luma8().get_pixel(0, 0)[0], 100);
        let luma_alpha = gpu_result_as_color_type(packed, ColorType::La8).unwrap();
        assert_eq!(luma_alpha.to_luma_alpha8().get_pixel(0, 0).0, [100, 77]);
    }

    #[test]
    fn separable_blur_parameters_match_the_fixed_point_contract() {
        assert_eq!(
            super::registry::extract_params(&PipelineOp::BoxBlur { radius: 2 }),
            vec![2, 3_355_443, 0]
        );
        assert_eq!(
            super::registry::extract_params(&PipelineOp::GaussianBlur { sigma: 0.0 }),
            vec![0, 16_777_216, 0]
        );
        // GaussianBlur(1) uses a fractional 0.25-radius box. Its integer
        // radius is zero, but the nonzero edge weight must still reach both
        // separable shaders instead of being mistaken for BoxBlur(0).
        assert_eq!(
            super::registry::extract_params(&PipelineOp::GaussianBlur { sigma: 1.0 }),
            vec![0, 11_184_811, 2_796_202]
        );
        assert_eq!(
            super::registry::extract_params(&PipelineOp::BoxBlur { radius: 100 }),
            vec![16, 508_400, 0]
        );
    }

    #[test]
    fn dispatch_grid_respects_adapter_workgroup_limit() {
        let wide_generator = PipelineOp::EffectMandelbrot {
            w: 1_048_576,
            h: 1,
            x0: -2.0,
            y0: -1.0,
            x1: 1.0,
            y1: 1.0,
            quality: 1,
        };
        assert!(gpu_dispatch_dimensions_require_cpu(
            std::slice::from_ref(&wide_generator),
            (1, 1),
            65_535,
        ));
        assert!(!gpu_dispatch_dimensions_require_cpu(
            std::slice::from_ref(&wide_generator),
            (1, 1),
            65_536,
        ));
        assert!(gpu_dispatch_dimensions_require_cpu(
            &[PipelineOp::Invert],
            (1_048_576, 1),
            65_535,
        ));
    }

    #[test]
    fn expensive_finite_shader_work_is_preflighted_to_cpu() {
        let mandelbrot = PipelineOp::EffectMandelbrot {
            w: 4096,
            h: 4096,
            x0: -2.0,
            y0: -1.0,
            x1: 1.0,
            y1: 1.0,
            quality: MAX_GPU_MANDELBROT_ITERS,
        };
        assert!(gpu_shader_work_requires_cpu(
            &mandelbrot,
            (1, 1),
            (4096, 4096),
        ));
        assert!(!gpu_shader_work_requires_cpu(
            &PipelineOp::GaussianBlur { sigma: 2.0 },
            (256, 256),
            (256, 256),
        ));
        assert!(gpu_shader_work_requires_cpu(
            &PipelineOp::GaussianBlur { sigma: 5.0 },
            (4096, 4096),
            (4096, 4096),
        ));
        assert!(gpu_shader_work_requires_cpu(
            &PipelineOp::Filter3x3 {
                kernel: [0.0; 9],
                scale: 1.0,
                offset: 0,
            },
            (2, 2),
            (2, 2),
        ));
        assert!(gpu_shader_work_requires_cpu(
            &PipelineOp::Filter5x5 {
                kernel: [0.0; 25],
                scale: 1.0,
                offset: 0,
            },
            (4, 4),
            (4, 4),
        ));
        assert!(gpu_shader_work_requires_cpu(
            &PipelineOp::MedianFilter { size: 9 },
            (256, 256),
            (256, 256),
        ));
        assert!(gpu_shader_work_requires_cpu(
            &PipelineOp::RankFilter { size: 9, rank: 40 },
            (256, 256),
            (256, 256),
        ));
        assert!(gpu_shader_work_requires_cpu(
            &PipelineOp::BoxBlur { radius: 1 },
            (2048, 2048),
            (2048, 2048),
        ));
        assert!(gpu_shader_work_requires_cpu(
            &PipelineOp::MaxFilter { size: 9 },
            (1024, 1024),
            (1024, 1024),
        ));
        assert!(gpu_shader_work_requires_cpu(
            &PipelineOp::MinFilter { size: 9 },
            (1024, 1024),
            (1024, 1024),
        ));
    }

    #[test]
    fn out_of_range_putpixel_is_preflighted_to_cpu_for_error_parity() {
        let image = DynamicImage::ImageRgb8(RgbImage::new(2, 2));
        assert!(super::gpu_dimensions_require_cpu(
            &[PipelineOp::PutPixel {
                x: 2,
                y: 0,
                color: (1, 2, 3, 255),
                palette_index: false,
            }],
            &image,
        ));
        assert!(!super::gpu_dimensions_require_cpu(
            &[PipelineOp::PutPixel {
                x: 1,
                y: 1,
                color: (1, 2, 3, 255),
                palette_index: false,
            }],
            &image,
        ));
    }

    #[test]
    fn auxiliary_native_modes_must_match_the_primary_binary_source() {
        let primary = DynamicImage::ImageRgb8(RgbImage::new(2, 2));
        let luma = DynamicImage::ImageLuma8(crate::raster::GrayImage::new(2, 2));
        let auxiliary = super::AuxiliaryImages {
            second: Some(std::sync::Arc::new(luma)),
            third: None,
        };
        let other = std::sync::Arc::new(crate::image::Image::new_palette_index(2, 2, 0));
        let op = PipelineOp::Multiply { other };
        assert!(!gpu_auxiliary_modes_are_safe(&op, &primary, &auxiliary));
        assert!(gpu_pipeline_requires_cpu(&[op], &primary, &[auxiliary]));
    }

    #[test]
    fn paste_luma_masks_must_use_a_native_luma_layout() {
        let primary = DynamicImage::ImageRgb8(RgbImage::new(2, 2));
        let source = DynamicImage::ImageRgb8(RgbImage::new(2, 2));
        let rgb_mask = DynamicImage::ImageRgb8(RgbImage::new(2, 2));
        let op = PipelineOp::Paste {
            source: std::sync::Arc::new(
                crate::image::Image::new(2, 2, "RGB", (0, 0, 0, 255)).unwrap(),
            ),
            x: 0,
            y: 0,
            w: 2,
            h: 2,
            mask: Some(std::sync::Arc::new(
                crate::image::Image::new(2, 2, "RGB", (0, 0, 0, 255)).unwrap(),
            )),
            mask_alpha: false,
        };
        let rgb_auxiliary = super::AuxiliaryImages {
            second: Some(std::sync::Arc::new(source)),
            third: Some(std::sync::Arc::new(rgb_mask)),
        };
        assert!(!gpu_auxiliary_modes_are_safe(&op, &primary, &rgb_auxiliary));

        let luma_auxiliary = super::AuxiliaryImages {
            second: Some(std::sync::Arc::new(DynamicImage::ImageRgb8(RgbImage::new(
                2, 2,
            )))),
            third: Some(std::sync::Arc::new(DynamicImage::ImageLuma8(
                crate::raster::GrayImage::new(2, 2),
            ))),
        };
        assert!(gpu_auxiliary_modes_are_safe(&op, &primary, &luma_auxiliary));
    }

    #[test]
    fn alpha_image_ops_with_rgb_temporaries_fall_back_before_gpu_dispatch() {
        let rgba = DynamicImage::ImageRgba8(RgbaImage::new(2, 2));
        assert!(super::gpu_dimensions_require_cpu(
            &[PipelineOp::Posterize { bits: 4 }],
            &rgba,
        ));
        assert!(super::gpu_dimensions_require_cpu(
            &[PipelineOp::Solarize { threshold: 128 }],
            &DynamicImage::ImageLumaA8(GrayAlphaImage::new(2, 2)),
        ));
        assert!(!super::gpu_dimensions_require_cpu(
            &[PipelineOp::Posterize { bits: 4 }],
            &DynamicImage::ImageRgb8(RgbImage::new(2, 2)),
        ));
    }

    #[test]
    fn alpha_composite_only_enters_gpu_for_native_alpha_modes() {
        let source =
            std::sync::Arc::new(crate::image::Image::new(2, 2, "RGBA", (0, 0, 0, 0)).unwrap());
        let op = PipelineOp::AlphaComposite {
            source,
            dest: (0, 0),
            src: (0, 0),
        };
        let luma = DynamicImage::ImageRgb8(RgbImage::new(2, 2));
        assert!(super::gpu_dimensions_require_cpu(&[op.clone()], &luma));
        let alpha = DynamicImage::ImageLumaA8(GrayAlphaImage::new(2, 2));
        assert!(!super::gpu_dimensions_require_cpu(&[op], &alpha));
    }

    #[test]
    fn resize_preflight_keeps_dimension_validation_independent_of_mode() {
        let image = DynamicImage::ImageRgba8(RgbaImage::new(2, 2));
        assert!(!super::gpu_dimensions_require_cpu(
            &[PipelineOp::Resize {
                w: 3,
                h: 3,
                filter: ResampleFilter::Bilinear,
            }],
            &image,
        ));
        let opaque = DynamicImage::ImageRgb8(RgbImage::new(2, 2));
        assert!(!super::gpu_dimensions_require_cpu(
            &[PipelineOp::Resize {
                w: 3,
                h: 3,
                filter: ResampleFilter::Bilinear,
            }],
            &opaque,
        ));
    }

    #[test]
    fn typed_native_images_never_enter_the_rgba8_gpu_transport() {
        let image = DynamicImage::ImageLuma16(crate::raster::ImageBuffer::new(1, 1));
        assert!(!gpu_image_layout_is_supported(&image));
        assert!(super::gpu_dimensions_require_cpu(
            &[PipelineOp::Invert],
            &image
        ));
    }

    #[test]
    fn batch_capacity_tracks_high_water_mark_not_global_limit() {
        let image = DynamicImage::ImageRgb8(RgbImage::new(2, 2));
        let ops = [
            PipelineOp::Invert,
            PipelineOp::Reduce {
                x_factor: 2,
                y_factor: 2,
            },
        ];
        let auxiliary = [
            super::AuxiliaryImages {
                second: None,
                third: None,
            },
            super::AuxiliaryImages {
                second: None,
                third: None,
            },
        ];
        assert_eq!(gpu_batch_capacity(&ops, &image, &auxiliary).unwrap(), 4);
    }

    #[test]
    fn gpu_image_buffers_respect_adapter_limits_before_allocation() {
        assert!(!gpu_buffer_capacity_exceeds_limits(1024, 4096, 4096));
        assert!(gpu_buffer_capacity_exceeds_limits(1024, 1023, 4096));
        assert!(gpu_buffer_capacity_exceeds_limits(1024, 4096, 4095));
    }
}
