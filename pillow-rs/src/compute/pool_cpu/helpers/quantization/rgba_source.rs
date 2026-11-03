/// Read-only RGBA samples exposed by an exact-mode source strategy.
///
/// Implementations read the native source and construct one color at a time;
/// callers do not need to build an intermediate RGBA raster.
pub(crate) trait RgbaPixelSource {
    /// Number of pixels represented by this source.
    fn pixel_count(&self) -> usize;

    /// Returns one straight-alpha RGBA color defined by the source strategy.
    fn rgba_at(&self, index: usize) -> [u8; 4];
}

impl RgbaPixelSource for [u8] {
    #[inline(always)]
    fn pixel_count(&self) -> usize {
        self.len() / 4
    }

    #[inline(always)]
    fn rgba_at(&self, index: usize) -> [u8; 4] {
        let offset = index * 4;
        [
            self[offset],
            self[offset + 1],
            self[offset + 2],
            self[offset + 3],
        ]
    }
}
