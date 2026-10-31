/// The intersection of a bitmap mask with a destination image.
pub(crate) struct BitmapRegion {
    pub(crate) x_start: u32,
    pub(crate) x_end: u32,
    pub(crate) y_start: u32,
    pub(crate) y_end: u32,
}

impl BitmapRegion {
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
            x_end: (i64::from(image_width) - i64::from(x))
                .clamp(0, i64::from(mask_width)) as u32,
            y_start: (-i64::from(y)).clamp(0, i64::from(mask_height)) as u32,
            y_end: (i64::from(image_height) - i64::from(y))
                .clamp(0, i64::from(mask_height)) as u32,
        }
    }
}
