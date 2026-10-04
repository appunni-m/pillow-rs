//! Explicit batching for compatible image jobs.
//!
//! This API is separate from [`crate::Image`] methods and never changes normal
//! image routing. Grouped workloads reuse the existing operation pipelines
//! over compatible native-mode images packed into one image, then split the
//! results back into ordinary per-image results. Grouped operations reuse
//! `MedianFilter(3)`, `MaxFilter(3)`, native-L `RankFilter(3, 1)`,
//! `ExtractBand`, `ImageOps.invert`, native-mode `Brightness`,
//! `ImageChops.multiply`, and same-mode RGBA `Color3DLUT` pipelines. Full-frame
//! native-mode masked Paste and Composite jobs with L masks, plus native-mode
//! `ImageOps.expand` jobs, also reuse their existing pipelines. Images that
//! cannot be grouped use their ordinary single-image pipeline.

use crate::compute::Backend;
use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::{PipelineOp, PixelMode};
use std::sync::Arc;

#[cfg(feature = "migration-fault-injection")]
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(feature = "migration-fault-injection")]
static RANK_FILTER_GROUP_FAILURE_INJECTED: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "migration-fault-injection")]
static EXPAND_GROUP_FAILURE_INJECTED: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "migration-fault-injection")]
static PASTE_GROUP_FAILURE_INJECTED: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "migration-fault-injection")]
static COMPOSITE_GROUP_FAILURE_INJECTED: AtomicBool = AtomicBool::new(false);

#[cfg(feature = "migration-fault-injection")]
fn injected_rank_filter_group_failure(job: &BatchJob, backend: Backend) -> Option<PilError> {
    if backend != Backend::Gpu
        || !matches!(
            &job.operation,
            BatchOperation::RankFilter { size: 3, rank: 1 }
        )
        || RANK_FILTER_GROUP_FAILURE_INJECTED.load(Ordering::Relaxed)
    {
        return None;
    }

    let error = match std::env::var("PILLOW_RS_MIGRATION_FAULT_POINT").as_deref() {
        Ok("image_batch.rank_filter.group_dimension_failure") => {
            PilError::DimensionError("injected grouped RankFilter dimension failure".into())
        }
        Ok("image_batch.rank_filter.group_memory_failure") => {
            PilError::MemoryError("injected grouped RankFilter memory failure".into())
        }
        _ => return None,
    };

    RANK_FILTER_GROUP_FAILURE_INJECTED
        .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
        .ok()?;
    Some(error)
}

#[cfg(feature = "migration-fault-injection")]
fn injected_expand_group_failure(job: &BatchJob, backend: Backend) -> Option<PilError> {
    if backend != Backend::Gpu
        || !matches!(&job.operation, BatchOperation::Expand { .. })
        || EXPAND_GROUP_FAILURE_INJECTED.load(Ordering::Relaxed)
    {
        return None;
    }

    let error = match std::env::var("PILLOW_RS_MIGRATION_FAULT_POINT").as_deref() {
        Ok("image_batch.expand.group_dimension_failure") => {
            PilError::DimensionError("injected grouped Expand dimension failure".into())
        }
        Ok("image_batch.expand.group_memory_failure") => {
            PilError::MemoryError("injected grouped Expand memory failure".into())
        }
        _ => return None,
    };

    EXPAND_GROUP_FAILURE_INJECTED
        .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
        .ok()?;
    Some(error)
}

#[cfg(feature = "migration-fault-injection")]
fn injected_paste_group_failure(job: &BatchJob, backend: Backend) -> Option<PilError> {
    if backend != Backend::Gpu
        || !matches!(&job.operation, BatchOperation::Paste { .. })
        || PASTE_GROUP_FAILURE_INJECTED.load(Ordering::Relaxed)
    {
        return None;
    }

    let error = match std::env::var("PILLOW_RS_MIGRATION_FAULT_POINT").as_deref() {
        Ok("image_batch.paste.group_dimension_failure") => {
            PilError::DimensionError("injected grouped Paste dimension failure".into())
        }
        Ok("image_batch.paste.group_memory_failure") => {
            PilError::MemoryError("injected grouped Paste memory failure".into())
        }
        _ => return None,
    };

    PASTE_GROUP_FAILURE_INJECTED
        .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
        .ok()?;
    Some(error)
}

#[cfg(feature = "migration-fault-injection")]
fn injected_composite_group_failure(job: &BatchJob, backend: Backend) -> Option<PilError> {
    if backend != Backend::Gpu
        || !matches!(&job.operation, BatchOperation::Composite { .. })
        || COMPOSITE_GROUP_FAILURE_INJECTED.load(Ordering::Relaxed)
    {
        return None;
    }

    let error = match std::env::var("PILLOW_RS_MIGRATION_FAULT_POINT").as_deref() {
        Ok("image_batch.composite.group_dimension_failure") => {
            PilError::DimensionError("injected grouped Composite dimension failure".into())
        }
        Ok("image_batch.composite.group_memory_failure") => {
            PilError::MemoryError("injected grouped Composite memory failure".into())
        }
        _ => return None,
    };

    COMPOSITE_GROUP_FAILURE_INJECTED
        .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
        .ok()?;
    Some(error)
}

/// One explicitly submitted Pillow operation.
#[derive(Debug, Clone)]
pub enum BatchOperation {
    /// Apply Pillow's `ImageFilter.MedianFilter(size)` operation.
    MedianFilter {
        /// Odd square filter size.
        size: u32,
    },
    /// Apply Pillow's `ImageFilter.MaxFilter(size)` operation.
    MaxFilter {
        /// Odd square filter size.
        size: u32,
    },
    /// Apply Pillow's native-L `ImageFilter.RankFilter(3, rank=1)` operation.
    RankFilter {
        /// Odd square filter size.
        size: u32,
        /// Zero-based order-statistic rank.
        rank: u32,
    },
    /// Extract one byte channel using Pillow's `Image.getchannel` operation.
    ExtractBand {
        /// Zero-based source channel index.
        channel: i32,
    },
    /// Apply Pillow's `ImageOps.invert(image)` operation.
    Invert,
    /// Apply Pillow's `ImageEnhance.Brightness(image).enhance(factor)` operation.
    Brightness {
        /// Brightness multiplier.
        factor: f64,
    },
    /// Apply Pillow's `ImageChops.multiply(image, other)` operation.
    Multiply {
        /// The second image operand. Compatible images are stacked separately
        /// in their native mode and passed through the ordinary Multiply pipeline.
        other: Box<Image>,
    },
    /// Paste a same-sized image through an L mask at the origin.
    Paste {
        /// Source image to paste.
        source: Box<Image>,
        /// L-mode mask with the same dimensions as the destination.
        mask: Box<Image>,
    },
    /// Composite a foreground and background through a same-size L mask.
    Composite {
        /// Background image; the result keeps this image's mode and metadata.
        background: Box<Image>,
        /// L-mode mask with the same dimensions as the foreground.
        mask: Box<Image>,
    },
    /// Add a native-mode border using Pillow's `ImageOps.expand` semantics.
    Expand {
        /// Symmetric border width in pixels.
        border: u32,
        /// Pillow ImageOps fill input, resolved separately for each image mode.
        fill: crate::ImageOpsColor,
    },
    /// Apply a shared same-mode RGBA 3D color lookup table.
    Color3DLut {
        /// LUT dimensions.
        size: (u32, u32, u32),
        /// Shared Pillow-order LUT values.
        table: Arc<[f64]>,
        /// Number of output channels per LUT entry.
        channels: u32,
        /// Explicit target mode, if supplied by the filter.
        target_mode: Option<String>,
    },
}

impl BatchOperation {
    fn apply(&self, image: &Image) -> Result<Image, PilError> {
        match self {
            Self::MedianFilter { size } => image.median_filter(*size),
            Self::MaxFilter { size } => image.max_filter(*size),
            Self::RankFilter { size, rank } => image.rank_filter(*size, *rank),
            Self::ExtractBand { channel } => image.getchannel(*channel),
            Self::Invert => crate::ops::imageops::invert_ops(image),
            Self::Brightness { factor } => image.enhance_brightness(*factor),
            Self::Multiply { other } => crate::ops::chops::multiply(image, other),
            Self::Paste { source, mask } => {
                let mut output = image.clone();
                output.paste_at(
                    crate::PasteSource::Image(Box::new((**source).clone())),
                    None,
                    Some(mask),
                )?;
                Ok(output)
            }
            Self::Composite { background, mask } => {
                crate::ops::module_fns::composite(image, background, mask)
            }
            Self::Expand { border, fill } => {
                crate::ops::imageops::expand_with_input(image, *border, fill.clone())
            }
            Self::Color3DLut {
                size,
                table,
                channels,
                target_mode,
            } => {
                if image.mode()? == "RGBA"
                    && *channels == 4
                    && matches!(target_mode.as_deref(), None | Some("RGBA"))
                {
                    Ok(Image::push_mode_changing_op(
                        image,
                        self.pipeline_op()
                            .expect("same-mode RGBA LUT has a pipeline operation"),
                        "RGBA",
                    ))
                } else {
                    image.color3dlut(
                        crate::PreparedColor3DLut {
                            size: *size,
                            table: table.to_vec(),
                            channels: *channels,
                        },
                        target_mode.as_deref(),
                    )
                }
            }
        }
    }

    fn can_group(&self, mode: &str, size: (u32, u32)) -> bool {
        let Some(channels) = mode_channels(mode) else {
            return false;
        };
        match self {
            Self::MedianFilter { size } | Self::MaxFilter { size } => *size == 3,
            Self::RankFilter { size, rank } => mode == "L" && *size == 3 && *rank == 1,
            Self::ExtractBand { channel } => {
                usize::try_from(*channel).is_ok_and(|channel| channel < channels)
            }
            Self::Invert => matches!(mode, "L" | "RGB"),
            Self::Brightness { factor } => {
                #[cfg(feature = "gpu")]
                {
                    matches!(mode, "L" | "LA" | "RGB")
                        && crate::compute::registry::gpu_brightness_factor_int(*factor).is_some()
                }
                #[cfg(not(feature = "gpu"))]
                {
                    let _ = factor;
                    false
                }
            }
            Self::Multiply { other } => {
                other.mode().is_ok_and(|other_mode| other_mode == mode)
                    && other.size().is_ok_and(|other_size| other_size == size)
            }
            Self::Paste { source, mask } => {
                source.mode().is_ok_and(|source_mode| source_mode == mode)
                    && source.size().is_ok_and(|source_size| source_size == size)
                    && mask.mode().is_ok_and(|mask_mode| mask_mode == "L")
                    && mask.size().is_ok_and(|mask_size| mask_size == size)
            }
            Self::Composite { background, mask } => {
                matches!(mode, "L" | "LA" | "RGB" | "RGBA")
                    && background
                        .mode()
                        .is_ok_and(|background_mode| background_mode == mode)
                    && background
                        .size()
                        .is_ok_and(|background_size| background_size == size)
                    && mask.mode().is_ok_and(|mask_mode| mask_mode == "L")
                    && mask.size().is_ok_and(|mask_size| mask_size == size)
            }
            Self::Expand { border, fill } => {
                let Some(border_twice) = border.checked_mul(2) else {
                    return false;
                };
                if !matches!(mode, "L" | "LA" | "RGB" | "RGBA")
                    || size.0.checked_add(border_twice).is_none()
                    || size.1.checked_add(border_twice).is_none()
                {
                    return false;
                }
                crate::ops::imageops::resolve_imageops_color(fill.clone(), mode).is_ok()
            }
            Self::Color3DLut {
                channels,
                target_mode,
                ..
            } => {
                mode == "RGBA"
                    && *channels == 4
                    && matches!(target_mode.as_deref(), None | Some("RGBA"))
            }
        }
    }

    fn matches_group(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::MedianFilter { size: left }, Self::MedianFilter { size: right }) => {
                left == right
            }
            (Self::MaxFilter { size: left }, Self::MaxFilter { size: right }) => left == right,
            (
                Self::RankFilter {
                    size: left_size,
                    rank: left_rank,
                },
                Self::RankFilter {
                    size: right_size,
                    rank: right_rank,
                },
            ) => left_size == right_size && left_rank == right_rank,
            (Self::ExtractBand { channel: left }, Self::ExtractBand { channel: right }) => {
                left == right
            }
            (Self::Invert, Self::Invert) => true,
            (Self::Brightness { factor: left }, Self::Brightness { factor: right }) => {
                left == right
            }
            (Self::Multiply { .. }, Self::Multiply { .. }) => true,
            (Self::Paste { .. }, Self::Paste { .. }) => true,
            (Self::Composite { .. }, Self::Composite { .. }) => true,
            (
                Self::Expand {
                    border: left_border,
                    fill: left_fill,
                },
                Self::Expand {
                    border: right_border,
                    fill: right_fill,
                },
            ) => left_border == right_border && left_fill == right_fill,
            (
                Self::Color3DLut {
                    size: left_size,
                    table: left_table,
                    channels: left_channels,
                    target_mode: left_target,
                },
                Self::Color3DLut {
                    size: right_size,
                    table: right_table,
                    channels: right_channels,
                    target_mode: right_target,
                },
            ) => {
                left_size == right_size
                    && left_channels == right_channels
                    && left_target == right_target
                    && Arc::ptr_eq(left_table, right_table)
            }
            _ => false,
        }
    }

    fn pipeline_op(&self) -> Option<PipelineOp> {
        match self {
            Self::MedianFilter { size } => Some(PipelineOp::MedianFilter { size: *size }),
            Self::MaxFilter { size } => Some(PipelineOp::MaxFilter { size: *size }),
            Self::RankFilter { size, rank } => Some(PipelineOp::RankFilter {
                size: *size,
                rank: *rank,
            }),
            Self::ExtractBand { channel } => Some(PipelineOp::ExtractBand {
                index: u8::try_from(*channel).ok()?,
            }),
            Self::Invert => Some(PipelineOp::Invert),
            Self::Brightness { factor } => Some(PipelineOp::Brightness { factor: *factor }),
            Self::Multiply { other } => Some(PipelineOp::Multiply {
                other: Arc::new((**other).clone()),
            }),
            Self::Paste { source, mask } => {
                let (width, height) = source.size().ok()?;
                Some(PipelineOp::Paste {
                    source: Arc::new((**source).clone()),
                    x: 0,
                    y: 0,
                    w: i32::try_from(width).ok()?,
                    h: i32::try_from(height).ok()?,
                    mask: Some(Arc::new((**mask).clone())),
                    mask_alpha: false,
                })
            }
            Self::Composite { background, mask } => Some(PipelineOp::CompositeModule {
                other: Arc::new((**background).clone()),
                mask: Arc::new((**mask).clone()),
                mask_alpha: false,
            }),
            Self::Expand { .. } => None,
            Self::Color3DLut {
                size,
                table,
                channels,
                ..
            } => Some(PipelineOp::Color3DLut {
                size: *size,
                table: Arc::clone(table),
                channels: *channels,
                source_mode: PixelMode::RGBA,
                target_mode: PixelMode::RGBA,
            }),
        }
    }

    fn pipeline_op_for_mode(&self, mode: &str) -> Option<PipelineOp> {
        if let Self::Expand { border, fill } = self {
            let fill = crate::ops::imageops::resolve_imageops_color(fill.clone(), mode)
                .ok()?
                .unwrap_or((0, 0, 0, 0));
            return Some(PipelineOp::Expand {
                border: *border,
                fill,
            });
        }
        self.pipeline_op()
    }
}

struct BatchJob {
    source: Box<Image>,
    operation: BatchOperation,
    mode: String,
    size: (u32, u32),
}

/// Queues explicit image operations and joins compatible GPU jobs together.
///
/// `queue = false` applies each operation immediately using the ordinary
/// single-image route. With `queue = true`, [`join`](Self::join) groups
/// compatible `MedianFilter(3)`, `MaxFilter(3)`, and native-L
/// `RankFilter(3, rank=1)`, `ExtractBand`, L/RGB `ImageOps.invert`, exact-factor native-mode
/// `Brightness`, `ImageChops.multiply`, full-frame masked Paste, same-mode
/// `Image.composite` through an L mask, same-mode RGBA `Color3DLUT`, and
/// native-mode `ImageOps.expand` jobs when GPU is the selected backend.
/// Grouping currently
/// requires equal-size native byte modes `L`, `LA`, `RGB`, and `RGBA`; batched
/// Brightness additionally requires an exact GPU factor and one of `L`, `LA`,
/// or `RGB`. Masked Paste additionally requires same-mode sources and
/// same-size `L` masks. Composite groups require equal-size foreground and
/// background images in the same native mode plus a same-size `L` mask, while
/// LUT groups require the same shared LUT object.
/// Other jobs use the ordinary per-image operation. No conversion to RGBA is
/// performed.
///
/// `backend` optionally locks each job to a backend. If it is `None`, normal
/// automatic routing is used and grouping is attempted only when GPU is the
/// active preferred backend.
pub struct BatchExecutor {
    queue: bool,
    backend: Option<Backend>,
    pending: Vec<BatchJob>,
    completed: Vec<Image>,
}

impl BatchExecutor {
    /// Creates an explicit image batch executor.
    #[must_use]
    pub fn new(queue: bool, backend: Option<Backend>) -> Self {
        Self {
            queue,
            backend,
            pending: Vec::new(),
            completed: Vec::new(),
        }
    }

    /// Submits one image and built-in operation, returning its submission
    /// index. In nonqueued mode the image is materialized before this method
    /// returns. In queued mode it is held until [`Self::join`].
    ///
    /// # Errors
    ///
    /// Returns the same validation or execution error as the selected
    /// single-image operation when `queue` is false. Queued operation
    /// validation and execution errors are returned from [`Self::join`].
    pub fn submit(&mut self, image: Image, operation: BatchOperation) -> Result<usize, PilError> {
        let mode = image.mode()?;
        let size = image.size()?;
        let index = if self.queue {
            self.pending.len()
        } else {
            self.completed.len()
        };

        if self.queue {
            self.pending.push(BatchJob {
                source: Box::new(image),
                operation,
                mode,
                size,
            });
        } else {
            let prepared = operation.apply(&image)?;
            let completed = self.execute_single(prepared)?;
            self.completed.push(completed);
        }
        Ok(index)
    }

    /// Executes all queued jobs and returns results in submission order.
    ///
    /// Calling `join` drains the current batch, so the executor can accept a
    /// later batch after the call. A GPU batch is one existing image pipeline
    /// over a stacked native-mode buffer; each returned image remains an
    /// ordinary `Image` result with its source metadata and logical mode.
    ///
    /// # Errors
    ///
    /// Returns an operation, allocation, or backend error from the same
    /// single-image implementation used by normal image methods.
    pub fn join(&mut self) -> Result<Vec<Image>, PilError> {
        if !self.queue {
            return Ok(std::mem::take(&mut self.completed));
        }

        let mut jobs = std::mem::take(&mut self.pending)
            .into_iter()
            .map(Some)
            .collect::<Vec<Option<BatchJob>>>();
        let mut results = std::iter::repeat_with(|| None)
            .take(jobs.len())
            .collect::<Vec<Option<Image>>>();

        let batch_backend = self.batch_backend();
        let mut index = 0usize;
        while index < jobs.len() {
            let Some(job) = jobs[index].as_ref() else {
                index = index.saturating_add(1);
                continue;
            };

            let mut group = Vec::new();
            if self.queue
                && batch_backend.is_some()
                && job.size.0 != 0
                && job.size.1 != 0
                && job.operation.can_group(&job.mode, job.size)
            {
                group.extend((index..jobs.len()).filter(|candidate| {
                    jobs[*candidate].as_ref().is_some_and(|other| {
                        job.operation.matches_group(&other.operation)
                            && other.mode == job.mode
                            && other.size == job.size
                    })
                }));
            }

            if group.len() >= 2 {
                let operation = job
                    .operation
                    .pipeline_op_for_mode(&job.mode)
                    .expect("groupable batch operation has a pipeline operation");
                let safe_group_len = crate::compute::gpu_batch_group_limit(
                    &operation,
                    job.mode.as_str(),
                    job.size,
                    group.len(),
                );
                group.truncate(safe_group_len);
            }

            if group.len() >= 2 {
                let selected_backend = batch_backend.expect("grouping requires a backend");
                match self.execute_group(&jobs, &group, selected_backend, &mut results) {
                    Ok(()) => {
                        for grouped_index in &group {
                            jobs[*grouped_index] = None;
                        }
                        index = index.saturating_add(1);
                        continue;
                    }
                    Err(PilError::DimensionError(_) | PilError::MemoryError(_)) => {
                        // A stacked allocation can exceed limits even when
                        // each source image is valid. Run those jobs through
                        // their normal single-image paths instead.
                    }
                    Err(error) => return Err(error),
                }
            }

            let job = jobs[index]
                .take()
                .expect("the current batch job was checked above");
            results[index] = Some(self.execute_single(job.operation.apply(&job.source)?)?);
            index = index.saturating_add(1);
        }

        results
            .into_iter()
            .map(|result| {
                result.ok_or_else(|| {
                    PilError::InternalError("batch join omitted a submitted image".into())
                })
            })
            .collect()
    }

    fn batch_backend(&self) -> Option<Backend> {
        if let Some(backend) = self.backend {
            return (backend == Backend::Gpu).then_some(backend);
        }
        crate::active_backends()
            .ok()
            .and_then(|backends| backends.first().copied())
            .filter(|backend| *backend == Backend::Gpu)
    }

    fn execute_single(&self, image: Image) -> Result<Image, PilError> {
        let image = match self.backend {
            Some(backend) => image.use_backend(backend),
            None => image,
        };
        image.materialize()?;
        Ok(image)
    }

    fn execute_group(
        &self,
        jobs: &[Option<BatchJob>],
        indices: &[usize],
        backend: Backend,
        results: &mut [Option<Image>],
    ) -> Result<(), PilError> {
        let first = jobs[indices[0]]
            .as_ref()
            .ok_or_else(|| PilError::InternalError("batch job was already consumed".into()))?;

        #[cfg(feature = "migration-fault-injection")]
        if let Some(error) = injected_rank_filter_group_failure(first, backend) {
            return Err(error);
        }
        #[cfg(feature = "migration-fault-injection")]
        if let Some(error) = injected_expand_group_failure(first, backend) {
            return Err(error);
        }
        #[cfg(feature = "migration-fault-injection")]
        if let Some(error) = injected_paste_group_failure(first, backend) {
            return Err(error);
        }
        #[cfg(feature = "migration-fault-injection")]
        if let Some(error) = injected_composite_group_failure(first, backend) {
            return Err(error);
        }

        let mode = first.mode.as_str();
        let channels = mode_channels(mode).ok_or_else(|| {
            PilError::ValueError(format!(
                "mode {mode} cannot be grouped by this batch operation"
            ))
        })?;
        let native_storage = mode_storage(mode).ok_or_else(|| {
            PilError::ValueError(format!(
                "mode {mode} cannot be grouped by this batch operation"
            ))
        })?;
        let (width, height) = first.size;
        let row_bytes = usize::try_from(width)
            .ok()
            .and_then(|width| width.checked_mul(channels))
            .ok_or_else(|| PilError::DimensionError("batch row size overflow".into()))?;
        let (halo, stacked_height, output_mode, output_channels) = match &first.operation {
            BatchOperation::MedianFilter { size: 3 }
            | BatchOperation::MaxFilter { size: 3 }
            | BatchOperation::RankFilter { size: 3, rank: 1 } => {
                let group_len = u32::try_from(indices.len())
                    .map_err(|_| PilError::DimensionError("batch count overflow".into()))?;
                let guarded_height = height
                    .checked_add(2)
                    .and_then(|height| height.checked_mul(group_len))
                    .ok_or_else(|| PilError::DimensionError("batch height overflow".into()))?;
                (1usize, guarded_height, mode, channels)
            }
            BatchOperation::Color3DLut { .. } => {
                let group_len = u32::try_from(indices.len())
                    .map_err(|_| PilError::DimensionError("batch count overflow".into()))?;
                let stacked_height = height
                    .checked_mul(group_len)
                    .ok_or_else(|| PilError::DimensionError("batch height overflow".into()))?;
                (0usize, stacked_height, mode, channels)
            }
            BatchOperation::ExtractBand { .. } => {
                let group_len = u32::try_from(indices.len())
                    .map_err(|_| PilError::DimensionError("batch count overflow".into()))?;
                let stacked_height = height
                    .checked_mul(group_len)
                    .ok_or_else(|| PilError::DimensionError("batch height overflow".into()))?;
                (0usize, stacked_height, "L", 1usize)
            }
            BatchOperation::Invert => {
                let group_len = u32::try_from(indices.len())
                    .map_err(|_| PilError::DimensionError("batch count overflow".into()))?;
                let stacked_height = height
                    .checked_mul(group_len)
                    .ok_or_else(|| PilError::DimensionError("batch height overflow".into()))?;
                (0usize, stacked_height, mode, channels)
            }
            BatchOperation::Brightness { .. } => {
                let group_len = u32::try_from(indices.len())
                    .map_err(|_| PilError::DimensionError("batch count overflow".into()))?;
                let stacked_height = height
                    .checked_mul(group_len)
                    .ok_or_else(|| PilError::DimensionError("batch height overflow".into()))?;
                (0usize, stacked_height, mode, channels)
            }
            BatchOperation::Multiply { .. } => {
                let group_len = u32::try_from(indices.len())
                    .map_err(|_| PilError::DimensionError("batch count overflow".into()))?;
                let stacked_height = height
                    .checked_mul(group_len)
                    .ok_or_else(|| PilError::DimensionError("batch height overflow".into()))?;
                (0usize, stacked_height, mode, channels)
            }
            BatchOperation::Paste { .. } | BatchOperation::Composite { .. } => {
                let group_len = u32::try_from(indices.len())
                    .map_err(|_| PilError::DimensionError("batch count overflow".into()))?;
                let stacked_height = height
                    .checked_mul(group_len)
                    .ok_or_else(|| PilError::DimensionError("batch height overflow".into()))?;
                (0usize, stacked_height, mode, channels)
            }
            BatchOperation::Expand { border, .. } => {
                let group_len = u32::try_from(indices.len())
                    .map_err(|_| PilError::DimensionError("batch count overflow".into()))?;
                let border_rows = border
                    .checked_mul(2)
                    .ok_or_else(|| PilError::DimensionError("Expand border overflow".into()))?;
                let output_height = height
                    .checked_add(border_rows)
                    .and_then(|height| height.checked_mul(group_len))
                    .ok_or_else(|| {
                        PilError::DimensionError("Expand batch height overflow".into())
                    })?;
                let stacked_height = output_height.checked_sub(border_rows).ok_or_else(|| {
                    PilError::DimensionError("Expand batch height underflow".into())
                })?;
                (0usize, stacked_height, mode, channels)
            }
            _ => {
                return Err(PilError::ValueError(
                    "this operation has no compatible batch layout".into(),
                ));
            }
        };
        let height_usize = usize::try_from(height)
            .map_err(|_| PilError::DimensionError("batch image height overflow".into()))?;
        let stacked_height_usize = usize::try_from(stacked_height)
            .map_err(|_| PilError::DimensionError("batch height overflow".into()))?;
        let capacity = row_bytes
            .checked_mul(stacked_height_usize)
            .ok_or_else(|| PilError::DimensionError("batch buffer size overflow".into()))?;
        let expected = row_bytes
            .checked_mul(height_usize)
            .ok_or_else(|| PilError::DimensionError("source image size overflow".into()))?;
        let mut packed = Vec::new();
        packed.try_reserve_exact(capacity).map_err(|error| {
            PilError::MemoryError(format!(
                "unable to allocate {capacity} batch bytes: {error}"
            ))
        })?;
        let mut packed_other = if matches!(
            first.operation,
            BatchOperation::Multiply { .. }
                | BatchOperation::Paste { .. }
                | BatchOperation::Composite { .. }
        ) {
            let mut packed_other = Vec::new();
            packed_other.try_reserve_exact(capacity).map_err(|error| {
                PilError::MemoryError(format!(
                    "unable to allocate {capacity} secondary batch bytes: {error}"
                ))
            })?;
            Some(packed_other)
        } else {
            None
        };
        let mask_capacity = usize::try_from(width)
            .ok()
            .and_then(|width| width.checked_mul(stacked_height_usize))
            .ok_or_else(|| PilError::DimensionError("batch mask size overflow".into()))?;
        let mut packed_mask = if matches!(
            first.operation,
            BatchOperation::Paste { .. } | BatchOperation::Composite { .. }
        ) {
            let mut packed_mask = Vec::new();
            packed_mask
                .try_reserve_exact(mask_capacity)
                .map_err(|error| {
                    PilError::MemoryError(format!(
                        "unable to allocate {mask_capacity} batch mask bytes: {error}"
                    ))
                })?;
            Some(packed_mask)
        } else {
            None
        };

        let expand_fill_row = if let BatchOperation::Expand { fill, .. } = &first.operation {
            let fill = crate::ops::imageops::resolve_imageops_color(fill.clone(), mode)?
                .unwrap_or((0, 0, 0, 0));
            let pixel = native_expand_fill_pixel(mode, fill).ok_or_else(|| {
                PilError::InternalError("Expand batch has no native fill layout".into())
            })?;
            let row_width = usize::try_from(width)
                .map_err(|_| PilError::DimensionError("Expand row width overflow".into()))?;
            let row_bytes = row_width
                .checked_mul(channels)
                .ok_or_else(|| PilError::DimensionError("Expand fill row size overflow".into()))?;
            let mut row = Vec::new();
            row.try_reserve_exact(row_bytes).map_err(|error| {
                PilError::MemoryError(format!(
                    "unable to allocate {row_bytes} Expand fill-row bytes: {error}"
                ))
            })?;
            for _ in 0..row_width {
                row.extend_from_slice(&pixel[..channels]);
            }
            Some(row)
        } else {
            None
        };
        let expand_separator_rows = match &first.operation {
            BatchOperation::Expand { border, .. } => usize::try_from(
                border
                    .checked_mul(2)
                    .ok_or_else(|| PilError::DimensionError("Expand border overflow".into()))?,
            )
            .map_err(|_| PilError::DimensionError("Expand separator rows overflow".into()))?,
            _ => 0,
        };

        for (group_index, index) in indices.iter().enumerate() {
            let job = jobs[*index]
                .as_ref()
                .ok_or_else(|| PilError::InternalError("batch job was already consumed".into()))?;
            let pixels = job.source.materialized_shared()?;
            if pixels.color() != native_storage {
                return Err(PilError::DimensionError(format!(
                    "mode {mode} does not use its native batch storage"
                )));
            }
            let pixel_bytes = pixels.as_bytes();
            if pixel_bytes.len() != expected {
                return Err(PilError::InternalError(format!(
                    "native batch input length mismatch: expected {expected}, got {}",
                    pixel_bytes.len()
                )));
            }
            for _ in 0..halo {
                packed.extend_from_slice(&pixel_bytes[..row_bytes]);
            }
            packed.extend_from_slice(pixel_bytes);
            let last_row_start = expected
                .checked_sub(row_bytes)
                .ok_or_else(|| PilError::DimensionError("batch row offset underflow".into()))?;
            for _ in 0..halo {
                packed.extend_from_slice(&pixel_bytes[last_row_start..expected]);
            }

            if group_index < indices.len().saturating_sub(1)
                && let Some(fill_row) = expand_fill_row.as_deref()
            {
                for _ in 0..expand_separator_rows {
                    packed.extend_from_slice(fill_row);
                }
            }

            match &job.operation {
                BatchOperation::Multiply { other } => {
                    let other_pixels = other.materialized_shared()?;
                    if other.mode()? != mode
                        || other.size()? != (width, height)
                        || other_pixels.color() != native_storage
                    {
                        return Err(PilError::DimensionError(
                            "multiply batch operands must have matching native dimensions".into(),
                        ));
                    }
                    let other_bytes = other_pixels.as_bytes();
                    if other_bytes.len() != expected {
                        return Err(PilError::InternalError(format!(
                            "native multiply batch input length mismatch: expected {expected}, got {}",
                            other_bytes.len()
                        )));
                    }
                    packed_other
                        .as_mut()
                        .ok_or_else(|| {
                            PilError::InternalError("multiply batch has no secondary buffer".into())
                        })?
                        .extend_from_slice(other_bytes);
                }
                BatchOperation::Paste { source, mask } => {
                    let source_pixels = source.materialized_shared()?;
                    if source.mode()? != mode
                        || source.size()? != (width, height)
                        || source_pixels.color() != native_storage
                    {
                        return Err(PilError::DimensionError(
                            "paste batch sources must match the destination's native dimensions"
                                .into(),
                        ));
                    }
                    let source_bytes = source_pixels.as_bytes();
                    if source_bytes.len() != expected {
                        return Err(PilError::InternalError(format!(
                            "native Paste batch source length mismatch: expected {expected}, got {}",
                            source_bytes.len()
                        )));
                    }
                    packed_other
                        .as_mut()
                        .ok_or_else(|| {
                            PilError::InternalError("Paste batch has no source buffer".into())
                        })?
                        .extend_from_slice(source_bytes);

                    let mask_pixels = mask.materialized_shared()?;
                    if mask.mode()? != "L"
                        || mask.size()? != (width, height)
                        || mask_pixels.color() != crate::raster::ColorType::L8
                    {
                        return Err(PilError::DimensionError(
                            "Paste batch masks must be same-size native L images".into(),
                        ));
                    }
                    let mask_bytes = mask_pixels.as_bytes();
                    let expected_mask = usize::try_from(width)
                        .ok()
                        .and_then(|width| {
                            usize::try_from(height)
                                .ok()
                                .and_then(|height| width.checked_mul(height))
                        })
                        .ok_or_else(|| {
                            PilError::DimensionError("Paste mask size overflow".into())
                        })?;
                    if mask_bytes.len() != expected_mask {
                        return Err(PilError::InternalError(format!(
                            "native Paste batch mask length mismatch: expected {expected_mask}, got {}",
                            mask_bytes.len()
                        )));
                    }
                    packed_mask
                        .as_mut()
                        .ok_or_else(|| {
                            PilError::InternalError("Paste batch has no mask buffer".into())
                        })?
                        .extend_from_slice(mask_bytes);
                }
                BatchOperation::Composite { background, mask } => {
                    let background_pixels = background.materialized_shared()?;
                    if background.mode()? != mode
                        || background.size()? != (width, height)
                        || background_pixels.color() != native_storage
                    {
                        return Err(PilError::DimensionError(
                            "composite backgrounds must match the foreground's native dimensions"
                                .into(),
                        ));
                    }
                    let background_bytes = background_pixels.as_bytes();
                    if background_bytes.len() != expected {
                        return Err(PilError::InternalError(format!(
                            "native Composite batch background length mismatch: expected {expected}, got {}",
                            background_bytes.len()
                        )));
                    }
                    packed_other
                        .as_mut()
                        .ok_or_else(|| {
                            PilError::InternalError(
                                "Composite batch has no background buffer".into(),
                            )
                        })?
                        .extend_from_slice(background_bytes);

                    let mask_pixels = mask.materialized_shared()?;
                    if mask.mode()? != "L"
                        || mask.size()? != (width, height)
                        || mask_pixels.color() != crate::raster::ColorType::L8
                    {
                        return Err(PilError::DimensionError(
                            "Composite batch masks must be same-size native L images".into(),
                        ));
                    }
                    let mask_bytes = mask_pixels.as_bytes();
                    let expected_mask = usize::try_from(width)
                        .ok()
                        .and_then(|width| {
                            usize::try_from(height)
                                .ok()
                                .and_then(|height| width.checked_mul(height))
                        })
                        .ok_or_else(|| {
                            PilError::DimensionError("Composite mask size overflow".into())
                        })?;
                    if mask_bytes.len() != expected_mask {
                        return Err(PilError::InternalError(format!(
                            "native Composite batch mask length mismatch: expected {expected_mask}, got {}",
                            mask_bytes.len()
                        )));
                    }
                    packed_mask
                        .as_mut()
                        .ok_or_else(|| {
                            PilError::InternalError("Composite batch has no mask buffer".into())
                        })?
                        .extend_from_slice(mask_bytes);
                }
                _ => {}
            }
        }
        if packed.len() != capacity {
            return Err(PilError::InternalError(
                "native batch packing produced an unexpected byte count".into(),
            ));
        }
        if packed_other
            .as_ref()
            .is_some_and(|packed_other| packed_other.len() != capacity)
        {
            return Err(PilError::InternalError(
                "native multiply batch packing produced an unexpected byte count".into(),
            ));
        }
        if packed_mask
            .as_ref()
            .is_some_and(|packed_mask| packed_mask.len() != mask_capacity)
        {
            return Err(PilError::InternalError(
                "native Paste batch packing produced an unexpected mask byte count".into(),
            ));
        }

        let (output_width, output_height) = match &first.operation {
            BatchOperation::Expand { border, .. } => {
                let border_twice = border.checked_mul(2).ok_or_else(|| {
                    PilError::DimensionError("Expand output border overflow".into())
                })?;
                (
                    width.checked_add(border_twice).ok_or_else(|| {
                        PilError::DimensionError("Expand output width overflow".into())
                    })?,
                    height.checked_add(border_twice).ok_or_else(|| {
                        PilError::DimensionError("Expand output height overflow".into())
                    })?,
                )
            }
            _ => (width, height),
        };

        // Keep each source mode's physical pixel layout. Existing Image
        // constructors and operation dispatch own validation and routing.
        let stacked = Image::frombytes_owned(mode, (width, stacked_height), packed)?;
        let operated = if matches!(first.operation, BatchOperation::Multiply { .. }) {
            let secondary = Image::frombytes_owned(
                mode,
                (width, stacked_height),
                packed_other.ok_or_else(|| {
                    PilError::InternalError("multiply batch lost its secondary buffer".into())
                })?,
            )?;
            crate::ops::chops::multiply(&stacked, &secondary)?
        } else if matches!(first.operation, BatchOperation::Paste { .. }) {
            let source = Image::frombytes_owned(
                mode,
                (width, stacked_height),
                packed_other.ok_or_else(|| {
                    PilError::InternalError("Paste batch lost its source buffer".into())
                })?,
            )?;
            let mask = Image::frombytes_owned(
                "L",
                (width, stacked_height),
                packed_mask.ok_or_else(|| {
                    PilError::InternalError("Paste batch lost its mask buffer".into())
                })?,
            )?;
            BatchOperation::Paste {
                source: Box::new(source),
                mask: Box::new(mask),
            }
            .apply(&stacked)?
        } else if matches!(first.operation, BatchOperation::Composite { .. }) {
            let background = Image::frombytes_owned(
                mode,
                (width, stacked_height),
                packed_other.ok_or_else(|| {
                    PilError::InternalError("Composite batch lost its background buffer".into())
                })?,
            )?;
            let mask = Image::frombytes_owned(
                "L",
                (width, stacked_height),
                packed_mask.ok_or_else(|| {
                    PilError::InternalError("Composite batch lost its mask buffer".into())
                })?,
            )?;
            BatchOperation::Composite {
                background: Box::new(background),
                mask: Box::new(mask),
            }
            .apply(&stacked)?
        } else if matches!(first.operation, BatchOperation::Color3DLut { .. }) {
            Image::push_mode_changing_op(
                &stacked,
                first
                    .operation
                    .pipeline_op()
                    .expect("groupable Color3DLut has a pipeline operation"),
                output_mode,
            )
        } else {
            first.operation.apply(&stacked)?
        };
        let operated = operated.use_backend(backend);
        // These grouped byte modes already store output in their Pillow byte
        // order. Borrow the materialized bytes directly instead of cloning
        // the complete stack through `tobytes()` before splitting it.
        let operated_pixels = operated.materialized_shared()?;
        let operated_bytes = operated_pixels.as_bytes();

        let output_row_bytes = usize::try_from(output_width)
            .ok()
            .and_then(|width| width.checked_mul(output_channels))
            .ok_or_else(|| PilError::DimensionError("batch output row size overflow".into()))?;
        let (image_rows, row_offset) = if matches!(first.operation, BatchOperation::Expand { .. }) {
            (
                usize::try_from(output_height).map_err(|_| {
                    PilError::DimensionError("Expand output height overflow".into())
                })?,
                0usize,
            )
        } else {
            (
                height_usize
                    .checked_add(halo.checked_mul(2).ok_or_else(|| {
                        PilError::DimensionError("batch halo row count overflow".into())
                    })?)
                    .ok_or_else(|| PilError::DimensionError("batch row count overflow".into()))?,
                halo,
            )
        };
        let row_offset_bytes = output_row_bytes
            .checked_mul(row_offset)
            .ok_or_else(|| PilError::DimensionError("batch output row offset overflow".into()))?;
        let image_stride = output_row_bytes
            .checked_mul(image_rows)
            .ok_or_else(|| PilError::DimensionError("batch output stride overflow".into()))?;
        let image_bytes = output_row_bytes
            .checked_mul(image_rows.checked_sub(row_offset).ok_or_else(|| {
                PilError::DimensionError("batch output row count underflow".into())
            })?)
            .ok_or_else(|| PilError::DimensionError("batch output size overflow".into()))?;
        let mut grouped_results = Vec::new();
        grouped_results
            .try_reserve_exact(indices.len())
            .map_err(|error| {
                PilError::MemoryError(format!(
                    "unable to allocate {} batch results: {error}",
                    indices.len()
                ))
            })?;
        for (group_index, job_index) in indices.iter().enumerate() {
            let start = group_index
                .checked_mul(image_stride)
                .and_then(|start| start.checked_add(row_offset_bytes))
                .ok_or_else(|| PilError::DimensionError("batch output offset overflow".into()))?;
            let end = start
                .checked_add(image_bytes)
                .ok_or_else(|| PilError::DimensionError("batch output end overflow".into()))?;
            let output = operated_bytes.get(start..end).ok_or_else(|| {
                PilError::InternalError("native batch output was shorter than expected".into())
            })?;
            let job = jobs[*job_index]
                .as_ref()
                .ok_or_else(|| PilError::InternalError("batch job was already consumed".into()))?;
            let mut output_copy = Vec::new();
            output_copy
                .try_reserve_exact(output.len())
                .map_err(|error| {
                    PilError::MemoryError(format!(
                        "unable to allocate {} batch output bytes: {error}",
                        output.len()
                    ))
                })?;
            output_copy.extend_from_slice(output);
            let pixels =
                Image::frombytes_owned(output_mode, (output_width, output_height), output_copy)?
                    .materialized_shared()?;
            let mut image = job.operation.apply(&job.source)?;
            image.cache_batched_materialization(pixels)?;
            grouped_results.push((*job_index, image));
        }
        for (job_index, image) in grouped_results {
            results[job_index] = Some(image);
        }
        Ok(())
    }
}

fn mode_channels(mode: &str) -> Option<usize> {
    match mode {
        "L" => Some(1),
        "LA" => Some(2),
        "RGB" => Some(3),
        "RGBA" => Some(4),
        _ => None,
    }
}

fn native_expand_fill_pixel(mode: &str, fill: (u8, u8, u8, u8)) -> Option<[u8; 4]> {
    match mode {
        "L" => Some([fill.0, 0, 0, 0]),
        // ImageOps' LA color resolver stores the luminance in the first byte
        // and alpha in the fourth byte of the shared fill tuple.
        "LA" => Some([fill.0, fill.3, 0, 0]),
        "RGB" => Some([fill.0, fill.1, fill.2, 0]),
        "RGBA" => Some([fill.0, fill.1, fill.2, fill.3]),
        _ => None,
    }
}

fn mode_storage(mode: &str) -> Option<crate::raster::ColorType> {
    match mode {
        "L" => Some(crate::raster::ColorType::L8),
        "LA" => Some(crate::raster::ColorType::La8),
        "RGB" => Some(crate::raster::ColorType::Rgb8),
        "RGBA" => Some(crate::raster::ColorType::Rgba8),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{BatchExecutor, BatchOperation, native_expand_fill_pixel};
    use crate::compute::Backend;
    use crate::error::PilError;
    use crate::image::Image;
    use crate::pipeline::{PipelineOp, PixelMode};
    use std::sync::Arc;

    fn fixture(mode: &str, width: u32, height: u32, channels: usize, seed: u8) -> Image {
        let pixel_count = usize::try_from(width)
            .unwrap()
            .checked_mul(usize::try_from(height).unwrap())
            .and_then(|count| count.checked_mul(channels))
            .unwrap();
        let mut value = seed;
        let mut pixels = Vec::with_capacity(pixel_count);
        for _ in 0..pixel_count {
            value = value.wrapping_mul(31).wrapping_add(seed);
            pixels.push(value);
        }
        Image::frombytes(mode, (width, height), &pixels).unwrap()
    }

    #[test]
    fn color3dlut_groups_only_shared_same_mode_rgba_tables() {
        let table = Arc::<[f64]>::from(vec![0.0; 2 * 2 * 2 * 4]);
        let operation = BatchOperation::Color3DLut {
            size: (2, 2, 2),
            table: Arc::clone(&table),
            channels: 4,
            target_mode: Some("RGBA".into()),
        };
        let same = BatchOperation::Color3DLut {
            size: (2, 2, 2),
            table: Arc::clone(&table),
            channels: 4,
            target_mode: Some("RGBA".into()),
        };
        let equal_but_distinct = BatchOperation::Color3DLut {
            size: (2, 2, 2),
            table: Arc::from(table.to_vec()),
            channels: 4,
            target_mode: Some("RGBA".into()),
        };

        assert!(operation.can_group("RGBA", (8, 5)));
        assert!(!operation.can_group("RGB", (8, 5)));
        assert!(operation.matches_group(&same));
        assert!(!operation.matches_group(&equal_but_distinct));
        assert!(matches!(
            operation.pipeline_op(),
            Some(PipelineOp::Color3DLut {
                source_mode: PixelMode::RGBA,
                target_mode: PixelMode::RGBA,
                channels: 4,
                ..
            })
        ));
    }

    #[test]
    fn brightness_groups_only_equal_exact_native_byte_operations() {
        let operation = BatchOperation::Brightness { factor: 0.5 };
        let same = BatchOperation::Brightness { factor: 0.5 };
        let different = BatchOperation::Brightness { factor: 0.25 };
        #[cfg(feature = "gpu")]
        let inexact = BatchOperation::Brightness { factor: 1.0 / 3.0 };

        assert!(operation.matches_group(&same));
        assert!(!operation.matches_group(&different));
        assert!(matches!(
            operation.pipeline_op(),
            Some(PipelineOp::Brightness { factor }) if factor == 0.5
        ));
        #[cfg(feature = "gpu")]
        {
            assert!(operation.can_group("L", (16, 16)));
            assert!(operation.can_group("LA", (16, 16)));
            assert!(operation.can_group("RGB", (16, 16)));
            assert!(!operation.can_group("RGBA", (16, 16)));
            assert!(!operation.can_group("P", (16, 16)));
            assert!(!inexact.can_group("LA", (16, 16)));
        }
        #[cfg(not(feature = "gpu"))]
        assert!(!operation.can_group("LA", (16, 16)));
    }

    #[test]
    fn invert_groups_only_supported_imageops_modes() {
        let operation = BatchOperation::Invert;
        assert!(operation.can_group("L", (16, 16)));
        assert!(operation.can_group("RGB", (16, 16)));
        assert!(!operation.can_group("LA", (16, 16)));
        assert!(!operation.can_group("RGBA", (16, 16)));
        assert!(!operation.can_group("P", (16, 16)));
        assert!(!operation.can_group("CMYK", (16, 16)));
        assert!(operation.matches_group(&BatchOperation::Invert));
        assert!(matches!(operation.pipeline_op(), Some(PipelineOp::Invert)));
    }

    #[test]
    fn queued_invert_matches_existing_imageops_path() {
        for (mode, channels) in [("L", 1usize), ("RGB", 3)] {
            let sources = [
                fixture(mode, 7, 5, channels, 23),
                fixture(mode, 7, 5, channels, 89),
            ];
            let expected = sources
                .iter()
                .map(|source| {
                    crate::ops::imageops::invert_ops(source)
                        .unwrap()
                        .tobytes()
                        .unwrap()
                })
                .collect::<Vec<_>>();
            let mut batch = BatchExecutor::new(true, Some(Backend::Cpu));
            for source in sources {
                batch.submit(source, BatchOperation::Invert).unwrap();
            }
            let actual = batch
                .join()
                .unwrap()
                .iter()
                .map(|image| image.tobytes().unwrap())
                .collect::<Vec<_>>();
            assert_eq!(actual, expected, "queued {mode} ImageOps.invert differs");
        }
    }

    #[test]
    fn queued_brightness_uses_the_existing_native_single_image_operation() {
        let sources = [
            fixture("L", 7, 5, 1, 23),
            fixture("LA", 7, 5, 2, 89),
            fixture("RGB", 7, 5, 3, 173),
        ];
        let expected = sources
            .iter()
            .map(|source| source.enhance_brightness(0.5).unwrap().tobytes().unwrap())
            .collect::<Vec<_>>();
        let mut batch = BatchExecutor::new(true, Some(Backend::Cpu));
        for source in sources {
            batch
                .submit(source, BatchOperation::Brightness { factor: 0.5 })
                .unwrap();
        }
        let actual = batch
            .join()
            .unwrap()
            .iter()
            .map(|image| image.tobytes().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);
    }

    #[test]
    fn queued_batch_keeps_submission_order_and_matches_single_images() {
        let modes = [("L", 1), ("LA", 2), ("RGB", 3), ("RGBA", 4)];
        for (mode, channels) in modes {
            let sources = [
                fixture(mode, 7, 5, channels, 17),
                fixture(mode, 7, 5, channels, 203),
            ];
            let expected = sources
                .iter()
                .map(|source| source.median_filter(3).unwrap().tobytes().unwrap())
                .collect::<Vec<_>>();
            let mut batch = BatchExecutor::new(true, Some(Backend::Cpu));
            for source in sources {
                batch
                    .submit(source, BatchOperation::MedianFilter { size: 3 })
                    .unwrap();
            }
            let actual = batch
                .join()
                .unwrap()
                .iter()
                .map(|image| image.tobytes().unwrap())
                .collect::<Vec<_>>();
            assert_eq!(actual, expected, "queued {mode} outputs differ");
        }
    }

    #[test]
    fn queued_max_filter_matches_single_images_without_cross_image_edges() {
        let modes = [("L", 1), ("LA", 2), ("RGB", 3), ("RGBA", 4)];
        for (mode, channels) in modes {
            let sources = [
                fixture(mode, 7, 5, channels, 17),
                fixture(mode, 7, 5, channels, 203),
                fixture(mode, 7, 5, channels, 73),
            ];
            let expected = sources
                .iter()
                .map(|source| source.max_filter(3).unwrap().tobytes().unwrap())
                .collect::<Vec<_>>();
            let mut batch = BatchExecutor::new(true, Some(Backend::Cpu));
            for source in sources {
                batch
                    .submit(source, BatchOperation::MaxFilter { size: 3 })
                    .unwrap();
            }
            let actual = batch
                .join()
                .unwrap()
                .iter()
                .map(|image| image.tobytes().unwrap())
                .collect::<Vec<_>>();
            assert_eq!(actual, expected, "queued {mode} MaxFilter outputs differ");
        }
    }

    #[test]
    fn rank_filter_batches_only_native_l_second_minimum() {
        let operation = BatchOperation::RankFilter { size: 3, rank: 1 };
        assert!(operation.can_group("L", (17, 9)));
        assert!(!operation.can_group("LA", (17, 9)));
        assert!(!operation.can_group("RGB", (17, 9)));
        assert!(!BatchOperation::RankFilter { size: 3, rank: 0 }.can_group("L", (17, 9)));
        assert!(!BatchOperation::RankFilter { size: 3, rank: 8 }.can_group("L", (17, 9)));
        assert!(!BatchOperation::RankFilter { size: 5, rank: 1 }.can_group("L", (17, 9)));
        assert!(operation.matches_group(&BatchOperation::RankFilter { size: 3, rank: 1 }));
        assert!(!operation.matches_group(&BatchOperation::RankFilter { size: 3, rank: 0 }));
        assert!(matches!(
            operation.pipeline_op(),
            Some(PipelineOp::RankFilter { size: 3, rank: 1 })
        ));
    }

    #[test]
    fn expand_batches_only_native_byte_modes_with_matching_fill() {
        for (mode, color) in [
            ("L", vec![11]),
            ("LA", vec![11, 49]),
            ("RGB", vec![11, 23, 37]),
            ("RGBA", vec![11, 23, 37, 49]),
        ] {
            let expand = BatchOperation::Expand {
                border: 2,
                fill: crate::ImageOpsColor::Components(color.clone()),
            };
            assert!(expand.can_group(mode, (7, 5)), "{mode} should group");
            assert!(expand.matches_group(&BatchOperation::Expand {
                border: 2,
                fill: crate::ImageOpsColor::Components(color),
            }));
        }
        let expand = BatchOperation::Expand {
            border: 2,
            fill: crate::ImageOpsColor::Components(vec![11, 23, 37, 49]),
        };
        for mode in ["1", "P", "CMYK", "RGBX"] {
            assert!(!expand.can_group(mode, (7, 5)), "{mode} should fall back");
        }
        assert!(!expand.matches_group(&BatchOperation::Expand {
            border: 1,
            fill: crate::ImageOpsColor::Components(vec![11, 23, 37, 49]),
        }));
        assert!(!expand.matches_group(&BatchOperation::Expand {
            border: 2,
            fill: crate::ImageOpsColor::Scalar(11),
        }));
        assert_eq!(
            native_expand_fill_pixel("LA", (11, 11, 11, 49)),
            Some([11, 49, 0, 0])
        );
    }

    #[test]
    fn queued_expand_keeps_native_modes_and_separates_each_image() {
        let modes = [("L", 1usize), ("LA", 2), ("RGB", 3), ("RGBA", 4)];
        for (mode, channels) in modes {
            let sources = [
                fixture(mode, 7, 5, channels, 17),
                fixture(mode, 7, 5, channels, 203),
            ];
            let fill = crate::ImageOpsColor::Components(match mode {
                "L" => vec![11],
                "LA" => vec![11, 49],
                "RGB" => vec![11, 23, 37],
                "RGBA" => vec![11, 23, 37, 49],
                _ => unreachable!(),
            });
            let expected = sources
                .iter()
                .map(|source| {
                    crate::ops::imageops::expand_with_input(source, 2, fill.clone())
                        .unwrap()
                        .tobytes()
                        .unwrap()
                })
                .collect::<Vec<_>>();
            let mut batch = BatchExecutor::new(true, Some(Backend::Cpu));
            for source in sources {
                batch
                    .submit(
                        source,
                        BatchOperation::Expand {
                            border: 2,
                            fill: fill.clone(),
                        },
                    )
                    .unwrap();
            }
            let actual = batch
                .join()
                .unwrap()
                .iter()
                .map(|image| image.tobytes().unwrap())
                .collect::<Vec<_>>();
            assert_eq!(actual, expected, "queued {mode} Expand differs");
        }
    }

    #[test]
    fn queued_rank_filter_uses_exact_existing_native_mode_operation() {
        let sources = [fixture("L", 17, 9, 1, 23), fixture("L", 17, 9, 1, 89)];
        let expected = sources
            .iter()
            .map(|source| source.rank_filter(3, 1).unwrap().tobytes().unwrap())
            .collect::<Vec<_>>();
        let mut batch = BatchExecutor::new(true, Some(Backend::Cpu));
        for source in sources {
            batch
                .submit(source, BatchOperation::RankFilter { size: 3, rank: 1 })
                .unwrap();
        }
        let actual = batch
            .join()
            .unwrap()
            .iter()
            .map(|image| image.tobytes().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);
    }

    #[test]
    fn queued_batch_keeps_interleaved_modes_in_submission_order() {
        let sources = [
            fixture("L", 5, 3, 1, 17),
            fixture("RGB", 5, 3, 3, 91),
            fixture("L", 5, 3, 1, 203),
        ];
        let expected = sources
            .iter()
            .map(|source| source.median_filter(3).unwrap().tobytes().unwrap())
            .collect::<Vec<_>>();
        let mut batch = BatchExecutor::new(true, Some(Backend::Cpu));
        for source in sources {
            batch
                .submit(source, BatchOperation::MedianFilter { size: 3 })
                .unwrap();
        }
        let actual = batch
            .join()
            .unwrap()
            .iter()
            .map(|image| image.tobytes().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);
    }

    #[test]
    fn queued_extract_band_falls_back_to_exact_single_image_outputs() {
        let modes = [("L", 1usize), ("LA", 2), ("RGB", 3), ("RGBA", 4)];
        for (mode, channels) in modes {
            for channel in 0..channels {
                let sources = [
                    fixture(mode, 7, 5, channels, 17),
                    fixture(mode, 7, 5, channels, 203),
                ];
                let expected = sources
                    .iter()
                    .map(|source| {
                        source
                            .getchannel(i32::try_from(channel).unwrap())
                            .unwrap()
                            .tobytes()
                            .unwrap()
                    })
                    .collect::<Vec<_>>();
                let mut batch = BatchExecutor::new(true, Some(Backend::Cpu));
                for source in sources {
                    batch
                        .submit(
                            source,
                            BatchOperation::ExtractBand {
                                channel: i32::try_from(channel).unwrap(),
                            },
                        )
                        .unwrap();
                }
                let actual = batch
                    .join()
                    .unwrap()
                    .iter()
                    .map(|image| {
                        assert_eq!(image.mode().unwrap(), "L");
                        image.tobytes().unwrap()
                    })
                    .collect::<Vec<_>>();
                assert_eq!(actual, expected, "queued {mode} channel {channel} differs");
            }
        }
    }

    #[test]
    fn queued_multiply_reuses_native_mode_pipeline_and_preserves_order() {
        let modes = [("L", 1usize), ("LA", 2), ("RGB", 3), ("RGBA", 4)];
        for (mode, channels) in modes {
            let operands = [
                (
                    fixture(mode, 7, 5, channels, 17),
                    fixture(mode, 7, 5, channels, 81),
                ),
                (
                    fixture(mode, 7, 5, channels, 203),
                    fixture(mode, 7, 5, channels, 149),
                ),
            ];
            let expected = operands
                .iter()
                .map(|(image, other)| {
                    crate::ops::chops::multiply(image, other)
                        .unwrap()
                        .tobytes()
                        .unwrap()
                })
                .collect::<Vec<_>>();
            let mut batch = BatchExecutor::new(true, Some(Backend::Cpu));
            for (image, other) in operands {
                batch
                    .submit(
                        image,
                        BatchOperation::Multiply {
                            other: Box::new(other),
                        },
                    )
                    .unwrap();
            }
            let actual = batch
                .join()
                .unwrap()
                .iter()
                .map(|image| {
                    assert_eq!(image.mode().unwrap(), mode);
                    image.tobytes().unwrap()
                })
                .collect::<Vec<_>>();
            assert_eq!(actual, expected, "queued {mode} Multiply outputs differ");
        }
    }

    #[test]
    fn extract_band_groups_only_valid_native_byte_channels() {
        for (mode, channels) in [("L", 1usize), ("LA", 2), ("RGB", 3), ("RGBA", 4)] {
            for channel in 0..channels {
                assert!(
                    BatchOperation::ExtractBand {
                        channel: i32::try_from(channel).unwrap()
                    }
                    .can_group(mode, (1, 1))
                );
            }
            assert!(
                !BatchOperation::ExtractBand {
                    channel: i32::try_from(channels).unwrap()
                }
                .can_group(mode, (1, 1))
            );
        }
        assert!(!BatchOperation::ExtractBand { channel: -1 }.can_group("RGB", (1, 1)));
        assert!(!BatchOperation::ExtractBand { channel: 0 }.can_group("P", (1, 1)));
    }

    #[test]
    fn multiply_groups_only_equal_native_mode_operands() {
        let matching = BatchOperation::Multiply {
            other: Box::new(fixture("RGB", 3, 2, 3, 41)),
        };
        assert!(matching.can_group("RGB", (3, 2)));
        assert!(!matching.can_group("RGB", (2, 3)));
        assert!(!matching.can_group("RGBA", (3, 2)));
        assert!(
            !BatchOperation::Multiply {
                other: Box::new(fixture("P", 3, 2, 1, 41)),
            }
            .can_group("P", (3, 2))
        );
    }

    #[test]
    fn paste_groups_only_same_size_native_sources_with_l_masks() {
        for (mode, channels) in [("L", 1), ("LA", 2), ("RGB", 3), ("RGBA", 4)] {
            let operation = BatchOperation::Paste {
                source: Box::new(fixture(mode, 3, 2, channels, 41)),
                mask: Box::new(fixture("L", 3, 2, 1, 23)),
            };
            assert!(operation.can_group(mode, (3, 2)), "mode {mode}");
            assert!(operation.matches_group(&operation), "mode {mode}");
            assert!(matches!(
                operation.pipeline_op(),
                Some(PipelineOp::Paste { .. })
            ));
        }

        let wrong_mask_mode = BatchOperation::Paste {
            source: Box::new(fixture("LA", 3, 2, 2, 41)),
            mask: Box::new(fixture("LA", 3, 2, 2, 23)),
        };
        assert!(!wrong_mask_mode.can_group("LA", (3, 2)));

        let wrong_source_size = BatchOperation::Paste {
            source: Box::new(fixture("LA", 2, 2, 2, 41)),
            mask: Box::new(fixture("L", 3, 2, 1, 23)),
        };
        assert!(!wrong_source_size.can_group("LA", (3, 2)));
        assert!(!wrong_source_size.can_group("PA", (3, 2)));
    }

    #[test]
    fn queued_paste_matches_the_single_image_operation_for_native_modes() {
        for (mode, channels) in [("L", 1), ("LA", 2), ("RGB", 3), ("RGBA", 4)] {
            let inputs = [
                (
                    fixture(mode, 7, 5, channels, 17),
                    fixture(mode, 7, 5, channels, 91),
                    fixture("L", 7, 5, 1, 39),
                ),
                (
                    fixture(mode, 7, 5, channels, 203),
                    fixture(mode, 7, 5, channels, 147),
                    fixture("L", 7, 5, 1, 71),
                ),
            ];
            let expected = inputs
                .iter()
                .map(|(destination, source, mask)| {
                    let mut expected = destination.clone();
                    expected
                        .paste_at(
                            crate::PasteSource::Image(Box::new(source.clone())),
                            None,
                            Some(mask),
                        )
                        .unwrap();
                    expected.tobytes().unwrap()
                })
                .collect::<Vec<_>>();

            let mut batch = BatchExecutor::new(true, Some(Backend::Cpu));
            for (destination, source, mask) in inputs {
                batch
                    .submit(
                        destination,
                        BatchOperation::Paste {
                            source: Box::new(source),
                            mask: Box::new(mask),
                        },
                    )
                    .unwrap();
            }
            let actual = batch
                .join()
                .unwrap()
                .iter()
                .map(|image| {
                    assert_eq!(image.mode().unwrap(), mode);
                    image.tobytes().unwrap()
                })
                .collect::<Vec<_>>();
            assert_eq!(actual, expected, "queued {mode} Paste results differ");
        }
    }

    #[test]
    fn composite_groups_only_matching_native_images_with_l_masks() {
        for (mode, channels) in [("L", 1), ("LA", 2), ("RGB", 3), ("RGBA", 4)] {
            let operation = BatchOperation::Composite {
                background: Box::new(fixture(mode, 3, 2, channels, 61)),
                mask: Box::new(fixture("L", 3, 2, 1, 23)),
            };
            assert!(operation.can_group(mode, (3, 2)), "mode {mode}");
            assert!(operation.matches_group(&operation), "mode {mode}");
            assert!(matches!(
                operation.pipeline_op(),
                Some(PipelineOp::CompositeModule {
                    mask_alpha: false,
                    ..
                })
            ));
        }

        let wrong_background_mode = BatchOperation::Composite {
            background: Box::new(fixture("RGBA", 3, 2, 4, 61)),
            mask: Box::new(fixture("L", 3, 2, 1, 23)),
        };
        assert!(!wrong_background_mode.can_group("RGB", (3, 2)));

        let wrong_background_size = BatchOperation::Composite {
            background: Box::new(fixture("LA", 2, 2, 2, 61)),
            mask: Box::new(fixture("L", 3, 2, 1, 23)),
        };
        assert!(!wrong_background_size.can_group("LA", (3, 2)));

        let wrong_mask_mode = BatchOperation::Composite {
            background: Box::new(fixture("LA", 3, 2, 2, 61)),
            mask: Box::new(fixture("LA", 3, 2, 2, 23)),
        };
        assert!(!wrong_mask_mode.can_group("LA", (3, 2)));
    }

    #[test]
    fn queued_composite_reuses_the_existing_native_mode_operation() {
        for (mode, channels) in [("L", 1), ("LA", 2), ("RGB", 3), ("RGBA", 4)] {
            let inputs = [
                (
                    fixture(mode, 7, 5, channels, 17),
                    fixture(mode, 7, 5, channels, 91),
                    fixture("L", 7, 5, 1, 39),
                ),
                (
                    fixture(mode, 7, 5, channels, 203),
                    fixture(mode, 7, 5, channels, 147),
                    fixture("L", 7, 5, 1, 71),
                ),
            ];
            let expected = inputs
                .iter()
                .map(|(foreground, background, mask)| {
                    crate::ops::module_fns::composite(foreground, background, mask)
                        .unwrap()
                        .tobytes()
                        .unwrap()
                })
                .collect::<Vec<_>>();

            let mut batch = BatchExecutor::new(true, Some(Backend::Cpu));
            for (foreground, background, mask) in inputs {
                batch
                    .submit(
                        foreground,
                        BatchOperation::Composite {
                            background: Box::new(background),
                            mask: Box::new(mask),
                        },
                    )
                    .unwrap();
            }
            let actual = batch
                .join()
                .unwrap()
                .iter()
                .map(|image| {
                    assert_eq!(image.mode().unwrap(), mode);
                    image.tobytes().unwrap()
                })
                .collect::<Vec<_>>();
            assert_eq!(actual, expected, "queued {mode} Composite outputs differ");
        }
    }

    #[test]
    fn queued_extract_band_keeps_single_image_channel_errors() {
        let mut batch = BatchExecutor::new(true, Some(Backend::Cpu));
        batch
            .submit(
                fixture("RGB", 3, 2, 3, 59),
                BatchOperation::ExtractBand { channel: 3 },
            )
            .unwrap();
        assert!(matches!(batch.join(), Err(PilError::ValueError(_))));
    }

    #[test]
    fn nonqueued_batch_materializes_each_job_immediately() {
        let mut batch = BatchExecutor::new(false, Some(Backend::Cpu));
        let submitted = batch
            .submit(
                fixture("L", 5, 3, 1, 23),
                BatchOperation::MedianFilter { size: 3 },
            )
            .unwrap();
        assert_eq!(submitted, 0);
        assert_eq!(batch.join().unwrap().len(), 1);
    }
}
