use super::{Arc, DynamicImage, Image, PilError, PipelineOp, PipelineOps, ProjectionAxes};
struct PreparedPutPixelProjection<'a> {
    mode: &'static str,
    terminal_backend: crate::compute::Backend,
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
        image: &crate::raster::RgbaImage,
        ops: &[PipelineOp],
        source_is_known_zero: bool,
    ) -> Result<(Vec<u32>, Vec<u32>), PilError> {
        Self::cmyk_projection_with_putpixel_ops_using(image, ops, source_is_known_zero, |image| {
            let (horizontal, vertical) = Self::cmyk_projection(image);
            (horizontal, vertical, 0, 0)
        })
        .map(|(horizontal, vertical, _, _)| (horizontal, vertical))
    }

    fn rgb_projection_with_putpixel_ops(
        image: &crate::raster::RgbImage,
        ops: &[PipelineOp],
        source_is_known_zero: bool,
    ) -> Result<(Vec<u32>, Vec<u32>), PilError> {
        Self::rgb_projection_with_putpixel_ops_using(image, ops, source_is_known_zero, |image| {
            let (horizontal, vertical) = Self::rgb_projection(image);
            (horizontal, vertical, 0, 0)
        })
        .map(|(horizontal, vertical, _, _)| (horizontal, vertical))
    }

    fn cmyk_projection_with_putpixel_ops_using(
        image: &crate::raster::RgbaImage,
        ops: &[PipelineOp],
        source_is_known_zero: bool,
        project: impl FnOnce(&crate::raster::RgbaImage) -> (Vec<u32>, Vec<u32>, u64, u64),
    ) -> Result<(Vec<u32>, Vec<u32>, u64, u64), PilError> {
        Self::projection_with_putpixel_ops_using(
            image.width(),
            image.height(),
            image.as_raw(),
            4,
            "CMYK",
            ops,
            source_is_known_zero,
            || project(image),
        )
    }

    fn rgb_projection_with_putpixel_ops_using(
        image: &crate::raster::RgbImage,
        ops: &[PipelineOp],
        source_is_known_zero: bool,
        project: impl FnOnce(&crate::raster::RgbImage) -> (Vec<u32>, Vec<u32>, u64, u64),
    ) -> Result<(Vec<u32>, Vec<u32>, u64, u64), PilError> {
        Self::projection_with_putpixel_ops_using(
            image.width(),
            image.height(),
            image.as_raw(),
            3,
            "RGB",
            ops,
            source_is_known_zero,
            || project(image),
        )
    }

    fn projection_with_putpixel_ops_using(
        width: u32,
        height: u32,
        bytes: &[u8],
        channels: usize,
        mode: &str,
        ops: &[PipelineOp],
        source_is_known_zero: bool,
        project: impl FnOnce() -> (Vec<u32>, Vec<u32>, u64, u64),
    ) -> Result<(Vec<u32>, Vec<u32>, u64, u64), PilError> {
        let (w, h) = (width as usize, height as usize);
        let mut pending_writes = Vec::with_capacity(ops.len());
        for (order, op) in ops.iter().enumerate() {
            let PipelineOp::PutPixel { x, y, color, .. } = op else {
                return Err(PilError::InternalError(format!(
                    "{mode} projection fusion received a non-pixel operation"
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

        // The constructor's immutable fill invariant lets the serial CPU
        // terminal path derive the result from deferred writes without
        // rereading the full backing frame. SIMD callers deliberately leave
        // this false so their receipt still proves vector execution.
        let (mut h_proj, mut v_proj, vector_blocks, scalar_tail) = if source_is_known_zero {
            (vec![0; w], vec![0; h], 0, 0)
        } else {
            project()
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
        mode: &'static str,
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
        let backend_matches = match terminal_backend {
            crate::compute::Backend::Cpu => prepared.is_serial_cpu_without_fallback(),
            crate::compute::Backend::Simd => prepared.is_simd_without_fallback(),
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
        if source.mode_from_materialized(&input) != mode {
            return Ok(None);
        }
        #[cfg(feature = "gpu")]
        if terminal_backend == crate::compute::Backend::Gpu {
            let supported = match mode {
                "RGB" => crate::compute::gpu_rgb_projection_terminal_supported(input.as_ref(), ops),
                "CMYK" => {
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
        let DynamicImage::ImageRgba8(image) = execution.input.as_ref() else {
            return Ok(None);
        };
        crate::compute::execute_prepared_cpu_terminal(
            &execution.prepared,
            execution.ops,
            execution.input.as_ref(),
            execution.output_sizes,
            || {
                Self::cmyk_projection_with_putpixel_ops(
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
                    image,
                    execution.ops,
                    execution.source_is_known_zero,
                )
            },
        )
        .map(Some)
    }

    #[inline(always)]
    fn execute_cmyk_simd_putpixel_projection(
        execution: &PreparedPutPixelProjection<'_>,
    ) -> Result<Option<ProjectionAxes>, PilError> {
        let DynamicImage::ImageRgba8(image) = execution.input.as_ref() else {
            return Ok(None);
        };
        crate::compute::execute_prepared_simd_terminal(
            &execution.prepared,
            execution.ops,
            execution.input.as_ref(),
            execution.output_sizes,
            || {
                if let Some((horizontal, vertical, vector_blocks, scalar_tail)) = execution
                    .source_is_known_zero
                    .then(|| {
                        crate::compute::simd_cmyk_projection_from_positive_writes(
                            image.width(),
                            image.height(),
                            execution.ops,
                        )
                    })
                    .flatten()
                {
                    Ok(((horizontal, vertical), vector_blocks, scalar_tail))
                } else {
                    Self::cmyk_projection_with_putpixel_ops_using(
                        image,
                        execution.ops,
                        false,
                        crate::compute::simd_cmyk_projection,
                    )
                    .map(
                        |(horizontal, vertical, vector_blocks, scalar_tail)| {
                            ((horizontal, vertical), vector_blocks, scalar_tail)
                        },
                    )
                }
            },
        )
        .map(Some)
    }

    #[inline(always)]
    fn execute_rgb_simd_putpixel_projection(
        execution: &PreparedPutPixelProjection<'_>,
    ) -> Result<Option<ProjectionAxes>, PilError> {
        let DynamicImage::ImageRgb8(image) = execution.input.as_ref() else {
            return Ok(None);
        };
        crate::compute::execute_prepared_simd_terminal(
            &execution.prepared,
            execution.ops,
            execution.input.as_ref(),
            execution.output_sizes,
            || {
                Self::rgb_projection_with_putpixel_ops_using(
                    image,
                    execution.ops,
                    false,
                    crate::compute::simd_rgb_projection,
                )
                .map(|(horizontal, vertical, vector_blocks, scalar_tail)| {
                    ((horizontal, vertical), vector_blocks, scalar_tail)
                })
            },
        )
        .map(Some)
    }

    fn execute_prepared_putpixel_projection(
        execution: PreparedPutPixelProjection<'_>,
    ) -> Result<Option<ProjectionAxes>, PilError> {
        match (execution.mode, execution.terminal_backend) {
            ("CMYK", crate::compute::Backend::Cpu) => {
                Self::execute_cmyk_cpu_putpixel_projection(&execution)
            }
            ("RGB", crate::compute::Backend::Cpu) => {
                Self::execute_rgb_cpu_putpixel_projection(&execution)
            }
            ("CMYK", crate::compute::Backend::Simd) => {
                Self::execute_cmyk_simd_putpixel_projection(&execution)
            }
            ("RGB", crate::compute::Backend::Simd) => {
                Self::execute_rgb_simd_putpixel_projection(&execution)
            }
            #[cfg(feature = "gpu")]
            ("CMYK", crate::compute::Backend::Gpu) => {
                crate::compute::execute_prepared_gpu_cmyk_projection_terminal(
                    &execution.prepared,
                    execution.ops,
                    execution.input.as_ref(),
                    execution.source_is_known_zero,
                )
                .map(Some)
            }
            #[cfg(feature = "gpu")]
            ("RGB", crate::compute::Backend::Gpu) => {
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
            explicit_mode,
            backend,
            ..
        } = self
        else {
            return Ok(None);
        };
        if !matches!(source.as_ref(), Image::Loaded(_) | Image::Bytes { .. }) {
            return Ok(None);
        }
        let mode = match explicit_mode.as_deref() {
            Some("RGB") => "RGB",
            Some("CMYK") => "CMYK",
            Some(_) => return Ok(None),
            // RGB is represented by the native three-byte raster and usually
            // has no explicit mode tag. Resolve that ordinary mode from the
            // source before admitting the fused terminal path.
            None if source.mode()? == "RGB" => "RGB",
            None => return Ok(None),
        };
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
