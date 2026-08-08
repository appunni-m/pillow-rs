use crate::error::PilError;
use crate::image::Image;
use crate::image::PalettedData;
use crate::ops::paste::PasteSource;
use crate::pipeline::PipelineOp;

const PIL_MAX_IMAGE_PIXELS: u64 = 1024 * 1024 * 1024 / 4 / 3;
const PIL_DECOMPRESSION_BOMB_LIMIT: u128 = (PIL_MAX_IMAGE_PIXELS as u128) * 2;

fn validate_crop_order<T: PartialOrd>(
    (left, top, right, bottom): (T, T, T, T),
) -> Result<(), PilError> {
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
    Ok(())
}

pub(crate) fn check_crop_extent(width: i64, height: i64) -> Result<(), PilError> {
    let pixels = u128::from(width.max(1) as u64) * u128::from(height.max(1) as u64);
    if pixels > PIL_DECOMPRESSION_BOMB_LIMIT {
        return Err(PilError::DecompressionBombError(format!(
            "Image size ({pixels} pixels) exceeds limit of {PIL_DECOMPRESSION_BOMB_LIMIT} pixels, \
             could be decompression bomb DOS attack."
        )));
    }
    Ok(())
}

impl Image {
    /// Crops a box whose coordinates arrived from an unsigned host binding.
    ///
    /// Coordinate conversion and overflow reporting stay in the core so WASM
    /// and other FFI layers only marshal their native integer types.
    pub fn crop_unsigned(&self, box_coords: (u32, u32, u32, u32)) -> Result<Image, PilError> {
        let coordinates = (
            i64::from(box_coords.0),
            i64::from(box_coords.1),
            i64::from(box_coords.2),
            i64::from(box_coords.3),
        );
        validate_crop_order(coordinates)?;
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
        validate_crop_order(coordinates)?;

        // Route the common non-negative path through the same checked
        // conversion used by the unsigned WASM entry point. This keeps the
        // coordinate contract in core and makes both bindings exercise one
        // implementation instead of duplicating dispatch at their boundary.
        if [coordinates.0, coordinates.1, coordinates.2, coordinates.3]
            .iter()
            .all(|coordinate| *coordinate >= 0)
        {
            return self.crop_unsigned((
                coordinates.0 as u32,
                coordinates.1 as u32,
                coordinates.2 as u32,
                coordinates.3 as u32,
            ));
        }

        self.crop_signed((
            i64::from(coordinates.0),
            i64::from(coordinates.1),
            i64::from(coordinates.2),
            i64::from(coordinates.3),
        ))
    }

    /// Crops using Pillow's floating-point box contract.
    ///
    /// Pillow rounds each coordinate with Python's ties-to-even `round` before
    /// entering the integer crop primitive. Keep that conversion in Rust so
    /// the Python binding only marshals host numbers and delegates semantics.
    pub fn crop_float(&self, box_coords: Option<(f64, f64, f64, f64)>) -> Result<Image, PilError> {
        let Some((left, top, right, bottom)) = box_coords else {
            return self.crop(None);
        };
        validate_crop_order((left, top, right, bottom))?;
        let rounded = (
            pillow_round(left)?,
            pillow_round(top)?,
            pillow_round(right)?,
            pillow_round(bottom)?,
        );
        let rounded_values = [rounded.0, rounded.1, rounded.2, rounded.3];
        if rounded_values
            .iter()
            .any(|coordinate| *coordinate < i64::from(i32::MIN))
            || rounded_values
                .iter()
                .any(|coordinate| *coordinate > i64::from(i32::MAX))
        {
            return self.crop_signed(rounded);
        }
        self.crop(Some((
            rounded.0 as i32,
            rounded.1 as i32,
            rounded.2 as i32,
            rounded.3 as i32,
        )))
    }

    fn crop_signed(
        &self,
        (left, top, right, bottom): (i64, i64, i64, i64),
    ) -> Result<Image, PilError> {
        let output_width = right
            .checked_sub(left)
            .ok_or_else(|| PilError::OverflowError("crop width overflow".into()))?;
        let output_height = bottom
            .checked_sub(top)
            .ok_or_else(|| PilError::OverflowError("crop height overflow".into()))?;
        check_crop_extent(output_width, output_height)?;
        for coordinate in [left, top, right, bottom] {
            if coordinate > i64::from(i32::MAX) {
                return Err(PilError::OverflowError(
                    "signed integer is greater than maximum".into(),
                ));
            }
            if coordinate < i64::from(i32::MIN) {
                return Err(PilError::OverflowError(
                    "signed integer is less than minimum".into(),
                ));
            }
        }
        let output_width = u32::try_from(output_width)
            .map_err(|_| PilError::OverflowError("crop width exceeds u32".into()))?;
        let output_height = u32::try_from(output_height)
            .map_err(|_| PilError::OverflowError("crop height exceeds u32".into()))?;
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
        )?;
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
    fn crop_box(&self, left: u32, top: u32, right: u32, bottom: u32) -> Result<Image, PilError> {
        let branch = self.materialized_branch()?;
        Ok(Image::push_op(
            &branch,
            PipelineOp::Crop {
                left,
                top,
                right,
                bottom,
            },
        ))
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

fn pillow_round(value: f64) -> Result<i64, PilError> {
    if value.is_nan() {
        return Err(PilError::ValueError(
            "cannot convert float NaN to integer".into(),
        ));
    }
    if value.is_infinite() {
        return Err(PilError::OverflowError(
            "cannot convert float infinity to integer".into(),
        ));
    }
    let max_i64 = 2_f64.powi(63);
    if value < i64::MIN as f64 || value >= max_i64 {
        return Err(PilError::OverflowError(
            "Python int too large to convert to C long".into(),
        ));
    }

    let lower = value.floor();
    let fraction = value - lower;
    let rounded = if fraction < 0.5 {
        lower
    } else if fraction > 0.5 {
        lower + 1.0
    } else if (lower as i64) % 2 == 0 {
        lower
    } else {
        lower + 1.0
    };
    Ok(rounded as i64)
}
