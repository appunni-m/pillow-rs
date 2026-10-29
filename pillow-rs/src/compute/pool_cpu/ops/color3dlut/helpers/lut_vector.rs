//! Eight-lane Color3DLut interpolation shared by concrete mode strategies.
//!
//! `wide::i32x8` uses the compile-time target's SIMD representation. Mode
//! owners expose this helper only from matching architecture modules, so the
//! injected architecture selects the implementation before entering the loop.

use crate::error::PilError;
use crate::raster::{DynamicImage, GenericImageView, Mode};
use wide::i32x8;

const PRECISION_BITS: i32 = 6;

struct Color3DLutKernel<'a> {
    source: &'a [u8],
    source_channels: usize,
    target_channels: usize,
    prepared: &'a [i16],
    size: (usize, usize, usize),
    scales: [u32; 3],
    channels: usize,
    source_mode: Mode,
}

#[inline]
fn interpolate_vector(a: i32x8, b: i32x8, shift: i32x8) -> i32x8 {
    const SHIFT_BITS: u32 = 15;
    let one = i32x8::splat(1i32 << SHIFT_BITS);
    (a * (one - shift) + b * shift) >> SHIFT_BITS
}

#[inline]
fn table_indices(
    source: &[u8],
    first_pixel: usize,
    source_channels: usize,
    scales: [u32; 3],
    size: (usize, usize),
    channels: usize,
) -> ([usize; 8], [i32; 8], [i32; 8], [i32; 8]) {
    let (sx, sy) = size;
    let sxy = sx * sy;
    let mut bases = [0usize; 8];
    let mut shift_x = [0i32; 8];
    let mut shift_y = [0i32; 8];
    let mut shift_z = [0i32; 8];
    for lane in 0..8 {
        let source_offset = (first_pixel + lane) * source_channels;
        let indices = [
            u32::from(source[source_offset]) * scales[0],
            u32::from(source[source_offset + 1]) * scales[1],
            u32::from(source[source_offset + 2]) * scales[2],
        ];
        bases[lane] = super::lut::table_index_3d(
            (indices[0] >> super::lut::SCALE_BITS) as usize,
            (indices[1] >> super::lut::SCALE_BITS) as usize,
            (indices[2] >> super::lut::SCALE_BITS) as usize,
            sx,
            sxy,
        ) * channels;
        shift_x[lane] = ((super::lut::SCALE_MASK & indices[0])
            >> (super::lut::SCALE_BITS - super::lut::SHIFT_BITS)) as i32;
        shift_y[lane] = ((super::lut::SCALE_MASK & indices[1])
            >> (super::lut::SCALE_BITS - super::lut::SHIFT_BITS)) as i32;
        shift_z[lane] = ((super::lut::SCALE_MASK & indices[2])
            >> (super::lut::SCALE_BITS - super::lut::SHIFT_BITS)) as i32;
    }
    (bases, shift_x, shift_y, shift_z)
}

fn interpolate_channel(
    prepared: &[i16],
    bases: [usize; 8],
    sx: usize,
    sxy: usize,
    channels: usize,
    channel: usize,
    shifts: [i32x8; 3],
) -> [i32; 8] {
    let left_left = interpolate_vector(
        i32x8::new(std::array::from_fn(|lane| prepared[bases[lane] + channel] as i32)),
        i32x8::new(std::array::from_fn(|lane| {
            prepared[bases[lane] + channels + channel] as i32
        })),
        shifts[0],
    );
    let left_right = interpolate_vector(
        i32x8::new(std::array::from_fn(|lane| {
            prepared[bases[lane] + sx * channels + channel] as i32
        })),
        i32x8::new(std::array::from_fn(|lane| {
            prepared[bases[lane] + sx * channels + channels + channel] as i32
        })),
        shifts[0],
    );
    let left = interpolate_vector(left_left, left_right, shifts[1]);
    let right_left = interpolate_vector(
        i32x8::new(std::array::from_fn(|lane| {
            prepared[bases[lane] + sxy * channels + channel] as i32
        })),
        i32x8::new(std::array::from_fn(|lane| {
            prepared[bases[lane] + sxy * channels + channels + channel] as i32
        })),
        shifts[0],
    );
    let right_right = interpolate_vector(
        i32x8::new(std::array::from_fn(|lane| {
            prepared[bases[lane] + sxy * channels + sx * channels + channel] as i32
        })),
        i32x8::new(std::array::from_fn(|lane| {
            prepared[bases[lane] + sxy * channels + sx * channels + channels + channel] as i32
        })),
        shifts[0],
    );
    interpolate_vector(
        left,
        interpolate_vector(right_left, right_right, shifts[1]),
        shifts[2],
    )
    .to_array()
}

impl Color3DLutKernel<'_> {
    fn write_vector_batch(&self, output: &mut [u8], first_pixel: usize) {
        let (sx, sy, _) = self.size;
        let sxy = sx * sy;
        let (bases, x, y, z) = table_indices(
            self.source,
            first_pixel,
            self.source_channels,
            self.scales,
            (sx, sy),
            self.channels,
        );
        let shifts = [i32x8::new(x), i32x8::new(y), i32x8::new(z)];
        for channel in 0..self.channels {
            let values = interpolate_channel(
                self.prepared,
                bases,
                sx,
                sxy,
                self.channels,
                channel,
                shifts,
            );
            for (lane, value) in values.into_iter().enumerate() {
                let output_offset = (first_pixel + lane) * self.target_channels;
                output[output_offset + channel] = ((value + (1 << (PRECISION_BITS - 1)))
                    >> PRECISION_BITS)
                    .clamp(0, 255) as u8;
            }
        }
        if self.channels == 3 && self.target_channels == 4 {
            for lane in 0..8 {
                let input_offset = (first_pixel + lane) * self.source_channels;
                let output_offset = (first_pixel + lane) * self.target_channels;
                output[output_offset + 3] = if self.source_channels == 4 {
                    self.source[input_offset + 3]
                } else {
                    255
                };
            }
        }
    }

    fn write_scalar_tail(&self, output: &mut [u8], start: usize, end: usize) {
        for pixel_index in start..end {
            let source_offset = pixel_index * self.source_channels;
            let pixel = [
                self.source[source_offset],
                self.source[source_offset + 1],
                self.source[source_offset + 2],
                if self.source_channels == 4 { self.source[source_offset + 3] } else { 255 },
            ];
            let output_offset = pixel_index * self.target_channels;
            super::lut::write_pixel(
                pixel,
                self.source_mode,
                self.prepared,
                self.size,
                self.scales,
                self.channels,
                self.target_channels,
                &mut output[output_offset..output_offset + self.target_channels],
            );
        }
    }
}

fn source_channels(mode: Mode) -> Result<usize, PilError> {
    match mode {
        Mode::ImageRgb8 | Mode::ImageYCbCr8 | Mode::ImageLab8 | Mode::ImageHsv8 => Ok(3),
        Mode::ImageRgba8 | Mode::ImageCmyk8 => Ok(4),
        _ => Err(PilError::InternalError(
            "validated Color3DLut source mode was not RGB, RGBA, or CMYK".into(),
        )),
    }
}

fn target_channels(mode: Mode) -> Result<usize, PilError> {
    match mode {
        Mode::ImageRgb8 => Ok(3),
        Mode::ImageRgba8 | Mode::ImageCmyk8 => Ok(4),
        _ => Err(PilError::InternalError(
            "validated Color3DLut target mode was not RGB, RGBA, or CMYK".into(),
        )),
    }
}

/// Applies the vector interpolation path selected by the mode's architecture owner.
pub(crate) fn apply(
    image: &DynamicImage,
    source_mode: Mode,
    size: (u32, u32, u32),
    table: &[f64],
    channels: u32,
    target_mode: Mode,
) -> Result<DynamicImage, PilError> {
    if image.mode() != source_mode {
        return Err(PilError::InternalError(
            "prepared Color3DLut source mode changed before execution".into(),
        ));
    }
    let source_channels = source_channels(source_mode)?;
    let target_channels = target_channels(target_mode)?;
    let (width, height) = image.dimensions();
    let pixels = (width as usize)
        .checked_mul(height as usize)
        .ok_or_else(|| PilError::ValueError("SIMD Color3DLut pixel count overflow".into()))?;
    let output_len = pixels
        .checked_mul(target_channels)
        .ok_or_else(|| PilError::ValueError("SIMD Color3DLut output size overflow".into()))?;
    let size = (size.0 as usize, size.1 as usize, size.2 as usize);
    let channels = channels as usize;
    let prepared = super::lut::prepare_table(table);
    let scales = super::lut::color_scales(size);
    let source = image.as_bytes();
    let mut output = vec![0u8; output_len];
    let kernel = Color3DLutKernel {
        source,
        source_channels,
        target_channels,
        prepared: &prepared,
        size,
        scales,
        channels,
        source_mode,
    };
    let vector_end = pixels - pixels % 8;
    for pixel in (0..vector_end).step_by(8) {
        kernel.write_vector_batch(&mut output, pixel);
    }
    kernel.write_scalar_tail(&mut output, vector_end, pixels);
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks((vector_end / 8) as u64);
    crate::compute::record_pipeline_operation_scalar_tail((pixels - vector_end) as u64);
    super::lut::image_from_native_bytes(width, height, output, target_mode)
}

