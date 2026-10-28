use super::float_resize;
use super::{PortableFma, f_resize_vertical_pass, precompute_coeffs_f64, resample_kernel};
use crate::pipeline::ResampleFilter;

#[test]
fn packed_vertical_output_matches_float_materialization() {
    fn compare<const FINITE_OPERANDS: bool>(source: &[f32]) {
        let destination_width = 3;
        let (kernel, support) = resample_kernel(&ResampleFilter::Bicubic);
        let coefficients = precompute_coeffs_f64(3, 5, kernel, support);
        let fma = PortableFma;
        let mut float_output = vec![0.0f32; destination_width * 3];
        f_resize_vertical_pass::<FINITE_OPERANDS, _>(
            source,
            &mut float_output,
            destination_width,
            3,
            &coefficients,
            &fma,
        );
        let mut packed_output = vec![[0u8; 4]; destination_width * 3];
        float_resize::vertical_pass_packed::<FINITE_OPERANDS, _>(
            source,
            &mut packed_output,
            destination_width,
            &coefficients,
            &fma,
        );
        let expected = float_output
            .into_iter()
            .flat_map(f32::to_le_bytes)
            .collect::<Vec<_>>();
        assert_eq!(packed_output.into_flattened(), expected);
    }

    let finite = [
        -0.0, 0.25, -0.5, 1.0, 2.5, 3.0, -4.0, 5.5, 6.0, 7.0, -8.25, 9.0, 10.0, 11.5, 12.0,
    ];
    compare::<true>(&finite);

    let nonfinite = [
        f32::INFINITY,
        -f32::INFINITY,
        f32::NAN,
        1.0,
        -0.0,
        2.0,
        3.0,
        4.0,
        -5.0,
        6.0,
        7.0,
        8.0,
        f32::from_bits(0x7fc0_1234),
        10.0,
        11.0,
    ];
    compare::<false>(&nonfinite);
}
