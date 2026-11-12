use super::layout::{
    PackedColorFormat, RawByteOrder, RawDecoderTransform, RawScalarOutput, RawScalarType,
};

#[derive(Clone, Copy)]
struct ChannelBandFormat {
    source_bytes_per_sample: usize,
    source_offset: usize,
    output_lane: usize,
    output_lanes: usize,
    invert: bool,
}

pub(super) fn transform_row(
    transform: RawDecoderTransform,
    width: usize,
    input: &[u8],
    output: &mut [u8],
) {
    match transform {
        RawDecoderTransform::Native => output.copy_from_slice(input),
        RawDecoderTransform::ChannelOrder {
            source_lanes,
            source_lanes_per_pixel,
            output_lanes_per_pixel,
        } => channel_order(
            source_lanes,
            source_lanes_per_pixel,
            output_lanes_per_pixel,
            input,
            output,
        ),
        RawDecoderTransform::ChannelOrderWithPadding {
            source_lanes,
            source_bytes_per_pixel,
        } => channel_order_with_padding(source_lanes, source_bytes_per_pixel, input, output),
        RawDecoderTransform::InterleavedTwoBandLine
        | RawDecoderTransform::InterleavedThreeBandLine
        | RawDecoderTransform::InterleavedThreeBandLineWithPadding { .. }
        | RawDecoderTransform::InterleavedFourBandLine
        | RawDecoderTransform::RgbTriplets { .. } => {
            interleaved_row(transform, width, input, output);
        }
        _ => sample_transform_row(transform, width, input, output),
    }
}

fn interleaved_row(transform: RawDecoderTransform, width: usize, input: &[u8], output: &mut [u8]) {
    match transform {
        RawDecoderTransform::InterleavedTwoBandLine => interleave_two_bands(width, input, output),
        RawDecoderTransform::InterleavedThreeBandLine => {
            interleave_three_bands(width, input, output)
        }
        RawDecoderTransform::InterleavedThreeBandLineWithPadding { padding } => {
            interleave_three_bands_with_padding(width, padding, input, output);
        }
        RawDecoderTransform::InterleavedFourBandLine => {
            interleave_four_bands(width, input, output);
        }
        RawDecoderTransform::RgbTriplets { source_lanes } => {
            rgb_triplets(source_lanes, input, output);
        }
        _ => unreachable!("only interleaved transforms reach this dispatcher"),
    }
}

fn sample_transform_row(
    transform: RawDecoderTransform,
    width: usize,
    input: &[u8],
    output: &mut [u8],
) {
    match transform {
        RawDecoderTransform::LumaAlphaToRgba {
            source_bytes_per_pixel,
            luma_high_byte_offset,
            alpha_high_byte_offset,
        } => luma_alpha_to_rgba(
            source_bytes_per_pixel,
            luma_high_byte_offset,
            alpha_high_byte_offset,
            input,
            output,
        ),
        RawDecoderTransform::UnpremultiplyRgba {
            source_bytes_per_pixel,
            source_lanes,
            sample_bytes,
            high_byte_offset,
        } => unpremultiply_rgba(
            source_bytes_per_pixel,
            source_lanes,
            sample_bytes,
            high_byte_offset,
            input,
            output,
        ),
        RawDecoderTransform::InvertRgbKeepAlpha => invert_rgb_keep_alpha(input, output),
        RawDecoderTransform::CmykToRgb => cmyk_to_rgb(input, output),
        RawDecoderTransform::YccToRgb { output_lanes } => {
            ycc_to_rgb(input, output, output_lanes);
        }
        RawDecoderTransform::YccaToRgba => ycca_to_rgba(input, output),
        _ => storage_transform_row(transform, width, input, output),
    }
}

fn storage_transform_row(
    transform: RawDecoderTransform,
    width: usize,
    input: &[u8],
    output: &mut [u8],
) {
    match transform {
        RawDecoderTransform::PackBilevelBytes => pack_bilevel_bytes(width, input, output),
        RawDecoderTransform::PackedLuma {
            bits,
            reverse,
            invert,
        } => {
            packed_luma(width, bits, reverse, invert, input, output);
        }
        RawDecoderTransform::PackedPalette { bits, bit_planes } => {
            packed_palette(width, bits, bit_planes, input, output);
        }
        RawDecoderTransform::HighByte16 { offset } => high_byte_16(width, offset, input, output),
        RawDecoderTransform::HighBytes16 {
            source_lanes,
            source_bytes_per_pixel,
            output_lanes,
            high_byte_offset,
            padding,
        } => high_bytes_16(
            source_lanes,
            source_bytes_per_pixel,
            output_lanes,
            high_byte_offset,
            padding,
            input,
            output,
        ),
        RawDecoderTransform::PackedColor16 {
            format,
            output_lanes,
        } => packed_color_16(width, format, output_lanes, input, output),
        RawDecoderTransform::Unpack12To16 => unpack_12_to_16(width, input, output),
        RawDecoderTransform::ChannelBand {
            source_bytes_per_sample,
            source_offset,
            output_lane,
            output_lanes,
            invert,
        } => channel_band(
            width,
            ChannelBandFormat {
                source_bytes_per_sample,
                source_offset,
                output_lane,
                output_lanes,
                invert,
            },
            input,
            output,
        ),
        RawDecoderTransform::DecodeScalar {
            sample_bytes,
            byte_order,
            sample_type,
            output_type,
        } => decode_scalar_samples(
            width,
            sample_bytes,
            byte_order,
            sample_type,
            output_type,
            input,
            output,
        ),
        RawDecoderTransform::LabBias => lab_bias(input, output),
        RawDecoderTransform::SwapBytes16 => swap_bytes_16(input, output),
        RawDecoderTransform::InvertBytes => invert_bytes(input, output),
        RawDecoderTransform::ReverseBits => reverse_bits(input, output),
        RawDecoderTransform::InvertReverseBits => invert_reverse_bits(input, output),
        _ => unreachable!("non-interleaved transforms reach this dispatcher"),
    }
}

fn channel_order(
    source_lanes: [u8; 4],
    source_count: usize,
    output_count: usize,
    input: &[u8],
    output: &mut [u8],
) {
    for (source, destination) in input
        .chunks_exact(source_count)
        .zip(output.chunks_exact_mut(output_count))
    {
        for (lane, target) in destination.iter_mut().enumerate() {
            *target = source[usize::from(source_lanes[lane])];
        }
    }
}

fn channel_order_with_padding(
    source_lanes: [u8; 3],
    source_bytes_per_pixel: usize,
    input: &[u8],
    output: &mut [u8],
) {
    for (source, destination) in input
        .chunks_exact(source_bytes_per_pixel)
        .zip(output.chunks_exact_mut(4))
    {
        for (lane, target) in destination.iter_mut().take(3).enumerate() {
            *target = source[usize::from(source_lanes[lane])];
        }
        destination[3] = u8::MAX;
    }
}

fn interleave_two_bands(width: usize, input: &[u8], output: &mut [u8]) {
    for x in 0..width {
        output[x * 2] = input[x];
        output[x * 2 + 1] = input[width + x];
    }
}

fn interleave_three_bands(width: usize, input: &[u8], output: &mut [u8]) {
    for x in 0..width {
        output[x * 3] = input[x];
        output[x * 3 + 1] = input[width + x];
        output[x * 3 + 2] = input[width * 2 + x];
    }
}

fn interleave_three_bands_with_padding(width: usize, padding: u8, input: &[u8], output: &mut [u8]) {
    for x in 0..width {
        let destination = x * 4;
        output[destination] = input[x];
        output[destination + 1] = input[width + x];
        output[destination + 2] = input[width * 2 + x];
        output[destination + 3] = padding;
    }
}

fn interleave_four_bands(width: usize, input: &[u8], output: &mut [u8]) {
    for x in 0..width {
        let destination = x * 4;
        output[destination] = input[x];
        output[destination + 1] = input[width + x];
        output[destination + 2] = input[width * 2 + x];
        output[destination + 3] = input[width * 3 + x];
    }
}

fn rgb_triplets(source_lanes: [u8; 3], input: &[u8], output: &mut [u8]) {
    for (source, destination) in input.chunks_exact(3).zip(output.chunks_exact_mut(4)) {
        destination[0] = source[usize::from(source_lanes[0])];
        destination[1] = source[usize::from(source_lanes[1])];
        destination[2] = source[usize::from(source_lanes[2])];
        destination[3] = u8::MAX;
    }
}

fn luma_alpha_to_rgba(
    source_bytes_per_pixel: usize,
    luma_offset: usize,
    alpha_offset: usize,
    input: &[u8],
    output: &mut [u8],
) {
    for (source, destination) in input
        .chunks_exact(source_bytes_per_pixel)
        .zip(output.chunks_exact_mut(4))
    {
        let luma = source[luma_offset];
        destination[0] = luma;
        destination[1] = luma;
        destination[2] = luma;
        destination[3] = source[source_bytes_per_pixel / 2 + alpha_offset];
    }
}

fn unpremultiply_rgba(
    source_bytes_per_pixel: usize,
    source_lanes: [u8; 4],
    sample_bytes: usize,
    high_byte_offset: usize,
    input: &[u8],
    output: &mut [u8],
) {
    for (source, destination) in input
        .chunks_exact(source_bytes_per_pixel)
        .zip(output.chunks_exact_mut(4))
    {
        let component =
            |lane: usize| source[usize::from(source_lanes[lane]) * sample_bytes + high_byte_offset];
        let alpha = component(3);
        destination[3] = alpha;
        if alpha == 0 {
            destination[..3].fill(0);
            continue;
        }
        for lane in 0..3 {
            let value = component(lane);
            destination[lane] = (u16::from(value) * 255 / u16::from(alpha)).min(255) as u8;
        }
    }
}

fn invert_rgb_keep_alpha(input: &[u8], output: &mut [u8]) {
    for (source, destination) in input.chunks_exact(4).zip(output.chunks_exact_mut(4)) {
        destination[0] = !source[0];
        destination[1] = !source[1];
        destination[2] = !source[2];
        destination[3] = source[3];
    }
}

fn cmyk_to_rgb(input: &[u8], output: &mut [u8]) {
    for (source, destination) in input.chunks_exact(4).zip(output.chunks_exact_mut(3)) {
        let non_key = 255 - u32::from(source[3]);
        for channel in 0..3 {
            let ink = u32::from(source[channel]);
            destination[channel] = (non_key - mul_div_255(ink, non_key)) as u8;
        }
    }
}

fn ycc_to_rgb(input: &[u8], output: &mut [u8], output_lanes: usize) {
    for (source, destination) in input
        .chunks_exact(3)
        .zip(output.chunks_exact_mut(output_lanes))
    {
        let rgb = crate::color::photo_ycc_pixel_to_rgb(source[0], source[1], source[2]);
        destination[..3].copy_from_slice(&rgb);
        if output_lanes == 4 {
            destination[3] = u8::MAX;
        }
    }
}

fn ycca_to_rgba(input: &[u8], output: &mut [u8]) {
    for (source, destination) in input.chunks_exact(4).zip(output.chunks_exact_mut(4)) {
        // Pillow's YCCA;P raw mode stores premultiplied PhotoYCC channels.
        // Unpremultiply before applying the PhotoYCC-to-RGB transform.
        let alpha = source[3];
        let ycc = if alpha == 0 {
            [0; 3]
        } else {
            [source[0], source[1], source[2]]
                .map(|channel| (u16::from(channel) * 255 / u16::from(alpha)) as u8)
        };
        let rgb = crate::color::photo_ycc_pixel_to_rgb(ycc[0], ycc[1], ycc[2]);
        destination[..3].copy_from_slice(&rgb);
        destination[3] = alpha;
    }
}

fn mul_div_255(left: u32, right: u32) -> u32 {
    let product = left * right + 128;
    ((product >> 8) + product) >> 8
}

fn pack_bilevel_bytes(width: usize, input: &[u8], output: &mut [u8]) {
    for (x, &sample) in input.iter().take(width).enumerate() {
        if sample != 0 {
            output[x / 8] |= 1 << (7 - x % 8);
        }
    }
}

fn packed_luma(
    width: usize,
    bits: u8,
    reverse: bool,
    invert: bool,
    input: &[u8],
    output: &mut [u8],
) {
    let mask = (1u8 << bits) - 1;
    let scale = 255 / usize::from(mask);
    for x in 0..width {
        let source = if reverse {
            input[x * usize::from(bits) / 8].reverse_bits()
        } else {
            input[x * usize::from(bits) / 8]
        };
        let shift = 8 - bits - ((x * usize::from(bits)) % 8) as u8;
        let value = usize::from((source >> shift) & mask) * scale;
        output[x] = if invert {
            255 - value as u8
        } else {
            value as u8
        };
    }
}

fn packed_palette(width: usize, bits: u8, bit_planes: u8, input: &[u8], output: &mut [u8]) {
    if bit_planes == 1 {
        let mask = (1u8 << bits) - 1;
        for x in 0..width {
            let bit_offset = x * usize::from(bits);
            let shift = 8 - bits - (bit_offset % 8) as u8;
            output[x] = (input[bit_offset / 8] >> shift) & mask;
        }
        return;
    }
    let plane_bytes = width.div_ceil(8);
    for (x, pixel) in output.iter_mut().take(width).enumerate() {
        let shift = 7 - x % 8;
        for plane in 0..usize::from(bit_planes) {
            let bit = (input[plane * plane_bytes + x / 8] >> shift) & 1;
            *pixel |= bit << plane;
        }
    }
}

fn high_byte_16(width: usize, offset: usize, input: &[u8], output: &mut [u8]) {
    for x in 0..width {
        output[x] = input[x * 2 + offset];
    }
}

fn high_bytes_16(
    source_lanes: [u8; 4],
    source_bytes_per_pixel: usize,
    output_lanes: usize,
    high_byte_offset: usize,
    padding: Option<u8>,
    input: &[u8],
    output: &mut [u8],
) {
    for (source, destination) in input
        .chunks_exact(source_bytes_per_pixel)
        .zip(output.chunks_exact_mut(output_lanes))
    {
        let source_lanes_per_pixel = source_bytes_per_pixel / 2;
        for (lane, target) in destination.iter_mut().enumerate() {
            if lane < source_lanes_per_pixel {
                let source_lane = usize::from(source_lanes[lane]);
                *target = source[source_lane * 2 + high_byte_offset];
            } else if let Some(padding) = padding {
                *target = padding;
            } else {
                unreachable!("high-byte transform has no source or padding for output lane");
            }
        }
    }
}

fn packed_color_16(
    width: usize,
    format: PackedColorFormat,
    output_lanes: usize,
    input: &[u8],
    output: &mut [u8],
) {
    for (x, destination) in output
        .chunks_exact_mut(output_lanes)
        .take(width)
        .enumerate()
    {
        let start = x * 2;
        let pixel = u16::from_le_bytes([input[start], input[start + 1]]);
        let (red, green, blue, alpha) = match format {
            PackedColorFormat::Rgb555 => (
                scale_channel(pixel & 0x1f, 31),
                scale_channel((pixel >> 5) & 0x1f, 31),
                scale_channel((pixel >> 10) & 0x1f, 31),
                u8::MAX,
            ),
            PackedColorFormat::Bgr555 => (
                scale_channel((pixel >> 10) & 0x1f, 31),
                scale_channel((pixel >> 5) & 0x1f, 31),
                scale_channel(pixel & 0x1f, 31),
                u8::MAX,
            ),
            PackedColorFormat::Rgb565 => (
                scale_channel(pixel & 0x1f, 31),
                scale_channel((pixel >> 5) & 0x3f, 63),
                scale_channel((pixel >> 11) & 0x1f, 31),
                u8::MAX,
            ),
            PackedColorFormat::Bgr565 => (
                scale_channel((pixel >> 11) & 0x1f, 31),
                scale_channel((pixel >> 5) & 0x3f, 63),
                scale_channel(pixel & 0x1f, 31),
                u8::MAX,
            ),
            PackedColorFormat::Rgb444 => (
                scale_nibble(pixel & 0x0f),
                scale_nibble((pixel >> 4) & 0x0f),
                scale_nibble((pixel >> 8) & 0x0f),
                u8::MAX,
            ),
            PackedColorFormat::Rgba5551 => (
                scale_channel(pixel & 0x1f, 31),
                scale_channel((pixel >> 5) & 0x1f, 31),
                scale_channel((pixel >> 10) & 0x1f, 31),
                if pixel & 0x8000 == 0 { 0 } else { u8::MAX },
            ),
            PackedColorFormat::Bgra5551 => (
                scale_channel((pixel >> 10) & 0x1f, 31),
                scale_channel((pixel >> 5) & 0x1f, 31),
                scale_channel(pixel & 0x1f, 31),
                if pixel & 0x8000 == 0 { 0 } else { u8::MAX },
            ),
            PackedColorFormat::Bgra5551InvertedAlpha => (
                scale_channel((pixel >> 10) & 0x1f, 31),
                scale_channel((pixel >> 5) & 0x1f, 31),
                scale_channel(pixel & 0x1f, 31),
                if pixel & 0x8000 == 0 { u8::MAX } else { 0 },
            ),
            PackedColorFormat::Rgba4444 => (
                scale_nibble(pixel & 0x0f),
                scale_nibble((pixel >> 4) & 0x0f),
                scale_nibble((pixel >> 8) & 0x0f),
                scale_nibble((pixel >> 12) & 0x0f),
            ),
        };
        destination[0] = red;
        destination[1] = green;
        destination[2] = blue;
        if output_lanes == 4 {
            destination[3] = alpha;
        }
    }
}

fn scale_channel(value: u16, maximum: u16) -> u8 {
    (u32::from(value) * u32::from(u8::MAX) / u32::from(maximum)) as u8
}

fn scale_nibble(value: u16) -> u8 {
    (value * 17) as u8
}

fn unpack_12_to_16(width: usize, input: &[u8], output: &mut [u8]) {
    let mut x = 0;
    let mut input_offset = 0;
    while x + 1 < width {
        write_native_u16(
            output,
            x,
            (u16::from(input[input_offset]) << 4) | u16::from(input[input_offset + 1] >> 4),
        );
        write_native_u16(
            output,
            x + 1,
            (u16::from(input[input_offset + 1] & 0x0f) << 8) | u16::from(input[input_offset + 2]),
        );
        x += 2;
        input_offset += 3;
    }
    if x < width {
        write_native_u16(
            output,
            x,
            (u16::from(input[input_offset]) << 4) | u16::from(input[input_offset + 1] >> 4),
        );
    }
}

fn write_native_u16(output: &mut [u8], sample: usize, value: u16) {
    let start = sample * 2;
    output[start..start + 2].copy_from_slice(&value.to_ne_bytes());
}

fn channel_band(width: usize, format: ChannelBandFormat, input: &[u8], output: &mut [u8]) {
    output.fill(0);
    for x in 0..width {
        let source = input[x * format.source_bytes_per_sample + format.source_offset];
        output[x * format.output_lanes + format.output_lane] =
            if format.invert { !source } else { source };
    }
}

fn decode_scalar_samples(
    width: usize,
    sample_bytes: u8,
    byte_order: RawByteOrder,
    sample_type: RawScalarType,
    output_type: RawScalarOutput,
    input: &[u8],
    output: &mut [u8],
) {
    let sample_bytes = usize::from(sample_bytes);
    for (source, destination) in input
        .chunks_exact(sample_bytes)
        .take(width)
        .zip(output.chunks_exact_mut(4))
    {
        let bytes = match (sample_type, output_type) {
            (RawScalarType::Integer { signed }, RawScalarOutput::Integer32) => {
                integer_sample_to_i32(source, byte_order, signed).to_ne_bytes()
            }
            (RawScalarType::Integer { signed }, RawScalarOutput::Float32) => {
                integer_sample_to_f32(source, byte_order, signed).to_ne_bytes()
            }
            (RawScalarType::Float32, RawScalarOutput::Float32) => {
                read_u32(source, byte_order).to_ne_bytes()
            }
            (RawScalarType::Float64, RawScalarOutput::Float32) => {
                (f64::from_bits(read_u64(source, byte_order)) as f32).to_ne_bytes()
            }
            _ => unreachable!("raw scalar layout has incompatible input and output types"),
        };
        destination.copy_from_slice(&bytes);
    }
}

fn integer_sample_to_i32(sample: &[u8], order: RawByteOrder, signed: bool) -> i32 {
    match sample.len() {
        1 if signed => i32::from(i8::from_ne_bytes([sample[0]])),
        1 => i32::from(sample[0]),
        2 if signed => i32::from(read_u16(sample, order) as i16),
        2 => i32::from(read_u16(sample, order)),
        4 => read_u32(sample, order) as i32,
        _ => unreachable!("raw integer sample width is unsupported"),
    }
}

fn integer_sample_to_f32(sample: &[u8], order: RawByteOrder, signed: bool) -> f32 {
    match sample.len() {
        1 if signed => f32::from(i8::from_ne_bytes([sample[0]])),
        1 => f32::from(sample[0]),
        2 if signed => f32::from(read_u16(sample, order) as i16),
        2 => f32::from(read_u16(sample, order)),
        4 if signed => (read_u32(sample, order) as i32) as f32,
        4 => read_u32(sample, order) as f32,
        _ => unreachable!("raw integer sample width is unsupported"),
    }
}

fn read_u16(sample: &[u8], order: RawByteOrder) -> u16 {
    let bytes = [sample[0], sample[1]];
    match order {
        RawByteOrder::Little => u16::from_le_bytes(bytes),
        RawByteOrder::Big => u16::from_be_bytes(bytes),
        RawByteOrder::Native => u16::from_ne_bytes(bytes),
    }
}

fn read_u32(sample: &[u8], order: RawByteOrder) -> u32 {
    let bytes = [sample[0], sample[1], sample[2], sample[3]];
    match order {
        RawByteOrder::Little => u32::from_le_bytes(bytes),
        RawByteOrder::Big => u32::from_be_bytes(bytes),
        RawByteOrder::Native => u32::from_ne_bytes(bytes),
    }
}

fn read_u64(sample: &[u8], order: RawByteOrder) -> u64 {
    let bytes = [
        sample[0], sample[1], sample[2], sample[3], sample[4], sample[5], sample[6], sample[7],
    ];
    match order {
        RawByteOrder::Little => u64::from_le_bytes(bytes),
        RawByteOrder::Big => u64::from_be_bytes(bytes),
        RawByteOrder::Native => u64::from_ne_bytes(bytes),
    }
}

fn lab_bias(input: &[u8], output: &mut [u8]) {
    for (source, destination) in input.chunks_exact(3).zip(output.chunks_exact_mut(3)) {
        destination[0] = source[0];
        destination[1] = source[1] ^ 128;
        destination[2] = source[2] ^ 128;
    }
}

fn swap_bytes_16(input: &[u8], output: &mut [u8]) {
    for (source, destination) in input.chunks_exact(2).zip(output.chunks_exact_mut(2)) {
        destination[0] = source[1];
        destination[1] = source[0];
    }
}

fn invert_bytes(input: &[u8], output: &mut [u8]) {
    for (destination, source) in output.iter_mut().zip(input) {
        *destination = !source;
    }
}

fn reverse_bits(input: &[u8], output: &mut [u8]) {
    for (destination, source) in output.iter_mut().zip(input) {
        *destination = source.reverse_bits();
    }
}

fn invert_reverse_bits(input: &[u8], output: &mut [u8]) {
    for (destination, source) in output.iter_mut().zip(input) {
        *destination = !source.reverse_bits();
    }
}
