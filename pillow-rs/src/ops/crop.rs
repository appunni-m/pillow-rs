use crate::error::PilError;
use crate::image::Image;
use crate::image::PalettedData;
use crate::ops::paste::PasteSource;
use crate::pipeline::PipelineOp;

impl Image {
    /// Crops a box whose coordinates arrived from an unsigned host binding.
    ///
    /// Coordinate conversion and overflow reporting stay in the core so WASM
    /// and other FFI layers only marshal their native integer types.
    pub fn crop_unsigned(&self, box_coords: (u32, u32, u32, u32)) -> Result<Image, PilError> {
        let coordinates = (
            i32::try_from(box_coords.0)
                .map_err(|_| PilError::ValueError("crop coordinate exceeds i32".into()))?,
            i32::try_from(box_coords.1)
                .map_err(|_| PilError::ValueError("crop coordinate exceeds i32".into()))?,
            i32::try_from(box_coords.2)
                .map_err(|_| PilError::ValueError("crop coordinate exceeds i32".into()))?,
            i32::try_from(box_coords.3)
                .map_err(|_| PilError::ValueError("crop coordinate exceeds i32".into()))?,
        );
        self.crop_signed(coordinates)
    }

    /// Crops using Pillow's optional `(left, top, right, bottom)` box.
    ///
    /// Negative and out-of-bounds coordinates are padded with zero-valued
    /// pixels, and a missing box returns an independent copy. Keeping this
    /// normalization in the core lets Python and WASM bindings delegate the
    /// complete Pillow crop contract without duplicating coordinate logic.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when dimensions, mode, or deferred source data
    /// cannot be resolved.
    pub fn crop(&self, box_coords: Option<(i32, i32, i32, i32)>) -> Result<Image, PilError> {
        let Some(coordinates) = box_coords else {
            return Ok(self.copy());
        };

        // Route the common non-negative path through the same checked
        // conversion used by the unsigned WASM entry point. This keeps the
        // coordinate contract in core and makes both bindings exercise one
        // implementation instead of duplicating dispatch at their boundary.
        if coordinates.0 >= 0 && coordinates.1 >= 0 && coordinates.2 >= 0 && coordinates.3 >= 0 {
            return self.crop_unsigned((
                coordinates.0 as u32,
                coordinates.1 as u32,
                coordinates.2 as u32,
                coordinates.3 as u32,
            ));
        }

        self.crop_signed(coordinates)
    }

    fn crop_signed(
        &self,
        (left, top, right, bottom): (i32, i32, i32, i32),
    ) -> Result<Image, PilError> {
        if right < left {
            return Err(PilError::ValueError(
                "Coordinate 'right' is less than 'left'".into(),
            ));
        }
        if bottom < top {
            return Err(PilError::ValueError(
                "Coordinate 'lower' is less than 'upper'".into(),
            ));
        }

        let output_width = (right - left) as u32;
        let output_height = (bottom - top) as u32;
        if output_width == 0 || output_height == 0 {
            return self.crop_canvas(output_width, output_height);
        }

        let (source_width, source_height) = self.size()?;
        let clip_left = i64::from(left).max(0);
        let clip_top = i64::from(top).max(0);
        let clip_right = i64::from(right).min(i64::from(source_width));
        let clip_bottom = i64::from(bottom).min(i64::from(source_height));
        if clip_right <= clip_left || clip_bottom <= clip_top {
            return self.crop_canvas(output_width, output_height);
        }

        // Clipping against non-negative u32 image dimensions proves these
        // i64 coordinates are representable as u32.
        let clipped = self.crop_box(
            clip_left as u32,
            clip_top as u32,
            clip_right as u32,
            clip_bottom as u32,
        );
        if clip_left == i64::from(left)
            && clip_top == i64::from(top)
            && clip_right == i64::from(right)
            && clip_bottom == i64::from(bottom)
        {
            return Ok(clipped);
        }

        let mut canvas = self.crop_canvas(output_width, output_height)?;
        let paste_x = i32::try_from(clip_left - i64::from(left))
            .map_err(|_| PilError::ValueError("crop offset overflow".into()))?;
        let paste_y = i32::try_from(clip_top - i64::from(top))
            .map_err(|_| PilError::ValueError("crop offset overflow".into()))?;
        canvas.paste_at(PasteSource::Image(clipped), Some((paste_x, paste_y)), None)?;
        Ok(canvas)
    }

    /// Crops a non-negative Pillow box after its coordinates have been
    /// normalized by [`Image::crop`].
    ///
    fn crop_box(&self, left: u32, top: u32, right: u32, bottom: u32) -> Image {
        Image::push_op(
            self,
            PipelineOp::Crop {
                left,
                top,
                right,
                bottom,
            },
        )
    }

    fn crop_canvas(&self, width: u32, height: u32) -> Result<Image, PilError> {
        let mode = self.mode()?;
        if mode == "P" {
            return Ok(Image::Paletted(PalettedData {
                indices: crate::raster::GrayImage::from_pixel(
                    width,
                    height,
                    crate::raster::Luma([0]),
                ),
                palette: self.palette().unwrap_or_default(),
                palette_alpha: self.palette_alpha().unwrap_or_default(),
                source_format: None,
                info: None,
                exif: None,
                materialized: crate::image::materialization_cache(),
            }));
        }
        Image::new(width, height, &mode, (0, 0, 0, 0))
    }
}
