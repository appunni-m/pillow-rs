// ============================================================================
// AS PER DESIGN — DO NOT REMOVE:
//   The par_rows! macro and par_pixels! macro are the ONLY approved way to
//   parallelize pixel loops. Direct use of rayon::par_iter() on raw pixel data
//   is banned because it:
//     (a) Can produce cache-thrashing access patterns when not row-aligned
//     (b) May introduce non-deterministic floating-point accumulation order
//     (c) Makes it hard to audit which operations are parallelized
//
//   When adding a new operation, parallelize it using these macros.
//   When the operation is deterministic and safe to parallelize, use par_rows!.
//   When row-level parallelism could cause artifacts (e.g., filters with
//   boundary conditions), use par_tiles! instead.
//
//   CI enforces: all trivially-parallel operations marked in
//   scripts/check_parallelism.sh should use these macros.
// ============================================================================

/// Parallelize pixel iteration over image rows.
///
/// Each row is processed independently. The closure receives the row's starting
/// byte offset, ending byte offset, and y-coordinate.
///
/// # Safety / Determinism
/// Row-level parallelization is safe for operations where output pixels depend
/// only on input pixels from the SAME row (e.g., per-pixel color transforms,
/// point operations, channel arithmetic).
///
/// DO NOT use for operations with cross-row dependencies (e.g., vertical blur
/// passes, transposes, affine transforms with non-zero shear).
///
/// # Example
/// ```ignore
/// par_rows!(data, stride, height, |row_start, _row_end, y| {
///     for x in 0..width {
///         let idx = row_start + x * channels;
///         // process pixel at idx
///     }
/// });
/// ```
#[macro_export]
macro_rules! par_rows {
    ($data:expr, $stride:expr, $height:expr, |$row_start:ident, $row_end:ident, $y:ident| $body:block) => {{
        let _data: &[u8] = $data;
        let _stride: usize = $stride;
        let _height: usize = $height;
        // AS PER DESIGN: par_chunks splits at row boundaries — each chunk is
        // one complete scanline. This ensures cache-friendly access and
        // correctness for row-independent operations.
        use rayon::iter::ParallelIterator;
        use rayon::slice::ParallelSlice;
        _data
            .par_chunks(_stride)
            .take(_height)
            .enumerate()
            .for_each(|(_y_idx, _row)| {
                let $row_start: usize = _y_idx * _stride;
                let $row_end: usize = $row_start + _stride;
                let $y: u32 = _y_idx as u32;
                $body
            });
    }};
}

/// Parallelize pixel iteration over independent pixels.
///
/// Each pixel is processed independently. The closure receives the pixel's
/// byte offset, x-coordinate, and y-coordinate.
///
/// # Safety / Determinism
/// Pixel-level parallelization is safe for operations where each output pixel
/// depends only on the SAME pixel in the input (e.g., invert, solarize,
/// brightness/contrast per-pixel, point operations).
///
/// DO NOT use for operations with spatial dependencies (filters, resizes,
/// rotations).
///
/// # Example
/// ```ignore
/// par_pixels!(data, width, height, stride, |idx, x, y| {
///     // process pixel at data[idx..idx+channels]
/// });
/// ```
#[macro_export]
macro_rules! par_pixels {
    ($data:expr, $width:expr, $height:expr, $stride:expr,
     |$idx:ident, $x:ident, $y:ident| $body:block) => {{
        use rayon::iter::IntoParallelIterator;
        use rayon::iter::ParallelIterator;
        let _data: &[u8] = $data;
        let _width: usize = $width;
        let _height: usize = $height;
        let _stride: usize = $stride;
        (0.._height).into_par_iter().for_each(|_ry| {
            let _y = _ry as u32;
            let _row_start = _ry * _stride;
            for _rx in 0.._width {
                let $x = _rx as u32;
                let $idx = _row_start + _rx; // NOTE: + channels offset in body
                let $y = _y;
                $body
            }
        });
    }};
}

/// Parallelize over independent tiles/blocks of the image.
///
/// Useful for operations like box blur where tiles have spatial dependencies
/// at their boundaries but are otherwise independent.
///
/// Tiles are sized to fit in L1 cache (default: 64×64 pixels).
#[macro_export]
macro_rules! par_tiles {
    ($data:expr, $width:expr, $height:expr, $tile_w:expr, $tile_h:expr,
     |$tile:ident, $tx:ident, $ty:ident| $body:block) => {{
        use rayon::iter::IntoParallelRefIterator;
        use rayon::iter::ParallelIterator;
        let _width: usize = $width;
        let _height: usize = $height;
        let _tile_w: usize = $tile_w;
        let _tile_h: usize = $tile_h;
        let _tiles_x = (_width + _tile_w - 1) / _tile_w;
        let _tiles_y = (_height + _tile_h - 1) / _tile_h;
        let _tile_indices: Vec<(usize, usize)> = (0.._tiles_y)
            .flat_map(|ty| (0.._tiles_x).map(move |tx| (tx, ty)))
            .collect();
        _tile_indices.par_iter().for_each(|&($tx, $ty)| {
            let $tx = $tx;
            let $ty = $ty;
            let $tile = ();
            $body
        });
    }};
}

// AS PER DESIGN — DO NOT REMOVE: Tests validate macro behavior.
#[cfg(test)]
mod tests {
    #[test]
    fn par_rows_covers_all_pixels() {
        let width = 100u32;
        let height = 50u32;
        let stride = (width * 4) as usize;
        let data = vec![0u8; stride * height as usize];

        let visited = std::sync::atomic::AtomicU32::new(0);
        par_rows!(
            data.as_slice(),
            stride,
            height as usize,
            |row_start, row_end, y| {
                assert!(row_end > row_start);
                assert!(y < height);
                visited.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
        );
        assert_eq!(visited.load(std::sync::atomic::Ordering::Relaxed), height);
    }
}
