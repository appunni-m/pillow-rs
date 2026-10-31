//! Shared native-byte sample movement for nearest architecture kernels.

#[inline(always)]
pub(crate) fn copy_nearest<const CHANNELS: usize>(
    source: &[u8],
    destination: &mut [u8],
    source_dimensions: (u32, u32),
    coordinate: (i64, i64),
    fill: [u8; 4],
) {
    let (source_width, source_height) = source_dimensions;
    let (x, y) = coordinate;
    if x >= 0 && x < i64::from(source_width) && y >= 0 && y < i64::from(source_height) {
        let source_start = (y as usize * source_width as usize + x as usize) * CHANNELS;
        destination.copy_from_slice(&source[source_start..source_start + CHANNELS]);
    } else {
        destination.copy_from_slice(&fill[..CHANNELS]);
    }
}
