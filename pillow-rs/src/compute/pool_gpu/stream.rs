//! Explicit GPU graph scheduling with bounded ownership and terminal-only readback.
//!
//! Each job records the existing PipelineOp graph in its own command buffer.
//! Several command buffers and their terminal staging copies share one queue
//! submission. This does not concatenate pixels or change image borders.

use super::{
    AuxiliaryImages, BufferPool, GPU_BUFFER_CAPACITY, GpuAuxiliaryCache, GpuInner, GpuPool,
    MAX_GPU_OPS_PER_SUBMISSION, MAX_GPU_SHADER_WORK_ITEMS, ReusableGpuBuffer, StagingBuffer,
    aligned_bytes, create_sized_buffer, gpu_buffer_capacity_exceeds_limits, gpu_dispatch_count,
    gpu_dispatch_dimensions_require_cpu, gpu_operation_is_safe, gpu_shader_work_items,
    gpu_working_set_bytes, op_has_explicit_output_dimensions, op_output_dims,
    plan_extract_band_dispatch, plan_native_rgb_point_output,
};
use crate::Image;
use crate::checked_dims::CheckedDims;
use crate::compute::registry;
use crate::error::PilError;
use crate::image::{GpuPendingGraph, GpuResultMetadata};
use crate::pipeline::{ColorMode, PipelineOp, ResampleFilter};
use crate::raster::DynamicImage;
use std::collections::VecDeque;
use std::num::NonZeroU64;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// A compact application key. Duplicate keys are allowed; job IDs stay unique.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BatchKey {
    /// An application-assigned integer.
    Integer(u64),
    /// A bounded UTF-8 label.
    Text(String),
}

/// Failure category for the explicit executor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchErrorKind {
    /// Admission cannot fit alongside currently retained work.
    QueueFull,
    /// Streaming-driver signal to release caller-held outputs before retrying.
    ResultBackpressure,
    /// The graph is unproven within the planner's layout, device, or work bounds.
    Unsupported,
    /// A known configuration, admission-resource, or identity bound was exceeded.
    Limit,
    /// An iterator already owns the executor, or it has been closed.
    State,
    /// A device, mapping, or input failure.
    Execution,
}

/// Identity-bearing failure; admission failures consume their allocated ID.
#[derive(Debug, Clone)]
pub struct BatchError {
    /// Job ID, if a graph was presented.
    pub job_id: Option<u64>,
    /// Application key, if supplied by the streaming driver.
    pub input_key: Option<BatchKey>,
    /// Failing lifecycle stage.
    pub stage: &'static str,
    /// Machine-readable failure category.
    pub kind: BatchErrorKind,
    /// Detailed explanation.
    pub message: String,
}
impl std::fmt::Display for BatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "GPU batch {} {:?}: {}",
            self.stage, self.job_id, self.message
        )
    }
}
impl std::error::Error for BatchError {}

/// Independent bounds; device limits do not measure available GPU memory.
#[derive(Debug, Clone)]
pub struct GpuBatchConfig {
    /// Queue jobs for grouped submission; false executes one job immediately.
    pub queue: bool,
    /// Maximum live executor-owned device allocations, including returned leases.
    pub gpu_bytes: u64,
    /// Maximum retained host input, metadata, and staging reservation.
    pub host_bytes: u64,
    /// Maximum admitted jobs, including pending and undelivered outputs.
    pub max_jobs: usize,
    /// Maximum outstanding queue submissions.
    pub max_in_flight: usize,
}
impl Default for GpuBatchConfig {
    fn default() -> Self {
        Self {
            queue: true,
            gpu_bytes: 512 << 20,
            host_bytes: 256 << 20,
            max_jobs: 64,
            max_in_flight: 2,
        }
    }
}

/// Bounded cumulative counters, suitable for verifying actual batch execution.
#[derive(Debug, Clone, Copy, Default)]
pub struct GpuBatchStats {
    /// Currently charged device bytes.
    pub gpu_bytes: u64,
    /// Currently charged host bytes.
    pub host_bytes: u64,
    /// Jobs pending, executing, or awaiting delivery.
    pub live_jobs: usize,
    /// Outstanding GPU submissions.
    pub in_flight: usize,
    /// Completed/submitted job count (no per-job history is retained).
    pub submitted_jobs: u64,
    /// Number of real queue submissions.
    pub submissions: u64,
    /// Number of real shader dispatches.
    pub dispatches: u64,
    /// Uploaded source bytes, excluding word padding.
    pub upload_bytes: u64,
    /// Terminal bytes copied for CPU output. Resident output adds zero.
    pub readback_bytes: u64,
    /// Largest job count in one submission.
    pub largest_submission: usize,
}

#[derive(Default)]
struct Usage {
    gpu: u64,
    host: u64,
    gpu_limit: u64,
    host_limit: u64,
}
struct Credit {
    usage: Arc<Mutex<Usage>>,
    gpu: u64,
    host: u64,
}
impl Drop for Credit {
    fn drop(&mut self) {
        let mut usage = self.usage.lock().unwrap_or_else(|e| e.into_inner());
        usage.gpu -= self.gpu;
        usage.host -= self.host;
    }
}
impl Credit {
    fn reduce_to(&mut self, gpu: u64, host: u64) {
        assert!(gpu <= self.gpu && host <= self.host);
        let mut usage = self.usage.lock().unwrap_or_else(|e| e.into_inner());
        usage.gpu -= self.gpu - gpu;
        usage.host -= self.host - host;
        self.gpu = gpu;
        self.host = host;
    }
    fn split(&mut self, gpu: u64, host: u64) -> Self {
        assert!(gpu <= self.gpu && host <= self.host);
        self.gpu -= gpu;
        self.host -= host;
        Self {
            usage: self.usage.clone(),
            gpu,
            host,
        }
    }
}

/// Immutable GPU-owned pixels. Clones share the same charged allocation.
#[derive(Clone)]
pub struct GpuImage {
    inner: Arc<Resident>,
}
struct Resident {
    gpu: &'static GpuInner,
    buffer: wgpu::Buffer,
    width: u32,
    height: u32,
    mode: String,
    length: usize,
    template: GpuResultMetadata,
    download_lock: Mutex<()>,
    _credit: Credit,
}
impl GpuImage {
    /// Native logical mode.
    pub fn mode(&self) -> &str {
        &self.inner.mode
    }
    /// Pixel dimensions.
    pub fn size(&self) -> (u32, u32) {
        (self.inner.width, self.inner.height)
    }
    /// Download a native CPU image. The resident handle remains charged.
    pub fn download(&self) -> Result<Image, PilError> {
        let inner = &self.inner;
        // Keep one download workspace reserved with the handle. Admission
        // must not consume the headroom needed to read the next delivered
        // result. Concurrent downloads of shared owners use that workspace
        // serially, while already returned CPU images are caller-owned.
        let _download = inner
            .download_lock
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        inner.gpu.ensure_healthy("resident GPU download")?;
        let size = aligned_bytes(inner.length, 4) as u64;
        let staging = StagingBuffer::new(&inner.gpu.device, size);
        let mut encoder =
            inner
                .gpu
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("gpu_stream_download"),
                });
        encoder.copy_buffer_to_buffer(&inner.buffer, 0, &staging.buffer, 0, size);
        inner.gpu.queue.submit([encoder.finish()]);
        let bytes = inner.gpu.readback_with(size, &staging.buffer, |view| {
            Ok(view[..inner.length].to_vec())
        })?;
        native_image(
            &inner.template,
            inner.width,
            inner.height,
            &inner.mode,
            bytes,
        )
    }
}

/// Materialized result storage selected by the stream.
pub enum BatchOutput {
    /// Native host pixels, ready for normal Pillow-style methods.
    Cpu(Box<Image>),
    /// Immutable device pixels, retaining an allocation lease.
    Gpu(GpuImage),
}
/// A result's identity does not depend on completion position or buffer offset.
pub struct BatchResult {
    /// Executor-unique monotonic ID.
    pub job_id: u64,
    /// Optional user key; duplicates are valid.
    pub input_key: Option<BatchKey>,
    /// Materialized storage.
    pub image: BatchOutput,
}

struct Job {
    id: u64,
    key: Option<BatchKey>,
    template: GpuResultMetadata,
    width: u32,
    height: u32,
    mode: String,
    length: usize,
    output: Option<wgpu::Buffer>,
    command: Option<wgpu::CommandBuffer>,
    credit: Credit,
    output_capacity: u64,
    metadata_bytes: u64,
    dispatches: u64,
    upload: u64,
    shader_work: u64,
    op_count: usize,
}
struct Flight {
    jobs: Vec<Job>,
    completed: Arc<AtomicBool>,
    failure: Arc<Mutex<Option<String>>>,
    _fence: Arc<ReadbackArena>,
    staging: Option<Arc<ReadbackArena>>,
    offsets: Vec<u64>,
    submitted_at: Instant,
}
struct ReadbackArena {
    buffer: wgpu::Buffer,
    bytes: u64,
    mapped: AtomicBool,
    _credit: Credit,
}
impl Drop for ReadbackArena {
    fn drop(&mut self) {
        if self.mapped.load(Ordering::Acquire) {
            self.buffer.unmap();
        }
    }
}
struct Ready {
    job: Job,
    arena: Option<Arc<ReadbackArena>>,
    offset: u64,
}

/// Caller-driven, bounded scheduler for existing lazy Image graphs.
///
/// No CPU image-operation fallback, Rayon scheduling, or intermediate readback
/// is used. Results may be delivered in any completion order.
pub struct GpuBatchExecutor {
    gpu: &'static GpuInner,
    config: GpuBatchConfig,
    usage: Arc<Mutex<Usage>>,
    next_id: Option<u64>,
    pending: VecDeque<Job>,
    flights: VecDeque<Flight>,
    ready: VecDeque<Ready>,
    stats: GpuBatchStats,
    resident: bool,
    closed: bool,
}
impl GpuBatchExecutor {
    /// Create an executor on pillow-rs's existing Device/Queue.
    pub fn new(config: GpuBatchConfig) -> Result<Self, BatchError> {
        if config.max_jobs == 0
            || config.max_in_flight == 0
            || config.gpu_bytes == 0
            || config.host_bytes == 0
        {
            return Err(error(
                None,
                None,
                "setup",
                BatchErrorKind::Limit,
                "all resource bounds must be nonzero",
            ));
        }
        let gpu = GpuPool::ensure_init().map_err(|e| {
            error(
                None,
                None,
                "setup",
                BatchErrorKind::Execution,
                e.to_string(),
            )
        })?;
        gpu.ensure_healthy("GPU batch setup").map_err(|e| {
            error(
                None,
                None,
                "setup",
                BatchErrorKind::Execution,
                e.to_string(),
            )
        })?;
        Ok(Self {
            gpu,
            config: config.clone(),
            usage: Arc::new(Mutex::new(Usage {
                gpu: 0,
                host: 0,
                gpu_limit: config.gpu_bytes,
                host_limit: config.host_bytes,
            })),
            next_id: Some(0),
            pending: VecDeque::new(),
            flights: VecDeque::new(),
            ready: VecDeque::new(),
            stats: GpuBatchStats::default(),
            resident: false,
            closed: false,
        })
    }

    /// Change output policy only while no undelivered work exists.
    pub fn set_resident_output(&mut self, resident: bool) -> Result<(), BatchError> {
        if self.stats.live_jobs != 0 {
            return Err(error(
                None,
                None,
                "driver",
                BatchErrorKind::State,
                "drain existing work before changing output policy",
            ));
        }
        self.resident = resident;
        Ok(())
    }

    /// Submit the existing lazy image directly, without an operation adapter.
    pub fn submit(&mut self, pipeline: &Image) -> Result<u64, BatchError> {
        self.submit_keyed(pipeline, None)
    }

    /// Admission used by keyed streaming drivers; keys are compact metadata.
    pub fn submit_keyed(
        &mut self,
        pipeline: &Image,
        key: Option<BatchKey>,
    ) -> Result<u64, BatchError> {
        self.submit_with_metadata(pipeline, key, 0)
    }

    /// Host-adapter admission with a separately measured metadata reservation.
    pub fn submit_with_metadata(
        &mut self,
        pipeline: &Image,
        key: Option<BatchKey>,
        metadata_bytes: u64,
    ) -> Result<u64, BatchError> {
        if self.closed {
            return Err(error(
                None,
                key,
                "admission",
                BatchErrorKind::State,
                "executor is closed",
            ));
        }
        let id = self.next_id.ok_or_else(|| {
            error(
                None,
                key.clone(),
                "admission",
                BatchErrorKind::Limit,
                "job ID exhausted",
            )
        })?;
        self.next_id = id.checked_add(1);
        let fail = |kind, message: String| error(Some(id), key.clone(), "admission", kind, message);
        self.gpu
            .ensure_healthy("GPU batch admission")
            .map_err(|e| fail(BatchErrorKind::Execution, e.to_string()))?;
        let key_bytes = match &key {
            Some(BatchKey::Text(text)) => text.len(),
            _ => 8,
        } as u64;
        if key_bytes > self.config.host_bytes {
            return Err(fail(
                BatchErrorKind::Limit,
                "input key exceeds host budget".into(),
            ));
        }
        let mut plan = plan_graph(pipeline, self.gpu)
            .map_err(|e| fail(BatchErrorKind::Unsupported, e.to_string()))?;
        // Source owners and graph descriptors are conservatively charged per
        // admitted job; shared owners never make the cap less restrictive.
        plan.metadata_bytes = plan
            .metadata_bytes
            .checked_add(metadata_bytes)
            .ok_or_else(|| fail(BatchErrorKind::Limit, "metadata byte count overflow".into()))?;
        let host = plan
            .host_bytes
            .checked_add(plan.metadata_bytes)
            .and_then(|bytes| bytes.checked_add(metadata_bytes))
            .and_then(|bytes| bytes.checked_add(key_bytes))
            .ok_or_else(|| {
                fail(
                    BatchErrorKind::Limit,
                    "host budget arithmetic overflow".into(),
                )
            })?;
        let gpu = plan.gpu_bytes;
        if host > self.config.host_bytes || gpu > self.config.gpu_bytes {
            return Err(fail(
                BatchErrorKind::Limit,
                format!(
                    "job needs {gpu} GPU and {host} host bytes; configured caps are {} and {}",
                    self.config.gpu_bytes, self.config.host_bytes
                ),
            ));
        }
        let mut usage = self.usage.lock().unwrap_or_else(|e| e.into_inner());
        if self.stats.live_jobs >= self.config.max_jobs
            || usage.gpu > self.config.gpu_bytes - gpu
            || usage.host > self.config.host_bytes - host
        {
            return Err(fail(
                BatchErrorKind::QueueFull,
                "live work or returned GPU leases fill the resource budget".into(),
            ));
        }
        usage.gpu += gpu;
        usage.host += host;
        drop(usage);
        let credit = Credit {
            usage: self.usage.clone(),
            gpu,
            host,
        };
        let job = match compile_graph(pipeline, key.clone(), id, plan, credit, self.gpu) {
            Ok(job) => job,
            Err(error) => {
                self.stats.submissions += 1; // transfer-only compilation cleanup fence
                return Err(fail(BatchErrorKind::Execution, error.to_string()));
            }
        };
        self.pending.push_back(job);
        self.stats.live_jobs += 1;
        if !self.config.queue {
            if let Err(error) = self.wait_for_result(Some(id)) {
                self.close();
                return Err(error);
            }
        }
        Ok(id)
    }

    /// Inspect bounded counters; returned GPU handles remain in byte totals.
    pub fn stats(&self) -> GpuBatchStats {
        let usage = self.usage.lock().unwrap_or_else(|e| e.into_inner());
        debug_assert!(usage.gpu <= usage.gpu_limit && usage.host <= usage.host_limit);
        GpuBatchStats {
            gpu_bytes: usage.gpu,
            host_bytes: usage.host,
            in_flight: self.flights.len(),
            ..self.stats
        }
    }

    /// Pull one completed result, scheduling admitted work as needed.
    /// Core admission reports QueueFull while returned handles retain credits.
    /// Python's streaming driver turns that pressure into a retryable signal.
    pub fn next_result(&mut self) -> Result<Option<BatchResult>, BatchError> {
        let result = self.next_result_inner();
        if result.is_err() {
            self.close();
        }
        result
    }

    fn next_result_inner(&mut self) -> Result<Option<BatchResult>, BatchError> {
        if self.closed {
            return Ok(None);
        }
        self.retire_completed()?;
        let target = self.config.max_jobs.div_ceil(self.config.max_in_flight);
        if self.ready.is_empty() || self.pending.len() >= target {
            self.dispatch()?;
        }
        self.retire_completed()?;
        if self.ready.is_empty() && !self.flights.is_empty() {
            self.wait_one()?;
        }
        let Some(ready) = self.ready.pop_front() else {
            return Ok(None);
        };
        let Ready {
            mut job,
            arena,
            offset,
        } = ready;
        self.stats.live_jobs -= 1;
        let image = if let Some(arena) = arena {
            if !arena.mapped.load(Ordering::Acquire) {
                self.gpu
                    .wait_for_buffer_mapping(
                        arena.bytes,
                        &arena.buffer,
                        wgpu::MapMode::Read,
                        "GPU stream readback",
                    )
                    .map_err(|e| {
                        error(
                            Some(job.id),
                            job.key.clone(),
                            "readback",
                            BatchErrorKind::Execution,
                            e.to_string(),
                        )
                    })?;
                arena.mapped.store(true, Ordering::Release);
            }
            let bytes = {
                let view = arena.buffer.slice(..arena.bytes).get_mapped_range();
                view[offset as usize..offset as usize + job.length].to_vec()
            };
            BatchOutput::Cpu(Box::new(
                native_image(&job.template, job.width, job.height, &job.mode, bytes).map_err(
                    |e| {
                        error(
                            Some(job.id),
                            job.key.clone(),
                            "materialization",
                            BatchErrorKind::Execution,
                            e.to_string(),
                        )
                    },
                )?,
            ))
        } else {
            let staging = aligned_bytes(job.length, 8) as u64;
            let credit = job.credit.split(
                job.output_capacity + staging,
                job.metadata_bytes * 2 + staging * 2,
            );
            BatchOutput::Gpu(GpuImage {
                inner: Arc::new(Resident {
                    gpu: self.gpu,
                    buffer: job
                        .output
                        .take()
                        .expect("resident terminal buffer retained"),
                    width: job.width,
                    height: job.height,
                    mode: job.mode,
                    length: job.length,
                    template: job.template,
                    download_lock: Mutex::new(()),
                    _credit: credit,
                }),
            })
        };
        Ok(Some(BatchResult {
            job_id: job.id,
            input_key: job.key,
            image,
        }))
    }

    fn dispatch(&mut self) -> Result<(), BatchError> {
        while !self.pending.is_empty() && self.flights.len() < self.config.max_in_flight {
            let mut count = 0;
            let mut chunk_bytes = 0u64;
            let mut chunk_work = 0u64;
            let mut chunk_ops = 0usize;
            for job in &self.pending {
                let Some(bytes) = chunk_bytes.checked_add(aligned_bytes(job.length, 8) as u64)
                else {
                    break;
                };
                let Some(work) = chunk_work.checked_add(job.shader_work) else {
                    break;
                };
                let ops = chunk_ops.saturating_add(job.op_count);
                if count > 0
                    && (bytes > self.gpu.device.limits().max_buffer_size
                        || work > MAX_GPU_SHADER_WORK_ITEMS
                        || ops > MAX_GPU_OPS_PER_SUBMISSION)
                {
                    break;
                }
                if bytes > self.gpu.device.limits().max_buffer_size {
                    return Err(error(
                        Some(job.id),
                        job.key.clone(),
                        "dispatch",
                        BatchErrorKind::Limit,
                        "terminal arena exceeds device buffer limit",
                    ));
                }
                (chunk_bytes, chunk_work, chunk_ops) = (bytes, work, ops);
                count += 1;
                if count >= self.config.max_jobs.div_ceil(self.config.max_in_flight) {
                    break;
                }
                if !self.config.queue {
                    break;
                }
            }
            let mut jobs: Vec<_> = self.pending.drain(..count).collect();
            let mut commands = Vec::with_capacity(count + 1);
            let mut offsets = Vec::with_capacity(count);
            let mut bytes = 0u64;
            for job in &mut jobs {
                offsets.push(bytes);
                bytes += aligned_bytes(job.length, 8) as u64;
                commands.push(
                    job.command
                        .take()
                        .expect("unsubmitted job owns its command buffer"),
                );
                self.stats.dispatches += job.dispatches;
                self.stats.upload_bytes += job.upload;
            }
            let staging = if self.resident {
                None
            } else {
                // Each job reserved an eight-byte-aligned terminal arena
                // share at admission. Moving that share keeps the physical
                // arena charged until its last ready range is consumed.
                let mut credit = jobs[0].credit.split(0, 0);
                for job in &mut jobs {
                    let mut part = job.credit.split(
                        aligned_bytes(job.length, 8) as u64,
                        aligned_bytes(job.length, 8) as u64,
                    );
                    credit.gpu += part.gpu;
                    credit.host += part.host;
                    part.gpu = 0;
                    part.host = 0; // transfer the charge; dropping part releases its Arc
                }
                let buffer = StagingBuffer::new(&self.gpu.device, bytes).buffer;
                let mut encoder =
                    self.gpu
                        .device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("gpu_stream_terminal_copies"),
                        });
                for (job, &offset) in jobs.iter().zip(&offsets) {
                    encoder.copy_buffer_to_buffer(
                        job.output.as_ref().expect("terminal buffer retained"),
                        0,
                        &buffer,
                        offset,
                        aligned_bytes(job.length, 4) as u64,
                    );
                }
                commands.push(encoder.finish());
                self.stats.readback_bytes += jobs.iter().map(|j| j.length as u64).sum::<u64>();
                Some(Arc::new(ReadbackArena {
                    buffer,
                    bytes,
                    mapped: AtomicBool::new(false),
                    _credit: credit,
                }))
            };
            let fence = if let Some(arena) = &staging {
                arena.clone()
            } else {
                let credit = jobs[0].credit.split(8, 8);
                let buffer = StagingBuffer::new(&self.gpu.device, 8).buffer;
                let mut encoder =
                    self.gpu
                        .device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("gpu_stream_resident_fence"),
                        });
                encoder.copy_buffer_to_buffer(
                    jobs[0].output.as_ref().expect("terminal buffer retained"),
                    0,
                    &buffer,
                    0,
                    4,
                );
                commands.push(encoder.finish());
                Arc::new(ReadbackArena {
                    buffer,
                    bytes: 8,
                    mapped: AtomicBool::new(false),
                    _credit: credit,
                })
            };
            self.gpu.queue.submit(commands);
            // Buffer mapping is a fence for this chunk's last buffer use. A
            // queue-wide callback could accidentally wait for unrelated work
            // submitted concurrently after ours on Pillow's shared queue.
            let completed = Arc::new(AtomicBool::new(false));
            let failure = Arc::new(Mutex::new(None));
            let flag = completed.clone();
            let failed = failure.clone();
            let mapped_arena = fence.clone();
            fence
                .buffer
                .slice(..fence.bytes)
                .map_async(wgpu::MapMode::Read, move |result| {
                    match result {
                        Ok(()) => mapped_arena.mapped.store(true, Ordering::Release),
                        Err(error) => {
                            *failed.lock().unwrap_or_else(|e| e.into_inner()) =
                                Some(error.to_string())
                        }
                    }
                    flag.store(true, Ordering::Release);
                });
            self.stats.submitted_jobs += count as u64;
            self.stats.submissions += 1;
            self.stats.largest_submission = self.stats.largest_submission.max(count);
            self.flights.push_back(Flight {
                jobs,
                completed,
                failure,
                _fence: fence,
                staging,
                offsets,
                submitted_at: Instant::now(),
            });
        }
        Ok(())
    }

    fn retire_completed(&mut self) -> Result<(), BatchError> {
        self.gpu
            .poll_device("GPU streaming completion")
            .map_err(|e| {
                error(
                    None,
                    None,
                    "completion",
                    BatchErrorKind::Execution,
                    e.to_string(),
                )
            })?;
        let mut index = 0;
        while index < self.flights.len() {
            if self.flights[index].completed.load(Ordering::Acquire) {
                let flight = self.flights.remove(index).expect("checked flight index");
                if let Some(message) = flight
                    .failure
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .take()
                {
                    self.stats.live_jobs -= flight.jobs.len();
                    self.gpu
                        .mark_failed(format!("GPU stream completion failed: {message}"));
                    self.gpu.device.destroy();
                    let job = flight.jobs.first();
                    return Err(error(
                        job.map(|job| job.id),
                        job.and_then(|job| job.key.clone()),
                        "completion",
                        BatchErrorKind::Execution,
                        message,
                    ));
                }
                for (mut job, offset) in flight.jobs.into_iter().zip(flight.offsets) {
                    if flight.staging.is_some() {
                        job.output = None;
                        job.credit
                            .reduce_to(0, job.metadata_bytes.saturating_mul(2) + job.length as u64);
                    } else {
                        let staging = aligned_bytes(job.length, 8) as u64;
                        job.credit.reduce_to(
                            job.output_capacity + staging,
                            job.metadata_bytes * 2 + staging * 2,
                        );
                    }
                    self.ready.push_back(Ready {
                        job,
                        offset,
                        arena: flight.staging.clone(),
                    });
                }
            } else {
                index += 1;
            }
        }
        Ok(())
    }

    fn wait_one(&mut self) -> Result<(), BatchError> {
        self.wait_for_result(None)
    }

    fn wait_for_result(&mut self, job_id: Option<u64>) -> Result<(), BatchError> {
        loop {
            self.retire_completed()?;
            let ready = match job_id {
                Some(id) => self.ready.iter().any(|ready| ready.job.id == id),
                None => !self.ready.is_empty(),
            };
            if ready {
                return Ok(());
            }
            self.dispatch()?;
            if self.flights.is_empty() {
                return Ok(());
            }
            if self
                .flights
                .iter()
                .any(|flight| flight.submitted_at.elapsed() > Duration::from_secs(30))
            {
                self.gpu
                    .mark_failed("GPU streaming completion timed out after 30s".into());
                self.gpu.device.destroy();
                return Err(error(
                    self.flights
                        .front()
                        .and_then(|flight| flight.jobs.first().map(|job| job.id)),
                    None,
                    "completion",
                    BatchErrorKind::Execution,
                    "GPU submission timed out",
                ));
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    /// Stop admission, cancel pending jobs, and discard outputs after fences.
    pub fn close(&mut self) {
        if !self.pending.is_empty() {
            // Queue uploads issued while encoding admission are not cancellable.
            // Flush only those transfers, discard image commands, and retain
            // their credits until a buffer fence observes completion.
            let mut jobs: Vec<_> = self.pending.drain(..).collect();
            for job in &mut jobs {
                job.command = None;
            }
            let credit = jobs[0].credit.split(8, 8);
            let buffer = StagingBuffer::new(&self.gpu.device, 8).buffer;
            let mut encoder =
                self.gpu
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("gpu_stream_cancel_upload_fence"),
                    });
            encoder.copy_buffer_to_buffer(
                jobs[0]
                    .output
                    .as_ref()
                    .expect("cancelled job retains its buffer"),
                0,
                &buffer,
                0,
                4,
            );
            self.gpu.queue.submit([encoder.finish()]);
            let fence = Arc::new(ReadbackArena {
                buffer,
                bytes: 8,
                mapped: AtomicBool::new(false),
                _credit: credit,
            });
            let completed = Arc::new(AtomicBool::new(false));
            let failure = Arc::new(Mutex::new(None));
            let flag = completed.clone();
            let failed = failure.clone();
            let mapped = fence.clone();
            fence
                .buffer
                .slice(..8)
                .map_async(wgpu::MapMode::Read, move |result| {
                    match result {
                        Ok(()) => mapped.mapped.store(true, Ordering::Release),
                        Err(error) => {
                            *failed.lock().unwrap_or_else(|e| e.into_inner()) =
                                Some(error.to_string())
                        }
                    }
                    flag.store(true, Ordering::Release);
                });
            let offsets = vec![0; jobs.len()];
            self.flights.push_back(Flight {
                jobs,
                offsets,
                completed,
                failure,
                _fence: fence,
                staging: None,
                submitted_at: Instant::now(),
            });
            self.stats.submissions += 1;
        }
        self.ready.clear();
        // Submissions cannot be cancelled. Keep their credits until complete;
        // mapping is skipped when the consumer no longer wants the outputs.
        while !self.flights.is_empty() {
            if self.wait_one().is_err() {
                break;
            }
            self.ready.clear();
        }
        self.flights.clear();
        self.stats.live_jobs = 0;
        self.closed = true;
    }
}
impl Drop for GpuBatchExecutor {
    fn drop(&mut self) {
        self.close();
    }
}

fn error(
    id: Option<u64>,
    key: Option<BatchKey>,
    stage: &'static str,
    kind: BatchErrorKind,
    message: impl Into<String>,
) -> BatchError {
    BatchError {
        job_id: id,
        input_key: key,
        stage,
        kind,
        message: message.into(),
    }
}
fn native_image(
    template: &GpuResultMetadata,
    width: u32,
    height: u32,
    mode: &str,
    bytes: Vec<u8>,
) -> Result<Image, PilError> {
    let channels = native_channels(mode)
        .ok_or_else(|| PilError::ValueError("unsupported native output mode".into()))?;
    let pixels = crate::image_utils::raw_bytes_to_image(width, height, bytes, channels as usize)?;
    Ok(template.materialize(pixels, mode))
}
fn planned_channels(mode: &str) -> Result<u8, PilError> {
    native_channels(mode).ok_or_else(|| {
        PilError::InternalError(format!("GPU plan has no native channel layout for {mode}"))
    })
}

fn native_channels(mode: &str) -> Option<u8> {
    match mode {
        "L" => Some(1),
        "LA" => Some(2),
        "RGB" => Some(3),
        "RGBA" | "CMYK" | "RGBX" => Some(4),
        _ => None,
    }
}

// Encoding plans refer to the canonical operations. Layout flags select existing
// native shaders; no second public operation class or host image evaluator exists.
#[derive(Default, Clone, Copy)]
struct Flags {
    extract: bool,
    luma_point: bool,
    luma_order: bool,
    byte_filter: bool,
    rgb: bool,
    rgb_blur: bool,
    grayscale_rgb: bool,
    luma_convert: bool,
    luma_colorize: bool,
}
struct Stage<'a> {
    op: &'a PipelineOp,
    secondary: Option<Box<Plan<'a>>>,
    width: u32,
    height: u32,
    mode: String,
    flags: Flags,
    byte: bool,
    relocation: bool,
}
struct Plan<'a> {
    source: GpuPendingGraph<'a>,
    stages: Vec<Stage<'a>>,
    width: u32,
    height: u32,
    mode: String,
    capacity: u32,
    gpu_bytes: u64,
    host_bytes: u64,
    metadata_bytes: u64,
    shader_work: u64,
    op_count: usize,
}

#[derive(Default)]
struct GraphBounds {
    work: u64,
    ops: usize,
}
impl GraphBounds {
    fn add(&mut self, work: u64) -> Result<(), PilError> {
        let total = self
            .work
            .checked_add(work)
            .ok_or_else(|| PilError::ValueError("GPU work arithmetic overflow".into()))?;
        if total > MAX_GPU_SHADER_WORK_ITEMS || self.ops >= MAX_GPU_OPS_PER_SUBMISSION {
            return Err(PilError::ValueError(
                "GPU graph exceeds cumulative submission work/operation bounds".into(),
            ));
        }
        self.work = total;
        self.ops += 1;
        Ok(())
    }
}
static COPY_OP: PipelineOp = PipelineOp::Duplicate;
static LUMA_BAND: PipelineOp = PipelineOp::ExtractBand { index: 0 };

fn plan_graph<'a>(image: &'a Image, gpu: &GpuInner) -> Result<Plan<'a>, PilError> {
    let mut bounds = GraphBounds::default();
    plan_graph_inner(image, gpu, 0, &mut bounds)
}

fn plan_graph_inner<'a>(
    image: &'a Image,
    gpu: &GpuInner,
    depth: usize,
    bounds: &mut GraphBounds,
) -> Result<Plan<'a>, PilError> {
    if depth > 64 {
        return Err(PilError::ValueError(
            "GPU auxiliary graph exceeds 64 levels".into(),
        ));
    }
    let starting_work = bounds.work;
    let starting_ops = bounds.ops;
    if !cfg!(target_endian = "little") {
        return Err(PilError::ValueError(
            "native GPU batching requires little-endian samples".into(),
        ));
    }
    let mut source = image.gpu_pending_graph()?;
    let mut mode = source.mode.clone();
    let mut width = source.width;
    let mut height = source.height;
    let channels = native_channels(&mode)
        .ok_or_else(|| PilError::ValueError(format!("no native GPU graph layout for {mode}")))?;
    let source_bytes = CheckedDims::new(width, height, channels)?.total_bytes();
    let ops = std::mem::take(&mut source.ops);
    let limits = gpu.device.limits();
    let mut capacity = 1u32;
    let mut stages = Vec::new();
    let ops = if ops.is_empty() { vec![&COPY_OP] } else { ops };
    for op in ops {
        if !GpuPool::descriptor_supports(op)? || !gpu_operation_is_safe(op) {
            return Err(PilError::ValueError(format!(
                "operation {} has no bounded exact GPU descriptor",
                registry::variant_key(op)
            )));
        }
        if matches!(op, PipelineOp::RankFilter { size, rank } if *rank >= size.saturating_mul(*size))
        {
            return Err(PilError::ValueError(
                "GPU rank lies outside its kernel".into(),
            ));
        }
        // The pixel shader silently ignores invalid coordinates; core/Pillow
        // reject them. A direct Rust descriptor must fail before upload too.
        if matches!(op, PipelineOp::PutPixel { x, y, .. } if *x >= width || *y >= height) {
            return Err(PilError::ValueError(
                "GPU pixel coordinate lies outside the current image".into(),
            ));
        }
        let mut flags = Flags::default();
        let mut byte = false;
        let mut relocation = false;
        let mut secondary = None;
        let channels = native_channels(&mode)
            .ok_or_else(|| PilError::ValueError("unproven mode transition".into()))?;
        match op {
            PipelineOp::Paste {
                source,
                w,
                h,
                mask: None,
                ..
            } => {
                let child = plan_graph_inner(source, gpu, depth + 1, bounds)?;
                if child.mode != mode
                    || u32::try_from(*w).ok() != Some(child.width)
                    || u32::try_from(*h).ok() != Some(child.height)
                {
                    return Err(PilError::ValueError(
                        "GPU paste requires matching native modes and source region".into(),
                    ));
                }
                secondary = Some(Box::new(child));
            }
            PipelineOp::Multiply { other }
            | PipelineOp::Screen { other }
            | PipelineOp::Overlay { other }
            | PipelineOp::HardLight { other }
            | PipelineOp::SoftLight { other }
            | PipelineOp::Difference { other }
            | PipelineOp::Darker { other }
            | PipelineOp::Lighter { other }
            | PipelineOp::AddModulo { other }
            | PipelineOp::SubtractModulo { other }
            | PipelineOp::BlendModule { other, .. } => {
                let child = plan_graph_inner(other, gpu, depth + 1, bounds)?;
                if child.mode != mode || (child.width, child.height) != (width, height) {
                    return Err(PilError::ValueError(
                        "GPU byte composition requires identical native modes and dimensions"
                            .into(),
                    ));
                }
                secondary = Some(Box::new(child));
                byte = true;
            }
            PipelineOp::Transpose { .. }
            | PipelineOp::Flip
            | PipelineOp::Mirror
            | PipelineOp::Crop { .. }
            | PipelineOp::CropBorder { .. }
            | PipelineOp::Expand { .. } => relocation = true,
            PipelineOp::Duplicate | PipelineOp::Invert | PipelineOp::InvertChops => byte = true,
            PipelineOp::Solarize { .. } | PipelineOp::Posterize { bits: 1..=8 }
                if matches!(mode.as_str(), "L" | "RGB") =>
            {
                byte = true
            }
            PipelineOp::ExtractBand { index } if *index < channels => flags.extract = true,
            PipelineOp::Eval { lut } if lut.len() == usize::from(channels) * 256 && mode == "L" => {
                flags.luma_point = true
            }
            PipelineOp::Eval { lut }
                if lut.len() == usize::from(channels) * 256 && mode == "RGB" =>
            {
                flags.rgb = true
            }
            PipelineOp::MedianFilter { size: 3 }
            | PipelineOp::MinFilter { size: 3 }
            | PipelineOp::MaxFilter { size: 3 } => match mode.as_str() {
                "L" => flags.luma_order = true,
                "LA" => flags.byte_filter = true,
                "RGB" => flags.rgb = true,
                "RGBA" => {}
                _ => {
                    return Err(PilError::ValueError(
                        "order filter layout unsupported".into(),
                    ));
                }
            },
            PipelineOp::RankFilter { size: 3, rank: 1 } if mode == "L" => flags.luma_order = true,
            PipelineOp::RankFilter { size: 3, rank } if *rank < 9 && mode == "LA" => {
                flags.byte_filter = true
            }
            PipelineOp::RankFilter { size: 9, rank } if *rank < 81 && mode == "L" => {
                flags.luma_order = true
            }
            PipelineOp::BoxBlur { .. }
            | PipelineOp::BoxBlurXY { .. }
            | PipelineOp::GaussianBlur { .. }
                if mode == "RGB" =>
            {
                flags.rgb_blur = true
            }
            PipelineOp::Resize {
                filter: ResampleFilter::Nearest,
                w,
                h,
            } if mode == "RGBA" && u64::from(*w) + u64::from(*h) <= 2048 => {}
            PipelineOp::BoxBlur { radius: 1 } | PipelineOp::GaussianBlur { .. }
                if matches!(mode.as_str(), "L" | "LA") =>
            {
                flags.byte_filter = true
            }
            PipelineOp::Constant { .. } => {}
            PipelineOp::Grayscale if matches!(mode.as_str(), "L" | "LA") => flags.extract = true,
            PipelineOp::Grayscale if mode == "RGB" => flags.grayscale_rgb = true,
            PipelineOp::Grayscale if matches!(mode.as_str(), "RGBA" | "CMYK" | "RGBX") => {}
            PipelineOp::Convert {
                mode: ColorMode::RGBA,
                matrix: None,
                dither: None,
            } if mode == "L" => flags.luma_convert = true,
            PipelineOp::Convert {
                mode: ColorMode::RGBA,
                matrix: None,
                dither: None,
            } if mode == "RGB" => flags.rgb = true,
            // Generic shaders are already native for an RGBA image. Restrict
            // host-controlled/auxiliary graphs until their native plans exist.
            _ if mode == "RGBA" && rgba_standalone_native(op) => {}
            _ => {
                return Err(PilError::ValueError(format!(
                    "operation {} has no composed native {mode} layout",
                    registry::variant_key(op)
                )));
            }
        }
        let output = match op_output_dims(op, width, height) {
            Some(output) => output,
            None if op_has_explicit_output_dimensions(op) => {
                return Err(PilError::ValueError(
                    "GPU graph has an unresolved output shape".into(),
                ));
            }
            None => (width, height),
        };
        let mut pixels = CheckedDims::new(width, height, 1)?
            .total_pixels()
            .max(CheckedDims::new(output.0, output.1, 1)?.total_pixels());
        if matches!(op, PipelineOp::Resize { .. }) {
            // The horizontal resize pass has destination width and SOURCE
            // height. It can exceed both the input and final pixel counts.
            pixels = pixels.max(CheckedDims::new(output.0, height, 1)?.total_pixels());
        }
        if width == 0 || height == 0 || pixels > GPU_BUFFER_CAPACITY as usize {
            return Err(PilError::ValueError(
                "GPU graph dimensions exceed native bounds".into(),
            ));
        }
        capacity = capacity.max(
            u32::try_from(pixels)
                .map_err(|_| PilError::ValueError("shader index overflow".into()))?,
        );
        if gpu_dispatch_dimensions_require_cpu(
            std::slice::from_ref(op),
            (width, height),
            limits.max_compute_workgroups_per_dimension,
            Some(&mode),
            false,
        ) {
            return Err(PilError::ValueError(
                "GPU dispatch dimensions exceed device limits".into(),
            ));
        }
        let work = gpu_shader_work_items(op, (width, height), output, Some(&mode))
            .ok_or_else(|| PilError::ValueError("GPU work arithmetic overflow".into()))?;
        bounds.add(work)?;
        let next_mode = if secondary.is_some() {
            mode.clone()
        } else {
            crate::image::known_pipeline_op_mode(op, &mode).ok_or_else(|| {
                PilError::ValueError(
                    "GPU mode transition cannot be determined without host execution".into(),
                )
            })?
        };
        stages.push(Stage {
            op,
            secondary,
            width,
            height,
            mode: mode.clone(),
            flags,
            byte,
            relocation,
        });
        mode = next_mode;
        (width, height) = output;
    }
    if gpu_buffer_capacity_exceeds_limits(
        capacity,
        limits.max_storage_buffer_binding_size,
        limits.max_buffer_size,
    ) {
        return Err(PilError::ValueError(
            "GPU graph storage exceeds device binding limit".into(),
        ));
    }
    // Exact main workspace + conservative per-stage arenas. Existing parameter
    // encoders can generate coefficient/LUT tables; all reservations precede
    // allocation, and actual retained bytes are checked during compilation.
    let resource_bytes = stages.len() as u64 * (64 << 10);
    let length = CheckedDims::new(width, height, planned_channels(&mode)?)?.total_bytes();
    let staging = aligned_bytes(length, 8) as u64;
    let mut gpu_bytes =
        gpu_working_set_bytes(capacity) + source_bytes as u64 + resource_bytes * 2 + staging;
    let metadata_bytes = image.gpu_metadata_bytes();
    let mut host_bytes = source_bytes as u64 * 3
        + source.encoded_bytes
        + staging * 2
        + resource_bytes
        + metadata_bytes
        + stages.len() as u64 * std::mem::size_of::<Stage<'_>>() as u64;
    for child in stages.iter().filter_map(|s| s.secondary.as_deref()) {
        gpu_bytes = gpu_bytes
            .checked_add(child.gpu_bytes)
            .ok_or_else(|| PilError::ValueError("GPU graph byte count overflow".into()))?;
        host_bytes = host_bytes
            .checked_add(child.host_bytes)
            .ok_or_else(|| PilError::ValueError("GPU graph byte count overflow".into()))?;
    }
    Ok(Plan {
        source,
        stages,
        width,
        height,
        mode,
        capacity,
        gpu_bytes,
        host_bytes,
        metadata_bytes,
        shader_work: bounds.work - starting_work,
        op_count: bounds.ops - starting_ops,
    })
}

fn rgba_standalone_native(op: &PipelineOp) -> bool {
    // Reject incomplete channel tables before source decode/upload. The
    // ordinary encoder also validates LUTs, but that is too late for admission.
    if let PipelineOp::Eval { lut } = op {
        return lut.len() == 4 * 256;
    }
    matches!(
        op,
        PipelineOp::Brightness { .. }
            | PipelineOp::ColorSaturation { .. }
            | PipelineOp::Filter3x3 { .. }
            | PipelineOp::Filter5x5 { .. }
            | PipelineOp::BoxBlur { .. }
            | PipelineOp::BoxBlurXY { .. }
            | PipelineOp::GaussianBlur { .. }
            | PipelineOp::MedianFilter { .. }
            | PipelineOp::MinFilter { .. }
            | PipelineOp::MaxFilter { .. }
            | PipelineOp::RankFilter { .. }
            | PipelineOp::Crop { .. }
            | PipelineOp::Transpose { .. }
            | PipelineOp::Flip
            | PipelineOp::Mirror
            | PipelineOp::Expand { .. }
            | PipelineOp::PutPixel {
                palette_index: false,
                ..
            }
            | PipelineOp::ExtractBand { .. }
            | PipelineOp::Grayscale
            | PipelineOp::Constant { .. }
    )
}

struct CompileCredit {
    gpu: &'static GpuInner,
    credit: Option<Credit>,
}
impl Drop for CompileCredit {
    fn drop(&mut self) {
        if self.credit.is_some() {
            // Queue writes survive failed encoding. Flush transfers only and
            // retain their reservation until this fence completes.
            let fence = StagingBuffer::new(&self.gpu.device, 8);
            let mut encoder =
                self.gpu
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("gpu_stream_failed_compile_fence"),
                    });
            encoder.clear_buffer(&fence.buffer, 0, None);
            self.gpu.queue.submit([encoder.finish()]);
            let _ = self.gpu.readback_with(8, &fence.buffer, |_| Ok(()));
        }
    }
}
struct EncodedGraph {
    buffer: wgpu::Buffer,
    dispatches: u64,
    upload: u64,
    retained: u64,
}
fn compile_graph(
    template: &Image,
    key: Option<BatchKey>,
    id: u64,
    plan: Plan<'_>,
    credit: Credit,
    gpu: &'static GpuInner,
) -> Result<Job, PilError> {
    let mut reservation = CompileCredit {
        gpu,
        credit: Some(credit),
    };
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("gpu_stream_graph"),
        });
    let encoded = encode_graph(&plan, gpu, &mut encoder)?;
    let length =
        CheckedDims::new(plan.width, plan.height, planned_channels(&plan.mode)?)?.total_bytes();
    if encoded.retained + aligned_bytes(length, 8) as u64 > plan.gpu_bytes {
        return Err(PilError::ValueError(
            "GPU generated resources exceed admission reservation".into(),
        ));
    }
    Ok(Job {
        id,
        key,
        template: template.gpu_result_metadata(),
        width: plan.width,
        height: plan.height,
        mode: plan.mode,
        length,
        output: Some(encoded.buffer),
        command: Some(encoder.finish()),
        credit: reservation.credit.take().ok_or_else(|| {
            PilError::InternalError("GPU compile reservation was already transferred".into())
        })?,
        output_capacity: u64::from(plan.capacity) * 4,
        dispatches: encoded.dispatches,
        shader_work: plan.shader_work,
        op_count: plan.op_count,
        metadata_bytes: plan.metadata_bytes,
        upload: encoded.upload,
    })
}
fn encode_graph(
    plan: &Plan<'_>,
    gpu: &'static GpuInner,
    encoder: &mut wgpu::CommandEncoder,
) -> Result<EncodedGraph, PilError> {
    let source = match &plan.source.cached {
        Some(pixels) => pixels.clone(),
        None => plan.source.source.materialized_shared()?,
    };
    let channels = planned_channels(&plan.source.mode)?;
    let physical_matches = matches!(
        (channels, source.as_ref()),
        (1, DynamicImage::ImageLuma8(_))
            | (2, DynamicImage::ImageLumaA8(_))
            | (3, DynamicImage::ImageRgb8(_))
            | (4, DynamicImage::ImageRgba8(_))
    );
    if !physical_matches
        || source.as_bytes().len()
            != CheckedDims::new(plan.source.width, plan.source.height, channels)?.total_bytes()
    {
        return Err(PilError::ValueError(
            "decoded source does not match the admitted native layout".into(),
        ));
    }
    let mut buffers = BufferPool::new(&gpu.device, plan.capacity, false);
    let source_bytes = source.as_bytes();
    let aligned = aligned_bytes(source_bytes.len(), 4);
    let mut upload = gpu
        .queue
        .write_buffer_with(
            &buffers.buf_a,
            0,
            NonZeroU64::new(aligned as u64)
                .ok_or_else(|| PilError::InternalError("GPU source upload is empty".into()))?,
        )
        .ok_or_else(|| PilError::InternalError("GPU source upload allocation failed".into()))?;
    upload[..source_bytes.len()].copy_from_slice(source_bytes);
    upload[source_bytes.len()..].fill(0);
    drop(upload);
    let mut current_a = true;
    let mut retained = gpu_working_set_bytes(plan.capacity);
    let mut dispatches = 0;
    let mut uploaded_bytes = source_bytes.len() as u64;
    let auxiliary = [AuxiliaryImages {
        second: None,
        third: None,
    }];
    for stage in &plan.stages {
        let secondary = stage
            .secondary
            .as_deref()
            .map(|child| encode_graph(child, gpu, encoder))
            .transpose()?;
        if let Some(child) = &secondary {
            retained += child.retained;
            uploaded_bytes += child.upload;
            dispatches += child.dispatches;
        }
        if stage.flags.rgb_blur {
            current_a = encode_rgb_blur_stage(gpu, encoder, &buffers, stage, current_a)?;
        } else if matches!(stage.op, PipelineOp::Paste { .. }) {
            current_a = encode_paste_stage(
                gpu,
                encoder,
                &buffers,
                stage,
                secondary.as_ref().ok_or_else(|| {
                    PilError::InternalError("GPU Paste source was not encoded".into())
                })?,
                current_a,
            )?;
        } else if stage.relocation {
            current_a = encode_relocation_stage(gpu, encoder, &buffers, stage, current_a)?;
        } else if stage.byte {
            current_a =
                encode_byte_stage(gpu, encoder, &buffers, stage, secondary.as_ref(), current_a)?;
        } else {
            // A fresh arena per stage prevents queue.write_buffer from changing
            // uniforms used by earlier commands before this graph is submitted.
            buffers.params_arena = ReusableGpuBuffer::new(
                &gpu.device,
                "gpu_stream_params",
                wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                4,
                4,
            );
            buffers.img2_arena = ReusableGpuBuffer::new(
                &gpu.device,
                "gpu_stream_aux",
                wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                4,
                4,
            );
            buffers.img3_arena = ReusableGpuBuffer::new(
                &gpu.device,
                "gpu_stream_mask",
                wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                4,
                4,
            );
            buffers.lut_arena = ReusableGpuBuffer::new(
                &gpu.device,
                "gpu_stream_lut",
                wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                1024,
                4,
            );
            let flags = stage.flags;
            // Grayscale on native L/LA selects its luma sample directly.
            // Keep the canonical graph; only the internal kernel is lowered.
            let encoded_op = if flags.extract && matches!(stage.op, PipelineOp::Grayscale) {
                &LUMA_BAND
            } else {
                stage.op
            };
            let estimated = gpu.estimate_resource_bytes(
                encoded_op,
                &auxiliary[0],
                &buffers,
                gpu.device.limits().min_uniform_buffer_offset_alignment as usize,
                gpu.device.limits().min_storage_buffer_offset_alignment as usize,
                (stage.width, stage.height),
                match stage.mode.as_str() {
                    "L" => 0,
                    "LA" => 1,
                    "RGB" => 2,
                    _ => 3,
                },
                false,
                Some(&stage.mode),
                false,
                false,
            )?;
            if estimated > (64 << 10) {
                return Err(PilError::ValueError(
                    "GPU stage arenas exceed their bounded admission reservation".into(),
                ));
            }
            let cache = GpuAuxiliaryCache::from_batch(
                std::slice::from_ref(encoded_op),
                &auxiliary,
                match stage.mode.as_str() {
                    "L" => 0,
                    "LA" => 1,
                    "RGB" => 2,
                    _ => 3,
                },
                plan.capacity,
                gpu.device.limits().min_storage_buffer_offset_alignment as usize,
            )?;
            let prepared = gpu.prepare_batch(
                std::slice::from_ref(encoded_op),
                &auxiliary,
                stage.width,
                stage.height,
                match stage.mode.as_str() {
                    "L" => 0,
                    "LA" => 1,
                    "RGB" => 2,
                    "CMYK" => 4,
                    "RGBX" => 6,
                    _ => 3,
                },
                Some(&stage.mode),
                None,
                None,
                false,
                false,
                false,
                false,
                false,
                false,
                flags.luma_colorize,
                flags.luma_convert,
                false,
                false,
                flags.rgb,
                flags.rgb && !matches!(stage.op, PipelineOp::Convert { .. }),
                false,
                flags.luma_point,
                false,
                flags.luma_order,
                flags.byte_filter,
                false,
                flags.extract,
                flags.grayscale_rgb,
                false,
                false,
                false,
                false,
                None,
                None,
                None,
                &mut buffers,
                &cache,
            )?;
            current_a = gpu.encode_batch(
                encoder,
                std::slice::from_ref(encoded_op),
                &prepared,
                current_a,
                Some(&stage.mode),
                flags.luma_point,
                false,
                flags.luma_order,
                flags.byte_filter,
                false,
                false,
                false,
                false,
                false,
                false,
            )?;
            drop(prepared);
            retained += buffers.params_arena.capacity_bytes
                + buffers.img2_arena.capacity_bytes
                + buffers.img3_arena.capacity_bytes
                + buffers.lut_arena.capacity_bytes;
        }
        dispatches += gpu_dispatch_count(
            std::slice::from_ref(stage.op),
            Some(&stage.mode),
            (stage.width, stage.height),
        );
    }
    let output = if current_a {
        buffers.buf_a.clone()
    } else {
        buffers.buf_b.clone()
    };
    Ok(EncodedGraph {
        buffer: output,
        dispatches,
        upload: uploaded_bytes,
        retained,
    })
}

fn encode_byte_stage(
    gpu: &GpuInner,
    encoder: &mut wgpu::CommandEncoder,
    buffers: &BufferPool,
    stage: &Stage<'_>,
    secondary: Option<&EncodedGraph>,
    current_a: bool,
) -> Result<bool, PilError> {
    let channels = planned_channels(&stage.mode)?;
    let length = CheckedDims::new(stage.width, stage.height, channels)?.total_bytes();
    let words = u32::try_from(length.div_ceil(4))
        .map_err(|_| PilError::ValueError("native shader index overflow".into()))?;
    let columns = words.min(1024);
    let rows = words.div_ceil(columns);
    let (key, file, source, param) = match stage.op {
        PipelineOp::Duplicate => (
            "__internal_duplicate_native_bytes",
            "duplicate.wgsl",
            include_str!("shaders/duplicate.wgsl"),
            None,
        ),
        PipelineOp::Invert | PipelineOp::InvertChops => (
            "__internal_solarize_native_bytes",
            "solarize.wgsl",
            include_str!("shaders/solarize.wgsl"),
            Some(0),
        ),
        PipelineOp::Solarize { threshold } => (
            "__internal_solarize_native_bytes",
            "solarize.wgsl",
            include_str!("shaders/solarize.wgsl"),
            Some(u32::from(*threshold)),
        ),
        PipelineOp::Posterize { bits } => (
            "__internal_posterize_native_bytes",
            "posterize.wgsl",
            include_str!("shaders/posterize.wgsl"),
            Some(u32::from(*bits)),
        ),
        PipelineOp::Multiply { .. } => (
            "Multiply",
            "multiply.wgsl",
            include_str!("shaders/multiply.wgsl"),
            None,
        ),
        PipelineOp::Screen { .. } => (
            "Screen",
            "screen.wgsl",
            include_str!("shaders/screen.wgsl"),
            None,
        ),
        PipelineOp::Overlay { .. } => (
            "Overlay",
            "overlay.wgsl",
            include_str!("shaders/overlay.wgsl"),
            None,
        ),
        PipelineOp::HardLight { .. } => (
            "HardLight",
            "hard_light.wgsl",
            include_str!("shaders/hard_light.wgsl"),
            None,
        ),
        PipelineOp::SoftLight { .. } => (
            "SoftLight",
            "soft_light.wgsl",
            include_str!("shaders/soft_light.wgsl"),
            None,
        ),
        PipelineOp::Difference { .. } => (
            "Difference",
            "difference.wgsl",
            include_str!("shaders/difference.wgsl"),
            None,
        ),
        PipelineOp::Darker { .. } => (
            "Darker",
            "darker.wgsl",
            include_str!("shaders/darker.wgsl"),
            None,
        ),
        PipelineOp::Lighter { .. } => (
            "Lighter",
            "lighter.wgsl",
            include_str!("shaders/lighter.wgsl"),
            None,
        ),
        PipelineOp::AddModulo { .. } => (
            "AddModulo",
            "add_modulo.wgsl",
            include_str!("shaders/add_modulo.wgsl"),
            None,
        ),
        PipelineOp::SubtractModulo { .. } => (
            "SubtractModulo",
            "subtract_modulo.wgsl",
            include_str!("shaders/subtract_modulo.wgsl"),
            None,
        ),
        PipelineOp::BlendModule { alpha, .. } => (
            "BlendModule",
            "blend_module.wgsl",
            include_str!("shaders/blend_module.wgsl"),
            Some((*alpha as f32).to_bits()),
        ),
        _ => {
            return Err(PilError::InternalError(
                "unrecognized native byte stage".into(),
            ));
        }
    };
    let cached = gpu.resolve_pipeline(key, file, source)?;
    let mut parameters = vec![columns, rows, 9, words];
    if let Some(param) = param {
        parameters.push(param);
    }
    if matches!(stage.op, PipelineOp::BlendModule { .. }) {
        parameters.resize(8, 0);
    }
    let uniform = create_sized_buffer(
        &gpu.device,
        "gpu_stream_byte_params",
        wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        parameters.len() * 4,
        4,
    );
    gpu.queue
        .write_buffer(&uniform, 0, bytemuck::cast_slice(&parameters));
    let (input, output) = if current_a {
        (&buffers.buf_a, &buffers.buf_b)
    } else {
        (&buffers.buf_b, &buffers.buf_a)
    };
    let entries = if let Some(secondary) = secondary {
        vec![
            wgpu::BindGroupEntry {
                binding: 0,
                resource: input.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: secondary.buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: output.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: uniform.as_entire_binding(),
            },
        ]
    } else {
        vec![
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
                resource: uniform.as_entire_binding(),
            },
        ]
    };
    let bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("gpu_stream_byte_bind"),
        layout: &cached.bind_group_layout,
        entries: &entries,
    });
    let groups = (columns.div_ceil(16), rows.div_ceil(16));
    if groups.0 > gpu.device.limits().max_compute_workgroups_per_dimension
        || groups.1 > gpu.device.limits().max_compute_workgroups_per_dimension
    {
        return Err(PilError::ValueError(
            "native byte dispatch exceeds device limit".into(),
        ));
    }
    let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
        label: Some("gpu_stream_native_bytes"),
        timestamp_writes: None,
    });
    pass.set_pipeline(&cached.pipeline);
    pass.set_bind_group(0, &bind, &[]);
    pass.dispatch_workgroups(groups.0, groups.1, 1);
    crate::compute::record_gpu_shader_dispatch(
        cached.variant_name,
        cached.shader_file,
        u64::from(groups.0) * u64::from(groups.1),
    );
    Ok(!current_a)
}

fn encode_relocation_stage(
    gpu: &GpuInner,
    encoder: &mut wgpu::CommandEncoder,
    buffers: &BufferPool,
    stage: &Stage,
    current_a: bool,
) -> Result<bool, PilError> {
    let output =
        op_output_dims(stage.op, stage.width, stage.height).unwrap_or((stage.width, stage.height));
    let channels = planned_channels(&stage.mode)?;
    let (opcode, offset_x, offset_y, fill) = match stage.op {
        PipelineOp::Transpose { .. } => (registry::extract_params(stage.op)[0], 0, 0, (0, 0, 0, 0)),
        PipelineOp::Mirror => (0, 0, 0, (0, 0, 0, 0)),
        PipelineOp::Flip => (1, 0, 0, (0, 0, 0, 0)),
        PipelineOp::Crop { left, top, .. } => (7, *left, *top, (0, 0, 0, 0)),
        PipelineOp::CropBorder { border } => (7, *border, *border, (0, 0, 0, 0)),
        PipelineOp::Expand { border, fill } => (8, *border, *border, *fill),
        _ => {
            return Err(PilError::InternalError(
                "unrecognized native relocation".into(),
            ));
        }
    };
    let parameters = [
        stage.width,
        stage.height,
        output.0,
        output.1,
        u32::from(channels),
        opcode,
        offset_x,
        offset_y,
        u32::from(fill.0)
            | u32::from(fill.1) << 8
            | u32::from(fill.2) << 16
            | u32::from(fill.3) << 24,
    ];
    let cached = gpu.resolve_pipeline(
        "__internal_relocate_native_bytes",
        "relocate_native_bytes.wgsl",
        include_str!("shaders/relocate_native_bytes.wgsl"),
    )?;
    let uniform = create_sized_buffer(
        &gpu.device,
        "gpu_stream_relocation_params",
        wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        parameters.len() * 4,
        4,
    );
    gpu.queue
        .write_buffer(&uniform, 0, bytemuck::cast_slice(&parameters));
    let (input, output_buffer) = if current_a {
        (&buffers.buf_a, &buffers.buf_b)
    } else {
        (&buffers.buf_b, &buffers.buf_a)
    };
    let bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("gpu_stream_relocation_bind"),
        layout: &cached.bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: input.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: output_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: uniform.as_entire_binding(),
            },
        ],
    });
    let bytes = CheckedDims::new(output.0, output.1, channels)?.total_bytes();
    let groups = plan_extract_band_dispatch(
        u32::try_from(bytes)
            .map_err(|_| PilError::ValueError("relocation native byte indexing overflow".into()))?,
        1,
        gpu.device.limits().max_compute_workgroups_per_dimension,
    )?;
    let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
        label: Some("gpu_stream_relocation"),
        timestamp_writes: None,
    });
    pass.set_pipeline(&cached.pipeline);
    pass.set_bind_group(0, &bind, &[]);
    pass.dispatch_workgroups(groups.0, groups.1, 1);
    crate::compute::record_gpu_shader_dispatch(
        cached.variant_name,
        cached.shader_file,
        u64::from(groups.0) * u64::from(groups.1),
    );
    Ok(!current_a)
}

fn encode_paste_stage(
    gpu: &GpuInner,
    encoder: &mut wgpu::CommandEncoder,
    buffers: &BufferPool,
    stage: &Stage<'_>,
    secondary: &EncodedGraph,
    current_a: bool,
) -> Result<bool, PilError> {
    let PipelineOp::Paste { x, y, .. } = stage.op else {
        unreachable!()
    };
    let child = stage.secondary.as_deref().ok_or_else(|| {
        PilError::InternalError("GPU Paste source is missing from its plan".into())
    })?;
    let channels = planned_channels(&stage.mode)?;
    let dims = CheckedDims::new(stage.width, stage.height, channels)?;
    let words = u32::try_from(dims.total_bytes().div_ceil(4))
        .map_err(|_| PilError::ValueError("GPU paste byte indexing overflow".into()))?;
    let parameters = [
        stage.width,
        stage.height,
        child.width,
        child.height,
        dims.total_pixels() as u32,
        words,
        u32::from(channels),
        0,
        *x as u32,
        *y as u32,
        0,
        0,
    ];
    let cached = gpu.resolve_pipeline(
        "__internal_stream_native_paste",
        "paste_native_bytes.wgsl",
        include_str!("shaders/paste_native_bytes.wgsl"),
    )?;
    let uniform = create_sized_buffer(
        &gpu.device,
        "gpu_stream_paste_params",
        wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        48,
        4,
    );
    gpu.queue
        .write_buffer(&uniform, 0, bytemuck::cast_slice(&parameters));
    let (input, output) = if current_a {
        (&buffers.buf_a, &buffers.buf_b)
    } else {
        (&buffers.buf_b, &buffers.buf_a)
    };
    let bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("gpu_stream_paste_bind"),
        layout: &cached.bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: input.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: secondary.buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: output.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: uniform.as_entire_binding(),
            },
        ],
    });
    let groups = plan_extract_band_dispatch(
        dims.total_bytes() as u32,
        1,
        gpu.device.limits().max_compute_workgroups_per_dimension,
    )?;
    let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
        label: Some("gpu_stream_paste"),
        timestamp_writes: None,
    });
    pass.set_pipeline(&cached.pipeline);
    pass.set_bind_group(0, &bind, &[]);
    pass.dispatch_workgroups(groups.0, groups.1, 1);
    crate::compute::record_gpu_shader_dispatch(
        cached.variant_name,
        cached.shader_file,
        u64::from(groups.0) * u64::from(groups.1),
    );
    Ok(!current_a)
}

fn encode_rgb_blur_stage(
    gpu: &GpuInner,
    encoder: &mut wgpu::CommandEncoder,
    buffers: &BufferPool,
    stage: &Stage<'_>,
    mut current_a: bool,
) -> Result<bool, PilError> {
    let limits = gpu.device.limits();
    let plan = plan_native_rgb_point_output(
        stage.width,
        stage.height,
        limits.max_compute_workgroups_per_dimension,
        limits.max_storage_buffer_binding_size,
        limits.max_buffer_size,
        buffers.capacity,
    )
    .ok_or_else(|| PilError::ValueError("native RGB blur dispatch exceeds device limits".into()))?;
    let mut params = vec![
        stage.width,
        stage.height,
        2,
        if plan.row_tiled { 16 } else { 0 },
    ];
    params.extend(registry::extract_params(stage.op));
    let uniform = create_sized_buffer(
        &gpu.device,
        "gpu_stream_rgb_blur_params",
        wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        params.len() * 4,
        4,
    );
    gpu.queue
        .write_buffer(&uniform, 0, bytemuck::cast_slice(&params));
    let passes = GpuInner::blur_pass_count(stage.op)
        .ok_or_else(|| PilError::InternalError("missing native RGB blur pass count".into()))?;
    for (key, file, source) in [
        (
            "__internal_blur_h_rgb_packed",
            "box_blur_h_rgb_packed.wgsl",
            include_str!("shaders/box_blur_h_rgb_packed.wgsl"),
        ),
        (
            "__internal_blur_v_rgb_packed",
            "box_blur_v_rgb_packed.wgsl",
            include_str!("shaders/box_blur_v_rgb_packed.wgsl"),
        ),
    ] {
        let cached = gpu.resolve_pipeline(key, file, source)?;
        for _ in 0..passes {
            let (input, output) = if current_a {
                (&buffers.buf_a, &buffers.buf_b)
            } else {
                (&buffers.buf_b, &buffers.buf_a)
            };
            let bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("gpu_stream_rgb_blur_bind"),
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
                        resource: uniform.as_entire_binding(),
                    },
                ],
            });
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("gpu_stream_rgb_blur"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&cached.pipeline);
            pass.set_bind_group(0, &bind, &[]);
            pass.dispatch_workgroups(plan.groups_x, plan.groups_y, 1);
            crate::compute::record_gpu_shader_dispatch(
                cached.variant_name,
                cached.shader_file,
                u64::from(plan.groups_x) * u64::from(plan.groups_y),
            );
            current_a = !current_a;
        }
    }
    Ok(current_a)
}

#[cfg(test)]
mod tests {
    use super::{
        Credit, GraphBounds, MAX_GPU_OPS_PER_SUBMISSION, MAX_GPU_SHADER_WORK_ITEMS, Usage,
        rgba_standalone_native,
    };
    use crate::pipeline::PipelineOp;
    use std::sync::{Arc, Mutex};

    #[test]
    fn rgba_point_admission_requires_all_four_channel_tables() {
        for length in [0, 256, 768, 1023, 1025] {
            let op = PipelineOp::Eval {
                lut: Arc::from(vec![0; length]),
            };
            assert!(!rgba_standalone_native(&op), "invalid LUT length {length}");
        }
        let op = PipelineOp::Eval {
            lut: Arc::from((0..4).flat_map(|_| 0u8..=255).collect::<Vec<_>>()),
        };
        assert!(rgba_standalone_native(&op));
    }

    #[test]
    fn graph_budget_checks_aggregate_boundaries_without_allocations() {
        let mut bounds = GraphBounds::default();
        bounds.add(MAX_GPU_SHADER_WORK_ITEMS - 1).unwrap();
        bounds.add(1).unwrap();
        assert!(bounds.add(1).is_err());
        assert_eq!(bounds.work, MAX_GPU_SHADER_WORK_ITEMS);
        let mut bounds = GraphBounds::default();
        for _ in 0..MAX_GPU_OPS_PER_SUBMISSION {
            bounds.add(0).unwrap();
        }
        assert!(bounds.add(0).is_err());
        let mut overflow = GraphBounds {
            work: u64::MAX,
            ops: 0,
        };
        assert!(overflow.add(1).is_err());
    }

    #[test]
    fn credits_follow_arena_and_resident_owners_until_the_final_drop() {
        let usage = Arc::new(Mutex::new(Usage {
            gpu: 100,
            host: 80,
            gpu_limit: 100,
            host_limit: 80,
        }));
        let mut job = Credit {
            usage: usage.clone(),
            gpu: 100,
            host: 80,
        };
        let arena = Arc::new(job.split(20, 20));
        let another_range = arena.clone();
        job.reduce_to(30, 10);
        {
            let held = usage.lock().unwrap();
            assert_eq!((held.gpu, held.host), (50, 30));
        }
        drop(job);
        drop(arena);
        {
            let held = usage.lock().unwrap();
            assert_eq!((held.gpu, held.host), (20, 20));
        }
        drop(another_range);
        let held = usage.lock().unwrap();
        assert_eq!((held.gpu, held.host), (0, 0));
    }
}
