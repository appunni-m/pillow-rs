//! Pillow `Image` module-level functions.
//!
//! These functions mirror surfaces such as `Image.merge`, `Image.blend`,
//! `Image.composite`, `Image.eval`, and synthetic image effects.

use std::sync::Arc;

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::image::Image;
use crate::ops::convert::parse_mode;
use crate::pipeline::PipelineOp;

/// Host-neutral input classification for the callable form of `Image.eval`.
#[derive(Debug, Clone, Copy)]
pub enum EvalInputKind {
    /// A Python string, which Pillow rejects with its legacy callable error.
    String,
    /// Any non-string value; callable validation remains host-owned.
    Other,
}

/// Validates the host-independent part of `Image.eval` input handling.
pub fn validate_eval_input(kind: EvalInputKind) -> Result<(), PilError> {
    if matches!(kind, EvalInputKind::String) {
        return Err(PilError::TypeError(
            "type str doesn't define __round__ method".into(),
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
enum GradientMode {
    Byte,
    One,
    Integer,
    Float,
}

fn parse_gradient_mode(mode: &str) -> Result<GradientMode, PilError> {
    if mode.len() != 1 {
        return Err(PilError::ValueError("image has wrong mode".into()));
    }
    match mode {
        "L" | "P" => Ok(GradientMode::Byte),
        "1" => Ok(GradientMode::One),
        "I" => Ok(GradientMode::Integer),
        "F" => Ok(GradientMode::Float),
        _ => Err(PilError::ValueError("image has wrong mode".into())),
    }
}

/// Creates an image from raw bytes using Pillow's supported raw decoder.
///
/// Decoder selection is part of the public operation contract, not a binding
/// concern. Host adapters pass the decoder name through after extracting it.
pub fn frombytes(
    mode: &str,
    size: (u32, u32),
    data: &[u8],
    decoder_name: &str,
) -> Result<Image, PilError> {
    if decoder_name != "raw" {
        return Err(PilError::IOError(format!(
            "decoder {decoder_name} not available"
        )));
    }
    Image::frombytes(mode, size, data)
}

/// Composites `im2` over `im1` and returns a new image.
pub fn alpha_composite(im1: &Image, im2: &Image) -> Result<Image, PilError> {
    let mut result = im1.copy();
    result.alpha_composite(im2, (0, 0), (0, 0))?;
    Ok(result)
}

fn validate_merge_shape(mode: &str, band_count: usize) -> Result<(), PilError> {
    let n_expected = match mode {
        "RGB" => 3,
        "RGBA" => 4,
        "CMYK" => 4,
        "LA" => 2,
        "L" => 1,
        _ => {
            // Pillow 12.2 looks up the mode in ``ImageMode.getmode`` before
            // validating the band sequence, so an unknown mode is surfaced as
            // ``KeyError(mode)`` rather than a generic value error.
            return Err(PilError::KeyError(mode.to_owned()));
        }
    };

    if band_count != n_expected {
        return Err(PilError::ValueError("wrong number of bands".into()));
    }
    Ok(())
}

fn validate_merge_band_modes(mode: &str, bands: &[Image]) -> Result<(), PilError> {
    // Pillow's ``Image.merge`` accepts an L image for every band.  For
    // multi-band outputs it also accepts a P image only in the first
    // position, where the indexed core is treated as a single-byte band;
    // later P bands fail in ``ImagingMerge`` with ``mode mismatch``.  The
    // single-band L path has its own ``images do not match`` error.
    for (index, band) in bands.iter().enumerate() {
        let band_mode = band.mode()?;
        let valid = band_mode == "L" || (index == 0 && mode != "L" && band_mode == "P");
        if !valid {
            return Err(if mode == "L" {
                PilError::ValueError("images do not match".into())
            } else {
                PilError::ValueError("mode mismatch".into())
            });
        }
    }
    Ok(())
}

fn validate_merge_band_sizes(bands: &[Image]) -> Result<(), PilError> {
    let expected = bands[0].size()?;
    for band in bands.iter().skip(1) {
        // Pillow's ImagingMerge rejects mismatched dimensions at the public
        // call boundary. Keep this validation eager so a lazy pipeline cannot
        // defer the ValueError into result serialization.
        if band.size()? != expected {
            return Err(PilError::ValueError("size mismatch".into()));
        }
    }
    Ok(())
}

/// Host-neutral input classification for `Image.merge` bands.
#[derive(Debug, Clone)]
pub enum MergeInput {
    /// A Rust image extracted by a binding.
    Image(Image),
    /// A non-image host value, retaining only its type name for the
    /// Pillow-compatible attribute error raised by the merge implementation.
    Invalid(String),
}

/// Merges single-band images into a multi-band image.
///
/// `mode` determines the required band count: `L=1`, `LA=2`, `RGB=3`, and
/// `RGBA=4`.
///
/// # Errors
///
/// Returns [`PilError::ValueError`] when `mode` is unsupported or `bands` has
/// the wrong length.
pub fn merge(mode: &str, bands: &[Image]) -> Result<Image, PilError> {
    validate_merge_shape(mode, bands.len())?;
    validate_merge_band_modes(mode, bands)?;
    validate_merge_band_sizes(bands)?;

    let mode_enum = parse_mode(mode)?;
    let mut result = Image::push_op(
        &bands[0],
        PipelineOp::Merge {
            mode: mode_enum,
            bands: bands.to_vec().into(),
        },
    );
    if let Image::Pipeline {
        explicit_mode: tag, ..
    } = &mut result
    {
        if mode == "CMYK" {
            *tag = Some("CMYK".to_string());
        }
    }
    Ok(result)
}

/// Merges host-extracted band inputs while keeping invalid-item handling in
/// the core contract.
///
/// Binding layers may only classify a host value as an image or retain its
/// type name. They must not decide whether a missing item means a mode error,
/// an arity error, or an invalid band. Preserve that ordering here and let the
/// existing mode/arity validation remain the single source of truth.
pub fn merge_inputs(mode: &str, bands: &[MergeInput]) -> Result<Image, PilError> {
    validate_merge_shape(mode, bands.len())?;
    let mut images = Vec::with_capacity(bands.len());
    for band in bands {
        match band {
            MergeInput::Image(image) => images.push(image.clone()),
            MergeInput::Invalid(type_name) => {
                return Err(PilError::AttributeError(format!(
                    "'{type_name}' object has no attribute 'load'"
                )));
            }
        }
    }
    merge(mode, &images)
}

/// Blends two same-sized images by linear interpolation.
///
/// `alpha` is clamped to `0.0..=1.0`; output is `(1 - alpha) * image1 +
/// alpha * image2`.
///
/// # Errors
///
/// Returns [`PilError::ValueError`] when modes or image dimensions differ, or
/// another [`PilError`] when metadata lookup fails.
pub fn blend(image1: &Image, image2: &Image, alpha: f64) -> Result<Image, PilError> {
    let mode1 = image1.mode()?;
    let mode2 = image2.mode()?;
    let compatible = match mode1.as_str() {
        "L" => mode2 == "L",
        "LA" => mode2 == "LA",
        "RGB" | "HSV" | "YCbCr" => matches!(mode2.as_str(), "RGB" | "HSV" | "YCbCr"),
        "RGBA" | "CMYK" | "RGBa" | "RGBX" => {
            matches!(mode2.as_str(), "RGBA" | "CMYK" | "RGBa" | "RGBX")
        }
        _ => false,
    };
    if !compatible {
        // Pillow reports unsupported first-image modes (and indexed/1-bit
        // second images) as ``image has wrong mode``. Other type-family
        // mismatches are reported as ``images do not match``.
        let wrong_mode = !matches!(
            mode1.as_str(),
            "L" | "LA" | "RGB" | "HSV" | "YCbCr" | "RGBA" | "CMYK" | "RGBa" | "RGBX"
        ) || matches!(mode2.as_str(), "1" | "P" | "PA");
        return Err(PilError::ValueError(
            if wrong_mode {
                "image has wrong mode"
            } else {
                "images do not match"
            }
            .into(),
        ));
    }
    let (w1, h1) = image1.size()?;
    let (w2, h2) = image2.size()?;
    if (w1, h1) != (w2, h2) {
        return Err(PilError::ValueError("images do not match".into()));
    }
    Ok(Image::push_op(
        image1,
        PipelineOp::BlendModule {
            other: Arc::new(image2.clone()),
            alpha,
        },
    ))
}

/// Composites `image1` over `image2` using `mask`.
///
/// # Errors
///
/// Returns [`PilError::ValueError`] when `image1` and `mask` dimensions differ,
/// or another [`PilError`] when size lookup fails.
pub fn composite(image1: &Image, image2: &Image, mask: &Image) -> Result<Image, PilError> {
    // PIL.composite = image2.copy() followed by image2.paste(image1, None, mask)
    // The output matches image2's size; smaller images are pasted at (0,0).
    // PIL requires image1 and mask to have the same size (via paste).
    // image2 can be a different size (it's the background canvas).
    let (w1, h1) = image1.size()?;
    let (wm, hm) = mask.size()?;
    if (w1, h1) != (wm, hm) {
        return Err(PilError::ValueError("images do not match".into()));
    }
    let mask_alpha = match mask.mode()?.as_str() {
        "1" | "L" => false,
        "LA" | "RGBA" | "RGBa" => true,
        _ => return Err(PilError::ValueError("bad transparency mask".into())),
    };
    let output_mode = image2.mode()?;
    let source = if image1.mode()? == output_mode {
        image1.clone()
    } else {
        // Pillow 12.2 PIL.Image.composite starts from image2.copy(), then
        // Image.paste converts image1 to the destination mode before blending.
        image1.convert(&output_mode, None, None, None, None)?
    };
    let mut result = Image::push_op(
        &source,
        PipelineOp::CompositeModule {
            other: Arc::new(image2.clone()),
            mask: Arc::new(mask.clone()),
            mask_alpha,
        },
    );
    if let Image::Pipeline {
        ref mut explicit_mode,
        ref mut palette,
        ref mut palette_alpha,
        ..
    } = result
    {
        *explicit_mode = image2.explicit_mode().map(str::to_owned);
        if output_mode == "P" {
            // The output begins as image2.copy(), so palette ownership follows
            // image2 even when image1 was converted to P for the paste.
            *palette = Some(image2.extract_palette().unwrap_or_default());
            *palette_alpha = image2.palette_alpha();
        } else {
            *palette = None;
            *palette_alpha = None;
        }
    }
    Ok(result)
}

/// Applies a lookup table to each pixel.
///
/// `lut` is copied into the lazy operation and interpreted by the point/eval
/// backend according to the image mode.
///
/// # Errors
///
/// Currently returns `Ok(Image)`; LUT length validation is handled by pipeline
/// execution.
pub fn eval(image: &Image, lut: &[u8]) -> Result<Image, PilError> {
    Ok(Image::push_op(
        image,
        PipelineOp::Eval {
            lut: lut.to_vec().into(),
        },
    ))
}

/// Applies a lookup table, expanding a single-band table across bands.
///
/// A 256-entry `lut` is replicated `n_bands` times. A table already sized to
/// `256 * n_bands` is used as-is.
///
/// # Errors
///
/// Returns [`PilError::ValueError`] when `n_bands` is outside `1..=4` or `lut`
/// has neither 256 nor `256 * n_bands` entries.
pub fn eval_replicated(image: &Image, lut: &[u8], n_bands: usize) -> Result<Image, PilError> {
    if n_bands == 0 || n_bands > 4 {
        return Err(PilError::ValueError("invalid band count".into()));
    }
    let expected = 256 * n_bands;
    if lut.len() == expected {
        return eval(image, lut);
    }
    if lut.len() != 256 {
        return Err(PilError::ValueError(format!(
            "wrong number of lut entries: expected {} or {} got {}",
            256,
            expected,
            lut.len()
        )));
    }
    let mut replicated = Vec::with_capacity(expected);
    for _ in 0..n_bands {
        replicated.extend_from_slice(lut);
    }
    eval(image, &replicated)
}

/// Applies a single-band lookup table to every band in `image`.
///
/// The image mode determines the band count. Bindings should use this entry
/// point instead of deriving semantic image information in the host language.
///
/// # Errors
///
/// Returns an image/materialization error when the band count cannot be read,
/// or [`PilError::ValueError`] when `lut` is not a valid single-band or
/// already-expanded table.
pub fn eval_replicated_for_image(image: &Image, lut: &[u8]) -> Result<Image, PilError> {
    let n_bands = image.getbands()?.len();
    eval_replicated(image, lut, n_bands)
}

/// Builds and applies a callable lookup table using the image's band count.
///
/// Keeping band discovery and LUT replication here means bindings only adapt
/// their host callback to the core callback contract. The callable path uses
/// the same expanded-table representation as Pillow's multiband point path.
pub fn eval_callable<F>(image: &Image, callback: F) -> Result<Image, PilError>
where
    F: FnMut(u32) -> Result<i32, PilError>,
{
    // Keep the callback table single-band here. The shared image-aware path
    // performs band discovery and replication, preserving the established
    // point/eval validation and materialization order.
    let lut = make_lut(1, callback)?;
    eval_replicated_for_image(image, &lut)
}

/// Applies Pillow's affine callable transform for scalar images.
///
/// Pillow does not build a byte LUT for callable `Image.point` calls on
/// `I`, `I;16*`, or `F` images.  It probes the callable with an affine
/// transform object, then applies the resulting `value * scale + offset`
/// expression in the image's native scalar domain.  Keeping this path eager
/// is intentional: Pillow loads the source before constructing the new image,
/// and these native scalar buffers are not represented by the byte-oriented
/// `Eval` pipeline operation.
pub fn eval_point_transform(image: &Image, scale: f64, offset: f64) -> Result<Image, PilError> {
    let mode = image.mode()?;
    if !matches!(
        mode.as_str(),
        "I" | "I;16" | "I;16L" | "I;16B" | "I;16N" | "F"
    ) {
        return Err(PilError::ValueError(
            "point operation not supported for this mode".to_owned(),
        ));
    }

    let mut result = image.materialize()?;
    match mode.as_str() {
        "I" => {
            let pixels = result.as_mut_rgba8().ok_or_else(|| {
                PilError::InternalError("I image has no four-byte scalar storage".to_owned())
            })?;
            for pixel in pixels.pixels_mut() {
                let value = i32::from_le_bytes(pixel.0);
                let transformed = (f64::from(value) * scale + offset) as i32;
                pixel.0 = transformed.to_le_bytes();
            }
        }
        "F" => {
            let pixels = result.as_mut_rgba8().ok_or_else(|| {
                PilError::InternalError("F image has no four-byte scalar storage".to_owned())
            })?;
            for pixel in pixels.pixels_mut() {
                let value = f32::from_le_bytes(pixel.0);
                let transformed = (f64::from(value) * scale + offset) as f32;
                pixel.0 = transformed.to_le_bytes();
            }
        }
        "I;16" | "I;16L" | "I;16B" | "I;16N" => {
            let pixels = result.as_mut_luma16().ok_or_else(|| {
                PilError::InternalError("I;16 image has no 16-bit scalar storage".to_owned())
            })?;
            for pixel in pixels.pixels_mut() {
                pixel.0[0] = (f64::from(pixel.0[0]) * scale + offset) as u16;
            }
        }
        _ => unreachable!("scalar point transform mode was validated above"),
    }

    Ok(Image::from_dynamic(result, Some(mode)))
}

/// Validates and applies a pre-expanded Pillow lookup table.
///
/// Pillow requires exactly 256 entries per image band for a non-callable LUT.
/// Keeping this validation in core ensures every ABI observes the same mode
/// semantics and error ordering.
///
/// # Errors
///
/// Returns an image/materialization error when the band count cannot be read,
/// or [`PilError::ValueError`] when the table length does not equal
/// `256 * image_band_count`.
pub fn eval_validated(image: &Image, lut: &[u8]) -> Result<Image, PilError> {
    let mode = image.mode()?;
    if matches!(
        mode.as_str(),
        "I" | "I;16" | "I;16L" | "I;16B" | "I;16N" | "F"
    ) {
        // Pillow routes callable transforms for scalar images through the
        // affine path, but rejects a byte LUT before entering the Imaging
        // point kernel. Keep that public error boundary here so a scalar
        // image cannot reach the byte-oriented executor with a short table.
        return Err(PilError::ValueError(
            "point operation not supported for this mode".to_owned(),
        ));
    }
    let n_bands = image.getbands()?.len();
    let expected = 256 * n_bands;
    if lut.len() != expected {
        return Err(PilError::ValueError("wrong number of lut entries".into()));
    }
    eval(image, lut)
}

/// Applies a floating-point LUT to Pillow's byte-valued single-band modes.
///
/// Pillow's ``Image.point(lut, "F")`` path is a public mode conversion for
/// ``1``, ``L``, and ``P`` images. It deliberately preserves fractional LUT
/// values instead of applying the byte-path rounding step. The existing
/// ``Eval`` pipeline stores byte samples, so this operation materializes an
/// explicit ``F`` image with native little-endian ``f32`` samples.
///
/// # Errors
///
/// Returns [`PilError::ValueError`] when the LUT length or source mode does
/// not match Pillow's single-band floating-output path.
pub fn eval_float(image: &Image, lut: &[f64]) -> Result<Image, PilError> {
    let mode = image.mode()?;
    let n_bands = image.getbands()?.len();
    if lut.len() != 256 * n_bands {
        return Err(PilError::ValueError("wrong number of lut entries".into()));
    }
    if !matches!(mode.as_str(), "1" | "L" | "P") {
        return Err(PilError::ValueError(
            "point operation not supported for this mode".to_owned(),
        ));
    }

    let size = image.size()?;
    let pixels = image.materialize()?.to_luma8().into_raw();
    let mut raw = Vec::with_capacity(pixels.len() * std::mem::size_of::<f32>());
    for sample in pixels {
        raw.extend_from_slice(&(lut[usize::from(sample)] as f32).to_le_bytes());
    }
    Image::frombytes("F", size, &raw)
}

/// Builds a Pillow point/eval lookup table from a host callback.
///
/// The callback is only responsible for producing one integer result for each
/// input sample. Clamping the result to Pillow's byte range and replicating the
/// table for multiband images remain core behavior so bindings do not duplicate
/// the algorithm.
pub fn make_lut<F>(n_bands: u32, mut callback: F) -> Result<Vec<u8>, PilError>
where
    F: FnMut(u32) -> Result<i32, PilError>,
{
    let mut table = Vec::with_capacity(256);
    for sample in 0..256u32 {
        let value = callback(sample)?;
        table.push(value.clamp(0, 255) as u8);
    }
    if n_bands > 1 {
        table = table.repeat(n_bands as usize);
    }
    Ok(table)
}

/// Generates Gaussian noise using the source image dimensions.
///
/// # Errors
///
/// Currently returns `Ok(Image)`; deferred pipeline execution reports later
/// materialization failures.
pub fn effect_noise(image: &Image, sigma: f64) -> Result<Image, PilError> {
    Ok(Image::push_op(image, PipelineOp::EffectNoise { sigma }))
}

/// Generates a Gaussian-noise `L` image for Pillow's module-level effect.
pub fn effect_noise_from_size(size: (u32, u32), sigma: f64) -> Result<Image, PilError> {
    let source = Image::new(size.0, size.1, "L", (0, 0, 0, 255))?;
    effect_noise(&source, sigma)
}

/// Spreads pixels outward by up to `distance` pixels.
///
/// # Errors
///
/// Currently returns `Ok(Image)`; deferred pipeline execution reports later
/// materialization failures.
pub fn effect_spread(image: &Image, distance: u32) -> Result<Image, PilError> {
    Ok(Image::push_op(image, PipelineOp::EffectSpread { distance }))
}

/// Generates a 256 by 256 linear gradient.
///
/// Single-channel modes increase from black at the top to white at the bottom.
/// Supported modes are `"1"`, `"L"`, `"P"`, `"I"`, and `"F"`.
///
/// # Errors
///
/// Returns [`PilError::ValueError`] when `mode` is unsupported, or another
/// [`PilError`] when raw image construction fails.
pub fn linear_gradient(mode: &str) -> Result<Image, PilError> {
    let gradient_mode = parse_gradient_mode(mode)?;
    if let Some(image) = crate::compute::try_simd_linear_gradient(mode)? {
        return Ok(Image::from_generated_dynamic(image, mode));
    }
    let (bytes_per_pixel, row_bytes) = match gradient_mode {
        GradientMode::Byte => (1, 256),
        GradientMode::One => (1, 256usize.div_ceil(8)),
        GradientMode::Integer | GradientMode::Float => (4, 256),
    };
    let size: usize = row_bytes * 256 * bytes_per_pixel;
    let mut data = vec![0u8; size];

    for y in 0..256usize {
        let row_start = y * row_bytes * bytes_per_pixel;
        match gradient_mode {
            GradientMode::Byte => {
                let val = y as u8;
                data[row_start..row_start + 256].fill(val);
            }
            GradientMode::One => {
                // Pillow's mode-1 gradient is a binary threshold, not an
                // 8-bit luma gradient: only the first row remains black.
                if y != 0 {
                    data[row_start..row_start + row_bytes].fill(0xff);
                }
            }
            GradientMode::Integer => {
                // 4-byte i32 LE per pixel
                let val = y as i32;
                let bytes = val.to_le_bytes();
                for x in 0..256 {
                    let off = row_start + x * 4;
                    data[off..off + 4].copy_from_slice(&bytes);
                }
            }
            GradientMode::Float => {
                // 4-byte f32 LE per pixel
                let val = y as f32;
                let bytes = val.to_le_bytes();
                for x in 0..256 {
                    let off = row_start + x * 4;
                    data[off..off + 4].copy_from_slice(&bytes);
                }
            }
        }
    }
    Image::frombytes(mode, (256, 256), &data)
}

/// Generates a 256 by 256 radial gradient.
///
/// Supported modes are `"1"`, `"L"`, `"P"`, `"I"`, and `"F"`. Pixel values
/// follow Pillow's radial distance formula.
///
/// # Errors
///
/// Returns [`PilError::ValueError`] when `mode` is unsupported, or another
/// [`PilError`] when raw image construction fails.
pub fn radial_gradient(mode: &str) -> Result<Image, PilError> {
    let gradient_mode = parse_gradient_mode(mode)?;
    let (bytes_per_pixel, row_bytes) = match gradient_mode {
        GradientMode::Byte => (1, 256),
        GradientMode::One => (1, 256usize.div_ceil(8)),
        GradientMode::Integer | GradientMode::Float => (4, 256),
    };
    let size: usize = row_bytes * 256 * bytes_per_pixel;
    let mut data = vec![0u8; size];

    for y in 0..256 {
        for x in 0..256 {
            let dx = x as f64 - 128.0;
            let dy = y as f64 - 128.0;
            // PIL exact formula: d = (int) sqrt((dx*dx + dy*dy) * 2.0)
            let d = ((dx * dx + dy * dy) * 2.0).sqrt() as i32;
            let val = if d >= 255 { 255u8 } else { d as u8 };

            match gradient_mode {
                GradientMode::Byte => {
                    data[y * 256 + x] = val;
                }
                GradientMode::One => {
                    // Mode-1 output keeps every nonzero radial sample white;
                    // the single zero-valued center sample remains black.
                    if val != 0 {
                        let byte_idx = y * row_bytes + x / 8;
                        let bit_idx = 7 - (x % 8);
                        data[byte_idx] |= 1 << bit_idx;
                    }
                }
                GradientMode::Integer => {
                    let bytes = (val as i32).to_le_bytes();
                    let off = (y * 256 + x) * 4;
                    data[off..off + 4].copy_from_slice(&bytes);
                }
                GradientMode::Float => {
                    let bytes = (val as f32).to_le_bytes();
                    let off = (y * 256 + x) * 4;
                    data[off..off + 4].copy_from_slice(&bytes);
                }
            }
        }
    }
    Image::frombytes(mode, (256, 256), &data)
}

/// Generates a Mandelbrot effect image.
///
/// `size` is output dimensions, `extent` is `(x0, y0, x1, y1)` in the complex
/// plane, and `quality` controls iteration count.
///
/// # Errors
///
/// Returns [`PilError::ValueError`] for zero size, negative extent dimensions,
/// or `quality < 2`. Returns [`PilError::DimensionError`] when allocation
/// checks fail.
pub fn effect_mandelbrot(
    size: (u32, u32),
    extent: (f64, f64, f64, f64),
    quality: i32,
) -> Result<Image, PilError> {
    let (w, h) = size;
    if w == 0 || h == 0 {
        // Pillow accepts a zero-size mandelbrot and returns an empty image.
        return Image::new(w, h, "L", (0, 0, 0, 255));
    }

    let (x0, y0, x1, y1) = extent;
    let width = x1 - x0;
    let height = y1 - y0;

    if width < 0.0 || height < 0.0 || quality < 2 {
        return Err(PilError::ValueError("unrecognized argument value".into()));
    }

    // Pillow's C implementation divides by ``width - 1`` and ``height - 1``
    // without a degenerate-dimension guard.  For a one-pixel axis this yields
    // NaN, and every comparison in the iteration loop remains false, producing
    // the all-zero row/column observed from the public API.  Preserve that
    // version-matched behavior instead of replacing it with a finite stride.
    let dr = width / (w - 1) as f64;
    let di = height / (h - 1) as f64;

    // PIL uses escape radius 100.0 (NOT the common 4.0)
    let radius = 100.0f64;
    let mut data = CheckedDims::new(w, h, 1)?.alloc_buffer();

    for y in 0..h {
        let row_start = (y * w) as usize;
        for x in 0..w {
            let cr = x as f64 * dr + x0;
            let ci = y as f64 * di + y0;

            // PIL's exact loop: for (k = 1;; k++) with check order:
            //   1. compute Mandelbrot iteration
            //   2. check escape → pixel = k*255/quality (as u8, may overflow)
            //   3. check k > quality → pixel = 0 (never escaped)
            let mut zx = 0.0f64;
            let mut zy = 0.0f64;
            let mut zx2 = 0.0f64;
            let mut zy2 = 0.0f64;

            let mut k: i32 = 1;
            loop {
                // y1 = 2 * x1 * y1 + ci
                zy = 2.0 * zx * zy + ci;
                // x1 = xi2 - yi2 + cr  (using OLD xi2/yi2)
                zx = zx2 - zy2 + cr;
                zx2 = zx * zx;
                zy2 = zy * zy;

                if zx2 + zy2 > radius {
                    // PIL: buf[x] = k * 255 / quality (stored as UINT8)
                    // In C: int val = k * 255 / quality; buf[x] = (UINT8)val;
                    let val = (k * 255 / quality) as u8;
                    data[row_start + x as usize] = val;
                    break;
                }
                if k > quality {
                    data[row_start + x as usize] = 0;
                    break;
                }
                k += 1;
            }
        }
    }

    Image::frombytes("L", (w, h), &data)
}

/// Validates a host-provided Mandelbrot extent and delegates to the typed
/// Rust implementation. The host type name is metadata used only to preserve
/// Pillow's diagnostic for non-four-item sequences.
pub fn effect_mandelbrot_with_extent(
    size: (u32, u32),
    extent: Option<&[f64]>,
    extent_type: &str,
    quality: i32,
) -> Result<Image, PilError> {
    let Some(extent) = extent else {
        return Err(PilError::TypeError(format!(
            "argument 2 must be 4-item sequence, not {extent_type}"
        )));
    };
    if extent.len() != 4 {
        // Pillow's argument parser reports the observed sequence arity before
        // it falls back to the host type name used for non-sequences.
        return Err(PilError::TypeError(format!(
            "argument 2 must be sequence of length 4, not {}",
            extent.len()
        )));
    }
    effect_mandelbrot(size, (extent[0], extent[1], extent[2], extent[3]), quality)
}
