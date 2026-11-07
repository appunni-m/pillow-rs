use crate::error::PilError;

/// Host protocols required by the custom-value `ImageOps.scale` strategy.
#[doc(hidden)]
pub trait ImageOpsScaleCallbacks {
    /// Carrier for the factor, image dimensions, and rounded values.
    type Value: Clone;
    /// Error type that accepts core validation errors.
    type Error: From<PilError>;

    /// Apply Pillow's `factor <= 0` comparison and truth conversion.
    fn factor_is_non_positive(&mut self, factor: &Self::Value) -> Result<bool, Self::Error>;
    /// Read the current image width property.
    fn image_width(&mut self) -> Result<Self::Value, Self::Error>;
    /// Read the current image height property.
    fn image_height(&mut self) -> Result<Self::Value, Self::Error>;
    /// Multiply two host numbers in the given operand order.
    fn multiply(
        &mut self,
        left: &Self::Value,
        right: &Self::Value,
    ) -> Result<Self::Value, Self::Error>;
    /// Apply Python's built-in `round` to a host number.
    fn round(&mut self, value: &Self::Value) -> Result<Self::Value, Self::Error>;
    /// Construct the final `(width, height)` value for public resize dispatch.
    fn size_pair(
        &mut self,
        width: &Self::Value,
        height: &Self::Value,
    ) -> Result<Self::Value, Self::Error>;
}

/// Host callbacks needed to route ImageOps Scale and preserve public methods.
#[doc(hidden)]
pub trait ImageOpsScaleRouteCallbacks: ImageOpsScaleCallbacks {
    /// Output wrapper returned by the host's public `copy` or `resize` method.
    type Output;

    /// Apply Pillow's `factor == 1` comparison and truth conversion.
    fn factor_is_one(&mut self, factor: &Self::Value) -> Result<bool, Self::Error>;
    /// Report whether the image and factor qualify for the native geometry path.
    fn native_route_is_eligible(&mut self, factor: &Self::Value) -> Result<bool, Self::Error>;
    /// Snapshot the core image after native-route eligibility succeeds.
    fn native_image(&mut self) -> Result<crate::image::Image, Self::Error>;
    /// Apply the facade's `float(factor)` conversion.
    fn factor_as_f64(&mut self, factor: &Self::Value) -> Result<f64, Self::Error>;
    /// Adapt a native geometry dimension to a host number for Python `round`.
    fn number(&mut self, value: f64) -> Result<Self::Value, Self::Error>;
    /// Run native geometry without holding the host runtime lock.
    fn run_core<T: Send>(&mut self, operation: impl FnOnce() -> T + Send) -> T;
    /// Call the current public `image.copy()` method.
    fn copy_image(&mut self) -> Result<Self::Output, Self::Error>;
    /// Call the current public `image.resize(size, resample)` method.
    fn resize_image(
        &mut self,
        size: &Self::Value,
        resample: &Self::Value,
    ) -> Result<Self::Output, Self::Error>;
}

/// Compute custom-value scale dimensions before public `Image.resize` dispatch.
pub fn scale_size_with_callbacks<C: ImageOpsScaleCallbacks>(
    callbacks: &mut C,
    factor: &C::Value,
) -> Result<C::Value, C::Error> {
    if callbacks.factor_is_non_positive(factor)? {
        return Err(PilError::ValueError("the factor must be greater than 0".into()).into());
    }
    let factor_width = callbacks.image_width()?;
    let scaled_width = callbacks.multiply(factor, &factor_width)?;
    let width = callbacks.round(&scaled_width)?;
    let factor_height = callbacks.image_height()?;
    let scaled_height = callbacks.multiply(factor, &factor_height)?;
    let height = callbacks.round(&scaled_height)?;
    callbacks.size_pair(&width, &height)
}

/// Select native or callback-driven Scale geometry, then use public wrapper calls.
pub fn scale_with_route_callbacks<C: ImageOpsScaleRouteCallbacks>(
    callbacks: &mut C,
    factor: &C::Value,
    resample: &C::Value,
) -> Result<C::Output, C::Error> {
    if callbacks.factor_is_one(factor)? {
        return callbacks.copy_image();
    }

    let size = if callbacks.native_route_is_eligible(factor)? {
        let image = callbacks.native_image()?;
        let factor = callbacks.factor_as_f64(factor)?;
        let (width, height) =
            callbacks.run_core(move || super::scale_geometry_preflight(&image, factor))?;
        let width = callbacks.number(width)?;
        let width = callbacks.round(&width)?;
        let height = callbacks.number(height)?;
        let height = callbacks.round(&height)?;
        callbacks.size_pair(&width, &height)?
    } else {
        scale_size_with_callbacks(callbacks, factor)?
    };
    callbacks.resize_image(&size, resample)
}
