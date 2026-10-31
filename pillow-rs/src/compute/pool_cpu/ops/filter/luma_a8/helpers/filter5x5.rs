use crate::compute::helpers::filters::{
    binomial5x5_parameters_match, la_single_outlier5x5_parameters_match,
};

/// Execute archived packed-LA vector kernels for the selected architecture.
pub(crate) fn execute_special(
    raw: &[u8],
    width: u32,
    height: u32,
    kernel: &[f32; 25],
    scale: f32,
    offset: f32,
) -> Option<Vec<u8>> {
    let dimensions = (width as usize, height as usize);
    let single_outlier = la_single_outlier5x5_parameters_match(kernel, scale, offset);
    let binomial = binomial5x5_parameters_match(kernel, scale, offset);
    if !single_outlier && !binomial {
        return None;
    }
    let mut output = raw.to_vec();
    let result = if single_outlier {
        super::single_outlier5x5_kernel::filter_5x5_single_outlier_la_rows(
            raw,
            &mut output,
            dimensions.0,
            dimensions.1,
        )
    } else {
        super::binomial5x5_kernel::filter_5x5_binomial_la_rows(
            raw,
            &mut output,
            dimensions.0,
            dimensions.1,
        )
    };
    let (vector_blocks, scalar_tail) = result?;
    crate::compute::record_pipeline_operation_path("vector");
    if vector_blocks != 0 {
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    }
    if scalar_tail != 0 {
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    }
    Some(output)
}
