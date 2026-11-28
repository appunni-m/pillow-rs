//! AVX-512F 2x2 reduction for exact four-byte pixel modes.
// TODO(pillow-rs#3ptb): verify AVX-512F output and strategy receipts on native
// hardware for complete blocks and the declared lower-strategy fallbacks.

#![allow(unsafe_code)] // Mode strategies select this leaf only after AVX-512F admission.

use crate::checked_dims::CheckedDims;
use crate::compute::registry::support::{OperationId, StrategyGuard, StrategySelection};
use crate::compute::{Architecture, Backend, ExecutionContext};
use crate::error::PilError;
use crate::raster::{Mode, RgbaImage};

#[cfg(target_arch = "x86")]
use std::arch::x86 as arch;
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64 as arch;

const LANES: usize = 16;
const EVEN: [i32; LANES] = [0, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 22, 24, 26, 28, 30];
const ODD: [i32; LANES] = [1, 3, 5, 7, 9, 11, 13, 15, 17, 19, 21, 23, 25, 27, 29, 31];

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn execute<const STRAIGHT_ALPHA: bool>(
    context: &ExecutionContext,
    selection: &StrategySelection,
    mode: Mode,
    input: &RgbaImage,
    x_factor: u32,
    y_factor: u32,
) -> Result<RgbaImage, PilError> {
    let implementation = selection.implementation;
    if context.backend() != Backend::Cpu
        || context.architecture() != Architecture::Avx512F
        || selection.requested_backend != context.backend()
        || selection.requested_architecture != context.architecture()
        || !context.permits_architecture(Architecture::Avx512F)
        || !context.supports_architecture(Architecture::Avx512F)
        || implementation.backend != Backend::Cpu
        || implementation.operation != OperationId::Reduce
        || implementation.mode != mode
        || implementation.architecture != Architecture::Avx512F
        || selection.guard != (StrategyGuard::ReduceAvx512Byte2x2 { channels: 4 })
        || STRAIGHT_ALPHA != matches!(mode, Mode::ImageRgba8)
        || x_factor != 2
        || y_factor != 2
        || !input.width().is_multiple_of(2)
        || !input.height().is_multiple_of(2)
        || !(input.width() / 2).is_multiple_of(LANES as u32)
    {
        return Err(PilError::InternalError(
            "AVX-512F byte Reduce received an ineligible strategy request".into(),
        ));
    }

    let output_width = input.width() / 2;
    let output_height = input.height() / 2;
    let source_dims = CheckedDims::new(input.width(), input.height(), 4)?;
    let output_dims = CheckedDims::new(output_width, output_height, 4)?;
    if input.as_raw().len() != source_dims.total_bytes() || output_dims.total_bytes() == 0 {
        return Err(PilError::InternalError(
            "AVX-512F byte Reduce received invalid four-byte storage".into(),
        ));
    }

    let source_width = usize::try_from(input.width())
        .map_err(|_| PilError::InternalError("AVX-512F Reduce width overflow".into()))?;
    let mut output = output_dims.alloc_buffer();
    for output_y in 0..output_height as usize {
        let top = output_y * 2 * source_width * 4;
        let bottom = top + source_width * 4;
        for output_x in (0..output_width as usize).step_by(LANES) {
            let source_x = output_x * 2 * 4;
            let top_row = unsafe { input.as_raw().as_ptr().add(top + source_x) };
            let bottom_row = unsafe { input.as_raw().as_ptr().add(bottom + source_x) };
            let destination = unsafe {
                output
                    .as_mut_ptr()
                    .add((output_y * output_width as usize + output_x) * 4)
            };
            // SAFETY: exact storage and complete 16-pixel output rows were checked above; each
            // source row has 32 readable four-byte pixels and the output has 64 writable bytes.
            unsafe { reduce_block::<STRAIGHT_ALPHA>(top_row, bottom_row, destination) };
        }
    }

    crate::compute::record_pipeline_operation_path("avx512f");
    crate::compute::record_pipeline_operation_vector_blocks(
        u64::from(output_width / LANES as u32) * u64::from(output_height),
    );
    crate::compute::record_pipeline_operation_scalar_tail(0);
    RgbaImage::from_raw(output_width, output_height, output)
        .ok_or_else(|| PilError::InternalError("AVX-512F Reduce output shape mismatch".into()))
}

/// # Safety
/// Row pointers must span 128 readable bytes each; output must span 64 writable bytes;
/// the caller must have admitted AVX-512F and the exact 2x2 strategy.
// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
#[target_feature(enable = "avx512f")]
unsafe fn reduce_block<const STRAIGHT_ALPHA: bool>(
    top: *const u8,
    bottom: *const u8,
    output: *mut u8,
) {
    // SAFETY: the caller provides the complete rows and output block; all gathers use indices 0..31.
    unsafe {
        let even = arch::_mm512_loadu_si512(EVEN.as_ptr().cast());
        let odd = arch::_mm512_loadu_si512(ODD.as_ptr().cast());
        let top_left = arch::_mm512_i32gather_epi32::<4>(even, top.cast());
        let top_right = arch::_mm512_i32gather_epi32::<4>(odd, top.cast());
        let bottom_left = arch::_mm512_i32gather_epi32::<4>(even, bottom.cast());
        let bottom_right = arch::_mm512_i32gather_epi32::<4>(odd, bottom.cast());
        let samples = [top_left, top_right, bottom_left, bottom_right];
        let alpha = [
            channel::<24>(samples[0]),
            channel::<24>(samples[1]),
            channel::<24>(samples[2]),
            channel::<24>(samples[3]),
        ];
        let red = if STRAIGHT_ALPHA {
            average_premultiplied::<0>(samples, alpha)
        } else {
            average_channel::<0>(samples)
        };
        let green = if STRAIGHT_ALPHA {
            average_premultiplied::<8>(samples, alpha)
        } else {
            average_channel::<8>(samples)
        };
        let blue = if STRAIGHT_ALPHA {
            average_premultiplied::<16>(samples, alpha)
        } else {
            average_channel::<16>(samples)
        };
        let alpha = average_channel::<24>(samples);

        if STRAIGHT_ALPHA {
            write_unpremultiplied(red, green, blue, alpha, output);
        } else {
            let packed = arch::_mm512_or_si512(
                arch::_mm512_or_si512(red, arch::_mm512_slli_epi32::<8>(green)),
                arch::_mm512_or_si512(
                    arch::_mm512_slli_epi32::<16>(blue),
                    arch::_mm512_slli_epi32::<24>(alpha),
                ),
            );
            arch::_mm512_storeu_si512(output.cast(), packed);
        }
    }
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
#[target_feature(enable = "avx512f")]
unsafe fn channel<const SHIFT: u32>(sample: arch::__m512i) -> arch::__m512i {
    // SAFETY: the caller enters through an AVX-512F target-feature function.
    {
        arch::_mm512_and_si512(
            arch::_mm512_srli_epi32::<SHIFT>(sample),
            arch::_mm512_set1_epi32(0xff),
        )
    }
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
#[target_feature(enable = "avx512f")]
unsafe fn average_channel<const SHIFT: u32>(samples: [arch::__m512i; 4]) -> arch::__m512i {
    // SAFETY: the caller enters through an AVX-512F target-feature function.
    unsafe {
        let first_pair =
            arch::_mm512_add_epi32(channel::<SHIFT>(samples[0]), channel::<SHIFT>(samples[1]));
        let second_pair =
            arch::_mm512_add_epi32(channel::<SHIFT>(samples[2]), channel::<SHIFT>(samples[3]));
        let sum = arch::_mm512_add_epi32(first_pair, second_pair);
        arch::_mm512_srli_epi32::<2>(arch::_mm512_add_epi32(sum, arch::_mm512_set1_epi32(2)))
    }
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
#[target_feature(enable = "avx512f")]
unsafe fn average_premultiplied<const SHIFT: u32>(
    samples: [arch::__m512i; 4],
    alpha: [arch::__m512i; 4],
) -> arch::__m512i {
    // SAFETY: the caller enters through an AVX-512F target-feature function.
    unsafe {
        let premultiplied = [
            premultiply_channel::<SHIFT>(samples[0], alpha[0]),
            premultiply_channel::<SHIFT>(samples[1], alpha[1]),
            premultiply_channel::<SHIFT>(samples[2], alpha[2]),
            premultiply_channel::<SHIFT>(samples[3], alpha[3]),
        ];
        let sum = arch::_mm512_add_epi32(
            arch::_mm512_add_epi32(premultiplied[0], premultiplied[1]),
            arch::_mm512_add_epi32(premultiplied[2], premultiplied[3]),
        );
        arch::_mm512_srli_epi32::<2>(arch::_mm512_add_epi32(sum, arch::_mm512_set1_epi32(2)))
    }
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
#[target_feature(enable = "avx512f")]
unsafe fn premultiply_channel<const SHIFT: u32>(
    sample: arch::__m512i,
    alpha: arch::__m512i,
) -> arch::__m512i {
    // SAFETY: each lane is a byte value; product plus rounding bias is at most 65,152,
    // so the exact divide-by-255 reciprocal identity fits in unsigned 16-bit range.
    {
        let color = arch::_mm512_and_si512(
            arch::_mm512_srli_epi32::<SHIFT>(sample),
            arch::_mm512_set1_epi32(0xff),
        );
        let product = arch::_mm512_add_epi32(
            arch::_mm512_mullo_epi32(color, alpha),
            arch::_mm512_set1_epi32(127),
        );
        let quotient = arch::_mm512_add_epi32(
            arch::_mm512_add_epi32(product, arch::_mm512_set1_epi32(1)),
            arch::_mm512_srli_epi32::<8>(product),
        );
        arch::_mm512_srli_epi32::<8>(quotient)
    }
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
#[target_feature(enable = "avx512f")]
unsafe fn write_unpremultiplied(
    red: arch::__m512i,
    green: arch::__m512i,
    blue: arch::__m512i,
    alpha: arch::__m512i,
    output: *mut u8,
) {
    let mut red_values = [0i32; LANES];
    let mut green_values = [0i32; LANES];
    let mut blue_values = [0i32; LANES];
    let mut alpha_values = [0i32; LANES];
    // SAFETY: each local array has sixteen initialized i32 slots and AVX-512F is enabled here.
    unsafe {
        arch::_mm512_storeu_si512(red_values.as_mut_ptr().cast(), red);
        arch::_mm512_storeu_si512(green_values.as_mut_ptr().cast(), green);
        arch::_mm512_storeu_si512(blue_values.as_mut_ptr().cast(), blue);
        arch::_mm512_storeu_si512(alpha_values.as_mut_ptr().cast(), alpha);
        for lane in 0..LANES {
            let a = alpha_values[lane];
            let alpha_byte = {
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "the 2x2 alpha average is bounded to one byte"
                )]
                {
                    a as u8
                }
            };
            let rgba = [
                unpremultiply(red_values[lane], a),
                unpremultiply(green_values[lane], a),
                unpremultiply(blue_values[lane], a),
                alpha_byte,
            ];
            output.add(lane * 4).cast::<[u8; 4]>().write_unaligned(rgba);
        }
    }
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
#[inline]
fn unpremultiply(sample: i32, alpha: i32) -> u8 {
    if alpha == 0 {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "the averaged premultiplied sample is bounded to one byte"
        )]
        {
            sample as u8
        }
    } else {
        #[expect(
            clippy::arithmetic_side_effects,
            reason = "the averaged color is at most 255, so the product fits i32"
        )]
        let value = sample * 255 / alpha;
        #[expect(
            clippy::cast_possible_truncation,
            reason = "unpremultiplication produces at most 255 for valid averaged RGBA"
        )]
        {
            value as u8
        }
    }
}
