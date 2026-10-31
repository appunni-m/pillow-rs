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
use crate::raster::Mode;

// TODO(pillow-rs#3ptb): accept `Mode` for merge after host bindings perform
// Pillow spelling conversion.

mod point;
pub use point::{
    EvalInputKind, eval, eval_callable, eval_float, eval_point_callable, eval_point_transform,
    eval_replicated, eval_replicated_for_image, eval_validated, make_float_lut, make_lut,
    validate_eval_input,
};

/// Creates an image from raw bytes using Pillow's supported raw decoder.
///
/// Decoder selection is part of the public operation contract, not a binding
/// concern. Host adapters pass the decoder name through after extracting it.
pub fn frombytes(
    mode: Mode,
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
    mode: Mode,
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
        *tag = crate::image::mode_boundary::parse_pillow_mode(mode);
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
        *explicit_mode = image2.known_mode_identity();
        if output_mode == "P" {
            // The output begins as image2.copy(), so palette ownership follows
            // image2 even when image1 was converted to P for the paste.
            *palette = Some(image2.extract_palette().unwrap_or_default());
            *palette_alpha = image2.palette_alpha();
        } else if output_mode == "PA" {
            // Image.composite starts with image2.copy(); Pillow preserves the
            // destination PA image's palette on that newly allocated result.
            *palette = image2.extract_palette();
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
    let source = Image::new(size.0, size.1, Mode::ImageLuma8, (0, 0, 0, 255))?;
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
pub fn linear_gradient(mode: Mode) -> Result<Image, PilError> {
    let generated = crate::compute::execute_eager_cpu_linear_gradient(mode)?;
    Image::from_generated_dynamic(generated, mode)
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
pub fn radial_gradient(mode: Mode) -> Result<Image, PilError> {
    let generated = crate::compute::execute_eager_cpu_radial_gradient(mode)?;
    Image::from_generated_dynamic(generated, mode)
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
        return Image::new(w, h, Mode::ImageLuma8, (0, 0, 0, 255));
    }

    let (x0, y0, x1, y1) = extent;
    let width = x1 - x0;
    let height = y1 - y0;

    if width < 0.0 || height < 0.0 || quality < 2 {
        return Err(PilError::ValueError("unrecognized argument value".into()));
    }

    if let Some(image) = crate::compute::try_simd_mandelbrot(size, extent, quality)? {
        return Image::from_generated_dynamic(image, Mode::ImageLuma8);
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
    let image = Image::frombytes_owned(crate::raster::Mode::ImageLuma8, (w, h), data)?;
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
mod tests;
