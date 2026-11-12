//! Host-callback flood fill with Pillow's public mutation and fault boundaries.

use crate::compute::registry::support::StrategySelection;
use crate::error::PilError;
use crate::image::Image;
use crate::raster::Mode;

/// Host operations required by Pillow's public flood-fill algorithm.
///
/// Python bindings implement these callbacks with Python's own coordinate,
/// pixel-access, arithmetic, equality, and set behavior. The core owns the
/// traversal order and the points at which Pillow swallows or propagates
/// callback errors.
#[doc(hidden)]
pub trait FloodFillAccess {
    /// Coordinate value passed to the host's pixel-access object.
    type Point: Clone;
    /// Pixel value returned by the host's pixel-access object.
    type Pixel: Clone;
    /// Threshold value supplied by the caller.
    type Threshold;
    /// Tuple-like color vector produced by Pillow's scalar wrapping rule.
    type ColorVector;
    /// Absolute difference between one pair of color samples.
    type ColorDifference;
    /// Running total used by Pillow's `sum` semantics.
    type ColorSum;
    /// Host set used for the current traversal frontier.
    type Frontier;
    /// Iterator over the host frontier.
    type FrontierIter;
    /// Error raised by one of the host operations.
    type Error;

    /// Creates the concrete empty set used by Pillow's traversal.
    fn new_frontier(&mut self) -> Result<Self::Frontier, Self::Error>;
    /// Checks the host frontier's truth value.
    fn frontier_is_empty(&mut self, frontier: &Self::Frontier) -> Result<bool, Self::Error>;
    /// Creates an iterator using the host's set iteration order.
    fn frontier_iter(
        &mut self,
        frontier: &Self::Frontier,
    ) -> Result<Self::FrontierIter, Self::Error>;
    /// Returns the next point from an existing frontier iterator.
    fn frontier_next(
        &mut self,
        iter: &mut Self::FrontierIter,
    ) -> Result<Option<Self::Point>, Self::Error>;
    /// Checks whether the host frontier contains a point.
    fn frontier_contains(
        &mut self,
        frontier: &Self::Frontier,
        point: &Self::Point,
    ) -> Result<bool, Self::Error>;
    /// Adds a point to a host frontier.
    fn frontier_insert(
        &mut self,
        frontier: &mut Self::Frontier,
        point: Self::Point,
    ) -> Result<(), Self::Error>;
    /// Constructs the ordered right, left, down, and up neighbors.
    fn neighbors(&mut self, point: &Self::Point) -> Result<[Self::Point; 4], Self::Error>;
    /// Applies Pillow's nonnegative-coordinate guard.
    fn point_is_nonnegative(&mut self, point: &Self::Point) -> Result<bool, Self::Error>;
    /// Reads one point through the host's pixel-access protocol.
    fn get_pixel(&mut self, point: &Self::Point) -> Result<Self::Pixel, Self::Error>;
    /// Writes one point through the host's pixel-access protocol.
    fn set_pixel(&mut self, point: &Self::Point, value: &Self::Pixel) -> Result<(), Self::Error>;
    /// Applies Pillow's tuple-or-scalar rule to both color values.
    fn color_vectors(
        &mut self,
        first: &Self::Pixel,
        second: &Self::Pixel,
    ) -> Result<(Self::ColorVector, Self::ColorVector), Self::Error>;
    /// Returns `len(second)` after the color values have been wrapped.
    fn color_vector_len(&mut self, vector: &Self::ColorVector) -> Result<usize, Self::Error>;
    /// Computes `abs(first[index] - second[index])` through host protocols.
    fn color_difference_at(
        &mut self,
        first: &Self::ColorVector,
        second: &Self::ColorVector,
        index: usize,
    ) -> Result<Self::ColorDifference, Self::Error>;
    /// Creates the exact zero value passed to Pillow's `sum`.
    fn color_sum_start(&mut self) -> Result<Self::ColorSum, Self::Error>;
    /// Adds one difference to the running host sum.
    fn add_color_difference(
        &mut self,
        sum: Self::ColorSum,
        difference: Self::ColorDifference,
    ) -> Result<Self::ColorSum, Self::Error>;
    /// Evaluates `color_sum <= threshold` using host comparison behavior.
    fn color_sum_is_within_threshold(
        &mut self,
        sum: &Self::ColorSum,
        threshold: &Self::Threshold,
    ) -> Result<bool, Self::Error>;
    /// Evaluates `pixel not in (value, border)` through host sequence semantics.
    fn differs_from_fill_and_border(
        &mut self,
        pixel: &Self::Pixel,
        value: &Self::Pixel,
        border: &Self::Pixel,
    ) -> Result<bool, Self::Error>;
    /// Identifies only the ValueError and IndexError swallowed by Pillow.
    fn is_swallowed_error(&self, error: &Self::Error) -> bool;
}

/// Prepared CPU mode strategy retained before host callbacks begin.
#[doc(hidden)]
pub struct PreparedFloodFill {
    mode: Mode,
    selection: StrategySelection,
}

impl PreparedFloodFill {
    pub(crate) const fn new(mode: Mode, selection: StrategySelection) -> Self {
        Self { mode, selection }
    }

    pub(crate) const fn mode(&self) -> Mode {
        self.mode
    }

    pub(crate) const fn selection(&self) -> &StrategySelection {
        &self.selection
    }
}

/// Failure from strategy validation or a host callback during flood fill.
#[doc(hidden)]
pub enum FloodFillError<E> {
    /// The prepared mode/architecture strategy was inconsistent.
    Strategy(PilError),
    /// The host raised an error while following Pillow's callback contract.
    Callback(E),
}

/// Selects the exact CPU mode strategy from the image's stored raster identity
/// and routed execution context.
#[doc(hidden)]
pub fn prepare_floodfill(image: &Image) -> Result<PreparedFloodFill, PilError> {
    let mode = image.mode_identity()?;
    let (_, context) = image.cpu_arch_execution_route_advisory();
    crate::compute::pool_cpu::ops::draw::ops::prepare_floodfill(mode, &context)
}

/// Executes Pillow-compatible flood fill through the supplied host callbacks.
#[doc(hidden)]
pub fn execute_floodfill<A: FloodFillAccess>(
    prepared: Option<&PreparedFloodFill>,
    access: &mut A,
    seed: A::Point,
    value: A::Pixel,
    border: Option<A::Pixel>,
    threshold: A::Threshold,
) -> Result<(), FloodFillError<A::Error>> {
    if let Some(prepared) = prepared {
        return crate::compute::pool_cpu::ops::draw::ops::execute_floodfill(
            prepared,
            access,
            seed,
            &value,
            border.as_ref(),
            &threshold,
        );
    }
    crate::compute::pool_cpu::ops::draw::helpers::floodfill::run(
        access,
        seed,
        &value,
        border.as_ref(),
        &threshold,
    )
    .map_err(FloodFillError::Callback)
}
