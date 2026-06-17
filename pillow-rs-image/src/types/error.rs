//! Minimal error types for the image type system.

use std::fmt;

/// A minimal image error type.
#[derive(Debug, Clone)]
pub enum ImageError {
    /// The operation dimensions are out of bounds or mismatched.
    Dimensions,
    /// An unsupported operation was attempted.
    Unsupported(String),
    /// A parameter error.
    Parameter(String),
    /// An I/O error.
    IoError(String),
}

impl fmt::Display for ImageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImageError::Dimensions => write!(f, "image dimensions are out of bounds"),
            ImageError::Unsupported(msg) => write!(f, "unsupported: {}", msg),
            ImageError::Parameter(msg) => write!(f, "parameter error: {}", msg),
            ImageError::IoError(msg) => write!(f, "I/O error: {}", msg),
        }
    }
}

impl std::error::Error for ImageError {}

/// A specialized Result type for image operations.
pub type ImageResult<T> = Result<T, ImageError>;

/// A rectangle representing a region of an image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    /// x coordinate of the top-left corner.
    pub x: u32,
    /// y coordinate of the top-left corner.
    pub y: u32,
    /// Width of the rectangle.
    pub width: u32,
    /// Height of the rectangle.
    pub height: u32,
}

impl Rect {
    /// Create a new rectangle.
    pub const fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Rect {
            x,
            y,
            width,
            height,
        }
    }
}
