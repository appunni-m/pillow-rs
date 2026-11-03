/// Read-only RGB samples exposed by an exact-mode source strategy.
///
/// Implementations are statically dispatched and should inline `rgb_at` so
/// algorithms can read native source modes without building converted rasters.
pub(crate) trait RgbPixelSource {
    /// Number of pixels represented by this source.
    fn pixel_count(&self) -> usize;

    /// Returns the RGB color defined for one source pixel by the operation.
    fn rgb_at(&self, index: usize) -> [u8; 3];
}

impl RgbPixelSource for [u8] {
    #[inline(always)]
    fn pixel_count(&self) -> usize {
        self.len() / 3
    }

    #[inline(always)]
    fn rgb_at(&self, index: usize) -> [u8; 3] {
        let offset = index * 3;
        [self[offset], self[offset + 1], self[offset + 2]]
    }
}

impl RgbPixelSource for [[u8; 3]] {
    #[inline(always)]
    fn pixel_count(&self) -> usize {
        self.len()
    }

    #[inline(always)]
    fn rgb_at(&self, index: usize) -> [u8; 3] {
        self[index]
    }
}
