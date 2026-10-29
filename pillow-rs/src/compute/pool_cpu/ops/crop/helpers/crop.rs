//! Original serial row-copy and typed-sample crop algorithms.
//!
//! The row-copy loop is preserved from the baseline CPU crop implementation.
//! Typed samples retain the existing raster crop order and arithmetic.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::raster::{ImageBuffer, Pixel};

#[derive(Debug, Clone, Copy)]
pub(crate) struct CropRegion {
    pub(crate) source_width: u32,
    pub(crate) source_height: u32,
    pub(crate) left: u32,
    pub(crate) top: u32,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

impl CropRegion {
    pub(crate) fn new(
        image_width: u32,
        image_height: u32,
        left: u32,
        top: u32,
        right: u32,
        bottom: u32,
    ) -> Result<Self, PilError> {
        debug_assert!(
            left < image_width && top < image_height
                && right <= image_width && bottom <= image_height,
            "crop pipeline coordinates must be normalized before execution"
        );
        let width = right.checked_sub(left).ok_or_else(|| {
            PilError::InternalError("crop pipeline width underflow after normalization".into())
        })?;
        let height = bottom.checked_sub(top).ok_or_else(|| {
            PilError::InternalError("crop pipeline height underflow after normalization".into())
        })?;
        Ok(Self {
            source_width: image_width,
            source_height: image_height,
            left,
            top,
            width,
            height,
        })
    }
}

pub(crate) struct ByteCrop<'a, P: Pixel<Subpixel = u8>> {
    image: &'a ImageBuffer<P, Vec<u8>>,
    region: CropRegion,
    dimensions: CheckedDims,
    shape_error: &'static str,
}

impl<'a, P: Pixel<Subpixel = u8>> ByteCrop<'a, P> {
    pub(crate) fn dimensions(region: CropRegion) -> Result<CheckedDims, PilError> {
        CheckedDims::new(region.width, region.height, P::CHANNEL_COUNT)
    }

    pub(crate) fn new(
        image: &'a ImageBuffer<P, Vec<u8>>,
        region: CropRegion,
        shape_error: &'static str,
    ) -> Result<Self, PilError> {
        let dimensions = Self::dimensions(region)?;
        Ok(Self { image, region, dimensions, shape_error })
    }

    pub(crate) fn execute(self) -> Result<ImageBuffer<P, Vec<u8>>, PilError> {
        let channels = usize::from(P::CHANNEL_COUNT);
        let source_stride = self.image.width() as usize * channels;
        let output_stride = self.dimensions.row_stride();
        let source = self.image.as_raw();
        let mut output = Vec::with_capacity(self.dimensions.total_bytes());
        crate::compute::record_pipeline_allocation(self.dimensions.total_bytes());
        for y in 0..self.region.height as usize {
            let source_start = (self.region.top as usize + y) * source_stride
                + self.region.left as usize * channels;
            output.extend_from_slice(&source[source_start..source_start + output_stride]);
        }
        ImageBuffer::from_raw(self.region.width, self.region.height, output)
            .ok_or_else(|| PilError::InternalError(self.shape_error.into()))
    }
}

pub(crate) struct SampleCrop<'a, P: Pixel> {
    image: &'a ImageBuffer<P, Vec<P::Subpixel>>,
    region: CropRegion,
}

impl<'a, P: Pixel> SampleCrop<'a, P> {
    pub(crate) fn new(image: &'a ImageBuffer<P, Vec<P::Subpixel>>, region: CropRegion) -> Self {
        Self { image, region }
    }

    pub(crate) fn execute(self) -> ImageBuffer<P, Vec<P::Subpixel>> {
        crate::raster::dynamic::crop_buffer(
            self.image,
            self.region.left,
            self.region.top,
            self.region.width,
            self.region.height,
        )
    }
}
