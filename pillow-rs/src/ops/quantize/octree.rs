//! Pillow-compatible FASTOCTREE quantization.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;

// ── Octree quantization (FASTOCTREE) — PIL's algorithm for RGB/RGBA images ──
// ═══════════════════════════════════════════════════════════════════════════════
//
// PIL's Quantize.FASTOCTREE (method=2) uses a multi-dimensional color cube
// approach. Two cubes are created at different bit depths:
//   - Fine cube:   higher resolution  (e.g., 4×4×4×0 = 4096 buckets for RGB)
//   - Coarse cube: lower resolution   (e.g., 2×2×2×0 = 64 buckets for RGB)
//
// RGB uses a 3-dimensional cube. With alpha, a 4th dimension is added:
//   - Fine:   3×4×3×3 = 8192 buckets
//   - Coarse: 2×2×2×2 = 256 buckets
//
// Algorithm:
//   1. Populate fine cube from pixel data
//   2. Derive coarse cube from fine cube (resolution reduction)
//   3. Count used coarse buckets, limit to n_colors
//   4. Sort fine buckets by popularity, take remaining slots
//   5. Subtract fine colors from coarse to avoid double-counting
//   6. Build lookup cube (coarse resolution first, then fine overlay)
//   7. Map each pixel to its palette index via lookup cube

/// Bit depths for fine and coarse cubes — RGBA mode (with alpha).
const CUBE_LEVELS_RGB: [u32; 8] = [4, 4, 4, 0, 2, 2, 2, 0];
const CUBE_LEVELS_RGBA: [u32; 8] = [3, 4, 3, 3, 2, 2, 2, 2];

// Behavioral port of Pillow 12.2.0 src/libImaging/QuantOctree.c. In
// particular, the cube is laid out R-major and copy_octree_cube handles both
// reduction and expansion. Palette order for equally populated buckets is
// observable, so the sorter reproduces the Apple Libc qsort used by the pinned
// macOS Pillow oracle rather than adding an artificial color tie-breaker.

#[derive(Clone, Default)]
struct OctreeBucket {
    count: u32,
    sums: [u64; 4],
}

impl OctreeBucket {
    fn add_color(&mut self, color: [u8; 4]) {
        self.count = self.count.saturating_add(1);
        for (sum, channel) in self.sums.iter_mut().zip(color) {
            *sum = sum.saturating_add(u64::from(channel));
        }
    }

    fn add_bucket(&mut self, other: &Self) {
        self.count = self.count.saturating_add(other.count);
        for (sum, other_sum) in self.sums.iter_mut().zip(other.sums) {
            *sum = sum.saturating_add(other_sum);
        }
    }

    fn average(&self) -> [u8; 4] {
        if self.count == 0 {
            return [0; 4];
        }
        // Pillow 12.2.0 QuantOctree.c:avg_color_from_color_bucket converts
        // both operands to C `float`, truncates the quotient to `int`, then
        // applies CLIP8. The f32 rounding before division is observable for
        // large buckets and must not be replaced with integer division.
        let count = self.count as f32;
        std::array::from_fn(|channel| {
            let average = (self.sums[channel] as f32 / count) as i32;
            average.clamp(0, 255) as u8
        })
    }
}

struct OctreeCube {
    bits: [u32; 4],
    widths: [usize; 4],
    offsets: [u32; 4],
    buckets: Vec<OctreeBucket>,
}

impl OctreeCube {
    fn new(bits: [u32; 4]) -> Self {
        debug_assert!(bits.iter().all(|&value| value < usize::BITS));
        debug_assert!(bits.iter().copied().sum::<u32>() < usize::BITS);
        let widths = bits.map(|value| 1usize << value);
        let offsets = [
            bits[1].saturating_add(bits[2]).saturating_add(bits[3]),
            bits[2].saturating_add(bits[3]),
            bits[3],
            0,
        ];
        let size = widths.into_iter().product();
        Self {
            bits,
            widths,
            offsets,
            buckets: vec![OctreeBucket::default(); size],
        }
    }

    fn offset_position(&self, values: [usize; 4]) -> usize {
        values
            .into_iter()
            .zip(self.offsets)
            .fold(0usize, |offset, (value, shift)| offset | (value << shift))
    }

    fn offset(&self, color: [u8; 4]) -> usize {
        let values = std::array::from_fn(|channel| {
            (usize::from(color[channel]) >> 8u32.saturating_sub(self.bits[channel]))
                & self.widths[channel].saturating_sub(1)
        });
        self.offset_position(values)
    }

    #[inline]
    fn offset_rgb(&self, color: [u8; 3]) -> usize {
        let values = [
            (usize::from(color[0]) >> 8u32.saturating_sub(self.bits[0]))
                & self.widths[0].saturating_sub(1),
            (usize::from(color[1]) >> 8u32.saturating_sub(self.bits[1]))
                & self.widths[1].saturating_sub(1),
            (usize::from(color[2]) >> 8u32.saturating_sub(self.bits[2]))
                & self.widths[2].saturating_sub(1),
            0,
        ];
        self.offset_position(values)
    }

    fn add_color(&mut self, color: [u8; 4]) {
        let offset = self.offset(color);
        self.buckets[offset].add_color(color);
    }

    #[inline]
    fn add_rgb_color(&mut self, color: [u8; 3]) {
        let offset = self.offset_rgb(color);
        let bucket = &mut self.buckets[offset];
        bucket.count = bucket.count.saturating_add(1);
        bucket.sums[0] = bucket.sums[0].saturating_add(u64::from(color[0]));
        bucket.sums[1] = bucket.sums[1].saturating_add(u64::from(color[1]));
        bucket.sums[2] = bucket.sums[2].saturating_add(u64::from(color[2]));
    }

    fn used(&self) -> usize {
        self.buckets
            .iter()
            .filter(|bucket| bucket.count > 0)
            .count()
    }
}

fn copy_octree_cube(cube: &OctreeCube, bits: [u32; 4]) -> OctreeCube {
    let mut result = OctreeCube::new(bits);
    let mut source_reduce = [0u32; 4];
    let mut destination_reduce = [0u32; 4];
    let widths: [usize; 4] = std::array::from_fn(|channel| {
        if cube.bits[channel] > bits[channel] {
            destination_reduce[channel] = cube.bits[channel].saturating_sub(bits[channel]);
            cube.widths[channel]
        } else {
            source_reduce[channel] = bits[channel].saturating_sub(cube.bits[channel]);
            result.widths[channel]
        }
    });
    for r in 0..widths[0] {
        for g in 0..widths[1] {
            for b in 0..widths[2] {
                for a in 0..widths[3] {
                    let values = [r, g, b, a];
                    let source = cube.offset_position(std::array::from_fn(|channel| {
                        values[channel] >> source_reduce[channel]
                    }));
                    let destination = result.offset_position(std::array::from_fn(|channel| {
                        values[channel] >> destination_reduce[channel]
                    }));
                    result.buckets[destination].add_bucket(&cube.buckets[source]);
                }
            }
        }
    }
    result
}

fn octree_bucket_order(left: &OctreeBucket, right: &OctreeBucket) -> std::cmp::Ordering {
    right.count.cmp(&left.count)
}

fn median_octree_bucket(values: &[OctreeBucket], a: usize, b: usize, c: usize) -> usize {
    if octree_bucket_order(&values[a], &values[b]).is_lt() {
        if octree_bucket_order(&values[b], &values[c]).is_lt() {
            b
        } else if octree_bucket_order(&values[a], &values[c]).is_lt() {
            c
        } else {
            a
        }
    } else if octree_bucket_order(&values[b], &values[c]).is_gt() {
        b
    } else if octree_bucket_order(&values[a], &values[c]).is_lt() {
        a
    } else {
        c
    }
}

fn insertion_sort_octree_buckets(values: &mut [OctreeBucket], swap_limit: Option<usize>) -> bool {
    let mut swaps = 0usize;
    for right in 1..values.len() {
        let mut cursor = right;
        while cursor > 0
            && octree_bucket_order(&values[cursor.saturating_sub(1)], &values[cursor]).is_gt()
        {
            values.swap(cursor, cursor.saturating_sub(1));
            swaps = swaps.saturating_add(1);
            if swap_limit.is_some_and(|limit| swaps > limit) {
                return false;
            }
            cursor = cursor.saturating_sub(1);
        }
    }
    true
}

fn swap_octree_bucket_ranges(
    values: &mut [OctreeBucket],
    left: usize,
    right: usize,
    length: usize,
) {
    for offset in 0..length {
        values.swap(left.saturating_add(offset), right.saturating_add(offset));
    }
}

struct OctreePartition {
    end: usize,
    left_length: usize,
    right_length: usize,
    swapped: bool,
}

fn apple_qsort_octree_buckets(values: &mut [OctreeBucket]) {
    let mut start = 0usize;
    let mut length = values.len();
    loop {
        let end = start.saturating_add(length);
        if length <= 7 {
            insertion_sort_octree_buckets(&mut values[start..end], None);
            return;
        }

        let partition = partition_octree_buckets(values, start, length);
        if !partition.swapped {
            let limit = 1usize.saturating_add(length.div_euclid(4));
            if insertion_sort_octree_buckets(&mut values[start..partition.end], Some(limit)) {
                return;
            }
        }

        if partition.left_length <= partition.right_length {
            if partition.left_length > 1 {
                apple_qsort_octree_buckets(
                    &mut values[start..start.saturating_add(partition.left_length)],
                );
            }
            if partition.right_length <= 1 {
                return;
            }
            start = partition.end.saturating_sub(partition.right_length);
            length = partition.right_length;
        } else {
            if partition.right_length > 1 {
                apple_qsort_octree_buckets(
                    &mut values
                        [partition.end.saturating_sub(partition.right_length)..partition.end],
                );
            }
            if partition.left_length <= 1 {
                return;
            }
            length = partition.left_length;
        }
    }
}

fn partition_octree_buckets(
    values: &mut [OctreeBucket],
    start: usize,
    length: usize,
) -> OctreePartition {
    let mut low = start;
    let mut middle = start.saturating_add(length.div_euclid(2));
    let mut high = start.saturating_add(length).saturating_sub(1);
    if length > 40 {
        let distance = length.div_euclid(8);
        low = median_octree_bucket(
            values,
            low,
            low.saturating_add(distance),
            low.saturating_add(distance.saturating_mul(2)),
        );
        middle = median_octree_bucket(
            values,
            middle.saturating_sub(distance),
            middle,
            middle.saturating_add(distance),
        );
        high = median_octree_bucket(
            values,
            high.saturating_sub(distance.saturating_mul(2)),
            high.saturating_sub(distance),
            high,
        );
    }
    middle = median_octree_bucket(values, low, middle, high);
    values.swap(start, middle);

    let mut equal_left = start.saturating_add(1);
    let mut scan_left = start.saturating_add(1);
    let mut scan_right = start.saturating_add(length).saturating_sub(1);
    let mut equal_right = scan_right;
    let mut swapped = false;
    scan_partition(
        values,
        start,
        &mut scan_left,
        &mut scan_right,
        &mut equal_left,
        &mut equal_right,
        &mut swapped,
    );

    let end = start.saturating_add(length);
    let left_equal = equal_left
        .saturating_sub(start)
        .min(scan_left.saturating_sub(equal_left));
    swap_octree_bucket_ranges(
        values,
        start,
        scan_left.saturating_sub(left_equal),
        left_equal,
    );
    let right_equal = equal_right
        .saturating_sub(scan_right)
        .min(end.saturating_sub(equal_right).saturating_sub(1));
    swap_octree_bucket_ranges(
        values,
        scan_left,
        end.saturating_sub(right_equal),
        right_equal,
    );

    OctreePartition {
        end,
        left_length: scan_left.saturating_sub(equal_left),
        right_length: equal_right.saturating_sub(scan_right),
        swapped,
    }
}

fn scan_partition(
    values: &mut [OctreeBucket],
    pivot: usize,
    scan_left: &mut usize,
    scan_right: &mut usize,
    equal_left: &mut usize,
    equal_right: &mut usize,
    swapped: &mut bool,
) {
    loop {
        scan_equal_left(values, pivot, scan_left, scan_right, equal_left, swapped);
        scan_equal_right(values, pivot, scan_left, scan_right, equal_right, swapped);
        if *scan_left > *scan_right {
            return;
        }
        values.swap(*scan_left, *scan_right);
        *swapped = true;
        *scan_left = scan_left.saturating_add(1);
        *scan_right = scan_right.saturating_sub(1);
    }
}

fn scan_equal_left(
    values: &mut [OctreeBucket],
    pivot: usize,
    scan_left: &mut usize,
    scan_right: &usize,
    equal_left: &mut usize,
    swapped: &mut bool,
) {
    while *scan_left <= *scan_right {
        let ordering = octree_bucket_order(&values[*scan_left], &values[pivot]);
        if ordering.is_gt() {
            break;
        }
        if ordering.is_eq() {
            values.swap(*equal_left, *scan_left);
            *equal_left = equal_left.saturating_add(1);
            *swapped = true;
        }
        *scan_left = scan_left.saturating_add(1);
    }
}

fn scan_equal_right(
    values: &mut [OctreeBucket],
    pivot: usize,
    scan_left: &usize,
    scan_right: &mut usize,
    equal_right: &mut usize,
    swapped: &mut bool,
) {
    while *scan_left <= *scan_right {
        let ordering = octree_bucket_order(&values[*scan_right], &values[pivot]);
        if ordering.is_lt() {
            break;
        }
        if ordering.is_eq() {
            values.swap(*scan_right, *equal_right);
            *equal_right = equal_right.saturating_sub(1);
            *swapped = true;
        }
        *scan_right = scan_right.saturating_sub(1);
    }
}

fn sorted_octree_buckets(cube: &OctreeCube) -> Vec<OctreeBucket> {
    let mut buckets = cube.buckets.clone();
    apple_qsort_octree_buckets(&mut buckets);
    buckets
}

fn subtract_octree_buckets(cube: &mut OctreeCube, buckets: &[OctreeBucket]) {
    for bucket in buckets.iter().filter(|bucket| bucket.count > 0) {
        let offset = cube.offset(bucket.average());
        let destination = &mut cube.buckets[offset];
        destination.count = destination.count.saturating_sub(bucket.count);
        for (sum, value) in destination.sums.iter_mut().zip(bucket.sums) {
            *sum = sum.saturating_sub(value);
        }
    }
}

fn add_octree_lookup(cube: &mut OctreeCube, palette: &[OctreeBucket], offset: usize) {
    for index in (offset..palette.len()).rev() {
        let bucket = &palette[index];
        let position = cube.offset(bucket.average());
        cube.buckets[position].count = index as u32;
    }
}

/// Main octree quantization function, matching Pillow's Quantize.FASTOCTREE.
///
/// `pixels` is flat RGBA byte data (4 bytes per pixel).
/// Returns (palette_indices, palette_bytes_rgba).
pub(crate) fn quantize_octree_rgba(
    pixels: &[u8],
    w: u32,
    h: u32,
    n_colors: usize,
    with_alpha: bool,
) -> Result<(Vec<u8>, Vec<u8>), PilError> {
    if w == 0 || h == 0 {
        // Pillow returns an empty P image for an empty FASTOCTREE input; there
        // are no palette entries to reduce or slice.
        return Ok((Vec::new(), Vec::new()));
    }
    let dimensions = CheckedDims::new(w, h, 4)?;
    if pixels.len() != dimensions.total_bytes() {
        return Err(PilError::InternalError(
            "FASTOCTREE RGBA input buffer shape mismatch".to_owned(),
        ));
    }
    quantize_octree_pixels(dimensions.total_pixels(), n_colors, with_alpha, |index| {
        let offset = index * 4;
        [
            pixels[offset],
            pixels[offset + 1],
            pixels[offset + 2],
            pixels[offset + 3],
        ]
    })
}

/// Quantize tightly packed RGB samples without widening them to RGBA.
pub(crate) fn quantize_octree_rgb(
    pixels: &[u8],
    w: u32,
    h: u32,
    n_colors: usize,
) -> Result<(Vec<u8>, Vec<u8>), PilError> {
    if w == 0 || h == 0 {
        return Ok((Vec::new(), Vec::new()));
    }
    let dimensions = CheckedDims::new(w, h, 3)?;
    if pixels.len() != dimensions.total_bytes() {
        return Err(PilError::InternalError(
            "FASTOCTREE RGB input buffer shape mismatch".to_owned(),
        ));
    }
    quantize_octree_pixels(dimensions.total_pixels(), n_colors, false, |index| {
        let offset = index * 3;
        [pixels[offset], pixels[offset + 1], pixels[offset + 2], 255]
    })
}

/// Shared octree reduction over a native-format pixel accessor.
///
/// The accessor constructs one stack color at a time, so RGB callers do not
/// allocate a four-byte-per-pixel staging image or a second color array.
fn quantize_octree_pixels(
    pixel_count: usize,
    n_colors: usize,
    with_alpha: bool,
    color_at: impl Fn(usize) -> [u8; 4],
) -> Result<(Vec<u8>, Vec<u8>), PilError> {
    let cube_levels = if with_alpha {
        CUBE_LEVELS_RGBA
    } else {
        CUBE_LEVELS_RGB
    };
    let fine_bits = [
        cube_levels[0],
        cube_levels[1],
        cube_levels[2],
        cube_levels[3],
    ];
    let coarse_bits = [
        cube_levels[4],
        cube_levels[5],
        cube_levels[6],
        cube_levels[7],
    ];
    // Quant.c normalizes every fully transparent pixel to the first one's RGB
    // before FASTOCTREE. Invisible garbage channels must not consume colors or
    // cause distinct indices for pixels that Pillow treats as equivalent.
    let first_transparent = if with_alpha {
        (0..pixel_count).map(&color_at).find(|color| color[3] == 0)
    } else {
        None
    };
    let normalize_transparent = |mut color: [u8; 4]| {
        if color[3] == 0
            && let Some(first) = first_transparent
        {
            color[..3].copy_from_slice(&first[..3]);
        }
        color
    };
    let mut fine = OctreeCube::new(fine_bits);
    if with_alpha {
        for index in 0..pixel_count {
            fine.add_color(normalize_transparent(color_at(index)));
        }
    } else {
        for index in 0..pixel_count {
            let color = color_at(index);
            fine.add_rgb_color([color[0], color[1], color[2]]);
        }
    }
    let mut coarse = copy_octree_cube(&fine, coarse_bits);
    let mut coarse_count = coarse.used().min(n_colors);
    let mut fine_count = n_colors.saturating_sub(coarse_count);
    let fine_palette = sorted_octree_buckets(&fine);
    subtract_octree_buckets(&mut coarse, &fine_palette[..fine_count]);
    while coarse_count > coarse.used() {
        let already_subtracted = fine_count;
        coarse_count = coarse.used();
        fine_count = n_colors.saturating_sub(coarse_count);
        subtract_octree_buckets(&mut coarse, &fine_palette[already_subtracted..fine_count]);
    }
    let coarse_palette = sorted_octree_buckets(&coarse);
    let mut palette = coarse_palette[..coarse_count].to_vec();
    palette.extend_from_slice(&fine_palette[..fine_count]);

    let mut coarse_lookup = OctreeCube::new(coarse_bits);
    add_octree_lookup(&mut coarse_lookup, &palette[..coarse_count], 0);
    let mut lookup = copy_octree_cube(&coarse_lookup, fine_bits);
    add_octree_lookup(&mut lookup, &palette, coarse_count);

    let indices = if with_alpha {
        (0..pixel_count)
            .map(|index| {
                let color = normalize_transparent(color_at(index));
                lookup.buckets[lookup.offset(color)].count as u8
            })
            .collect()
    } else {
        (0..pixel_count)
            .map(|index| {
                let color = color_at(index);
                let rgb = [color[0], color[1], color[2]];
                lookup.buckets[lookup.offset_rgb(rgb)].count as u8
            })
            .collect()
    };
    let palette_bytes = palette
        .iter()
        .flat_map(OctreeBucket::average)
        .collect::<Vec<_>>();
    Ok((indices, palette_bytes))
}

// ═══════════════════════════════════════════════════════════════════════════════
// ── Image.quantize method ──
// ═══════════════════════════════════════════════════════════════════════════════
