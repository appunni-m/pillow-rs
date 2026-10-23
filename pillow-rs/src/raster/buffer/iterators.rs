//! Borrowed pixel and row iterators for [`ImageBuffer`](super::ImageBuffer).

use std::fmt;
use std::slice::{ChunksExact, ChunksExactMut};

use crate::raster::traits::Pixel;

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
    pub(super) fn with_image(pixels: &'a [P::Subpixel], width: u32, height: u32) -> Self {
        let width = usize::try_from(width).unwrap_or(usize::MAX);
        let height = usize::try_from(height).unwrap_or(usize::MAX);
        let row_len = width.saturating_mul(usize::from(<P as Pixel>::CHANNEL_COUNT));
        if row_len == 0 {
            Rows {
                pixels: [].chunks_exact(1),
            }
        } else {
            let total_len = row_len.saturating_mul(height);
            let Some(pixels) = pixels.get(..total_len) else {
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
    pub(super) fn with_image(pixels: &'a mut [P::Subpixel], width: u32, height: u32) -> Self {
        let width = usize::try_from(width).unwrap_or(usize::MAX);
        let height = usize::try_from(height).unwrap_or(usize::MAX);
        let row_len = width.saturating_mul(usize::from(<P as Pixel>::CHANNEL_COUNT));
        if row_len == 0 {
            RowsMut {
                pixels: [].chunks_exact_mut(1),
            }
        } else {
            let total_len = row_len.saturating_mul(height);
            let Some(pixels) = pixels.get_mut(..total_len) else {
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
    pub(super) pixels: Pixels<'a, P>,
    pub(super) x: u32,
    pub(super) y: u32,
    pub(super) width: u32,
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
            self.y = self.y.saturating_add(1);
        }
        let (x, y) = (self.x, self.y);
        self.x = self.x.saturating_add(1);
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
    pub(super) pixels: PixelsMut<'a, P>,
    pub(super) x: u32,
    pub(super) y: u32,
    pub(super) width: u32,
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
            self.y = self.y.saturating_add(1);
        }
        let (x, y) = (self.x, self.y);
        self.x = self.x.saturating_add(1);
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
    pub(super) rows: Rows<'a, P>,
    pub(super) y: u32,
    pub(super) width: u32,
}

impl<'a, P: Pixel + 'a> Iterator for EnumerateRows<'a, P>
where
    P::Subpixel: 'a,
{
    type Item = (u32, EnumeratePixels<'a, P>);

    #[inline(always)]
    fn next(&mut self) -> Option<(u32, EnumeratePixels<'a, P>)> {
        let y = self.y;
        self.y = self.y.saturating_add(1);
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
    pub(super) rows: RowsMut<'a, P>,
    pub(super) y: u32,
    pub(super) width: u32,
}

impl<'a, P: Pixel + 'a> Iterator for EnumerateRowsMut<'a, P>
where
    P::Subpixel: 'a,
{
    type Item = (u32, EnumeratePixelsMut<'a, P>);

    #[inline(always)]
    fn next(&mut self) -> Option<(u32, EnumeratePixelsMut<'a, P>)> {
        let y = self.y;
        self.y = self.y.saturating_add(1);
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
