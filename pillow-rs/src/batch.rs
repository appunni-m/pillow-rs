//! Explicit batching for compatible image jobs.
//!
//! This API is separate from [`crate::Image`] methods and never changes normal
//! image routing. Grouped workloads reuse the existing operation pipelines
//! over compatible native-mode images packed into one image, then split the
//! results back into ordinary per-image results. Grouped operations reuse
//! `MedianFilter(3)`, `ExtractBand`, `ImageChops.multiply`, and same-mode RGBA
//! `Color3DLUT` pipelines. Images that cannot be grouped use their ordinary
//! single-image pipeline.

use crate::compute::Backend;
use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::{PipelineOp, PixelMode};
use std::sync::Arc;

/// One explicitly submitted Pillow operation.
#[derive(Debug, Clone)]
pub enum BatchOperation {
    /// Apply Pillow's `ImageFilter.MedianFilter(size)` operation.
    MedianFilter {
        /// Odd square filter size.
        size: u32,
    },
    /// Extract one byte channel using Pillow's `Image.getchannel` operation.
    ExtractBand {
        /// Zero-based source channel index.
        channel: i32,
    },
    /// Apply Pillow's `ImageChops.multiply(image, other)` operation.
    Multiply {
        /// The second image operand. Compatible images are stacked separately
        /// in their native mode and passed through the ordinary Multiply pipeline.
        other: Box<Image>,
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
            Self::ExtractBand { channel } => image.getchannel(*channel),
            Self::Multiply { other } => crate::ops::chops::multiply(image, other),
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
            Self::MedianFilter { size } => *size == 3,
            Self::ExtractBand { channel } => {
                usize::try_from(*channel).is_ok_and(|channel| channel < channels)
            }
            Self::Multiply { other } => {
                other.mode().is_ok_and(|other_mode| other_mode == mode)
                    && other.size().is_ok_and(|other_size| other_size == size)
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
            (Self::ExtractBand { channel: left }, Self::ExtractBand { channel: right }) => {
                left == right
            }
            (Self::Multiply { .. }, Self::Multiply { .. }) => true,
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
            Self::ExtractBand { channel } => Some(PipelineOp::ExtractBand {
                index: u8::try_from(*channel).ok()?,
            }),
            Self::Multiply { other } => Some(PipelineOp::Multiply {
                other: Arc::new((**other).clone()),
            }),
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
/// compatible `MedianFilter(3)`, `ExtractBand`, `ImageChops.multiply`, and
/// same-mode RGBA `Color3DLUT` jobs when GPU is the selected backend. Grouping
/// currently requires equal-size native byte modes `L`, `LA`, `RGB`, and
/// `RGBA`; LUT groups additionally require the same shared LUT object. Other
/// jobs are executed through the ordinary per-image operation. No conversion
/// to RGBA is performed.
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
                    .pipeline_op()
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
            BatchOperation::MedianFilter { size: 3 } => {
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
            BatchOperation::Multiply { .. } => {
                let group_len = u32::try_from(indices.len())
                    .map_err(|_| PilError::DimensionError("batch count overflow".into()))?;
                let stacked_height = height
                    .checked_mul(group_len)
                    .ok_or_else(|| PilError::DimensionError("batch height overflow".into()))?;
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
        let mut packed_other = if matches!(first.operation, BatchOperation::Multiply { .. }) {
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

        for index in indices {
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

            if let BatchOperation::Multiply { other } = &job.operation {
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

        let output_row_bytes = usize::try_from(width)
            .ok()
            .and_then(|width| width.checked_mul(output_channels))
            .ok_or_else(|| PilError::DimensionError("batch output row size overflow".into()))?;
        let halo_bytes = output_row_bytes
            .checked_mul(halo)
            .ok_or_else(|| PilError::DimensionError("batch halo size overflow".into()))?;
        let image_rows =
            height_usize
                .checked_add(halo.checked_mul(2).ok_or_else(|| {
                    PilError::DimensionError("batch halo row count overflow".into())
                })?)
                .ok_or_else(|| PilError::DimensionError("batch row count overflow".into()))?;
        let image_stride = output_row_bytes
            .checked_mul(image_rows)
            .ok_or_else(|| PilError::DimensionError("batch output stride overflow".into()))?;
        let image_bytes = output_row_bytes
            .checked_mul(height_usize)
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
                .and_then(|start| start.checked_add(halo_bytes))
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
            let pixels = Image::frombytes(output_mode, (width, height), output)?.materialize()?;
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
    use super::{BatchExecutor, BatchOperation};
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
