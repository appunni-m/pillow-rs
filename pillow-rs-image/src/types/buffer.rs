//! Generic `ImageBuffer` and associated iterator types.
//!
//! This module provides the `ImageBuffer<P, Container>` generic struct and type
//! aliases like `RgbImage`, `GrayImage`, etc., matching the `image` crate.

use std::fmt;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut, Index, IndexMut, Range};
use std::slice::{ChunksExact, ChunksExactMut};

use super::color::{FromColor, Luma, LumaA, Rgb, Rgba};

use super::traits::{GenericImage, GenericImageView, Pixel, Primitive};

// ---------------------------------------------------------------------------
// Pixels iterator
// ---------------------------------------------------------------------------

/// Iterate over pixel refs.
pub struct Pixels<'a, P: Pixel + 'a>
where
    P::Subpixel: 'a,
{
    pub(crate) chunks: ChunksExact<'a, P::Subpixel>,
}

impl<'a, P: Pixel + 'a> Iterator for Pixels<'a, P>
where
    P::Subpixel: 'a,
{
    type Item = &'a P;

    #[inline(always)]
    fn next(&mut self) -> Option<&'a P> {
        self.chunks.next().map(|v| <P as Pixel>::from_slice(v))
    }

    #[inline(always)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl<'a, P: Pixel + 'a> ExactSizeIterator for Pixels<'a, P>
where
    P::Subpixel: 'a,
{
    fn len(&self) -> usize {
        self.chunks.len()
    }
}

impl<'a, P: Pixel + 'a> DoubleEndedIterator for Pixels<'a, P>
where
    P::Subpixel: 'a,
{
    #[inline(always)]
    fn next_back(&mut self) -> Option<&'a P> {
        self.chunks.next_back().map(|v| <P as Pixel>::from_slice(v))
    }
}

impl<P: Pixel> Clone for Pixels<'_, P> {
    fn clone(&self) -> Self {
        Pixels {
            chunks: self.chunks.clone(),
        }
    }
}

impl<P: Pixel> fmt::Debug for Pixels<'_, P>
where
    P::Subpixel: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Pixels")
            .field("chunks", &self.chunks)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// PixelsMut iterator
// ---------------------------------------------------------------------------

/// Iterate over mutable pixel refs.
pub struct PixelsMut<'a, P: Pixel + 'a>
where
    P::Subpixel: 'a,
{
    pub(crate) chunks: ChunksExactMut<'a, P::Subpixel>,
}

impl<'a, P: Pixel + 'a> Iterator for PixelsMut<'a, P>
where
    P::Subpixel: 'a,
{
    type Item = &'a mut P;

    #[inline(always)]
    fn next(&mut self) -> Option<&'a mut P> {
        self.chunks.next().map(|v| <P as Pixel>::from_slice_mut(v))
    }

    #[inline(always)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl<'a, P: Pixel + 'a> ExactSizeIterator for PixelsMut<'a, P>
where
    P::Subpixel: 'a,
{
    fn len(&self) -> usize {
        self.chunks.len()
    }
}

impl<'a, P: Pixel + 'a> DoubleEndedIterator for PixelsMut<'a, P>
where
    P::Subpixel: 'a,
{
    #[inline(always)]
    fn next_back(&mut self) -> Option<&'a mut P> {
        self.chunks
            .next_back()
            .map(|v| <P as Pixel>::from_slice_mut(v))
    }
}

impl<P: Pixel> fmt::Debug for PixelsMut<'_, P>
where
    P::Subpixel: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("PixelsMut")
            .field("chunks", &self.chunks)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Rows iterator
// ---------------------------------------------------------------------------

/// Iterate over rows of an image
pub struct Rows<'a, P: Pixel + 'a>
where
    <P as Pixel>::Subpixel: 'a,
{
    pixels: ChunksExact<'a, P::Subpixel>,
}

impl<'a, P: Pixel + 'a> Rows<'a, P> {
    fn with_image(pixels: &'a [P::Subpixel], width: u32, height: u32) -> Self {
        let row_len = (width as usize) * usize::from(<P as Pixel>::CHANNEL_COUNT);
        if row_len == 0 {
            Rows {
                pixels: [].chunks_exact(1),
            }
        } else {
            let Some(pixels) = pixels.get(..row_len * height as usize) else {
                panic!("Pixel buffer has too few subpixels");
            };
            Rows {
                pixels: pixels.chunks_exact(row_len),
            }
        }
    }
}

impl<'a, P: Pixel + 'a> Iterator for Rows<'a, P>
where
    P::Subpixel: 'a,
{
    type Item = Pixels<'a, P>;

    #[inline(always)]
    fn next(&mut self) -> Option<Pixels<'a, P>> {
        let row = self.pixels.next()?;
        Some(Pixels {
            chunks: row.chunks_exact(<P as Pixel>::CHANNEL_COUNT as usize),
        })
    }

    #[inline(always)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl<'a, P: Pixel + 'a> ExactSizeIterator for Rows<'a, P>
where
    P::Subpixel: 'a,
{
    fn len(&self) -> usize {
        self.pixels.len()
    }
}

impl<'a, P: Pixel + 'a> DoubleEndedIterator for Rows<'a, P>
where
    P::Subpixel: 'a,
{
    #[inline(always)]
    fn next_back(&mut self) -> Option<Pixels<'a, P>> {
        let row = self.pixels.next_back()?;
        Some(Pixels {
            chunks: row.chunks_exact(<P as Pixel>::CHANNEL_COUNT as usize),
        })
    }
}

impl<P: Pixel> Clone for Rows<'_, P> {
    fn clone(&self) -> Self {
        Rows {
            pixels: self.pixels.clone(),
        }
    }
}

impl<P: Pixel> fmt::Debug for Rows<'_, P>
where
    P::Subpixel: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Rows")
            .field("pixels", &self.pixels)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// RowsMut iterator
// ---------------------------------------------------------------------------

/// Iterate over mutable rows of an image
pub struct RowsMut<'a, P: Pixel + 'a>
where
    <P as Pixel>::Subpixel: 'a,
{
    pixels: ChunksExactMut<'a, P::Subpixel>,
}

impl<'a, P: Pixel + 'a> RowsMut<'a, P> {
    fn with_image(pixels: &'a mut [P::Subpixel], width: u32, height: u32) -> Self {
        let row_len = (width as usize) * usize::from(<P as Pixel>::CHANNEL_COUNT);
        if row_len == 0 {
            RowsMut {
                pixels: [].chunks_exact_mut(1),
            }
        } else {
            let Some(pixels) = pixels.get_mut(..row_len * height as usize) else {
                panic!("Pixel buffer has too few subpixels");
            };
            RowsMut {
                pixels: pixels.chunks_exact_mut(row_len),
            }
        }
    }
}

impl<'a, P: Pixel + 'a> Iterator for RowsMut<'a, P>
where
    P::Subpixel: 'a,
{
    type Item = PixelsMut<'a, P>;

    #[inline(always)]
    fn next(&mut self) -> Option<PixelsMut<'a, P>> {
        let row = self.pixels.next()?;
        Some(PixelsMut {
            chunks: row.chunks_exact_mut(<P as Pixel>::CHANNEL_COUNT as usize),
        })
    }

    #[inline(always)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl<'a, P: Pixel + 'a> ExactSizeIterator for RowsMut<'a, P>
where
    P::Subpixel: 'a,
{
    fn len(&self) -> usize {
        self.pixels.len()
    }
}

impl<'a, P: Pixel + 'a> DoubleEndedIterator for RowsMut<'a, P>
where
    P::Subpixel: 'a,
{
    #[inline(always)]
    fn next_back(&mut self) -> Option<PixelsMut<'a, P>> {
        let row = self.pixels.next_back()?;
        Some(PixelsMut {
            chunks: row.chunks_exact_mut(<P as Pixel>::CHANNEL_COUNT as usize),
        })
    }
}

impl<P: Pixel> fmt::Debug for RowsMut<'_, P>
where
    P::Subpixel: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("RowsMut")
            .field("pixels", &self.pixels)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// EnumeratePixels iterator
// ---------------------------------------------------------------------------

/// Enumerate the pixels of an image.
pub struct EnumeratePixels<'a, P: Pixel + 'a>
where
    <P as Pixel>::Subpixel: 'a,
{
    pixels: Pixels<'a, P>,
    x: u32,
    y: u32,
    width: u32,
}

impl<'a, P: Pixel + 'a> Iterator for EnumeratePixels<'a, P>
where
    P::Subpixel: 'a,
{
    type Item = (u32, u32, &'a P);

    #[inline(always)]
    fn next(&mut self) -> Option<(u32, u32, &'a P)> {
        if self.x >= self.width {
            self.x = 0;
            self.y += 1;
        }
        let (x, y) = (self.x, self.y);
        self.x += 1;
        self.pixels.next().map(|p| (x, y, p))
    }

    #[inline(always)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl<'a, P: Pixel + 'a> ExactSizeIterator for EnumeratePixels<'a, P>
where
    P::Subpixel: 'a,
{
    fn len(&self) -> usize {
        self.pixels.len()
    }
}

impl<P: Pixel> Clone for EnumeratePixels<'_, P> {
    fn clone(&self) -> Self {
        EnumeratePixels {
            pixels: self.pixels.clone(),
            ..*self
        }
    }
}

impl<P: Pixel> fmt::Debug for EnumeratePixels<'_, P>
where
    P::Subpixel: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("EnumeratePixels")
            .field("pixels", &self.pixels)
            .field("x", &self.x)
            .field("y", &self.y)
            .field("width", &self.width)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// EnumeratePixelsMut iterator
// ---------------------------------------------------------------------------

/// Enumerate the pixels of an image.
pub struct EnumeratePixelsMut<'a, P: Pixel + 'a>
where
    <P as Pixel>::Subpixel: 'a,
{
    pixels: PixelsMut<'a, P>,
    x: u32,
    y: u32,
    width: u32,
}

impl<'a, P: Pixel + 'a> Iterator for EnumeratePixelsMut<'a, P>
where
    P::Subpixel: 'a,
{
    type Item = (u32, u32, &'a mut P);

    #[inline(always)]
    fn next(&mut self) -> Option<(u32, u32, &'a mut P)> {
        if self.x >= self.width {
            self.x = 0;
            self.y += 1;
        }
        let (x, y) = (self.x, self.y);
        self.x += 1;
        self.pixels.next().map(|p| (x, y, p))
    }

    #[inline(always)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl<'a, P: Pixel + 'a> ExactSizeIterator for EnumeratePixelsMut<'a, P>
where
    P::Subpixel: 'a,
{
    fn len(&self) -> usize {
        self.pixels.len()
    }
}

impl<P: Pixel> fmt::Debug for EnumeratePixelsMut<'_, P>
where
    P::Subpixel: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("EnumeratePixelsMut")
            .field("pixels", &self.pixels)
            .field("x", &self.x)
            .field("y", &self.y)
            .field("width", &self.width)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// EnumerateRows iterator
// ---------------------------------------------------------------------------

/// Enumerate the rows of an image.
pub struct EnumerateRows<'a, P: Pixel + 'a>
where
    <P as Pixel>::Subpixel: 'a,
{
    rows: Rows<'a, P>,
    y: u32,
    width: u32,
}

impl<'a, P: Pixel + 'a> Iterator for EnumerateRows<'a, P>
where
    P::Subpixel: 'a,
{
    type Item = (u32, EnumeratePixels<'a, P>);

    #[inline(always)]
    fn next(&mut self) -> Option<(u32, EnumeratePixels<'a, P>)> {
        let y = self.y;
        self.y += 1;
        self.rows.next().map(|r| {
            (
                y,
                EnumeratePixels {
                    x: 0,
                    y,
                    width: self.width,
                    pixels: r,
                },
            )
        })
    }

    #[inline(always)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl<'a, P: Pixel + 'a> ExactSizeIterator for EnumerateRows<'a, P>
where
    P::Subpixel: 'a,
{
    fn len(&self) -> usize {
        self.rows.len()
    }
}

impl<P: Pixel> Clone for EnumerateRows<'_, P> {
    fn clone(&self) -> Self {
        EnumerateRows {
            rows: self.rows.clone(),
            ..*self
        }
    }
}

impl<P: Pixel> fmt::Debug for EnumerateRows<'_, P>
where
    P::Subpixel: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("EnumerateRows")
            .field("rows", &self.rows)
            .field("y", &self.y)
            .field("width", &self.width)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// EnumerateRowsMut iterator
// ---------------------------------------------------------------------------

/// Enumerate the rows of an image.
pub struct EnumerateRowsMut<'a, P: Pixel + 'a>
where
    <P as Pixel>::Subpixel: 'a,
{
    rows: RowsMut<'a, P>,
    y: u32,
    width: u32,
}

impl<'a, P: Pixel + 'a> Iterator for EnumerateRowsMut<'a, P>
where
    P::Subpixel: 'a,
{
    type Item = (u32, EnumeratePixelsMut<'a, P>);

    #[inline(always)]
    fn next(&mut self) -> Option<(u32, EnumeratePixelsMut<'a, P>)> {
        let y = self.y;
        self.y += 1;
        self.rows.next().map(|r| {
            (
                y,
                EnumeratePixelsMut {
                    x: 0,
                    y,
                    width: self.width,
                    pixels: r,
                },
            )
        })
    }

    #[inline(always)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl<'a, P: Pixel + 'a> ExactSizeIterator for EnumerateRowsMut<'a, P>
where
    P::Subpixel: 'a,
{
    fn len(&self) -> usize {
        self.rows.len()
    }
}

impl<P: Pixel> fmt::Debug for EnumerateRowsMut<'_, P>
where
    P::Subpixel: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("EnumerateRowsMut")
            .field("rows", &self.rows)
            .field("y", &self.y)
            .field("width", &self.width)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// ImageBuffer
// ---------------------------------------------------------------------------

/// Generic image buffer parameterised by its Pixel type and Container.
#[derive(Debug, Hash, PartialEq, Eq)]
pub struct ImageBuffer<P: Pixel, Container> {
    width: u32,
    height: u32,
    _phantom: PhantomData<P>,
    data: Container,
}

// Generic implementation, shared along all image buffers
impl<P, Container> ImageBuffer<P, Container>
where
    P: Pixel,
    Container: Deref<Target = [P::Subpixel]>,
{
    /// Constructs a buffer from a generic container.
    ///
    /// Returns `None` if the container is not big enough.
    pub fn from_raw(width: u32, height: u32, buf: Container) -> Option<ImageBuffer<P, Container>> {
        if Self::check_image_fits(width, height, buf.len()) {
            Some(ImageBuffer {
                data: buf,
                width,
                height,
                _phantom: PhantomData,
            })
        } else {
            None
        }
    }

    /// Returns the underlying raw buffer
    pub fn into_raw(self) -> Container {
        self.data
    }

    /// Returns the underlying raw buffer
    pub fn as_raw(&self) -> &Container {
        &self.data
    }

    /// The width and height of this image.
    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// The width of this image.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// The height of this image.
    pub fn height(&self) -> u32 {
        self.height
    }

    fn inner_pixels(&self) -> &[P::Subpixel] {
        let len = Self::image_buffer_len(self.width, self.height).unwrap_or(0);
        &self.data[..len]
    }

    /// Returns an iterator over the pixels of this image.
    pub fn pixels(&self) -> Pixels<'_, P> {
        Pixels {
            chunks: self
                .inner_pixels()
                .chunks_exact(<P as Pixel>::CHANNEL_COUNT as usize),
        }
    }

    /// Returns an iterator over the rows of this image.
    pub fn rows(&self) -> Rows<'_, P> {
        Rows::with_image(&self.data, self.width, self.height)
    }

    /// Enumerates over the pixels of the image.
    pub fn enumerate_pixels(&self) -> EnumeratePixels<'_, P> {
        EnumeratePixels {
            pixels: self.pixels(),
            x: 0,
            y: 0,
            width: self.width,
        }
    }

    /// Enumerates over the rows of the image.
    pub fn enumerate_rows(&self) -> EnumerateRows<'_, P> {
        EnumerateRows {
            rows: self.rows(),
            y: 0,
            width: self.width,
        }
    }

    /// Gets a reference to the pixel at location `(x, y)`.
    ///
    /// # Panics
    ///
    /// Panics if `(x, y)` is out of the bounds `(width, height)`.
    #[inline]
    #[track_caller]
    pub fn get_pixel(&self, x: u32, y: u32) -> &P {
        match self.pixel_indices(x, y) {
            None => {
                panic!(
                    "Image index {:?} out of bounds {:?}",
                    (x, y),
                    (self.width, self.height)
                )
            }
            Some(pixel_indices) => <P as Pixel>::from_slice(&self.data[pixel_indices]),
        }
    }

    /// Gets a reference to the pixel at location `(x, y)` or returns `None` if
    /// the index is out of the bounds `(width, height)`.
    pub fn get_pixel_checked(&self, x: u32, y: u32) -> Option<&P> {
        if x >= self.width {
            return None;
        }
        let num_channels = <P as Pixel>::CHANNEL_COUNT as usize;
        let i = (y as usize)
            .saturating_mul(self.width as usize)
            .saturating_add(x as usize)
            .saturating_mul(num_channels);

        self.data
            .get(i..i.checked_add(num_channels)?)
            .map(|pixel_indices| <P as Pixel>::from_slice(pixel_indices))
    }

    fn check_image_fits(width: u32, height: u32, len: usize) -> bool {
        let checked_len = Self::image_buffer_len(width, height);
        checked_len.is_some_and(|min_len| min_len <= len)
    }

    fn image_buffer_len(width: u32, height: u32) -> Option<usize> {
        Some(<P as Pixel>::CHANNEL_COUNT as usize)
            .and_then(|size| size.checked_mul(width as usize))
            .and_then(|size| size.checked_mul(height as usize))
    }

    #[inline(always)]
    fn pixel_indices(&self, x: u32, y: u32) -> Option<Range<usize>> {
        if x >= self.width || y >= self.height {
            return None;
        }
        Some(self.pixel_indices_unchecked(x, y))
    }

    #[inline(always)]
    fn pixel_indices_unchecked(&self, x: u32, y: u32) -> Range<usize> {
        let no_channels = <P as Pixel>::CHANNEL_COUNT as usize;
        let min_index = (y as usize * self.width as usize + x as usize) * no_channels;
        min_index..min_index + no_channels
    }
}

// Mutable pixel access
impl<P, Container> ImageBuffer<P, Container>
where
    P: Pixel,
    Container: Deref<Target = [P::Subpixel]> + DerefMut,
{
    fn inner_pixels_mut(&mut self) -> &mut [P::Subpixel] {
        let len = Self::image_buffer_len(self.width, self.height).unwrap_or(0);
        &mut self.data[..len]
    }

    /// Returns an iterator over the mutable pixels of this image.
    pub fn pixels_mut(&mut self) -> PixelsMut<'_, P> {
        PixelsMut {
            chunks: self
                .inner_pixels_mut()
                .chunks_exact_mut(<P as Pixel>::CHANNEL_COUNT as usize),
        }
    }

    /// Returns an iterator over the mutable rows of this image.
    pub fn rows_mut(&mut self) -> RowsMut<'_, P> {
        RowsMut::with_image(&mut self.data, self.width, self.height)
    }

    /// Enumerates over the pixels of the image.
    pub fn enumerate_pixels_mut(&mut self) -> EnumeratePixelsMut<'_, P> {
        let width = self.width;
        EnumeratePixelsMut {
            pixels: self.pixels_mut(),
            x: 0,
            y: 0,
            width,
        }
    }

    /// Enumerates over the rows of the image.
    pub fn enumerate_rows_mut(&mut self) -> EnumerateRowsMut<'_, P> {
        let width = self.width;
        EnumerateRowsMut {
            rows: self.rows_mut(),
            y: 0,
            width,
        }
    }

    /// Gets a reference to the mutable pixel at location `(x, y)`.
    #[inline]
    #[track_caller]
    pub fn get_pixel_mut(&mut self, x: u32, y: u32) -> &mut P {
        match self.pixel_indices(x, y) {
            None => {
                panic!(
                    "Image index {:?} out of bounds {:?}",
                    (x, y),
                    (self.width, self.height)
                )
            }
            Some(pixel_indices) => <P as Pixel>::from_slice_mut(&mut self.data[pixel_indices]),
        }
    }

    /// Gets a reference to the mutable pixel at location `(x, y)` or returns
    /// `None` if the index is out of the bounds `(width, height)`.
    pub fn get_pixel_mut_checked(&mut self, x: u32, y: u32) -> Option<&mut P> {
        if x >= self.width {
            return None;
        }
        let num_channels = <P as Pixel>::CHANNEL_COUNT as usize;
        let i = (y as usize)
            .saturating_mul(self.width as usize)
            .saturating_add(x as usize)
            .saturating_mul(num_channels);

        self.data
            .get_mut(i..i.checked_add(num_channels)?)
            .map(|pixel_indices| <P as Pixel>::from_slice_mut(pixel_indices))
    }

    /// Puts a pixel at location `(x, y)`.
    ///
    /// # Panics
    ///
    /// Panics if `(x, y)` is out of the bounds `(width, height)`.
    #[inline]
    #[track_caller]
    pub fn put_pixel(&mut self, x: u32, y: u32, pixel: P) {
        *self.get_pixel_mut(x, y) = pixel;
    }
}

// Vec-backed buffer implementation
impl<P: Pixel> ImageBuffer<P, Vec<P::Subpixel>> {
    /// Creates a new image buffer based on a `Vec<P::Subpixel>`.
    ///
    /// All pixels have a value of zero.
    ///
    /// # Panics
    ///
    /// Panics when the resulting image is larger than the maximum size of a vector.
    #[must_use]
    pub fn new(width: u32, height: u32) -> ImageBuffer<P, Vec<P::Subpixel>> {
        let Some(size) = Self::image_buffer_len(width, height) else {
            panic!("Buffer length in `ImageBuffer::new` overflows usize");
        };
        ImageBuffer {
            data: vec![P::Subpixel::DEFAULT_MIN_VALUE; size],
            width,
            height,
            _phantom: PhantomData,
        }
    }

    /// Constructs a new `ImageBuffer` by copying a pixel.
    ///
    /// # Panics
    ///
    /// Panics when the resulting image is larger than the maximum size of a vector.
    pub fn from_pixel(width: u32, height: u32, pixel: P) -> ImageBuffer<P, Vec<P::Subpixel>> {
        let mut buf = ImageBuffer::new(width, height);
        for p in buf.pixels_mut() {
            *p = pixel;
        }
        buf
    }

    /// Constructs a new `ImageBuffer` by repeated application of the supplied function.
    ///
    /// # Panics
    ///
    /// Panics when the resulting image is larger than the maximum size of a vector.
    pub fn from_fn<F>(width: u32, height: u32, mut f: F) -> ImageBuffer<P, Vec<P::Subpixel>>
    where
        F: FnMut(u32, u32) -> P,
    {
        let mut buf = ImageBuffer::new(width, height);
        for (x, y, p) in buf.enumerate_pixels_mut() {
            *p = f(x, y);
        }
        buf
    }

    /// Creates an image buffer out of an existing buffer.
    /// Returns None if the buffer is not big enough.
    #[must_use]
    pub fn from_vec(
        width: u32,
        height: u32,
        buf: Vec<P::Subpixel>,
    ) -> Option<ImageBuffer<P, Vec<P::Subpixel>>> {
        ImageBuffer::from_raw(width, height, buf)
    }

    /// Consumes the image buffer and returns the underlying data as an owned buffer.
    #[must_use]
    pub fn into_vec(self) -> Vec<P::Subpixel> {
        self.into_raw()
    }
}

// Default
impl<P, Container> Default for ImageBuffer<P, Container>
where
    P: Pixel,
    Container: Default,
{
    fn default() -> Self {
        Self {
            width: 0,
            height: 0,
            _phantom: PhantomData,
            data: Default::default(),
        }
    }
}

// Deref
impl<P, Container> Deref for ImageBuffer<P, Container>
where
    P: Pixel,
    Container: Deref<Target = [P::Subpixel]>,
{
    type Target = [P::Subpixel];

    fn deref(&self) -> &<Self as Deref>::Target {
        &self.data
    }
}

impl<P, Container> DerefMut for ImageBuffer<P, Container>
where
    P: Pixel,
    Container: Deref<Target = [P::Subpixel]> + DerefMut,
{
    fn deref_mut(&mut self) -> &mut <Self as Deref>::Target {
        &mut self.data
    }
}

// Index<(u32, u32)>
impl<P, Container> Index<(u32, u32)> for ImageBuffer<P, Container>
where
    P: Pixel,
    Container: Deref<Target = [P::Subpixel]>,
{
    type Output = P;

    fn index(&self, (x, y): (u32, u32)) -> &P {
        self.get_pixel(x, y)
    }
}

impl<P, Container> IndexMut<(u32, u32)> for ImageBuffer<P, Container>
where
    P: Pixel,
    Container: Deref<Target = [P::Subpixel]> + DerefMut,
{
    fn index_mut(&mut self, (x, y): (u32, u32)) -> &mut P {
        self.get_pixel_mut(x, y)
    }
}

// Clone
impl<P, Container> Clone for ImageBuffer<P, Container>
where
    P: Pixel,
    Container: Deref<Target = [P::Subpixel]> + Clone,
{
    fn clone(&self) -> ImageBuffer<P, Container> {
        ImageBuffer {
            data: self.data.clone(),
            width: self.width,
            height: self.height,
            _phantom: PhantomData,
        }
    }

    fn clone_from(&mut self, source: &Self) {
        self.data.clone_from(&source.data);
        self.width = source.width;
        self.height = source.height;
    }
}

// GenericImageView
impl<P, Container> GenericImageView for ImageBuffer<P, Container>
where
    P: Pixel,
    Container: Deref<Target = [P::Subpixel]>,
{
    type Pixel = P;

    fn dimensions(&self) -> (u32, u32) {
        self.dimensions()
    }

    fn get_pixel(&self, x: u32, y: u32) -> P {
        *self.get_pixel(x, y)
    }
}

// GenericImage
impl<P, Container> GenericImage for ImageBuffer<P, Container>
where
    P: Pixel,
    Container: Deref<Target = [P::Subpixel]> + DerefMut,
{
    fn get_pixel_mut(&mut self, x: u32, y: u32) -> &mut P {
        self.get_pixel_mut(x, y)
    }

    fn put_pixel(&mut self, x: u32, y: u32, pixel: P) {
        *self.get_pixel_mut(x, y) = pixel;
    }

    fn blend_pixel(&mut self, x: u32, y: u32, p: P) {
        self.get_pixel_mut(x, y).blend(&p);
    }
}

// ---------------------------------------------------------------------------
// Type aliases
// ---------------------------------------------------------------------------

/// Sendable RGB image buffer (8-bit per channel).
pub type RgbImage = ImageBuffer<Rgb<u8>, Vec<u8>>;

/// Sendable RGBA image buffer (8-bit per channel).
pub type RgbaImage = ImageBuffer<Rgba<u8>, Vec<u8>>;

/// Sendable grayscale image buffer (8-bit).
pub type GrayImage = ImageBuffer<Luma<u8>, Vec<u8>>;

/// Sendable grayscale + alpha channel image buffer (8-bit).
pub type GrayAlphaImage = ImageBuffer<LumaA<u8>, Vec<u8>>;

/// Sendable 32-bit float RGB image buffer.
pub type Rgb32FImage = ImageBuffer<Rgb<f32>, Vec<f32>>;

/// Sendable 32-bit float RGBA image buffer.
pub type Rgba32FImage = ImageBuffer<Rgba<f32>, Vec<f32>>;

/// Sendable 16-bit grayscale image buffer.
#[allow(dead_code)]
pub(crate) type Gray16Image = ImageBuffer<Luma<u16>, Vec<u16>>;

/// Sendable 16-bit grayscale + alpha channel image buffer.
#[allow(dead_code)]
pub(crate) type GrayAlpha16Image = ImageBuffer<LumaA<u16>, Vec<u16>>;

/// Sendable 16-bit RGB image buffer.
#[allow(dead_code)]
pub(crate) type Rgb16Image = ImageBuffer<Rgb<u16>, Vec<u16>>;

/// Sendable 16-bit RGBA image buffer.
#[allow(dead_code)]
pub(crate) type Rgba16Image = ImageBuffer<Rgba<u16>, Vec<u16>>;

// ---------------------------------------------------------------------------
// ConvertBuffer trait
// ---------------------------------------------------------------------------

/// Provides color conversions for whole image buffers.
pub trait ConvertBuffer<T> {
    /// Converts `self` to a buffer of type T.
    fn convert(&self) -> T;
}

impl<Container, FromType: Pixel, ToType: Pixel>
    ConvertBuffer<ImageBuffer<ToType, Vec<ToType::Subpixel>>> for ImageBuffer<FromType, Container>
where
    Container: Deref<Target = [FromType::Subpixel]>,
    ToType: FromColor<FromType>,
{
    fn convert(&self) -> ImageBuffer<ToType, Vec<ToType::Subpixel>> {
        let mut buffer: ImageBuffer<ToType, Vec<ToType::Subpixel>> =
            ImageBuffer::new(self.width, self.height);
        for (to, from) in buffer.pixels_mut().zip(self.pixels()) {
            to.from_color(from);
        }
        buffer
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
