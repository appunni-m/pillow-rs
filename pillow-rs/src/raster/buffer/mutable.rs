use std::ops::{Deref, DerefMut};

use crate::raster::traits::Pixel;

use super::ImageBuffer;

impl<P, Container> ImageBuffer<P, Container>
where
    P: Pixel,
    Container: Deref<Target = [P::Subpixel]> + DerefMut,
{
    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    /// Updates logical samples while leaving mode-owned physical lanes intact.
    #[track_caller]
    pub(crate) fn put_pixel_preserving_hidden_lanes(&mut self, x: u32, y: u32, pixel: P) {
        let pixel_indices = match self.pixel_indices(x, y) {
            None => panic!(
                "Image index {:?} out of bounds {:?}",
                (x, y),
                (self.width, self.height)
            ),
            Some(pixel_indices) => pixel_indices,
        };
        *<P as Pixel>::from_slice_mut(&mut self.data[pixel_indices]) = pixel;
    }
}
