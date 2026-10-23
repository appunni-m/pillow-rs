use super::composite_l_row;
#[cfg(not(feature = "parallel"))]
use super::interleave_rgb_luma_bands;

#[test]
fn native_l_composite_rows_keep_exact_blend_rounding_and_tails() {
    for length in [0usize, 1, 15, 16, 17, 31, 32, 33, 257] {
        let source = (0..length)
            .map(|index| (index.wrapping_mul(73).wrapping_add(19)) as u8)
            .collect::<Vec<_>>();
        let mut destination = (0..length)
            .map(|index| (index.wrapping_mul(151).wrapping_add(211)) as u8)
            .collect::<Vec<_>>();
        let mask = (0..length)
            .map(|index| match index % 7 {
                0 => 0,
                1 => 255,
                _ => (index.wrapping_mul(97).wrapping_add(43)) as u8,
            })
            .collect::<Vec<_>>();
        let expected = source
            .iter()
            .zip(destination.iter())
            .zip(mask.iter())
            .map(|((&source, &destination), &mask)| {
                ((u32::from(source) * u32::from(mask)
                    + u32::from(destination) * u32::from(255 - mask)
                    + 127)
                    / 255) as u8
            })
            .collect::<Vec<_>>();

        composite_l_row(&source, &mut destination, &mask);
        assert_eq!(destination, expected, "row length {length}");
    }
}

#[cfg(not(feature = "parallel"))]
#[test]
fn native_rgb_luma_merge_interleave_writes_exact_samples() {
    for length in [0usize, 1, 2, 3, 4, 5, 7, 8, 9, 15, 16, 17, 31, 32, 33] {
        let red = (0..length).map(|i| (i * 7 + 1) as u8).collect::<Vec<_>>();
        let green = (0..length).map(|i| (i * 11 + 3) as u8).collect::<Vec<_>>();
        let blue = (0..length).map(|i| (i * 13 + 5) as u8).collect::<Vec<_>>();
        let expected = (0..length)
            .flat_map(|i| [red[i], green[i], blue[i]])
            .collect::<Vec<_>>();

        let actual = interleave_rgb_luma_bands(&red, &green, &blue)
            .expect("equal L bands interleave as native RGB bytes");
        assert_eq!(actual, expected, "pixel count {length}");
    }
}
