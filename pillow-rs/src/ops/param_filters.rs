//! Parameterized Pillow image filters.
//!
//! Most methods return lazy pipeline operations. Filters that need multiple
//! passes or mode-specific CPU behavior may materialize immediately.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::{PipelineOp, PixelMode};
use crate::raster::DynamicImage;

/// Find the mode (most common value) and its count from a histogram.
/// Uses PIL's strict `>` tie-breaking (lower value wins on tie).
/// Starts with pixel 0 as initial mode, scans 1..255.
fn find_mode_with_count(hist: &[u32; 256]) -> (u8, u32) {
    let mut mode = 0u8;
    let mut max_count = hist[0];
    for (v, &count) in hist.iter().enumerate().skip(1) {
        if count > max_count {
            max_count = count;
            mode = v as u8;
        }
    }
    (mode, max_count)
}

/// Formats the Pillow-compatible representation of a Color3DLUT filter.
pub fn color3dlut_repr(
    table_type: &str,
    size: (u32, u32, u32),
    channels: u32,
    target_mode: Option<&str>,
) -> String {
    let target = target_mode
        .map(|mode| format!(" target_mode={mode}"))
        .unwrap_or_default();
    format!(
        "<Color3DLUT from {table_type} size={}x{}x{} channels={channels}{target}>",
        size.0, size.1, size.2
    )
}

/// Host-neutral table input accepted by `ImageFilter.Color3DLUT`.
pub enum Color3DLutTable {
    /// A flat sequence of channel values.
    Flat(Vec<f64>),
    /// One channel-valued sequence per table entry.
    Nested(Vec<Vec<f64>>),
}

/// A table whose dimensions and channel count have passed the public
/// `Color3DLUT` constructor checks.
#[derive(Debug, Clone)]
pub struct PreparedColor3DLut {
    /// The three-dimensional table dimensions.
    pub size: (u32, u32, u32),
    /// Flattened table values in Pillow traversal order.
    pub table: Vec<f64>,
    /// Number of output channels stored at each table entry.
    pub channels: u32,
}

fn validate_color3dlut_size_tuple(size: (u32, u32, u32)) -> Result<(), PilError> {
    if !(2..=65).contains(&size.0) || !(2..=65).contains(&size.1) || !(2..=65).contains(&size.2) {
        return Err(PilError::ValueError(
            "Size should be in [2, 65] range.".into(),
        ));
    }
    Ok(())
}

fn validate_color3dlut_channels(channels: u32) -> Result<(), PilError> {
    if channels != 3 && channels != 4 {
        return Err(PilError::ValueError(
            "Only 3 or 4 output channels are supported".into(),
        ));
    }
    Ok(())
}

fn color3dlut_expected_len(size: (u32, u32, u32), channels: u32) -> usize {
    size.0 as usize * size.1 as usize * size.2 as usize * channels as usize
}

fn color3dlut_table_length_error(
    size: (u32, u32, u32),
    channels: u32,
    actual_len: usize,
) -> PilError {
    PilError::ValueError(format!(
        "The table should have either channels * size**3 float items or size**3 items of channels-sized tuples with floats. Table should be: {}x{}x{}x{}. Actual length: {}",
        channels, size.0, size.1, size.2, actual_len
    ))
}

/// Validates the odd square window required by Pillow's rank-family filters.
///
/// The Python `MaxFilter`, `MinFilter`, `MedianFilter`, and `RankFilter`
/// constructors retain the caller's size.  Their native filter call rejects
/// zero and even sizes; silently rounding those values changes the operation
/// instead of reporting the same public error.
fn validate_rank_filter_size(size: u32) -> Result<(), PilError> {
    if size == 0 || size % 2 == 0 || size.checked_mul(size).is_none() {
        return Err(PilError::ValueError("bad filter size".into()));
    }
    Ok(())
}

fn gaussian_box_radius(sigma: f32) -> f32 {
    let sigma = sigma.abs();
    if sigma == 0.0 {
        return 0.0;
    }
    let passes = 3.0f64;
    let sigma2 = f64::from(sigma) * f64::from(sigma) / passes;
    let l_val = ((12.0 * sigma2 + 1.0).sqrt() - 1.0) / 2.0;
    let l = l_val.floor();
    let l1 = l + 1.0;
    let a_num = (2.0 * l + 1.0) * (l * l1 - 3.0 * sigma2);
    let a_den = 6.0 * (sigma2 - l1 * l1);
    (l + a_num / a_den) as f32
}

/// Apply the slice-assignment semantics used by Pillow's Python callbacks.
///
/// Pillow starts with a zero-filled table and assigns each callback result to
/// a fixed-width slice. A result with the wrong length can therefore resize
/// the list, including the append behavior when a shortened list moves the
/// next slice beyond its end. Preserve that behavior before the constructor
/// performs the final exact-length validation.
fn color3dlut_assign_callback_values(
    table: &mut Vec<f64>,
    index: usize,
    width: usize,
    values: Vec<f64>,
) {
    let start = index.min(table.len());
    let end = index.saturating_add(width).min(table.len());
    table.splice(start..end, values);
}

/// Validates and normalizes the user-facing Color3DLUT size argument.
pub fn color3dlut_check_size(values: &[f64]) -> Result<(u32, u32, u32), PilError> {
    let dimensions = match values {
        [value] => [*value as i32; 3],
        [s1, s2, s3] => [*s1 as i32, *s2 as i32, *s3 as i32],
        _ => {
            return Err(PilError::ValueError(
                "Size should be either an integer or a tuple of three integers.".into(),
            ));
        }
    };
    let size = (
        dimensions[0] as u32,
        dimensions[1] as u32,
        dimensions[2] as u32,
    );
    validate_color3dlut_size_tuple(size)?;
    Ok(size)
}

/// Validates and flattens a Color3DLUT table.
pub fn color3dlut_prepare_table(
    table: Color3DLutTable,
    size: (u32, u32, u32),
    channels: u32,
) -> Result<Vec<f64>, PilError> {
    validate_color3dlut_size_tuple(size)?;
    validate_color3dlut_channels(channels)?;
    let expected_len = color3dlut_expected_len(size, channels);
    let table = match table {
        Color3DLutTable::Flat(table) => table,
        Color3DLutTable::Nested(table) => {
            let mut flat = Vec::with_capacity(expected_len);
            for pixel in table {
                if pixel.len() != channels as usize {
                    return Err(PilError::ValueError(format!(
                        "The elements of the table should have a length of {}.",
                        channels
                    )));
                }
                flat.extend(pixel);
            }
            flat
        }
    };
    if table.is_empty() || table.len() != expected_len {
        return Err(color3dlut_table_length_error(size, channels, table.len()));
    }
    Ok(table)
}

/// Validates a flat table once at the Rust public boundary.
pub fn prepare_color3dlut(
    table: Vec<f64>,
    size: (u32, u32, u32),
    channels: u32,
) -> Result<PreparedColor3DLut, PilError> {
    let table = color3dlut_prepare_table(Color3DLutTable::Flat(table), size, channels)?;
    Ok(PreparedColor3DLut {
        size,
        table,
        channels,
    })
}

/// Generates a Color3DLUT table while keeping traversal and callback result
/// policy in the runtime-independent core.
pub fn color3dlut_generate_table<E, F, M>(
    size: (u32, u32, u32),
    channels: u32,
    mut callback: F,
    map_error: M,
) -> Result<Vec<f64>, E>
where
    F: FnMut(&[f64]) -> Result<Vec<f64>, E>,
    M: Fn(PilError) -> E,
{
    validate_color3dlut_size_tuple(size).map_err(|error| map_error(error))?;
    validate_color3dlut_channels(channels).map_err(|error| map_error(error))?;

    let (s1, s2, s3) = size;
    let channel_count = channels as usize;
    let mut table = vec![0.0; color3dlut_expected_len(size, channels)];
    let mut index = 0;
    for b in 0..s3 {
        for g in 0..s2 {
            for r in 0..s1 {
                let args = [
                    r as f64 / (s1 - 1) as f64,
                    g as f64 / (s2 - 1) as f64,
                    b as f64 / (s3 - 1) as f64,
                ];
                let values = callback(&args)?;
                color3dlut_assign_callback_values(&mut table, index, channel_count, values);
                index += channel_count;
            }
        }
    }
    Ok(table)
}

/// Transforms a Color3DLUT table while keeping traversal and callback result
/// validation in the runtime-independent core.
pub fn color3dlut_transform_table<E, F, M>(
    input: &PreparedColor3DLut,
    channels_out: Option<u32>,
    with_normals: bool,
    mut callback: F,
    map_error: M,
) -> Result<(Vec<f64>, u32), E>
where
    F: FnMut(&[f64]) -> Result<Vec<f64>, E>,
    M: Fn(PilError) -> E,
{
    let size = input.size;
    let channels_in = input.channels;
    let table = &input.table;
    let channels_out = channels_out.unwrap_or(channels_in);
    validate_color3dlut_channels(channels_out).map_err(|error| map_error(error))?;

    let (s1, s2, s3) = size;
    let input_channels = channels_in as usize;
    let output_channels = channels_out as usize;
    let mut output = vec![0.0; color3dlut_expected_len(size, channels_out)];
    let mut index_in = 0usize;
    let mut index_out = 0usize;
    let mut output_layout_unchanged = true;
    for b in 0..s3 {
        for g in 0..s2 {
            for r in 0..s1 {
                let values = &table[index_in..index_in + input_channels];
                let new_values = if with_normals {
                    // Valid LUTs have three or four channels. Keep callback
                    // arguments on the stack instead of allocating a Vec for
                    // every entry; retain a dynamic fallback for manually
                    // constructed PreparedColor3DLut values with more bands.
                    if input_channels <= 4 {
                        let mut args = [0.0; 7];
                        args[0] = r as f64 / (s1 - 1) as f64;
                        args[1] = g as f64 / (s2 - 1) as f64;
                        args[2] = b as f64 / (s3 - 1) as f64;
                        args[3..3 + input_channels].copy_from_slice(values);
                        callback(&args[..3 + input_channels])?
                    } else {
                        let mut args = Vec::with_capacity(input_channels + 3);
                        args.extend([
                            r as f64 / (s1 - 1) as f64,
                            g as f64 / (s2 - 1) as f64,
                            b as f64 / (s3 - 1) as f64,
                        ]);
                        args.extend_from_slice(values);
                        callback(&args)?
                    }
                } else {
                    callback(values)?
                };

                // Most callbacks return exactly the requested channel count.
                // Write that common case in place; use the original Vec splice
                // semantics for an incorrectly sized result so Pillow's later
                // constructor validation still sees the same resized table.
                let output_end = index_out.checked_add(output_channels).filter(|end| {
                    output_layout_unchanged
                        && new_values.len() == output_channels
                        && *end <= output.len()
                });
                if let Some(output_end) = output_end {
                    output[index_out..output_end].copy_from_slice(&new_values);
                } else {
                    output_layout_unchanged = false;
                    color3dlut_assign_callback_values(
                        &mut output,
                        index_out,
                        output_channels,
                        new_values,
                    );
                }
                index_in += input_channels;
                index_out += output_channels;
            }
        }
    }
    Ok((output, channels_out))
}

impl Image {
    /// Applies Gaussian blur with the given radius.
    ///
    /// Larger radius produces more blur.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(Image)`; deferred pipeline execution reports later
    /// materialization failures.
    pub fn gaussian_blur(&self, radius: f32) -> Result<Image, PilError> {
        self.validate_filter("GaussianBlur")?;
        Ok(Image::push_op(
            self,
            PipelineOp::GaussianBlur { sigma: radius },
        ))
    }

    /// Applies Gaussian blur with independent horizontal and vertical radii.
    pub fn gaussian_blur_xy(&self, radius_x: f32, radius_y: f32) -> Result<Image, PilError> {
        self.validate_filter("GaussianBlur")?;
        if radius_x == radius_y {
            return self.gaussian_blur(radius_x);
        }
        Ok(Image::push_op(
            self,
            PipelineOp::BoxBlurXY {
                radius_x: gaussian_box_radius(radius_x),
                radius_y: gaussian_box_radius(radius_y),
                passes: 3,
            },
        ))
    }

    /// Applies box blur with a uniform kernel radius.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(Image)`; deferred pipeline execution reports later
    /// materialization failures.
    pub fn box_blur(&self, radius: f32) -> Result<Image, PilError> {
        self.validate_filter("BoxBlur")?;
        self.box_blur_xy(radius, radius)
    }

    /// Applies box blur with independent horizontal and vertical radii.
    pub fn box_blur_xy(&self, radius_x: f32, radius_y: f32) -> Result<Image, PilError> {
        self.validate_filter("BoxBlur")?;
        if !radius_x.is_finite() || !radius_y.is_finite() || radius_x < 0.0 || radius_y < 0.0 {
            return Err(PilError::ValueError("radius must be >= 0".into()));
        }

        // Keep the long-standing uniform integer descriptor on the existing
        // backend path. Fractional and anisotropic forms need the exact
        // two-radius CPU implementation below.
        if radius_x == radius_y && radius_x.fract() == 0.0 && radius_x <= u32::MAX as f32 {
            return Ok(Image::push_op(
                self,
                PipelineOp::BoxBlur {
                    radius: radius_x as u32,
                },
            ));
        }
        Ok(Image::push_op(
            self,
            PipelineOp::BoxBlurXY {
                radius_x,
                radius_y,
                passes: 1,
            },
        ))
    }

    /// PIL-compatible clip8: clamp to [0, 255].
    /// Matches PIL's clip8(): `return ss <= 0.0 ? 0 : ss >= 255.0 ? 255 : (UINT8)ss`
    fn pil_clip8(v: i32) -> u8 {
        if v >= 255 {
            255
        } else if v <= 0 {
            0
        } else {
            v as u8
        }
    }

    /// Pillow's truncating `diff * 150 / 100` for byte samples.
    ///
    /// Here `diff` is bounded to [-255, 255]. Correct the arithmetic shift's
    /// negative rounding so this remains signed division truncated toward zero.
    #[inline(always)]
    fn pil_unsharp_adjustment_150(diff: i32) -> i32 {
        let tripled = diff * 3;
        (tripled + i32::from(tripled < 0)) >> 1
    }

    /// Applies Pillow-style unsharp masking.
    ///
    /// `radius` controls blur amount, `percent` controls strength (150 = 150%),
    /// `threshold` is minimum difference to apply.
    /// Uses PIL-style GaussianBlur for the blurred version.
    /// Handles any number of channels (L=1, LA=2, RGB=3, RGBA=4).
    /// Uses PIL's exact integer arithmetic: `clip8(original + diff * percent / 100)`.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when materialization, blur execution, allocation
    /// checks, or raw image reconstruction fails.
    pub fn unsharp_mask(
        &self,
        radius: f32,
        percent: i32,
        threshold: i32,
    ) -> Result<Image, PilError> {
        self.validate_filter("UnsharpMask")?;
        let mode = self.mode()?;
        let simd_blend_selected = match self.backend() {
            Some(backend) => backend == crate::compute::Backend::Simd,
            None => {
                crate::compute::active_backends()?.first().copied()
                    == Some(crate::compute::Backend::Simd)
            }
        };
        // The source remains shared; the blurred result is the output buffer,
        // so reuse its uniquely owned allocation for the blend.
        let img = self.materialized_shared_for_ops()?;
        let mut blurred =
            Image::push_op(self, PipelineOp::GaussianBlur { sigma: radius }).materialize_owned()?;

        let (w, h) = (img.width(), img.height());
        let channels = img.color().channel_count() as usize;
        let raw = img.as_bytes();
        let blur_bytes = blurred.as_bytes();
        let expected_bytes = CheckedDims::new(w, h, channels as u8)?.total_bytes();

        // Keep the established typed-storage fallback for any representation
        // whose byte count does not match its logical band count.
        if raw.len() != expected_bytes || blur_bytes.len() != expected_bytes {
            let mut out = CheckedDims::new(w, h, channels as u8)?.alloc_buffer();
            for y in 0..h {
                for x in 0..w {
                    let base = (y * w + x) as usize * channels;
                    for c in 0..channels {
                        let p = i32::from(raw[base + c]);
                        let diff = p - i32::from(blur_bytes[base + c]);
                        out[base + c] = if diff.abs() > threshold {
                            Self::pil_clip8(p + diff * percent / 100)
                        } else {
                            p as u8
                        };
                    }
                }
            }
            let result = crate::image_utils::raw_bytes_to_image(w, h, out, channels)?;
            return Ok(Image::from_dynamic(result, None));
        }

        let native_rgb = mode == "RGB"
            && matches!(img.as_ref(), DynamicImage::ImageRgb8(_))
            && matches!(&blurred, DynamicImage::ImageRgb8(_));
        let Some(blur_bytes) = blurred.as_bytes_mut() else {
            return Err(PilError::InternalError(
                "byte-mode UnsharpMask blur has no mutable byte storage".into(),
            ));
        };
        let simd_blended = simd_blend_selected
            && native_rgb
            && percent == 150
            && crate::compute::simd_unsharp_blend_rgb_150(raw, blur_bytes, threshold);
        if simd_blended {
            // The SIMD kernel updated the blur allocation in place.
        } else if percent == 150 {
            // Every stored byte is an independent UnsharpMask sample in the
            // existing byte path. Flattening the common 150% case removes
            // nested pixel/channel indexing and replaces multiply/divide by
            // 100 with Pillow-exact truncating half of `diff * 3`.
            for (output, &p) in blur_bytes.iter_mut().zip(raw) {
                let diff = i32::from(p) - i32::from(*output);
                *output = if diff.abs() > threshold {
                    Self::pil_clip8(i32::from(p) + Self::pil_unsharp_adjustment_150(diff))
                } else {
                    p
                };
            }
        } else {
            for (output, &original) in blur_bytes.iter_mut().zip(raw) {
                let original = i32::from(original);
                let diff = original - i32::from(*output);
                // PIL uses integer arithmetic: diff * percent / 100 (truncating).
                *output = if diff.abs() > threshold {
                    Self::pil_clip8(original + diff * percent / 100)
                } else {
                    original as u8
                };
            }
        }

        Ok(Image::from_dynamic(blurred, Some(mode)))
    }

    /// Applies a maximum filter over an odd neighborhood.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(Image)`; deferred pipeline execution reports later
    /// materialization failures.
    pub fn max_filter(&self, size: u32) -> Result<Image, PilError> {
        self.validate_filter("MaxFilter")?;
        validate_rank_filter_size(size)?;
        Ok(Image::push_op(self, PipelineOp::MaxFilter { size }))
    }

    /// Applies a minimum filter over an odd neighborhood.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(Image)`; deferred pipeline execution reports later
    /// materialization failures.
    pub fn min_filter(&self, size: u32) -> Result<Image, PilError> {
        self.validate_filter("MinFilter")?;
        validate_rank_filter_size(size)?;
        Ok(Image::push_op(self, PipelineOp::MinFilter { size }))
    }

    /// Applies a median filter over an odd neighborhood.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(Image)`; deferred pipeline execution reports later
    /// materialization failures.
    pub fn median_filter(&self, size: u32) -> Result<Image, PilError> {
        self.validate_filter("MedianFilter")?;
        validate_rank_filter_size(size)?;
        Ok(Image::push_op(self, PipelineOp::MedianFilter { size }))
    }

    /// Applies a mode filter over an odd neighborhood.
    ///
    /// Each pixel becomes the most common value in its neighborhood when that
    /// value occurs more than twice; otherwise the original pixel is preserved.
    ///
    /// PIL C behavior:
    ///   - Single-band only at C level; multi-band processed per-channel
    ///   - Strict `>` tie-breaking (lower value wins)
    ///   - If max count ≤ 2, original pixel is preserved unchanged
    ///   - Pixels outside image boundary are SKIPPED (not clamped/replicated)
    ///   - Supports any channel count (1=L, 2=LA, 3=RGB, 4=RGBA)
    ///   - For P-mode (palette): operates on palette indices, preserves palette
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when materialization, allocation checks, or raw image
    /// reconstruction fails.
    pub fn mode_filter(&self, size: u32) -> Result<Image, PilError> {
        self.validate_filter("Mode")?;
        // Pillow's modefilter uses `size / 2` as its radius.  Positive even
        // sizes therefore have the same neighborhood as the next odd size,
        // while zero and negative Python inputs are identity operations.
        // Keep zero distinct from the normalized rank-family filters.
        let size = if size <= 1 { size } else { size | 1 };

        // For palette images: extract palette before materialize
        let palette = self.palette();
        let palette_alpha = self.palette_alpha();
        let explicit = self.explicit_mode().map(|s| s.to_string());

        let img = self.materialize()?;
        let half = (size / 2) as i32;

        let (w_u32, h_u32) = (img.width(), img.height());
        let w = w_u32 as i32;
        let h = h_u32 as i32;
        let channels = img.color().channel_count() as usize;
        let raw = img.as_bytes();

        let mut out = vec![0u8; (w_u32 * h_u32) as usize * channels];

        for y in 0..h {
            for x in 0..w {
                // Per-channel histograms
                let mut hists: Vec<[u32; 256]> = vec![[0u32; 256]; channels];
                for dy in -half..=half {
                    let sy = y + dy;
                    if sy < 0 || sy >= h {
                        continue; // PIL skips out-of-bounds rows
                    }
                    for dx in -half..=half {
                        let sx = x + dx;
                        if sx < 0 || sx >= w {
                            continue; // PIL skips out-of-bounds columns
                        }
                        let base = ((sy * w + sx) as usize) * channels;
                        for c in 0..channels {
                            hists[c][raw[base + c] as usize] += 1;
                        }
                    }
                }
                let out_base = ((y * w + x) as usize) * channels;
                for c in 0..channels {
                    let orig_val = raw[((y * w + x) as usize) * channels + c];
                    let (mode, max_count) = find_mode_with_count(&hists[c]);
                    out[out_base + c] = if max_count > 2 { mode } else { orig_val };
                }
            }
        }
        let result = crate::image_utils::raw_bytes_to_image(w_u32, h_u32, out, channels)?;
        // Preserve palette for P-mode images
        if let Some(pal) = palette {
            // A palette image materializes as one-band indices, so the raw
            // reconstruction above is necessarily Luma8 here. Keeping the
            // invariant typed avoids an unreachable error path while
            // preserving the palette and index buffer exactly.
            let indices = result.into_luma8();
            return Ok(Image::Paletted(crate::image::PalettedData {
                indices,
                palette: pal,
                palette_alpha: palette_alpha.unwrap_or_default(),
                source_format: None,
                info: None,
                exif: None,
                materialized: crate::image::materialization_cache(),
            }));
        }
        // Preserve explicit mode (e.g. "1", "P" via explicit_mode)
        if explicit.is_some() {
            return Ok(Image::from_dynamic(result, explicit));
        }
        Ok(Image::from_dynamic(result, None))
    }

    /// Applies a rank filter over an odd neighborhood.
    ///
    /// Each pixel becomes the `rank`-th value after sorting the neighborhood.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(Image)`; deferred pipeline execution reports later
    /// materialization failures.
    pub fn rank_filter(&self, size: u32, rank: u32) -> Result<Image, PilError> {
        self.validate_filter("RankFilter")?;
        validate_rank_filter_size(size)?;
        let window = size
            .checked_mul(size)
            .ok_or_else(|| PilError::ValueError("bad filter size".into()))?;
        if rank >= window {
            return Err(PilError::ValueError("bad rank value".into()));
        }
        Ok(Image::push_op(self, PipelineOp::RankFilter { size, rank }))
    }

    /// Applies a 3D color lookup table with trilinear interpolation.
    ///
    /// `size` is the LUT grid size, `table` contains the LUT values, and
    /// `channels` is the number of output channels.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(Image)`; LUT validation is handled during pipeline
    /// execution.
    pub fn color3dlut(
        &self,
        input: PreparedColor3DLut,
        target_mode: Option<&str>,
    ) -> Result<Image, PilError> {
        self.validate_filter("Color3DLUT")?;
        let PreparedColor3DLut {
            size,
            table,
            channels,
        } = input;
        let source_name = self.mode()?;
        let source_mode = match source_name.as_str() {
            "RGB" => PixelMode::RGB,
            "RGBA" => PixelMode::RGBA,
            "CMYK" => PixelMode::CMYK,
            _ => return Err(PilError::ValueError("image has wrong mode".into())),
        };
        let target_name = target_mode.unwrap_or(source_name.as_str());
        let target = match target_mode {
            Some("RGB") => PixelMode::RGB,
            Some("RGBA") => PixelMode::RGBA,
            Some("CMYK") => PixelMode::CMYK,
            Some(_) => {
                return Err(PilError::ValueError("unrecognized image mode".into()));
            }
            None => source_mode,
        };
        if target.channels() < channels as usize {
            return Err(PilError::ValueError("image has wrong mode".into()));
        }

        // Color3DLUT stores its output mode in `explicit_mode`; Image::mode
        // therefore returns it directly instead of entering the metadata
        // planner for this operation.
        Ok(Image::push_mode_changing_op(
            self,
            PipelineOp::Color3DLut {
                size,
                table: table.into(),
                channels,
                source_mode,
                target_mode: target,
            },
            target_name,
        ))
    }
}

#[cfg(test)]
mod unsharp_tests {
    use super::Image;

    #[test]
    fn percent_150_adjustment_matches_signed_truncation_for_every_byte_difference() {
        for difference in -255..=255 {
            assert_eq!(
                Image::pil_unsharp_adjustment_150(difference),
                difference * 150 / 100,
                "difference={difference}"
            );
        }
    }
}

#[cfg(test)]
mod color3dlut_transform_tests {
    use super::{PilError, PreparedColor3DLut, color3dlut_transform_table};

    fn fixture() -> PreparedColor3DLut {
        PreparedColor3DLut {
            size: (2, 2, 2),
            table: (0..24).map(f64::from).collect(),
            channels: 3,
        }
    }

    #[test]
    fn exact_width_results_preserve_the_preallocated_output_layout() {
        let input = fixture();
        let (actual, channels) = color3dlut_transform_table(
            &input,
            None,
            false,
            |values| Ok::<_, PilError>(values.to_vec()),
            |error| error,
        )
        .unwrap();

        assert_eq!(channels, 3);
        assert_eq!(actual, input.table);
    }

    #[test]
    fn with_normals_keeps_pillow_cube_traversal_order() {
        let input = fixture();
        let (actual, channels) = color3dlut_transform_table(
            &input,
            None,
            true,
            |args| {
                assert_eq!(args.len(), 6);
                Ok::<_, PilError>(args[..3].to_vec())
            },
            |error| error,
        )
        .unwrap();

        let mut expected = Vec::with_capacity(24);
        for b in 0..2 {
            for g in 0..2 {
                for r in 0..2 {
                    expected.extend([r as f64, g as f64, b as f64]);
                }
            }
        }
        assert_eq!(channels, 3);
        assert_eq!(actual, expected);
    }

    #[test]
    fn wrong_callback_width_retains_pillow_slice_resize_behavior() {
        let input = fixture();
        let (actual, channels) = color3dlut_transform_table(
            &input,
            None,
            false,
            |values| Ok::<_, PilError>(values[..2].to_vec()),
            |error| error,
        )
        .unwrap();

        let mut expected = vec![0.0; 24];
        for entry in 0..8 {
            let start = (entry * 3).min(expected.len());
            let end = (entry * 3 + 3).min(expected.len());
            expected.splice(
                start..end,
                [input.table[entry * 3], input.table[entry * 3 + 1]],
            );
        }
        assert_eq!(channels, 3);
        assert_eq!(actual, expected);
    }
}
