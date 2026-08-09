//! SIMD adapter wrappers — bridge `pool_simd::ops::scalar` functions to the
//! registry's `SimdOpFn` signature.
//!
//! Each adapter:
//! 1. Extracts packed u32 RGBA pixels from `DynamicImage`
//! 2. Calls the scalar SIMD function
//! 3. Reconstructs `DynamicImage` from the result

use crate::error::PilError;
use crate::image::{Image, preserve_mode};
use crate::pipeline::{
    ColorMode, PipelineOp, PixelMode, ResampleFilter, TransformMethod, TransposeMethod,
};
use crate::raster::{
    DynamicImage, GenericImageView, GrayAlphaImage, GrayImage, RgbImage, RgbaImage,
};
use std::sync::Arc;

// ── Helper: mode string → encoding ─────────────────────────────────────

/// Convert PIL mode string to SIMD mode code.
/// 0=L, 1=LA, 2=RGB, 3=RGBA
fn mode_to_u32(mode: Option<&str>) -> u32 {
    match mode {
        None | Some("RGBA") => 3,
        Some("RGB") => 2,
        Some("LA" | "PA") => 1,
        Some("L" | "1" | "P") => 0,
        _ => 3, // default to RGBA
    }
}

/// Convert ColorMode to SIMD mode code.
fn color_mode_to_u32(cm: &ColorMode) -> u32 {
    match cm {
        ColorMode::L | ColorMode::Mode1 => 0,
        ColorMode::LA => 1,
        ColorMode::RGB => 2,
        ColorMode::RGBA => 3,
        ColorMode::CMYK => 4,
        _ => 3, // fallback
    }
}

/// Convert ResampleFilter to SIMD filter code (0=nearest, 1=bilinear).
fn filter_to_u32(f: &ResampleFilter) -> u32 {
    match f {
        ResampleFilter::Nearest | ResampleFilter::Box => 0,
        _ => 1,
    }
}

/// Pack a `(r,g,b,a)` tuple into a u32 for SIMD functions.
fn pack_rgba(c: (u8, u8, u8, u8)) -> u32 {
    (c.0 as u32) | ((c.1 as u32) << 8) | ((c.2 as u32) << 16) | ((c.3 as u32) << 24)
}

/// Derive SIMD mode code from a DynamicImage's channel count.
/// 0=L (1ch), 1=LA (2ch), 2=RGB (3ch), 3=RGBA (4ch)
fn dynimg_mode(img: &DynamicImage) -> u32 {
    match img.color().channel_count() {
        1 => 0,
        2 => 1,
        3 => 2,
        4 => 3,
        _ => 3,
    }
}

// ── Helper: DynamicImage ↔ packed u32 ─────────────────────────────────

/// Extract packed u32 RGBA pixels from a DynamicImage.
fn pixels_from_dynimg(img: &DynamicImage) -> Vec<u32> {
    img.to_rgba8()
        .pixels()
        .map(|p| {
            (p[0] as u32) | ((p[1] as u32) << 8) | ((p[2] as u32) << 16) | ((p[3] as u32) << 24)
        })
        .collect()
}

/// Reconstruct a DynamicImage from packed u32 RGBA pixels.
fn dynimg_from_rgba(pixels: Vec<u32>, w: u32, h: u32) -> Result<DynamicImage, PilError> {
    let rgba_bytes: Vec<u8> = pixels
        .iter()
        .flat_map(|&p| {
            vec![
                (p & 0xFF) as u8,
                ((p >> 8) & 0xFF) as u8,
                ((p >> 16) & 0xFF) as u8,
                ((p >> 24) & 0xFF) as u8,
            ]
        })
        .collect();
    RgbaImage::from_raw(w, h, rgba_bytes)
        .map(DynamicImage::ImageRgba8)
        .ok_or_else(|| PilError::InternalError("SIMD RGBA buffer shape mismatch".to_string()))
}

/// Reconstruct the logical sample layout used by a mode-preserving mutator.
fn dynimg_from_pixel_mode(
    pixels: Vec<u32>,
    w: u32,
    h: u32,
    mode: PixelMode,
) -> Result<DynamicImage, PilError> {
    match mode {
        PixelMode::L | PixelMode::P | PixelMode::Mode1 => {
            let bytes = pixels.iter().map(|pixel| (*pixel & 0xFF) as u8).collect();
            GrayImage::from_raw(w, h, bytes)
                .map(DynamicImage::ImageLuma8)
                .ok_or_else(|| PilError::InternalError("SIMD L buffer shape mismatch".to_string()))
        }
        PixelMode::LA | PixelMode::PA => {
            let bytes = pixels
                .iter()
                .flat_map(|pixel| [(*pixel & 0xFF) as u8, ((*pixel >> 24) & 0xFF) as u8])
                .collect();
            GrayAlphaImage::from_raw(w, h, bytes)
                .map(DynamicImage::ImageLumaA8)
                .ok_or_else(|| PilError::InternalError("SIMD LA buffer shape mismatch".to_string()))
        }
        PixelMode::RGB | PixelMode::YCbCr | PixelMode::HSV => {
            let bytes = pixels
                .iter()
                .flat_map(|pixel| {
                    [
                        (*pixel & 0xFF) as u8,
                        ((*pixel >> 8) & 0xFF) as u8,
                        ((*pixel >> 16) & 0xFF) as u8,
                    ]
                })
                .collect();
            RgbImage::from_raw(w, h, bytes)
                .map(DynamicImage::ImageRgb8)
                .ok_or_else(|| {
                    PilError::InternalError("SIMD RGB buffer shape mismatch".to_string())
                })
        }
        PixelMode::RGBA | PixelMode::CMYK | PixelMode::I | PixelMode::F => {
            dynimg_from_rgba(pixels, w, h)
        }
    }
}

/// Reconstruct the promoted result of `Image.putalpha`.
fn dynimg_from_put_alpha(
    pixels: Vec<u32>,
    w: u32,
    h: u32,
    mode: PixelMode,
) -> Result<DynamicImage, PilError> {
    match mode {
        PixelMode::L | PixelMode::LA | PixelMode::P | PixelMode::PA => {
            dynimg_from_pixel_mode(pixels, w, h, PixelMode::LA)
        }
        _ => dynimg_from_rgba(pixels, w, h),
    }
}

/// Materialize an Arc<Image> → DynamicImage.
fn arc_to_dynimg(arc: &Arc<Image>) -> Result<DynamicImage, PilError> {
    arc.materialize_for_ops()
}

/// Extract packed u32 pixels from an Arc<Image>.
fn pixels_from_arc(arc: &Arc<Image>) -> Result<Vec<u32>, PilError> {
    let img = arc_to_dynimg(arc)?;
    Ok(pixels_from_dynimg(&img))
}

// ═══════════════════════════════════════════════════════════════════════
// Section A: Simple single-image ops (no extra params beyond mode)
// ═══════════════════════════════════════════════════════════════════════

pub fn simd_invert(
    img: &DynamicImage,
    _op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let mode_code = mode_to_u32(mode);
    let (w, h) = img.dimensions();
    let mut pixels = pixels_from_dynimg(img);
    super::scalar::invert(&mut pixels, mode_code);
    dynimg_from_rgba(pixels, w, h)
}

pub fn simd_grayscale(
    img: &DynamicImage,
    _op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let mode_code = mode_to_u32(mode);
    let (w, h) = img.dimensions();
    let mut pixels = pixels_from_dynimg(img);
    super::scalar::grayscale(&mut pixels, mode_code);
    let luma = pixels.into_iter().map(|pixel| pixel as u8).collect();
    GrayImage::from_raw(w, h, luma)
        .map(DynamicImage::ImageLuma8)
        .ok_or_else(|| PilError::InternalError("SIMD grayscale buffer shape mismatch".to_string()))
}

pub fn simd_duplicate(
    img: &DynamicImage,
    _op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let mode_code = mode_to_u32(mode);
    let (w, h) = img.dimensions();
    let mut pixels = pixels_from_dynimg(img);
    super::scalar::duplicate(&mut pixels, mode_code);
    dynimg_from_rgba(pixels, w, h)
}

pub fn simd_invert_chops(
    img: &DynamicImage,
    _op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let mode_code = mode_to_u32(mode);
    let (w, h) = img.dimensions();
    let mut pixels = pixels_from_dynimg(img);
    super::scalar::invert_chops(&mut pixels, mode_code);
    Ok(preserve_mode(img, dynimg_from_rgba(pixels, w, h)?))
}

// ═══════════════════════════════════════════════════════════════════════
// Section B: Single-image with extra params (solarize, posterize, ...)
// ═══════════════════════════════════════════════════════════════════════

pub fn simd_solarize(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(mode);
    let mut pixels = pixels_from_dynimg(img);
    if let PipelineOp::Solarize { threshold } = op {
        super::scalar::solarize(&mut pixels, mode_code, *threshold);
    }
    dynimg_from_rgba(pixels, w, h)
}

pub fn simd_posterize(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(mode);
    let mut pixels = pixels_from_dynimg(img);
    if let PipelineOp::Posterize { bits } = op {
        super::scalar::posterize(&mut pixels, mode_code, *bits as u32);
    }
    dynimg_from_rgba(pixels, w, h)
}

pub fn simd_brightness(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(mode);
    let mut pixels = pixels_from_dynimg(img);
    if let PipelineOp::Brightness { factor } = op {
        super::scalar::brightness(&mut pixels, mode_code, (factor * 1000.0) as u32);
    }
    dynimg_from_rgba(pixels, w, h)
}

pub fn simd_contrast(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(mode);
    let mut pixels = pixels_from_dynimg(img);
    if let PipelineOp::Contrast { factor } = op {
        super::scalar::contrast(&mut pixels, mode_code, (factor * 1000.0) as u32);
    }
    dynimg_from_rgba(pixels, w, h)
}

pub fn simd_color_saturation(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(mode);
    let mut pixels = pixels_from_dynimg(img);
    if let PipelineOp::ColorSaturation { factor } = op {
        super::scalar::color_saturation(&mut pixels, mode_code, (factor * 1000.0) as u32);
    }
    dynimg_from_rgba(pixels, w, h)
}

pub fn simd_sharpness(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(mode);
    let mut pixels = pixels_from_dynimg(img);
    if let PipelineOp::Sharpness { factor } = op {
        super::scalar::sharpness(&mut pixels, w, h, mode_code, (factor * 1000.0) as u32);
    }
    dynimg_from_rgba(pixels, w, h)
}

pub fn simd_colorize(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(mode);
    let mut pixels = pixels_from_dynimg(img);
    if let PipelineOp::Colorize {
        black,
        white,
        mid,
        blackpoint,
        midpoint,
        whitepoint,
    } = op
    {
        let lut = crate::compute::pool_cpu::ops::imageops::colorize_lut(
            black,
            white,
            *mid,
            *blackpoint,
            *midpoint,
            *whitepoint,
        );
        super::scalar::colorize(&mut pixels, mode_code, &lut);
    }
    // Pillow's ImageOps.colorize always promotes its L input to RGB. Keeping
    // the packed SIMD result as RGBA leaks the implementation storage type
    // into the public result and breaks exact mode/byte parity.
    dynimg_from_pixel_mode(pixels, w, h, PixelMode::RGB)
}

pub fn simd_constant(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(mode);
    let mut pixels = pixels_from_dynimg(img);
    if let PipelineOp::Constant { value } = op {
        let packed =
            (*value as u32) | ((*value as u32) << 8) | ((*value as u32) << 16) | 0xFF00_0000;
        super::scalar::constant(&mut pixels, mode_code, packed);
    }
    dynimg_from_rgba(pixels, w, h)
}

pub fn simd_offset(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(mode);
    let mut pixels = pixels_from_dynimg(img);
    if let PipelineOp::Offset { x, y } = op {
        let dx = x.rem_euclid(256) as u32;
        let dy = y.rem_euclid(256) as u32;
        super::scalar::offset(&mut pixels, mode_code, dx, dy);
    }
    dynimg_from_rgba(pixels, w, h)
}

// ═══════════════════════════════════════════════════════════════════════
// Section C: Spatial single-image (flip, mirror, equalize, autocontrast)
// ═══════════════════════════════════════════════════════════════════════

pub fn simd_flip(
    img: &DynamicImage,
    _op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(mode);
    let mut pixels = pixels_from_dynimg(img);
    super::scalar::flip(&mut pixels, w, h, mode_code);
    dynimg_from_rgba(pixels, w, h)
}

pub fn simd_mirror(
    img: &DynamicImage,
    _op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(mode);
    let mut pixels = pixels_from_dynimg(img);
    super::scalar::mirror(&mut pixels, w, h, mode_code);
    dynimg_from_rgba(pixels, w, h)
}

pub fn simd_equalize(
    img: &DynamicImage,
    _op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(mode);
    let mut pixels = pixels_from_dynimg(img);
    super::scalar::equalize(&mut pixels, w, h, mode_code);
    dynimg_from_rgba(pixels, w, h)
}

pub fn simd_autocontrast(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(mode);
    let mut pixels = pixels_from_dynimg(img);
    if let PipelineOp::Autocontrast { cutoff } = op {
        super::scalar::autocontrast(&mut pixels, w, h, mode_code, (*cutoff as u32).max(1));
    }
    dynimg_from_rgba(pixels, w, h)
}

// ═══════════════════════════════════════════════════════════════════════
// Section D: Filter/window ops (median, max, min, rank, conv, blur)
// ═══════════════════════════════════════════════════════════════════════

pub fn simd_median_filter(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(mode);
    let mut pixels = pixels_from_dynimg(img);
    if let PipelineOp::MedianFilter { size } = op {
        super::scalar::median_filter(&mut pixels, w, h, mode_code, *size);
    }
    Ok(preserve_mode(img, dynimg_from_rgba(pixels, w, h)?))
}

pub fn simd_max_filter(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(mode);
    let mut pixels = pixels_from_dynimg(img);
    if let PipelineOp::MaxFilter { size } = op {
        super::scalar::max_filter(&mut pixels, w, h, mode_code, *size);
    }
    Ok(preserve_mode(img, dynimg_from_rgba(pixels, w, h)?))
}

pub fn simd_min_filter(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(mode);
    let mut pixels = pixels_from_dynimg(img);
    if let PipelineOp::MinFilter { size } = op {
        super::scalar::min_filter(&mut pixels, w, h, mode_code, *size);
    }
    Ok(preserve_mode(img, dynimg_from_rgba(pixels, w, h)?))
}

pub fn simd_rank_filter(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(mode);
    let mut pixels = pixels_from_dynimg(img);
    if let PipelineOp::RankFilter { size, rank } = op {
        super::scalar::rank_filter(&mut pixels, w, h, mode_code, *size, *rank);
    }
    Ok(preserve_mode(img, dynimg_from_rgba(pixels, w, h)?))
}

pub fn simd_filter_3x3(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(mode);
    let mut pixels = pixels_from_dynimg(img);
    if let PipelineOp::Filter3x3 {
        kernel,
        scale,
        offset,
    } = op
    {
        super::scalar::filter_3x3(&mut pixels, w, h, mode_code, kernel, *scale, *offset);
    }
    Ok(preserve_mode(img, dynimg_from_rgba(pixels, w, h)?))
}

pub fn simd_filter_5x5(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(mode);
    let mut pixels = pixels_from_dynimg(img);
    if let PipelineOp::Filter5x5 {
        kernel,
        scale,
        offset,
    } = op
    {
        super::scalar::filter_5x5(&mut pixels, w, h, mode_code, kernel, *scale, *offset);
    }
    Ok(preserve_mode(img, dynimg_from_rgba(pixels, w, h)?))
}

pub fn simd_box_blur(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(mode);
    let mut pixels = pixels_from_dynimg(img);
    if let PipelineOp::BoxBlur { radius } = op {
        super::scalar::box_blur(&mut pixels, w, h, mode_code, *radius);
    }
    Ok(preserve_mode(img, dynimg_from_rgba(pixels, w, h)?))
}

pub fn simd_gaussian_blur(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(mode);
    let mut pixels = pixels_from_dynimg(img);
    if let PipelineOp::GaussianBlur { sigma } = op {
        super::scalar::gaussian_blur(&mut pixels, w, h, mode_code, *sigma);
    }
    Ok(preserve_mode(img, dynimg_from_rgba(pixels, w, h)?))
}

pub fn simd_quantize(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(mode);
    let mut pixels = pixels_from_dynimg(img);
    if let PipelineOp::Quantize { colors, .. } = op {
        super::scalar::quantize(&mut pixels, w, h, mode_code, *colors);
    }
    dynimg_from_rgba(pixels, w, h)
}

// ═══════════════════════════════════════════════════════════════════════
// Section E: Dual-image per-pixel ops (Add, Subtract, Multiply, ...)
// ═══════════════════════════════════════════════════════════════════════

macro_rules! dual_op_adapter {
    ($name:ident, $scalar_fn:path) => {
        pub fn $name(
            img: &DynamicImage,
            op: &PipelineOp,
            mode: Option<&str>,
        ) -> Result<DynamicImage, PilError> {
            let (w, h) = img.dimensions();
            let mode_code = mode_to_u32(mode);
            let mut pixels = pixels_from_dynimg(img);
            let other_pixels = match op {
                PipelineOp::Multiply { other }
                | PipelineOp::Screen { other }
                | PipelineOp::Darker { other }
                | PipelineOp::Lighter { other }
                | PipelineOp::Difference { other }
                | PipelineOp::AddModulo { other }
                | PipelineOp::SubtractModulo { other }
                | PipelineOp::LogicalAnd { other }
                | PipelineOp::LogicalOr { other }
                | PipelineOp::LogicalXor { other }
                | PipelineOp::Overlay { other }
                | PipelineOp::HardLight { other }
                | PipelineOp::SoftLight { other } => pixels_from_arc(other)?,
                _ => {
                    return Err(PilError::ValueError(
                        "unexpected op variant for dual operation".into(),
                    ))
                }
            };
            $scalar_fn(&mut pixels, mode_code, &other_pixels);
            dynimg_from_rgba(pixels, w, h)
        }
    };
}

dual_op_adapter!(simd_multiply, super::scalar::multiply);
dual_op_adapter!(simd_screen, super::scalar::screen);
dual_op_adapter!(simd_darker, super::scalar::darker);
dual_op_adapter!(simd_lighter, super::scalar::lighter);
dual_op_adapter!(simd_difference, super::scalar::difference);
dual_op_adapter!(simd_add_modulo, super::scalar::add_modulo);
dual_op_adapter!(simd_subtract_modulo, super::scalar::subtract_modulo);
dual_op_adapter!(simd_logical_and, super::scalar::logical_and);
dual_op_adapter!(simd_logical_or, super::scalar::logical_or);
dual_op_adapter!(simd_logical_xor, super::scalar::logical_xor);
dual_op_adapter!(simd_overlay, super::scalar::overlay);
dual_op_adapter!(simd_hard_light, super::scalar::hard_light);
dual_op_adapter!(simd_soft_light, super::scalar::soft_light);

pub fn simd_add(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(mode);
    let mut pixels = pixels_from_dynimg(img);
    if let PipelineOp::Add {
        other,
        scale,
        offset,
    } = op
    {
        let other_pixels = pixels_from_arc(other)?;
        super::scalar::add(
            &mut pixels,
            mode_code,
            &other_pixels,
            *scale as f32,
            *offset as f32,
        );
    }
    dynimg_from_rgba(pixels, w, h)
}

pub fn simd_subtract(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(mode);
    let mut pixels = pixels_from_dynimg(img);
    if let PipelineOp::Subtract {
        other,
        scale,
        offset,
    } = op
    {
        let other_pixels = pixels_from_arc(other)?;
        super::scalar::subtract(
            &mut pixels,
            mode_code,
            &other_pixels,
            *scale as f32,
            *offset as f32,
        );
    }
    dynimg_from_rgba(pixels, w, h)
}

pub fn simd_blend(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(mode);
    let mut pixels = pixels_from_dynimg(img);
    if let PipelineOp::Blend { other, alpha } = op {
        let other_pixels = pixels_from_arc(other)?;
        super::scalar::blend(&mut pixels, mode_code, &other_pixels, *alpha);
    }
    dynimg_from_rgba(pixels, w, h)
}

pub fn simd_blend_module(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(mode);
    let mut pixels = pixels_from_dynimg(img);
    if let PipelineOp::BlendModule { other, alpha } = op {
        let other_pixels = pixels_from_arc(other)?;
        super::scalar::blend_module(&mut pixels, mode_code, &other_pixels, *alpha);
    }
    dynimg_from_rgba(pixels, w, h)
}

pub fn simd_composite(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(mode);
    let mut pixels = pixels_from_dynimg(img);
    if let PipelineOp::Composite { other, mask } = op {
        let other_pixels = pixels_from_arc(other)?;
        let mask_pixels = pixels_from_arc(mask)?;
        super::scalar::composite(&mut pixels, mode_code, &other_pixels, &mask_pixels);
    }
    dynimg_from_rgba(pixels, w, h)
}

pub fn simd_composite_module(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(mode);
    let pixels = pixels_from_dynimg(img);
    if let PipelineOp::CompositeModule {
        other,
        mask,
        mask_alpha,
    } = op
    {
        let other_img = if mode == Some("P") {
            other.materialize_indices()?
        } else {
            other.materialize_for_ops()?
        };
        let mask_img = mask.materialize_for_ops()?;
        let (other_w, other_h) = other_img.dimensions();
        let (mask_w, mask_h) = mask_img.dimensions();
        let other_pixels = pixels_from_dynimg(&other_img);
        let mask_pixels = pixels_from_dynimg(&mask_img);
        let result = super::scalar::composite_module(
            &pixels,
            w,
            h,
            mode_code,
            &other_pixels,
            other_w,
            other_h,
            &mask_pixels,
            mask_w,
            mask_h,
            *mask_alpha,
        );
        return Ok(preserve_mode(
            &other_img,
            dynimg_from_rgba(result, other_w, other_h)?,
        ));
    }
    Err(PilError::ValueError(
        "expected CompositeModule op".to_owned(),
    ))
}

// ═══════════════════════════════════════════════════════════════════════
// Section F: Ops that change dimensions (return new pixel buffer)
// ═══════════════════════════════════════════════════════════════════════

pub fn simd_transpose(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(mode);
    let method_code: u32 = match op {
        PipelineOp::Transpose { method } => match method {
            TransposeMethod::FlipLeftRight => 0,
            TransposeMethod::FlipTopBottom => 1,
            TransposeMethod::Rotate90 => 2,
            TransposeMethod::Rotate180 => 3,
            TransposeMethod::Rotate270 => 4,
            TransposeMethod::Transpose => 5,
            TransposeMethod::Transverse => 6,
        },
        _ => return Err(PilError::ValueError("expected Transpose op".into())),
    };
    // scalar::transpose modifies pixels in-place for ops 0,1,3 and returns new buffer
    // for ops 2,4,5,6. Pass the actual pixel buffer so in-place ops work correctly.
    let mut pixels = pixels_from_dynimg(img);
    let (result, nw, nh) = super::scalar::transpose(&mut pixels, w, h, mode_code, method_code);
    let final_pixels = if result.is_empty() { pixels } else { result };
    dynimg_from_rgba(final_pixels, nw, nh)
}

// ═══════════════════════════════════════════════════════════════════════
// Section G: New-buffer ops with PipelineOp dispatch
// ═══════════════════════════════════════════════════════════════════════

pub fn simd_resize(
    img: &DynamicImage,
    op: &PipelineOp,
    _mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let pixels = pixels_from_dynimg(img);
    if let PipelineOp::Resize {
        w: dst_w,
        h: dst_h,
        filter,
    } = op
    {
        let mode_code = dynimg_mode(img);
        let f = filter_to_u32(filter);
        let (result, new_w, new_h) =
            super::scalar::resize(&pixels, w, h, *dst_w, *dst_h, mode_code, f);
        dynimg_from_rgba(result, new_w, new_h)
    } else {
        Err(PilError::ValueError("expected Resize op".into()))
    }
}

pub fn simd_thumbnail(
    img: &DynamicImage,
    op: &PipelineOp,
    _mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let pixels = pixels_from_dynimg(img);
    if let PipelineOp::Thumbnail { w: dw, h: dh, .. } = op {
        let mode_code = dynimg_mode(img);
        let (result, nw, nh) = super::scalar::thumbnail(&pixels, w, h, mode_code, *dw, *dh);
        dynimg_from_rgba(result, nw, nh)
    } else {
        Err(PilError::ValueError("expected Thumbnail op".into()))
    }
}

pub fn simd_contain(
    img: &DynamicImage,
    op: &PipelineOp,
    _mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let pixels = pixels_from_dynimg(img);
    if let PipelineOp::Contain { w: dw, h: dh, .. } = op {
        let mode_code = dynimg_mode(img);
        let (result, nw, nh) = super::scalar::contain(&pixels, w, h, mode_code, *dw, *dh);
        dynimg_from_rgba(result, nw, nh)
    } else {
        Err(PilError::ValueError("expected Contain op".into()))
    }
}

pub fn simd_cover(
    img: &DynamicImage,
    op: &PipelineOp,
    _mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let pixels = pixels_from_dynimg(img);
    if let PipelineOp::Cover { w: dw, h: dh, .. } = op {
        let mode_code = dynimg_mode(img);
        let (result, nw, nh) = super::scalar::cover(&pixels, w, h, mode_code, *dw, *dh);
        dynimg_from_rgba(result, nw, nh)
    } else {
        Err(PilError::ValueError("expected Cover op".into()))
    }
}

pub fn simd_fit(
    img: &DynamicImage,
    op: &PipelineOp,
    _mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let pixels = pixels_from_dynimg(img);
    if let PipelineOp::Fit {
        w: dw,
        h: dh,
        bleed,
        centering,
        ..
    } = op
    {
        let mode_code = dynimg_mode(img);
        let (result, nw, nh) = super::scalar::fit(
            &pixels,
            w,
            h,
            mode_code,
            *dw,
            *dh,
            *bleed as f32,
            (centering.0 as f32, centering.1 as f32),
        );
        dynimg_from_rgba(result, nw, nh)
    } else {
        Err(PilError::ValueError("expected Fit op".into()))
    }
}

pub fn simd_scale(
    img: &DynamicImage,
    op: &PipelineOp,
    _mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let pixels = pixels_from_dynimg(img);
    if let PipelineOp::Scale { factor, .. } = op {
        let mode_code = dynimg_mode(img);
        let (result, nw, nh) = super::scalar::scale(&pixels, w, h, mode_code, *factor);
        dynimg_from_rgba(result, nw, nh)
    } else {
        Err(PilError::ValueError("expected Scale op".into()))
    }
}

pub fn simd_pad(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(mode);
    let pixels = pixels_from_dynimg(img);
    if let PipelineOp::Pad {
        w: dw,
        h: dh,
        color,
        ..
    } = op
    {
        let fill = match color {
            Some(c) => pack_rgba(*c),
            None if mode_code == 1 || mode_code == 3 => 0,
            None => 0xFF00_0000u32,
        };
        let (result, nw, nh) = super::scalar::pad(&pixels, w, h, mode_code, *dw, *dh, fill);
        dynimg_from_rgba(result, nw, nh)
    } else {
        Err(PilError::ValueError("expected Pad op".into()))
    }
}

pub fn simd_expand(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(mode);
    let pixels = pixels_from_dynimg(img);
    if let PipelineOp::Expand { border, fill } = op {
        let fill_rgba = pack_rgba(*fill);
        let (result, nw, nh) = super::scalar::expand(&pixels, w, h, mode_code, *border, fill_rgba);
        dynimg_from_rgba(result, nw, nh)
    } else {
        Err(PilError::ValueError("expected Expand op".into()))
    }
}

pub fn simd_crop_border(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(mode);
    let pixels = pixels_from_dynimg(img);
    if let PipelineOp::CropBorder { border } = op {
        let (result, nw, nh) = super::scalar::crop_border(&pixels, w, h, mode_code, *border);
        dynimg_from_rgba(result, nw, nh)
    } else {
        Err(PilError::ValueError("expected CropBorder op".into()))
    }
}

pub fn simd_crop(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(mode);
    let pixels = pixels_from_dynimg(img);
    if let PipelineOp::Crop {
        left,
        top,
        right,
        bottom,
    } = op
    {
        let (result, nw, nh) =
            super::scalar::crop(&pixels, w, h, mode_code, *left, *top, *right, *bottom);
        dynimg_from_rgba(result, nw, nh)
    } else {
        Err(PilError::ValueError("expected Crop op".into()))
    }
}

pub fn simd_rotate(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(mode);
    let pixels = pixels_from_dynimg(img);
    if let PipelineOp::Rotate {
        angle,
        expand,
        fill,
        ..
    } = op
    {
        let fill_rgba = match fill {
            Some(c) => pack_rgba(*c),
            None => 0u32,
        };
        let (result, nw, nh) =
            super::scalar::rotate(&pixels, w, h, mode_code, *angle, *expand, fill_rgba);
        dynimg_from_rgba(result, nw, nh)
    } else {
        Err(PilError::ValueError("expected Rotate op".into()))
    }
}

pub fn simd_reduce(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(mode);
    let pixels = pixels_from_dynimg(img);
    if let PipelineOp::Reduce { x_factor, y_factor } = op {
        let (result, nw, nh) =
            super::scalar::reduce(&pixels, w, h, mode_code, *x_factor, *y_factor);
        dynimg_from_rgba(result, nw, nh)
    } else {
        Err(PilError::ValueError("expected Reduce op".into()))
    }
}

pub fn simd_convert(
    img: &DynamicImage,
    op: &PipelineOp,
    _mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let src_mode = dynimg_mode(img);
    let pixels = pixels_from_dynimg(img);
    if let PipelineOp::Convert { mode: cm, .. } = op {
        let target_mode = color_mode_to_u32(cm);
        let (result, _nw, _nh) = super::scalar::convert(&pixels, w, h, src_mode, target_mode);
        // Reconstruct with proper mode based on target
        let rgba = dynimg_from_rgba(result, w, h)?;
        Ok(rgba)
    } else {
        Err(PilError::ValueError("expected Convert op".into()))
    }
}

pub fn simd_remap_palette(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(mode);
    let pixels = pixels_from_dynimg(img);
    if let PipelineOp::RemapPalette { dest_map } = op {
        let mut inverse = [0u8; 256];
        for (new_index, &old_index) in dest_map.iter().take(256).enumerate() {
            inverse[usize::from(old_index)] = new_index as u8;
        }
        let result = super::scalar::remap_palette(&pixels, mode_code, &inverse);
        Ok(preserve_mode(img, dynimg_from_rgba(result, w, h)?))
    } else {
        Err(PilError::ValueError("expected RemapPalette op".into()))
    }
}

pub fn simd_transform(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    if let PipelineOp::Transform {
        w: dw,
        h: dh,
        method,
        data,
        filter,
        fill,
        palette_fill,
        ..
    } = op
    {
        let resolved_fill = palette_fill.map(|index| (index, 0, 0, 255)).or(*fill);
        if !matches!(method, TransformMethod::Affine) {
            return crate::compute::pool_cpu::ops::effects::op_transform(
                img,
                *dw,
                *dh,
                method,
                data,
                filter,
                resolved_fill,
                mode,
            );
        }
        let mode_code = mode_to_u32(mode);
        let pixels = pixels_from_dynimg(img);
        let matrix: [f64; 8] = {
            let mut arr = [0.0f64; 8];
            let len = data.len().min(8);
            arr[..len].copy_from_slice(&data[..len]);
            arr
        };
        let f = filter_to_u32(filter);
        let fill_rgba = match resolved_fill {
            Some(c) => pack_rgba(c),
            None => 0u32,
        };
        let (result, nw, nh) =
            super::scalar::transform(&pixels, w, h, mode_code, *dw, *dh, &matrix, f, fill_rgba);
        Ok(preserve_mode(img, dynimg_from_rgba(result, nw, nh)?))
    } else {
        Err(PilError::ValueError("expected Transform op".into()))
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Section H: Special/mutating ops
// ═══════════════════════════════════════════════════════════════════════

pub fn simd_put_pixel(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(mode);
    let mut pixels = pixels_from_dynimg(img);
    if let PipelineOp::PutPixel { x, y, color, .. } = op {
        let packed = pack_rgba(*color);
        super::scalar::put_pixel(&mut pixels, w, mode_code, *x, *y, packed);
    }
    // `PutPixel` is mode-preserving in Pillow. Rebuilding every result as
    // RGBA changes the logical mode of an L/LA/RGB pipeline when no explicit
    // mode tag is present, so a following mode-sensitive operation such as
    // ImageOps.colorize observes RGBA and raises instead of receiving L.
    Ok(preserve_mode(img, dynimg_from_rgba(pixels, w, h)?))
}

pub fn simd_put_data(
    img: &DynamicImage,
    op: &PipelineOp,
    _mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let mut pixels = pixels_from_dynimg(img);
    if let PipelineOp::PutData { data, mode } = op {
        super::scalar::put_data(&mut pixels, mode.code(), data);
        return dynimg_from_pixel_mode(pixels, w, h, *mode);
    }
    Err(PilError::ValueError("expected PutData op".into()))
}

pub fn simd_put_alpha(
    img: &DynamicImage,
    op: &PipelineOp,
    _mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let mut pixels = pixels_from_dynimg(img);
    if let PipelineOp::PutAlpha { alpha, mode } = op {
        super::scalar::put_alpha(&mut pixels, mode.code(), *alpha);
        return dynimg_from_put_alpha(pixels, w, h, *mode);
    }
    Err(PilError::ValueError("expected PutAlpha op".into()))
}

pub fn simd_eval(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(mode);
    let mut pixels = pixels_from_dynimg(img);
    if let PipelineOp::Eval { lut } = op {
        let lut_arr: [u8; 1024] = {
            let mut arr = [0u8; 1024];
            let len = lut.len().min(1024);
            arr[..len].copy_from_slice(&lut[..len]);
            arr
        };
        super::scalar::eval(&mut pixels, mode_code, &lut_arr);
    }
    Ok(crate::image::preserve_mode(
        img,
        dynimg_from_rgba(pixels, w, h)?,
    ))
}

pub fn simd_point_op(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(mode);
    let mut pixels = pixels_from_dynimg(img);
    if let PipelineOp::PointOp { lut } = op {
        let lut_arr: [u8; 1024] = {
            let mut arr = [0u8; 1024];
            let len = lut.len().min(1024);
            arr[..len].copy_from_slice(&lut[..len]);
            arr
        };
        super::scalar::point_op(&mut pixels, mode_code, &lut_arr);
    }
    Ok(crate::image::preserve_mode(
        img,
        dynimg_from_rgba(pixels, w, h)?,
    ))
}

pub fn simd_paste(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let mode_code = if mode == Some("P") {
        0
    } else if mode == Some("PA") {
        1
    } else if mode.is_some() {
        mode_to_u32(mode)
    } else {
        dynimg_mode(img)
    };
    let mut pixels = pixels_from_dynimg(img);
    if let PipelineOp::Paste {
        source,
        x,
        y,
        w: _,
        h: _,
        mask,
        mask_alpha,
    } = op
    {
        let src_img = if matches!(mode, Some("P" | "PA")) {
            source.materialize_indices()?
        } else {
            arc_to_dynimg(source)?
        };
        let (src_w, src_h) = src_img.dimensions();
        let src_pixels = pixels_from_dynimg(&src_img);
        let mask_pixels: Option<Vec<u32>> = match mask {
            Some(m) => Some(pixels_from_arc(m)?),
            None => None,
        };
        super::scalar::paste(
            &mut pixels,
            w,
            h,
            mode_code,
            &src_pixels,
            src_w,
            src_h,
            *x,
            *y,
            mask_pixels.as_deref(),
            *mask_alpha,
        );
    }
    Ok(crate::image::preserve_mode(
        img,
        dynimg_from_rgba(pixels, w, h)?,
    ))
}

pub fn simd_alpha_composite(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(mode);
    let mut pixels = pixels_from_dynimg(img);
    if let PipelineOp::AlphaComposite { source, dest, src } = op {
        let src_img = arc_to_dynimg(source)?;
        let (src_w, src_h) = src_img.dimensions();
        let src_pixels = pixels_from_dynimg(&src_img);
        super::scalar::alpha_composite(
            &mut pixels,
            w,
            h,
            mode_code,
            &src_pixels,
            src_w,
            src_h,
            dest.0,
            dest.1,
            src.0,
            src.1,
        );
    }
    dynimg_from_rgba(pixels, w, h)
}

// ═══════════════════════════════════════════════════════════════════════
// Section I: Merge — multi-image band composition
// ═══════════════════════════════════════════════════════════════════════

pub fn simd_merge(
    img: &DynamicImage,
    op: &PipelineOp,
    _mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let mut pixels = pixels_from_dynimg(img);
    if let PipelineOp::Merge {
        mode: merge_mode,
        bands,
    } = op
    {
        // The registry `mode` argument carries the source image's legacy mode
        // tag and is `None` for ordinary Image.merge calls. The operation's
        // ColorMode is the authoritative output mode and also determines how
        // many bands are valid. Using the registry tag here made RGB merges
        // fall through to the RGBA path and index a fourth (nonexistent) band.
        let (mode_code, output_mode) = match merge_mode {
            ColorMode::L => (0, PixelMode::L),
            ColorMode::LA => (1, PixelMode::LA),
            ColorMode::RGB => (2, PixelMode::RGB),
            ColorMode::RGBA => (3, PixelMode::RGBA),
            // CMYK is stored in the packed four-byte representation used by
            // the RGBA SIMD lane, but retains its logical mode at the Image
            // layer through the existing explicit-mode tag.
            ColorMode::CMYK => (3, PixelMode::CMYK),
            _ => {
                return Err(PilError::ValueError(
                    "SIMD merge: unsupported output mode".to_string(),
                ));
            }
        };
        let band_pixels: Vec<Vec<u32>> = bands
            .iter()
            .map(|b| {
                let img = b.materialize_for_ops()?;
                Ok(pixels_from_dynimg(&img))
            })
            .collect::<Result<Vec<_>, PilError>>()?;
        let expected_bands = match mode_code {
            0 => 1,
            1 => 2,
            2 => 3,
            3 => 4,
            _ => unreachable!(),
        };
        if band_pixels.len() != expected_bands
            || band_pixels.iter().any(|band| band.len() != pixels.len())
        {
            return Err(PilError::ValueError(
                "SIMD merge: invalid band shape".to_string(),
            ));
        }
        let band_refs: Vec<&[u32]> = band_pixels.iter().map(|v| v.as_slice()).collect();
        super::scalar::merge(&mut pixels, mode_code, &band_refs);
        return dynimg_from_pixel_mode(pixels, w, h, output_mode);
    }
    Err(PilError::ValueError("expected Merge op".to_string()))
}
