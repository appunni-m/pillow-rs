//! RGB/CMYK PutPixel projection terminals use their prepared CPU mode and
//! architecture strategies. GPU terminal admission remains backend-specific.

use super::{Arc, DynamicImage, Image, Mode, PilError, PipelineOp, PipelineOps, ProjectionAxes};

type ProjectionResult = (Vec<u32>, Vec<u32>, u64, u64);

// TODO(pillow-rs#3ptb): Route the GPU terminal through its operation-owned
// support declaration and retain the prepared GPU context through execution.

struct PreparedPutPixelProjection<'a> {
    mode: Mode,
    terminal_backend: crate::compute::Backend,
    execution_context: crate::compute::ExecutionContext,
    prepared: crate::compute::PreparedExecution,
    ops: &'a [PipelineOp],
    input: Arc<DynamicImage>,
    source_is_known_zero: bool,
    output_sizes: [usize; 2],
}

fn projection_terminal_backend(
    backend: Option<crate::compute::Backend>,
) -> Result<Option<crate::compute::Backend>, PilError> {
    let selected = match backend {
        Some(crate::compute::Backend::Cpu) => Some(crate::compute::Backend::Cpu),
        Some(crate::compute::Backend::Simd) => Some(crate::compute::Backend::Simd),
        #[cfg(feature = "gpu")]
        Some(crate::compute::Backend::Gpu) => Some(crate::compute::Backend::Gpu),
        #[cfg(not(feature = "gpu"))]
        Some(crate::compute::Backend::Gpu) => None,
        // A single-backend profile already gives unlocked lazy graphs an
        // unambiguous route. Preserve terminal fusion in those profiles
        // without requiring benchmark adapters to mutate each image handle
        // just to prove which backend executed.
        None => match crate::compute::preferred_active_backend()? {
            Some((backend, true)) => Some(backend),
            _ => None,
        },
    };
    Ok(selected)
}

impl Image {
    fn cmyk_projection_with_putpixel_ops(
        context: &crate::compute::ExecutionContext,
        image: &crate::raster::RgbaImage,
        ops: &[PipelineOp],
        source_is_known_zero: bool,
    ) -> Result<(Vec<u32>, Vec<u32>), PilError> {
        Self::cmyk_projection_with_putpixel_ops_using(image, ops, source_is_known_zero, || {
            let output = crate::compute::cpu_cmyk_projection_in_context(context, image)?;
            Ok((output.axes.0, output.axes.1, output.vector_blocks, output.scalar_tail))
        })
        .map(|(horizontal, vertical, _, _)| (horizontal, vertical))
    }

    fn rgb_projection_with_putpixel_ops(
        context: &crate::compute::ExecutionContext,
        image: &crate::raster::RgbImage,
        ops: &[PipelineOp],
        source_is_known_zero: bool,
    ) -> Result<(Vec<u32>, Vec<u32>), PilError> {
        Self::rgb_projection_with_putpixel_ops_using(image, ops, source_is_known_zero, || {
            let output = crate::compute::cpu_rgb_projection_in_context(
                context,
                image,
                Mode::ImageRgb8,
            )?;
            Ok((output.axes.0, output.axes.1, output.vector_blocks, output.scalar_tail))
        })
        .map(|(horizontal, vertical, _, _)| (horizontal, vertical))
    }

    fn cmyk_projection_with_putpixel_ops_using(
        image: &crate::raster::RgbaImage,
        ops: &[PipelineOp],
        source_is_known_zero: bool,
        project: impl FnOnce() -> Result<ProjectionResult, PilError>,
    ) -> Result<(Vec<u32>, Vec<u32>, u64, u64), PilError> {
        Self::projection_with_putpixel_ops_using(
            image.width(),
            image.height(),
            image.as_raw(),
            4,
            Mode::ImageCmyk8,
            ops,
            source_is_known_zero,
            project,
        )
    }

    fn rgb_projection_with_putpixel_ops_using(
        image: &crate::raster::RgbImage,
        ops: &[PipelineOp],
        source_is_known_zero: bool,
        project: impl FnOnce() -> Result<ProjectionResult, PilError>,
    ) -> Result<(Vec<u32>, Vec<u32>, u64, u64), PilError> {
        Self::projection_with_putpixel_ops_using(
            image.width(),
            image.height(),
            image.as_raw(),
            3,
            Mode::ImageRgb8,
            ops,
            source_is_known_zero,
            project,
        )
    }

    fn projection_with_putpixel_ops_using(
        width: u32,
        height: u32,
        bytes: &[u8],
        channels: usize,
        mode: Mode,
        ops: &[PipelineOp],
        source_is_known_zero: bool,
        project: impl FnOnce() -> Result<ProjectionResult, PilError>,
    ) -> Result<(Vec<u32>, Vec<u32>, u64, u64), PilError> {
        let (w, h) = (width as usize, height as usize);
        let mut pending_writes = Vec::with_capacity(ops.len());
        for (order, op) in ops.iter().enumerate() {
            let PipelineOp::PutPixel { x, y, color, .. } = op else {
                return Err(PilError::InternalError(format!(
                    "{mode:?} projection fusion received a non-pixel operation"
                )));
            };
            if *x >= width || *y >= height {
                return Err(PilError::IndexError("image index out of range".into()));
            }
            let index = *y as usize * w + *x as usize;
            pending_writes.push((index, order, [color.0, color.1, color.2, color.3]));
        }
        pending_writes.sort_unstable_by_key(|(index, order, _)| (*index, *order));
        let mut writes: Vec<(usize, [u8; 4])> = Vec::with_capacity(pending_writes.len());
        for (index, _, color) in pending_writes {
            if let Some((previous_index, previous_color)) = writes.last_mut()
                && *previous_index == index
            {
                *previous_color = color;
            } else {
                writes.push((index, color));
            }
        }

        // The constructor's immutable fill invariant lets CPU Default derive
        // the result from deferred writes without rereading the backing frame.
        // Architecture-selected callers leave this false so the selected
        // vector scanner still performs the measured projection work.
        let (mut h_proj, mut v_proj, vector_blocks, scalar_tail) = if source_is_known_zero {
            (vec![0; w], vec![0; h], 0, 0)
        } else {
            project()?
        };
        let mut cleared_columns = Vec::new();
        let mut cleared_rows = Vec::new();
        if source_is_known_zero {
            for &(index, color) in &writes {
                if color[..channels].iter().any(|value| *value != 0) {
                    h_proj[index % w] = 1;
                    v_proj[index / w] = 1;
                }
            }
        } else {
            for &(index, color) in &writes {
                let x = index % w;
                let y = index / w;
                if color[..channels].iter().any(|value| *value != 0) {
                    h_proj[x] = 1;
                    v_proj[y] = 1;
                } else {
                    cleared_columns.push(x);
                    cleared_rows.push(y);
                }
            }
        }
        cleared_columns.sort_unstable();
        cleared_columns.dedup();
        cleared_rows.sort_unstable();
        cleared_rows.dedup();

        let pixel_is_nonzero = |index: usize| {
            if let Ok(write) = writes.binary_search_by_key(&index, |(write_index, _)| *write_index)
            {
                writes[write].1[..channels].iter().any(|value| *value != 0)
            } else {
                let offset = index * channels;
                bytes[offset..offset + channels]
                    .iter()
                    .any(|value| *value != 0)
            }
        };
        for x in cleared_columns {
            h_proj[x] = u32::from((0..h).any(|y| pixel_is_nonzero(y * w + x)));
        }
        for y in cleared_rows {
            v_proj[y] = u32::from((0..w).any(|x| pixel_is_nonzero(y * w + x)));
        }
        Ok((h_proj, v_proj, vector_blocks, scalar_tail))
    }

    fn prepare_putpixel_projection<'a>(
        source: &Image,
        ops: &'a PipelineOps,
        mode: Mode,
        backend: Option<crate::compute::Backend>,
    ) -> Result<Option<PreparedPutPixelProjection<'a>>, PilError> {
        let Some(terminal_backend) = projection_terminal_backend(backend)? else {
            return Ok(None);
        };
        let ops = ops.as_slice();
        if !ops
            .iter()
            .all(|op| matches!(op, PipelineOp::PutPixel { .. }))
        {
            return Ok(None);
        }
        let prepared = crate::compute::prepare_execution(ops, Some(terminal_backend))?;
        let execution_context = *prepared.context();
        let backend_matches = match terminal_backend {
            crate::compute::Backend::Cpu => prepared.is_serial_cpu_without_fallback(),
            crate::compute::Backend::Simd => {
                prepared.is_cpu_architecture_alias_without_fallback()
            }
            crate::compute::Backend::Gpu => prepared.is_gpu_without_fallback(),
        };
        if !backend_matches {
            return Ok(None);
        }
        let source_is_known_zero = matches!(
            source,
            Image::Loaded(data) if data.known_zero_fill
        );
        let input = source.materialized_shared()?;
        if input.mode() != mode {
            return Ok(None);
        }
        #[cfg(feature = "gpu")]
        if terminal_backend == crate::compute::Backend::Gpu {
            let supported = match mode {
                Mode::ImageRgb8 => {
                    crate::compute::gpu_rgb_projection_terminal_supported(input.as_ref(), ops)
                }
                Mode::ImageCmyk8 => {
                    crate::compute::gpu_cmyk_projection_terminal_supported(input.as_ref(), ops)
                }
                _ => false,
            };
            if !supported {
                return Ok(None);
            }
        }
        let (width, height) = (input.width() as usize, input.height() as usize);
        let output_sizes = [
            width * std::mem::size_of::<u32>(),
            height * std::mem::size_of::<u32>(),
        ];
        Ok(Some(PreparedPutPixelProjection {
            mode,
            terminal_backend,
            execution_context,
            prepared,
            ops,
            input,
            source_is_known_zero,
            output_sizes,
        }))
    }

    #[inline(always)]
    fn execute_cmyk_cpu_putpixel_projection(
        execution: &PreparedPutPixelProjection<'_>,
    ) -> Result<Option<ProjectionAxes>, PilError> {
        let DynamicImage::ImageCmyk8(image) = execution.input.as_ref() else {
            return Ok(None);
        };
        crate::compute::execute_prepared_cpu_terminal(
            &execution.prepared,
            execution.ops,
            execution.input.as_ref(),
            execution.output_sizes,
            || {
                Self::cmyk_projection_with_putpixel_ops(
                    &execution.execution_context,
                    image,
                    execution.ops,
                    execution.source_is_known_zero,
                )
            },
        )
        .map(Some)
    }

    #[inline(always)]
    fn execute_rgb_cpu_putpixel_projection(
        execution: &PreparedPutPixelProjection<'_>,
    ) -> Result<Option<ProjectionAxes>, PilError> {
        let DynamicImage::ImageRgb8(image) = execution.input.as_ref() else {
            return Ok(None);
        };
        crate::compute::execute_prepared_cpu_terminal(
            &execution.prepared,
            execution.ops,
            execution.input.as_ref(),
            execution.output_sizes,
            || {
                Self::rgb_projection_with_putpixel_ops(
                    &execution.execution_context,
                    image,
                    execution.ops,
                    execution.source_is_known_zero,
                )
            },
        )
        .map(Some)
    }

    #[inline(always)]
    fn execute_cmyk_arch_putpixel_projection(
        execution: &PreparedPutPixelProjection<'_>,
    ) -> Result<Option<ProjectionAxes>, PilError> {
        let DynamicImage::ImageCmyk8(image) = execution.input.as_ref() else {
            return Ok(None);
        };
        crate::compute::execute_prepared_cpu_arch_terminal(
            &execution.prepared,
            execution.ops,
            execution.input.as_ref(),
            execution.output_sizes,
            || {
                if execution.source_is_known_zero
                    && let Some(output) = crate::compute::pool_cpu::ops::analysis::cmyk8::ops::execute_positive_writes(
                        &execution.execution_context,
                        image.width(),
                        image.height(),
                        execution.ops,
                    )?
                {
                    return Ok((output.axes, output.vector_blocks, output.scalar_tail));
                }
                let output = crate::compute::cpu_cmyk_projection_in_context(
                    &execution.execution_context,
                    image,
                )?;
                let (horizontal, vertical, _, _) = Self::cmyk_projection_with_putpixel_ops_using(
                    image,
                    execution.ops,
                    false,
                    || Ok((output.axes.0, output.axes.1, output.vector_blocks, output.scalar_tail)),
                )?;
                Ok(((horizontal, vertical), output.vector_blocks, output.scalar_tail))
            },
        )
        .map(Some)
    }

    #[inline(always)]
    fn execute_rgb_arch_putpixel_projection(
        execution: &PreparedPutPixelProjection<'_>,
    ) -> Result<Option<ProjectionAxes>, PilError> {
        let DynamicImage::ImageRgb8(image) = execution.input.as_ref() else {
            return Ok(None);
        };
        crate::compute::execute_prepared_cpu_arch_terminal(
            &execution.prepared,
            execution.ops,
            execution.input.as_ref(),
            execution.output_sizes,
            || {
                let output = crate::compute::cpu_rgb_projection_in_context(
                    &execution.execution_context,
                    image,
                    Mode::ImageRgb8,
                )?;
                let (horizontal, vertical, _, _) = Self::rgb_projection_with_putpixel_ops_using(
                    image,
                    execution.ops,
                    false,
                    || Ok((output.axes.0, output.axes.1, output.vector_blocks, output.scalar_tail)),
                )?;
                Ok(((horizontal, vertical), output.vector_blocks, output.scalar_tail))
            },
        )
        .map(Some)
    }

    fn execute_prepared_putpixel_projection(
        execution: PreparedPutPixelProjection<'_>,
    ) -> Result<Option<ProjectionAxes>, PilError> {
        match (execution.mode, execution.terminal_backend) {
            (Mode::ImageCmyk8, crate::compute::Backend::Cpu) => {
                Self::execute_cmyk_cpu_putpixel_projection(&execution)
            }
            (Mode::ImageRgb8, crate::compute::Backend::Cpu) => {
                Self::execute_rgb_cpu_putpixel_projection(&execution)
            }
            (Mode::ImageCmyk8, crate::compute::Backend::Simd) => {
                Self::execute_cmyk_arch_putpixel_projection(&execution)
            }
            (Mode::ImageRgb8, crate::compute::Backend::Simd) => {
                Self::execute_rgb_arch_putpixel_projection(&execution)
            }
            #[cfg(feature = "gpu")]
            (Mode::ImageCmyk8, crate::compute::Backend::Gpu) => {
                crate::compute::execute_prepared_gpu_cmyk_projection_terminal(
                    &execution.prepared,
                    execution.ops,
                    execution.input.as_ref(),
                    execution.source_is_known_zero,
                )
                .map(Some)
            }
            #[cfg(feature = "gpu")]
            (Mode::ImageRgb8, crate::compute::Backend::Gpu) => {
                crate::compute::execute_prepared_gpu_rgb_projection_terminal(
                    &execution.prepared,
                    execution.ops,
                    execution.input.as_ref(),
                    execution.source_is_known_zero,
                )
                .map(Some)
            }
            #[cfg(not(feature = "gpu"))]
            (_, crate::compute::Backend::Gpu) => Ok(None),
            _ => Ok(None),
        }
    }

    pub(super) fn try_getprojection_putpixel_pipeline(
        &self,
    ) -> Result<Option<ProjectionAxes>, PilError> {
        // A projection is the terminal result here. Applying queued RGB or
        // CMYK pixels to the two axes is exact and avoids creating a full-frame
        // image that the caller never receives. The lazy pipeline remains
        // intact for any later image operation.
        let Image::Pipeline {
            source,
            ops,
            backend,
            ..
        } = self
        else {
            return Ok(None);
        };
        if !matches!(source.as_ref(), Image::Loaded(_) | Image::Bytes { .. }) {
            return Ok(None);
        }
        let mode = match self
            .known_mode_identity()
            .or_else(|| source.known_mode_identity())
        {
            Some(mode) => mode,
            None => source.materialized_shared()?.mode(),
        };
        if !matches!(mode, Mode::ImageRgb8 | Mode::ImageCmyk8) {
            return Ok(None);
        }
        if ops.is_empty() {
            return Ok(None);
        }
        let _origin_scope = ops.telemetry_origin_scope();
        let Some(execution) = Self::prepare_putpixel_projection(source, ops, mode, *backend)?
        else {
            return Ok(None);
        };
        Self::execute_prepared_putpixel_projection(execution)
    }
}
