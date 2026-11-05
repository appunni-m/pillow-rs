use crate::error::PilError;

/// Aspect-ratio strategy used by ImageOps contain and cover.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageOpsAspectOperation {
    /// Fit the source inside the requested dimensions.
    Contain,
    /// Scale the source until it covers the requested dimensions.
    Cover,
}

/// Arithmetic requested by the ImageOps aspect-ratio and fit strategies.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageOpsAspectArithmetic {
    /// Add two host numbers.
    Add,
    /// Subtract one host number from another.
    Subtract,
    /// Divide two host numbers.
    Divide,
    /// Multiply two host numbers.
    Multiply,
}

/// Comparisons used by ImageOps aspect-ratio and fit geometry.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageOpsAspectComparison {
    /// Test whether two values are equal.
    Equal,
    /// Test whether ratios or dimensions differ.
    NotEqual,
    /// Test whether the left value is less than or equal to the right value.
    LessThanOrEqual,
    /// Test whether the source ratio is greater than the destination ratio.
    GreaterThan,
    /// Test whether the left value is greater than or equal to the right value.
    GreaterThanOrEqual,
    /// Test whether the source ratio is less than the destination ratio.
    LessThan,
}

/// Result of applying ImageOps aspect geometry to a host size value.
#[doc(hidden)]
pub enum ImageOpsAspectSize<Value> {
    /// Preserve the original size object and its identity.
    Original,
    /// Use the newly computed pair for the public resize call.
    Resized(Value),
}

/// Host operations shared by the core-owned ImageOps aspect-ratio and fit strategies.
#[doc(hidden)]
pub trait ImageOpsAspectCallbacks {
    /// Value carrier for the host's dimensions and arithmetic results.
    type Value: Clone;
    /// Callback error type; core errors convert into the host error.
    type Error: From<PilError>;

    /// Read the current image width through the host property.
    fn image_width(&mut self) -> Result<Self::Value, Self::Error>;
    /// Read the current image height through the host property.
    fn image_height(&mut self) -> Result<Self::Value, Self::Error>;
    /// Read one value from the requested output size.
    fn size_coordinate(&mut self, index: usize) -> Result<Self::Value, Self::Error>;
    /// Apply the requested host numeric operation.
    fn arithmetic(
        &mut self,
        operation: ImageOpsAspectArithmetic,
        left: &Self::Value,
        right: &Self::Value,
    ) -> Result<Self::Value, Self::Error>;
    /// Apply one host comparison and its truth test.
    fn compare(
        &mut self,
        operation: ImageOpsAspectComparison,
        left: &Self::Value,
        right: &Self::Value,
    ) -> Result<bool, Self::Error>;
    /// Apply the host's built-in round operation.
    fn round(&mut self, value: &Self::Value) -> Result<Self::Value, Self::Error>;
    /// Construct a host size pair after the core selects a changed dimension.
    fn size_pair(
        &mut self,
        width: &Self::Value,
        height: &Self::Value,
    ) -> Result<Self::Value, Self::Error>;
}

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

/// Host operations for normalizing scalar, pair, and four-value borders.
#[doc(hidden)]
pub trait ImageOpsBorderCallbacks {
    /// Carrier for the border components and crop-box coordinates.
    type Value: Clone;
    /// Callback error type.
    type Error;

    /// Check whether the border follows Python's tuple branch.
    fn border_is_tuple(&mut self, border: &Self::Value) -> Result<bool, Self::Error>;
    /// Read the tuple length when the tuple branch is selected.
    fn border_length(&mut self, border: &Self::Value) -> Result<usize, Self::Error>;
    /// Read one item from a two- or four-component border.
    fn border_coordinate(
        &mut self,
        border: &Self::Value,
        index: usize,
    ) -> Result<Self::Value, Self::Error>;
}

fn border_components<C: ImageOpsBorderCallbacks>(
    callbacks: &mut C,
    border: &C::Value,
) -> Result<(C::Value, C::Value, C::Value, C::Value), C::Error> {
    if callbacks.border_is_tuple(border)? {
        match callbacks.border_length(border)? {
            2 => {
                let left = callbacks.border_coordinate(border, 0)?;
                let top = callbacks.border_coordinate(border, 1)?;
                Ok((left.clone(), top.clone(), left, top))
            }
            4 => Ok((
                callbacks.border_coordinate(border, 0)?,
                callbacks.border_coordinate(border, 1)?,
                callbacks.border_coordinate(border, 2)?,
                callbacks.border_coordinate(border, 3)?,
            )),
            _ => Ok((border.clone(), border.clone(), border.clone(), border.clone())),
        }
    } else {
        Ok((border.clone(), border.clone(), border.clone(), border.clone()))
    }
}

/// Host operations required by the custom-border `ImageOps.crop` strategy.
#[doc(hidden)]
pub trait ImageOpsCropCallbacks: ImageOpsBorderCallbacks {
    /// Read one coordinate from the current `image.size` property.
    fn image_size_coordinate(&mut self, index: usize) -> Result<Self::Value, Self::Error>;
    /// Subtract one host coordinate from another.
    fn subtract(
        &mut self,
        left: &Self::Value,
        right: &Self::Value,
    ) -> Result<Self::Value, Self::Error>;
    /// Construct the four-coordinate crop box after the core selects it.
    fn box_tuple(
        &mut self,
        left: &Self::Value,
        top: &Self::Value,
        right: &Self::Value,
        bottom: &Self::Value,
    ) -> Result<Self::Value, Self::Error>;
}

/// Expand a scalar, pair, or four-value border and compute its crop box.
pub fn crop_box_with_callbacks<C: ImageOpsCropCallbacks>(
    callbacks: &mut C,
    border: &C::Value,
) -> Result<C::Value, C::Error> {
    let (left, top, right, bottom) = border_components(callbacks, border)?;
    let source_width = callbacks.image_size_coordinate(0)?;
    let right = callbacks.subtract(&source_width, &right)?;
    let source_height = callbacks.image_size_coordinate(1)?;
    let bottom = callbacks.subtract(&source_height, &bottom)?;
    callbacks.box_tuple(&left, &top, &right, &bottom)
}

/// Host operations required by the custom-border `ImageOps.expand` strategy.
#[doc(hidden)]
pub trait ImageOpsExpandGeometryCallbacks: ImageOpsBorderCallbacks {
    /// Read one coordinate from the current `image.size` property.
    fn image_size_coordinate(&mut self, index: usize) -> Result<Self::Value, Self::Error>;
    /// Add two host dimensions in operand order.
    fn add(&mut self, left: &Self::Value, right: &Self::Value) -> Result<Self::Value, Self::Error>;
    /// Return the computed width, height, left, and top values to the wrapper.
    fn expand_geometry_tuple(
        &mut self,
        width: &Self::Value,
        height: &Self::Value,
        left: &Self::Value,
        top: &Self::Value,
    ) -> Result<Self::Value, Self::Error>;
}

/// Normalize custom borders and compute the dimensions and source offset for Expand.
pub fn expand_geometry_with_callbacks<C: ImageOpsExpandGeometryCallbacks>(
    callbacks: &mut C,
    border: &C::Value,
) -> Result<C::Value, C::Error> {
    let (left, top, right, bottom) = border_components(callbacks, border)?;
    let source_width = callbacks.image_size_coordinate(0)?;
    let width_with_source = callbacks.add(&left, &source_width)?;
    let width = callbacks.add(&width_with_source, &right)?;
    let source_height = callbacks.image_size_coordinate(1)?;
    let height_with_source = callbacks.add(&top, &source_height)?;
    let height = callbacks.add(&height_with_source, &bottom)?;
    callbacks.expand_geometry_tuple(&width, &height, &left, &top)
}

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
    let bleed_left = callbacks.arithmetic(
        ImageOpsAspectArithmetic::Multiply,
        &bleed,
        &source_width,
    )?;
    let source_height = callbacks.image_size_coordinate(1)?;
    let bleed_top = callbacks.arithmetic(
        ImageOpsAspectArithmetic::Multiply,
        &bleed,
        &source_height,
    )?;
    let live_width = live_axis(callbacks, 0, &bleed_left, &two)?;
    let live_height = live_axis(callbacks, 1, &bleed_top, &two)?;
    let live_ratio = callbacks.arithmetic(
        ImageOpsAspectArithmetic::Divide,
        &live_width,
        &live_height,
    )?;
    let target_width = callbacks.size_coordinate(0)?;
    let target_height = callbacks.size_coordinate(1)?;
    let output_ratio = callbacks.arithmetic(
        ImageOpsAspectArithmetic::Divide,
        &target_width,
        &target_height,
    )?;
    let (crop_width, crop_height) = if callbacks.compare(
        ImageOpsAspectComparison::Equal,
        &live_ratio,
        &output_ratio,
    )? {
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
    let left = offset_axis(callbacks, &bleed_left, &live_width, &crop_width, &centering_x)?;
    let top = offset_axis(callbacks, &bleed_top, &live_height, &crop_height, &centering_y)?;
    let right = callbacks.arithmetic(ImageOpsAspectArithmetic::Add, &left, &crop_width)?;
    let bottom = callbacks.arithmetic(ImageOpsAspectArithmetic::Add, &top, &crop_height)?;
    callbacks.box_tuple(&left, &top, &right, &bottom)
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
    Ok(if in_range { value.clone() } else { half.clone() })
}

fn live_axis<C: ImageOpsFitCallbacks>(
    callbacks: &mut C,
    index: usize,
    bleed: &C::Value,
    two: &C::Value,
) -> Result<C::Value, C::Error> {
    let size = callbacks.image_size_coordinate(index)?;
    let bleed_twice = callbacks.arithmetic(ImageOpsAspectArithmetic::Multiply, bleed, two)?;
    callbacks.arithmetic(
        ImageOpsAspectArithmetic::Subtract,
        &size,
        &bleed_twice,
    )
}

fn offset_axis<C: ImageOpsFitCallbacks>(
    callbacks: &mut C,
    bleed: &C::Value,
    live: &C::Value,
    crop: &C::Value,
    centering: &C::Value,
) -> Result<C::Value, C::Error> {
    let remaining = callbacks.arithmetic(ImageOpsAspectArithmetic::Subtract, live, crop)?;
    let centered = callbacks.arithmetic(
        ImageOpsAspectArithmetic::Multiply,
        &remaining,
        centering,
    )?;
    callbacks.arithmetic(ImageOpsAspectArithmetic::Add, bleed, &centered)
}

/// Compute the size argument that ImageOps passes to public Image.resize.
pub fn aspect_size_with_callbacks<C: ImageOpsAspectCallbacks>(
    callbacks: &mut C,
    operation: ImageOpsAspectOperation,
) -> Result<ImageOpsAspectSize<C::Value>, C::Error> {
    let source_width = callbacks.image_width()?;
    let source_height = callbacks.image_height()?;
    let source_ratio = callbacks.arithmetic(
        ImageOpsAspectArithmetic::Divide,
        &source_width,
        &source_height,
    )?;
    let target_width = callbacks.size_coordinate(0)?;
    let target_height = callbacks.size_coordinate(1)?;
    let target_ratio = callbacks.arithmetic(
        ImageOpsAspectArithmetic::Divide,
        &target_width,
        &target_height,
    )?;
    if !callbacks.compare(
        ImageOpsAspectComparison::NotEqual,
        &source_ratio,
        &target_ratio,
    )? {
        return Ok(ImageOpsAspectSize::Original);
    }
    match operation {
        ImageOpsAspectOperation::Contain => contain_size(callbacks, &source_ratio, &target_ratio),
        ImageOpsAspectOperation::Cover => cover_size(callbacks, &source_ratio, &target_ratio),
    }
}

fn contain_size<C: ImageOpsAspectCallbacks>(
    callbacks: &mut C,
    source_ratio: &C::Value,
    target_ratio: &C::Value,
) -> Result<ImageOpsAspectSize<C::Value>, C::Error> {
    if callbacks.compare(
        ImageOpsAspectComparison::GreaterThan,
        source_ratio,
        target_ratio,
    )? {
        let height = rounded_scaled_axis(callbacks, 1, 0)?;
        let target_height = callbacks.size_coordinate(1)?;
        if callbacks.compare(ImageOpsAspectComparison::NotEqual, &height, &target_height)? {
            let width = callbacks.size_coordinate(0)?;
            return Ok(ImageOpsAspectSize::Resized(callbacks.size_pair(&width, &height)?));
        }
        return Ok(ImageOpsAspectSize::Original);
    }
    let width = rounded_scaled_axis(callbacks, 0, 1)?;
    let target_width = callbacks.size_coordinate(0)?;
    if callbacks.compare(ImageOpsAspectComparison::NotEqual, &width, &target_width)? {
        let height = callbacks.size_coordinate(1)?;
        return Ok(ImageOpsAspectSize::Resized(callbacks.size_pair(&width, &height)?));
    }
    Ok(ImageOpsAspectSize::Original)
}

fn cover_size<C: ImageOpsAspectCallbacks>(
    callbacks: &mut C,
    source_ratio: &C::Value,
    target_ratio: &C::Value,
) -> Result<ImageOpsAspectSize<C::Value>, C::Error> {
    if callbacks.compare(
        ImageOpsAspectComparison::LessThan,
        source_ratio,
        target_ratio,
    )? {
        let height = rounded_scaled_axis(callbacks, 1, 0)?;
        let target_height = callbacks.size_coordinate(1)?;
        if callbacks.compare(ImageOpsAspectComparison::NotEqual, &height, &target_height)? {
            let width = callbacks.size_coordinate(0)?;
            return Ok(ImageOpsAspectSize::Resized(callbacks.size_pair(&width, &height)?));
        }
        return Ok(ImageOpsAspectSize::Original);
    }
    let width = rounded_scaled_axis(callbacks, 0, 1)?;
    let target_width = callbacks.size_coordinate(0)?;
    if callbacks.compare(ImageOpsAspectComparison::NotEqual, &width, &target_width)? {
        let height = callbacks.size_coordinate(1)?;
        return Ok(ImageOpsAspectSize::Resized(callbacks.size_pair(&width, &height)?));
    }
    Ok(ImageOpsAspectSize::Original)
}

fn rounded_scaled_axis<C: ImageOpsAspectCallbacks>(
    callbacks: &mut C,
    image_numerator: usize,
    size_denominator: usize,
) -> Result<C::Value, C::Error> {
    let ratio = if image_numerator == 1 {
        let height = callbacks.image_height()?;
        let width = callbacks.image_width()?;
        callbacks.arithmetic(ImageOpsAspectArithmetic::Divide, &height, &width)?
    } else {
        let width = callbacks.image_width()?;
        let height = callbacks.image_height()?;
        callbacks.arithmetic(ImageOpsAspectArithmetic::Divide, &width, &height)?
    };
    let target = callbacks.size_coordinate(size_denominator)?;
    let scaled = callbacks.arithmetic(ImageOpsAspectArithmetic::Multiply, &ratio, &target)?;
    callbacks.round(&scaled)
}
