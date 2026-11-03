
use crate::raster::GenericImageView;
use std::borrow::Cow;
use std::ops::Deref;
use std::sync::Arc;

use crate::compute::registry::support::StrategySelection;
use crate::error::PilError;
use crate::raster::buffer::HiddenPixelLanes;
use crate::raster::{DynamicImage, ImageBuffer, Mode, Pixel};

use super::layout::{
    MaskLayout, SourceLayout, byte_carrier_pixel_width, hidden_lane_count, hidden_lane_offset,
    hidden_lanes, mask_layout, physical_four_byte_source, source_layout,
};
use super::region::PasteRegion;

/// Prepared byte-oriented Paste inputs for one exact destination mode.
pub(crate) struct BytePlan<'a, I = Arc<DynamicImage>> {
    pub(crate) destination: &'a [u8],
    pub(crate) destination_size: (u32, u32),
    destination_mode: Mode,
    destination_hidden_lanes: Option<&'a HiddenPixelLanes>,
    pub(crate) source: I,
    source_physical: Option<Vec<u8>>,
    pub(crate) source_size: (u32, u32),
    pub(crate) region_size: (u32, u32),
    pub(crate) source_layout: SourceLayout,
    pub(crate) broadcast_source: bool,
    pub(crate) mask: Option<I>,
    pub(crate) mask_layout: Option<MaskLayout>,
    pub(crate) region: PasteRegion,
    pub(crate) selection: StrategySelection,
    pub(crate) destination_initialization: DestinationInitialization,
}

/// Output setup required by the operation that composes this native-byte plan.
#[derive(Clone, Copy)]
pub(crate) enum DestinationInitialization {
    CloneDestination,
    ZeroThenVectorCopy,
}

impl<I> BytePlan<'_, I>
where
    I: Deref<Target = DynamicImage>,
{
    pub(crate) fn source_bytes(&self) -> &[u8] {
        self.source_physical
            .as_deref()
            .unwrap_or_else(|| self.source.as_bytes())
    }

    pub(crate) fn initialize_destination(&self) -> Result<Vec<u8>, PilError> {
        match self.destination_initialization {
            DestinationInitialization::CloneDestination => Ok(self.destination.to_vec()),
            DestinationInitialization::ZeroThenVectorCopy => {
                let mut output = vec![0; self.destination.len()];
                let (copy_blocks, copy_tail) =
                    crate::compute::pool_cpu::helpers::bytes::vector_copy::copy_native_bytes(
                        self.destination,
                        &mut output,
                    )
                    .ok_or_else(|| {
                        PilError::InternalError("Paste destination copy shape mismatch".into())
                    })?;
                crate::compute::record_pipeline_operation_vector_blocks(copy_blocks);
                crate::compute::record_pipeline_operation_scalar_tail(copy_tail);
                Ok(output)
            }
        }
    }

    /// Computes the physical lanes omitted by a compact destination carrier.
    pub(crate) fn hidden_output_lanes(&self) -> Result<Option<HiddenPixelLanes>, PilError> {
        let Some(lane_count) = hidden_lane_count(self.destination_mode) else {
            return Ok(None);
        };
        let pixel_count = checked_extent(self.destination_size, "destination pixels")?;
        if pixel_count == 0 {
            return Ok(None);
        }
        let destination_lane_bytes = checked_product(pixel_count, lane_count, "destination lanes")?;
        let source_physical = self.hidden_source_bytes()?;
        self.check_hidden_lane_extents(pixel_count, source_physical.len())?;
        let mut output = Vec::with_capacity(destination_lane_bytes);
        let destination_width = usize::try_from(self.destination_size.0)
            .map_err(|_| paste_overflow("destination width"))?;
        let destination_height = usize::try_from(self.destination_size.1)
            .map_err(|_| paste_overflow("destination height"))?;
        for destination_y in 0..destination_height {
            for destination_x in 0..destination_width {
                let destination_index = destination_y * destination_width + destination_x;
                self.append_hidden_pixel(
                    &mut output,
                    lane_count,
                    destination_index,
                    destination_x,
                    destination_y,
                    source_physical.as_ref(),
                )?;
            }
        }
        HiddenPixelLanes::per_pixel(lane_count, pixel_count, output)
            .map(Some)
            .ok_or_else(|| paste_layout_error("computed hidden lane extent"))
    }

    fn check_hidden_lane_extents(
        &self,
        pixel_count: usize,
        source_bytes_len: usize,
    ) -> Result<(), PilError> {
        let destination_pixel_width = byte_carrier_pixel_width(self.destination_mode)
            .ok_or_else(|| paste_layout_error("destination byte-carrier width"))?;
        let destination_bytes =
            checked_product(pixel_count, destination_pixel_width, "destination bytes")?;
        if self.destination.len() != destination_bytes {
            return Err(paste_layout_error("destination logical byte length"));
        }
        let source_pixels = checked_extent(self.source_size, "source pixels")?;
        if source_bytes_len != checked_product(source_pixels, 4, "source physical bytes")? {
            return Err(paste_layout_error("source physical byte length"));
        }
        Ok(())
    }

    fn hidden_source_bytes(&self) -> Result<Cow<'_, [u8]>, PilError> {
        if let Some(bytes) = self.source_physical.as_deref() {
            return Ok(Cow::Borrowed(bytes));
        }
        if byte_carrier_pixel_width(self.source.mode()) == Some(4) {
            return Ok(Cow::Borrowed(self.source.as_bytes()));
        }
        let bytes = physical_four_byte_source(&*self.source)
            .ok_or_else(|| paste_layout_error("source physical pixel layout"))?;
        Ok(Cow::Owned(bytes))
    }

    fn append_hidden_pixel(
        &self,
        output: &mut Vec<u8>,
        lane_count: usize,
        destination_index: usize,
        destination_x: usize,
        destination_y: usize,
        source_physical: &[u8],
    ) -> Result<(), PilError> {
        let destination_pixel_width = byte_carrier_pixel_width(self.destination_mode)
            .ok_or_else(|| paste_layout_error("destination byte-carrier width"))?;
        let logical_start = destination_index
            .checked_mul(destination_pixel_width)
            .ok_or_else(|| paste_overflow("destination logical offset"))?;
        let previous = self
            .destination_hidden_lanes
            .filter(|lanes| lanes.lane_count() == lane_count)
            .and_then(|lanes| lanes.for_pixel(destination_index));
        let source_position = self.source_position(destination_x, destination_y);
        for lane in 0..lane_count {
            let mut value = previous
                .and_then(|lanes| lanes.get(lane))
                .copied()
                .unwrap_or_else(|| {
                    default_hidden_lane(
                        self.destination_mode,
                        lane,
                        self.destination[logical_start],
                    )
                });
            if let Some((source_x, source_y)) = source_position {
                value = self.paste_hidden_lane(
                    lane,
                    source_x,
                    source_y,
                    source_physical,
                    value,
                )?;
            }
            output.push(value);
        }
        Ok(())
    }

    fn source_position(&self, destination_x: usize, destination_y: usize) -> Option<(usize, usize)> {
        let left = self.region.destination_left as usize;
        let top = self.region.destination_top as usize;
        let width = self.region.width as usize;
        let height = self.region.height as usize;
        if destination_x < left
            || destination_x >= left.saturating_add(width)
            || destination_y < top
            || destination_y >= top.saturating_add(height)
        {
            return None;
        }
        Some((
            self.region.source_left as usize + destination_x - left,
            self.region.source_top as usize + destination_y - top,
        ))
    }

    fn paste_hidden_lane(
        &self,
        lane: usize,
        source_x: usize,
        source_y: usize,
        source_physical: &[u8],
        destination: u8,
    ) -> Result<u8, PilError> {
        let source_index = if self.broadcast_source {
            0
        } else {
            source_y
                .checked_mul(self.source_size.0 as usize)
                .and_then(|row| row.checked_add(source_x))
                .ok_or_else(|| paste_overflow("source pixel index"))?
        };
        let physical_lane = hidden_lane_offset(self.destination_mode, lane)
            .ok_or_else(|| paste_layout_error("source hidden lane"))?;
        let source_offset = source_index
            .checked_mul(4)
            .and_then(|start| start.checked_add(physical_lane))
            .ok_or_else(|| paste_layout_error("source hidden lane"))?;
        let source = *source_physical
            .get(source_offset)
            .ok_or_else(|| paste_layout_error("source hidden lane sample"))?;
        self.blend_hidden_lane(source_x, source_y, source, destination)
    }

    fn blend_hidden_lane(
        &self,
        source_x: usize,
        source_y: usize,
        source: u8,
        destination: u8,
    ) -> Result<u8, PilError> {
        let Some((mask, layout)) = self.mask.as_deref().zip(self.mask_layout) else {
            return Ok(source);
        };
        let mask_index = source_y
            .checked_mul(self.region_size.0 as usize)
            .and_then(|row| row.checked_add(source_x))
            .and_then(|index| index.checked_mul(layout.channels))
            .and_then(|start| start.checked_add(layout.value_index))
            .ok_or_else(|| paste_overflow("mask sample offset"))?;
        let weight = *mask
            .as_bytes()
            .get(mask_index)
            .ok_or_else(|| paste_layout_error("mask sample"))?;
        if layout.binary {
            return Ok(if weight == 0 { destination } else { source });
        }
        if weight == u8::MAX {
            return Ok(source);
        }
        if weight == 0 && !layout.premultiplied {
            return Ok(destination);
        }
        if layout.premultiplied {
            return Ok(crate::compute::pool_cpu::helpers::bytes::preblend::apply(
                source,
                destination,
                weight,
            ));
        }
        let inverse = 255 - u16::from(weight);
        Ok(((u16::from(source) * u16::from(weight)
            + u16::from(destination) * inverse
            + 127)
            / 255) as u8)
    }
}

/// Runs a selected implementation and restores the destination's full physical lanes.
pub(crate) fn execute_with_hidden_lanes<'a, I>(
    strategy: BytePlan<'a, I>,
    execute: impl FnOnce(BytePlan<'a, I>) -> Result<DynamicImage, PilError>,
) -> Result<DynamicImage, PilError>
where
    I: Deref<Target = DynamicImage>,
{
    let expected_mode = strategy.destination_mode;
    let hidden_lanes = strategy.hidden_output_lanes()?;
    let mut output = execute(strategy)?;
    if output.mode() != expected_mode {
        return Err(paste_layout_error("output mode"));
    }
    if let Some(lanes) = hidden_lanes {
        if !output.replace_hidden_pixel_lanes(Some(lanes)) {
            return Err(paste_layout_error("output hidden lane destination"));
        }
    }
    Ok(output)
}

fn default_hidden_lane(mode: Mode, lane: usize, logical_first: u8) -> u8 {
    crate::compute::pool_cpu::helpers::bytes::hidden_lanes::default_hidden_pixel_lanes(
        mode,
        logical_first,
    )
    .and_then(|values| values.get(lane).copied())
    .unwrap_or(0)
}

fn checked_extent(size: (u32, u32), part: &str) -> Result<usize, PilError> {
    usize::try_from(size.0)
        .ok()
        .and_then(|width| usize::try_from(size.1).ok().and_then(|height| width.checked_mul(height)))
        .ok_or_else(|| paste_overflow(part))
}

fn checked_product(left: usize, right: usize, part: &str) -> Result<usize, PilError> {
    left.checked_mul(right).ok_or_else(|| paste_overflow(part))
}

fn paste_overflow(part: &str) -> PilError {
    PilError::OverflowError(format!("Paste {part} overflowed"))
}

fn paste_layout_error(part: &str) -> PilError {
    PilError::InternalError(format!("Paste hidden pixel lane {part} was invalid"))
}

/// Prepared typed I;16 inputs, retaining the mode-specific stored byte order.
pub(crate) struct Luma16Plan<'a> {
    pub(crate) destination: &'a [u16],
    pub(crate) destination_size: (u32, u32),
    pub(crate) source: Arc<DynamicImage>,
    pub(crate) source_mode: Mode,
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

pub(crate) fn byte_plan<'a, I>(
    destination: &'a ImageBuffer<impl Pixel<Subpixel = u8>, Vec<u8>>,
    destination_size: (u32, u32),
    destination_mode: Mode,
    source: I,
    mask: Option<I>,
    mask_alpha: bool,
    position: (i64, i64),
    region_size: (u32, u32),
    selection: StrategySelection,
) -> Result<BytePlan<'a, I>, PilError>
where
    I: Deref<Target = DynamicImage>,
{
    let destination_hidden_lanes = destination.hidden_pixel_lanes();
    byte_plan_with_initialization(
        destination.as_raw(),
        destination_size,
        destination_mode,
        destination_hidden_lanes,
        source,
        mask,
        mask_alpha,
        position,
        region_size,
        selection,
        DestinationInitialization::CloneDestination,
    )
}

/// Prepares an exact-mode Composite plan that borrows all raster inputs.
pub(crate) fn composite_byte_plan<'a>(
    destination: &'a DynamicImage,
    mode: Mode,
    source: &'a DynamicImage,
    mask: &'a DynamicImage,
    mask_alpha: bool,
    selection: StrategySelection,
) -> Result<BytePlan<'a, &'a DynamicImage>, PilError> {
    if destination.mode() != mode || source.mode() != mode {
        return Err(PilError::InternalError(
            "Composite byte plan requires exact source and destination modes".into(),
        ));
    }
    let destination_hidden_lanes = hidden_lanes(destination, mode);
    byte_plan_with_initialization(
        destination.as_bytes(),
        destination.dimensions(),
        mode,
        destination_hidden_lanes,
        source,
        Some(mask),
        mask_alpha,
        (0, 0),
        source.dimensions(),
        selection,
        if mode == Mode::ImageLuma8 {
            DestinationInitialization::CloneDestination
        } else {
            DestinationInitialization::ZeroThenVectorCopy
        },
    )
}

fn byte_plan_with_initialization<'a, I>(
    destination: &'a [u8],
    destination_size: (u32, u32),
    destination_mode: Mode,
    destination_hidden_lanes: Option<&'a HiddenPixelLanes>,
    source: I,
    mask: Option<I>,
    mask_alpha: bool,
    position: (i64, i64),
    region_size: (u32, u32),
    selection: StrategySelection,
    destination_initialization: DestinationInitialization,
) -> Result<BytePlan<'a, I>, PilError>
where
    I: Deref<Target = DynamicImage>,
{
    let source_size = source.dimensions();
    let source_mode = source.mode();
    let source_layout = source_layout(destination_mode, source_mode).ok_or_else(|| {
        PilError::InternalError("Paste source strategy did not match destination mode".into())
    })?;
    let source_channels = match source_layout {
        SourceLayout::Native => byte_carrier_pixel_width(source_mode),
        SourceLayout::PhysicalFourBytesToLumaAlpha
        | SourceLayout::PhysicalFourBytesToRgb
        | SourceLayout::PhysicalFourBytesToFour => Some(4),
    }
    .ok_or_else(|| PilError::InternalError("Paste source has no byte layout".into()))?;
    let source_physical = match source_layout {
        SourceLayout::Native => None,
        SourceLayout::PhysicalFourBytesToLumaAlpha
        | SourceLayout::PhysicalFourBytesToRgb
        | SourceLayout::PhysicalFourBytesToFour
            if byte_carrier_pixel_width(source_mode) == Some(4) => None,
        SourceLayout::PhysicalFourBytesToLumaAlpha
        | SourceLayout::PhysicalFourBytesToRgb
        | SourceLayout::PhysicalFourBytesToFour => Some(
            physical_four_byte_source(&*source).ok_or_else(|| {
                PilError::InternalError("Paste source could not expose its physical pixel layout".into())
            })?,
        ),
    };
    let source_bytes = source_physical
        .as_deref()
        .unwrap_or_else(|| source.as_bytes());
    check_byte_len(source_bytes, source_size, source_channels, "source")?;
    check_byte_len(
        destination,
        destination_size,
        byte_carrier_pixel_width(destination_mode).ok_or_else(|| {
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
        destination_mode,
        destination_hidden_lanes,
        source,
        source_physical,
        source_size,
        region_size,
        source_layout,
        broadcast_source,
        mask,
        mask_layout,
        region,
        selection,
        destination_initialization,
    })
}

/// TODO(pillow-rs#3ptb): Freeze parity for all source/destination byte orders,
/// masks, and clipped regions before claiming raw I;16 alias support.
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
    let source_mode = source.mode();
    if !super::contracts::I16_PIXEL_SIZE_SOURCES.contains(&source_mode)
        || source.dimensions() != region_size
    {
        return Err(PilError::InternalError(
            "I;16 Paste requires a same-pixel-size source with matching dimensions".into(),
        ));
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
        source_mode,
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
    validate_native_source(source.as_ref(), mode, checked_sample_bytes(channels, 2)?, region_size)?;
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
    validate_native_source(source.as_ref(), mode, checked_sample_bytes(channels, 4)?, region_size)?;
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

fn checked_sample_bytes(channels: usize, bytes_per_sample: usize) -> Result<usize, PilError> {
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
