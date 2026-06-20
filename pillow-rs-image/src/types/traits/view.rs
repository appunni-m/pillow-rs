use super::pixel::Pixel;
use crate::types::buffer::ImageBuffer;
pub trait GenericImageView {
    /// The type of pixel.
    type Pixel: Pixel;

    /// The width and height of this image.
    fn dimensions(&self) -> (u32, u32);

    /// The width of this image.
    fn width(&self) -> u32 {
        let (w, _) = self.dimensions();
        w
    }

    /// The height of this image.
    fn height(&self) -> u32 {
        let (_, h) = self.dimensions();
        h
    }

    /// Returns true if this x, y coordinate is contained inside the image.
    fn in_bounds(&self, x: u32, y: u32) -> bool {
        let (width, height) = self.dimensions();
        x < width && y < height
    }

    /// Returns the pixel located at (x, y). Indexed from top left.
    ///
    /// # Panics
    ///
    /// Panics if `(x, y)` is out of bounds.
    fn get_pixel(&self, x: u32, y: u32) -> Self::Pixel;

    /// Returns the pixel located at (x, y). Indexed from top left.
    ///
    /// This function can be implemented in a way that ignores bounds checking.
    ///
    /// Returns an Iterator over the pixels of this image.
    /// The iterator yields the coordinates of each pixel
    /// along with their value
    fn pixels(&self) -> Pixels<'_, Self>
    where
        Self: Sized,
    {
        let (width, height) = self.dimensions();
        Pixels {
            image: self,
            x: 0,
            y: 0,
            width,
            height,
        }
    }

    /// Create an empty [`ImageBuffer`] with the same pixel type as this image.
    fn buffer_like(&self) -> ImageBuffer<Self::Pixel, Vec<<Self::Pixel as Pixel>::Subpixel>> {
        let (w, h) = self.dimensions();
        ImageBuffer::new(w, h)
    }

    /// Create an empty [`ImageBuffer`] with different dimensions.
    fn buffer_with_dimensions(
        &self,
        width: u32,
        height: u32,
    ) -> ImageBuffer<Self::Pixel, Vec<<Self::Pixel as Pixel>::Subpixel>> {
        ImageBuffer::new(width, height)
    }
}

/// Immutable pixel iterator over a GenericImageView.
#[derive(Debug)]
pub struct Pixels<'a, I: ?Sized + 'a> {
    image: &'a I,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

impl<I: GenericImageView> Iterator for Pixels<'_, I> {
    type Item = (u32, u32, I::Pixel);

    fn next(&mut self) -> Option<(u32, u32, I::Pixel)> {
        if self.x >= self.width {
            self.x = 0;
            self.y += 1;
        }

        if self.y >= self.height {
            None
        } else {
            let pixel = self.image.get_pixel(self.x, self.y);
            let p = (self.x, self.y, pixel);
            self.x += 1;
            Some(p)
        }
    }
}

impl<I: ?Sized> Clone for Pixels<'_, I> {
    fn clone(&self) -> Self {
        Pixels { ..*self }
    }
}

// ---------------------------------------------------------------------------
// GenericImage
// ---------------------------------------------------------------------------

/// A trait for manipulating images.
pub trait GenericImage: GenericImageView {
    /// Gets a reference to the mutable pixel at location (x, y).
    #[deprecated(since = "0.24.0", note = "Use `get_pixel` and `put_pixel` instead.")]
    fn get_pixel_mut(&mut self, x: u32, y: u32) -> &mut Self::Pixel;

    /// Put a pixel at location (x, y). Indexed from top left.
    ///
    /// # Panics
    ///
    /// Panics if `(x, y)` is out of bounds.
    fn put_pixel(&mut self, x: u32, y: u32, pixel: Self::Pixel);

    /// Puts a pixel at location (x, y), ignoring bounds checking.
    /// Put a pixel at location (x, y), taking into account alpha channels
    #[deprecated(
        since = "0.24.0",
        note = "Use iterator `pixels_mut` to blend the pixels directly"
    )]
    fn blend_pixel(&mut self, x: u32, y: u32, pixel: Self::Pixel);

    /// Copies all of the pixels from another image into this image.
    fn copy_from<O>(
        &mut self,
        other: &O,
        x: u32,
        y: u32,
    ) -> Result<(), crate::types::error::ImageError>
    where
        O: GenericImageView<Pixel = Self::Pixel>,
    {
        let (width, height) = other.dimensions();

        // Check bounds
        if x + width > self.width() || y + height > self.height() {
            return Err(crate::types::error::ImageError::Dimensions);
        }

        for k in 0..height {
            for i in 0..width {
                let p = other.get_pixel(i, k);
                self.put_pixel(i + x, k + y, p);
            }
        }

        Ok(())
    }

    /// Copies all of the pixels from one part of this image to another part of this image.
    fn copy_within(&mut self, source: crate::types::error::Rect, x: u32, y: u32) -> bool {
        let (sx, sy, width, height) = (source.x, source.y, source.width, source.height);
        let dx = x;
        let dy = y;
        if sx >= self.width() || dx >= self.width() {
            return false;
        }
        if sy >= self.height() || dy >= self.height() {
            return false;
        }
        if self.width() - dx.max(sx) < width || self.height() - dy.max(sy) < height {
            return false;
        }

        match (sx < dx, sy < dy) {
            (true, true) => {
                for y in (0..height).rev() {
                    let sy = sy + y;
                    let dy = dy + y;
                    for x in (0..width).rev() {
                        let sx = sx + x;
                        let dx = dx + x;
                        let pixel = self.get_pixel(sx, sy);
                        self.put_pixel(dx, dy, pixel);
                    }
                }
            }
            (true, false) => {
                for y in 0..height {
                    let sy = sy + y;
                    let dy = dy + y;
                    for x in (0..width).rev() {
                        let sx = sx + x;
                        let dx = dx + x;
                        let pixel = self.get_pixel(sx, sy);
                        self.put_pixel(dx, dy, pixel);
                    }
                }
            }
            (false, true) => {
                for y in (0..height).rev() {
                    let sy = sy + y;
                    let dy = dy + y;
                    for x in 0..width {
                        let sx = sx + x;
                        let dx = dx + x;
                        let pixel = self.get_pixel(sx, sy);
                        self.put_pixel(dx, dy, pixel);
                    }
                }
            }
            (false, false) => {
                for y in 0..height {
                    let sy = sy + y;
                    let dy = dy + y;
                    for x in 0..width {
                        let sx = sx + x;
                        let dx = dx + x;
                        let pixel = self.get_pixel(sx, sy);
                        self.put_pixel(dx, dy, pixel);
                    }
                }
            }
        }
        true
    }
}
