/// The intersection of a bitmap mask with a destination image.
pub(crate) struct BitmapRegion {
    pub(crate) x_start: u32,
    pub(crate) x_end: u32,
    pub(crate) y_start: u32,
    pub(crate) y_end: u32,
}

impl BitmapRegion {
    /// Resolve the destination origin for the nonempty clipped mask region.
    ///
    /// Since `clipped` constrains both mask ranges to the image, callers can
    /// advance from this point using offsets within those ranges without
    /// another signed-coordinate check.
    #[inline]
    pub(crate) fn destination_origin(
        &self,
        x: i32,
        y: i32,
        image_width: u32,
        image_height: u32,
    ) -> Option<(u32, u32)> {
        if self.x_start >= self.x_end || self.y_start >= self.y_end {
            return None;
        }
        Self::destination_coordinates(x, y, self.x_start, self.y_start, image_width, image_height)
    }

    /// Add a mask offset without overflowing a signed image coordinate.
    #[inline]
    fn destination_coordinates(
        x: i32,
        y: i32,
        mask_x: u32,
        mask_y: u32,
        image_width: u32,
        image_height: u32,
    ) -> Option<(u32, u32)> {
        let image_x = i64::from(x) + i64::from(mask_x);
        let image_y = i64::from(y) + i64::from(mask_y);
        if image_x < 0
            || image_y < 0
            || image_x >= i64::from(image_width)
            || image_y >= i64::from(image_height)
        {
            return None;
        }
        Some((image_x as u32, image_y as u32))
    }

    /// Intersect the mask and image in mask coordinates.
    ///
    /// Promote the signed origin and unsigned extents before subtraction:
    /// every possible `i32` origin plus a `u32` mask offset fits in `i64`,
    /// including `i32::MIN` and `i32::MAX`.
    pub(crate) fn clipped(
        x: i32,
        y: i32,
        image_width: u32,
        image_height: u32,
        mask_width: u32,
        mask_height: u32,
    ) -> Self {
        Self {
            x_start: (-i64::from(x)).clamp(0, i64::from(mask_width)) as u32,
            x_end: (i64::from(image_width) - i64::from(x)).clamp(0, i64::from(mask_width)) as u32,
            y_start: (-i64::from(y)).clamp(0, i64::from(mask_height)) as u32,
            y_end: (i64::from(image_height) - i64::from(y)).clamp(0, i64::from(mask_height)) as u32,
        }
    }
}
