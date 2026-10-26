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

mod point;
pub use point::{
    EvalInputKind, eval, eval_callable, eval_float, eval_point_transform, eval_replicated,
    eval_replicated_for_image, eval_validated, make_lut, validate_eval_input,
};

#[derive(Debug, Clone, Copy)]
enum GradientMode {
    Byte,
    One,
    Integer,
    Float,
}

const fn radial_integer_sqrt(value: u32) -> u32 {
    let mut low = 0u32;
    let mut high = 256u32;
    while low < high {
        let middle = (low + high).div_ceil(2);
        if middle * middle <= value {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    low
}

const fn build_radial_gradient_l_template() -> [u8; 256 * 256] {
    let mut output = [0u8; 256 * 256];
    let mut y = 0usize;
    while y < 256 {
        let dy = y as i32 - 128;
        let mut x = 0usize;
        while x < 256 {
            let dx = x as i32 - 128;
            let squared_radius = (dx * dx + dy * dy) as u32 * 2;
            let distance = radial_integer_sqrt(squared_radius);
            output[y * 256 + x] = if distance >= 255 { 255 } else { distance as u8 };
            x += 1;
        }
        y += 1;
    }
    output
}

/// Compile-time Pillow-compatible native-L raster for the fixed radial
/// gradient. The coordinate domain and integer square root preserve the exact
/// truncation of Pillow's `sqrt((dx*dx + dy*dy) * 2.0)` formula.
pub(crate) static RADIAL_GRADIENT_L_TEMPLATE: [u8; 256 * 256] = build_radial_gradient_l_template();

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

// Eager generators bypass PipelineOp routing. When their SIMD path declines a
// mode or GPU is selected, record the host implementation instead of letting
// a strict backend run pass without an executor receipt.
fn execute_eager_cpu_gradient(
    operation: &'static str,
    output_bytes: usize,
    execute: impl FnOnce() -> Result<Image, PilError>,
) -> Result<Image, PilError> {
    let requested_backend = if crate::compute::Backend::pipeline_telemetry_enabled() {
        crate::compute::preferred_active_backend()
            .ok()
            .flatten()
            .filter(|(_, only_active_backend)| *only_active_backend)
            .map(|(backend, _)| backend)
    } else {
        None
    };
    let telemetry =
        crate::compute::begin_eager_cpu_operation_telemetry(operation, requested_backend);
    match execute() {
        Ok(image) => {
            crate::compute::finish_eager_cpu_operation_telemetry(telemetry, output_bytes);
            Ok(image)
        }
        Err(error) => {
            crate::compute::cancel_eager_cpu_operation_telemetry(telemetry);
            Err(error)
        }
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

/// Creates an image from an owned raw byte buffer, avoiding a second copy for
/// raw raster modes when the decoder can adopt the supplied allocation.
pub fn frombytes_owned(
    mode: &str,
    size: (u32, u32),
    data: Vec<u8>,
    decoder_name: &str,
) -> Result<Image, PilError> {
    if decoder_name != "raw" {
        return Err(PilError::IOError(format!(
            "decoder {decoder_name} not available"
        )));
    }
    Image::frombytes_owned(mode, size, data)
}

/// Composites `im2` over `im1` and returns a new image.
pub fn alpha_composite(im1: &Image, im2: &Image) -> Result<Image, PilError> {
    let mut result = im1.copy();
    result.alpha_composite(im2, (0, 0), (0, 0))?;
    Ok(result)
}

fn validate_merge_shape(mode: &str, band_count: usize) -> Result<(), PilError> {
    let n_expected = match mode {
        "1" | "I" | "F" | "P" | "I;16" | "I;16L" | "I;16B" | "I;16N" => 1,
        "RGB" => 3,
        "RGBA" => 4,
        "RGBX" | "RGBa" => 4,
        "CMYK" => 4,
        "LA" | "La" | "PA" => 2,
        "YCbCr" | "HSV" | "LAB" => 3,
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
        let valid = match mode {
            // ImagingMerge requires a source core with the exact scalar mode
            // for these destinations; an L/P band is not coerced to 1/I/F.
            "1" | "I" | "F" | "P" | "I;16" | "I;16L" | "I;16B" | "I;16N" => band_mode == mode,
            "L" => band_mode == "L",
            _ => band_mode == "L" || (index == 0 && band_mode == "P"),
        };
        if !valid {
            return Err(
                if matches!(
                    mode,
                    "L" | "1" | "I" | "F" | "P" | "I;16" | "I;16L" | "I;16B" | "I;16N"
                ) {
                    PilError::ValueError("images do not match".into())
                } else {
                    PilError::ValueError("mode mismatch".into())
                },
            );
        }
    }
    Ok(())
}

/// Parses the mode names accepted specifically by Pillow's `Image.merge`.
///
/// The raster core stores `La`/`PA` as `LA`, `RGBX`/`RGBa` as `RGBA`, and
/// `LAB` as RGB bytes. The public mode tag remains attached to the pipeline so
/// callers still observe the requested Pillow spelling; conversion continues
/// to use its narrower, independently validated mode parser.
fn parse_merge_mode(mode: &str) -> Result<crate::pipeline::ColorMode, PilError> {
    match mode {
        "La" | "PA" => Ok(crate::pipeline::ColorMode::LA),
        "RGBX" | "RGBa" => Ok(crate::pipeline::ColorMode::RGBA),
        "LAB" => Ok(crate::pipeline::ColorMode::RGB),
        // DynamicImage carries all I;16* variants as native unsigned-16
        // luma. The requested spelling remains on PipelineOp::logical_mode
        // and the public explicit mode tag; mapping here lets the executor
        // take its existing typed scalar identity path without narrowing to
        // a four-byte I buffer.
        "I;16" | "I;16L" | "I;16B" | "I;16N" => Ok(crate::pipeline::ColorMode::I),
        _ => parse_mode(mode),
    }
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
    Image(Box<Image>),
    /// A non-image host value, retaining only its type name for the
    /// Pillow-compatible attribute error raised by the merge implementation.
    Invalid(String),
}

/// Merges single-band images into a multi-band image.
///
/// `mode` determines the required band count. Pillow's byte layouts include
/// `L`, `La`, `PA`, `RGB`, `RGBX`, `RGBA`, `RGBa`, `YCbCr`, `HSV`, `LAB`, and
/// `CMYK`; the typed scalar layouts `1`, `I`, `F`, `P`, and `I;16*` are also
/// accepted.
///
/// # Errors
///
/// Returns [`PilError::ValueError`] when `mode` is unsupported or `bands` has
/// the wrong length.
pub fn merge(mode: &str, bands: &[Image]) -> Result<Image, PilError> {
    validate_merge_shape(mode, bands.len())?;
    validate_merge_band_modes(mode, bands)?;
    validate_merge_band_sizes(bands)?;

    let mode_enum = parse_merge_mode(mode)?;
    let mut result = Image::push_op(
        &bands[0],
        PipelineOp::Merge {
            mode: mode_enum,
            logical_mode: mode.to_owned(),
            bands: bands.to_vec().into(),
        },
    );
    if let Image::Pipeline {
        explicit_mode: tag, ..
    } = &mut result
        && mode != "L"
        && mode != "LA"
        && mode != "RGB"
        && mode != "RGBA"
    {
        *tag = Some(mode.to_owned());
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
            MergeInput::Image(image) => images.push((**image).clone()),
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
/// `alpha` may extrapolate outside `0.0..=1.0`. Each sample is blended
/// with fused float32 arithmetic, then clipped to the byte range and truncated.
///
/// # Errors
///
/// Returns [`PilError::ValueError`] for incompatible modes or unequal dimensions, or
/// another [`PilError`] when metadata lookup fails.
pub fn blend(image1: &Image, image2: &Image, alpha: f64) -> Result<Image, PilError> {
    let mode1 = image1.mode()?;
    let mode2 = image2.mode()?;
    let compatible = match mode1.as_str() {
        "L" => mode2 == "L",
        "LA" | "La" => matches!(mode2.as_str(), "LA" | "La"),
        "RGB" | "LAB" | "HSV" | "YCbCr" => {
            matches!(mode2.as_str(), "RGB" | "LAB" | "HSV" | "YCbCr")
        }
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
            "L" | "LA" | "La" | "RGB" | "LAB" | "HSV" | "YCbCr" | "RGBA" | "CMYK" | "RGBa" | "RGBX"
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
    execute_eager_cpu_gradient("LinearGradient", size, || {
        let mut data = if mode == "L" {
            Vec::with_capacity(size)
        } else {
            vec![0u8; size]
        };

        for y in 0..256usize {
            if mode == "L" {
                // Each native-L row is one repeated sample; initialize every
                // output byte once instead of zeroing the frame before filling it.
                data.resize((y + 1) * 256, y as u8);
                continue;
            }
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
        // The generated buffer is already the final raster, so transfer ownership
        // instead of copying it through the borrowed `frombytes` constructor.
        Image::frombytes_owned(mode, (256, 256), data)
    })
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
    if let Some(image) = crate::compute::try_simd_radial_gradient(mode)? {
        return Ok(Image::from_generated_dynamic(image, mode));
    }
    if mode == "L" {
        // L is the fixed dense one-byte output, so reuse its exact constant
        // raster and allocate only the owned result buffer.
        return execute_eager_cpu_gradient(
            "RadialGradient",
            RADIAL_GRADIENT_L_TEMPLATE.len(),
            || Image::frombytes_owned(mode, (256, 256), RADIAL_GRADIENT_L_TEMPLATE.to_vec()),
        );
    }

    let (bytes_per_pixel, row_bytes) = match gradient_mode {
        GradientMode::Byte => (1, 256),
        GradientMode::One => (1, 256usize.div_ceil(8)),
        GradientMode::Integer | GradientMode::Float => (4, 256),
    };
    let size: usize = row_bytes * 256 * bytes_per_pixel;
    execute_eager_cpu_gradient("RadialGradient", size, || {
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
    })
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

    if let Some(image) = crate::compute::try_simd_mandelbrot(size, extent, quality)? {
        return Ok(Image::from_generated_dynamic(image, "L"));
    }

    // Pillow's C implementation divides by ``width - 1`` and ``height - 1``
    // without a degenerate-dimension guard.  For a one-pixel axis this yields
    // NaN, and every comparison in the iteration loop remains false, producing
    // the all-zero row/column observed from the public API.  Preserve that
    // version-matched behavior instead of replacing it with a finite stride.
    let telemetry = crate::compute::begin_eager_cpu_mandelbrot_telemetry();
    let dr = width / (w - 1) as f64;
    let di = height / (h - 1) as f64;

    // Pillow uses escape radius 100.0 (NOT the common 4.0).
    let mut data = CheckedDims::new(w, h, 1)?.alloc_buffer();

    // These two connected components are provably bounded: points inside the
    // main cardioid and period-2 bulb never reach Pillow's escape radius. Keep
    // a quality-scaled floating-point margin so coordinates near their
    // boundaries still take the exact reference loop below.
    let membership_margin = mandelbrot_membership_margin(quality);

    for y in 0..h {
        let row_start = (y * w) as usize;
        // `ci` is constant for the row. Pillow computes this same expression
        // per pixel; hoisting it preserves the exact rounded value while
        // removing redundant coordinate arithmetic from the inner loop.
        let ci = y as f64 * di + y0;
        for x in 0..w {
            let cr = x as f64 * dr + x0;

            if mandelbrot_known_interior(cr, ci, membership_margin) {
                data[row_start + x as usize] = 0;
                continue;
            }
            data[row_start + x as usize] = mandelbrot_pixel_value(cr, ci, quality);
        }
    }

    // Transfer the completed L buffer into raster storage; borrowing here
    // would copy the entire generated frame before returning it.
    let image = Image::frombytes_owned("L", (w, h), data)?;
    crate::compute::finish_eager_cpu_mandelbrot_telemetry(telemetry, size);
    Ok(image)
}

#[inline]
pub(crate) fn mandelbrot_membership_margin(quality: i32) -> f64 {
    128.0 * f64::EPSILON * (f64::from(quality) + 1.0).powi(2)
}

#[inline]
pub(crate) fn mandelbrot_known_interior(cr: f64, ci: f64, margin: f64) -> bool {
    let ci2 = ci * ci;

    // Main cardioid: q * (q + x) <= y^2 / 4, x = cr - 1/4.
    let x = cr - 0.25;
    let q = x * x + ci2;
    let cardioid_left = q * (q + x);
    let cardioid_right = 0.25 * ci2;
    if cardioid_left < cardioid_right - margin * (1.0 + cardioid_left.abs() + cardioid_right.abs())
    {
        return true;
    }

    // Period-2 bulb: (cr + 1)^2 + ci^2 < 1/16.
    let bulb_x = cr + 1.0;
    let bulb_distance = bulb_x * bulb_x + ci2;
    bulb_distance < 0.0625 - margin * (1.0 + bulb_distance.abs())
}

#[inline(always)]
pub(crate) fn mandelbrot_pixel_value(cr: f64, ci: f64, quality: i32) -> u8 {
    let mut zx = 0.0f64;
    let mut zy = 0.0f64;
    let mut zx2 = 0.0f64;
    let mut zy2 = 0.0f64;

    let mut k: i32 = 1;
    loop {
        // Preserve Pillow's operation order: compute y first, then x from the
        // previous squares, then test escape before testing quality.
        zy = 2.0 * zx * zy + ci;
        zx = zx2 - zy2 + cr;
        zx2 = zx * zx;
        zy2 = zy * zy;

        if zx2 + zy2 > 100.0 {
            return (k * 255 / quality) as u8;
        }
        if k > quality {
            return 0;
        }
        k += 1;
    }
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

#[cfg(test)]
mod tests {
    use super::merge;
    use crate::image::{FormattedImageData, FormattedPixelValue, Image};

    fn l_band(bytes: &[u8]) -> Image {
        Image::frombytes("L", (2, 1), bytes).expect("valid L band")
    }

    #[test]
    fn merge_preserves_alias_modes_and_lab_encoding() {
        let l0 = l_band(&[0x10, 0x11]);
        let l1 = l_band(&[0x20, 0x21]);
        let l2 = l_band(&[0x30, 0x31]);
        let l3 = l_band(&[0x40, 0x41]);

        let cases = [
            (
                "La",
                vec![l0.clone(), l1.clone()],
                vec![0x10, 0x20, 0x11, 0x21],
                vec!["L", "a"],
            ),
            (
                "PA",
                vec![l0.clone(), l1.clone()],
                vec![0x10, 0x20, 0x11, 0x21],
                vec!["P", "A"],
            ),
            (
                "RGBX",
                vec![l0.clone(), l1.clone(), l2.clone(), l3.clone()],
                vec![0x10, 0x20, 0x30, 0x40, 0x11, 0x21, 0x31, 0x41],
                vec!["R", "G", "B", "X"],
            ),
            (
                "RGBa",
                vec![l0.clone(), l1.clone(), l2.clone(), l3.clone()],
                vec![0x10, 0x20, 0x30, 0x40, 0x11, 0x21, 0x31, 0x41],
                vec!["R", "G", "B", "a"],
            ),
            (
                "YCbCr",
                vec![l0.clone(), l1.clone(), l2.clone()],
                vec![0x10, 0x20, 0x30, 0x11, 0x21, 0x31],
                vec!["Y", "Cb", "Cr"],
            ),
            (
                "HSV",
                vec![l0.clone(), l1.clone(), l2.clone()],
                vec![0x10, 0x20, 0x30, 0x11, 0x21, 0x31],
                vec!["H", "S", "V"],
            ),
        ];
        for (mode, bands, bytes, names) in cases {
            let result = merge(mode, &bands).expect("alias merge succeeds");
            assert_eq!(result.mode().expect("mode"), mode);
            assert_eq!(result.tobytes().expect("bytes"), bytes);
            assert_eq!(result.getbands().expect("bands"), names);
        }
        assert_lab_merge_encoding(l0, l1, l2);
    }

    fn assert_lab_merge_encoding(l0: Image, l1: Image, l2: Image) {
        let lab = merge("LAB", &[l0, l1, l2]).expect("LAB merge succeeds");
        assert_eq!(lab.mode().expect("mode"), "LAB");
        assert_eq!(
            lab.tobytes().expect("bytes"),
            [0x10, 0xa0, 0xb0, 0x11, 0xa1, 0xb1]
        );
        assert_eq!(
            lab.getpixel_formatted(0, 0).expect("pixel"),
            FormattedPixelValue::Components(vec![0x10, 0x20, 0x30])
        );
        assert_eq!(
            lab.getdata_formatted(None).expect("data"),
            FormattedImageData::Components(vec![vec![0x10, 0x20, 0x30], vec![0x11, 0x21, 0x31]])
        );
    }

    #[test]
    fn merge_preserves_first_palette_band_and_rejects_later_palette() {
        let palette = Image::frombytes("P", (2, 1), &[1, 2]).expect("valid P band");
        let l0 = l_band(&[0x20, 0x21]);
        let l1 = l_band(&[0x30, 0x31]);

        let result = merge("RGB", &[palette.clone(), l0, l1.clone()]).expect("first P is accepted");
        assert_eq!(
            result.tobytes().expect("bytes"),
            [1, 0x20, 0x30, 2, 0x21, 0x31]
        );
        assert_eq!(result.mode().expect("mode"), "RGB");

        let later = merge(
            "RGB",
            &[Image::frombytes("L", (2, 1), &[1, 2]).unwrap(), palette, l1],
        );
        assert_eq!(
            later.expect_err("later P must fail").to_string(),
            "mode mismatch"
        );
    }

    #[test]
    fn merge_keeps_typed_scalar_storage() {
        let mode1 = merge(
            "1",
            &[Image::frombytes("1", (2, 1), &[0xa0]).expect("valid 1 band")],
        )
        .expect("1 merge");
        assert_eq!(mode1.tobytes().expect("1 bytes"), [0x80]);

        let int = merge(
            "I",
            &[
                Image::frombytes("I", (2, 1), &[1, 0, 0, 0, 0xff, 0xff, 0xff, 0xff])
                    .expect("valid I band"),
            ],
        )
        .expect("I merge");
        assert_eq!(
            int.tobytes().expect("I bytes"),
            [1, 0, 0, 0, 0xff, 0xff, 0xff, 0xff]
        );

        let float = merge(
            "F",
            &[
                Image::frombytes("F", (2, 1), &[0, 0, 0xc0, 0x3f, 0, 0, 0x10, 0xc0])
                    .expect("valid F band"),
            ],
        )
        .expect("F merge");
        assert_eq!(
            float.tobytes().expect("F bytes"),
            [0, 0, 0xc0, 0x3f, 0, 0, 0x10, 0xc0]
        );

        assert_eq!(
            merge("I", &[l_band(&[1, 2])])
                .expect_err("L cannot merge into I")
                .to_string(),
            "images do not match"
        );
    }

    #[test]
    fn merge_keeps_i16_byte_order_and_scalar_values() {
        for (mode, bytes) in [
            ("I;16", vec![0x01, 0x02, 0x03, 0x04]),
            ("I;16L", vec![0x01, 0x02, 0x03, 0x04]),
            ("I;16B", vec![0x01, 0x02, 0x03, 0x04]),
            ("I;16N", vec![0x01, 0x02, 0x03, 0x04]),
        ] {
            let band = Image::frombytes(mode, (2, 1), &bytes).expect("valid I;16 band");
            let merged = merge(mode, &[band]).expect("I;16 merge succeeds");
            assert_eq!(merged.mode().expect("mode"), mode);
            assert_eq!(merged.tobytes().expect("raw bytes"), bytes);
            assert!(matches!(
                merged.getdata_formatted(None).expect("data"),
                FormattedImageData::IntegerScalars(values) if values.len() == 2
            ));
        }
    }

    #[test]
    fn merge_rejects_i16_band_mode_and_arity_mismatches() {
        let band = Image::frombytes("I;16", (1, 1), &[0x2a, 0x00]).expect("valid I;16 band");
        let other = Image::frombytes("I;16L", (1, 1), &[0x2a, 0x00]).expect("valid I;16L band");
        assert_eq!(
            merge("I;16", &[]).expect_err("missing band").to_string(),
            "wrong number of bands"
        );
        assert_eq!(
            merge("I;16", &[band.clone(), band.clone()])
                .expect_err("extra band")
                .to_string(),
            "wrong number of bands"
        );
        assert_eq!(
            merge("I;16", &[other])
                .expect_err("wrong I;16 spelling")
                .to_string(),
            "images do not match"
        );
    }
}
