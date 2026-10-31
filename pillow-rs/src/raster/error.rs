//! Errors and geometry used by Pillow's in-memory raster operations.

use std::fmt;

/// Failure produced by an in-memory raster operation.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum RasterError {
    /// Image dimensions or placement are incompatible with the operation.
    Dimensions(String),
}

impl fmt::Display for RasterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Dimensions(message) => write!(formatter, "invalid image dimensions: {message}"),
        }
    }
}

impl std::error::Error for RasterError {}

/// A rectangular region within an in-memory raster.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    /// Horizontal coordinate of the top-left corner.
    pub x: u32,
    /// Vertical coordinate of the top-left corner.
    pub y: u32,
    /// Rectangle width in pixels.
    pub width: u32,
    /// Rectangle height in pixels.
    pub height: u32,
}

impl Rect {
    /// Creates a rectangle from its origin and dimensions.
    #[must_use]
    pub const fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}
