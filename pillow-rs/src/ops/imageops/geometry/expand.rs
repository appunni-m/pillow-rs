use crate::checked_dims::CheckedDims;
use super::callbacks::{expand_geometry_values_with_callbacks, ImageOpsExpandCallbacks};
use super::native_geometry_color_input;
use crate::compute::registry::support::{ExpandFillRule, InputContract, ModeSupport, OperationId};
use crate::error::PilError;
use crate::image::Image;
use crate::ops::imageops::{ImageOpsColor, resolve_imageops_color};
use crate::pipeline::{ExpandMargins, NativeColorInput, PipelineOp};
use crate::raster::{DynamicImage, Mode};

/// A fill resolved for the exact mode of a captured `Image.new` target.
///
/// The Python wrapper resolves names and palette callbacks first, captures
/// the target mode, validates the size, then prepares this mode strategy.
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
    prepare_expand_fill_for_mode(image.mode_identity()?, input)
}

/// Converts and validates an Expand fill against an explicit target mode.
#[doc(hidden)]
pub fn prepare_expand_fill_for_mode(
    mode: Mode,
    input: ImageOpsColor,
) -> Result<PreparedExpandFill, PilError> {
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
    let palette_fill = match &prepared.input {
        ImageOpsColor::PaletteFill { index, palette } => Some((*index, palette.clone())),
        _ => None,
    };
    if let Some((index, fill_palette)) = palette_fill {
        let (palette, palette_alpha) = match image.palette() {
            Some(palette) => (palette, image.palette_alpha()),
            None => (fill_palette, None),
        };
        apply_expand_palette_fill(&mut prepared, index, palette, palette_alpha);
    } else if let Some((index, palette, palette_alpha)) =
        resolve_expand_palette_fill(image, &prepared.input, prepared.fill_rule)?
    {
        apply_expand_palette_fill(&mut prepared, index, palette, palette_alpha);
    } else if let Some(palette) = source_palette {
        prepared.output_palette = Some(palette);
    }
    prepared.palette_resolved = true;
    Ok(prepared)
}

/// Completes the host palette stage for an already captured target mode.
#[doc(hidden)]
pub fn finish_expand_fill_palette_from_host(
    mode: Mode,
    mut prepared: PreparedExpandFill,
) -> Result<PreparedExpandFill, PilError> {
    if mode != prepared.mode {
        return Err(PilError::InternalError(
            "Expand fill was prepared for a different image mode".into(),
        ));
    }
    if prepared.fill_rule == ExpandFillRule::PaletteIndexOrColor {
        let palette_fill = match &prepared.input {
            ImageOpsColor::PaletteFill { index, palette } if mode == Mode::ImagePalette8 => {
                Some((*index, palette.clone()))
            }
            _ => None,
        };
        if let Some((index, palette)) = palette_fill {
            apply_expand_palette_fill(&mut prepared, index, palette, None);
        }
        if let Some(components) = super::color::palette_fill_components(&prepared.input)? {
            match mode {
                Mode::ImagePalette8 if uses_new_palette_color(&prepared.input, &components) => {
                    // Image.new("P", ..., tuple_or_list) creates a temporary
                    // RGB palette when ImageOps did not resolve the fill
                    // through an existing palette callback.
                    let palette = super::color::resolve_palette_tuple_bytes(&components)?;
                    apply_expand_palette_fill(
                        &mut prepared,
                        0,
                        palette.to_vec(),
                        None,
                    );
                }
                Mode::ImagePalette8 => {
                    return Err(PilError::TypeError(
                        "color must be int or single-element tuple".into(),
                    ));
                }
                Mode::ImagePaletteAlpha8 => {
                    return Err(palette_alpha_fill_error(&prepared.input));
                }
                _ => {
                    return Err(PilError::InternalError(
                        "Expand palette fill rule was assigned to a non-palette mode".into(),
                    ));
                }
            }
        }
    }
    prepared.palette_resolved = true;
    Ok(prepared)
}

/// Runs Pillow ImageOps.expand sequencing through host callbacks and typed Rust strategies.
#[doc(hidden)]
pub fn expand_with_callbacks<C: ImageOpsExpandCallbacks>(
    callbacks: &mut C,
    border: &C::Value,
    initial_fill: &C::Value,
) -> Result<C::Output, C::Error>
where
    C::Error: From<PilError>,
{
    let (width, height, left, top) = expand_geometry_values_with_callbacks(callbacks, border)?;
    drop(callbacks.expand_geometry_tuple(&width, &height, &left, &top)?);

    let fill_mode = callbacks.image_mode()?;
    let mut fill = initial_fill.clone();
    if callbacks.value_is_string(&fill)? {
        fill = callbacks.color_from_name(&fill, &fill_mode)?;
    }

    let source_palette = callbacks.image_palette()?;
    let palette = if let Some(source_palette) = source_palette.as_ref() {
        if callbacks.palette_is_truthy(source_palette)? {
            let palette_mode = callbacks.palette_mode(source_palette)?;
            let palette = callbacks.copy_palette(&palette_mode)?;
            if callbacks.value_is_tuple(&fill)? {
                let length = callbacks.value_tuple_length(&fill)?;
                if matches!(length, 3 | 4) {
                    fill = callbacks.palette_get_color(&palette, &fill)?;
                }
            }
            Some((palette, palette_mode))
        } else {
            None
        }
    } else {
        None
    };

    let target_mode = callbacks.image_mode()?;
    let size = callbacks.size_pair(&width, &height)?;
    if callbacks.size_is_negative(&size, 0)? || callbacks.size_is_negative(&size, 1)? {
        return Err(PilError::ValueError("Width and height must be >= 0".into()).into());
    }
    let width = callbacks.size_component_as_c_int(&size, 0)?;
    let height = callbacks.size_component_as_c_int(&size, 1)?;
    if width < 0 || height < 0 {
        return Err(PilError::ValueError("Width and height must be >= 0".into()).into());
    }
    let mode = callbacks.mode_identity(&target_mode)?;
    let size = (width as u32, height as u32);
    callbacks.run_core(move || validate_expand_output_size_for_mode(mode, size))?;

    let input = callbacks.fill_input(mode, &fill)?;
    let prepared = callbacks.run_core(move || {
        let prepared = prepare_expand_fill_for_mode(mode, input)?;
        finish_expand_fill_palette_from_host(mode, prepared)
    })?;
    let constructor = callbacks.capture_wrapper_constructor()?;
    let canvas = callbacks
        .run_core(move || expand_canvas_with_prepared_fill(size, &prepared))?;
    let mut output = callbacks.wrap_canvas(&constructor, canvas)?;
    if let Some((palette, palette_mode)) = palette {
        if callbacks.palette_is_truthy(&palette)? {
            callbacks.put_palette(&mut output, &palette, &palette_mode)?;
        }
    }
    callbacks.paste(&mut output, &left, &top)?;
    callbacks.load_output(&mut output)?;
    Ok(output)
}

fn apply_expand_palette_fill(
    prepared: &mut PreparedExpandFill,
    index: u8,
    palette: Vec<u8>,
    palette_alpha: Option<Vec<u8>>,
) {
    prepared.fill = (index, index, index, u8::MAX);
    prepared.native_fill = None;
    prepared.output_palette = Some((Some(palette), palette_alpha));
}

fn uses_new_palette_color(
    input: &ImageOpsColor,
    components: &[crate::image::PaletteColorComponent],
) -> bool {
    if !matches!(components.len(), 3 | 4) {
        return false;
    }
    let integer_components = components.iter().all(|component| {
        matches!(
            component,
            crate::image::PaletteColorComponent::Integer(_)
                | crate::image::PaletteColorComponent::WideInteger { .. }
        )
    });
    match input {
        ImageOpsColor::Name(_)
        | ImageOpsColor::Components(_)
        | ImageOpsColor::TupleComponents(_)
        | ImageOpsColor::ListComponents(_) => true,
        ImageOpsColor::TupleNumericComponents(_)
        | ImageOpsColor::TupleComponentsWithFallback { .. }
        | ImageOpsColor::WideTupleComponents { .. }
        | ImageOpsColor::ListNumericComponents(_) => integer_components,
        _ => false,
    }
}

fn palette_alpha_fill_error(input: &ImageOpsColor) -> PilError {
    if matches!(
        input,
        ImageOpsColor::ListComponents(_) | ImageOpsColor::ListNumericComponents(_)
    ) {
        PilError::TypeError("color must be int or tuple".into())
    } else {
        PilError::TypeError("color must be int, or tuple of one or two elements".into())
    }
}

/// Checks the allocation dimensions against the mode captured for `Image.new`.
#[doc(hidden)]
pub fn validate_prepared_expand_output_size(
    prepared: &PreparedExpandFill,
    size: (u32, u32),
) -> Result<(), PilError> {
    validate_expand_output_size_for_mode(prepared.mode, size)
}

#[doc(hidden)]
pub fn validate_expand_output_size_for_mode(
    mode: Mode,
    size: (u32, u32),
) -> Result<(), PilError> {
    CheckedDims::new_allow_empty(size.0, size.1, expand_bytes_per_pixel(mode)?)?;
    Ok(())
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

fn resolve_expand_palette_fill(
    image: &Image,
    fill: &ImageOpsColor,
    fill_rule: ExpandFillRule,
) -> Result<Option<(u8, Vec<u8>, Option<Vec<u8>>)>, PilError> {
    // The Python adapter resolves P/PA colors against a fresh palette snapshot
    // before preparing this Rust mode strategy. This retains source palette
    // metadata for routes that do not perform the host tuple allocation.
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
