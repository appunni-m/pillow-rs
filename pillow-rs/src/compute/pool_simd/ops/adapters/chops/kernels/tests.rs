use super::{native_chops_lut_vector, simd_div127};

#[test]
fn div127_matches_every_selected_blend_product() {
    for start in (0..=32385u16).step_by(16) {
        let values = std::array::from_fn(|lane| (start + lane as u16).min(32385));
        let actual = simd_div127(wide::u16x16::new(values)).to_array();
        assert_eq!(actual, values.map(|value| value / 127));
    }
}

#[test]
fn overlay_and_hardlight_vectors_match_all_byte_pairs() {
    let tables: [(bool, &[u8; 65536]); 2] = [
        (
            false,
            include_bytes!("../../../../../../ops/lut_overlay.bin"),
        ),
        (
            true,
            include_bytes!("../../../../../../ops/lut_hardlight.bin"),
        ),
    ];
    for (hard_light, table) in tables {
        for start in (0..65536usize).step_by(16) {
            let left = std::array::from_fn(|lane| ((start + lane) >> 8) as u8);
            let right = std::array::from_fn(|lane| (start + lane) as u8);
            assert_eq!(
                native_chops_lut_vector(left, right, hard_light),
                table[start..start + 16],
                "hard_light={hard_light} pair_start={start}"
            );
        }
    }
}
