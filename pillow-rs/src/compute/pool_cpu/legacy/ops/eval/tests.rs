#[test]
fn native_rgb_eval_affine_luts_are_exact_and_non_affine_tables_fall_back() {
    let coefficients = [(3u8, 5u8), (5, 13), (255, 255)];
    let lut = coefficients
        .iter()
        .flat_map(|&(multiplier, offset)| {
            (0..=u8::MAX).map(move |sample| sample.wrapping_mul(multiplier).wrapping_add(offset))
        })
        .collect::<Vec<_>>();
    let source = (0..768usize)
        .map(|index| index.wrapping_mul(73).wrapping_add(index / 7) as u8)
        .collect::<Vec<_>>();

    let mut variants = vec![lut.clone()];
    let mut non_affine = lut;
    non_affine[512 + 173] ^= 1;
    variants.push(non_affine);

    for variant in variants {
        let expected = source
            .iter()
            .enumerate()
            .map(|(index, &sample)| variant[(index % 3) * 256 + usize::from(sample)])
            .collect::<Vec<_>>();
        let actual = super::map_native_byte_lut::<3>(&source, &variant).expect("native RGB LUT");
        assert_eq!(actual, expected);
    }
}
