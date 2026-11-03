//! Pillow-compatible resampling arithmetic for native `I;16*` buffers.

use crate::compute::helpers::resampling::luma16_samples::{read, write};
use crate::ops::pil_resize::FilterCoeffsF64;
use crate::pipeline::ResampleFilter;
use crate::raster::{ImageBuffer, Luma};

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

    let (horizontal, vertical) = super::coefficients::for_resize(
        input.dimensions(),
        (destination_width, destination_height),
        filter,
        bounds,
    );
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

fn horizontal_pass<const BIG_ENDIAN: bool>(
    source_row: &[u16],
    coefficients: &FilterCoeffsF64,
    output_width: u32,
    output_row: &mut [u16],
) {
    for (x, output) in output_row.iter_mut().enumerate().take(output_width as usize) {
        let first = coefficients.xmin[x];
        let count = coefficients.count[x];
        if count == 0 {
            continue;
        }
        let mut accumulator = 0.0f64;
        for (tap, &weight) in coefficients.weights[x].iter().enumerate() {
            let source_x = (first + tap as i64) as usize;
            accumulator += f64::from(read::<BIG_ENDIAN>(source_row[source_x])) * weight;
        }
        *output = write::<BIG_ENDIAN>(accumulator);
    }
}

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
        accumulator += f64::from(read::<BIG_ENDIAN>(sample)) * weight;
    }
    write::<BIG_ENDIAN>(accumulator)
}
