//! Portable native-layout transpose kernels.

#[cfg(feature = "parallel")]
use super::super::super::simd_rows_mut_serial;
#[cfg(feature = "parallel")]
use crate::checked_dims::CheckedDims;
#[cfg(feature = "parallel")]
use crate::pipeline::TransposeMethod;
#[cfg(feature = "parallel")]
use std::sync::atomic::{AtomicUsize, Ordering};
use wide::{f32x4, u8x16, u16x8};

#[cfg(feature = "parallel")]
macro_rules! simd_row_blocks_collect_serial {
    ($blocks:expr, $min_blocks:expr, $initialize:expr, $map:expr) => {{ simd_row_blocks_collect_serial_impl($blocks, $min_blocks, $initialize, $map) }};
}

#[cfg(feature = "parallel")]
fn simd_row_blocks_collect_serial_impl<State, Output>(
    blocks: usize,
    _min_blocks: usize,
    mut initialize: impl FnMut() -> State,
    mut map: impl FnMut(&mut State, usize) -> Output,
) -> Vec<Output> {
    let mut state = initialize();
    (0..blocks).map(|index| map(&mut state, index)).collect()
}

/// Reorder a native byte image for one Pillow transpose method.
///
/// Geometry does not need an RGBA representation: each pixel is an opaque
/// byte group whose channels must move together. Keeping the operation in its
/// original layout avoids the pack/kernel/unpack round trip that dominated
/// the SIMD transpose benchmark for RGB and RGBA images.
pub(in crate::compute::pool_simd::ops::adapters::transpose) fn transpose_gathered_row(
    source: &[u8],
    output: &mut [u8],
    source_row_bytes: usize,
    source_height: usize,
    channels: usize,
    source_x: usize,
    reverse_source_y: bool,
) -> Option<(u64, u64)> {
    let output_row_bytes = output.len();
    let vector_len = output_row_bytes / 16 * 16;
    let mut vector_blocks = 0u64;
    for output_offset in (0..vector_len).step_by(16) {
        let mut block = [0u8; 16];
        for (lane, value) in block.iter_mut().enumerate() {
            let output_byte = output_offset + lane;
            let output_pixel = output_byte / channels;
            let channel = output_byte % channels;
            let source_y = if reverse_source_y {
                source_height - 1 - output_pixel
            } else {
                output_pixel
            };
            let source_offset = source_y
                .checked_mul(source_row_bytes)?
                .checked_add(source_x.checked_mul(channels)?)?
                .checked_add(channel)?;
            *value = *source.get(source_offset)?;
        }
        output[output_offset..output_offset + 16].copy_from_slice(&u8x16::new(block).to_array());
        vector_blocks = vector_blocks.saturating_add(1);
    }
    let scalar_tail = output_row_bytes - vector_len;
    for output_offset in vector_len..output_row_bytes {
        let output_pixel = output_offset / channels;
        let channel = output_offset % channels;
        let source_y = if reverse_source_y {
            source_height - 1 - output_pixel
        } else {
            output_pixel
        };
        let source_offset = source_y
            .checked_mul(source_row_bytes)?
            .checked_add(source_x.checked_mul(channels)?)?
            .checked_add(channel)?;
        output[output_offset] = *source.get(source_offset)?;
    }
    Some((vector_blocks, scalar_tail as u64))
}

/// Transpose one complete tile with opaque pixel lanes. `wide` 1.6's byte
/// transpose and ARM integer-32 transpose are scalar, so use its accelerated
/// 16-bit transpose and float unpack network. The float vectors only shuffle
/// bits: no arithmetic touches RGBA, I/F samples, or their NaN payloads.
#[inline]
fn transpose_native_tile<const CHANNELS: usize, const TILE: usize>(
    source: &[u8],
    source_stride: usize,
    source_x: usize,
    source_y: usize,
) -> [u8x16; TILE] {
    let mut rows = [u8x16::splat(0); TILE];
    for (row_index, row) in rows.iter_mut().enumerate() {
        let start = (source_y + row_index) * source_stride + source_x * CHANNELS;
        let mut bytes = [0u8; 16];
        bytes[..TILE * CHANNELS].copy_from_slice(&source[start..start + TILE * CHANNELS]);
        *row = u8x16::new(bytes);
        if CHANNELS == 3 {
            // Expand four RGB pixels in registers; the fourth lane byte is
            // ignored again when the transposed pixels are stored.
            *row = row.swizzle_relaxed(u8x16::new([
                0, 1, 2, 0, 3, 4, 5, 0, 6, 7, 8, 0, 9, 10, 11, 0,
            ]));
        }
    }
    if CHANNELS == 1 {
        // Pair source rows as opaque 16-bit lanes, then transpose those
        // lanes. Each resulting vector is one full sixteen-pixel column.
        let low = u16x8::transpose(std::array::from_fn(|index| {
            bytemuck::cast(u8x16::unpack_low(rows[index * 2], rows[index * 2 + 1]))
        }));
        let high = u16x8::transpose(std::array::from_fn(|index| {
            bytemuck::cast(u8x16::unpack_high(rows[index * 2], rows[index * 2 + 1]))
        }));
        for (output, column) in rows.iter_mut().zip(low.into_iter().chain(high)) {
            *output = bytemuck::cast(column);
        }
    } else if CHANNELS == 2 {
        let columns = u16x8::transpose(std::array::from_fn(|index| bytemuck::cast(rows[index])));
        for (output, column) in rows.iter_mut().zip(columns) {
            *output = bytemuck::cast(column);
        }
    } else {
        let columns = f32x4::transpose(std::array::from_fn(|index| bytemuck::cast(rows[index])));
        for (output, column) in rows.iter_mut().zip(columns) {
            *output = bytemuck::cast(column);
        }
    }
    rows
}

/// Each call owns a complete group of destination rows. Tile reads and
/// writes stay contiguous within those rows, unlike the former full-column
/// gather which revisited a source cache line once per output byte.
fn transpose_native_row_group<const CHANNELS: usize, const TILE: usize>(
    source: &[u8],
    output: &mut [u8],
    width: usize,
    height: usize,
    output_y_start: usize,
    reverse_x: bool,
    reverse_y: bool,
) {
    let source_stride = width * CHANNELS;
    let output_stride = height * CHANNELS;
    let row_count = output.len() / output_stride;
    let full_rows = row_count / TILE * TILE;
    let full_columns = height / TILE * TILE;
    let store_mask = u8x16::new(std::array::from_fn(|byte| {
        if CHANNELS == 3 {
            let pixel = byte / 3;
            if pixel < TILE {
                let pixel = if reverse_y { TILE - 1 - pixel } else { pixel };
                (pixel * 4 + byte % 3) as u8
            } else {
                0
            }
        } else if reverse_y {
            ((TILE - 1 - byte / CHANNELS) * CHANNELS + byte % CHANNELS) as u8
        } else {
            byte as u8
        }
    }));

    // Keep source and destination cache lines live across adjacent tiles.
    for block_x in (0..full_columns).step_by(32) {
        for local_y in (0..full_rows).step_by(TILE) {
            let output_y = output_y_start + local_y;
            let source_x = if reverse_x {
                width - output_y - TILE
            } else {
                output_y
            };
            for output_x in (block_x..(block_x + 32).min(full_columns)).step_by(TILE) {
                let source_y = if reverse_y {
                    height - output_x - TILE
                } else {
                    output_x
                };
                let columns = transpose_native_tile::<CHANNELS, TILE>(
                    source,
                    source_stride,
                    source_x,
                    source_y,
                );
                for (column_index, column) in columns.into_iter().enumerate() {
                    let row = if reverse_x {
                        TILE - 1 - column_index
                    } else {
                        column_index
                    };
                    let start = (local_y + row) * output_stride + output_x * CHANNELS;
                    let column = if CHANNELS == 3 || reverse_y {
                        column.swizzle_relaxed(store_mask)
                    } else {
                        column
                    };
                    output[start..start + TILE * CHANNELS]
                        .copy_from_slice(&column.to_array()[..TILE * CHANNELS]);
                }
            }
        }
    }
    // Only incomplete tile edges use scalar pixel copies. Complete samples
    // remain together even for LA, packed typed words, and I;16 byte pairs.
    for (local_y, row) in output.chunks_exact_mut(output_stride).enumerate() {
        let output_y = output_y_start + local_y;
        let source_x = if reverse_x {
            width - 1 - output_y
        } else {
            output_y
        };
        let first_x = if local_y < full_rows { full_columns } else { 0 };
        for output_x in first_x..height {
            let source_y = if reverse_y {
                height - 1 - output_x
            } else {
                output_x
            };
            let source_start = source_y * source_stride + source_x * CHANNELS;
            let output_start = output_x * CHANNELS;
            row[output_start..output_start + CHANNELS]
                .copy_from_slice(&source[source_start..source_start + CHANNELS]);
        }
    }
}

pub(in crate::compute::pool_simd::ops::adapters) fn transpose_native_tiled<
    const CHANNELS: usize,
    const TILE: usize,
>(
    source: &[u8],
    output: &mut [u8],
    width: usize,
    height: usize,
    reverse_x: bool,
    reverse_y: bool,
) -> (u64, u64) {
    let rows_per_group = width.min(32);
    let output_stride = height * CHANNELS;
    let group_stride = output_stride * rows_per_group;
    let groups = width.div_ceil(rows_per_group);
    let process_group = |index: usize, rows: &mut [u8]| {
        transpose_native_row_group::<CHANNELS, TILE>(
            source,
            rows,
            width,
            height,
            index * rows_per_group,
            reverse_x,
            reverse_y,
        );
    };
    #[cfg(feature = "parallel")]
    if width * height >= 256 * 1024 {
        simd_rows_mut_serial!(
            output,
            group_stride,
            groups,
            |_row_start, _row_end, group_index, rows| {
                process_group(group_index as usize, rows);
            }
        );
    } else {
        for (index, rows) in output.chunks_mut(group_stride).enumerate() {
            process_group(index, rows);
        }
    }
    #[cfg(not(feature = "parallel"))]
    {
        let _ = groups;
        for (index, rows) in output.chunks_mut(group_stride).enumerate() {
            process_group(index, rows);
        }
    }
    let tiles = (width / TILE) * (height / TILE);
    let vector_blocks = tiles * TILE;
    let scalar_bytes = (width * height - tiles * TILE * TILE) * CHANNELS;
    (vector_blocks as u64, scalar_bytes as u64)
}

#[cfg(feature = "parallel")]
pub(in crate::compute::pool_simd::ops::adapters) fn transpose_native_collect_admitted(
    width: usize,
    height: usize,
    channels: usize,
    total_bytes: usize,
    method: &TransposeMethod,
) -> bool {
    // The extra row cache pays off only for this bounded size and aspect ratio;
    // larger outputs favor the existing direct-write kernel's memory traffic.
    matches!(channels, 3 | 4)
        && width.is_multiple_of(4)
        && height.is_multiple_of(4)
        && total_bytes > 1024 * 1024
        && total_bytes <= 8 * 1024 * 1024
        && matches!(
            method,
            TransposeMethod::Rotate90
                | TransposeMethod::Rotate270
                | TransposeMethod::Transpose
                | TransposeMethod::Transverse
        )
}

#[cfg(feature = "parallel")]
#[derive(Default)]
pub(in crate::compute::pool_simd::ops::adapters) struct TransposeNativeCollectCounters {
    pub(in crate::compute::pool_simd::ops::adapters) scratch_allocations: AtomicUsize,
    pub(in crate::compute::pool_simd::ops::adapters) cache_rebuilds: AtomicUsize,
}

#[cfg(feature = "parallel")]
impl TransposeNativeCollectCounters {
    pub(in crate::compute::pool_simd::ops::adapters) fn finish<const BLOCK_BYTES: usize>(
        &self,
        output: Vec<[u8; BLOCK_BYTES]>,
        height: usize,
        scratch_bytes: usize,
    ) -> (Vec<u8>, u64) {
        // Indexed collection initializes the final allocation directly.
        // Flattening arrays keeps it and preserves typed sample payloads.
        let output = output.into_flattened();
        crate::compute::record_pipeline_allocation(output.len());
        for _ in 0..self.scratch_allocations.load(Ordering::Relaxed) {
            crate::compute::record_pipeline_allocation(scratch_bytes);
        }
        // One four-row cache rebuild processes height four-pixel vectors.
        // Rebuilds count actual cache misses in the serial SIMD collector.
        let blocks = self.cache_rebuilds.load(Ordering::Relaxed) as u64 * height as u64;
        (output, blocks)
    }
}

#[cfg(feature = "parallel")]
pub(in crate::compute::pool_simd::ops::adapters) struct TransposeNativeRowCache<'a> {
    pub(in crate::compute::pool_simd::ops::adapters) first_block: usize,
    pub(in crate::compute::pool_simd::ops::adapters) end_block: usize,
    pub(in crate::compute::pool_simd::ops::adapters) scratch: Vec<u8>,
    pub(in crate::compute::pool_simd::ops::adapters) rebuilds: usize,
    pub(in crate::compute::pool_simd::ops::adapters) counters: &'a TransposeNativeCollectCounters,
}

#[cfg(feature = "parallel")]
impl<'a> TransposeNativeRowCache<'a> {
    pub(in crate::compute::pool_simd::ops::adapters) fn new(
        scratch_dims: &CheckedDims,
        counters: &'a TransposeNativeCollectCounters,
    ) -> Self {
        // Dimensions were checked on the caller. Record the allocation there
        // after collection because managed allocation telemetry is thread-local.
        let scratch = vec![0u8; scratch_dims.total_bytes()];
        counters.scratch_allocations.fetch_add(1, Ordering::Relaxed);
        Self {
            first_block: 0,
            end_block: 0,
            scratch,
            rebuilds: 0,
            counters,
        }
    }

    pub(in crate::compute::pool_simd::ops::adapters) fn block<
        const CHANNELS: usize,
        const BLOCK_BYTES: usize,
    >(
        &mut self,
        source: &[u8],
        width: usize,
        height: usize,
        index: usize,
        reverse_x: bool,
        reverse_y: bool,
    ) -> [u8; BLOCK_BYTES] {
        if index < self.first_block || index >= self.end_block {
            // Four output rows contain height four-pixel vectors. Each
            // collected element holds one or four vectors; the checked
            // collector preserves complete groups for either block size.
            let vectors_per_block = BLOCK_BYTES / (4 * CHANNELS);
            let blocks_per_group = height / vectors_per_block;
            let group = index / blocks_per_group;
            transpose_native_row_group::<CHANNELS, 4>(
                source,
                &mut self.scratch,
                width,
                height,
                group * 4,
                reverse_x,
                reverse_y,
            );
            self.first_block = group * blocks_per_group;
            self.end_block = self.first_block + blocks_per_group;
            self.rebuilds += 1;
        }
        // The checked four-row scratch contains exactly the cached block
        // range, so this block's end remains within that allocation.
        let start = (index - self.first_block) * BLOCK_BYTES;
        self.scratch[start..start + BLOCK_BYTES]
            .try_into()
            .expect("complete native transpose block")
    }
}

#[cfg(feature = "parallel")]
impl Drop for TransposeNativeRowCache<'_> {
    fn drop(&mut self) {
        // Accumulate once per task, keeping atomics out of the tile loop.
        self.counters
            .cache_rebuilds
            .fetch_add(self.rebuilds, Ordering::Relaxed);
    }
}

#[cfg(feature = "parallel")]
pub(in crate::compute::pool_simd::ops::adapters) fn transpose_native_collect<
    const CHANNELS: usize,
    const BLOCK_BYTES: usize,
>(
    source: &[u8],
    width: usize,
    height: usize,
    reverse_x: bool,
    reverse_y: bool,
    output_dims: &CheckedDims,
) -> Option<(Vec<u8>, u64)> {
    if !matches!(CHANNELS, 3 | 4)
        || (BLOCK_BYTES != 4 * CHANNELS && BLOCK_BYTES != 16 * CHANNELS)
        || width == 0
        || height == 0
        || !width.is_multiple_of(4)
        || !height.is_multiple_of(4)
    {
        return None;
    }
    let pixels = width.checked_mul(height)?;
    let pixels_per_block = BLOCK_BYTES / CHANNELS;
    let block_count = pixels / pixels_per_block;
    let output_bytes = block_count.checked_mul(BLOCK_BYTES)?;
    if pixels != output_dims.total_pixels()
        || output_bytes != output_dims.total_bytes()
        || output_bytes > isize::MAX as usize
    {
        return None;
    }
    let source = source.get(..output_bytes)?;
    let scratch_dims =
        CheckedDims::new(u32::try_from(height).ok()?, 4, u8::try_from(CHANNELS).ok()?).ok()?;
    // Keep the same four-row minimum task grain while collecting sixteen
    // pixels per callback instead of four. Both axes are four-aligned, so
    // each complete cache and output contain an exact number of elements.
    let min_blocks = scratch_dims.total_bytes() / BLOCK_BYTES;
    if min_blocks.checked_mul(BLOCK_BYTES)? != scratch_dims.total_bytes() {
        return None;
    }
    let counters = TransposeNativeCollectCounters::default();
    let output: Vec<[u8; BLOCK_BYTES]> = simd_row_blocks_collect_serial!(
        block_count,
        min_blocks,
        || TransposeNativeRowCache::new(&scratch_dims, &counters),
        |cache, index| cache
            .block::<CHANNELS, BLOCK_BYTES>(source, width, height, index, reverse_x, reverse_y,)
    );
    Some(counters.finish(output, height, scratch_dims.total_bytes()))
}

#[cfg(feature = "parallel")]
pub(in crate::compute::pool_simd::ops::adapters) fn transpose_native_odd_collect_admitted(
    width: usize,
    height: usize,
    channels: usize,
    total_bytes: usize,
    method: &TransposeMethod,
) -> bool {
    // Odd four-byte rasters benefit only within this bounded shape range.
    // Bound the four-row cache to 32 KiB and avoid narrow images whose
    // repeated cache fills lose the direct-write kernel's locality.
    channels == 4
        && total_bytes > 2 * 1024 * 1024
        && total_bytes <= 8 * 1024 * 1024
        && width >= 256
        && (256..=2048).contains(&height)
        && width.max(height) <= width.min(height).saturating_mul(4)
        && (!width.is_multiple_of(4) || !height.is_multiple_of(4))
        && matches!(
            method,
            TransposeMethod::Rotate90
                | TransposeMethod::Rotate270
                | TransposeMethod::Transpose
                | TransposeMethod::Transverse
        )
}

#[cfg(feature = "parallel")]
#[derive(Default)]
pub(in crate::compute::pool_simd::ops::adapters) struct TransposeNativeOddCollectCounters {
    pub(in crate::compute::pool_simd::ops::adapters) scratch_allocations: AtomicUsize,
    pub(in crate::compute::pool_simd::ops::adapters) vector_blocks: AtomicUsize,
    pub(in crate::compute::pool_simd::ops::adapters) scalar_tail: AtomicUsize,
}

#[cfg(feature = "parallel")]
impl TransposeNativeOddCollectCounters {
    pub(in crate::compute::pool_simd::ops::adapters) fn finish(
        &self,
        output: Vec<[u8; 16]>,
        logical_bytes: usize,
        scratch_bytes: usize,
    ) -> (Vec<u8>, u64, u64) {
        let mut output = output.into_flattened();
        // Account the complete padded allocation before shortening its
        // logical length. No uninitialized bytes are ever collected.
        crate::compute::record_pipeline_allocation(output.len());
        for _ in 0..self.scratch_allocations.load(Ordering::Relaxed) {
            crate::compute::record_pipeline_allocation(scratch_bytes);
        }
        output.truncate(logical_bytes);
        (
            output,
            self.vector_blocks.load(Ordering::Relaxed) as u64,
            self.scalar_tail.load(Ordering::Relaxed) as u64,
        )
    }
}

#[cfg(feature = "parallel")]
pub(in crate::compute::pool_simd::ops::adapters) struct TransposeNativeOddRowCache<'a> {
    pub(in crate::compute::pool_simd::ops::adapters) first_block: usize,
    pub(in crate::compute::pool_simd::ops::adapters) end_block: usize,
    pub(in crate::compute::pool_simd::ops::adapters) scratch: Vec<u8>,
    pub(in crate::compute::pool_simd::ops::adapters) vector_blocks: usize,
    pub(in crate::compute::pool_simd::ops::adapters) scalar_tail: usize,
    pub(in crate::compute::pool_simd::ops::adapters) counters:
        &'a TransposeNativeOddCollectCounters,
}

#[cfg(feature = "parallel")]
impl<'a> TransposeNativeOddRowCache<'a> {
    pub(in crate::compute::pool_simd::ops::adapters) fn new(
        scratch_dims: &CheckedDims,
        counters: &'a TransposeNativeOddCollectCounters,
    ) -> Self {
        let scratch = vec![0; scratch_dims.total_bytes()];
        counters.scratch_allocations.fetch_add(1, Ordering::Relaxed);
        Self {
            first_block: 0,
            end_block: 0,
            scratch,
            vector_blocks: 0,
            scalar_tail: 0,
            counters,
        }
    }

    pub(in crate::compute::pool_simd::ops::adapters) fn block(
        &mut self,
        source: &[u8],
        width: usize,
        height: usize,
        block_count: usize,
        index: usize,
        reverse_x: bool,
        reverse_y: bool,
    ) -> [u8; 16] {
        if index < self.first_block || index >= self.end_block {
            // Four complete output rows contain height four-pixel blocks,
            // even when a block straddles two odd-width output rows.
            let group = index / height;
            let first_row = group * 4;
            let rows = (width - first_row).min(4);
            transpose_native_row_group::<4, 4>(
                source,
                &mut self.scratch[..rows * height * 4],
                width,
                height,
                first_row,
                reverse_x,
                reverse_y,
            );
            let tile_pixels = (rows / 4 * 4) * (height / 4 * 4);
            self.vector_blocks += tile_pixels / 4;
            self.scalar_tail += (rows * height - tile_pixels) * 4;
            self.first_block = group * height;
            self.end_block = (self.first_block + height).min(block_count);
        }
        // The final partial group writes all its logical pixels. Only the
        // final global block may retain initialized scratch padding; finish
        // removes that padding. The full cache bounds every sixteen-byte read.
        let start = (index - self.first_block) * 16;
        self.scratch[start..start + 16]
            .try_into()
            .expect("checked four-row cache contains each complete output block")
    }
}

#[cfg(feature = "parallel")]
impl Drop for TransposeNativeOddRowCache<'_> {
    fn drop(&mut self) {
        // Count actual work and publish allocation telemetry after collection.
        self.counters
            .vector_blocks
            .fetch_add(self.vector_blocks, Ordering::Relaxed);
        self.counters
            .scalar_tail
            .fetch_add(self.scalar_tail, Ordering::Relaxed);
    }
}

#[cfg(feature = "parallel")]
pub(in crate::compute::pool_simd::ops::adapters) fn transpose_native_odd_collect(
    source: &[u8],
    width: usize,
    height: usize,
    reverse_x: bool,
    reverse_y: bool,
    output_dims: &CheckedDims,
) -> Option<(Vec<u8>, u64, u64)> {
    debug_assert_eq!(output_dims.channels, 4);
    let block_count = output_dims.total_pixels().div_ceil(4);
    let padded_bytes = block_count.checked_mul(16)?;
    if padded_bytes > isize::MAX as usize || padded_bytes < output_dims.total_bytes() {
        return None;
    }
    let scratch_dims = CheckedDims::new(u32::try_from(height).ok()?, 4, 4).ok()?;
    let counters = TransposeNativeOddCollectCounters::default();
    let output: Vec<[u8; 16]> = simd_row_blocks_collect_serial!(
        block_count,
        height,
        || TransposeNativeOddRowCache::new(&scratch_dims, &counters),
        |cache, index| cache.block(
            source,
            width,
            height,
            block_count,
            index,
            reverse_x,
            reverse_y,
        )
    );
    Some(counters.finish(
        output,
        output_dims.total_bytes(),
        scratch_dims.total_bytes(),
    ))
}
