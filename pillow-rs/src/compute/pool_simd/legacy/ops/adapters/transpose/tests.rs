//! Focused contract tests for native Transpose and aligned collection paths.

use super::TransposeMethod;
use super::native_transpose_bytes;
#[cfg(feature = "parallel")]
use super::{
    TransposeNativeCollectCounters, TransposeNativeRowCache, transpose_native_collect_admitted,
};

fn assert_native_transpose_matches_forward_mapping(
    source: &[u8],
    width: usize,
    height: usize,
    channels: usize,
) {
    for method in [
        TransposeMethod::FlipLeftRight,
        TransposeMethod::FlipTopBottom,
        TransposeMethod::Rotate90,
        TransposeMethod::Rotate180,
        TransposeMethod::Rotate270,
        TransposeMethod::Transpose,
        TransposeMethod::Transverse,
    ] {
        assert_native_transpose_method(source, width, height, channels, method);
    }
}

fn assert_native_transpose_method(
    source: &[u8],
    width: usize,
    height: usize,
    channels: usize,
    method: TransposeMethod,
) {
    let swapped = transpose_swaps_dimensions(&method);
    let (expected_width, expected_height) = if swapped {
        (height, width)
    } else {
        (width, height)
    };
    let expected = expected_transpose_bytes(source, width, height, channels, &method);
    let (actual, actual_width, actual_height, blocks, tail, kernel_path) = native_transpose_bytes(
        source,
        width as u32,
        height as u32,
        channels,
        method.clone(),
    )
    .expect("valid native transpose");
    assert_eq!(
        (actual_width, actual_height),
        (expected_width as u32, expected_height as u32)
    );
    assert_eq!(
        actual, expected,
        "{method:?}: {width}x{height}, {channels} channels"
    );
    #[cfg(all(target_arch = "x86_64", not(feature = "parallel")))]
    if channels == 4
        && swapped
        && width >= 8
        && height >= 8
        && std::is_x86_feature_detected!("avx2")
    {
        assert_eq!(kernel_path, "avx2-transpose");
    }
    assert_native_transpose_metrics(
        width,
        height,
        channels,
        swapped,
        source.len(),
        &method,
        kernel_path,
        blocks,
        tail,
    );
}

fn transpose_swaps_dimensions(method: &TransposeMethod) -> bool {
    matches!(
        method,
        TransposeMethod::Rotate90
            | TransposeMethod::Rotate270
            | TransposeMethod::Transpose
            | TransposeMethod::Transverse
    )
}

fn expected_transpose_bytes(
    source: &[u8],
    width: usize,
    height: usize,
    channels: usize,
    method: &TransposeMethod,
) -> Vec<u8> {
    let (expected_width, _) = if transpose_swaps_dimensions(method) {
        (height, width)
    } else {
        (width, height)
    };
    let mut expected = vec![0; source.len()];
    // Scatter the original samples using the public forward coordinate
    // mapping, independent of the kernel's inverse mapping and tile layout.
    for y in 0..height {
        for x in 0..width {
            let (target_x, target_y) = match method {
                TransposeMethod::FlipLeftRight => (width - 1 - x, y),
                TransposeMethod::FlipTopBottom => (x, height - 1 - y),
                TransposeMethod::Rotate90 => (y, width - 1 - x),
                TransposeMethod::Rotate180 => (width - 1 - x, height - 1 - y),
                TransposeMethod::Rotate270 => (height - 1 - y, x),
                TransposeMethod::Transpose => (y, x),
                TransposeMethod::Transverse => (height - 1 - y, width - 1 - x),
            };
            let source_start = (y * width + x) * channels;
            let target_start = (target_y * expected_width + target_x) * channels;
            expected[target_start..target_start + channels]
                .copy_from_slice(&source[source_start..source_start + channels]);
        }
    }
    expected
}

fn assert_native_transpose_metrics(
    width: usize,
    height: usize,
    channels: usize,
    swapped: bool,
    _source_len: usize,
    _method: &TransposeMethod,
    kernel_path: &str,
    blocks: u64,
    tail: u64,
) {
    let tile = if kernel_path == "avx2-transpose" {
        8
    } else {
        match channels {
            1 => 16,
            2 => 8,
            _ => 4,
        }
    };
    if swapped && width >= tile && height >= tile {
        let tiles = (width / tile) * (height / tile);
        #[cfg(feature = "parallel")]
        let uses_collect =
            transpose_native_collect_admitted(width, height, channels, _source_len, _method);
        #[cfg(not(feature = "parallel"))]
        let uses_collect = false;
        if uses_collect {
            // Work includes complete groups rebuilt at task splits.
            // The forced-split test checks that exact extra count.
            assert!(blocks >= (tiles * tile) as u64);
            assert_eq!(blocks % height as u64, 0);
        } else {
            assert_eq!(blocks, (tiles * tile) as u64);
        }
        assert_eq!(
            tail,
            ((width * height - tiles * tile * tile) * channels) as u64
        );
    }
}

#[test]
fn transpose_native_tiles_preserve_pixels_at_odd_boundaries() {
    for (width, height) in [
        (0, 0),
        (0, 33),
        (33, 0),
        (1, 37),
        (37, 1),
        (3, 5),
        (4, 4),
        (7, 9),
        (8, 8),
        (15, 17),
        (16, 16),
        (31, 33),
        (33, 31),
        (65, 67),
        (513, 515),
    ] {
        for channels in 1..=4 {
            let source: Vec<u8> = (0..width * height * channels)
                .map(|index| ((index * 73 + index / 17 * 11 + 29) % 256) as u8)
                .collect();
            assert_native_transpose_matches_forward_mapping(&source, width, height, channels);
        }
    }
}

#[test]
fn transpose_native_tiles_preserve_float_payload_bits() {
    let patterns = [0x7f80_0001u32, 0x7fc1_2345, 0x8000_0000, 0xff80_0000];
    let source: Vec<u8> = (0..35)
        .flat_map(|index| patterns[index % patterns.len()].to_ne_bytes())
        .collect();
    let (transposed, width, height, _, _, _) =
        native_transpose_bytes(&source, 7, 5, 4, TransposeMethod::Transpose)
            .expect("packed F transpose");
    let (restored, _, _, _, _, _) =
        native_transpose_bytes(&transposed, width, height, 4, TransposeMethod::Transpose)
            .expect("inverse packed F transpose");
    assert_eq!(restored, source);
}

#[test]
fn transpose_native_collect_preserves_aligned_pixels() {
    for (width, height) in [(768, 768), (768, 772)] {
        for channels in [3, 4] {
            let source: Vec<u8> = (0..width * height * channels)
                .map(|index| ((index * 73 + index / 17 * 11 + 29) % 256) as u8)
                .collect();
            #[cfg(feature = "parallel")]
            assert!(transpose_native_collect_admitted(
                width,
                height,
                channels,
                source.len(),
                &TransposeMethod::Transpose,
            ));
            assert_native_transpose_matches_forward_mapping(&source, width, height, channels);
        }
    }
}

#[test]
fn transpose_native_collect_preserves_typed_payload_bits() {
    let (width, height) = (772, 768);
    let patterns = [
        0x7f80_0001u32,
        0x7fc1_2345,
        0x8000_0000,
        0x0000_0000,
        0xff80_0000,
        0x3f81_2345,
        0xffc5_4321,
    ];
    let source: Vec<u8> = (0..width * height)
        .flat_map(|index| patterns[(index * 3 + index / 19) % patterns.len()].to_ne_bytes())
        .collect();
    // Packed I/F storage uses the same four-byte permutation without
    // interpreting or canonicalizing NaNs, infinities, or signed zero.
    assert_native_transpose_matches_forward_mapping(&source, width, height, 4);
}

#[test]
fn transpose_native_collect_cutoffs_preserve_fallback_pixels() {
    // Exactly 1 MiB and just above 8 MiB both keep the direct-write path.
    for (width, height) in [(512, 512), (1024, 2052)] {
        let source: Vec<u8> = (0..width * height * 4)
            .map(|index| ((index * 73 + index / 17 * 11 + 29) % 256) as u8)
            .collect();
        #[cfg(feature = "parallel")]
        assert!(!transpose_native_collect_admitted(
            width,
            height,
            4,
            source.len(),
            &TransposeMethod::Transpose,
        ));
        assert_native_transpose_matches_forward_mapping(&source, width, height, 4);
    }
}

#[cfg(feature = "parallel")]
#[test]
fn transpose_native_collect_admission_requires_complete_bounded_tiles() {
    for (width, height, channels, expected) in [
        (512, 512, 4, false),
        (512, 516, 4, true),
        (1024, 2048, 4, true),
        (1024, 2052, 4, false),
        (768, 769, 3, false),
        (769, 768, 4, false),
        (2048, 2048, 1, false),
        (1024, 1024, 2, false),
        (0, 768, 4, false),
    ] {
        assert_eq!(
            transpose_native_collect_admitted(
                width,
                height,
                channels,
                width * height * channels,
                &TransposeMethod::Transpose,
            ),
            expected,
            "{width}x{height}, {channels} channels"
        );
    }
    for method in [
        TransposeMethod::FlipLeftRight,
        TransposeMethod::FlipTopBottom,
        TransposeMethod::Rotate180,
    ] {
        assert!(!transpose_native_collect_admitted(
            768,
            768,
            4,
            768 * 768 * 4,
            &method,
        ));
    }
}

#[cfg(feature = "parallel")]
#[test]
fn transpose_native_collect_counts_split_cache_work_and_allocations() {
    use crate::checked_dims::CheckedDims;
    use crate::compute::Backend;
    use std::sync::atomic::Ordering;

    struct RestoreTelemetry(bool);
    impl Drop for RestoreTelemetry {
        fn drop(&mut self) {
            Backend::set_pipeline_telemetry_enabled(self.0);
        }
    }

    let (width, height) = (772usize, 768usize);
    let source: Vec<u8> = (0..width * height * 3)
        .map(|index| ((index * 73 + index / 17 * 11 + 29) % 256) as u8)
        .collect();
    let output_dims = CheckedDims::new(height as u32, width as u32, 3).unwrap();
    let scratch_dims = CheckedDims::new(height as u32, 4, 3).unwrap();
    let counters = TransposeNativeCollectCounters::default();
    let block_count = output_dims.total_bytes() / 12;
    let midpoint = block_count / 2;
    assert_eq!(midpoint / (height / 4), 386);

    let _telemetry = RestoreTelemetry(Backend::set_pipeline_telemetry_enabled(true));
    let _ = Backend::take_pipeline_allocation_telemetry();
    let mut blocks = Vec::with_capacity(block_count);
    // Force two indexed tasks to meet halfway through rows 384..388.
    // Each task must build that complete group in its private cache.
    for indices in [0..midpoint, midpoint..block_count] {
        let mut cache = TransposeNativeRowCache::new(&scratch_dims, &counters);
        for index in indices {
            blocks.push(cache.block::<3, 12>(&source, width, height, index, false, false));
        }
    }
    assert_eq!(counters.scratch_allocations.load(Ordering::Relaxed), 2);
    assert_eq!(counters.cache_rebuilds.load(Ordering::Relaxed), 194);
    let before_finish = Backend::take_pipeline_allocation_telemetry();
    assert_eq!(before_finish.allocation_count, 0);
    assert_eq!(before_finish.allocated_bytes, 0);

    let (actual, vector_blocks) = counters.finish(blocks, height, scratch_dims.total_bytes());
    assert_eq!(vector_blocks, 194 * height as u64);
    assert_eq!(vector_blocks - (width * height / 4) as u64, height as u64);
    let allocations = Backend::take_pipeline_allocation_telemetry();
    assert_eq!(allocations.allocation_count, 3);
    assert_eq!(
        allocations.allocated_bytes,
        (output_dims.total_bytes() + 2 * scratch_dims.total_bytes()) as u64
    );
    let mut expected = vec![0; output_dims.total_bytes()];
    for y in 0..height {
        for x in 0..width {
            let source_start = (y * width + x) * 3;
            let target_start = (x * height + y) * 3;
            expected[target_start..target_start + 3]
                .copy_from_slice(&source[source_start..source_start + 3]);
        }
    }
    assert_eq!(actual, expected);
}

#[cfg(feature = "parallel")]
#[test]
fn transpose_native_collect_cache_handles_arbitrary_block_access() {
    check_native_collect_cache_access::<3, 12>();
    check_native_collect_cache_access::<4, 16>();
}

#[cfg(feature = "parallel")]
fn check_native_collect_cache_access<const CHANNELS: usize, const BLOCK_BYTES: usize>() {
    use crate::checked_dims::CheckedDims;

    let (width, height) = (16usize, 20usize);
    let source: Vec<u8> = (0..width * height * CHANNELS)
        .map(|index| ((index * 73 + index / 17 * 11 + 29) % 256) as u8)
        .collect();
    let output_dims = CheckedDims::new(height as u32, width as u32, CHANNELS as u8).unwrap();
    let scratch_dims = CheckedDims::new(height as u32, 4, CHANNELS as u8).unwrap();
    for method in [
        TransposeMethod::Transpose,
        TransposeMethod::Rotate90,
        TransposeMethod::Rotate270,
        TransposeMethod::Transverse,
    ] {
        assert_native_collect_cache_method::<CHANNELS, BLOCK_BYTES>(
            &source,
            width,
            height,
            &output_dims,
            &scratch_dims,
            method,
        );
    }
}

#[cfg(feature = "parallel")]
fn assert_native_collect_cache_method<const CHANNELS: usize, const BLOCK_BYTES: usize>(
    source: &[u8],
    width: usize,
    height: usize,
    output_dims: &crate::checked_dims::CheckedDims,
    scratch_dims: &crate::checked_dims::CheckedDims,
    method: TransposeMethod,
) {
    use std::sync::atomic::Ordering;

    let mut expected = vec![0; output_dims.total_bytes()];
    for y in 0..height {
        for x in 0..width {
            let (target_x, target_y) = match method {
                TransposeMethod::Transpose => (y, x),
                TransposeMethod::Rotate90 => (y, width - 1 - x),
                TransposeMethod::Rotate270 => (height - 1 - y, x),
                TransposeMethod::Transverse => (height - 1 - y, width - 1 - x),
                _ => unreachable!(),
            };
            let from = (y * width + x) * CHANNELS;
            let to = (target_y * height + target_x) * CHANNELS;
            expected[to..to + CHANNELS].copy_from_slice(&source[from..from + CHANNELS]);
        }
    }
    let reverse_x = matches!(
        method,
        TransposeMethod::Rotate90 | TransposeMethod::Transverse
    );
    let reverse_y = matches!(
        method,
        TransposeMethod::Rotate270 | TransposeMethod::Transverse
    );
    let counters = TransposeNativeCollectCounters::default();
    {
        let mut cache = TransposeNativeRowCache::new(scratch_dims, &counters);
        // Start within a group, visit both edges, jump forward,
        // then revisit earlier groups. Group sequence is
        // 0,0,0,1,1,1,3,0,3,0: exactly six complete rebuilds.
        for index in [
            height / 2,
            0,
            height - 1,
            height,
            height + 1,
            2 * height - 1,
            3 * height + 2,
            height / 2,
            4 * height - 1,
            0,
        ] {
            let actual = cache
                .block::<CHANNELS, BLOCK_BYTES>(source, width, height, index, reverse_x, reverse_y);
            let start = index * BLOCK_BYTES;
            assert_eq!(
                actual.as_slice(),
                &expected[start..start + BLOCK_BYTES],
                "{method:?}, {CHANNELS} channels, block {index}"
            );
        }
    }
    assert_eq!(counters.scratch_allocations.load(Ordering::Relaxed), 1);
    assert_eq!(counters.cache_rebuilds.load(Ordering::Relaxed), 6);
}

#[cfg(feature = "parallel")]
mod transpose_native_collect_sixteen_pixel_tests {
    use super::super::{
        TransposeNativeCollectCounters, TransposeNativeRowCache, transpose_native_collect,
    };
    use crate::checked_dims::CheckedDims;
    use crate::compute::Backend;
    use crate::pipeline::TransposeMethod;
    use std::sync::atomic::Ordering;

    fn reference<const CHANNELS: usize>(
        source: &[u8],
        width: usize,
        height: usize,
        method: &TransposeMethod,
    ) -> Vec<u8> {
        let dimensions = CheckedDims::new(height as u32, width as u32, CHANNELS as u8).unwrap();
        let mut output = vec![0; dimensions.total_bytes()];
        for y in 0..height {
            for x in 0..width {
                let (tx, ty) = match method {
                    TransposeMethod::Transpose => (y, x),
                    TransposeMethod::Rotate90 => (y, width - 1 - x),
                    TransposeMethod::Rotate270 => (height - 1 - y, x),
                    TransposeMethod::Transverse => (height - 1 - y, width - 1 - x),
                    _ => unreachable!(),
                };
                let from = (y * width + x) * CHANNELS;
                let to = (ty * height + tx) * CHANNELS;
                output[to..to + CHANNELS].copy_from_slice(&source[from..from + CHANNELS]);
            }
        }
        output
    }

    fn patterned<const CHANNELS: usize>(width: usize, height: usize) -> Vec<u8> {
        let dimensions = CheckedDims::new(width as u32, height as u32, CHANNELS as u8).unwrap();
        let exceptional = [0x7f80_0001u32, 0x7fc1_2345, 0x8000_0000, 0, 0xff80_0000];
        let mut source: Vec<u8> = (0..dimensions.total_bytes().div_ceil(4))
            .flat_map(|index| {
                let bits = if index % 11 < exceptional.len() {
                    exceptional[index % 11]
                } else {
                    (index as u32)
                        .wrapping_mul(0x9e37_79b9)
                        .rotate_left((index % 31) as u32)
                };
                bits.to_ne_bytes()
            })
            .collect();
        source.truncate(dimensions.total_bytes());
        // Unused ImageBuffer suffix must never become a pixel.
        source.extend_from_slice(&[0xA5; 19]);
        source
    }

    fn verify_allocations(
        logical_bytes: usize,
        height: usize,
        channels: usize,
    ) -> crate::compute::PipelineAllocationTelemetry {
        let allocations = Backend::take_pipeline_allocation_telemetry();
        assert!(allocations.allocation_count >= 2);
        assert_eq!(
            allocations.allocated_bytes,
            logical_bytes as u64
                + (allocations.allocation_count - 1) * (height * 4 * channels) as u64,
        );
        allocations
    }

    fn check_pixels<const CHANNELS: usize, const BLOCK_BYTES: usize>() {
        for (width, height) in [
            (4, 4),
            (4, 12),
            (12, 4),
            (8, 12),
            (12, 20),
            (20, 12),
            (32, 36),
            (36, 32),
        ] {
            let source = patterned::<CHANNELS>(width, height);
            let original = source.clone();
            let dims = CheckedDims::new(height as u32, width as u32, CHANNELS as u8).unwrap();
            for (method, rx, ry) in [
                (TransposeMethod::Transpose, false, false),
                (TransposeMethod::Rotate90, true, false),
                (TransposeMethod::Rotate270, false, true),
                (TransposeMethod::Transverse, true, true),
            ] {
                let expected = reference::<CHANNELS>(&source, width, height, &method);
                let _ = Backend::take_pipeline_allocation_telemetry();
                let (actual, vectors) = transpose_native_collect::<CHANNELS, BLOCK_BYTES>(
                    &source, width, height, rx, ry, &dims,
                )
                .unwrap();
                verify_allocations(dims.total_bytes(), height, CHANNELS);
                assert_eq!(actual, expected);
                assert!(vectors >= (width * height / 4) as u64);
                assert_eq!(vectors % height as u64, 0);
                assert_eq!(source, original);
            }
        }
    }

    fn arbitrary_cache_checks<const CHANNELS: usize, const BLOCK_BYTES: usize>() {
        let (width, height) = (16usize, 20usize);
        let source = patterned::<CHANNELS>(width, height);
        let group_blocks = height / 4;
        let count = width * height / 16;
        let scratch_dims = CheckedDims::new(height as u32, 4, CHANNELS as u8).unwrap();
        for (method, rx, ry) in [
            (TransposeMethod::Transpose, false, false),
            (TransposeMethod::Rotate90, true, false),
            (TransposeMethod::Rotate270, false, true),
            (TransposeMethod::Transverse, true, true),
        ] {
            let expected = reference::<CHANNELS>(&source, width, height, &method);
            let counters = TransposeNativeCollectCounters::default();
            {
                let mut cache = TransposeNativeRowCache::new(&scratch_dims, &counters);
                cache.scratch.fill(0xA5);
                for index in [
                    count - 1,
                    0,
                    group_blocks - 1,
                    group_blocks,
                    group_blocks + 1,
                    count - 1,
                    0,
                    count - 2,
                    count - 1,
                    group_blocks - 1,
                ] {
                    let actual =
                        cache.block::<CHANNELS, BLOCK_BYTES>(&source, width, height, index, rx, ry);
                    assert_eq!(
                        actual.as_slice(),
                        &expected[index * BLOCK_BYTES..(index + 1) * BLOCK_BYTES]
                    );
                }
            }
            assert_eq!(counters.scratch_allocations.load(Ordering::Relaxed), 1);
            assert_eq!(counters.cache_rebuilds.load(Ordering::Relaxed), 7);
        }
    }

    fn forced_split_checks<const CHANNELS: usize, const BLOCK_BYTES: usize>() {
        let (width, height) = (772usize, 768usize);
        let source = patterned::<CHANNELS>(width, height);
        let count = width * height / 16;
        let midpoint = count / 2;
        assert_eq!(midpoint * 16 / height, 386);
        let scratch_dims = CheckedDims::new(height as u32, 4, CHANNELS as u8).unwrap();
        for (method, rx, ry) in [
            (TransposeMethod::Transpose, false, false),
            (TransposeMethod::Rotate90, true, false),
            (TransposeMethod::Rotate270, false, true),
            (TransposeMethod::Transverse, true, true),
        ] {
            let expected = reference::<CHANNELS>(&source, width, height, &method);
            let counters = TransposeNativeCollectCounters::default();
            let _ = Backend::take_pipeline_allocation_telemetry();
            let mut blocks = Vec::with_capacity(count);
            for indices in [0..midpoint, midpoint..count] {
                let mut cache = TransposeNativeRowCache::new(&scratch_dims, &counters);
                cache.scratch.fill(0xA5);
                for index in indices {
                    blocks.push(
                        cache.block::<CHANNELS, BLOCK_BYTES>(&source, width, height, index, rx, ry),
                    );
                }
            }
            assert_eq!(counters.cache_rebuilds.load(Ordering::Relaxed), 194);
            assert_eq!(counters.scratch_allocations.load(Ordering::Relaxed), 2);
            let before_finish = Backend::take_pipeline_allocation_telemetry();
            assert_eq!(before_finish.allocation_count, 0);
            assert_eq!(before_finish.allocated_bytes, 0);
            let (output, vectors) = counters.finish(blocks, height, scratch_dims.total_bytes());
            let allocations = Backend::take_pipeline_allocation_telemetry();
            assert_eq!(output, expected);
            assert_eq!(vectors, 194 * height as u64);
            assert_eq!(vectors - (width * height / 4) as u64, height as u64);
            assert_eq!(allocations.allocation_count, 3);
            assert_eq!(
                allocations.allocated_bytes,
                (width * height * CHANNELS + 2 * height * 4 * CHANNELS) as u64
            );
        }
    }

    struct RestoreTelemetry(bool);
    impl Drop for RestoreTelemetry {
        fn drop(&mut self) {
            Backend::set_pipeline_telemetry_enabled(self.0);
        }
    }

    #[test]
    fn transpose_native_collect_sixteen_pixel_preserves_payloads_and_edges() {
        let _telemetry = RestoreTelemetry(Backend::set_pipeline_telemetry_enabled(true));
        check_pixels::<3, 48>();
        check_pixels::<4, 64>();
    }

    #[test]
    fn transpose_native_collect_sixteen_pixel_handles_arbitrary_cache_access() {
        arbitrary_cache_checks::<3, 48>();
        arbitrary_cache_checks::<4, 64>();
    }

    #[test]
    fn transpose_native_collect_sixteen_pixel_counts_split_work_and_allocations() {
        let _telemetry = RestoreTelemetry(Backend::set_pipeline_telemetry_enabled(true));
        forced_split_checks::<3, 48>();
        forced_split_checks::<4, 64>();
    }
}
