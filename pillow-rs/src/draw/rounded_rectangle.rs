use crate::error::PilError;

/// Arithmetic and Python protocol operations requested by the Rust geometry
/// strategy while preparing a Pillow rounded rectangle.
#[doc(hidden)]
pub trait RoundedRectanglePreparationCallbacks {
    /// Host numeric value retained until Pillow's primitive boundary.
    type Value: Clone;
    /// Host callback failure, such as a custom numeric protocol exception.
    type Error;

    /// Construct a Python-compatible integer constant.
    fn integer(&mut self, value: i32) -> Result<Self::Value, Self::Error>;
    /// Execute the original value's multiplication protocol.
    fn multiply(
        &mut self,
        left: &Self::Value,
        right: &Self::Value,
    ) -> Result<Self::Value, Self::Error>;
    /// Apply Python's `round` protocol to one coordinate.
    fn round(&mut self, value: &Self::Value) -> Result<Self::Value, Self::Error>;
    /// Execute Python's less-than comparison and truth conversion.
    fn less_than(&mut self, left: &Self::Value, right: &Self::Value) -> Result<bool, Self::Error>;
    /// Execute Python subtraction.
    fn subtract(
        &mut self,
        left: &Self::Value,
        right: &Self::Value,
    ) -> Result<Self::Value, Self::Error>;
    /// Execute Python's greater-than-or-equal comparison and truth conversion.
    fn greater_equal(
        &mut self,
        left: &Self::Value,
        right: &Self::Value,
    ) -> Result<bool, Self::Error>;
    /// Compare a diameter with integer zero.
    fn equals_zero(&mut self, value: &Self::Value) -> Result<bool, Self::Error>;
    /// Evaluate Pillow's `all(corners)` call.
    fn all_corners(&mut self) -> Result<bool, Self::Error>;
    /// Evaluate Pillow's `any(corners)` call.
    fn any_corners(&mut self) -> Result<bool, Self::Error>;
    /// Execute Python floor division by two.
    fn floor_half(&mut self, value: &Self::Value) -> Result<Self::Value, Self::Error>;
    /// Apply Python's `int` conversion to the floored half-diameter.
    fn to_integer(&mut self, value: &Self::Value) -> Result<Self::Value, Self::Error>;
}

/// Host numeric operations needed when Pillow builds an outline border.
#[doc(hidden)]
pub trait RoundedRectangleOutlineCallbacks: RoundedRectanglePreparationCallbacks {
    /// Execute Python addition for a border coordinate.
    fn add(&mut self, left: &Self::Value, right: &Self::Value) -> Result<Self::Value, Self::Error>;
    /// Execute Python's in-place addition protocol for a list item `+=`.
    fn in_place_add(
        &mut self,
        left: &Self::Value,
        right: &Self::Value,
    ) -> Result<Self::Value, Self::Error>;
    /// Execute Python's in-place subtraction protocol for a list item `-=`.
    fn in_place_subtract(
        &mut self,
        left: &Self::Value,
        right: &Self::Value,
    ) -> Result<Self::Value, Self::Error>;
    /// Fetch and truth-test `corners[index]` at Pillow's border step.
    fn corner_truthy(&mut self, index: usize) -> Result<bool, Self::Error>;
}

/// Build one outline border rectangle while keeping Python numeric operations
/// at the point where Pillow evaluates them.
#[doc(hidden)]
pub fn rounded_rectangle_outline_bounds_with_callbacks<C: RoundedRectangleOutlineCallbacks>(
    kind: RoundedRectangleOutlineBorder,
    coordinates: [C::Value; 4],
    radius: &C::Value,
    width: &C::Value,
    callbacks: &mut C,
) -> Result<[C::Value; 4], C::Error> {
    let mut bounds = coordinates;
    apply_border_width(kind, &mut bounds, width, callbacks)?;
    let (positive_corner, positive_coordinate, negative_corner, negative_coordinate) =
        border_corner_adjustments(kind);
    if callbacks.corner_truthy(positive_corner)? {
        let positive_offset = border_offset(radius, callbacks)?;
        let current = bounds[positive_coordinate].clone();
        bounds[positive_coordinate] = callbacks.in_place_add(&current, &positive_offset)?;
    }
    if callbacks.corner_truthy(negative_corner)? {
        let positive_offset = border_offset(radius, callbacks)?;
        let current = bounds[negative_coordinate].clone();
        bounds[negative_coordinate] = callbacks.in_place_subtract(&current, &positive_offset)?;
    }
    Ok(bounds)
}

fn border_offset<C: RoundedRectangleOutlineCallbacks>(
    radius: &C::Value,
    callbacks: &mut C,
) -> Result<C::Value, C::Error> {
    let one = callbacks.integer(1)?;
    callbacks.add(radius, &one)
}

fn apply_border_width<C: RoundedRectangleOutlineCallbacks>(
    kind: RoundedRectangleOutlineBorder,
    bounds: &mut [C::Value; 4],
    width: &C::Value,
    callbacks: &mut C,
) -> Result<(), C::Error> {
    let (coordinate, base, subtract) = match kind {
        RoundedRectangleOutlineBorder::Top => (3, 1, false),
        RoundedRectangleOutlineBorder::Bottom => (1, 3, true),
        RoundedRectangleOutlineBorder::Left => (2, 0, false),
        RoundedRectangleOutlineBorder::Right => (0, 2, true),
    };
    let origin = bounds[base].clone();
    let shifted = if subtract {
        callbacks.subtract(&origin, width)?
    } else {
        callbacks.add(&origin, width)?
    };
    let one = callbacks.integer(1)?;
    bounds[coordinate] = match kind {
        RoundedRectangleOutlineBorder::Top | RoundedRectangleOutlineBorder::Left => {
            callbacks.subtract(&shifted, &one)?
        }
        RoundedRectangleOutlineBorder::Bottom | RoundedRectangleOutlineBorder::Right => {
            callbacks.add(&shifted, &one)?
        }
    };
    Ok(())
}

fn border_corner_adjustments(kind: RoundedRectangleOutlineBorder) -> (usize, usize, usize, usize) {
    match kind {
        RoundedRectangleOutlineBorder::Top => (0, 0, 1, 2),
        RoundedRectangleOutlineBorder::Bottom => (3, 0, 2, 2),
        RoundedRectangleOutlineBorder::Left => (0, 1, 3, 3),
        RoundedRectangleOutlineBorder::Right => (1, 1, 2, 3),
    }
}

/// Public primitive selected by rounded-rectangle geometry preparation.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoundedRectangleEarlyPath {
    /// Pillow delegates a fully joined rounded rectangle to `ellipse`.
    Ellipse,
    /// Pillow delegates a zero-diameter or cornerless shape to `rectangle`.
    Rectangle,
}

/// Geometry values prepared by the core before colors and draw primitives.
#[doc(hidden)]
pub struct RoundedRectanglePreparedGeometry<Value> {
    /// Coordinates after Pillow's ordered `round()` calls.
    pub coordinates: [Value; 4],
    /// Diameter after sequential full-axis clipping.
    pub diameter: Value,
    /// Pillow's integer floor-half diameter, absent on an early route.
    pub radius: Option<Value>,
    /// Whether the two horizontal corner pairs form one continuous half-curve.
    pub full_x: bool,
    /// Whether the two vertical corner pairs form one continuous half-curve.
    pub full_y: bool,
    /// Early public method dispatch, if geometry selects one.
    pub early_path: Option<RoundedRectangleEarlyPath>,
}

/// Prepare the mode-independent rounded-rectangle geometry in Rust.
///
/// Numeric protocols remain host callbacks so custom Python values keep their
/// public behavior. Rust owns their order, clipping decisions, and early-path
/// selection; each callback result is retained without narrowing.
#[doc(hidden)]
pub fn prepare_rounded_rectangle_with_callbacks<C: RoundedRectanglePreparationCallbacks>(
    coordinates: [C::Value; 4],
    radius: C::Value,
    callbacks: &mut C,
) -> Result<RoundedRectanglePreparedGeometry<C::Value>, RoundedRectanglePreparationError<C::Error>>
{
    validate_coordinate_order(&coordinates, callbacks)?;
    let diameter = initial_diameter(&coordinates, &radius, callbacks)?;
    let rounded = round_coordinates(coordinates, callbacks)?;
    let (diameter, full_x, full_y) = clip_full_axes(diameter, &rounded, callbacks)?;
    let early_path = early_path(diameter.clone(), full_x, full_y, callbacks)?;
    if let Some(path) = early_path {
        return Ok(prepared(
            rounded,
            diameter,
            None,
            full_x,
            full_y,
            Some(path),
        ));
    }
    let half = callbacks
        .floor_half(&diameter)
        .map_err(RoundedRectanglePreparationError::Callback)?;
    let radius = callbacks
        .to_integer(&half)
        .map_err(RoundedRectanglePreparationError::Callback)?;
    Ok(prepared(
        rounded,
        diameter,
        Some(radius),
        full_x,
        full_y,
        None,
    ))
}

fn initial_diameter<C: RoundedRectanglePreparationCallbacks>(
    coordinates: &[C::Value; 4],
    radius: &C::Value,
    callbacks: &mut C,
) -> Result<C::Value, RoundedRectanglePreparationError<C::Error>> {
    let width = callbacks
        .subtract(&coordinates[2], &coordinates[0])
        .map_err(RoundedRectanglePreparationError::Callback)?;
    let height = callbacks
        .subtract(&coordinates[3], &coordinates[1])
        .map_err(RoundedRectanglePreparationError::Callback)?;
    let two = callbacks
        .integer(2)
        .map_err(RoundedRectanglePreparationError::Callback)?;
    let doubled_radius = callbacks
        .multiply(radius, &two)
        .map_err(RoundedRectanglePreparationError::Callback)?;
    let smaller_extent = if callbacks
        .less_than(&height, &width)
        .map_err(RoundedRectanglePreparationError::Callback)?
    {
        height
    } else {
        width
    };
    if callbacks
        .less_than(&doubled_radius, &smaller_extent)
        .map_err(RoundedRectanglePreparationError::Callback)?
    {
        Ok(doubled_radius)
    } else {
        Ok(smaller_extent)
    }
}

/// Core error versus a failure raised by a host numeric protocol.
#[doc(hidden)]
pub enum RoundedRectanglePreparationError<Error> {
    /// Pillow's public inverted-coordinate fault.
    Core(PilError),
    /// An exception raised while invoking a Python protocol.
    Callback(Error),
}

fn validate_coordinate_order<C: RoundedRectanglePreparationCallbacks>(
    coordinates: &[C::Value; 4],
    callbacks: &mut C,
) -> Result<(), RoundedRectanglePreparationError<C::Error>> {
    if callbacks
        .less_than(&coordinates[2], &coordinates[0])
        .map_err(RoundedRectanglePreparationError::Callback)?
    {
        return Err(RoundedRectanglePreparationError::Core(
            PilError::ValueError("x1 must be greater than or equal to x0".into()),
        ));
    }
    if callbacks
        .less_than(&coordinates[3], &coordinates[1])
        .map_err(RoundedRectanglePreparationError::Callback)?
    {
        return Err(RoundedRectanglePreparationError::Core(
            PilError::ValueError("y1 must be greater than or equal to y0".into()),
        ));
    }
    Ok(())
}

fn round_coordinates<C: RoundedRectanglePreparationCallbacks>(
    coordinates: [C::Value; 4],
    callbacks: &mut C,
) -> Result<[C::Value; 4], RoundedRectanglePreparationError<C::Error>> {
    Ok([
        callbacks
            .round(&coordinates[0])
            .map_err(RoundedRectanglePreparationError::Callback)?,
        callbacks
            .round(&coordinates[1])
            .map_err(RoundedRectanglePreparationError::Callback)?,
        callbacks
            .round(&coordinates[2])
            .map_err(RoundedRectanglePreparationError::Callback)?,
        callbacks
            .round(&coordinates[3])
            .map_err(RoundedRectanglePreparationError::Callback)?,
    ])
}

fn clip_full_axes<C: RoundedRectanglePreparationCallbacks>(
    mut diameter: C::Value,
    coordinates: &[C::Value; 4],
    callbacks: &mut C,
) -> Result<(C::Value, bool, bool), RoundedRectanglePreparationError<C::Error>> {
    if !callbacks
        .all_corners()
        .map_err(RoundedRectanglePreparationError::Callback)?
    {
        return Ok((diameter, false, false));
    }
    let (next, full_x) = clip_axis(&diameter, &coordinates[2], &coordinates[0], callbacks)?;
    diameter = next;
    let (next, full_y) = clip_axis(&diameter, &coordinates[3], &coordinates[1], callbacks)?;
    Ok((next, full_x, full_y))
}

fn clip_axis<C: RoundedRectanglePreparationCallbacks>(
    diameter: &C::Value,
    end: &C::Value,
    start: &C::Value,
    callbacks: &mut C,
) -> Result<(C::Value, bool), RoundedRectanglePreparationError<C::Error>> {
    let extent = callbacks
        .subtract(end, start)
        .map_err(RoundedRectanglePreparationError::Callback)?;
    let one = callbacks
        .integer(1)
        .map_err(RoundedRectanglePreparationError::Callback)?;
    let limit = callbacks
        .subtract(&extent, &one)
        .map_err(RoundedRectanglePreparationError::Callback)?;
    if !callbacks
        .greater_equal(diameter, &limit)
        .map_err(RoundedRectanglePreparationError::Callback)?
    {
        return Ok((diameter.clone(), false));
    }
    let clipped = callbacks
        .subtract(end, start)
        .map_err(RoundedRectanglePreparationError::Callback)?;
    Ok((clipped, true))
}

fn early_path<C: RoundedRectanglePreparationCallbacks>(
    diameter: C::Value,
    full_x: bool,
    full_y: bool,
    callbacks: &mut C,
) -> Result<Option<RoundedRectangleEarlyPath>, RoundedRectanglePreparationError<C::Error>> {
    if full_x && full_y {
        return Ok(Some(RoundedRectangleEarlyPath::Ellipse));
    }
    if callbacks
        .equals_zero(&diameter)
        .map_err(RoundedRectanglePreparationError::Callback)?
        || !callbacks
            .any_corners()
            .map_err(RoundedRectanglePreparationError::Callback)?
    {
        return Ok(Some(RoundedRectangleEarlyPath::Rectangle));
    }
    Ok(None)
}

fn prepared<Value>(
    coordinates: [Value; 4],
    diameter: Value,
    radius: Option<Value>,
    full_x: bool,
    full_y: bool,
    early_path: Option<RoundedRectangleEarlyPath>,
) -> RoundedRectanglePreparedGeometry<Value> {
    RoundedRectanglePreparedGeometry {
        coordinates,
        diameter,
        radius,
        full_x,
        full_y,
        early_path,
    }
}

/// Four-byte draw color used by the hidden rounded-rectangle adapter.
#[doc(hidden)]
pub type RoundedRectangleColor = (u8, u8, u8, u8);
/// Numeric bounds retained until Pillow's native draw primitive parses them.
#[doc(hidden)]
pub type RoundedRectangleBounds<Value> = [Value; 4];

/// One of Pillow's sequential rounded-rectangle border rectangles.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoundedRectangleOutlineBorder {
    /// Top horizontal border.
    Top,
    /// Bottom horizontal border.
    Bottom,
    /// Left vertical border.
    Left,
    /// Right vertical border.
    Right,
}

#[path = "rounded_rectangle_program.rs"]
mod program;
pub use program::{
    RoundedRectangleDrawStep, RoundedRectangleProgram, RoundedRectangleProgramCallbacks,
    RoundedRectangleProgramError, RoundedRectangleProgramResponse, RoundedRectangleProgramStep,
};
