//! Contract tests for parallel odd-width Transpose collection.

use super::native_transpose_bytes;
use super::{
    TransposeNativeOddCollectCounters, TransposeNativeOddRowCache,
    transpose_native_odd_collect_admitted,
};
use crate::pipeline::TransposeMethod;

#[cfg(feature = "parallel")]
fn odd_transpose_reference(
    source: &[u8],
    width: usize,
    height: usize,
    method: &TransposeMethod,
) -> Vec<u8> {
    let output_width = if matches!(
        method,
        TransposeMethod::Rotate90
            | TransposeMethod::Rotate270
            | TransposeMethod::Transpose
            | TransposeMethod::Transverse
    ) {
        height
    } else {
        width
    };
    let mut expected = vec![0; width * height * 4];
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
            let from = (y * width + x) * 4;
            let to = (target_y * output_width + target_x) * 4;
            expected[to..to + 4].copy_from_slice(&source[from..from + 4]);
        }
    }
    expected
}

#[cfg(feature = "parallel")]
#[test]
fn transpose_native_odd_collect_admission_bounds() {
    let method = TransposeMethod::Transpose;
    for (width, height, expected) in [
        (767, 683, false), // Immediately below two MiB.
        (768, 683, true),
        (1024, 512, false), // Exactly two MiB, aligned.
        (2047, 1024, true),
        (2049, 1024, false), // Above eight MiB.
        (1024, 2048, false), // Aligned eight-MiB collector stays separate.
        (769, 2048, true),
        (769, 2049, false), // Four-row scratch would exceed 32 KiB.
        (1604, 401, true),
        (1605, 401, false),
        (401, 1604, true),
        (401, 1605, false),
        (255, 2048, false),
        (2048, 255, false),
        (33, 31777, false),
        (31777, 33, false),
        (768, 769, true),
        (769, 768, true),
        (1024, 1024, false),
        (513, 515, false),
        (0, 1025, false),
        (1025, 0, false),
    ] {
        assert_eq!(
            transpose_native_odd_collect_admitted(width, height, 4, width * height * 4, &method,),
            expected,
            "{width}x{height}"
        );
    }
    for channels in 1..=4 {
        for method in [
            TransposeMethod::FlipLeftRight,
            TransposeMethod::FlipTopBottom,
            TransposeMethod::Rotate90,
            TransposeMethod::Rotate180,
            TransposeMethod::Rotate270,
            TransposeMethod::Transpose,
            TransposeMethod::Transverse,
        ] {
            let swapped = matches!(
                method,
                TransposeMethod::Rotate90
                    | TransposeMethod::Rotate270
                    | TransposeMethod::Transpose
                    | TransposeMethod::Transverse
            );
            assert_eq!(
                transpose_native_odd_collect_admitted(
                    1023,
                    1025,
                    channels,
                    1023 * 1025 * channels,
                    &method,
                ),
                channels == 4 && swapped,
            );
        }
    }
}

#[cfg(feature = "parallel")]
#[test]
fn transpose_native_odd_collect_preserves_all_methods_and_payload_bits() {
    use crate::compute::Backend;

    struct RestoreTelemetry(bool);
    impl Drop for RestoreTelemetry {
        fn drop(&mut self) {
            Backend::set_pipeline_telemetry_enabled(self.0);
        }
    }
    let _telemetry = RestoreTelemetry(Backend::set_pipeline_telemetry_enabled(true));
    let typed_bits = [
        0x7f80_0001u32,
        0x7fc1_2345,
        0x8000_0000,
        0x0000_0000,
        0xff80_0000,
        0xffc5_4321,
    ];
    for (width, height) in [(768usize, 769usize), (1023, 1025)] {
        assert_native_odd_collect_payload_case(width, height, &typed_bits);
    }
}

#[cfg(feature = "parallel")]
fn assert_native_odd_collect_payload_case(width: usize, height: usize, typed_bits: &[u32; 6]) {
    let mut source: Vec<u8> = (0..width * height)
        .flat_map(|index| {
            let bits = if index % 11 < typed_bits.len() {
                typed_bits[index % 11]
            } else {
                (index as u32)
                    .wrapping_mul(0x9e37_79b9)
                    .rotate_left((index % 31) as u32)
            };
            bits.to_ne_bytes()
        })
        .collect();
    // Valid ImageBuffer storage may include unused trailing bytes.
    source.extend_from_slice(&[0xA5; 19]);
    let original = source.clone();
    for method in [
        TransposeMethod::FlipLeftRight,
        TransposeMethod::FlipTopBottom,
        TransposeMethod::Rotate90,
        TransposeMethod::Rotate180,
        TransposeMethod::Rotate270,
        TransposeMethod::Transpose,
        TransposeMethod::Transverse,
    ] {
        assert_native_odd_collect_payload_method(&source, &original, width, height, method);
    }
}

#[cfg(feature = "parallel")]
fn assert_native_odd_collect_payload_method(
    source: &[u8],
    original: &[u8],
    width: usize,
    height: usize,
    method: TransposeMethod,
) {
    let context = crate::compute::ExecutionContext::for_backend(crate::compute::Backend::Simd);
    let swapped = matches!(
        method,
        TransposeMethod::Rotate90
            | TransposeMethod::Rotate270
            | TransposeMethod::Transpose
            | TransposeMethod::Transverse
    );
    let expected = odd_transpose_reference(source, width, height, &method);
    let _ = crate::compute::Backend::take_pipeline_allocation_telemetry();
    let (actual, out_width, out_height, vectors, tail, _) =
        native_transpose_bytes(
            &context,
            source,
            width as u32,
            height as u32,
            4,
            method.clone(),
        )
            .expect("odd native four-byte transpose");
    let allocations = crate::compute::Backend::take_pipeline_allocation_telemetry();
    assert_eq!(actual, expected, "{width}x{height} {method:?}");
    assert_eq!(
        (out_width, out_height),
        if swapped {
            (height as u32, width as u32)
        } else {
            (width as u32, height as u32)
        }
    );
    assert_native_odd_collect_payload_metrics(
        width,
        height,
        expected.len(),
        &method,
        swapped,
        vectors,
        tail,
        allocations.allocation_count,
        allocations.allocated_bytes,
    );
    assert_eq!(source, original);
}

#[cfg(feature = "parallel")]
fn assert_native_odd_collect_payload_metrics(
    width: usize,
    height: usize,
    expected_len: usize,
    method: &TransposeMethod,
    swapped: bool,
    vectors: u64,
    tail: u64,
    allocation_count: u64,
    allocated_bytes: u64,
) {
    if swapped {
        assert!(transpose_native_odd_collect_admitted(
            width,
            height,
            4,
            expected_len,
            method,
        ));
        let tile_pixels = (width / 4 * 4) * (height / 4 * 4);
        // Indexed task splits may repeat complete cache fills.
        // The deterministic split test below proves exact work.
        assert!(vectors >= (tile_pixels / 4) as u64);
        assert!(tail >= ((width * height - tile_pixels) * 4) as u64);
        let padded_bytes = (width * height).div_ceil(4) * 16;
        let scratch_bytes = height * 4 * 4;
        assert!(allocation_count >= 2);
        assert_eq!(
            allocated_bytes,
            padded_bytes as u64 + (allocation_count - 1) * scratch_bytes as u64,
        );
    }
}

#[cfg(feature = "parallel")]
#[test]
fn transpose_native_odd_collect_boundaries_keep_fallback_exact() {
    for (width, height, admitted) in [
        (767usize, 683usize, false),
        (768, 683, true),
        (2047, 1024, true),
        (2049, 1024, false),
        (769, 2048, true),
        (769, 2049, false),
        (1604, 401, true),
        (1605, 401, false),
    ] {
        let source: Vec<u8> = (0..width * height * 4)
            .map(|index| ((index * 73 + index / 17 * 29) % 256) as u8)
            .collect();
        let method = TransposeMethod::Transverse;
        assert_eq!(
            transpose_native_odd_collect_admitted(width, height, 4, source.len(), &method),
            admitted,
        );
        let expected = odd_transpose_reference(&source, width, height, &method);
        let context = crate::compute::ExecutionContext::for_backend(crate::compute::Backend::Simd);
        let (actual, out_width, out_height, vectors, tail, _) =
            native_transpose_bytes(&context, &source, width as u32, height as u32, 4, method)
                .expect("odd transpose boundary");
        assert_eq!(actual, expected, "{width}x{height}");
        assert_eq!((out_width, out_height), (height as u32, width as u32));
        if !admitted {
            let tile_pixels = (width / 4 * 4) * (height / 4 * 4);
            assert_eq!(vectors, (tile_pixels / 4) as u64);
            assert_eq!(tail, ((width * height - tile_pixels) * 4) as u64);
        }
    }
}

#[cfg(feature = "parallel")]
#[test]
fn transpose_native_odd_collect_all_padding_lengths() {
    use crate::checked_dims::CheckedDims;
    use crate::compute::Backend;

    struct RestoreTelemetry(bool);
    impl Drop for RestoreTelemetry {
        fn drop(&mut self) {
            Backend::set_pipeline_telemetry_enabled(self.0);
        }
    }
    let _telemetry = RestoreTelemetry(Backend::set_pipeline_telemetry_enabled(true));
    let height = 13usize;
    for width in [13usize, 14, 15, 16] {
        let source: Vec<u8> = (0..width * height * 4)
            .map(|index| ((index * 73 + index / 17 * 29) % 256) as u8)
            .collect();
        let dimensions = CheckedDims::new(height as u32, width as u32, 4).unwrap();
        for (method, reverse_x, reverse_y) in [
            (TransposeMethod::Transpose, false, false),
            (TransposeMethod::Rotate90, true, false),
            (TransposeMethod::Rotate270, false, true),
            (TransposeMethod::Transverse, true, true),
        ] {
            let expected = odd_transpose_reference(&source, width, height, &method);
            let _ = Backend::take_pipeline_allocation_telemetry();
            // Exercise every final-block length directly below admission;
            // admission tests separately preserve the scheduling bounds.
            let (actual, _, _) = super::transpose_native_odd_collect(
                &source,
                width,
                height,
                reverse_x,
                reverse_y,
                &dimensions,
            )
            .expect("complete initialized odd transpose blocks");
            let allocations = Backend::take_pipeline_allocation_telemetry();
            assert_eq!(actual, expected);
            let padded_bytes = (width * height).div_ceil(4) * 16;
            assert_eq!(padded_bytes - actual.len(), (4 - width % 4) % 4 * 4);
            assert!(allocations.allocation_count >= 2);
            assert_eq!(
                allocations.allocated_bytes,
                padded_bytes as u64 + (allocations.allocation_count - 1) * (height * 4 * 4) as u64,
            );
        }
    }
}

#[cfg(feature = "parallel")]
#[test]
fn transpose_native_odd_collect_final_cache_revisits_and_split_counters() {
    use crate::checked_dims::CheckedDims;
    use crate::compute::Backend;

    struct RestoreTelemetry(bool);
    impl Drop for RestoreTelemetry {
        fn drop(&mut self) {
            Backend::set_pipeline_telemetry_enabled(self.0);
        }
    }
    let _telemetry = RestoreTelemetry(Backend::set_pipeline_telemetry_enabled(true));
    let (width, height) = (15usize, 13usize);
    let source: Vec<u8> = (0..width * height * 4)
        .map(|index| ((index * 73 + index / 17 * 29) % 256) as u8)
        .collect();
    let scratch_dims = CheckedDims::new(height as u32, 4, 4).unwrap();
    let count = (width * height).div_ceil(4);
    for (method, reverse_x, reverse_y) in [
        (TransposeMethod::Transpose, false, false),
        (TransposeMethod::Rotate90, true, false),
        (TransposeMethod::Rotate270, false, true),
        (TransposeMethod::Transverse, true, true),
    ] {
        assert_transpose_native_odd_collect_method(
            &source,
            width,
            height,
            count,
            &scratch_dims,
            method,
            reverse_x,
            reverse_y,
        );
    }
}

#[cfg(feature = "parallel")]
fn assert_transpose_native_odd_collect_method(
    source: &[u8],
    width: usize,
    height: usize,
    count: usize,
    scratch_dims: &crate::checked_dims::CheckedDims,
    method: TransposeMethod,
    reverse_x: bool,
    reverse_y: bool,
) {
    use crate::compute::Backend;
    use std::sync::atomic::Ordering;

    let expected = odd_transpose_reference(source, width, height, &method);
    let counters = TransposeNativeOddCollectCounters::default();
    {
        let mut cache = TransposeNativeOddRowCache::new(scratch_dims, &counters);
        cache.scratch.fill(0xA5);
        // Four complete and three partial cache fills, including a
        // first access to the padded final block and backward visits.
        for index in [
            count - 1,
            0,
            height - 1,
            height,
            height + 1,
            count - 1,
            0,
            count - 2,
            count - 1,
            height - 1,
        ] {
            let actual = cache.block(source, width, height, count, index, reverse_x, reverse_y);
            let start = index * 16;
            let valid = (expected.len() - start).min(16);
            assert_eq!(&actual[..valid], &expected[start..start + valid]);
        }
    }
    assert_eq!(counters.scratch_allocations.load(Ordering::Relaxed), 1);
    assert_eq!(counters.vector_blocks.load(Ordering::Relaxed), 48);
    assert_eq!(counters.scalar_tail.load(Ordering::Relaxed), 532);

    let counters = TransposeNativeOddCollectCounters::default();
    let mut blocks = Vec::with_capacity(count);
    let split = height + height / 2;
    for range in [0..split, split..count] {
        let mut cache = TransposeNativeOddRowCache::new(scratch_dims, &counters);
        cache.scratch.fill(0xA5);
        for index in range {
            blocks.push(cache.block(source, width, height, count, index, reverse_x, reverse_y));
        }
    }
    let _ = Backend::take_pipeline_allocation_telemetry();
    let (actual, vectors, tail) =
        counters.finish(blocks, expected.len(), scratch_dims.total_bytes());
    let allocations = Backend::take_pipeline_allocation_telemetry();
    assert_eq!(actual, expected);
    // The middle group is computed by both tasks: 48 actual vectors
    // instead of 36 unique vectors, and 220 rather than 204 tails.
    assert_eq!(vectors, 48);
    assert_eq!(tail, 220);
    assert_eq!(allocations.allocation_count, 3);
    assert_eq!(allocations.allocated_bytes, 1200);
    assert_eq!(actual.len(), 780); // Four initialized padding bytes removed.
}
