//! Explicit GPU graph scheduling with bounded ownership and terminal-only readback.
//!
//! Each job records the existing PipelineOp graph in its own command buffer.
//! Several command buffers and their terminal staging copies share one queue
//! submission. This does not concatenate pixels or change image borders.

mod encoding;
mod graph;
mod types;

use self::encoding::compile_graph;
use self::graph::plan_graph;
pub use self::types::{
    BatchError, BatchErrorKind, BatchKey, BatchOutput, BatchResult, GpuBatchConfig, GpuBatchStats,
    GpuImage,
};
use self::types::{Credit, Usage};
use super::{
    GpuInner, GpuPool, MAX_GPU_OPS_PER_SUBMISSION, MAX_GPU_SHADER_WORK_ITEMS, StagingBuffer,
    aligned_bytes,
};
use crate::Image;
use crate::error::PilError;
use crate::image::GpuResultMetadata;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

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
            BatchOutput::Gpu(GpuImage::from_parts(
                self.gpu,
                job.output
                    .take()
                    .expect("resident terminal buffer retained"),
                job.width,
                job.height,
                job.mode,
                job.length,
                job.template,
                credit,
            ))
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
fn native_channels(mode: &str) -> Option<u8> {
    match mode {
        "L" => Some(1),
        "LA" => Some(2),
        "RGB" => Some(3),
        // F stores one opaque f32 word per pixel in the RGBA transport buffer.
        "RGBA" | "CMYK" | "RGBX" | "F" => Some(4),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{Credit, GpuBatchConfig, Usage};
    use std::sync::{Arc, Mutex};

    #[test]
    fn default_batch_configuration_uses_measured_in_flight_limit() {
        let config = GpuBatchConfig::default();
        assert_eq!(config.max_jobs, 64);
        assert_eq!(config.max_in_flight, 4);
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
