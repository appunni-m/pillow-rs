//! AVX-512F 2x2 reduction strategy for exact ImageF32 storage.

#![allow(unsafe_code)] // Prepared dispatch validates AVX-512F before entering the leaf.

use crate::compute::pool_cpu::ops::reduce::f32::helpers::storage;
use crate::error::PilError;
use crate::raster::{DynamicImage, RgbaImage};

#[cfg(target_arch = "x86")]
use std::arch::x86 as arch;
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64 as arch;

const LANES: usize = 16;
const EVEN_INDICES: [i32; LANES] = [0, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 22, 24, 26, 28, 30];
const ODD_INDICES: [i32; LANES] = [1, 3, 5, 7, 9, 11, 13, 15, 17, 19, 21, 23, 25, 27, 29, 31];

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn execute(
    input: &RgbaImage,
    x_factor: u32,
    y_factor: u32,
) -> Result<DynamicImage, PilError> {
    let output_width = input.width().div_ceil(x_factor.max(1));
    let output_height = input.height().div_ceil(y_factor.max(1));
    if x_factor != 2
        || y_factor != 2
        || !input.width().is_multiple_of(2)
        || !input.height().is_multiple_of(2)
        || !(output_width as usize).is_multiple_of(LANES)
    {
        return super::super::default::reduce_impl::execute(input, x_factor, y_factor);
    }

    let source = storage::source_samples(input)?;
    let source_width = usize::try_from(input.width())
        .map_err(|_| PilError::InternalError("ImageF32 AVX-512F width overflow".into()))?;
    let mut output = Vec::with_capacity(storage::output_capacity(output_width, output_height)?);
    for output_y in 0..output_height as usize {
        let source_top = output_y * 2 * source_width;
        let source_bottom = source_top + source_width;
        for output_x in (0..output_width as usize).step_by(LANES) {
            let source_x = output_x * 2;
            // SAFETY: the prepared ReduceF32Vector guard validates a 2x2
            // reduction with complete 16-pixel output blocks, and dispatch
            // requires the captured host to support AVX-512F.
            let block = unsafe {
                reduce_row(
                    source.as_ptr().add(source_top + source_x),
                    source.as_ptr().add(source_bottom + source_x),
                )
            };
            storage::append_samples(&mut output, &block);
        }
    }
    storage::finish_output(output_width, output_height, output)
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
#[target_feature(enable = "avx512f")]
unsafe fn reduce_row(top: *const f32, bottom: *const f32) -> [f32; LANES] {
    // SAFETY: both row pointers address 32 source samples for this block.
    let even = unsafe { arch::_mm512_loadu_si512(EVEN_INDICES.as_ptr().cast()) };
    let odd = unsafe { arch::_mm512_loadu_si512(ODD_INDICES.as_ptr().cast()) };
    let top_first = unsafe { arch::_mm512_loadu_ps(top) };
    let top_second = unsafe { arch::_mm512_loadu_ps(top.add(LANES)) };
    let bottom_first = unsafe { arch::_mm512_loadu_ps(bottom) };
    let bottom_second = unsafe { arch::_mm512_loadu_ps(bottom.add(LANES)) };
    let top_left = arch::_mm512_permutex2var_ps(top_first, even, top_second);
    let top_right = arch::_mm512_permutex2var_ps(top_first, odd, top_second);
    let bottom_left = arch::_mm512_permutex2var_ps(bottom_first, even, bottom_second);
    let bottom_right = arch::_mm512_permutex2var_ps(bottom_first, odd, bottom_second);
    let sum = arch::_mm512_add_ps(
        arch::_mm512_add_ps(arch::_mm512_add_ps(top_left, top_right), bottom_left),
        bottom_right,
    );
    let mut quartets = [0.0; LANES];
    unsafe { arch::_mm512_storeu_ps(quartets.as_mut_ptr(), sum) };
    quartets.map(super::super::helpers::vector::average_quartet)
}
