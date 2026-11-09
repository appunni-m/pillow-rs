//! Exact sample-copy nearest resampling for native typed buffers.

use crate::raster::{ImageBuffer, Pixel};

pub(crate) type BoxBounds = Option<(f64, f64, f64, f64)>;

pub(crate) fn resize<P: Pixel>(
    input: &ImageBuffer<P, Vec<P::Subpixel>>,
    destination_width: u32,
    destination_height: u32,
    bounds: BoxBounds,
) -> ImageBuffer<P, Vec<P::Subpixel>> {
    let mut output = ImageBuffer::new(destination_width, destination_height);
    let (source_width, source_height) = input.dimensions();
    if source_width == 0 || source_height == 0 || destination_width == 0 || destination_height == 0
    {
        return output;
    }
    let channels = usize::from(P::CHANNEL_COUNT);
    let source_samples: &[P::Subpixel] = input.as_raw();
    let output_samples: &mut [P::Subpixel] = &mut output;

    let (left, top, scale_x, scale_y) = match bounds {
        Some((left, top, right, bottom)) => {
            let left = left as f32;
            let top = top as f32;
            let right = right as f32;
            let bottom = bottom as f32;
            (
                f64::from(left),
                f64::from(top),
                f64::from(right - left) / f64::from(destination_width),
                f64::from(bottom - top) / f64::from(destination_height),
            )
        }
        None => (
            0.0,
            0.0,
            f64::from(source_width) / f64::from(destination_width),
            f64::from(source_height) / f64::from(destination_height),
        ),
    };

    let mut x_table = Vec::with_capacity(destination_width as usize);
    let mut source_x = left + scale_x * 0.5;
    for _ in 0..destination_width {
        let coordinate = source_x as u32;
        x_table.push(if coordinate >= source_width {
            source_width - 1
        } else {
            coordinate
        });
        source_x += scale_x;
    }

    let mut source_y = top + scale_y * 0.5;
    for destination_y in 0..destination_height {
        let coordinate = if source_y >= f64::from(source_height) {
            source_height - 1
        } else {
            source_y as u32
        };
        for (destination_x, &source_x) in x_table.iter().enumerate() {
            let source_start =
                ((coordinate as usize * source_width as usize) + source_x as usize) * channels;
            let destination_start =
                ((destination_y as usize * destination_width as usize) + destination_x) * channels;
            output_samples[destination_start..destination_start + channels]
                .copy_from_slice(&source_samples[source_start..source_start + channels]);
        }
        source_y += scale_y;
    }
    output
}
