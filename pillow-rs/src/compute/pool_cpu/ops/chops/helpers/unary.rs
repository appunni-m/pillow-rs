//! Serial native-mode ImageChops unary strategies.

use crate::error::PilError;
use crate::raster::buffer::{HiddenPixelLanes, ImageBuffer};
use crate::raster::traits::Pixel;
use crate::raster::{DynamicImage, GrayImage, Luma};

pub(crate) fn constant(width: u32, height: u32, value: u8) -> DynamicImage {
    DynamicImage::ImageLuma8(GrayImage::from_pixel(width, height, Luma([value])))
}

pub(crate) fn duplicate(image: &DynamicImage) -> DynamicImage {
    // NO_PILLOW_SUPPORT for internal high-depth sources: preserve their exact
    // native identity through this core-only route.
    image.clone()
}

pub(crate) fn invert<P, S>(
    image: &ImageBuffer<P, Vec<S>>,
) -> Result<ImageBuffer<P, Vec<S>>, PilError>
where
    P: Pixel<Subpixel = S>,
    S: InvertSample,
{
    let values = image
        .as_raw()
        .iter()
        .copied()
        .map(InvertSample::invert_storage)
        .collect();
    let mut output = ImageBuffer::<P, Vec<S>>::from_raw(image.width(), image.height(), values)
        .ok_or_else(|| PilError::InternalError("invert Chops buffer shape mismatch".into()))?;
    carry_inverted_hidden_lanes(image, &mut output)?;
    Ok(output)
}

pub(crate) fn carry_inverted_hidden_lanes<P, S>(
    source: &ImageBuffer<P, Vec<S>>,
    output: &mut ImageBuffer<P, Vec<S>>,
) -> Result<(), PilError>
where
    P: Pixel<Subpixel = S>,
{
    let lanes = source
        .hidden_pixel_lanes()
        .map(|lanes| lanes.map_values(|value| !value));
    if output.replace_hidden_pixel_lanes(lanes) {
        Ok(())
    } else {
        Err(PilError::InternalError(
            "invert Chops hidden lane shape mismatch".into(),
        ))
    }
}

/// Preserves an inverted three-band fallback byte when no producer sidecar
/// was present on the source carrier.
pub(crate) fn carry_inverted_lane_fallback(
    source: &DynamicImage,
    output: &mut DynamicImage,
) -> Result<(), PilError> {
    let mode = source.mode();
    if mode.hidden_pixel_lane_count() != Some(1) || source.hidden_pixel_lanes().is_some() {
        return Ok(());
    }
    let pixel_count = (source.width() as usize)
        .checked_mul(source.height() as usize)
        .ok_or_else(|| PilError::InternalError("Chops hidden lane extent overflow".into()))?;
    if pixel_count == 0 {
        return Ok(());
    }
    let fallback =
        crate::compute::pool_cpu::helpers::bytes::hidden_lanes::default_hidden_pixel_lanes(mode, 0)
            .ok_or_else(|| {
                PilError::InternalError("Chops mode omitted its physical-lane fallback".into())
            })?;
    let lanes = HiddenPixelLanes::constant(1, pixel_count, [!fallback[0], 0])
        .ok_or_else(|| PilError::InternalError("Chops inverted lane result was invalid".into()))?;
    if output.mode() != mode || !output.replace_hidden_pixel_lanes(Some(lanes)) {
        return Err(PilError::InternalError(
            "Chops inverted lane output did not match its mode".into(),
        ));
    }
    Ok(())
}

pub(crate) trait InvertSample: Copy {
    fn invert_storage(self) -> Self;
}

impl InvertSample for u8 {
    fn invert_storage(self) -> Self {
        !self
    }
}

impl InvertSample for u16 {
    fn invert_storage(self) -> Self {
        !self
    }
}

impl InvertSample for f32 {
    fn invert_storage(self) -> Self {
        Self::from_bits(!self.to_bits())
    }
}
