use super::callbacks::{
    ImageOpsAspectArithmetic, ImageOpsAspectCallbacks, ImageOpsAspectComparison,
    ImageOpsNativeSizeCallbacks, native_size_pair_with_callbacks,
};
use crate::error::PilError;

/// Numeric constants required by the host-compatible `ImageOps.fit` strategy.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageOpsFitConstant {
    /// The floating-point value `0.0`.
    Zero,
    /// The floating-point value `0.5`.
    Half,
    /// The floating-point value `1.0`.
    One,
    /// The integer value `2`.
    Two,
}

/// Host operations needed to preserve dynamic Python values during fit sizing.
#[doc(hidden)]
pub trait ImageOpsFitCallbacks: ImageOpsAspectCallbacks {
    /// Read one coordinate from the current `image.size` property.
    fn image_size_coordinate(&mut self, index: usize) -> Result<Self::Value, Self::Error>;
    /// Construct the four-coordinate crop box after the core selects it.
    fn box_tuple(
        &mut self,
        left: &Self::Value,
        top: &Self::Value,
        right: &Self::Value,
        bottom: &Self::Value,
    ) -> Result<Self::Value, Self::Error>;
    /// Construct one host numeric constant used by the Pillow expression.
    fn fit_constant(&mut self, constant: ImageOpsFitConstant) -> Result<Self::Value, Self::Error>;
}

/// Host capabilities used by the core-owned ImageOps Fit route selector.
#[doc(hidden)]
pub trait ImageOpsFitRouteCallbacks:
    ImageOpsFitCallbacks
    + ImageOpsNativeSizeCallbacks<
        Value = <Self as ImageOpsAspectCallbacks>::Value,
        Error = <Self as ImageOpsAspectCallbacks>::Error,
    >
where
    <Self as ImageOpsAspectCallbacks>::Error: From<PilError>,
{
    /// Check the exact numeric and tuple types used by Fit's native fast path.
    fn native_fit_inputs_are_eligible(
        &mut self,
        bleed: &<Self as ImageOpsAspectCallbacks>::Value,
        centering: &<Self as ImageOpsAspectCallbacks>::Value,
    ) -> Result<bool, <Self as ImageOpsAspectCallbacks>::Error>;
    /// Snapshot the core image after native size and Fit admission succeed.
    fn native_image(
        &mut self,
    ) -> Result<crate::image::Image, <Self as ImageOpsAspectCallbacks>::Error>;
    /// Apply the facade's `float(bleed)` conversion.
    fn bleed_as_f64(
        &mut self,
        bleed: &<Self as ImageOpsAspectCallbacks>::Value,
    ) -> Result<f64, <Self as ImageOpsAspectCallbacks>::Error>;
    /// Adapt the already-admitted numeric tuple to the core centering value.
    fn native_centering(
        &mut self,
        centering: &<Self as ImageOpsAspectCallbacks>::Value,
    ) -> Result<crate::ops::imageops::CenteringInput, <Self as ImageOpsAspectCallbacks>::Error>;
    /// Unpack the custom centering iterable using the host's native semantics.
    fn unpack_centering(
        &mut self,
        centering: &<Self as ImageOpsAspectCallbacks>::Value,
    ) -> Result<
        (
            <Self as ImageOpsAspectCallbacks>::Value,
            <Self as ImageOpsAspectCallbacks>::Value,
        ),
        <Self as ImageOpsAspectCallbacks>::Error,
    >;
    /// Run native geometry without holding the host runtime lock.
    fn run_core<T: Send>(&mut self, operation: impl FnOnce() -> T + Send) -> T;
    /// Convert native crop-box coordinates to the host tuple representation.
    fn native_box_tuple(
        &mut self,
        box_: (f64, f64, f64, f64),
    ) -> Result<<Self as ImageOpsAspectCallbacks>::Value, <Self as ImageOpsAspectCallbacks>::Error>;
}

/// Compute Pillow-compatible `ImageOps.fit` crop coordinates with host callbacks.
pub fn fit_box_with_callbacks<C: ImageOpsFitCallbacks>(
    callbacks: &mut C,
    bleed: &C::Value,
    centering_x: &C::Value,
    centering_y: &C::Value,
) -> Result<C::Value, C::Error> {
    let zero = callbacks.fit_constant(ImageOpsFitConstant::Zero)?;
    let one = callbacks.fit_constant(ImageOpsFitConstant::One)?;
    let half = callbacks.fit_constant(ImageOpsFitConstant::Half)?;
    let two = callbacks.fit_constant(ImageOpsFitConstant::Two)?;
    let centering_x = normalized_centering(callbacks, centering_x, &zero, &one, &half)?;
    let centering_y = normalized_centering(callbacks, centering_y, &zero, &one, &half)?;
    let bleed = if callbacks.compare(ImageOpsAspectComparison::LessThanOrEqual, &zero, bleed)?
        && callbacks.compare(ImageOpsAspectComparison::LessThan, bleed, &half)?
    {
        bleed.clone()
    } else {
        zero.clone()
    };

    let source_width = callbacks.image_size_coordinate(0)?;
    let bleed_left =
        callbacks.arithmetic(ImageOpsAspectArithmetic::Multiply, &bleed, &source_width)?;
    let source_height = callbacks.image_size_coordinate(1)?;
    let bleed_top =
        callbacks.arithmetic(ImageOpsAspectArithmetic::Multiply, &bleed, &source_height)?;
    let live_width = live_axis(callbacks, 0, &bleed_left, &two)?;
    let live_height = live_axis(callbacks, 1, &bleed_top, &two)?;
    let live_ratio =
        callbacks.arithmetic(ImageOpsAspectArithmetic::Divide, &live_width, &live_height)?;
    let target_width = callbacks.size_coordinate(0)?;
    let target_height = callbacks.size_coordinate(1)?;
    let output_ratio = callbacks.arithmetic(
        ImageOpsAspectArithmetic::Divide,
        &target_width,
        &target_height,
    )?;
    let (crop_width, crop_height) =
        if callbacks.compare(ImageOpsAspectComparison::Equal, &live_ratio, &output_ratio)? {
            (live_width.clone(), live_height.clone())
        } else if callbacks.compare(
            ImageOpsAspectComparison::GreaterThanOrEqual,
            &live_ratio,
            &output_ratio,
        )? {
            (
                callbacks.arithmetic(
                    ImageOpsAspectArithmetic::Multiply,
                    &output_ratio,
                    &live_height,
                )?,
                live_height.clone(),
            )
        } else {
            (
                live_width.clone(),
                callbacks.arithmetic(
                    ImageOpsAspectArithmetic::Divide,
                    &live_width,
                    &output_ratio,
                )?,
            )
        };
    let left = offset_axis(
        callbacks,
        &bleed_left,
        &live_width,
        &crop_width,
        &centering_x,
    )?;
    let top = offset_axis(
        callbacks,
        &bleed_top,
        &live_height,
        &crop_height,
        &centering_y,
    )?;
    let right = callbacks.arithmetic(ImageOpsAspectArithmetic::Add, &left, &crop_width)?;
    let bottom = callbacks.arithmetic(ImageOpsAspectArithmetic::Add, &top, &crop_height)?;
    callbacks.box_tuple(&left, &top, &right, &bottom)
}

/// Select native or host-protocol Fit geometry from one core entry point.
pub fn fit_box_with_route_callbacks<C: ImageOpsFitRouteCallbacks>(
    callbacks: &mut C,
    bleed: &<C as ImageOpsAspectCallbacks>::Value,
    centering: &<C as ImageOpsAspectCallbacks>::Value,
) -> Result<<C as ImageOpsAspectCallbacks>::Value, <C as ImageOpsAspectCallbacks>::Error> {
    if let Some((width, height)) = native_size_pair_with_callbacks(callbacks)? {
        if callbacks.native_fit_inputs_are_eligible(bleed, centering)? {
            let image = callbacks.native_image()?;
            let bleed = callbacks.bleed_as_f64(bleed)?;
            let centering = callbacks.native_centering(centering)?;
            let box_ = callbacks.run_core(move || {
                super::fit_geometry_preflight(&image, width, height, bleed, centering)
            })?;
            return callbacks.native_box_tuple(box_);
        }
    }

    let (centering_x, centering_y) = callbacks.unpack_centering(centering)?;
    fit_box_with_callbacks(callbacks, bleed, &centering_x, &centering_y)
}

fn normalized_centering<C: ImageOpsFitCallbacks>(
    callbacks: &mut C,
    value: &C::Value,
    zero: &C::Value,
    one: &C::Value,
    half: &C::Value,
) -> Result<C::Value, C::Error> {
    let in_range = callbacks.compare(ImageOpsAspectComparison::LessThanOrEqual, zero, value)?
        && callbacks.compare(ImageOpsAspectComparison::LessThanOrEqual, value, one)?;
    Ok(if in_range {
        value.clone()
    } else {
        half.clone()
    })
}

fn live_axis<C: ImageOpsFitCallbacks>(
    callbacks: &mut C,
    index: usize,
    bleed: &C::Value,
    two: &C::Value,
) -> Result<C::Value, C::Error> {
    let size = callbacks.image_size_coordinate(index)?;
    let bleed_twice = callbacks.arithmetic(ImageOpsAspectArithmetic::Multiply, bleed, two)?;
    callbacks.arithmetic(ImageOpsAspectArithmetic::Subtract, &size, &bleed_twice)
}

fn offset_axis<C: ImageOpsFitCallbacks>(
    callbacks: &mut C,
    bleed: &C::Value,
    live: &C::Value,
    crop: &C::Value,
    centering: &C::Value,
) -> Result<C::Value, C::Error> {
    let remaining = callbacks.arithmetic(ImageOpsAspectArithmetic::Subtract, live, crop)?;
    let centered =
        callbacks.arithmetic(ImageOpsAspectArithmetic::Multiply, &remaining, centering)?;
    callbacks.arithmetic(ImageOpsAspectArithmetic::Add, bleed, &centered)
}
