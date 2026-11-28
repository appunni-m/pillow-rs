use crate::error::PilError;
use crate::image::Image;
use crate::ops::paste::PasteSource;
use crate::pipeline::PipelineOp;

const PIL_MAX_IMAGE_PIXELS: u64 = 1024 * 1024 * 1024 / 4 / 3;

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
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

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
// Rust callers use this fixed safety limit. Python Image.crop applies its
// mutable Pillow warning/error policy before calling `crop_with_max_pixels`.
pub(crate) fn check_crop_extent(width: i64, height: i64) -> Result<(), PilError> {
    check_crop_extent_with_limit(width, height, Some(PIL_MAX_IMAGE_PIXELS))
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
fn check_crop_extent_with_limit(
    width: i64,
    height: i64,
    max_image_pixels: Option<u64>,
) -> Result<(), PilError> {
    // Pillow's decompression-bomb guard counts each zero axis as one pixel.
    let pixels = u128::from(width.max(1) as u64) * u128::from(height.max(1) as u64);
    let Some(max_image_pixels) = max_image_pixels else {
        return Ok(());
    };
    let limit = u128::from(max_image_pixels) * 2;
    if pixels > limit {
        return Err(PilError::DecompressionBombError(format!(
            "Image size ({pixels} pixels) exceeds limit of {limit} pixels, \
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
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    pub fn crop_unsigned(&self, box_coords: (u32, u32, u32, u32)) -> Result<Image, PilError> {
        let coordinates = (
            i64::from(box_coords.0),
            i64::from(box_coords.1),
            i64::from(box_coords.2),
            i64::from(box_coords.3),
        );
        validate_crop_order(coordinates)?;
        self.crop_signed(coordinates, Some(PIL_MAX_IMAGE_PIXELS))
    }

    /// Crops a normalized integer `(left, top, right, bottom)` box.
    ///
    /// Negative and out-of-bounds coordinates are padded with zero-valued
    /// pixels. The Python facade handles Pillow's source-value comparison and
    /// round/int order before passing this typed box; this method owns crop
    /// dimensions, padding, and pixel execution.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when dimensions, mode, or deferred source data
    /// cannot be resolved.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    // TODO(pillow-rs#3ptb): verify lazy-decode fault ordering for zero-area
    // crops and oversized dimensions against the frozen Pillow 13.0 matrix.
    pub fn crop(&self, box_coords: Option<(i32, i32, i32, i32)>) -> Result<Image, PilError> {
        self.crop_with_max_pixels(box_coords, Some(PIL_MAX_IMAGE_PIXELS))
    }

    /// Crops after a host wrapper has applied its configurable bomb policy.
    ///
    /// The Python adapter performs Pillow's arbitrary-precision extent check
    /// and warning before C-int coordinate conversion. This limit carries the
    /// same fatal threshold into native execution. `None` means the host has
    /// already checked a policy that cannot be represented as `u64`, or has
    /// disabled that policy; allocation safety remains enforced by
    /// [`crate::CheckedDims`].
    ///
    /// # Errors
    ///
    /// Returns crop ordering, extent, allocation, mode, or deferred source errors.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    // TODO(pillow-rs#3ptb): verify configurable host limits and fatal-threshold
    // parity after the host preflight, including warning-filter escalation.
    pub fn crop_with_max_pixels(
        &self,
        box_coords: Option<(i32, i32, i32, i32)>,
        max_image_pixels: Option<u64>,
    ) -> Result<Image, PilError> {
        self.crop_with_max_pixels_and_context(box_coords, max_image_pixels, None)
    }

    /// Crops with the host bomb policy and an injected parent execution route.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn crop_with_max_pixels_and_context(
        &self,
        box_coords: Option<(i32, i32, i32, i32)>,
        max_image_pixels: Option<u64>,
        context: Option<crate::compute::ExecutionContext>,
    ) -> Result<Image, PilError> {
        let routed = context.map_or_else(
            || self.clone(),
            |context| self.clone().with_execution_context(context),
        );
        routed.crop_with_max_pixels_inner(box_coords, max_image_pixels)
    }

    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    fn crop_with_max_pixels_inner(
        &self,
        box_coords: Option<(i32, i32, i32, i32)>,
        max_image_pixels: Option<u64>,
    ) -> Result<Image, PilError> {
        let Some(coordinates) = box_coords else {
            return Ok(self.materialized_branch()?.copy());
        };
        validate_crop_order(coordinates)?;

        // The Python carrier is already C-int bounded. Convert to the core's
        // signed coordinate representation so both crop policy variants use
        // the same padded-pixel implementation.
        self.crop_signed(
            (
                i64::from(coordinates.0),
                i64::from(coordinates.1),
                i64::from(coordinates.2),
                i64::from(coordinates.3),
            ),
            max_image_pixels,
        )
    }

    /// Crops using a Rust `f64` box and Python-compatible coordinate rounding.
    ///
    /// The Python facade uses [`Image::crop`] after invoking Python's own
    /// `round` and `int`, preserving host numeric protocols and conversion
    /// order. This entry is for Rust callers that already hold `f64` values.
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    // TODO(pillow-rs#3ptb): verify Rust-only f64 ordering, non-finite values,
    // and overflow fault contracts separately from the Python crop route.
    pub fn crop_float(&self, box_coords: Option<(f64, f64, f64, f64)>) -> Result<Image, PilError> {
        let Some((left, top, right, bottom)) = box_coords else {
            return self.crop(None);
        };
        validate_crop_order((left, top, right, bottom))?;
        // Pillow validates ordering first, loads the source, and only then
        // rounds coordinates inside its crop helper. Preserve that fault order
        // for deferred sources and non-finite/overflowing coordinates.
        self.materialized_shared()?;
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
            return self.crop_signed(rounded, Some(PIL_MAX_IMAGE_PIXELS));
        }
        self.crop(Some((
            rounded.0 as i32,
            rounded.1 as i32,
            rounded.2 as i32,
            rounded.3 as i32,
        )))
    }

    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    fn crop_signed(
        &self,
        (left, top, right, bottom): (i64, i64, i64, i64),
        max_image_pixels: Option<u64>,
    ) -> Result<Image, PilError> {
        let output_width = right
            .checked_sub(left)
            .ok_or_else(|| PilError::OverflowError("crop width overflow".into()))?;
        let output_height = bottom
            .checked_sub(top)
            .ok_or_else(|| PilError::OverflowError("crop height overflow".into()))?;
        let (source_width, source_height) = self.size()?;
        if left == 0
            && top == 0
            && right == i64::from(source_width)
            && bottom == i64::from(source_height)
        {
            return Ok(self.materialized_branch()?.copy());
        }

        // Pillow loads before its crop-size check, including zero-area and
        // wholly out-of-bounds boxes.
        self.materialized_shared()?;
        check_crop_extent_with_limit(output_width, output_height, max_image_pixels)?;
        let output_width = u32::try_from(output_width)
            .map_err(|_| PilError::OverflowError("crop width exceeds u32".into()))?;
        let output_height = u32::try_from(output_height)
            .map_err(|_| PilError::OverflowError("crop height exceeds u32".into()))?;
        if output_width == 0 || output_height == 0 {
            return self.crop_canvas(output_width, output_height);
        }

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

        let clip_left = left.max(0);
        let clip_top = top.max(0);
        let clip_right = right.min(i64::from(source_width));
        let clip_bottom = bottom.min(i64::from(source_height));
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
        if clip_left == left && clip_top == top && clip_right == right && clip_bottom == bottom {
            return Ok(clipped);
        }

        let mut canvas = self.crop_canvas(output_width, output_height)?;
        let paste_x = i32::try_from(clip_left - left)
            .map_err(|_| PilError::ValueError("crop offset overflow".into()))?;
        let paste_y = i32::try_from(clip_top - top)
            .map_err(|_| PilError::ValueError("crop offset overflow".into()))?;
        canvas.paste_at(
            PasteSource::Image(Box::new(clipped)),
            Some((paste_x, paste_y)),
            None,
        )?;
        // Negative/out-of-bounds crops build a fresh canvas. Reapply the
        // source pipeline's pinned backend to the composed crop-and-paste
        // graph so the geometry path retains its injected execution context.
        Ok(self.preserve_source_execution(canvas))
    }

    /// Crops a non-negative Pillow box after its coordinates have been
    /// normalized by [`Image::crop`].
    ///
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    fn crop_box(&self, left: u32, top: u32, right: u32, bottom: u32) -> Result<Image, PilError> {
        let branch = self.materialized_branch()?;
        // Materializing a pipeline branch removes its backend pin. Crop stays
        // deferred, so restore the source pin on the new operation graph.
        Ok(self.preserve_source_execution(Image::push_op(
            &branch,
            PipelineOp::Crop {
                left,
                top,
                right,
                bottom,
            },
        )))
    }

    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    fn preserve_source_execution(&self, image: Image) -> Image {
        let image = self.preserve_source_backend(image);
        match self.injected_execution_context() {
            Some(context) => image.with_execution_context(context),
            None => image,
        }
    }

    // VERIFY: source=reviewed@2026-11-20; parity=pending; benchmark=pending
    fn preserve_source_backend(&self, image: Image) -> Image {
        match self.backend() {
            Some(backend) => image.use_backend(backend),
            None => image,
        }
    }

    // VERIFY: source=reviewed@2026-11-26; parity=pending; benchmark=pending
    fn crop_canvas(&self, width: u32, height: u32) -> Result<Image, PilError> {
        let mode = self.mode_identity()?;
        if mode == crate::raster::Mode::ImagePalette8 {
            return Ok(Image::new_palette_index_from_source(self, width, height, 0));
        }
        let fill = if mode == crate::raster::Mode::ImageLab8 {
            (0, 128, 128, 0)
        } else {
            (0, 0, 0, 0)
        };
        let mut canvas = Image::new(width, height, mode, fill)?;
        let Image::Loaded(data) = &mut canvas else {
            return Err(PilError::InternalError(
                "non-palette crop canvas did not create loaded storage".into(),
            ));
        };
        data.info = self.image_info();
        data.exif = self.exif_metadata();
        if mode == crate::raster::Mode::ImagePaletteAlpha8 {
            // Pillow carries the source palette through PA crops, including
            // padded crops whose transparent pixels are filled with index 0.
            data.palette = self.palette();
            data.palette_alpha = self.palette_alpha();
        }
        Ok(canvas)
    }
}

// VERIFY: source=reviewed@2026-11-20; parity=pending; benchmark=pending
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
