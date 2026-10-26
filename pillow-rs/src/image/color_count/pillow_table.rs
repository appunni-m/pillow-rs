//! Pillow-compatible open-addressed color-count table.

use crate::raster::{DynamicImage, GenericImageView};

use super::super::FormattedPixelValue;

/// Pillow's `ImagingGetColors` hash-table sizes and associated probing
/// polynomials, indexed by the first table size greater than `maxcolors`.
const PILLOW_COLOR_TABLE_SIZES: &[(u32, u32)] = &[
    (4, 3),
    (8, 3),
    (16, 3),
    (32, 5),
    (64, 3),
    (128, 3),
    (256, 29),
    (512, 17),
    (1024, 9),
    (2048, 5),
    (4096, 83),
    (8192, 27),
    (16384, 43),
    (32768, 3),
    (65536, 45),
    (131072, 9),
    (262144, 39),
    (524288, 39),
    (1048576, 9),
    (2097152, 5),
    (4194304, 3),
    (8388608, 33),
    (16777216, 27),
    (33554432, 9),
    (67108864, 71),
    (134217728, 39),
    (268435456, 9),
    (536870912, 5),
    (1073741824, 83),
];

pub(in crate::image) fn pillow_color_table_parameters(maxcolors: u32) -> Option<(u32, u32)> {
    PILLOW_COLOR_TABLE_SIZES
        .iter()
        .find(|(size, _)| *size > maxcolors)
        .map(|(size, polynomial)| (size - 1, *polynomial))
}

pub(in crate::image) const PILLOW_COLOR_DIRECT_TABLE_MAX_BYTES: usize = 8 * 1024 * 1024;

pub(in crate::image) fn pillow_color_direct_table_slots(
    code_mask: u32,
    pixel_count: usize,
) -> Option<usize> {
    let slots = usize::try_from(code_mask).ok()?.checked_add(1)?;
    if code_mask <= 1023 {
        return Some(slots);
    }

    let bytes = slots.checked_mul(std::mem::size_of::<u64>())?;
    (bytes <= PILLOW_COLOR_DIRECT_TABLE_MAX_BYTES && slots <= pixel_count.saturating_mul(2))
        .then_some(slots)
}

pub(in crate::image) fn pillow_image_pixel_count(image: &DynamicImage) -> usize {
    let (width, height) = image.dimensions();
    usize::try_from(width)
        .ok()
        .and_then(|width| {
            usize::try_from(height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .unwrap_or(0)
}

pub(in crate::image) enum PillowColorCounts {
    Direct(Vec<u64>),
    Sparse(std::collections::HashMap<u32, (u32, u32)>),
}

impl PillowColorCounts {
    pub(in crate::image) fn new(
        code_mask: u32,
        sparse_capacity: usize,
        pixel_count: usize,
    ) -> Self {
        // Keep large tables sparse for small images or memory-heavy limits.
        // Dense slots avoid a second randomized hash probe per input pixel
        // when a bounded table is well amortized by the scan.
        if let Some(slot_count) = pillow_color_direct_table_slots(code_mask, pixel_count) {
            if slot_count <= 1024 {
                return Self::Direct(vec![0; slot_count]);
            }

            let mut slots = Vec::new();
            if slots.try_reserve_exact(slot_count).is_ok() {
                slots.extend(std::iter::repeat_n(0, slot_count));
                return Self::Direct(slots);
            }
        }
        Self::Sparse(std::collections::HashMap::with_capacity(sparse_capacity))
    }

    pub(in crate::image) fn count(
        &mut self,
        colors: &mut usize,
        pixel: u32,
        maxcolors: usize,
        code_mask: u32,
        code_polynomial: u32,
    ) -> bool {
        match self {
            Self::Direct(slots) => count_pillow_color_direct(
                slots,
                colors,
                pixel,
                maxcolors,
                code_mask,
                code_polynomial,
            ),
            Self::Sparse(table) => count_pillow_color_sparse(
                table,
                colors,
                pixel,
                maxcolors,
                code_mask,
                code_polynomial,
            ),
        }
    }

    pub(in crate::image) fn count_repeated(
        &mut self,
        colors: &mut usize,
        pixel: u32,
        repetitions: usize,
        maxcolors: usize,
        code_mask: u32,
        code_polynomial: u32,
    ) -> Option<bool> {
        if repetitions <= 1 {
            return Some(self.count(colors, pixel, maxcolors, code_mask, code_polynomial));
        }
        let repetitions = u32::try_from(repetitions).ok()?;
        match self {
            Self::Direct(slots) => count_pillow_color_direct_repeated(
                slots,
                colors,
                pixel,
                repetitions,
                maxcolors,
                code_mask,
                code_polynomial,
            ),
            Self::Sparse(table) => count_pillow_color_sparse_repeated(
                table,
                colors,
                pixel,
                repetitions,
                maxcolors,
                code_mask,
                code_polynomial,
            ),
        }
    }

    pub(in crate::image) fn into_slot_order(
        self,
        distinct_colors: usize,
    ) -> Vec<(u32, (u32, u32))> {
        match self {
            Self::Direct(slots) => {
                let mut result = Vec::with_capacity(distinct_colors);
                result.extend(slots.into_iter().enumerate().filter_map(|(slot, packed)| {
                    (packed != 0).then_some((slot as u32, (packed as u32, (packed >> 32) as u32)))
                }));
                result
            }
            Self::Sparse(table) => {
                let mut result: Vec<_> = table.into_iter().collect();
                result.sort_unstable_by_key(|(slot, _)| *slot);
                result
            }
        }
    }
}

/// Insert one Pillow-packed pixel using the exact probe sequence used by
/// `ImagingGetColors` in a directly indexed small table.
fn count_pillow_color_direct(
    slots: &mut [u64],
    colors: &mut usize,
    pixel: u32,
    maxcolors: usize,
    code_mask: u32,
    code_polynomial: u32,
) -> bool {
    let mut slot = (!pixel) & code_mask;
    let mut increment = 0u32;
    let mut first_conflict = true;

    loop {
        let entry = &mut slots[slot as usize];
        if *entry == 0 {
            if *colors == maxcolors {
                return false;
            }
            *entry = (1u64 << 32) | u64::from(pixel);
            *colors += 1;
            return true;
        }
        if *entry as u32 == pixel {
            let count = (*entry >> 32) as u32 + 1;
            *entry = (u64::from(count) << 32) | u64::from(pixel);
            return true;
        }

        if first_conflict {
            increment = (pixel ^ (pixel >> 3)) & code_mask;
            if increment == 0 {
                increment = code_mask;
            }
            first_conflict = false;
        } else {
            increment <<= 1;
            if increment > code_mask {
                increment ^= code_polynomial;
            }
        }
        slot = (slot + increment) & code_mask;
    }
}

/// Insert one Pillow-packed pixel using the exact probe sequence used by
/// `ImagingGetColors`. Sparse storage avoids allocations proportional to
/// large `maxcolors` values.
fn count_pillow_color_sparse(
    table: &mut std::collections::HashMap<u32, (u32, u32)>,
    colors: &mut usize,
    pixel: u32,
    maxcolors: usize,
    code_mask: u32,
    code_polynomial: u32,
) -> bool {
    let mut slot = (!pixel) & code_mask;
    let mut increment = 0u32;
    let mut first_conflict = true;

    loop {
        match table.entry(slot) {
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                let (stored_pixel, count) = entry.get_mut();
                if *stored_pixel == pixel {
                    *count += 1;
                    return true;
                }
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                if *colors == maxcolors {
                    return false;
                }
                entry.insert((pixel, 1));
                *colors += 1;
                return true;
            }
        }

        if first_conflict {
            increment = (pixel ^ (pixel >> 3)) & code_mask;
            if increment == 0 {
                increment = code_mask;
            }
            first_conflict = false;
        } else {
            increment <<= 1;
            if increment > code_mask {
                increment ^= code_polynomial;
            }
        }
        slot = (slot + increment) & code_mask;
    }
}

/// Count scalar pixels after the prefix check has ruled out adjacent runs.
/// Match the storage representation once instead of redispatching through the
/// enum for every sample.
pub(in crate::image) fn count_pillow_color_scalar_samples(
    counts: &mut PillowColorCounts,
    colors: &mut usize,
    bytes: &[u8],
    maxcolors: usize,
    code_mask: u32,
    code_polynomial: u32,
) -> bool {
    match counts {
        PillowColorCounts::Direct(slots) => {
            for sample in bytes.chunks_exact(4) {
                let pixel = u32::from_ne_bytes([sample[0], sample[1], sample[2], sample[3]]);
                if !count_pillow_color_direct(
                    slots,
                    colors,
                    pixel,
                    maxcolors,
                    code_mask,
                    code_polynomial,
                ) {
                    return false;
                }
            }
        }
        PillowColorCounts::Sparse(table) => {
            for sample in bytes.chunks_exact(4) {
                let pixel = u32::from_ne_bytes([sample[0], sample[1], sample[2], sample[3]]);
                if !count_pillow_color_sparse(
                    table,
                    colors,
                    pixel,
                    maxcolors,
                    code_mask,
                    code_polynomial,
                ) {
                    return false;
                }
            }
        }
    }
    true
}

/// Count an adjacent run with the same Pillow probe sequence as scalar inserts.
/// `None` asks the caller to replay the run through `count`, preserving the
/// scalar counter-overflow behavior.
fn count_pillow_color_direct_repeated(
    slots: &mut [u64],
    colors: &mut usize,
    pixel: u32,
    repetitions: u32,
    maxcolors: usize,
    code_mask: u32,
    code_polynomial: u32,
) -> Option<bool> {
    let mut slot = (!pixel) & code_mask;
    let mut increment = 0u32;
    let mut first_conflict = true;

    loop {
        let entry = &mut slots[slot as usize];
        if *entry == 0 {
            if *colors == maxcolors {
                return Some(false);
            }
            *entry = (u64::from(repetitions) << 32) | u64::from(pixel);
            *colors += 1;
            return Some(true);
        }
        if *entry as u32 == pixel {
            let count = (*entry >> 32) as u32;
            let count = count.checked_add(repetitions)?;
            *entry = (u64::from(count) << 32) | u64::from(pixel);
            return Some(true);
        }

        if first_conflict {
            increment = (pixel ^ (pixel >> 3)) & code_mask;
            if increment == 0 {
                increment = code_mask;
            }
            first_conflict = false;
        } else {
            increment <<= 1;
            if increment > code_mask {
                increment ^= code_polynomial;
            }
        }
        slot = (slot + increment) & code_mask;
    }
}

/// Count an adjacent run in sparse Pillow slots without changing slot order.
/// `None` asks the caller to replay the run through `count` on counter overflow.
fn count_pillow_color_sparse_repeated(
    table: &mut std::collections::HashMap<u32, (u32, u32)>,
    colors: &mut usize,
    pixel: u32,
    repetitions: u32,
    maxcolors: usize,
    code_mask: u32,
    code_polynomial: u32,
) -> Option<bool> {
    let mut slot = (!pixel) & code_mask;
    let mut increment = 0u32;
    let mut first_conflict = true;

    loop {
        match table.get_mut(&slot) {
            Some((stored_pixel, count)) if *stored_pixel == pixel => {
                *count = count.checked_add(repetitions)?;
                return Some(true);
            }
            Some(_) => {}
            None => {
                if *colors == maxcolors {
                    return Some(false);
                }
                table.insert(slot, (pixel, repetitions));
                *colors += 1;
                return Some(true);
            }
        }

        if first_conflict {
            increment = (pixel ^ (pixel >> 3)) & code_mask;
            if increment == 0 {
                increment = code_mask;
            }
            first_conflict = false;
        } else {
            increment <<= 1;
            if increment > code_mask {
                increment ^= code_polynomial;
            }
        }
        slot = (slot + increment) & code_mask;
    }
}

pub(in crate::image) fn pillow_color_components(pixel: u32, bands: usize) -> FormattedPixelValue {
    let bytes = pixel.to_ne_bytes();
    match bands {
        // Pillow's four-byte LA storage places alpha in the final byte.
        2 => FormattedPixelValue::InlineComponents {
            bytes: [bytes[0], bytes[3], 0, 0],
            bands: 2,
        },
        3 => FormattedPixelValue::InlineComponents {
            bytes: [bytes[0], bytes[1], bytes[2], 0],
            bands: 3,
        },
        4 => FormattedPixelValue::InlineComponents { bytes, bands: 4 },
        _ => unreachable!("getcolors only supports one through four bands"),
    }
}

pub(in crate::image) fn pillow_color_pixel(components: &[u8]) -> u32 {
    let mut bytes = [0u8; 4];
    match components {
        [luma, alpha] => {
            bytes[0] = *luma;
            // Pillow expands L into the three color bytes of its internal
            // 32-bit LA pixel before placing alpha in the final byte.
            bytes[1] = *luma;
            bytes[2] = *luma;
            bytes[3] = *alpha;
        }
        [first, second, third] => bytes[..3].copy_from_slice(&[*first, *second, *third]),
        [first, second, third, fourth] => {
            bytes.copy_from_slice(&[*first, *second, *third, *fourth])
        }
        _ => unreachable!("getcolors only supports one through four bands"),
    }
    u32::from_ne_bytes(bytes)
}
