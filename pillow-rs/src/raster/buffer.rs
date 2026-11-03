//! Generic `ImageBuffer` and associated iterator types.
//!
//! This module provides the `ImageBuffer<P, Container>` generic struct and type
//! aliases like `RgbImage`, `GrayImage`, etc., matching the `image` crate.

use std::marker::PhantomData;
use std::hash::{Hash, Hasher};
use std::ops::{Deref, DerefMut, Index, IndexMut, Range};

use super::color::{FromColor, Luma, LumaA, Rgb, Rgba};

use super::traits::{GenericImage, GenericImageView, Pixel, Primitive};

#[path = "buffer/mutable.rs"]
mod mutable;

mod iterators;
pub use self::iterators::{
    EnumeratePixels, EnumeratePixelsMut, EnumerateRows, EnumerateRowsMut, Pixels, PixelsMut, Rows,
    RowsMut,
};

// ---------------------------------------------------------------------------
// ImageBuffer
// ---------------------------------------------------------------------------

/// Generic image buffer parameterised by its Pixel type and Container.
#[derive(Debug)]
pub struct ImageBuffer<P: Pixel, Container> {
    width: u32,
    height: u32,
    _phantom: PhantomData<P>,
    data: Container,
    hidden_pixel_lanes: Option<HiddenPixelLanes>,
}

// Hidden physical lanes affect Pillow-compatible byte operations, but they
// are not part of the typed buffer's logical-pixel equality or hash contract.
impl<P: Pixel, Container: PartialEq> PartialEq for ImageBuffer<P, Container> {
    fn eq(&self, other: &Self) -> bool {
        self.width == other.width
            && self.height == other.height
            && self.data == other.data
    }
}

impl<P: Pixel, Container: Eq> Eq for ImageBuffer<P, Container> {}

impl<P: Pixel, Container: Hash> Hash for ImageBuffer<P, Container> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.width.hash(state);
        self.height.hash(state);
        self.data.hash(state);
    }
}

/// Physical bytes omitted by a mode's logical typed carrier.
///
/// Mode-specific raster code supplies the lane count and physical interpretation.
/// This value stores pixels only; it does not identify an image mode.
#[derive(Debug, Hash, PartialEq, Eq, Clone)]
pub(crate) struct HiddenPixelLanes {
    lane_count: usize,
    pixel_count: usize,
    values: HiddenLaneValues,
}

#[derive(Debug, Hash, PartialEq, Eq, Clone)]
enum HiddenLaneValues {
    Constant([u8; 2]),
    PerPixel(Vec<u8>),
}

impl HiddenPixelLanes {
    pub(crate) fn constant(
        lane_count: usize,
        pixel_count: usize,
        values: [u8; 2],
    ) -> Option<Self> {
        (1..=2).contains(&lane_count).then_some(Self {
            lane_count,
            pixel_count,
            values: HiddenLaneValues::Constant(values),
        })
    }

    pub(crate) fn per_pixel(
        lane_count: usize,
        pixel_count: usize,
        values: Vec<u8>,
    ) -> Option<Self> {
        if !(1..=2).contains(&lane_count)
            || pixel_count.checked_mul(lane_count) != Some(values.len())
        {
            return None;
        }
        Some(Self {
            lane_count,
            pixel_count,
            values: HiddenLaneValues::PerPixel(values),
        })
    }

    pub(crate) const fn lane_count(&self) -> usize {
        self.lane_count
    }

    pub(crate) const fn pixel_count(&self) -> usize {
        self.pixel_count
    }

    pub(crate) fn is_constant(&self) -> bool {
        matches!(&self.values, HiddenLaneValues::Constant(_))
    }

    pub(crate) fn for_pixel(&self, index: usize) -> Option<&[u8]> {
        if index >= self.pixel_count {
            return None;
        }
        match &self.values {
            HiddenLaneValues::Constant(values) => Some(&values[..self.lane_count]),
            HiddenLaneValues::PerPixel(values) => {
                let start = index.checked_mul(self.lane_count)?;
                values.get(start..start.checked_add(self.lane_count)?)
            }
        }
    }

    pub(crate) fn set_pixel(&mut self, index: usize, values: [u8; 2]) -> Option<()> {
        if index >= self.pixel_count {
            return None;
        }
        let lane_count = self.lane_count;
        let pixel_count = self.pixel_count;
        let start = index.checked_mul(lane_count)?;
        let end = start.checked_add(lane_count)?;
        let replacement = match &mut self.values {
            HiddenLaneValues::Constant(current) => {
                if current[..lane_count] == values[..lane_count] {
                    return Some(());
                }
                let size = pixel_count.checked_mul(lane_count)?;
                let mut per_pixel = Vec::with_capacity(size);
                for _ in 0..pixel_count {
                    per_pixel.extend_from_slice(&current[..lane_count]);
                }
                per_pixel
                    .get_mut(start..end)?
                    .copy_from_slice(&values[..lane_count]);
                Some(HiddenLaneValues::PerPixel(per_pixel))
            }
            HiddenLaneValues::PerPixel(per_pixel) => {
                per_pixel
                    .get_mut(start..end)?
                    .copy_from_slice(&values[..lane_count]);
                None
            }
        };
        if let Some(replacement) = replacement {
            self.values = replacement;
        }
        Some(())
    }

    pub(crate) fn map_values<F>(&self, mut map: F) -> Self
    where
        F: FnMut(u8) -> u8,
    {
        let values = match &self.values {
            HiddenLaneValues::Constant(current) => {
                let mut mapped = *current;
                for value in mapped.iter_mut().take(self.lane_count) {
                    *value = map(*value);
                }
                HiddenLaneValues::Constant(mapped)
            }
            HiddenLaneValues::PerPixel(current) => {
                HiddenLaneValues::PerPixel(current.iter().copied().map(map).collect())
            }
        };
        Self {
            lane_count: self.lane_count,
            pixel_count: self.pixel_count,
            values,
        }
    }

    pub(crate) fn map_values_in_place<F>(&mut self, mut map: F)
    where
        F: FnMut(u8) -> u8,
    {
        match &mut self.values {
            HiddenLaneValues::Constant(values) => {
                for value in values.iter_mut().take(self.lane_count) {
                    *value = map(*value);
                }
            }
            HiddenLaneValues::PerPixel(values) => {
                for value in values {
                    *value = map(*value);
                }
            }
        }
    }

    /// Applies a byte operation to matching physical lanes while clipping to
    /// the output extent used by Pillow's two-image Chops operations.
    pub(crate) fn zip_map_clipped<F>(
        &self,
        other: &Self,
        left_dimensions: (usize, usize),
        right_dimensions: (usize, usize),
        output_dimensions: (usize, usize),
        mut map: F,
    ) -> Option<Self>
    where
        F: FnMut(u8, u8) -> u8,
    {
        if self.lane_count != other.lane_count
            || self.pixel_count != left_dimensions.0.checked_mul(left_dimensions.1)?
            || other.pixel_count != right_dimensions.0.checked_mul(right_dimensions.1)?
            || output_dimensions.0 > left_dimensions.0
            || output_dimensions.0 > right_dimensions.0
            || output_dimensions.1 > left_dimensions.1
            || output_dimensions.1 > right_dimensions.1
        {
            return None;
        }
        let pixel_count = output_dimensions.0.checked_mul(output_dimensions.1)?;
        if let (HiddenLaneValues::Constant(left), HiddenLaneValues::Constant(right)) =
            (&self.values, &other.values)
        {
            let mut values = [0; 2];
            for lane in 0..self.lane_count {
                values[lane] = map(left[lane], right[lane]);
            }
            return Self::constant(self.lane_count, pixel_count, values);
        }

        let value_count = pixel_count.checked_mul(self.lane_count)?;
        let mut values = Vec::with_capacity(value_count);
        for y in 0..output_dimensions.1 {
            let left_row = y.checked_mul(left_dimensions.0)?;
            let right_row = y.checked_mul(right_dimensions.0)?;
            for x in 0..output_dimensions.0 {
                let left = self.for_pixel(left_row.checked_add(x)?)?;
                let right = other.for_pixel(right_row.checked_add(x)?)?;
                for lane in 0..self.lane_count {
                    values.push(map(left[lane], right[lane]));
                }
            }
        }
        Self::per_pixel(self.lane_count, pixel_count, values)
    }

    pub(crate) fn remap<F>(&self, pixel_count: usize, mut source_index: F) -> Option<Self>
    where
        F: FnMut(usize) -> usize,
    {
        match &self.values {
            HiddenLaneValues::Constant(values) => {
                Self::constant(self.lane_count, pixel_count, *values)
            }
            HiddenLaneValues::PerPixel(_) => {
                let size = pixel_count.checked_mul(self.lane_count)?;
                let mut values = Vec::with_capacity(size);
                for index in 0..pixel_count {
                    values.extend_from_slice(self.for_pixel(source_index(index))?);
                }
                Self::per_pixel(self.lane_count, pixel_count, values)
            }
        }
    }
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
                hidden_pixel_lanes: None,
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

    /// Returns physical lanes omitted by this buffer's logical pixel carrier.
    pub(crate) fn hidden_pixel_lanes(&self) -> Option<&HiddenPixelLanes> {
        self.hidden_pixel_lanes.as_ref()
    }

    pub(crate) fn take_hidden_pixel_lanes(&mut self) -> Option<HiddenPixelLanes> {
        self.hidden_pixel_lanes.take()
    }

    /// Replaces hidden physical lanes after validating the image extent.
    pub(crate) fn replace_hidden_pixel_lanes(
        &mut self,
        lanes: Option<HiddenPixelLanes>,
    ) -> bool {
        let Some(pixel_count) = (self.width as usize).checked_mul(self.height as usize) else {
            return false;
        };
        if pixel_count == 0 {
            self.hidden_pixel_lanes = None;
            return lanes.map_or(true, |value| value.pixel_count() == 0);
        }
        if lanes
            .as_ref()
            .is_some_and(|value| value.pixel_count() != pixel_count)
        {
            return false;
        }
        self.hidden_pixel_lanes = lanes;
        true
    }

    /// Carries physical lanes from a source buffer through a pixel-index map.
    pub(crate) fn remap_hidden_pixel_lanes_from<F>(&mut self, source: &Self, source_index: F) -> Option<()>
    where
        F: FnMut(usize) -> usize,
    {
        let Some(lanes) = source.hidden_pixel_lanes() else {
            self.hidden_pixel_lanes = None;
            return Some(());
        };
        let pixel_count = (self.width as usize).checked_mul(self.height as usize)?;
        if pixel_count == 0 {
            self.hidden_pixel_lanes = None;
            return Some(());
        }
        self.hidden_pixel_lanes = Some(lanes.remap(pixel_count, source_index)?);
        Some(())
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
        let no_channels = usize::from(<P as Pixel>::CHANNEL_COUNT);
        let y = usize::try_from(y).unwrap_or(usize::MAX);
        let x = usize::try_from(x).unwrap_or(usize::MAX);
        let width = usize::try_from(self.width).unwrap_or(usize::MAX);
        let min_index = y
            .saturating_mul(width)
            .saturating_add(x)
            .saturating_mul(no_channels);
        let max_index = min_index.saturating_add(no_channels);
        min_index..max_index
    }
}

// Mutable pixel access
impl<P, Container> ImageBuffer<P, Container>
where
    P: Pixel,
    Container: Deref<Target = [P::Subpixel]> + DerefMut,
{
    fn inner_pixels_mut(&mut self) -> &mut [P::Subpixel] {
        self.invalidate_hidden_pixel_lanes();
        let len = Self::image_buffer_len(self.width, self.height).unwrap_or(0);
        &mut self.data[..len]
    }

    // Generic mutable escapes invalidate retained physical lanes: this helper
    // backs pixels_mut and its enumerator; rows_mut, get_pixel_mut and its
    // checked variant invalidate directly; DerefMut covers raw slices and
    // IndexMut. GenericImage::put_pixel reaches get_pixel_mut. The private
    // logical pixel writer in buffer/mutable.rs is the only writer that
    // bypasses invalidation; DynamicImage::put_pixel updates its lane sidecar.
    // TODO(pillow-rs#3ptb): keep downstream Paste/Histogram outcomes in CPU
    // parity; this mode-free buffer cannot infer Pillow's physical lanes.
    fn invalidate_hidden_pixel_lanes(&mut self) {
        self.hidden_pixel_lanes = None;
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
        self.invalidate_hidden_pixel_lanes();
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
        let pixel_indices = match self.pixel_indices(x, y) {
            None => {
                panic!(
                    "Image index {:?} out of bounds {:?}",
                    (x, y),
                    (self.width, self.height)
                )
            }
            Some(pixel_indices) => pixel_indices,
        };
        self.invalidate_hidden_pixel_lanes();
        <P as Pixel>::from_slice_mut(&mut self.data[pixel_indices])
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

        let range = i..i.checked_add(num_channels)?;
        self.data.get(range.clone())?;
        self.invalidate_hidden_pixel_lanes();
        self.data
            .get_mut(range)
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
    fn zero_hidden_pixel_lanes(width: u32, height: u32) -> Option<HiddenPixelLanes> {
        if std::mem::size_of::<P::Subpixel>() != std::mem::size_of::<u8>() {
            return None;
        }
        let lane_count = match P::CHANNEL_COUNT {
            2 => 2,
            3 => 1,
            _ => return None,
        };
        let pixel_count = (width as usize).checked_mul(height as usize)?;
        if pixel_count == 0 {
            return None;
        }
        HiddenPixelLanes::constant(lane_count, pixel_count, [0; 2])
    }

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
            hidden_pixel_lanes: Self::zero_hidden_pixel_lanes(width, height),
        }
    }

    /// Constructs a new `ImageBuffer` by copying a pixel.
    ///
    /// # Panics
    ///
    /// Panics when the resulting image is larger than the maximum size of a vector.
    pub fn from_pixel(width: u32, height: u32, pixel: P) -> ImageBuffer<P, Vec<P::Subpixel>> {
        let Some(size) = Self::image_buffer_len(width, height) else {
            panic!("Buffer length in `ImageBuffer::new` overflows usize");
        };
        let channels = usize::from(P::CHANNEL_COUNT);
        // The former pixels_mut() iterator rejected a zero-sized pixel even
        // for empty images. Keep that malformed Pixel contract rejected.
        assert!(channels != 0, "chunk size must be non-zero");
        let mut data = Vec::with_capacity(size);
        if size != 0 {
            // Seed through the same typed assignment as pixels_mut(), rather
            // than assuming channels() exposes exactly CHANNEL_COUNT values.
            // Only this first pixel needs initialization before assignment;
            // every subsequent sample is copied from initialized storage.
            data.resize(channels, P::Subpixel::DEFAULT_MIN_VALUE);
            *P::from_slice_mut(&mut data) = pixel;
            while data.len() < size {
                let additional = data.len().min(size - data.len());
                data.extend_from_within(..additional);
            }
        }
        ImageBuffer {
            data,
            width,
            height,
            _phantom: PhantomData,
            hidden_pixel_lanes: None,
        }
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
            hidden_pixel_lanes: None,
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
        self.invalidate_hidden_pixel_lanes();
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
            hidden_pixel_lanes: self.hidden_pixel_lanes.clone(),
        }
    }

    fn clone_from(&mut self, source: &Self) {
        self.data.clone_from(&source.data);
        self.width = source.width;
        self.height = source.height;
        self.hidden_pixel_lanes
            .clone_from(&source.hidden_pixel_lanes);
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
    fn put_pixel(&mut self, x: u32, y: u32, pixel: P) {
        *self.get_pixel_mut(x, y) = pixel;
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
            to.copy_from_color(from);
        }
        buffer
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::{GrayImage, ImageBuffer, Pixel};
    use crate::raster::color::{Luma, LumaA, Rgb, Rgba};

    #[test]
    fn from_pixel_fills_complete_patterns_and_partial_doubling_blocks() {
        for (width, height) in [(1, 1), (3, 5), (17, 19), (513, 515)] {
            let pixels = width as usize * height as usize;
            let luma = ImageBuffer::from_pixel(width, height, Luma([193u8]));
            let la = ImageBuffer::from_pixel(width, height, LumaA([31u8, 207]));
            let rgb = ImageBuffer::from_pixel(width, height, Rgb([17u8, 29, 233]));
            let rgba = ImageBuffer::from_pixel(width, height, Rgba([9u8, 33, 117, 231]));
            let wide = ImageBuffer::from_pixel(width, height, Rgba([0u16, 32767, 65535, 257]));
            assert_eq!(luma.dimensions(), (width, height));
            assert_eq!(luma.as_raw(), &vec![193u8; pixels]);
            assert_eq!(la.as_raw(), &[31u8, 207].repeat(pixels));
            assert_eq!(rgb.as_raw(), &[17u8, 29, 233].repeat(pixels));
            assert_eq!(rgba.as_raw(), &[9u8, 33, 117, 231].repeat(pixels));
            assert_eq!(wide.as_raw(), &[0u16, 32767, 65535, 257].repeat(pixels));
        }
    }

    #[test]
    fn from_pixel_preserves_float_payload_bits() {
        let words32 = [0x8000_0000u32, 0x7f80_0001, 0x7fc1_2345, 0xff80_0000];
        let image32 = ImageBuffer::from_pixel(17, 19, Rgba(words32.map(f32::from_bits)));
        let actual32: Vec<u32> = image32
            .as_raw()
            .iter()
            .map(|sample| sample.to_bits())
            .collect();
        assert_eq!(actual32, words32.repeat(17 * 19));

        let words64 = [0x8000_0000_0000_0000u64, 0x7ff0_0000_0000_0001];
        let image64 = ImageBuffer::from_pixel(9, 13, LumaA(words64.map(f64::from_bits)));
        let actual64: Vec<u64> = image64
            .as_raw()
            .iter()
            .map(|sample| sample.to_bits())
            .collect();
        assert_eq!(actual64, words64.repeat(9 * 13));
    }

    #[test]
    fn from_pixel_preserves_empty_dimensions() {
        for (width, height) in [(0, 0), (0, 17), (23, 0)] {
            let image = ImageBuffer::from_pixel(width, height, Rgb([11u8, 99, 213]));
            assert_eq!(image.dimensions(), (width, height));
            assert!(image.as_raw().is_empty());
        }
    }

    #[test]
    #[should_panic(expected = "Buffer length in `ImageBuffer::new` overflows usize")]
    fn from_pixel_rejects_dimension_overflow_before_allocation() {
        let _ = ImageBuffer::from_pixel(u32::MAX, u32::MAX, Rgba([1u8, 2, 3, 4]));
    }

    /// A storage-compatible pixel whose channel accessor exposes a subset.
    /// from_pixel must retain the original typed-assignment semantics rather
    /// than silently shorten its storage to match this accessor.
    #[repr(transparent)]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    struct ChannelSubset([u8; 2]);

    impl Pixel for ChannelSubset {
        type Subpixel = u8;
        const CHANNEL_COUNT: u8 = 2;
        const COLOR_MODEL: &'static str = "test";

        fn channels(&self) -> &[u8] {
            &self.0[..1]
        }
        fn channels_mut(&mut self) -> &mut [u8] {
            &mut self.0[..1]
        }
        fn from_slice(slice: &[u8]) -> &Self {
            bytemuck::cast_ref(<&[u8; 2]>::try_from(slice).expect("two stored channels"))
        }
        fn from_slice_mut(slice: &mut [u8]) -> &mut Self {
            bytemuck::cast_mut(<&mut [u8; 2]>::try_from(slice).expect("two stored channels"))
        }
        fn to_rgb(&self) -> Rgb<u8> {
            LumaA(self.0).to_rgb()
        }
        fn to_rgba(&self) -> Rgba<u8> {
            LumaA(self.0).to_rgba()
        }
        fn to_luma(&self) -> Luma<u8> {
            LumaA(self.0).to_luma()
        }
        fn to_luma_alpha(&self) -> LumaA<u8> {
            LumaA(self.0)
        }
        fn map<F: FnMut(u8) -> u8>(&self, f: F) -> Self {
            Self(LumaA(self.0).map(f).0)
        }
        fn apply<F: FnMut(u8) -> u8>(&mut self, f: F) {
            LumaA::from_slice_mut(&mut self.0).apply(f);
        }
        fn map_with_alpha<F: FnMut(u8) -> u8, G: FnMut(u8) -> u8>(&self, f: F, g: G) -> Self {
            Self(LumaA(self.0).map_with_alpha(f, g).0)
        }
        fn apply_with_alpha<F: FnMut(u8) -> u8, G: FnMut(u8) -> u8>(&mut self, f: F, g: G) {
            LumaA::from_slice_mut(&mut self.0).apply_with_alpha(f, g);
        }
        fn map2<F: FnMut(u8, u8) -> u8>(&self, other: &Self, f: F) -> Self {
            Self(LumaA(self.0).map2(&LumaA(other.0), f).0)
        }
        fn apply2<F: FnMut(u8, u8) -> u8>(&mut self, other: &Self, f: F) {
            LumaA::from_slice_mut(&mut self.0).apply2(&LumaA(other.0), f);
        }
        fn invert(&mut self) {
            LumaA::from_slice_mut(&mut self.0).invert();
        }
        fn blend(&mut self, other: &Self) {
            LumaA::from_slice_mut(&mut self.0).blend(&LumaA(other.0));
        }
    }

    #[test]
    fn from_pixel_copies_storage_instead_of_assuming_channel_accessor_length() {
        let pixel = ChannelSubset([43, 219]);
        assert_eq!(pixel.channels().len(), 1);
        let image = ImageBuffer::from_pixel(13, 7, pixel);
        assert_eq!(image.as_raw(), &[43u8, 219].repeat(13 * 7));
        assert_eq!(GrayImage::from_pixel(3, 1, Luma([0])).as_raw(), &[0, 0, 0]);
    }
}
