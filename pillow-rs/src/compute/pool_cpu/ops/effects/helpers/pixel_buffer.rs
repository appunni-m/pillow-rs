//! Compile-time pixel identity for a typed native image buffer.

use crate::raster::{ImageBuffer, Pixel};

/// Connects a mode-owned `ImageBuffer` carrier to its inner pixel type.
pub(crate) trait RasterBufferPixel {
    /// Exact logical pixel stored by the buffer.
    type Pixel: Pixel;
}

impl<P: Pixel> RasterBufferPixel for ImageBuffer<P, Vec<P::Subpixel>> {
    type Pixel = P;
}

/// Inner pixel type for one concrete raster carrier.
pub(crate) type PixelOf<Buffer> = <Buffer as RasterBufferPixel>::Pixel;
