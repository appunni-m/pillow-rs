//! Native LA-mode 5x5 filter strategy.

use crate::compute::pool_cpu::ops::filter::helpers;

pub(super) fn execute(
    raw: &[u8],
    width: usize,
    height: usize,
    channels: usize,
    kernel: &[f32; 25],
    scale: f32,
    offset: f32,
) -> Vec<u8> {
    let normalized = helpers::normalized_filter5x5_kernel(kernel, scale);
    let rounding_bias = offset + 0.5;
    let mut output = raw.to_vec();

    if helpers::la_single_outlier5x5_parameters_match(kernel, scale, offset)
        && helpers::filter_5x5_la_single_outlier_rows(raw, &mut output, width, height)
    {
        return output;
    }

    if helpers::is_small_uniform_filter5x5_bytes(raw, width, height, channels) {
        helpers::filter_5x5_constant_bytes(
            raw,
            &mut output,
            width,
            height,
            channels,
            &normalized,
            rounding_bias,
        );
        return output;
    }

    if helpers::binomial5x5_parameters_match(kernel, scale, offset)
        && helpers::filter_5x5_binomial_la_rows(raw, &mut output, width, height)
    {
        return output;
    }

    helpers::apply_filter5x5_native_rows(
        raw,
        &mut output,
        width,
        height,
        channels,
        &normalized,
        rounding_bias,
    );
    output
}
