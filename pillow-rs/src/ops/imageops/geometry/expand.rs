use crate::checked_dims::CheckedDims;
use super::native_geometry_color_input;
use crate::compute::registry::support::{ExpandFillRule, InputContract, OperationId};
use crate::error::PilError;
use crate::image::Image;
use crate::ops::imageops::{ImageOpsColor, resolve_imageops_color};
use crate::pipeline::{ExpandMargins, NativeColorInput, PipelineOp};
use crate::raster::Mode;

/// A fill converted against the source mode before Expand geometry is checked.
///
/// The Python binding carries this through Pillow's fill, palette, and
/// `Image.new` size stages.
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

/// Converts and validates an Expand fill against the source's exact mode.
#[doc(hidden)]
pub fn prepare_expand_fill(
    image: &Image,
    input: ImageOpsColor,
) -> Result<PreparedExpandFill, PilError> {
    let mode = image.mode_identity()?;
    let fill_rule = expand_fill_rule(mode)?;
    let palette_components = expand_palette_components(&input, fill_rule)?;
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
    if let Some((index, palette, palette_alpha)) =
        resolve_expand_palette_fill(image, &prepared.input, prepared.fill_rule)?
    {
        prepared.fill = (index, index, index, u8::MAX);
        prepared.native_fill = None;
        prepared.output_palette = Some((Some(palette), palette_alpha));
    }
    prepared.palette_resolved = true;
    Ok(prepared)
}

/// Checks the allocation dimensions that Pillow validates in `Image.new`.
#[doc(hidden)]
pub fn validate_expand_output_size(image: &Image, size: (u32, u32)) -> Result<(), PilError> {
    let mode = image.mode_identity()?;
    CheckedDims::new_allow_empty(size.0, size.1, expand_bytes_per_pixel(mode)?)?;
    Ok(())
}

/// Queues Expand with dimensions and placement already resolved by the host.
///
/// The explicit output size preserves Python's arithmetic result, including
/// custom numeric border objects. The source still runs through its exact-mode
/// CPU Expand strategy when the lazy pipeline is materialized.
#[doc(hidden)]
pub fn expand_with_output_size_and_position(
    image: &Image,
    size: (u32, u32),
    position: (i64, i64),
    prepared: &PreparedExpandFill,
) -> Result<Image, PilError> {
    validate_expand_output_size(image, size)?;
    let mode = image.mode_identity()?;
    if mode != prepared.mode {
        return Err(PilError::InternalError(
            "Expand fill was prepared for a different image mode".into(),
        ));
    }
    if !prepared.palette_resolved {
        return Err(PilError::InternalError(
            "Expand palette fill was not resolved before geometry".into(),
        ));
    }
    let right = expand_trailing_margin(size.0, image.width(), position.0, "width")?;
    let bottom = expand_trailing_margin(size.1, image.height(), position.1, "height")?;
    let border = ExpandMargins {
        left: position.0,
        top: position.1,
        right,
        bottom,
    };
    expand_with_prepared_fill(image, border, prepared)
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
    OperationId::Expand
        .support()
        .for_mode(mode)
        .inputs
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
    // The Python adapter converts the fill before committing a dirty palette,
    // then this stage looks up or appends the color in that committed snapshot.
    // TODO(pillow-rs#3ptb): verify dirty/raw palette error ordering, malformed
    // palette buffers, allocation order, and output metadata in CPU parity.
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
    OperationId::Expand
        .support()
        .for_mode(mode)
        .inputs
        .iter()
        .find_map(|input| match input {
            InputContract::ExpandFill(rule) => Some(*rule),
            _ => None,
        })
        .ok_or_else(|| {
            PilError::InternalError("Expand support omitted its exact-mode fill rule".into())
        })
}
