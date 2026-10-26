//! Pillow-compatible horizontal and vertical image projections.

use crate::error::PilError;
use crate::image::PipelineOps;
use crate::pipeline::PipelineOp;
use crate::raster::DynamicImage;
use std::sync::Arc;

use super::Image;

type ProjectionAxes = (Vec<u32>, Vec<u32>);
mod cmyk;

mod putpixel_pipeline;

impl Image {
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
    ) -> Result<ProjectionAxes, PilError> {
        let (w, h) = (img.width() as usize, img.height() as usize);
        let mut h_proj = vec![0u32; w];
        let mut v_proj = vec![0u32; h];

        if matches!(mode, "I" | "F") {
            Self::getprojection_scalar_materialized(img, mode, w, h, &mut h_proj, &mut v_proj)?;
        } else if let Some((horizontal, vertical)) =
            Self::getprojection_pixels_materialized(img, mode, w, &mut h_proj, &mut v_proj)
        {
            h_proj = horizontal;
            v_proj = vertical;
        }
        Ok((h_proj, v_proj))
    }

    fn getprojection_scalar_materialized(
        img: &DynamicImage,
        mode: &str,
        w: usize,
        h: usize,
        h_proj: &mut [u32],
        v_proj: &mut [u32],
    ) -> Result<(), PilError> {
        // Materialization validates scalar storage, so decode each sample
        // from the retained frame a row at a time. Projection only records
        // whether a column and row contain any non-zero word, so direct
        // coordinates avoid division/modulo and repeated row writes. Once
        // every column is marked, each row needs only its first non-zero word.
        let row_bytes = w.checked_mul(std::mem::size_of::<u32>()).ok_or_else(|| {
            PilError::DimensionError("getprojection row byte count overflow".into())
        })?;
        if row_bytes == 0 {
            return Ok(());
        }
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
                        let value =
                            i32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]]);
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
                        // Pillow projects F samples by their stored word:
                        // -0.0's sign bit makes it nonzero.
                        let value =
                            u32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]]);
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
        Ok(())
    }

    fn getprojection_pixels_materialized(
        img: &DynamicImage,
        mode: &str,
        w: usize,
        h_proj: &mut [u32],
        v_proj: &mut [u32],
    ) -> Option<ProjectionAxes> {
        if mode == "CMYK"
            && let crate::raster::DynamicImage::ImageRgba8(image) = img
        {
            return Some(Self::cmyk_projection(image));
        }
        if mode == "LAB"
            && let crate::raster::DynamicImage::ImageRgb8(image) = img
        {
            Self::getprojection_lab_materialized(image, w, h_proj, v_proj);
            return None;
        }
        if matches!(mode, "RGBA" | "RGBa" | "RGBX")
            && let crate::raster::DynamicImage::ImageRgba8(image) = img
        {
            Self::getprojection_rgba_materialized(image, w, h_proj, v_proj);
            return None;
        }
        if w >= 64
            && matches!(mode, "RGB" | "HSV" | "YCbCr")
            && let crate::raster::DynamicImage::ImageRgb8(image) = img
        {
            Self::getprojection_rgb_wide_materialized(image, w, h_proj, v_proj);
            return None;
        }
        let mut mark = |index: usize| {
            let x = index % w;
            let y = index / w;
            h_proj[x] = 1;
            v_proj[y] = 1;
        };
        match img {
            crate::raster::DynamicImage::ImageLuma8(image) if matches!(mode, "L" | "1" | "P") => {
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
                // Keep the existing traversal for narrow images, where row-
                // state setup did not produce a reliable gain.
                for (index, pixel) in image.pixels().enumerate() {
                    if pixel.0.iter().any(|value| *value != 0) {
                        mark(index);
                    }
                }
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
        None
    }

    fn getprojection_lab_materialized(
        image: &crate::raster::RgbImage,
        w: usize,
        h_proj: &mut [u32],
        v_proj: &mut [u32],
    ) {
        // LAB stores logical A/B zero points at byte 128. Scanning through
        // RGBA synthesizes alpha 255; raw zero checks mark neutral LAB pixels.
        for (index, pixel) in image.pixels().enumerate() {
            if pixel[0] != 0 || pixel[1] != 128 || pixel[2] != 128 {
                h_proj[index % w] = 1;
                v_proj[index / w] = 1;
            }
        }
    }

    fn getprojection_rgba_materialized(
        image: &crate::raster::RgbaImage,
        w: usize,
        h_proj: &mut [u32],
        v_proj: &mut [u32],
    ) {
        for (index, pixel) in image.pixels().enumerate() {
            if pixel.0.iter().any(|value| *value != 0) {
                h_proj[index % w] = 1;
                v_proj[index / w] = 1;
            }
        }
    }

    fn getprojection_rgb_wide_materialized(
        image: &crate::raster::RgbImage,
        w: usize,
        h_proj: &mut [u32],
        v_proj: &mut [u32],
    ) {
        // Walk packed rows to avoid a per-pixel index division. Once every
        // horizontal column is set, later rows need only a short nonzero scan.
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
    }
}

#[cfg(test)]
#[path = "projection/getprojection_cmyk_tests.rs"]
mod getprojection_cmyk_tests;

#[cfg(test)]
#[path = "projection/getprojection_rgb_pipeline_tests.rs"]
mod getprojection_rgb_pipeline_tests;
