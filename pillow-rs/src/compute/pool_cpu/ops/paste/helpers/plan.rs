use std::sync::Arc;

use crate::compute::registry::support::StrategySelection;
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

use super::layout::{MaskLayout, SourceLayout, byte_channels, mask_layout, source_layout};
use super::region::PasteRegion;

/// Prepared byte-oriented Paste inputs for one exact destination mode.
pub(crate) struct BytePlan<'a> {
    pub(crate) destination: &'a [u8],
    pub(crate) destination_size: (u32, u32),
    pub(crate) source: Arc<DynamicImage>,
    pub(crate) source_size: (u32, u32),
    pub(crate) region_size: (u32, u32),
    pub(crate) source_layout: SourceLayout,
    pub(crate) broadcast_source: bool,
    pub(crate) mask: Option<Arc<DynamicImage>>,
    pub(crate) mask_layout: Option<MaskLayout>,
    pub(crate) region: PasteRegion,
    pub(crate) selection: StrategySelection,
}

/// Prepared typed I;16 inputs, retaining the mode-specific stored byte order.
pub(crate) struct Luma16Plan<'a> {
    pub(crate) destination: &'a [u16],
    pub(crate) destination_size: (u32, u32),
    pub(crate) source: Arc<DynamicImage>,
    pub(crate) source_size: (u32, u32),
    pub(crate) mask: Option<Arc<DynamicImage>>,
    pub(crate) mask_layout: Option<MaskLayout>,
    pub(crate) region: PasteRegion,
    pub(crate) mode: Mode,
    pub(crate) selection: StrategySelection,
}

/// Prepared core-only native u16 samples for a high-depth internal mode.
pub(crate) struct NativeU16Plan<'a> {
    pub(crate) destination: &'a [u16],
    pub(crate) destination_size: (u32, u32),
    pub(crate) source_size: (u32, u32),
    pub(crate) source: Arc<DynamicImage>,
    pub(crate) mask: Option<Arc<DynamicImage>>,
    pub(crate) mask_layout: Option<MaskLayout>,
    pub(crate) region: PasteRegion,
    pub(crate) selection: StrategySelection,
}

/// Prepared core-only native f32 samples for an internal floating mode.
pub(crate) struct NativeF32Plan<'a> {
    pub(crate) destination: &'a [f32],
    pub(crate) destination_size: (u32, u32),
    pub(crate) source_size: (u32, u32),
    pub(crate) source: Arc<DynamicImage>,
    pub(crate) mask: Option<Arc<DynamicImage>>,
    pub(crate) mask_layout: Option<MaskLayout>,
    pub(crate) region: PasteRegion,
    pub(crate) selection: StrategySelection,
}

pub(crate) fn byte_plan<'a>(
    destination: &'a [u8],
    destination_size: (u32, u32),
    destination_mode: Mode,
    source: Arc<DynamicImage>,
    mask: Option<Arc<DynamicImage>>,
    mask_alpha: bool,
    position: (i64, i64),
    region_size: (u32, u32),
    selection: StrategySelection,
) -> Result<BytePlan<'a>, PilError> {
    let source_size = source.dimensions();
    let source_mode = source.mode();
    let source_layout = source_layout(destination_mode, source_mode).ok_or_else(|| {
        PilError::InternalError("Paste source strategy did not match destination mode".into())
    })?;
    let source_channels = match source_layout {
        SourceLayout::Native => byte_channels(source_mode),
        SourceLayout::LumaAlphaToRgb => Some(2),
        SourceLayout::FourBytesToRgb => Some(4),
        SourceLayout::RgbToRgba => Some(3),
    }
    .ok_or_else(|| PilError::InternalError("Paste source has no byte layout".into()))?;
    check_byte_len(source.as_bytes(), source_size, source_channels, "source")?;
    check_byte_len(
        destination,
        destination_size,
        byte_channels(destination_mode).ok_or_else(|| {
            PilError::InternalError("Paste destination has no byte layout".into())
        })?,
        "destination",
    )?;

    let mask_layout = mask.as_deref().map(|image| {
        mask_layout(image.mode(), mask_alpha).ok_or_else(|| {
            PilError::InternalError("Paste mask strategy did not match mask mode".into())
        })
    }).transpose()?;
    if let (Some(image), Some(layout)) = (mask.as_deref(), mask_layout) {
        check_byte_len(image.as_bytes(), image.dimensions(), layout.channels, "mask")?;
        if image.dimensions() != region_size {
            return Err(PilError::InternalError("Paste mask dimensions changed after admission".into()));
        }
    }
    let broadcast_source = source_size != region_size;
    if broadcast_source && source_size != (1, 1) {
        return Err(PilError::InternalError("Paste source dimensions changed after admission".into()));
    }
    let region = PasteRegion::clipped(position.0, position.1, region_size, destination_size);
    Ok(BytePlan {
        destination,
        destination_size,
        source,
        source_size,
        region_size,
        source_layout,
        broadcast_source,
        mask,
        mask_layout,
        region,
        selection,
    })
}

pub(crate) fn luma16_plan<'a>(
    destination: &'a [u16],
    destination_size: (u32, u32),
    mode: Mode,
    source: Arc<DynamicImage>,
    mask: Option<Arc<DynamicImage>>,
    mask_alpha: bool,
    position: (i64, i64),
    region_size: (u32, u32),
    selection: StrategySelection,
) -> Result<Luma16Plan<'a>, PilError> {
    if source.mode() != mode || source.dimensions() != region_size {
        return Err(PilError::InternalError("I;16 Paste requires an exact same-mode source".into()));
    }
    let mask_layout = mask.as_deref().map(|image| {
        mask_layout(image.mode(), mask_alpha).ok_or_else(|| {
            PilError::InternalError("Paste mask strategy did not match mask mode".into())
        })
    }).transpose()?;
    if let (Some(image), Some(layout)) = (mask.as_deref(), mask_layout) {
        check_byte_len(image.as_bytes(), image.dimensions(), layout.channels, "mask")?;
        if image.dimensions() != region_size {
            return Err(PilError::InternalError("Paste mask dimensions changed after admission".into()));
        }
    }
    Ok(Luma16Plan {
        destination,
        destination_size,
        source,
        source_size: region_size,
        mask,
        mask_layout,
        region: PasteRegion::clipped(position.0, position.1, region_size, destination_size),
        mode,
        selection,
    })
}

pub(crate) fn native_u16_plan<'a>(
    destination: &'a [u16],
    destination_size: (u32, u32),
    mode: Mode,
    channels: usize,
    source: Arc<DynamicImage>,
    mask: Option<Arc<DynamicImage>>,
    mask_alpha: bool,
    position: (i64, i64),
    region_size: (u32, u32),
    selection: StrategySelection,
) -> Result<NativeU16Plan<'a>, PilError> {
    validate_native_source(source.as_ref(), mode, byte_channels(channels, 2)?, region_size)?;
    let mask_layout = prepare_native_mask(mask.as_deref(), mask_alpha, region_size)?;
    Ok(NativeU16Plan {
        destination,
        destination_size,
        source_size: region_size,
        source,
        mask,
        mask_layout,
        region: PasteRegion::clipped(position.0, position.1, region_size, destination_size),
        selection,
    })
}

pub(crate) fn native_f32_plan<'a>(
    destination: &'a [f32],
    destination_size: (u32, u32),
    mode: Mode,
    channels: usize,
    source: Arc<DynamicImage>,
    mask: Option<Arc<DynamicImage>>,
    mask_alpha: bool,
    position: (i64, i64),
    region_size: (u32, u32),
    selection: StrategySelection,
) -> Result<NativeF32Plan<'a>, PilError> {
    validate_native_source(source.as_ref(), mode, byte_channels(channels, 4)?, region_size)?;
    let mask_layout = prepare_native_mask(mask.as_deref(), mask_alpha, region_size)?;
    Ok(NativeF32Plan {
        destination,
        destination_size,
        source_size: region_size,
        source,
        mask,
        mask_layout,
        region: PasteRegion::clipped(position.0, position.1, region_size, destination_size),
        selection,
    })
}

fn prepare_native_mask(
    mask: Option<&DynamicImage>,
    mask_alpha: bool,
    region_size: (u32, u32),
) -> Result<Option<MaskLayout>, PilError> {
    let layout = mask.map(|image| {
        mask_layout(image.mode(), mask_alpha).ok_or_else(|| {
            PilError::InternalError("Paste mask strategy did not match mask mode".into())
        })
    }).transpose()?;
    if let (Some(image), Some(layout)) = (mask, layout) {
        check_byte_len(image.as_bytes(), image.dimensions(), layout.channels, "mask")?;
        if image.dimensions() != region_size {
            return Err(PilError::InternalError("Paste mask dimensions changed after admission".into()));
        }
    }
    Ok(layout)
}

fn validate_native_source(
    source: &DynamicImage,
    mode: Mode,
    bytes_per_pixel: usize,
    region_size: (u32, u32),
) -> Result<(), PilError> {
    if source.mode() != mode || source.dimensions() != region_size {
        return Err(PilError::InternalError("native Paste requires an exact same-mode source".into()));
    }
    let expected = sample_len(region_size, bytes_per_pixel)?;
    if source.as_bytes().len() != expected {
        return Err(PilError::InternalError("native Paste source storage did not match dimensions".into()));
    }
    Ok(())
}

fn byte_channels(channels: usize, bytes_per_sample: usize) -> Result<usize, PilError> {
    channels
        .checked_mul(bytes_per_sample)
        .ok_or_else(|| PilError::OverflowError("Paste sample layout overflowed".into()))
}

fn check_byte_len(
    bytes: &[u8],
    dimensions: (u32, u32),
    channels: usize,
    role: &str,
) -> Result<(), PilError> {
    let expected = sample_len(dimensions, channels)?;
    if bytes.len() != expected {
        return Err(PilError::InternalError(format!("Paste {role} storage did not match dimensions")));
    }
    Ok(())
}

fn sample_len(dimensions: (u32, u32), channels: usize) -> Result<usize, PilError> {
    usize::try_from(dimensions.0)
        .ok()
        .and_then(|width| {
            usize::try_from(dimensions.1)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| PilError::OverflowError("Paste storage dimensions overflowed".into()))
}
