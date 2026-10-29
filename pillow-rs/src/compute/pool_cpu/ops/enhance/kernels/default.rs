//! Default serial kernels for native enhancement samples.

use crate::image::Mode;

pub(in crate::compute::pool_cpu::ops::enhance) fn scale_bytes(
    bytes: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    active_channels: usize,
    factor: f64,
) -> bool {
    let Some(expected) = width.checked_mul(height).and_then(|n| n.checked_mul(channels)) else {
        return false;
    };
    if channels == 0 || active_channels > channels || bytes.len() != expected {
        return false;
    }
    for pixel in bytes.chunks_exact_mut(channels) {
        for sample in &mut pixel[..active_channels] {
            *sample = (*sample as f64 * factor).clamp(0.0, 255.0) as u8;
        }
    }
    true
}

pub(in crate::compute::pool_cpu::ops::enhance) fn scale_luma_bytes(
    bytes: &mut [u8],
    width: usize,
    height: usize,
    factor: f64,
) -> bool {
    let Some(expected) = width.checked_mul(height) else {
        return false;
    };
    if bytes.len() != expected {
        return false;
    }
    let shift = match factor {
        0.5 => Some(1),
        0.25 => Some(2),
        0.125 => Some(3),
        _ => None,
    };
    if let Some(shift) = shift {
        for sample in bytes {
            *sample >>= shift;
        }
    } else {
        for sample in bytes {
            *sample = (*sample as f64 * factor).clamp(0.0, 255.0) as u8;
        }
    }
    true
}

pub(in crate::compute::pool_cpu::ops::enhance) fn scale_u16_bytes(
    bytes: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    active_channels: usize,
    factor: f64,
) -> bool {
    let Some(expected) = width
        .checked_mul(height)
        .and_then(|n| n.checked_mul(channels))
        .and_then(|n| n.checked_mul(2))
    else {
        return false;
    };
    if channels == 0 || active_channels > channels || bytes.len() != expected {
        return false;
    }
    for pixel in bytes.chunks_exact_mut(channels * 2) {
        for sample in pixel[..active_channels * 2].chunks_exact_mut(2) {
            let value = u16::from_ne_bytes([sample[0], sample[1]]);
            let scaled = (f64::from(value) * factor).clamp(0.0, f64::from(u16::MAX)) as u16;
            sample.copy_from_slice(&scaled.to_ne_bytes());
        }
    }
    true
}

pub(in crate::compute::pool_cpu::ops::enhance) fn scale_f32_bytes(
    bytes: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    active_channels: usize,
    factor: f32,
) -> bool {
    let Some(expected) = width
        .checked_mul(height)
        .and_then(|n| n.checked_mul(channels))
        .and_then(|n| n.checked_mul(4))
    else {
        return false;
    };
    if channels == 0 || active_channels > channels || bytes.len() != expected {
        return false;
    }
    for pixel in bytes.chunks_exact_mut(channels * 4) {
        for sample in pixel[..active_channels * 4].chunks_exact_mut(4) {
            let value = f32::from_ne_bytes([sample[0], sample[1], sample[2], sample[3]]) * factor;
            sample.copy_from_slice(&value.to_ne_bytes());
        }
    }
    true
}

pub(in crate::compute::pool_cpu::ops::enhance) fn scale_i32_bytes(bytes: &mut [u8], factor: f64) -> bool {
    if bytes.len() % 4 != 0 {
        return false;
    }
    for sample in bytes.chunks_exact_mut(4) {
        let value = i32::from_ne_bytes([sample[0], sample[1], sample[2], sample[3]]);
        let scaled = (f64::from(value) * factor)
            .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32;
        sample.copy_from_slice(&scaled.to_ne_bytes());
    }
    true
}

pub(in crate::compute::pool_cpu::ops::enhance) fn blend_color_bytes(
    source: &[u8],
    output: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    mode: Mode,
    factor: f32,
) -> bool {
    let Some(expected) = width.checked_mul(height).and_then(|n| n.checked_mul(channels)) else {
        return false;
    };
    if !(1..=4).contains(&channels) || source.len() != expected || output.len() != expected {
        return false;
    }
    if mode == Mode::ImageRgba8 && factor == 0.0 {
        for (source_pixel, output_pixel) in source
            .chunks_exact(4)
            .zip(output.chunks_exact_mut(4))
        {
            let gray = crate::color::rgb_to_luma_u8(source_pixel[0], source_pixel[1], source_pixel[2]);
            output_pixel[..3].fill(gray);
            output_pixel[3] = source_pixel[3];
        }
        return true;
    }
    for (source_pixel, output_pixel) in source
        .chunks_exact(channels)
        .zip(output.chunks_exact_mut(channels))
    {
        let base = crate::ops::enhance::color_pixel_base_for_mode(source_pixel, mode);
        for (index, (source, destination)) in source_pixel.iter().zip(output_pixel).enumerate() {
            let base = f32::from(base[index]);
            *destination = factor
                .mul_add(f32::from(*source) - base, base)
                .clamp(0.0, 255.0) as u8;
        }
    }
    true
}

pub(in crate::compute::pool_cpu::ops::enhance) fn blend_color_u16_bytes(
    source: &[u8],
    output: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    factor: f32,
) -> bool {
    let Some(expected) = width
        .checked_mul(height)
        .and_then(|n| n.checked_mul(channels))
        .and_then(|n| n.checked_mul(2))
    else {
        return false;
    };
    if !(1..=4).contains(&channels) || source.len() != expected || output.len() != expected {
        return false;
    }
    for (source_pixel, output_pixel) in source
        .chunks_exact(channels * 2)
        .zip(output.chunks_exact_mut(channels * 2))
    {
        let values = read_u16_pixel(source_pixel);
        let gray = if channels >= 3 {
            rgb_to_luma_u16(values[0], values[1], values[2])
        } else {
            values[0]
        };
        let base = [
            gray,
            if channels == 2 { values[1] } else { gray },
            gray,
            if channels == 4 { values[3] } else { 0 },
        ];
        for (index, destination) in output_pixel.chunks_exact_mut(2).enumerate() {
            let value = values[index];
            let base = f32::from(base[index]);
            let blended = factor
                .mul_add(f32::from(value) - base, base)
                .clamp(0.0, f32::from(u16::MAX)) as u16;
            destination.copy_from_slice(&blended.to_ne_bytes());
        }
    }
    true
}

pub(in crate::compute::pool_cpu::ops::enhance) fn blend_color_i32_bytes(
    source: &[u8],
    output: &mut [u8],
    factor: f32,
) -> bool {
    if source.len() % 4 != 0 || source.len() != output.len() {
        return false;
    }
    for (source_sample, output_sample) in source
        .chunks_exact(4)
        .zip(output.chunks_exact_mut(4))
    {
        let value = read_i32(source_sample, 0);
        let blended = (factor as f64)
            .mul_add(0.0, f64::from(value))
            .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32;
        output_sample.copy_from_slice(&blended.to_ne_bytes());
    }
    true
}

pub(in crate::compute::pool_cpu::ops::enhance) fn blend_color_f32_bytes(
    source: &[u8],
    output: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    factor: f32,
) -> bool {
    let Some(expected) = width
        .checked_mul(height)
        .and_then(|n| n.checked_mul(channels))
        .and_then(|n| n.checked_mul(4))
    else {
        return false;
    };
    if !(1..=4).contains(&channels) || source.len() != expected || output.len() != expected {
        return false;
    }
    for (source_pixel, output_pixel) in source
        .chunks_exact(channels * 4)
        .zip(output.chunks_exact_mut(channels * 4))
    {
        let values = read_f32_pixel(source_pixel);
        let gray = if channels >= 3 {
            values[0].mul_add(0.299, values[1].mul_add(0.587, values[2] * 0.114))
        } else {
            values[0]
        };
        let base = [
            gray,
            if channels == 2 { values[1] } else { gray },
            gray,
            if channels == 4 { values[3] } else { 0.0 },
        ];
        for (index, destination) in output_pixel.chunks_exact_mut(4).enumerate() {
            let value = values[index];
            let blended = factor.mul_add(value - base[index], base[index]);
            destination.copy_from_slice(&blended.to_ne_bytes());
        }
    }
    true
}

pub(in crate::compute::pool_cpu::ops::enhance) fn sharpness_bytes(
    source: &[u8],
    width: usize,
    height: usize,
    channels: usize,
    active_channels: usize,
    factor: f32,
) -> Option<Vec<u8>> {
    let row_stride = width.checked_mul(channels)?;
    let expected_len = row_stride.checked_mul(height)?;
    if channels == 0 || active_channels == 0 || active_channels > channels || source.len() != expected_len {
        return None;
    }
    if width == 0 || height == 0 {
        return Some(source.to_vec());
    }

    let mut output = source.to_vec();
    for (y, row) in output.chunks_exact_mut(row_stride).enumerate() {
        for (x, pixel) in row.chunks_exact_mut(channels).enumerate() {
            let pixel_start = y * row_stride + x * channels;
            for (channel, destination) in pixel[..active_channels].iter_mut().enumerate() {
                let index = pixel_start + channel;
                let original = source[index];
                let smooth = smooth_byte(source, width, height, channels, x, y, channel, original);
                *destination = factor
                    .mul_add(f32::from(original) - f32::from(smooth), f32::from(smooth))
                    .clamp(0.0, 255.0) as u8;
            }
        }
    }
    Some(output)
}

pub(in crate::compute::pool_cpu::ops::enhance) fn sharpness_u16_bytes(
    source: &[u8],
    width: usize,
    height: usize,
    channels: usize,
    active_channels: usize,
    factor: f32,
) -> Option<Vec<u8>> {
    let row_stride = width.checked_mul(channels)?.checked_mul(2)?;
    let expected = row_stride.checked_mul(height)?;
    if channels == 0
        || active_channels == 0
        || active_channels > channels
        || source.len() != expected
    {
        return None;
    }
    let mut output = source.to_vec();
    for y in 0..height {
        for x in 0..width {
            for channel in 0..active_channels {
                let index = (y * width * channels + x * channels + channel) * 2;
                let original = read_u16(source, index);
                let smooth = smooth_u16(source, width, height, channels, x, y, channel);
                let blended = factor
                    .mul_add(f32::from(original) - f32::from(smooth), f32::from(smooth))
                    .clamp(0.0, f32::from(u16::MAX)) as u16;
                output[index..index + 2].copy_from_slice(&blended.to_ne_bytes());
            }
        }
    }
    Some(output)
}

pub(in crate::compute::pool_cpu::ops::enhance) fn sharpness_f32_bytes(
    source: &[u8],
    width: usize,
    height: usize,
    channels: usize,
    active_channels: usize,
    factor: f32,
) -> Option<Vec<u8>> {
    let row_stride = width.checked_mul(channels)?.checked_mul(4)?;
    let expected = row_stride.checked_mul(height)?;
    if channels == 0
        || active_channels == 0
        || active_channels > channels
        || source.len() != expected
    {
        return None;
    }
    let mut output = source.to_vec();
    for y in 0..height {
        for x in 0..width {
            for channel in 0..active_channels {
                let index = (y * width * channels + x * channels + channel) * 4;
                let original = read_f32(source, index);
                let smooth = smooth_f32(source, width, height, channels, x, y, channel);
                let blended = factor.mul_add(original - smooth, smooth);
                output[index..index + 4].copy_from_slice(&blended.to_ne_bytes());
            }
        }
    }
    Some(output)
}

pub(in crate::compute::pool_cpu::ops::enhance) fn sharpness_i32_bytes(
    source: &[u8],
    width: usize,
    height: usize,
    factor: f32,
) -> Option<Vec<u8>> {
    let expected = width.checked_mul(height)?.checked_mul(4)?;
    if source.len() != expected {
        return None;
    }
    let mut output = source.to_vec();
    for y in 0..height {
        for x in 0..width {
            let index = (y * width + x) * 4;
            let original = read_i32(source, index);
            let smooth = smooth_i32(source, width, height, x, y, original);
            let blended = (factor as f64)
                .mul_add(f64::from(original) - f64::from(smooth), f64::from(smooth))
                .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32;
            output[index..index + 4].copy_from_slice(&blended.to_ne_bytes());
        }
    }
    Some(output)
}

fn smooth_u16(
    source: &[u8],
    width: usize,
    height: usize,
    channels: usize,
    x: usize,
    y: usize,
    channel: usize,
) -> u16 {
    let sample = |x, y| read_u16(source, ((y * width + x) * channels + channel) * 2);
    let value = if x == 0 || x + 1 >= width || y == 0 || y + 1 >= height {
        sample(x, y)
    } else {
        let weighted = u64::from(sample(x - 1, y - 1))
            + u64::from(sample(x, y - 1))
            + u64::from(sample(x + 1, y - 1))
            + u64::from(sample(x - 1, y))
            + u64::from(sample(x, y)) * 5
            + u64::from(sample(x + 1, y))
            + u64::from(sample(x - 1, y + 1))
            + u64::from(sample(x, y + 1))
            + u64::from(sample(x + 1, y + 1));
        ((weighted + 6) / 13) as u16
    };
    value
}

fn smooth_f32(
    source: &[u8],
    width: usize,
    height: usize,
    channels: usize,
    x: usize,
    y: usize,
    channel: usize,
) -> f32 {
    let sample = |x, y| read_f32(source, ((y * width + x) * channels + channel) * 4);
    let value = if x == 0 || x + 1 >= width || y == 0 || y + 1 >= height {
        sample(x, y)
    } else {
        let weighted = sample(x - 1, y - 1)
            + sample(x, y - 1)
            + sample(x + 1, y - 1)
            + sample(x - 1, y)
            + sample(x, y) * 5.0
            + sample(x + 1, y)
            + sample(x - 1, y + 1)
            + sample(x, y + 1)
            + sample(x + 1, y + 1);
        weighted / 13.0
    };
    value
}

fn smooth_i32(source: &[u8], width: usize, height: usize, x: usize, y: usize, original: i32) -> i32 {
    if x == 0 || x + 1 >= width || y == 0 || y + 1 >= height {
        return original;
    }
    let sample = |x, y| i64::from(read_i32(source, (y * width + x) * 4));
    let weighted = sample(x - 1, y - 1)
        + sample(x, y - 1)
        + sample(x + 1, y - 1)
        + sample(x - 1, y)
        + sample(x, y) * 5
        + sample(x + 1, y)
        + sample(x - 1, y + 1)
        + sample(x, y + 1)
        + sample(x + 1, y + 1);
    (weighted / 13).clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

fn read_u16(bytes: &[u8], index: usize) -> u16 {
    u16::from_ne_bytes([bytes[index], bytes[index + 1]])
}

fn read_i32(bytes: &[u8], index: usize) -> i32 {
    i32::from_ne_bytes([bytes[index], bytes[index + 1], bytes[index + 2], bytes[index + 3]])
}

fn read_f32(bytes: &[u8], index: usize) -> f32 {
    f32::from_ne_bytes([bytes[index], bytes[index + 1], bytes[index + 2], bytes[index + 3]])
}

fn read_u16_pixel(bytes: &[u8]) -> [u16; 4] {
    let mut values = [0; 4];
    for (index, sample) in bytes.chunks_exact(2).enumerate() {
        values[index] = u16::from_ne_bytes([sample[0], sample[1]]);
    }
    values
}

fn read_f32_pixel(bytes: &[u8]) -> [f32; 4] {
    let mut values = [0.0; 4];
    for (index, sample) in bytes.chunks_exact(4).enumerate() {
        values[index] = f32::from_ne_bytes([sample[0], sample[1], sample[2], sample[3]]);
    }
    values
}

fn rgb_to_luma_u16(red: u16, green: u16, blue: u16) -> u16 {
    ((u64::from(red) * 19_595 + u64::from(green) * 38_470 + u64::from(blue) * 7_471 + 32_768)
        >> 16) as u16
}

fn smooth_byte(
    source: &[u8],
    width: usize,
    height: usize,
    channels: usize,
    x: usize,
    y: usize,
    channel: usize,
    original: u8,
) -> u8 {
    if x == 0 || x + 1 >= width || y == 0 || y + 1 >= height {
        return original;
    }
    let row_stride = width * channels;
    let above = (y - 1) * row_stride;
    let current = y * row_stride;
    let below = (y + 1) * row_stride;
    let left = (x - 1) * channels + channel;
    let center = x * channels + channel;
    let right = (x + 1) * channels + channel;
    let weighted = u16::from(source[below + left])
        + u16::from(source[below + center])
        + u16::from(source[below + right])
        + u16::from(source[current + left])
        + u16::from(source[current + center]) * 5
        + u16::from(source[current + right])
        + u16::from(source[above + left])
        + u16::from(source[above + center])
        + u16::from(source[above + right]);
    ((weighted + 6) / 13) as u8
}
