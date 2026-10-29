#[test]
fn native_rgb_3x5_vector_reduce_matches_scalar_with_partial_edges() {
    let (width, height) = (67u32, 53u32);
    let source: Vec<u8> = (0..width as usize * height as usize * 3)
        .map(|index| (index.wrapping_mul(37).wrapping_add(index / 11 * 17 + 29) % 256) as u8)
        .collect();
    let image = crate::raster::DynamicImage::ImageRgb8(
        crate::raster::RgbImage::from_raw(width, height, source.clone())
            .expect("RGB source dimensions must be valid"),
    );
    let geometry = super::NativeReduceGeometry::new(width as usize, height as usize, 3, 5)
        .expect("3×5 RGB reduction geometry must be supported");
    let vector_block = super::native_reduce_rgb_3x5_vector_block(&source, &geometry, 0, 0)
        .expect("an interior vector block must use the specialized kernel");
    let mut expected_block = [0u8; super::SIMD_REDUCE_LANES * 3];
    let average = geometry.full.expect("full blocks have an average");
    for lane in 0..super::SIMD_REDUCE_LANES {
        let (sums, _, multiplier, amend) =
            super::native_reduce_pixel_sums(&source, &geometry, 3, false, lane, 0)
                .expect("the scalar reference block must be in bounds");
        assert_eq!(multiplier, average.multiplier);
        assert_eq!(amend, average.amend);
        for channel in 0..3 {
            expected_block[lane * 3 + channel] =
                super::native_reduce_average(sums[channel], multiplier, amend);
        }
    }
    assert_eq!(
        &vector_block[..expected_block.len()],
        expected_block.as_slice()
    );

    let (actual, output_width, output_height, vector_blocks, scalar_tail) =
        super::native_reduce_bytes(&image, 3, false, 3, 5, true)
            .expect("native RGB reduction must succeed");
    assert_eq!((output_width, output_height), (23, 11));
    assert_eq!(vector_blocks, 22);
    assert_eq!(scalar_tail, 77);
    let mut expected = Vec::with_capacity(actual.len());
    for y in 0..output_height as usize {
        for x in 0..output_width as usize {
            let (sums, _, multiplier, amend) =
                super::native_reduce_pixel_sums(&source, &geometry, 3, false, x, y)
                    .expect("scalar reference pixel must be in bounds");
            for channel in 0..3 {
                expected.push(super::native_reduce_average(
                    sums[channel],
                    multiplier,
                    amend,
                ));
            }
        }
    }
    assert_eq!(actual, expected);
}

#[test]
fn native_3byte_2x2_vector_reduce_matches_scalar_with_partial_edges() {
    let (width, height) = (33u32, 5u32);
    let source: Vec<u8> = (0..width as usize * height as usize * 3)
        .map(|index| (index.wrapping_mul(53).wrapping_add(index / 7 * 19 + 11) % 256) as u8)
        .collect();
    let image = crate::raster::DynamicImage::ImageRgb8(
        crate::raster::RgbImage::from_raw(width, height, source.clone())
            .expect("RGB source dimensions must be valid"),
    );
    let geometry = super::NativeReduceGeometry::new(width as usize, height as usize, 2, 2)
        .expect("2×2 RGB reduction geometry must be supported");
    for output_x in [0usize, 8] {
        let vector_block =
            super::native_reduce_3byte_2x2_vector_block(&source, &geometry, output_x, 0)
                .expect("complete eight-pixel blocks must use the specialized kernel");
        for lane in 0..super::SIMD_REDUCE_LANES {
            let (sums, _, multiplier, amend) =
                super::native_reduce_pixel_sums(&source, &geometry, 3, false, output_x + lane, 0)
                    .expect("scalar reference pixel must be in bounds");
            for channel in 0..3 {
                assert_eq!(
                    vector_block[lane * 3 + channel],
                    super::native_reduce_average(sums[channel], multiplier, amend),
                    "output ({}, {}) channel {channel}",
                    output_x + lane,
                    0
                );
            }
        }
    }
    assert!(super::native_reduce_3byte_2x2_vector_block(&source, &geometry, 16, 0).is_none());

    let (actual, output_width, output_height, vector_blocks, scalar_tail) =
        super::native_reduce_bytes(&image, 3, false, 2, 2, false)
            .expect("native RGB reduction must succeed");
    assert_eq!((output_width, output_height), (17, 3));
    assert_eq!((vector_blocks, scalar_tail), (6, 3));
    let mut expected = Vec::with_capacity(actual.len());
    for y in 0..output_height as usize {
        for x in 0..output_width as usize {
            let (sums, _, multiplier, amend) =
                super::native_reduce_pixel_sums(&source, &geometry, 3, false, x, y)
                    .expect("scalar reference pixel must be in bounds");
            for channel in 0..3 {
                expected.push(super::native_reduce_average(
                    sums[channel],
                    multiplier,
                    amend,
                ));
            }
        }
    }
    assert_eq!(actual, expected);
}
