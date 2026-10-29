//! Public batch result types and byte-credit ownership for explicit GPU streaming.

use super::super::{GpuInner, StagingBuffer, aligned_bytes};
use super::native_image;
use crate::Image;
use crate::error::PilError;
use crate::image::GpuResultMetadata;
use std::sync::{Arc, Mutex};

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
            max_in_flight: 4,
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
pub(super) struct Usage {
    pub(super) gpu: u64,
    pub(super) host: u64,
    pub(super) gpu_limit: u64,
    pub(super) host_limit: u64,
}
pub(super) struct Credit {
    pub(super) usage: Arc<Mutex<Usage>>,
    pub(super) gpu: u64,
    pub(super) host: u64,
}
impl Drop for Credit {
    fn drop(&mut self) {
        let mut usage = self.usage.lock().unwrap_or_else(|e| e.into_inner());
        usage.gpu -= self.gpu;
        usage.host -= self.host;
    }
}
impl Credit {
    pub(super) fn reduce_to(&mut self, gpu: u64, host: u64) {
        assert!(gpu <= self.gpu && host <= self.host);
        let mut usage = self.usage.lock().unwrap_or_else(|e| e.into_inner());
        usage.gpu -= self.gpu - gpu;
        usage.host -= self.host - host;
        self.gpu = gpu;
        self.host = host;
    }
    pub(super) fn split(&mut self, gpu: u64, host: u64) -> Self {
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
    pub(super) fn from_parts(
        gpu: &'static GpuInner,
        buffer: wgpu::Buffer,
        width: u32,
        height: u32,
        mode: String,
        length: usize,
        template: GpuResultMetadata,
        credit: Credit,
    ) -> Self {
        Self {
            inner: Arc::new(Resident {
                gpu,
                buffer,
                width,
                height,
                mode,
                length,
                template,
                download_lock: Mutex::new(()),
                _credit: credit,
            }),
        }
    }

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
