//! Pillow-compatible horizontal and vertical image projections.

use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::DynamicImage;

use super::Image;

type ProjectionAxes = (Vec<u32>, Vec<u32>);

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
    fn cmyk_projection(image: &crate::raster::RgbaImage) -> (Vec<u32>, Vec<u32>) {
        Self::cmyk_projection_with_blocks(image, 16, |samples| {
            let mut packed = [0; 16];
            packed.copy_from_slice(samples);
            u128::from_ne_bytes(packed) != 0
        })
        .0
    }

    fn rgb_projection(image: &crate::raster::RgbImage) -> (Vec<u32>, Vec<u32>) {
        let (width, height) = (image.width() as usize, image.height() as usize);
        let mut horizontal = vec![0u32; width];
        let mut vertical = vec![0u32; height];
        for (y, row) in image.rows().enumerate() {
            let mut row_nonzero = false;
            for (x, pixel) in row.enumerate() {
                if pixel[0] != 0 || pixel[1] != 0 || pixel[2] != 0 {
                    horizontal[x] = 1;
                    row_nonzero = true;
                }
            }
            vertical[y] = u32::from(row_nonzero);
        }
        (horizontal, vertical)
    }

    /// Scan CMYK storage in fixed-size groups while leaving block detection to
    /// the selected execution profile. Every group is a whole number of
    /// four-byte pixels; the final partial group is handled pixel by pixel.
    pub(crate) fn cmyk_projection_with_blocks(
        image: &crate::raster::RgbaImage,
        block_bytes: usize,
        mut block_has_nonzero: impl FnMut(&[u8]) -> bool,
    ) -> ((Vec<u32>, Vec<u32>), u64) {
        let (w, h) = (image.width() as usize, image.height() as usize);
        let mut h_proj = vec![0u32; w];
        let mut v_proj = vec![0u32; h];
        let mut covered_columns = 0usize;
        let mut all_columns_covered = w == 0;
        let row_bytes = w * 4;
        let bytes = image.as_raw();
        debug_assert!(block_bytes >= 4 && block_bytes.is_multiple_of(4));
        let nonzero_4 = |samples: &[u8]| {
            let mut packed = [0; 4];
            packed.copy_from_slice(samples);
            u32::from_ne_bytes(packed) != 0
        };
        let pixels_per_block = block_bytes / 4;
        let mut scalar_tail_pixels = 0u64;
        for y in 0..h {
            let row = &bytes[y * row_bytes..(y + 1) * row_bytes];
            let blocks = row.chunks_exact(block_bytes);
            scalar_tail_pixels =
                scalar_tail_pixels.saturating_add((blocks.remainder().len() / 4) as u64);
            let row_nonzero = if all_columns_covered {
                blocks.clone().any(&mut block_has_nonzero)
                    || blocks.remainder().chunks_exact(4).any(nonzero_4)
            } else {
                let mut any = false;
                for (block_index, block) in blocks.clone().enumerate() {
                    if !block_has_nonzero(block) {
                        continue;
                    }
                    for (lane, pixel) in block.chunks_exact(4).enumerate() {
                        if !nonzero_4(pixel) {
                            continue;
                        }
                        let x = block_index * pixels_per_block + lane;
                        if h_proj[x] == 0 {
                            h_proj[x] = 1;
                            covered_columns += 1;
                            all_columns_covered = covered_columns == w;
                        }
                        any = true;
                    }
                }
                let tail_start = blocks.len() * block_bytes;
                for (tail_index, pixel) in row[tail_start..].chunks_exact(4).enumerate() {
                    if !nonzero_4(pixel) {
                        continue;
                    }
                    let x = tail_start / 4 + tail_index;
                    if h_proj[x] == 0 {
                        h_proj[x] = 1;
                        covered_columns += 1;
                        all_columns_covered = covered_columns == w;
                    }
                    any = true;
                }
                any
            };
            if row_nonzero {
                v_proj[y] = 1;
            }
        }
        ((h_proj, v_proj), scalar_tail_pixels)
    }

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

    fn try_getprojection_putpixel_pipeline(&self) -> Result<Option<ProjectionAxes>, PilError> {
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
        let terminal_backend = projection_terminal_backend(*backend)?;
        let Some(terminal_backend) = terminal_backend else {
            return Ok(None);
        };
        if !ops
            .as_slice()
            .iter()
            .all(|op| matches!(op, PipelineOp::PutPixel { .. }))
        {
            return Ok(None);
        }
        let prepared = crate::compute::prepare_execution(ops.as_slice(), Some(terminal_backend))?;
        let backend_matches = match terminal_backend {
            crate::compute::Backend::Cpu => prepared.is_serial_cpu_without_fallback(),
            crate::compute::Backend::Simd => prepared.is_simd_without_fallback(),
            crate::compute::Backend::Gpu => prepared.is_gpu_without_fallback(),
        };
        if !backend_matches {
            return Ok(None);
        }
        let source_is_known_zero = matches!(
            source.as_ref(),
            Image::Loaded(data) if data.known_zero_fill
        );
        let input = source.materialized_shared()?;
        if source.mode_from_materialized(&input) != mode {
            return Ok(None);
        }
        #[cfg(feature = "gpu")]
        if terminal_backend == crate::compute::Backend::Gpu {
            let supported = match mode {
                "RGB" => crate::compute::gpu_rgb_projection_terminal_supported(
                    input.as_ref(),
                    ops.as_slice(),
                ),
                "CMYK" => crate::compute::gpu_cmyk_projection_terminal_supported(
                    input.as_ref(),
                    ops.as_slice(),
                ),
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
        let result = match (mode, terminal_backend) {
            ("CMYK", crate::compute::Backend::Cpu) => {
                let DynamicImage::ImageRgba8(image) = input.as_ref() else {
                    return Ok(None);
                };
                crate::compute::execute_prepared_cpu_terminal(
                    &prepared,
                    ops.as_slice(),
                    input.as_ref(),
                    output_sizes,
                    || {
                        Self::cmyk_projection_with_putpixel_ops(
                            image,
                            ops.as_slice(),
                            source_is_known_zero,
                        )
                    },
                )?
            }
            ("RGB", crate::compute::Backend::Cpu) => {
                let DynamicImage::ImageRgb8(image) = input.as_ref() else {
                    return Ok(None);
                };
                crate::compute::execute_prepared_cpu_terminal(
                    &prepared,
                    ops.as_slice(),
                    input.as_ref(),
                    output_sizes,
                    || {
                        Self::rgb_projection_with_putpixel_ops(
                            image,
                            ops.as_slice(),
                            source_is_known_zero,
                        )
                    },
                )?
            }
            ("CMYK", crate::compute::Backend::Simd) => {
                let DynamicImage::ImageRgba8(image) = input.as_ref() else {
                    return Ok(None);
                };
                crate::compute::execute_prepared_simd_terminal(
                    &prepared,
                    ops.as_slice(),
                    input.as_ref(),
                    output_sizes,
                    || {
                        if let Some((horizontal, vertical, vector_blocks, scalar_tail)) =
                            source_is_known_zero
                                .then(|| {
                                    crate::compute::simd_cmyk_projection_from_positive_writes(
                                        image.width(),
                                        image.height(),
                                        ops.as_slice(),
                                    )
                                })
                                .flatten()
                        {
                            Ok(((horizontal, vertical), vector_blocks, scalar_tail))
                        } else {
                            Self::cmyk_projection_with_putpixel_ops_using(
                                image,
                                ops.as_slice(),
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
                )?
            }
            ("RGB", crate::compute::Backend::Simd) => {
                let DynamicImage::ImageRgb8(image) = input.as_ref() else {
                    return Ok(None);
                };
                crate::compute::execute_prepared_simd_terminal(
                    &prepared,
                    ops.as_slice(),
                    input.as_ref(),
                    output_sizes,
                    || {
                        Self::rgb_projection_with_putpixel_ops_using(
                            image,
                            ops.as_slice(),
                            false,
                            crate::compute::simd_rgb_projection,
                        )
                        .map(
                            |(horizontal, vertical, vector_blocks, scalar_tail)| {
                                ((horizontal, vertical), vector_blocks, scalar_tail)
                            },
                        )
                    },
                )?
            }
            #[cfg(feature = "gpu")]
            ("CMYK", crate::compute::Backend::Gpu) => {
                return crate::compute::execute_prepared_gpu_cmyk_projection_terminal(
                    &prepared,
                    ops.as_slice(),
                    input.as_ref(),
                    source_is_known_zero,
                )
                .map(Some);
            }
            #[cfg(feature = "gpu")]
            ("RGB", crate::compute::Backend::Gpu) => {
                return crate::compute::execute_prepared_gpu_rgb_projection_terminal(
                    &prepared,
                    ops.as_slice(),
                    input.as_ref(),
                    source_is_known_zero,
                )
                .map(Some);
            }
            #[cfg(not(feature = "gpu"))]
            (_, crate::compute::Backend::Gpu) => return Ok(None),
            _ => return Ok(None),
        };
        Ok(Some(result))
    }

    /// Returns horizontal and vertical non-zero pixel projections.
    ///
    /// The first vector has one entry per column and the second has one entry
    /// per row. Entries are `1` when any source sample on that axis is
    /// non-zero, otherwise `0`.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when materialization fails.
    pub fn getprojection(&self) -> Result<(Vec<u32>, Vec<u32>), PilError> {
        if let Some(projection) = self.try_getprojection_putpixel_pipeline()? {
            return Ok(projection);
        }
        let img = self.materialized_shared()?;
        let mode = self.mode_from_materialized(&img);
        if let Some(projection) =
            self.try_getprojection_eager_backend(img.as_ref(), mode.as_str())?
        {
            return Ok(projection);
        }
        if !crate::compute::Backend::pipeline_telemetry_enabled() {
            return Self::getprojection_from_materialized(img.as_ref(), mode.as_str());
        }

        let selected = crate::compute::preferred_active_backend().ok().flatten();
        let requested_backend = selected
            .filter(|(_, only_active_backend)| *only_active_backend)
            .map(|(backend, _)| backend);
        let fallback_reason = requested_backend
            .filter(|backend| *backend != crate::compute::Backend::Cpu)
            .map(|backend| {
                format!(
                    "GetProjection materialized pipeline used the host CPU while {backend:?} was requested"
                )
            });
        crate::compute::execute_eager_cpu_getprojection(
            img.as_ref(),
            requested_backend,
            fallback_reason,
            || Self::getprojection_from_materialized(img.as_ref(), mode.as_str()),
        )
    }

    fn try_getprojection_eager_backend(
        &self,
        img: &DynamicImage,
        mode: &str,
    ) -> Result<Option<ProjectionAxes>, PilError> {
        if !matches!(self, Image::Loaded(_) | Image::Paletted(_)) {
            return Ok(None);
        }
        let Some((selected, only_active_backend)) = crate::compute::preferred_active_backend()?
        else {
            return Ok(None);
        };
        let requested = only_active_backend.then_some(selected);
        match selected {
            crate::compute::Backend::Cpu => {
                crate::compute::execute_eager_cpu_getprojection(img, requested, None, || {
                    Self::getprojection_from_materialized(img, mode)
                })
                .map(Some)
            }
            crate::compute::Backend::Simd if mode == "RGB" => {
                let DynamicImage::ImageRgb8(image) = img else {
                    return Ok(None);
                };
                crate::compute::execute_eager_simd_getprojection(img, || {
                    let (horizontal, vertical, vector_blocks, scalar_tail) =
                        crate::compute::simd_rgb_projection(image);
                    Ok(((horizontal, vertical), vector_blocks, scalar_tail))
                })
                .map(Some)
            }
            crate::compute::Backend::Simd if mode == "CMYK" => {
                let DynamicImage::ImageRgba8(image) = img else {
                    return Ok(None);
                };
                crate::compute::execute_eager_simd_getprojection(img, || {
                    let (horizontal, vertical, vector_blocks, scalar_tail) =
                        crate::compute::simd_cmyk_projection(image);
                    Ok(((horizontal, vertical), vector_blocks, scalar_tail))
                })
                .map(Some)
            }
            crate::compute::Backend::Simd => crate::compute::execute_eager_cpu_getprojection(
                img,
                requested,
                Some(format!("SIMD getprojection is unsupported for mode {mode}")),
                || Self::getprojection_from_materialized(img, mode),
            )
            .map(Some),
            #[cfg(feature = "gpu")]
            crate::compute::Backend::Gpu if mode == "RGB" => {
                if crate::compute::gpu_rgb_projection_supported(img) {
                    crate::compute::execute_eager_gpu_rgb_getprojection(img).map(Some)
                } else {
                    crate::compute::execute_eager_cpu_getprojection(
                        img,
                        requested,
                        Some("GPU getprojection is unsupported for this RGB image".into()),
                        || Self::getprojection_from_materialized(img, mode),
                    )
                    .map(Some)
                }
            }
            #[cfg(feature = "gpu")]
            crate::compute::Backend::Gpu => crate::compute::execute_eager_cpu_getprojection(
                img,
                requested,
                Some(format!("GPU getprojection is unsupported for mode {mode}")),
                || Self::getprojection_from_materialized(img, mode),
            )
            .map(Some),
            #[cfg(not(feature = "gpu"))]
            crate::compute::Backend::Gpu => crate::compute::execute_eager_cpu_getprojection(
                img,
                requested,
                Some(format!("GPU getprojection is unsupported for mode {mode}")),
                || Self::getprojection_from_materialized(img, mode),
            )
            .map(Some),
        }
    }

    fn getprojection_from_materialized(
        img: &DynamicImage,
        mode: &str,
    ) -> Result<(Vec<u32>, Vec<u32>), PilError> {
        let (w, h) = (img.width() as usize, img.height() as usize);
        let mut h_proj = vec![0u32; w];
        let mut v_proj = vec![0u32; h];

        // Pillow's ImagingGetProjection checks source bands for non-zero
        // samples. Converting RGB/RGBA to luma first loses low red/blue and
        // alpha-only pixels, even though Pillow projects those pixels.
        if matches!(mode, "I" | "F") {
            // Materialization validates scalar storage, so decode each sample
            // from the retained frame a row at a time. Projection only records
            // whether a column and row contain any non-zero word, so direct
            // coordinates avoid a division/modulo and repeated row writes per
            // sample. Once every column is marked, each remaining row needs
            // only its first non-zero sample to determine its vertical bit.
            let row_bytes = w.checked_mul(std::mem::size_of::<u32>()).ok_or_else(|| {
                PilError::DimensionError("getprojection row byte count overflow".into())
            })?;
            if row_bytes != 0 {
                let raw = img.as_bytes();
                let mut remaining_columns = w;
                for (y, row) in raw.chunks_exact(row_bytes).take(h).enumerate() {
                    let row_nonzero = if remaining_columns == 0 {
                        row.chunks_exact(4).any(|sample| {
                            u32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]]) != 0
                        })
                    } else {
                        let mut row_nonzero = false;
                        if mode == "I" {
                            for (x, sample) in row.chunks_exact(4).enumerate() {
                                let value = i32::from_le_bytes([
                                    sample[0], sample[1], sample[2], sample[3],
                                ]);
                                if value != 0 {
                                    row_nonzero = true;
                                    if h_proj[x] == 0 {
                                        h_proj[x] = 1;
                                        remaining_columns -= 1;
                                    }
                                    if remaining_columns == 0 {
                                        break;
                                    }
                                }
                            }
                        } else {
                            for (x, sample) in row.chunks_exact(4).enumerate() {
                                // Pillow projects F samples by their stored
                                // word: -0.0's sign bit makes it nonzero.
                                let value = u32::from_le_bytes([
                                    sample[0], sample[1], sample[2], sample[3],
                                ]);
                                if value != 0 {
                                    row_nonzero = true;
                                    if h_proj[x] == 0 {
                                        h_proj[x] = 1;
                                        remaining_columns -= 1;
                                    }
                                    if remaining_columns == 0 {
                                        break;
                                    }
                                }
                            }
                        }
                        row_nonzero
                    };
                    if row_nonzero {
                        v_proj[y] = 1;
                    }
                }
            }
        } else {
            let mut mark = |index: usize| {
                let x = index % w;
                let y = index / w;
                h_proj[x] = 1;
                v_proj[y] = 1;
            };
            match img {
                crate::raster::DynamicImage::ImageLuma8(image)
                    if matches!(mode, "L" | "1" | "P") =>
                {
                    for (index, pixel) in image.pixels().enumerate() {
                        if pixel[0] != 0 {
                            mark(index);
                        }
                    }
                }
                crate::raster::DynamicImage::ImageLumaA8(image) if matches!(mode, "LA" | "PA") => {
                    for (index, pixel) in image.pixels().enumerate() {
                        if pixel[0] != 0 || pixel[1] != 0 {
                            mark(index);
                        }
                    }
                }
                crate::raster::DynamicImage::ImageRgb8(image)
                    if matches!(mode, "RGB" | "HSV" | "YCbCr") =>
                {
                    if w >= 64 {
                        // For wider images, walk packed rows directly to avoid
                        // a per-pixel index division in `mark`. Once every
                        // horizontal column is set, later rows only need a
                        // short-circuit nonzero scan for their vertical bit.
                        let row_bytes = w * 3;
                        let mut covered_columns = 0usize;
                        let mut all_columns_covered = false;
                        for (y, row) in image.as_raw().chunks_exact(row_bytes).enumerate() {
                            if all_columns_covered {
                                v_proj[y] = u32::from(row.iter().any(|value| *value != 0));
                                continue;
                            }
                            let mut row_nonzero = false;
                            for (x, pixel) in row.chunks_exact(3).enumerate() {
                                if pixel[0] == 0 && pixel[1] == 0 && pixel[2] == 0 {
                                    continue;
                                }
                                if h_proj[x] == 0 {
                                    h_proj[x] = 1;
                                    covered_columns += 1;
                                }
                                row_nonzero = true;
                                if covered_columns == w {
                                    all_columns_covered = true;
                                    break;
                                }
                            }
                            v_proj[y] = u32::from(row_nonzero);
                        }
                    } else {
                        // Keep the existing traversal for narrow images, where
                        // row-state setup did not produce a reliable gain.
                        for (index, pixel) in image.pixels().enumerate() {
                            if pixel.0.iter().any(|value| *value != 0) {
                                mark(index);
                            }
                        }
                    }
                }
                crate::raster::DynamicImage::ImageRgb8(image) if mode == "LAB" => {
                    // LAB stores its logical A/B zero points at byte value
                    // 128. Scanning through RGBA would synthesize alpha 255
                    // and mark every pixel, while testing raw bytes against
                    // zero would mark neutral LAB pixels.
                    for (index, pixel) in image.pixels().enumerate() {
                        if pixel[0] != 0 || pixel[1] != 128 || pixel[2] != 128 {
                            mark(index);
                        }
                    }
                }
                crate::raster::DynamicImage::ImageRgba8(image)
                    if matches!(mode, "RGBA" | "RGBa" | "RGBX") =>
                {
                    for (index, pixel) in image.pixels().enumerate() {
                        if pixel.0.iter().any(|value| *value != 0) {
                            mark(index);
                        }
                    }
                }
                crate::raster::DynamicImage::ImageRgba8(image) if mode == "CMYK" => {
                    (h_proj, v_proj) = Self::cmyk_projection(image);
                }
                _ => match mode {
                    "L" | "1" | "P" => {
                        for (index, pixel) in img.to_luma8().pixels().enumerate() {
                            if pixel[0] != 0 {
                                mark(index);
                            }
                        }
                    }
                    "LA" | "PA" => {
                        for (index, pixel) in img.to_luma_alpha8().pixels().enumerate() {
                            if pixel[0] != 0 || pixel[1] != 0 {
                                mark(index);
                            }
                        }
                    }
                    "RGB" | "HSV" | "YCbCr" => {
                        for (index, pixel) in img.to_rgb8().pixels().enumerate() {
                            if pixel.0.iter().any(|value| *value != 0) {
                                mark(index);
                            }
                        }
                    }
                    _ => {
                        for (index, pixel) in img.to_rgba8().pixels().enumerate() {
                            if pixel.0.iter().any(|value| *value != 0) {
                                mark(index);
                            }
                        }
                    }
                },
            }
        }
        Ok((h_proj, v_proj))
    }
}

#[cfg(test)]
#[path = "projection/getprojection_cmyk_tests.rs"]
mod getprojection_cmyk_tests;

#[cfg(test)]
#[path = "projection/getprojection_rgb_pipeline_tests.rs"]
mod getprojection_rgb_pipeline_tests;
