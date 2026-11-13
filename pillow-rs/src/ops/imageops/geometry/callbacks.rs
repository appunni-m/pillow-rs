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
            _ => Ok((
                border.clone(),
                border.clone(),
                border.clone(),
                border.clone(),
            )),
        }
    } else {
        Ok((
            border.clone(),
            border.clone(),
            border.clone(),
            border.clone(),
        ))
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

/// Host facts used to choose the native or protocol-driven ImageOps Crop path.
#[doc(hidden)]
pub trait ImageOpsCropRouteCallbacks: ImageOpsCropCallbacks
where
    <Self as ImageOpsBorderCallbacks>::Error: From<PilError>,
{
    /// Whether `type(image) is Image` at the Python facade.
    fn image_is_exact_base_type(&mut self) -> Result<bool, Self::Error>;
    /// Whether the facade's `Image.size` is still its captured base descriptor.
    fn size_property_is_base(&mut self) -> Result<bool, Self::Error>;
    /// Whether the facade's `Image.crop` is still its captured base method.
    fn crop_method_is_base(&mut self) -> Result<bool, Self::Error>;
    /// Whether the instance dictionary has no `crop` override.
    fn instance_crop_is_absent(&mut self) -> Result<bool, Self::Error>;
    /// Whether the border has the exact host integer type.
    fn border_is_exact_integer(&mut self, border: &Self::Value) -> Result<bool, Self::Error>;
    /// Apply the left operand of Pillow's `0 <= border` check.
    fn border_is_nonnegative(&mut self, border: &Self::Value) -> Result<bool, Self::Error>;
    /// Apply Pillow's `border <= 0xFFFFFFFF` check.
    fn border_fits_u32(&mut self, border: &Self::Value) -> Result<bool, Self::Error>;
    /// Extract the checked border as an unsigned 32-bit value.
    fn border_as_u32(&mut self, border: &Self::Value) -> Result<u32, Self::Error>;
    /// Snapshot the Rust image after native-route eligibility checks.
    fn native_image(&mut self) -> Result<crate::image::Image, Self::Error>;
    /// Run native crop geometry without holding the host runtime lock.
    fn run_core<T: Send>(&mut self, operation: impl FnOnce() -> T + Send) -> T;
    /// Convert the native geometry result to the host's crop-box value.
    fn native_box_tuple(&mut self, box_: (u32, u32, u32, u32)) -> Result<Self::Value, Self::Error>;
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
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

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
/// Select native crop geometry for the unmodified base-image route, otherwise
/// preserve Python's dynamic border protocols through the host callbacks.
pub fn crop_box_with_route_callbacks<C: ImageOpsCropRouteCallbacks>(
    callbacks: &mut C,
    border: &C::Value,
) -> Result<C::Value, C::Error>
where
    <C as ImageOpsBorderCallbacks>::Error: From<PilError>,
{
    let native_route = callbacks.image_is_exact_base_type()?
        && callbacks.size_property_is_base()?
        && callbacks.crop_method_is_base()?
        && callbacks.instance_crop_is_absent()?
        && callbacks.border_is_exact_integer(border)?
        && callbacks.border_is_nonnegative(border)?
        && callbacks.border_fits_u32(border)?;
    if !native_route {
        return crop_box_with_callbacks(callbacks, border);
    }

    let border = callbacks.border_as_u32(border)?;
    let image = callbacks.native_image()?;
    let box_ = callbacks.run_core(move || super::crop_box_geometry_preflight(&image, border))?;
    callbacks.native_box_tuple(box_)
}

/// Host operations required by the custom-border `ImageOps.expand` strategy.
#[doc(hidden)]
pub trait ImageOpsExpandGeometryCallbacks: ImageOpsBorderCallbacks {
    /// Read one coordinate from the current `image.size` property.
    fn image_size_coordinate(&mut self, index: usize) -> Result<Self::Value, Self::Error>;
    /// Add two host dimensions in operand order.
    fn add(&mut self, left: &Self::Value, right: &Self::Value) -> Result<Self::Value, Self::Error>;
}

/// Host protocols required by the core-owned ImageOps Expand workflow.
///
/// The callback adapter performs Python object operations and public wrapper
/// calls. Core owns their order, mode selection, fill preparation, and canvas
/// strategy.
#[doc(hidden)]
pub trait ImageOpsExpandCallbacks: ImageOpsExpandGeometryCallbacks
where
    <Self as ImageOpsBorderCallbacks>::Error: From<PilError>,
{
    /// Host palette wrapper used between source inspection and canvas creation.
    type Palette;
    /// Public image wrapper returned by the host boundary.
    type Output;
    /// Callable selected by the Python facade for wrapping the output image.
    type WrapperConstructor;

    /// Run a Rust-only phase without holding a host runtime lock.
    fn run_core<T: Send>(&mut self, operation: impl FnOnce() -> T + Send) -> T;

    /// Read the current source mode property.
    fn image_mode(&mut self) -> Result<Self::Value, Self::Error>;
    /// Test Python's `isinstance(value, str)` branch.
    fn value_is_string(&mut self, value: &Self::Value) -> Result<bool, Self::Error>;
    /// Resolve a string fill through the host ImageColor implementation.
    fn color_from_name(
        &mut self,
        fill: &Self::Value,
        mode: &Self::Value,
    ) -> Result<Self::Value, Self::Error>;
    /// Read the source palette property without forcing image materialization.
    fn image_palette(&mut self) -> Result<Option<Self::Palette>, Self::Error>;
    /// Apply the host truth test used by the source and copied palettes.
    fn palette_is_truthy(&mut self, palette: &Self::Palette) -> Result<bool, Self::Error>;
    /// Read the source palette's mode property.
    fn palette_mode(&mut self, palette: &Self::Palette) -> Result<Self::Value, Self::Error>;
    /// Copy the source palette using the public `getpalette` and constructor APIs.
    fn copy_palette(&mut self, mode: &Self::Value) -> Result<Self::Palette, Self::Error>;
    /// Test Python's `isinstance(fill, tuple)` branch.
    fn value_is_tuple(&mut self, value: &Self::Value) -> Result<bool, Self::Error>;
    /// Evaluate one source-ordered `len(fill) == expected` branch.
    fn value_length_equals(
        &mut self,
        value: &Self::Value,
        expected: i64,
    ) -> Result<bool, Self::Error>;
    /// Resolve a tuple fill with the copied palette's public `getcolor` method.
    fn palette_get_color(
        &mut self,
        palette: &Self::Palette,
        fill: &Self::Value,
    ) -> Result<Self::Value, Self::Error>;
    /// Construct the `(width, height)` tuple used by the size preflight.
    fn size_pair(
        &mut self,
        width: &Self::Value,
        height: &Self::Value,
    ) -> Result<Self::Value, Self::Error>;
    /// Apply Python's comparison and truth conversion for one size coordinate.
    fn size_is_negative(&mut self, size: &Self::Value, index: usize) -> Result<bool, Self::Error>;
    /// Extract one size coordinate through the host's C-int conversion.
    fn size_component_as_c_int(
        &mut self,
        size: &Self::Value,
        index: usize,
    ) -> Result<i32, Self::Error>;
    /// Parse the Pillow mode property into the core's typed mode identity.
    fn mode_identity(&mut self, value: &Self::Value) -> Result<crate::raster::Mode, Self::Error>;
    /// Adapt the fill object into the host-neutral typed fill input.
    fn fill_input(
        &mut self,
        mode: crate::raster::Mode,
        value: &Self::Value,
    ) -> Result<crate::ops::imageops::ImageOpsColor, Self::Error>;
    /// Resolve the facade's image constructor at the point Pillow evaluates it.
    fn capture_wrapper_constructor(&mut self) -> Result<Self::WrapperConstructor, Self::Error>;
    /// Construct the public Image wrapper around the prepared Rust canvas.
    fn wrap_canvas(
        &mut self,
        constructor: &Self::WrapperConstructor,
        image: crate::image::Image,
    ) -> Result<Self::Output, Self::Error>;
    /// Call the output wrapper's public `putpalette` method with its palette payload.
    fn put_palette(
        &mut self,
        output: &mut Self::Output,
        palette: &Self::Palette,
        mode: &Self::Value,
    ) -> Result<(), Self::Error>;
    /// Call the output wrapper's public `paste` method with the current source.
    fn paste(
        &mut self,
        output: &mut Self::Output,
        left: &Self::Value,
        top: &Self::Value,
    ) -> Result<(), Self::Error>;
}

/// Axis selected by the core-owned ImageOps Pad fallback.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageOpsPadAxis {
    /// Center the resized image horizontally.
    Horizontal,
    /// Center the resized image vertically.
    Vertical,
}

/// Host protocols required by the core-owned ImageOps Pad fallback.
#[doc(hidden)]
pub trait ImageOpsPadCallbacks {
    /// Callback error type.
    type Error: From<PilError>;
    /// Carrier for Python sizes, centering values, and arithmetic results.
    type Value: Clone;
    /// Public image wrapper used for resized and output images.
    type Image;
    /// Palette object returned by `Image.getpalette`.
    type Palette;

    /// Call the current public ImageOps `contain` function.
    fn contain(
        &mut self,
        size: &Self::Value,
        method: &Self::Value,
    ) -> Result<Self::Image, Self::Error>;
    /// Apply `resized.size == size` and truth conversion.
    fn resized_size_matches(
        &mut self,
        resized: &Self::Image,
        size: &Self::Value,
    ) -> Result<bool, Self::Error>;
    /// Call the current public `Image.new(image.mode, size, color)` expression.
    fn new_canvas(
        &mut self,
        size: &Self::Value,
        color: &Self::Value,
    ) -> Result<Self::Image, Self::Error>;
    /// Read `resized.palette` and apply its truth test.
    fn resized_palette_is_truthy(&mut self, resized: &Self::Image) -> Result<bool, Self::Error>;
    /// Call the public `resized.getpalette()` method.
    fn resized_palette(&mut self, resized: &Self::Image) -> Result<Self::Palette, Self::Error>;
    /// Test whether the returned palette is the Python `None` singleton.
    fn palette_is_none(&mut self, palette: &Self::Palette) -> Result<bool, Self::Error>;
    /// Call `output.putpalette(palette)` through the public wrapper.
    fn put_palette(
        &mut self,
        output: &mut Self::Image,
        palette: &Self::Palette,
    ) -> Result<(), Self::Error>;
    /// Apply `resized.width != size[0]` and truth conversion.
    fn width_differs(
        &mut self,
        resized: &Self::Image,
        size: &Self::Value,
    ) -> Result<bool, Self::Error>;
    /// Evaluate the selected Pillow centering offset expression.
    fn centered_offset(
        &mut self,
        axis: ImageOpsPadAxis,
        resized: &Self::Image,
        size: &Self::Value,
        centering: &Self::Value,
    ) -> Result<Self::Value, Self::Error>;
    /// Call `output.paste(resized, position)` through the public wrapper.
    fn paste_at(
        &mut self,
        output: &mut Self::Image,
        resized: &Self::Image,
        axis: ImageOpsPadAxis,
        offset: &Self::Value,
    ) -> Result<(), Self::Error>;
}

/// Cached Image property checked by the native ImageOps size route.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageOpsNativeSizeProperty {
    /// The base `Image.size` descriptor.
    Size,
    /// The base `Image.width` descriptor.
    Width,
    /// The base `Image.height` descriptor.
    Height,
}

/// Host capability facts used to select the native ImageOps size strategy.
#[doc(hidden)]
pub trait ImageOpsNativeSizeCallbacks {
    /// Host value type used for the two size components.
    type Value;
    /// Host callback error type.
    type Error;

    /// Whether `image` is an instance of the facade's current Image class.
    fn image_is_instance(&mut self) -> Result<bool, Self::Error>;
    /// Whether instance property lookup uses `object.__getattribute__`.
    fn image_uses_default_getattribute(&mut self) -> Result<bool, Self::Error>;
    /// Whether a class descriptor is the import-time base Image descriptor.
    fn image_property_matches_base(
        &mut self,
        property: ImageOpsNativeSizeProperty,
    ) -> Result<bool, Self::Error>;
    /// Whether the requested size has the exact built-in tuple type.
    fn size_is_exact_tuple(&mut self) -> Result<bool, Self::Error>;
    /// Return the requested size tuple length.
    fn size_length(&mut self) -> Result<usize, Self::Error>;
    /// Read one item from the requested size tuple.
    fn size_coordinate(&mut self, index: usize) -> Result<Self::Value, Self::Error>;
    /// Whether a coordinate has the exact built-in integer type.
    fn coordinate_is_exact_integer(&mut self, value: &Self::Value) -> Result<bool, Self::Error>;
    /// Whether an exact integer lies within the unsigned 32-bit range.
    fn coordinate_is_u32(&mut self, value: &Self::Value) -> Result<bool, Self::Error>;
    /// Extract an integer after the core has admitted it to the u32 range.
    fn coordinate_as_u32(&mut self, value: &Self::Value) -> Result<u32, Self::Error>;
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
/// Selects the native ImageOps size route from ordered host capability facts.
#[doc(hidden)]
pub fn native_size_pair_with_callbacks<C: ImageOpsNativeSizeCallbacks>(
    callbacks: &mut C,
) -> Result<Option<(u32, u32)>, C::Error> {
    if !callbacks.image_is_instance()? {
        return Ok(None);
    }
    if !callbacks.image_uses_default_getattribute()? {
        return Ok(None);
    }
    for property in [
        ImageOpsNativeSizeProperty::Size,
        ImageOpsNativeSizeProperty::Width,
        ImageOpsNativeSizeProperty::Height,
    ] {
        if !callbacks.image_property_matches_base(property)? {
            return Ok(None);
        }
    }
    if !callbacks.size_is_exact_tuple()? || callbacks.size_length()? != 2 {
        return Ok(None);
    }
    let width = callbacks.size_coordinate(0)?;
    let height = callbacks.size_coordinate(1)?;
    if !callbacks.coordinate_is_exact_integer(&width)?
        || !callbacks.coordinate_is_exact_integer(&height)?
        || !callbacks.coordinate_is_u32(&width)?
        || !callbacks.coordinate_is_u32(&height)?
    {
        return Ok(None);
    }
    Ok(Some((
        callbacks.coordinate_as_u32(&width)?,
        callbacks.coordinate_as_u32(&height)?,
    )))
}

/// Host capabilities needed to route contain/cover geometry from one core entry point.
#[doc(hidden)]
pub trait ImageOpsAspectRouteCallbacks:
    ImageOpsAspectCallbacks
    + ImageOpsNativeSizeCallbacks<
        Value = <Self as ImageOpsAspectCallbacks>::Value,
        Error = <Self as ImageOpsAspectCallbacks>::Error,
    >
{
    /// Run native geometry without holding the host runtime lock.
    fn run_core<T: Send>(&mut self, operation: impl FnOnce() -> T + Send) -> T;
    /// Snapshot the core image only after native-size admission succeeds.
    fn native_image(
        &mut self,
    ) -> Result<crate::image::Image, <Self as ImageOpsAspectCallbacks>::Error>;
    /// Construct a host size pair from native geometry results.
    fn host_size_pair(
        &mut self,
        width: u32,
        height: u32,
    ) -> Result<<Self as ImageOpsAspectCallbacks>::Value, <Self as ImageOpsAspectCallbacks>::Error>;
}

pub(super) fn expand_geometry_values_with_callbacks<C: ImageOpsExpandGeometryCallbacks>(
    callbacks: &mut C,
    border: &C::Value,
) -> Result<(C::Value, C::Value, C::Value, C::Value), C::Error> {
    let (left, top, right, bottom) = border_components(callbacks, border)?;
    let source_width = callbacks.image_size_coordinate(0)?;
    let width_with_source = callbacks.add(&left, &source_width)?;
    let width = callbacks.add(&width_with_source, &right)?;
    let source_height = callbacks.image_size_coordinate(1)?;
    let height_with_source = callbacks.add(&top, &source_height)?;
    let height = callbacks.add(&height_with_source, &bottom)?;
    Ok((width, height, left, top))
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
/// Compute the size argument that ImageOps passes to public Image.resize.
fn aspect_size_with_callbacks<C: ImageOpsAspectCallbacks>(
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

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
/// Selects native or callback-driven contain/cover geometry from one core route.
#[doc(hidden)]
pub fn aspect_size_with_route_callbacks<C: ImageOpsAspectRouteCallbacks>(
    callbacks: &mut C,
    operation: ImageOpsAspectOperation,
) -> Result<
    ImageOpsAspectSize<<C as ImageOpsAspectCallbacks>::Value>,
    <C as ImageOpsAspectCallbacks>::Error,
> {
    if let Some((width, height)) = native_size_pair_with_callbacks(callbacks)? {
        let image = callbacks.native_image()?;
        let (width, height) = callbacks.run_core(move || match operation {
            ImageOpsAspectOperation::Contain => {
                super::contain_geometry_preflight(&image, width, height)
            }
            ImageOpsAspectOperation::Cover => {
                super::cover_geometry_preflight(&image, width, height)
            }
        })?;
        return Ok(ImageOpsAspectSize::Resized(
            callbacks.host_size_pair(width, height)?,
        ));
    }
    aspect_size_with_callbacks(callbacks, operation)
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
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
            return Ok(ImageOpsAspectSize::Resized(
                callbacks.size_pair(&width, &height)?,
            ));
        }
        return Ok(ImageOpsAspectSize::Original);
    }
    let width = rounded_scaled_axis(callbacks, 0, 1)?;
    let target_width = callbacks.size_coordinate(0)?;
    if callbacks.compare(ImageOpsAspectComparison::NotEqual, &width, &target_width)? {
        let height = callbacks.size_coordinate(1)?;
        return Ok(ImageOpsAspectSize::Resized(
            callbacks.size_pair(&width, &height)?,
        ));
    }
    Ok(ImageOpsAspectSize::Original)
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
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
            return Ok(ImageOpsAspectSize::Resized(
                callbacks.size_pair(&width, &height)?,
            ));
        }
        return Ok(ImageOpsAspectSize::Original);
    }
    let width = rounded_scaled_axis(callbacks, 0, 1)?;
    let target_width = callbacks.size_coordinate(0)?;
    if callbacks.compare(ImageOpsAspectComparison::NotEqual, &width, &target_width)? {
        let height = callbacks.size_coordinate(1)?;
        return Ok(ImageOpsAspectSize::Resized(
            callbacks.size_pair(&width, &height)?,
        ));
    }
    Ok(ImageOpsAspectSize::Original)
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
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
