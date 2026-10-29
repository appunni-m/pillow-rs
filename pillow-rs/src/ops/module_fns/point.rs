//! Pillow point/eval operations and their host-neutral input contract.

use crate::error::PilError;
use crate::image::Image;
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

    Ok(Image::from_dynamic(
        result,
        crate::image::mode_boundary::parse_pillow_mode(&mode),
    ))
}

/// Applies a host callback to each scalar sample in an integer or float image.
///
/// Unlike byte-valued point operations, scalar images do not have a bounded
/// 256-entry lookup table. The caller owns the callback; Rust owns image
/// materialization, sample decoding, and output conversion.
pub fn eval_point_callable<F>(image: &Image, mut callback: F) -> Result<Image, PilError>
where
    F: FnMut(f64) -> Result<f64, PilError>,
{
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
                let value = f64::from(i32::from_le_bytes(pixel.0));
                pixel.0 = (callback(value)? as i32).to_le_bytes();
            }
        }
        "F" => {
            let pixels = result.as_mut_rgba8().ok_or_else(|| {
                PilError::InternalError("F image has no four-byte scalar storage".to_owned())
            })?;
            for pixel in pixels.pixels_mut() {
                let value = f64::from(f32::from_le_bytes(pixel.0));
                pixel.0 = (callback(value)? as f32).to_le_bytes();
            }
        }
        "I;16" | "I;16L" | "I;16B" | "I;16N" => {
            let pixels = result.as_mut_luma16().ok_or_else(|| {
                PilError::InternalError("I;16 image has no 16-bit scalar storage".to_owned())
            })?;
            for pixel in pixels.pixels_mut() {
                pixel.0[0] = callback(f64::from(pixel.0[0]))? as u16;
            }
        }
        _ => unreachable!("scalar point mode was validated above"),
    }

    Ok(Image::from_dynamic(
        result,
        crate::image::mode_boundary::parse_pillow_mode(&mode),
    ))
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

/// Builds a floating-point point-operation LUT from a host callback.
///
/// The callback remains in its owning runtime. Core controls the Pillow byte
/// sample order and returns the values without rounding or clamping.
pub fn make_float_lut<E, F>(mut callback: F) -> Result<Vec<f64>, E>
where
    F: FnMut(u32) -> Result<f64, E>,
{
    let mut table = Vec::with_capacity(256);
    for sample in 0..256u32 {
        table.push(callback(sample)?);
    }
    Ok(table)
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
