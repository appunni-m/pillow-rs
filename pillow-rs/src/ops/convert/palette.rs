use std::borrow::Cow;
use std::sync::Arc;

use crate::color;
use crate::error::PilError;
use crate::image::{Image, PipelineOps};
use crate::pipeline::DitherMethod;
use crate::raster::DynamicImage;

/// Per-entry alpha table Pillow applies when converting a palette image to an
/// alpha band: a single transparent index becomes 0 with everything else 255,
/// a PNG `tRNS` table is used verbatim.
pub(super) fn palette_alpha_for_convert(img: &Image) -> Option<Vec<u8>> {
    if let Some(transparency) = img.pending_palette_transparency() {
        return match transparency {
            crate::image::PaletteTransparency::Index(index) => {
                let mut table = vec![255u8; 256];
                table[usize::from(index)] = 0;
                Some(table)
            }
            crate::image::PaletteTransparency::Table(alpha) => Some(alpha),
        };
    }
    // `putpalette(..., rawmode="RGBA")` stores attached alpha on the
    // committed palette rather than in pending `info["transparency"]`.
    // Pillow's P->LA conversion consumes both representations.
    img.palette_alpha().filter(|alpha| !alpha.is_empty())
}

/// Convert a source image to Pillow's palette-plus-alpha representation.
///
/// Pillow's `Convert.c` treats `PA` as an indexed destination, not as a
/// spelling of `LA`: RGB-family and CMYK/HSV/YCbCr sources are mapped through
/// the default WEB palette, RGBA keeps the source alpha bytes, and scalar or
/// indexed sources retain their byte/index values. Scalar sources also receive
/// Pillow's identity grayscale palette, so a later `PA` expansion resolves an
/// index back to the original scalar value. Keeping those paths explicit also
/// avoids feeding `PA` into the ordinary scalar pipeline, which has no palette
/// storage to attach to its result.
fn identity_grayscale_palette() -> Vec<u8> {
    (0u8..=u8::MAX)
        .flat_map(|value| [value, value, value])
        .collect()
}

/// Splits RGBA bytes for Pillow's RGBA-to-PA conversion without widening or
/// copying an already materialized RGBA source.  Pillow quantizes RGB only,
/// then carries the original alpha bytes into the PA result.
pub(super) fn split_rgba_for_palette(
    source: &DynamicImage,
) -> Result<(crate::raster::RgbImage, Vec<u8>), PilError> {
    let rgba: Cow<'_, crate::raster::RgbaImage> = match source {
        DynamicImage::ImageRgba8(rgba) => Cow::Borrowed(rgba),
        other => Cow::Owned(other.to_rgba8()),
    };
    let width = rgba.width();
    let height = rgba.height();
    let pixel_count = (width as usize)
        .checked_mul(height as usize)
        .ok_or_else(|| PilError::ValueError("palette conversion failed".to_owned()))?;
    let rgb_len = pixel_count
        .checked_mul(3)
        .ok_or_else(|| PilError::ValueError("palette conversion failed".to_owned()))?;
    let rgba_len = pixel_count
        .checked_mul(4)
        .ok_or_else(|| PilError::ValueError("palette conversion failed".to_owned()))?;
    let input = rgba
        .as_raw()
        .get(..rgba_len)
        .ok_or_else(|| PilError::ValueError("palette conversion failed".to_owned()))?;
    let mut rgb = Vec::with_capacity(rgb_len);
    let mut alpha = Vec::with_capacity(pixel_count);
    for pixel in input.chunks_exact(4) {
        rgb.extend_from_slice(&pixel[..3]);
        alpha.push(pixel[3]);
    }
    let rgb = crate::raster::RgbImage::from_raw(width, height, rgb)
        .ok_or_else(|| PilError::ValueError("palette conversion failed".to_owned()))?;
    Ok((rgb, alpha))
}

pub(super) fn convert_to_palette_alpha(
    image: &Image,
    source_mode: &str,
    dither: Option<DitherMethod>,
) -> Result<Image, PilError> {
    // Keep shared source storage for read-only conversions while preserving
    // the logical-mode validation performed by Image::materialize().
    let source = image.materialized_shared_for_ops()?;
    let (indices, palette, alpha) = match source_mode {
        "P" => {
            let indices = source.to_luma8();
            let table = palette_alpha_for_convert(image);
            let alpha = indices
                .pixels()
                .map(|pixel| {
                    table
                        .as_ref()
                        .and_then(|values| values.get(usize::from(pixel[0])))
                        .copied()
                        .unwrap_or(255)
                })
                .collect();
            (indices, image.palette(), alpha)
        }
        "1" | "L" | "LA" => {
            let indices = source.to_luma8();
            let alpha = vec![255; indices.width() as usize * indices.height() as usize];
            (indices, Some(identity_grayscale_palette()), alpha)
        }
        "I" => {
            // I->P is a direct clamped scalar-to-index conversion.  The
            // intermediate RGB helper preserves that value without applying
            // the I->L scaling formula used by ordinary grayscale conversion.
            let indices = color::i_to_rgb(&source).to_luma8();
            let alpha = vec![255; indices.width() as usize * indices.height() as usize];
            (indices, Some(identity_grayscale_palette()), alpha)
        }
        "F" => {
            // F->P truncates the clamped float value to an index, matching the
            // existing F->RGB helper because all three broadcast channels are
            // identical.
            let indices = color::f_to_rgb(&source).to_luma8();
            let alpha = vec![255; indices.width() as usize * indices.height() as usize];
            (indices, Some(identity_grayscale_palette()), alpha)
        }
        "RGBA" => {
            // Pillow's Python conversion splits RGBA into RGB and alpha,
            // quantizes only the RGB image, then merges the original alpha
            // band into PA.  Quantizing RGBA and moving transparent entries
            // afterward produces different palette-index order for valid
            // images with distinct transparent and visible colors.
            let (rgb, alpha) = split_rgba_for_palette(source.as_ref())?;
            let (width, height) = rgb.dimensions();
            let quantized =
                Image::from_dynamic(DynamicImage::ImageRgb8(rgb), Some("RGB".to_owned()))
                    // Image.convert("PA") calls RGB.quantize() without a method;
                    // RGB therefore uses MEDIANCUT (method 0), not RGBA's FASTOCTREE
                    // default.  The palette-index order is observable in PA bytes.
                    .quantize(256, 0, None, true, 0)?;
            let index_bytes = quantized.materialize()?.to_luma8().into_raw();
            let palette = quantized.palette();
            let indices = crate::raster::GrayImage::from_raw(width, height, index_bytes)
                .ok_or_else(|| PilError::ValueError("palette conversion failed".to_owned()))?;
            (indices, palette, alpha)
        }
        "CMYK" | "HSV" | "YCbCr" => {
            let source_palette = image.palette();
            let rgb =
                color::convert_from_nonstandard(source_mode, &source, source_palette.as_deref())
                    .unwrap_or_else(|| source.to_rgb8().into())
                    .to_rgb8();
            let (width, height) = rgb.dimensions();
            let dither = !matches!(dither, Some(DitherMethod::None));
            let (indices, palette) = crate::ops::quantize::web_palette_quantize(
                &rgb.clone().into_raw(),
                width,
                height,
                dither,
            )?;
            let indices = crate::raster::GrayImage::from_raw(width, height, indices)
                .ok_or_else(|| PilError::ValueError("palette conversion failed".to_owned()))?;
            let alpha = vec![255; width as usize * height as usize];
            (indices, Some(palette), alpha)
        }
        _ => {
            let rgb = source.to_rgb8();
            let (width, height) = rgb.dimensions();
            let dither = !matches!(dither, Some(DitherMethod::None));
            let (indices, palette) = crate::ops::quantize::web_palette_quantize(
                &rgb.clone().into_raw(),
                width,
                height,
                dither,
            )?;
            let indices = crate::raster::GrayImage::from_raw(width, height, indices)
                .ok_or_else(|| PilError::ValueError("palette conversion failed".to_owned()))?;
            let alpha = vec![255; width as usize * height as usize];
            (indices, Some(palette), alpha)
        }
    };

    let (width, height) = indices.dimensions();
    let index_bytes = indices.into_raw();
    let mut pa = crate::raster::GrayAlphaImage::new(width, height);
    for (position, output) in pa.pixels_mut().enumerate() {
        output[0] = index_bytes[position];
        output[1] = alpha.get(position).copied().unwrap_or(255);
    }

    Ok(Image::Loaded(crate::image::LoadedData {
        image: std::sync::Arc::new(DynamicImage::ImageLumaA8(pa)),
        mode: crate::image::Mode::ImagePaletteAlpha8,
        known_zero_fill: false,
        explicit_mode: Some("PA".to_owned()),
        decoded_mode: crate::raster::ColorType::La8.into(),
        palette,
        palette_alpha: None,
        // Pillow's mode conversion returns a detached image and does not
        // carry the source decoder format or transient info dictionary into
        // the result.  Retain only EXIF, matching the existing P/PA path.
        source_format: None,
        info: None,
        exif: image.exif_metadata(),
    }))
}

pub(super) fn convert_to_web_palette(
    image: &Image,
    source_mode: &str,
    dither_enum: Option<DitherMethod>,
) -> Result<Image, PilError> {
    if source_mode == "RGBA" {
        return image.quantize(256, 0, None, true, 2);
    }
    let source = image.materialize()?;
    let (width, height) = (source.width(), source.height());
    let (indices, palette_bytes) = if matches!(source_mode, "1" | "L" | "LA") {
        // Pillow maps L/1 samples directly to P indices and installs the
        // identity grayscale palette; LA drops alpha on this path.
        let indices = source.to_luma8().into_raw();
        let palette = identity_grayscale_palette();
        (indices, palette)
    } else {
        let rgb_raw = source.to_rgb8().into_raw();
        let dither = !matches!(dither_enum, Some(DitherMethod::None));
        crate::ops::quantize::web_palette_quantize(&rgb_raw, width, height, dither)?
    };
    let mut output = crate::raster::GrayImage::new(width, height);
    for (index, pixel) in output.pixels_mut().enumerate() {
        pixel[0] = indices.get(index).copied().unwrap_or(0);
    }
    let source = crate::image::LoadedData {
        image: Arc::new(DynamicImage::ImageLuma8(output)),
        mode: crate::image::Mode::ImagePalette8,
        known_zero_fill: false,
        explicit_mode: Some("P".to_owned()),
        decoded_mode: crate::raster::ColorType::L8.into(),
        palette: None,
        palette_alpha: None,
        source_format: image.source_format(),
        info: image.image_info(),
        exif: image.exif_metadata(),
    };
    Ok(Image::Pipeline {
        source: Arc::new(Image::Loaded(source)),
        ops: PipelineOps::empty(),
        format: None,
        explicit_mode: Some("P".to_owned()),
        mode_selector: Some(crate::image::Mode::ImagePalette8),
        backend: None,
        palette: Some(palette_bytes),
        palette_alpha: None,
        materialized: crate::image::materialization_cache(),
        shape: crate::image::pipeline_shape_cache(),
        mode: crate::image::pipeline_mode_cache(),
    })
}
