//! Native-byte alpha arithmetic for Transform's LA and RGBA filtering paths.

pub(crate) fn premultiply(source: &[u8], channels: usize) -> Vec<u8> {
    let mut output = source.to_vec();
    let alpha_index = channels - 1;
    for (source_pixel, output_pixel) in source
        .chunks_exact(channels)
        .zip(output.chunks_exact_mut(channels))
    {
        let alpha = source_pixel[alpha_index];
        for channel in 0..alpha_index {
            output_pixel[channel] = ((u16::from(source_pixel[channel]) * u16::from(alpha) + 127)
                / 255) as u8;
        }
    }
    output
}

pub(crate) fn premultiply_into(source: &[u8], channels: usize, output: &mut Vec<u8>) {
    output.clear();
    output.extend_from_slice(source);
    let alpha_index = channels - 1;
    for (source_pixel, output_pixel) in source
        .chunks_exact(channels)
        .zip(output.chunks_exact_mut(channels))
    {
        let alpha = source_pixel[alpha_index];
        for channel in 0..alpha_index {
            output_pixel[channel] = ((u16::from(source_pixel[channel]) * u16::from(alpha) + 127)
                / 255) as u8;
        }
    }
}

pub(crate) fn unpremultiply(output: &mut [u8], channels: usize) {
    let alpha_index = channels - 1;
    for pixel in output.chunks_exact_mut(channels) {
        let alpha = u16::from(pixel[alpha_index]);
        if alpha == 0 {
            continue;
        }
        for channel in 0..alpha_index {
            pixel[channel] = (u16::from(pixel[channel]) * 255 / alpha).min(255) as u8;
        }
    }
}

pub(crate) fn unpremultiply_region(
    output: &mut [u8],
    channels: usize,
    dimensions: (u32, u32),
    box_coordinates: (i64, i64, i64, i64),
) {
    let (width, height) = dimensions;
    let (left, top, right, bottom) = box_coordinates;
    let left = left.clamp(0, i64::from(width)) as usize;
    let top = top.clamp(0, i64::from(height)) as usize;
    let right = right.clamp(0, i64::from(width)) as usize;
    let bottom = bottom.clamp(0, i64::from(height)) as usize;
    if left >= right || top >= bottom || channels < 2 {
        return;
    }
    let row_bytes = width as usize * channels;
    for y in top..bottom {
        let start = y * row_bytes + left * channels;
        let end = y * row_bytes + right * channels;
        unpremultiply(&mut output[start..end], channels);
    }
}
