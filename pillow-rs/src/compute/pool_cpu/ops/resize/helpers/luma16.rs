//! Pillow-compatible resampling arithmetic for native `I;16*` buffers.

use crate::compute::helpers::resampling::luma16_samples::{read, write};
use crate::ops::pil_resize::FilterCoeffsF64;
use crate::pipeline::ResampleFilter;
use crate::raster::{ImageBuffer, Luma};

// VERIFY: source=reviewed@2026-11-22; parity=pass@migration-parity-75cc2aeb1bde4900b01f7ce223621407; benchmark=pending
pub(crate) fn resize<const BIG_ENDIAN: bool>(
    input: &ImageBuffer<Luma<u16>, Vec<u16>>,
    destination_width: u32,
    destination_height: u32,
    filter: ResampleFilter,
    bounds: super::typed_nearest::BoxBounds,
) -> ImageBuffer<Luma<u16>, Vec<u16>> {
    if matches!(filter, ResampleFilter::Nearest) {
        return super::typed_nearest::resize(input, destination_width, destination_height, bounds);
    }

    let source_size = input.dimensions();
    let destination_size = (destination_width, destination_height);
    let (need_horizontal, need_vertical) =
        super::coefficients::resample_pass_requirements(source_size, destination_size, bounds);
    if !need_horizontal && !need_vertical {
        return input.clone();
    }

    let (horizontal, vertical) =
        super::coefficients::for_resize(source_size, destination_size, filter, bounds);
    if need_horizontal && !need_vertical {
        return resize_horizontal_only::<BIG_ENDIAN>(
            input,
            destination_width,
            destination_height,
            &horizontal,
        );
    }
    if !need_horizontal && need_vertical {
        return resize_vertical_only::<BIG_ENDIAN>(
            input,
            destination_width,
            destination_height,
            &vertical,
        );
    }
    if vertical_first(source_size, destination_size) {
        return resize_vertical_first::<BIG_ENDIAN>(
            input,
            destination_width,
            destination_height,
            &horizontal,
            &vertical,
        );
    }

    let mut intermediate =
        ImageBuffer::<Luma<u16>, Vec<u16>>::new(destination_width, input.height());
    for source_y in 0..input.height() {
        let source_start = source_y as usize * input.width() as usize;
        let intermediate_start = source_y as usize * destination_width as usize;
        horizontal_pass::<BIG_ENDIAN>(
            &input.as_raw()[source_start..source_start + input.width() as usize],
            &horizontal,
            destination_width,
            &mut intermediate.as_mut()
                [intermediate_start..intermediate_start + destination_width as usize],
        );
    }

    let mut output = ImageBuffer::<Luma<u16>, Vec<u16>>::new(destination_width, destination_height);
    for y in 0..destination_height {
        for x in 0..destination_width {
            let value = vertical_sample::<BIG_ENDIAN>(
                intermediate.as_raw(),
                x,
                destination_width,
                &vertical,
                y as usize,
            );
            output.put_pixel(x, y, Luma([value]));
        }
    }
    output
}

// VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
fn vertical_first(source: (u32, u32), destination: (u32, u32)) -> bool {
    source.0 != destination.0
        && source.1 != destination.1
        && !super::coefficients::horizontal_first(source, destination)
}

// VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
fn resize_horizontal_only<const BIG_ENDIAN: bool>(
    input: &ImageBuffer<Luma<u16>, Vec<u16>>,
    destination_width: u32,
    destination_height: u32,
    horizontal: &FilterCoeffsF64,
) -> ImageBuffer<Luma<u16>, Vec<u16>> {
    let source_width = input.width() as usize;
    let destination_width_usize = destination_width as usize;
    let mut output = ImageBuffer::<Luma<u16>, Vec<u16>>::new(destination_width, destination_height);
    for y in 0..destination_height as usize {
        let source_start = y * source_width;
        let destination_start = y * destination_width_usize;
        horizontal_pass::<BIG_ENDIAN>(
            &input.as_raw()[source_start..source_start + source_width],
            horizontal,
            destination_width,
            &mut output.as_mut()[destination_start..destination_start + destination_width_usize],
        );
    }
    output
}

// VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
fn resize_vertical_only<const BIG_ENDIAN: bool>(
    input: &ImageBuffer<Luma<u16>, Vec<u16>>,
    destination_width: u32,
    destination_height: u32,
    vertical: &FilterCoeffsF64,
) -> ImageBuffer<Luma<u16>, Vec<u16>> {
    let mut output = ImageBuffer::<Luma<u16>, Vec<u16>>::new(destination_width, destination_height);
    for y in 0..destination_height {
        for x in 0..destination_width {
            let value = vertical_sample::<BIG_ENDIAN>(
                input.as_raw(),
                x,
                input.width(),
                vertical,
                y as usize,
            );
            output.put_pixel(x, y, Luma([value]));
        }
    }
    output
}

// VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
fn resize_vertical_first<const BIG_ENDIAN: bool>(
    input: &ImageBuffer<Luma<u16>, Vec<u16>>,
    destination_width: u32,
    destination_height: u32,
    horizontal: &FilterCoeffsF64,
    vertical: &FilterCoeffsF64,
) -> ImageBuffer<Luma<u16>, Vec<u16>> {
    let source_width = input.width() as usize;
    let destination_height_usize = destination_height as usize;
    let mut intermediate = vec![0u16; source_width * destination_height_usize];
    for y in 0..destination_height_usize {
        let first = vertical.xmin[y];
        let count = vertical.count[y];
        for x in 0..source_width {
            let mut accumulator = 0.0f64;
            for (tap, &weight) in vertical.weights[y].iter().enumerate() {
                let source_y = (first + tap as i64) as usize;
                accumulator = weight.mul_add(
                    f64::from(read::<BIG_ENDIAN>(
                        input.as_raw()[source_y * source_width + x],
                    )),
                    accumulator,
                );
            }
            if count != 0 {
                intermediate[y * source_width + x] = write::<BIG_ENDIAN>(accumulator);
            }
        }
    }

    let mut output = ImageBuffer::<Luma<u16>, Vec<u16>>::new(destination_width, destination_height);
    let destination_width_usize = destination_width as usize;
    for y in 0..destination_height_usize {
        let source_start = y * source_width;
        let destination_start = y * destination_width_usize;
        horizontal_pass::<BIG_ENDIAN>(
            &intermediate[source_start..source_start + source_width],
            horizontal,
            destination_width,
            &mut output.as_mut()[destination_start..destination_start + destination_width_usize],
        );
    }
    output
}

// VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
fn horizontal_pass<const BIG_ENDIAN: bool>(
    source_row: &[u16],
    coefficients: &FilterCoeffsF64,
    output_width: u32,
    output_row: &mut [u16],
) {
    for (x, output) in output_row
        .iter_mut()
        .enumerate()
        .take(output_width as usize)
    {
        let first = coefficients.xmin[x];
        let count = coefficients.count[x];
        if count == 0 {
            continue;
        }
        let mut accumulator = 0.0f64;
        for (tap, &weight) in coefficients.weights[x].iter().enumerate() {
            let source_x = (first + tap as i64) as usize;
            accumulator = weight.mul_add(
                f64::from(read::<BIG_ENDIAN>(source_row[source_x])),
                accumulator,
            );
        }
        *output = write::<BIG_ENDIAN>(accumulator);
    }
}

// VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
fn vertical_sample<const BIG_ENDIAN: bool>(
    intermediate: &[u16],
    output_x: u32,
    output_width: u32,
    coefficients: &FilterCoeffsF64,
    output_y: usize,
) -> u16 {
    let first = coefficients.xmin[output_y];
    let count = coefficients.count[output_y];
    if count == 0 {
        return 0;
    }
    let mut accumulator = 0.0f64;
    for (tap, &weight) in coefficients.weights[output_y].iter().enumerate() {
        let source_y = (first + tap as i64) as usize;
        let sample = intermediate[source_y * output_width as usize + output_x as usize];
        accumulator = weight.mul_add(f64::from(read::<BIG_ENDIAN>(sample)), accumulator);
    }
    write::<BIG_ENDIAN>(accumulator)
}
