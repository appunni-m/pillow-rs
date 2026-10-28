//! Mode-neutral CPU reduction kernel with alpha handling injected by the family router.

use super::super::ReduceAlphaLayout;
use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::image_utils::raw_bytes_to_image;
use crate::raster::DynamicImage;

#[derive(Clone, Copy)]
struct ReduceRounding {
    multiplier: u64,
    amend: u32,
}

struct ReduceRowContext<'a> {
    raw: &'a [u8],
    width: u32,
    output_width: u32,
    x_factor: u32,
    y_factor: u32,
    channels: usize,
    main_width: u32,
    main_height: u32,
    right_width: u32,
    bottom_height: u32,
    full: ReduceRounding,
    right: ReduceRounding,
    bottom: ReduceRounding,
    corner: ReduceRounding,
    premultiplied_alpha: bool,
    native_rgba_reduce: bool,
}

#[inline(always)]
fn process_reduce_row(context: &ReduceRowContext<'_>, y: u32, row: &mut [u8]) {
    let full_y = y < context.main_height;
    let y_count = if full_y {
        context.y_factor
    } else {
        context.bottom_height
    };
    let source_y = if full_y {
        y * context.y_factor
    } else {
        context.main_height * context.y_factor
    };
    if context.native_rgba_reduce {
        process_reduce_rgba_row(context, full_y, y_count, source_y, row);
    } else {
        process_reduce_interleaved_row(context, full_y, y_count, source_y, row);
    }
}

#[inline(always)]
fn reduce_rounding_for_block(
    context: &ReduceRowContext<'_>,
    full_x: bool,
    full_y: bool,
) -> ReduceRounding {
    match (full_x, full_y) {
        (true, true) => context.full,
        (false, true) => context.right,
        (true, false) => context.bottom,
        (false, false) => context.corner,
    }
}

#[inline(always)]
fn process_reduce_rgba_row(
    context: &ReduceRowContext<'_>,
    full_y: bool,
    y_count: u32,
    source_y: u32,
    row: &mut [u8],
) {
    for x in 0..context.output_width {
        let full_x = x < context.main_width;
        let x_count = if full_x {
            context.x_factor
        } else {
            context.right_width
        };
        let source_x = if full_x {
            x * context.x_factor
        } else {
            context.main_width * context.x_factor
        };
        let rounding = reduce_rounding_for_block(context, full_x, full_y);
        let dst_idx = x as usize * context.channels;
        let sums = reduce_rgba_premultiplied_block_sums(
            context.raw,
            context.width as usize,
            source_x as usize,
            source_y as usize,
            x_count as usize,
            y_count as usize,
        );
        let average =
            |sum: u64| (((sum + u64::from(rounding.amend)) * rounding.multiplier) >> 24) as u8;
        let alpha = average(sums[3]);
        let mut red = average(sums[0]);
        let mut green = average(sums[1]);
        let mut blue = average(sums[2]);
        if alpha != 0 {
            red = (u16::from(red) * 255 / u16::from(alpha)) as u8;
            green = (u16::from(green) * 255 / u16::from(alpha)) as u8;
            blue = (u16::from(blue) * 255 / u16::from(alpha)) as u8;
        }
        row[dst_idx] = red;
        row[dst_idx + 1] = green;
        row[dst_idx + 2] = blue;
        row[dst_idx + 3] = alpha;
    }
}

#[inline(always)]
fn process_reduce_interleaved_row(
    context: &ReduceRowContext<'_>,
    full_y: bool,
    y_count: u32,
    source_y: u32,
    row: &mut [u8],
) {
    for x in 0..context.output_width {
        let full_x = x < context.main_width;
        let x_count = if full_x {
            context.x_factor
        } else {
            context.right_width
        };
        let source_x = if full_x {
            x * context.x_factor
        } else {
            context.main_width * context.x_factor
        };
        let rounding = reduce_rounding_for_block(context, full_x, full_y);
        let dst_idx = x as usize * context.channels;
        let mut sums = [0u64; 4];
        for dy in 0..y_count {
            for dx in 0..x_count {
                let src_idx =
                    ((source_y + dy) * context.width + source_x + dx) as usize * context.channels;
                for c in 0..context.channels {
                    let sample = if context.premultiplied_alpha && c + 1 < context.channels {
                        ((u16::from(context.raw[src_idx + c])
                            * u16::from(context.raw[src_idx + context.channels - 1])
                            + 127)
                            / 255) as u8
                    } else {
                        context.raw[src_idx + c]
                    };
                    sums[c] += u64::from(sample);
                }
            }
        }
        for c in 0..context.channels {
            let mut value =
                (((sums[c] + u64::from(rounding.amend)) * rounding.multiplier) >> 24) as u8;
            if context.premultiplied_alpha && c + 1 < context.channels {
                let alpha = (((sums[context.channels - 1] + u64::from(rounding.amend))
                    * rounding.multiplier)
                    >> 24) as u8;
                if alpha != 0 {
                    value = (u16::from(value) * 255 / u16::from(alpha)) as u8;
                }
            }
            row[dst_idx + c] = value;
        }
    }
}

#[inline(always)]
fn reduce_division_multiplier(divider: u32) -> u64 {
    // division_UINT32(divider, 8): 2^32 / (256 * divider), truncated.
    ((1u128 << 32) / (u128::from(divider) * 256)) as u64
}

/// Sum an RGBA reduction block after Pillow's per-input premultiplication.
#[inline]
fn reduce_rgba_premultiplied_block_sums(
    source: &[u8],
    width: usize,
    source_x: usize,
    source_y: usize,
    block_width: usize,
    block_height: usize,
) -> [u64; 4] {
    let mut sums = [0u64; 4];
    for dy in 0..block_height {
        let mut source_index = ((source_y + dy) * width + source_x) * 4;
        for _ in 0..block_width {
            let alpha = u32::from(source[source_index + 3]);
            let red = u32::from(source[source_index]);
            let green = u32::from(source[source_index + 1]);
            let blue = u32::from(source[source_index + 2]);
            sums[0] += u64::from((red * alpha + 127) / 255);
            sums[1] += u64::from((green * alpha + 127) / 255);
            sums[2] += u64::from((blue * alpha + 127) / 255);
            sums[3] += u64::from(alpha);
            source_index += 4;
        }
    }
    sums
}

pub(in crate::compute::pool_cpu::ops::reduce) fn execute_reduce_default(
    img: &DynamicImage,
    x_factor: u32,
    y_factor: u32,
    alpha_layout: ReduceAlphaLayout,
) -> Result<DynamicImage, PilError> {
    let fx = x_factor.max(1);
    let fy = y_factor.max(1);
    let (w, h) = (img.width(), img.height());
    let channels = img.color().channel_count() as usize;
    let new_w = w.div_ceil(fx);
    let new_h = h.div_ceil(fy);
    let raw = img.as_bytes();
    let mut out = CheckedDims::new(new_w, new_h, channels as u8)?.alloc_buffer();
    if new_w == 0 || new_h == 0 {
        return raw_bytes_to_image(new_w, new_h, out, channels);
    }

    // Every reduced output row reads immutable source pixels and owns a
    // disjoint destination slice. Keep the partial right/bottom blocks in the
    // same inlined row function as the full blocks so both CPU lanes share one
    // exact rounding order.
    let main_w = w / fx;
    let main_h = h / fy;
    let right_width = w % fx;
    let bottom_height = h % fy;
    let full_divider = fx * fy;
    let full = ReduceRounding {
        multiplier: reduce_division_multiplier(full_divider),
        amend: full_divider / 2,
    };
    let right_divider = right_width * fy;
    let right = ReduceRounding {
        multiplier: reduce_division_multiplier(right_divider.max(1)),
        amend: right_divider / 2,
    };
    let bottom_divider = fx * bottom_height;
    let bottom = ReduceRounding {
        multiplier: reduce_division_multiplier(bottom_divider.max(1)),
        amend: bottom_divider / 2,
    };
    let corner_divider = right_width * bottom_height;
    let corner = ReduceRounding {
        multiplier: reduce_division_multiplier(corner_divider.max(1)),
        amend: corner_divider / 2,
    };
    let row_context = ReduceRowContext {
        raw,
        width: w,
        output_width: new_w,
        x_factor: fx,
        y_factor: fy,
        channels,
        main_width: main_w,
        main_height: main_h,
        right_width,
        bottom_height,
        full,
        right,
        bottom,
        corner,
        premultiplied_alpha: alpha_layout.premultiplied_alpha,
        native_rgba_reduce: alpha_layout.native_rgba_reduce,
    };

    let output_stride = new_w as usize * channels;

    for y in 0..new_h {
        let start = y as usize * output_stride;
        process_reduce_row(&row_context, y, &mut out[start..start + output_stride]);
    }

    let result = raw_bytes_to_image(new_w, new_h, out, channels)?;
    Ok(result)
}
