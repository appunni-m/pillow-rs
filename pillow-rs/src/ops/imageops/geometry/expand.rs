use crate::checked_dims::CheckedDims;
use super::native_geometry_color_input;
use crate::compute::registry::support::{ExpandFillRule, InputContract, ModeSupport, OperationId};
use crate::error::PilError;
use crate::image::Image;
use crate::ops::imageops::{ImageOpsColor, resolve_imageops_color};
use crate::pipeline::{ExpandMargins, NativeColorInput, PipelineOp};
use crate::raster::{DynamicImage, Mode};

/// A fill resolved for the exact mode of a captured `Image.new` target.
///
/// The Python wrapper resolves names and P-palette callbacks first, captures
/// the target image, validates the size, then prepares this mode strategy.
#[doc(hidden)]
#[derive(Debug, Clone)]
pub struct PreparedExpandFill {
    mode: Mode,
    input: ImageOpsColor,
    fill_rule: ExpandFillRule,
    fill: (u8, u8, u8, u8),
    native_fill: Option<NativeColorInput>,
    output_palette: Option<(Option<Vec<u8>>, Option<Vec<u8>>)>,
    palette_resolved: bool,
}

/// Converts and validates an Expand fill against the captured image's mode.
#[doc(hidden)]
pub fn prepare_expand_fill(
    image: &Image,
    input: ImageOpsColor,
) -> Result<PreparedExpandFill, PilError> {
    let mode = image.mode_identity()?;
    let fill_rule = expand_fill_rule(mode)?;
    let palette_components = if fill_rule == ExpandFillRule::PaletteIndexOrColor {
        super::color::palette_fill_components(&input)?
    } else {
        None
    };
    let fill = if palette_components.is_some() {
        (0, 0, 0, u8::MAX)
    } else {
        resolve_imageops_color(input.clone(), mode)?.unwrap_or((0, 0, 0, 0))
    };
    let native_fill = match fill_rule {
        ExpandFillRule::NativeU16 | ExpandFillRule::NativeF32 => {
            native_geometry_color_input(&input)
        }
        ExpandFillRule::ModeEncodedBytes | ExpandFillRule::PaletteIndexOrColor => None,
    };
    Ok(PreparedExpandFill {
        mode,
        input,
        fill_rule,
        fill,
        native_fill,
        output_palette: None,
        palette_resolved: false,
    })
}

/// Resolves palette colors after the host has committed pending palette data.
#[doc(hidden)]
pub fn resolve_expand_fill_palette(
    image: &Image,
    mut prepared: PreparedExpandFill,
) -> Result<PreparedExpandFill, PilError> {
    let mode = image.mode_identity()?;
    if mode != prepared.mode {
        return Err(PilError::InternalError(
            "Expand fill was prepared for a different image mode".into(),
        ));
    }
    let source_palette = matches!(mode, Mode::ImagePalette8 | Mode::ImagePaletteAlpha8)
        .then(|| (image.palette(), image.palette_alpha()));
    if let Some((index, palette, palette_alpha)) =
        resolve_expand_palette_fill(image, &prepared.input, prepared.fill_rule)?
    {
        prepared.fill = (index, index, index, u8::MAX);
        prepared.native_fill = None;
        prepared.output_palette = Some((Some(palette), palette_alpha));
    } else if let Some(palette) = source_palette {
        prepared.output_palette = Some(palette);
    }
    prepared.palette_resolved = true;
    Ok(prepared)
}

/// Checks the allocation dimensions against the mode captured for `Image.new`.
#[doc(hidden)]
pub fn validate_prepared_expand_output_size(
    prepared: &PreparedExpandFill,
    size: (u32, u32),
) -> Result<(), PilError> {
    validate_expand_output_size_for_mode(prepared.mode, size)
}

fn validate_expand_output_size_for_mode(mode: Mode, size: (u32, u32)) -> Result<(), PilError> {
    CheckedDims::new_allow_empty(size.0, size.1, expand_bytes_per_pixel(mode)?)?;
    Ok(())
}

/// Validates the allocation size against an already captured Image.new source.
///
/// The host wrapper snapshots this image before effectful Python size
/// comparisons, matching the mode value evaluated for `Image.new`.
#[doc(hidden)]
pub fn validate_expand_output_size(image: &Image, size: (u32, u32)) -> Result<(), PilError> {
    validate_expand_output_size_for_mode(image.mode_identity()?, size)
}

/// Creates and materializes the filled canvas before the source is loaded.
///
/// ImageOps.expand follows Pillow's Image.new-then-paste order. The host
/// wrapper uses this entry point to preserve allocation faults before source
/// decoding while it delegates the source conversion and copy to Image.paste.
#[doc(hidden)]
pub fn expand_canvas_with_prepared_fill(
    size: (u32, u32),
    prepared: &PreparedExpandFill,
) -> Result<Image, PilError> {
    validate_prepared_expand_output_size(prepared, size)?;
    if !prepared.palette_resolved {
        return Err(PilError::InternalError(
            "Expand palette fill was not resolved before canvas allocation".into(),
        ));
    }
    let canvas = empty_expand_canvas(prepared.mode, prepared.fill)?;
    let filled = expand_with_prepared_fill(
        &canvas,
        ExpandMargins {
            left: 0,
            top: 0,
            right: i64::from(size.0),
            bottom: i64::from(size.1),
        },
        prepared,
    )?;
    filled.materialized_branch()
}

fn empty_expand_canvas(mode: Mode, fill: (u8, u8, u8, u8)) -> Result<Image, PilError> {
    if mode == Mode::ImagePalette8 {
        return Ok(Image::new_palette_index(0, 0, fill.0));
    }
    let image = match mode {
        Mode::ImageLumaA16 => DynamicImage::ImageLumaA16(
            crate::raster::ImageBuffer::from_pixel(0, 0, crate::raster::LumaA([0, 0])),
        ),
        Mode::ImageRgb16 => DynamicImage::ImageRgb16(
            crate::raster::ImageBuffer::from_pixel(0, 0, crate::raster::Rgb([0, 0, 0])),
        ),
        Mode::ImageRgba16 => DynamicImage::ImageRgba16(
            crate::raster::ImageBuffer::from_pixel(0, 0, crate::raster::Rgba([0, 0, 0, 0])),
        ),
        Mode::ImageRgb32F => DynamicImage::ImageRgb32F(
            crate::raster::Rgb32FImage::from_pixel(0, 0, crate::raster::Rgb([0.0, 0.0, 0.0])),
        ),
        Mode::ImageRgba32F => DynamicImage::ImageRgba32F(
            crate::raster::Rgba32FImage::from_pixel(
                0,
                0,
                crate::raster::Rgba([0.0, 0.0, 0.0, 0.0]),
            ),
        ),
        _ => return Image::new(0, 0, mode, fill),
    };
    Ok(Image::from_dynamic(image))
}

/// Adds a border around an image.
///
/// # Errors
///
/// Currently returns `Ok(Image)`; deferred pipeline execution reports later
/// materialization failures.
pub fn expand(image: &Image, border: u32, fill: (u8, u8, u8, u8)) -> Result<Image, PilError> {
    expand_with_margins(image, ExpandMargins::uniform(border), fill)
}

/// Adds independent margins around an image.
pub fn expand_with_margins(
    image: &Image,
    border: ExpandMargins,
    fill: (u8, u8, u8, u8),
) -> Result<Image, PilError> {
    Ok(Image::push_op(image, PipelineOp::Expand { border, fill }))
}

/// Expands an image after resolving the fill against its native Pillow mode.
///
/// Keeping the host value intact until the mode is known preserves native
/// component counts and meanings: CMYK's fourth component is K, RGBX's is X,
/// and a three-component color receives Pillow's opaque fourth byte for both.
pub fn expand_with_input(
    image: &Image,
    border: u32,
    fill: ImageOpsColor,
) -> Result<Image, PilError> {
    expand_with_margins_and_input(image, ExpandMargins::uniform(border), fill)
}

/// Resolves a fill and adds independent margins around an image.
pub fn expand_with_margins_and_input(
    image: &Image,
    border: ExpandMargins,
    fill: ImageOpsColor,
) -> Result<Image, PilError> {
    let prepared = prepare_expand_fill(image, fill)?;
    let prepared = resolve_expand_fill_palette(image, prepared)?;
    expand_with_prepared_fill(image, border, &prepared)
}

fn expand_with_prepared_fill(
    image: &Image,
    border: ExpandMargins,
    prepared: &PreparedExpandFill,
) -> Result<Image, PilError> {
    let mode = image.mode_identity()?;
    if mode != prepared.mode {
        return Err(PilError::InternalError(
            "Expand fill was prepared for a different image mode".into(),
        ));
    }
    if !prepared.palette_resolved {
        return Err(PilError::InternalError(
            "Expand palette fill was not resolved before execution".into(),
        ));
    }
    let mut output = match prepared.native_fill {
        Some(input) => Image::push_op(
            image,
            PipelineOp::ExpandTyped {
                border,
                fill: prepared.fill,
                input,
            },
        ),
        None => expand_with_margins(image, border, prepared.fill)?,
    };
    if let Some((palette, palette_alpha)) = prepared.output_palette.clone() {
        output.set_pipeline_output_palette(palette, palette_alpha);
    }
    Ok(output)
}

fn expand_bytes_per_pixel(mode: Mode) -> Result<u8, PilError> {
    let inputs = match OperationId::Expand.support().for_mode(mode) {
        ModeSupport::Supported { inputs, .. } => inputs,
        ModeSupport::Rejected { reason, kind, .. } => return Err(kind.into_error(reason)),
    };
    inputs
        .iter()
        .find_map(|input| match input {
            InputContract::ExpandedDimensions { bytes_per_pixel } => Some(*bytes_per_pixel),
            _ => None,
        })
        .ok_or_else(|| {
            PilError::InternalError("Expand support omitted its exact-mode dimensions".into())
        })
}

fn expand_trailing_margin(
    output_extent: u32,
    source_extent: u32,
    leading_margin: i64,
    axis: &str,
) -> Result<i64, PilError> {
    i64::try_from(
        i128::from(output_extent) - i128::from(source_extent) - i128::from(leading_margin),
    )
    .map_err(|_| PilError::DimensionError(format!("expanded image {axis} is invalid")))
}

fn resolve_expand_palette_fill(
    image: &Image,
    fill: &ImageOpsColor,
    fill_rule: ExpandFillRule,
) -> Result<Option<(u8, Vec<u8>, Option<Vec<u8>>)>, PilError> {
    // The Python adapter resolves P colors against its copied palette before
    // preparing this Rust mode strategy. This retains source palette metadata
    // for P/PA routes that do not perform the host tuple allocation.
    // TODO(pillow-rs#3ptb): verify dirty/raw palette faults, malformed buffers,
    // palette output metadata, and the documented CoreOnly fill extensions.
    if fill_rule != ExpandFillRule::PaletteIndexOrColor {
        return Ok(None);
    }
    let Some(components) = super::color::palette_fill_components(fill)? else {
        return Ok(None);
    };
    let (index, palette, palette_alpha) = image.resolve_palette_fill_color(&components)?;
    Ok(Some((index, palette, palette_alpha)))
}

fn expand_fill_rule(mode: crate::raster::Mode) -> Result<ExpandFillRule, PilError> {
    let inputs = match OperationId::Expand.support().for_mode(mode) {
        ModeSupport::Supported { inputs, .. } => inputs,
        ModeSupport::Rejected { reason, kind, .. } => return Err(kind.into_error(reason)),
    };
    inputs
        .iter()
        .find_map(|input| match input {
            InputContract::ExpandFill(rule) => Some(*rule),
            _ => None,
        })
        .ok_or_else(|| {
            PilError::InternalError("Expand support omitted its exact-mode fill rule".into())
        })
}
