use super::callbacks::{
    ImageOpsNativeSizeCallbacks, ImageOpsPadCallbacks, native_size_pair_with_callbacks,
};
use crate::ResampleInput;
use crate::ops::imageops::{CenteringInput, ImageOpsColor};
use crate::raster::Mode;

/// Host facts and wrapper calls needed to select ImageOps Pad's execution route.
#[doc(hidden)]
pub trait ImageOpsPadRouteCallbacks:
    ImageOpsPadCallbacks
    + ImageOpsNativeSizeCallbacks<
        Value = <Self as ImageOpsPadCallbacks>::Value,
        Error = <Self as ImageOpsPadCallbacks>::Error,
    >
{
    /// Whether the facade, image instance, and its Pad methods are unmodified.
    fn native_pad_wrapper_is_eligible(
        &mut self,
    ) -> Result<bool, <Self as ImageOpsPadCallbacks>::Error>;
    /// Whether filter, color, and centering use the exact native input types.
    fn native_pad_arguments_are_eligible(
        &mut self,
        method: &<Self as ImageOpsPadCallbacks>::Value,
        color: &<Self as ImageOpsPadCallbacks>::Value,
        centering: &<Self as ImageOpsPadCallbacks>::Value,
    ) -> Result<bool, <Self as ImageOpsPadCallbacks>::Error>;
    /// Run Rust-only preparation and execution without holding the host lock.
    fn run_core<T: Send>(&mut self, operation: impl FnOnce() -> T + Send) -> T;
    /// Snapshot the current native raster after host callbacks.
    fn native_image_snapshot(
        &mut self,
    ) -> Result<crate::image::Image, <Self as ImageOpsPadCallbacks>::Error>;
    /// Parse Pillow's resize filter input after contain geometry is calculated.
    fn resample_input(
        &mut self,
        method: &<Self as ImageOpsPadCallbacks>::Value,
    ) -> Result<Option<ResampleInput>, <Self as ImageOpsPadCallbacks>::Error>;
    /// Realize the current public wrapper's pending palette mutation.
    fn realize_dirty_palette(&mut self) -> Result<(), <Self as ImageOpsPadCallbacks>::Error>;
    /// Adapt the fill after palette realization and exact-mode selection.
    fn fill_input(
        &mut self,
        mode: Mode,
        color: &<Self as ImageOpsPadCallbacks>::Value,
    ) -> Result<ImageOpsColor, <Self as ImageOpsPadCallbacks>::Error>;
    /// Parse centering only after fill validation succeeds.
    fn centering_input(
        &mut self,
        centering: &<Self as ImageOpsPadCallbacks>::Value,
    ) -> Result<CenteringInput, <Self as ImageOpsPadCallbacks>::Error>;
    /// Wrap the final native result using Pillow's contained/padded route.
    fn wrap_native_pad_result(
        &mut self,
        image: crate::image::Image,
        contained: bool,
    ) -> Result<<Self as ImageOpsPadCallbacks>::Image, <Self as ImageOpsPadCallbacks>::Error>;
}

/// Select typed Pad or its host-protocol fallback from one core route.
///
/// Keep the native branch ordered: contain geometry precedes filter parsing;
/// dirty palette realization follows filter validation; fill parsing and
/// validation precede centering; wrapper construction follows image work.
/// Each host callback remains at its Pillow-visible point in that sequence.
pub fn pad_with_route_callbacks<C: ImageOpsPadRouteCallbacks>(
    callbacks: &mut C,
    size: &<C as ImageOpsPadCallbacks>::Value,
    method: &<C as ImageOpsPadCallbacks>::Value,
    color: &<C as ImageOpsPadCallbacks>::Value,
    centering: &<C as ImageOpsPadCallbacks>::Value,
) -> Result<<C as ImageOpsPadCallbacks>::Image, <C as ImageOpsPadCallbacks>::Error> {
    if let Some(dimensions) = native_size_pair_with_callbacks(callbacks)? {
        if callbacks.native_pad_wrapper_is_eligible()?
            && callbacks.native_pad_arguments_are_eligible(method, color, centering)?
        {
            return native_pad_with_callbacks(callbacks, dimensions, method, color, centering);
        }
    }

    super::pad_fallback_with_callbacks(callbacks, size, method, color, centering)
}

fn native_pad_with_callbacks<C: ImageOpsPadRouteCallbacks>(
    callbacks: &mut C,
    dimensions: (u32, u32),
    method: &<C as ImageOpsPadCallbacks>::Value,
    color: &<C as ImageOpsPadCallbacks>::Value,
    centering: &<C as ImageOpsPadCallbacks>::Value,
) -> Result<<C as ImageOpsPadCallbacks>::Image, <C as ImageOpsPadCallbacks>::Error> {
    let initial = callbacks.native_image_snapshot()?;
    let geometry = callbacks
        .run_core(move || super::pad_geometry_preflight(&initial, dimensions.0, dimensions.1))?;
    let filter = callbacks.resample_input(method)?;
    let preflight = callbacks.run_core(move || super::pad_filter_preflight(geometry, filter))?;
    let needs_padding = preflight.needs_padding();

    let mut image = callbacks.native_image_snapshot()?;
    if matches!(
        image.mode_identity()?,
        Mode::ImagePalette8 | Mode::ImagePaletteAlpha8
    ) {
        callbacks.realize_dirty_palette()?;
        image = callbacks.native_image_snapshot()?;
    }

    let fill_input = if needs_padding {
        callbacks.fill_input(image.mode_identity()?, color)?
    } else {
        ImageOpsColor::None
    };
    let fill_image = image.clone();
    let fill = callbacks
        .run_core(move || super::pad_fill_preflight(&fill_image, preflight, fill_input))?;
    let centering = if needs_padding {
        callbacks.centering_input(centering)?
    } else {
        CenteringInput::Default
    };
    let (image, contained) = callbacks
        .run_core(move || super::pad_with_preflight_and_fill(&image, preflight, fill, centering))?;
    callbacks.wrap_native_pad_result(image, contained)
}
