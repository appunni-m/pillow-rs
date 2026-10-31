//! Serial native-sample Brightness kernels shared by identical mode algorithms.

pub(crate) fn scale_u8(
    source: &[u8],
    channels: usize,
    active_channels: usize,
    factor: f64,
) -> Vec<u8> {
    let mut output = source.to_vec();
    for pixel in output.chunks_exact_mut(channels) {
        for sample in &mut pixel[..active_channels] {
            *sample = (f64::from(*sample) * factor).clamp(0.0, 255.0) as u8;
        }
    }
    output
}

pub(crate) fn scale_u16(
    samples: &mut [u16],
    channels: usize,
    active_channels: usize,
    factor: f64,
) {
    for pixel in samples.chunks_exact_mut(channels) {
        for sample in &mut pixel[..active_channels] {
            *sample = (f64::from(*sample) * factor).clamp(0.0, f64::from(u16::MAX)) as u16;
        }
    }
}

pub(crate) fn scale_f32(
    samples: &mut [f32],
    channels: usize,
    active_channels: usize,
    factor: f64,
) {
    let factor = factor as f32;
    for pixel in samples.chunks_exact_mut(channels) {
        for sample in &mut pixel[..active_channels] {
            *sample *= factor;
        }
    }
}

pub(crate) fn scale_i32_carrier(source: &[u8], factor: f64) -> Vec<u8> {
    let mut output = Vec::with_capacity(source.len());
    for sample in source.chunks_exact(4) {
        let value = i32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]]);
        let scaled = (f64::from(value) * factor)
            .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32;
        output.extend_from_slice(&scaled.to_le_bytes());
    }
    output
}

pub(crate) fn scale_f32_carrier(source: &[u8], factor: f64) -> Vec<u8> {
    let factor = factor as f32;
    let mut output = Vec::with_capacity(source.len());
    for sample in source.chunks_exact(4) {
        let value = f32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]]);
        output.extend_from_slice(&(value * factor).to_le_bytes());
    }
    output
}
