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
    crate::compute::pool_cpu::ops::draw::floodfill::ops::prepare(mode, &context)
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
        return crate::compute::pool_cpu::ops::draw::floodfill::ops::execute(
            prepared,
            access,
            seed,
            &value,
            border.as_ref(),
            &threshold,
        );
    }
    run(access, seed, &value, border.as_ref(), &threshold).map_err(FloodFillError::Callback)
}

pub(crate) fn run<A: FloodFillAccess>(
    access: &mut A,
    seed: A::Point,
    value: &A::Pixel,
    border: Option<&A::Pixel>,
    threshold: &A::Threshold,
) -> Result<(), A::Error> {
    let Some(background) = fill_seed(access, &seed, value, threshold)? else {
        return Ok(());
    };
    let mut edge = access.new_frontier()?;
    access.frontier_insert(&mut edge, seed)?;
    let mut full_edge = access.new_frontier()?;
    while !access.frontier_is_empty(&edge)? {
        let mut next_edge = access.new_frontier()?;
        fill_frontier(
            access,
            &edge,
            &mut full_edge,
            &mut next_edge,
            value,
            border,
            &background,
            threshold,
        )?;
        // Pillow retains only the just-processed edge for the next pass.
        full_edge = edge;
        edge = next_edge;
    }
    Ok(())
}

fn fill_seed<A: FloodFillAccess>(
    access: &mut A,
    seed: &A::Point,
    value: &A::Pixel,
    threshold: &A::Threshold,
) -> Result<Option<A::Pixel>, A::Error> {
    let background = match access.get_pixel(seed) {
        Ok(background) => background,
        Err(error) if access.is_swallowed_error(&error) => return Ok(None),
        Err(error) => return Err(error),
    };
    let should_fill = match color_diff_is_within(access, value, &background, threshold) {
        Ok(should_fill) => should_fill,
        Err(error) if access.is_swallowed_error(&error) => return Ok(None),
        Err(error) => return Err(error),
    };
    if !should_fill {
        return Ok(None);
    }
    match access.set_pixel(seed, value) {
        Ok(()) => Ok(Some(background)),
        Err(error) if access.is_swallowed_error(&error) => Ok(None),
        Err(error) => Err(error),
    }
}

fn fill_frontier<A: FloodFillAccess>(
    access: &mut A,
    edge: &A::Frontier,
    full_edge: &mut A::Frontier,
    next_edge: &mut A::Frontier,
    value: &A::Pixel,
    border: Option<&A::Pixel>,
    background: &A::Pixel,
    threshold: &A::Threshold,
) -> Result<(), A::Error> {
    let mut points = access.frontier_iter(edge)?;
    while let Some(point) = access.frontier_next(&mut points)? {
        fill_neighbors(
            access, &point, full_edge, next_edge, value, border, background, threshold,
        )?;
    }
    Ok(())
}

fn fill_neighbors<A: FloodFillAccess>(
    access: &mut A,
    point: &A::Point,
    full_edge: &mut A::Frontier,
    next_edge: &mut A::Frontier,
    value: &A::Pixel,
    border: Option<&A::Pixel>,
    background: &A::Pixel,
    threshold: &A::Threshold,
) -> Result<(), A::Error> {
    for neighbor in access.neighbors(point)? {
        // Pillow checks set membership before its short-circuiting negatives.
        if access.frontier_contains(full_edge, &neighbor)?
            || !access.point_is_nonnegative(&neighbor)?
        {
            continue;
        }
        let pixel = match access.get_pixel(&neighbor) {
            Ok(pixel) => pixel,
            Err(error) if access.is_swallowed_error(&error) => continue,
            Err(error) => return Err(error),
        };
        access.frontier_insert(full_edge, neighbor.clone())?;
        if should_fill_neighbor(access, &pixel, value, border, background, threshold)? {
            access.set_pixel(&neighbor, value)?;
            access.frontier_insert(next_edge, neighbor)?;
        }
    }
    Ok(())
}

fn should_fill_neighbor<A: FloodFillAccess>(
    access: &mut A,
    pixel: &A::Pixel,
    value: &A::Pixel,
    border: Option<&A::Pixel>,
    background: &A::Pixel,
    threshold: &A::Threshold,
) -> Result<bool, A::Error> {
    match border {
        Some(border) => access.differs_from_fill_and_border(pixel, value, border),
        None => color_diff_is_within(access, pixel, background, threshold),
    }
}

fn color_diff_is_within<A: FloodFillAccess>(
    access: &mut A,
    first: &A::Pixel,
    second: &A::Pixel,
    threshold: &A::Threshold,
) -> Result<bool, A::Error> {
    let (first, second) = access.color_vectors(first, second)?;
    let length = access.color_vector_len(&second)?;
    let mut sum = access.color_sum_start()?;
    // Pillow iterates range(len(second)), even when first has fewer samples.
    for index in 0..length {
        let difference = access.color_difference_at(&first, &second, index)?;
        sum = access.add_color_difference(sum, difference)?;
    }
    access.color_sum_is_within_threshold(&sum, threshold)
}
