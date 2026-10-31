//! Native SIMD color transforms over RGB samples in three- or four-byte pixels.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::raster::Mode;
use std::sync::OnceLock;
use wide::{f32x8, f64x8, i32x8, u8x16, u32x8};

type YcbcrTables = (
    &'static [i32; 256],
    &'static [i32; 256],
    &'static [i32; 256],
    &'static [i32; 256],
    &'static [i32; 256],
    &'static [i32; 256],
    &'static [i32; 256],
    &'static [i32; 256],
);

pub(crate) fn to_cmyk<const CHANNELS: usize>(
    source: &[u8],
    width: u32,
    height: u32,
    source_mode: Mode,
) -> Result<(Vec<u8>, u64), PilError> {
    let source_dims = checked_source::<CHANNELS>(source, width, height, source_mode)?;
    let output_dims = CheckedDims::new_allow_empty(width, height, 4)?;
    let mut output = Vec::with_capacity(output_dims.total_bytes());
    let mut vector_blocks = 0u64;
    for start in (0..source_dims.total_pixels()).step_by(4) {
        let active = (source_dims.total_pixels() - start).min(4);
        let (red, green, blue) = rgb_block4::<CHANNELS>(source, start, active);
        let cyan = (u8x16::splat(u8::MAX) - u8x16::new(red)).to_array();
        let magenta = (u8x16::splat(u8::MAX) - u8x16::new(green)).to_array();
        let yellow = (u8x16::splat(u8::MAX) - u8x16::new(blue)).to_array();
        for lane in 0..active {
            output.extend_from_slice(&[cyan[lane], magenta[lane], yellow[lane], 0]);
        }
        vector_blocks = vector_blocks.saturating_add(1);
    }
    finish(
        output,
        output_dims.total_bytes(),
        vector_blocks,
        source_mode,
        Mode::ImageCmyk8,
    )
}

pub(crate) fn to_ycbcr<const CHANNELS: usize>(
    source: &[u8],
    width: u32,
    height: u32,
    source_mode: Mode,
) -> Result<(Vec<u8>, u64), PilError> {
    let source_dims = checked_source::<CHANNELS>(source, width, height, source_mode)?;
    let output_dims = CheckedDims::new_allow_empty(width, height, 3)?;
    let mut output = Vec::with_capacity(output_dims.total_bytes());
    let mut vector_blocks = 0u64;
    let tables = ycbcr_tables();
    for start in (0..source_dims.total_pixels()).step_by(8) {
        let active = (source_dims.total_pixels() - start).min(8);
        let (red, green, blue) = rgb_block::<CHANNELS>(source, start, active);
        append_ycbcr_block(red, green, blue, active, tables, &mut output);
        vector_blocks = vector_blocks.saturating_add(1);
    }
    finish(
        output,
        output_dims.total_bytes(),
        vector_blocks,
        source_mode,
        Mode::ImageYCbCr8,
    )
}

pub(crate) fn to_hsv<const CHANNELS: usize>(
    source: &[u8],
    width: u32,
    height: u32,
    source_mode: Mode,
) -> Result<(Vec<u8>, u64), PilError> {
    let source_dims = checked_source::<CHANNELS>(source, width, height, source_mode)?;
    let output_dims = CheckedDims::new_allow_empty(width, height, 3)?;
    let mut output = Vec::with_capacity(output_dims.total_bytes());
    let mut vector_blocks = 0u64;
    for start in (0..source_dims.total_pixels()).step_by(8) {
        let active = (source_dims.total_pixels() - start).min(8);
        let (red, green, blue) = rgb_block::<CHANNELS>(source, start, active);
        append_hsv_block(red, green, blue, active, &mut output);
        vector_blocks = vector_blocks.saturating_add(1);
    }
    finish(
        output,
        output_dims.total_bytes(),
        vector_blocks,
        source_mode,
        Mode::ImageHsv8,
    )
}

pub(crate) fn to_i32<const CHANNELS: usize>(
    source: &[u8],
    width: u32,
    height: u32,
    source_mode: Mode,
) -> Result<(Vec<u8>, u64), PilError> {
    let source_dims = checked_source::<CHANNELS>(source, width, height, source_mode)?;
    let output_dims = CheckedDims::new_allow_empty(width, height, 4)?;
    let mut output = Vec::with_capacity(output_dims.total_bytes());
    let mut vector_blocks = 0u64;
    for start in (0..source_dims.total_pixels()).step_by(8) {
        let active = (source_dims.total_pixels() - start).min(8);
        let (red, green, blue) = rgb_block::<CHANNELS>(source, start, active);
        let values = luma_i32(red, green, blue);
        for value in values.into_iter().take(active) {
            output.extend_from_slice(&value.to_le_bytes());
        }
        vector_blocks = vector_blocks.saturating_add(1);
    }
    finish(
        output,
        output_dims.total_bytes(),
        vector_blocks,
        source_mode,
        Mode::ImageI32,
    )
}

pub(crate) fn to_f32<const CHANNELS: usize>(
    source: &[u8],
    width: u32,
    height: u32,
    source_mode: Mode,
) -> Result<(Vec<u8>, u64), PilError> {
    let source_dims = checked_source::<CHANNELS>(source, width, height, source_mode)?;
    let output_dims = CheckedDims::new_allow_empty(width, height, 4)?;
    let mut output = Vec::with_capacity(output_dims.total_bytes());
    let mut vector_blocks = 0u64;
    for start in (0..source_dims.total_pixels()).step_by(8) {
        let active = (source_dims.total_pixels() - start).min(8);
        let (red, green, blue) = rgb_block::<CHANNELS>(source, start, active);
        let sum = rgb_luma_sum(red, green, blue);
        let values = f32x8::new(sum.to_array().map(|value| value as f32)) / f32x8::splat(1000.0);
        for value in values.to_array().into_iter().take(active) {
            output.extend_from_slice(&value.to_le_bytes());
        }
        vector_blocks = vector_blocks.saturating_add(1);
    }
    finish(
        output,
        output_dims.total_bytes(),
        vector_blocks,
        source_mode,
        Mode::ImageF32,
    )
}

pub(crate) fn to_luma_alpha_opaque<const CHANNELS: usize>(
    source: &[u8],
    width: u32,
    height: u32,
    source_mode: Mode,
) -> Result<(Vec<u8>, u64), PilError> {
    let source_dims = checked_source::<CHANNELS>(source, width, height, source_mode)?;
    let output_dims = CheckedDims::new_allow_empty(width, height, 2)?;
    let mut output = Vec::with_capacity(output_dims.total_bytes());
    let mut vector_blocks = 0u64;
    for start in (0..source_dims.total_pixels()).step_by(8) {
        let active = (source_dims.total_pixels() - start).min(8);
        let (red, green, blue) = rgb_block::<CHANNELS>(source, start, active);
        let luma = luma_i32(red, green, blue).map(|value| value.min(255) as u8);
        append_opaque_luma_alpha(luma, active, &mut output);
        vector_blocks = vector_blocks.saturating_add(1);
    }
    finish(
        output,
        output_dims.total_bytes(),
        vector_blocks,
        source_mode,
        Mode::ImageLumaA8,
    )
}

fn append_opaque_luma_alpha(luma: [u8; 8], active: usize, output: &mut Vec<u8>) {
    for value in luma.into_iter().take(active) {
        output.extend_from_slice(&[value, u8::MAX]);
    }
}

fn checked_source<const CHANNELS: usize>(
    source: &[u8],
    width: u32,
    height: u32,
    source_mode: Mode,
) -> Result<CheckedDims, PilError> {
    let channels = u8::try_from(CHANNELS)
        .map_err(|_| PilError::InternalError("native RGB channel count is invalid".into()))?;
    let dims = CheckedDims::new_allow_empty(width, height, channels)?;
    if source.len() != dims.total_bytes() {
        let source_name = source_mode.pillow_name().unwrap_or("unknown");
        return Err(PilError::InternalError(format!(
            "{source_name} Convert source storage did not match its dimensions"
        )));
    }
    Ok(dims)
}

fn rgb_block<const CHANNELS: usize>(
    source: &[u8],
    start: usize,
    active: usize,
) -> ([u8; 8], [u8; 8], [u8; 8]) {
    let red = std::array::from_fn(|lane| {
        if lane < active { source[(start + lane) * CHANNELS] } else { 0 }
    });
    let green = std::array::from_fn(|lane| {
        if lane < active { source[(start + lane) * CHANNELS + 1] } else { 0 }
    });
    let blue = std::array::from_fn(|lane| {
        if lane < active { source[(start + lane) * CHANNELS + 2] } else { 0 }
    });
    (red, green, blue)
}

fn rgb_block4<const CHANNELS: usize>(
    source: &[u8],
    start: usize,
    active: usize,
) -> ([u8; 16], [u8; 16], [u8; 16]) {
    let red = std::array::from_fn(|lane| {
        if lane < active { source[(start + lane) * CHANNELS] } else { 0 }
    });
    let green = std::array::from_fn(|lane| {
        if lane < active { source[(start + lane) * CHANNELS + 1] } else { 0 }
    });
    let blue = std::array::from_fn(|lane| {
        if lane < active { source[(start + lane) * CHANNELS + 2] } else { 0 }
    });
    (red, green, blue)
}

fn luma_i32(red: [u8; 8], green: [u8; 8], blue: [u8; 8]) -> [i32; 8] {
    (u32x8::new(red.map(u32::from)) * u32x8::splat(19595)
        + u32x8::new(green.map(u32::from)) * u32x8::splat(38470)
        + u32x8::new(blue.map(u32::from)) * u32x8::splat(7471)
        + u32x8::splat(32768))
        .to_array()
        .map(|value| (value >> 16) as i32)
}

fn rgb_luma_sum(red: [u8; 8], green: [u8; 8], blue: [u8; 8]) -> u32x8 {
    u32x8::new(red.map(u32::from)) * u32x8::splat(299)
        + u32x8::new(green.map(u32::from)) * u32x8::splat(587)
        + u32x8::new(blue.map(u32::from)) * u32x8::splat(114)
}

fn ycbcr_table(coefficient: f64) -> [i32; 256] {
    let mut table = [0i32; 256];
    for (value, output) in table.iter_mut().enumerate() {
        *output = (value as f64 * coefficient * 64.0 + 0.5) as i32;
    }
    table
}

fn ycbcr_tables() -> YcbcrTables {
    static Y_R: OnceLock<[i32; 256]> = OnceLock::new();
    static Y_G: OnceLock<[i32; 256]> = OnceLock::new();
    static Y_B: OnceLock<[i32; 256]> = OnceLock::new();
    static CB_R: OnceLock<[i32; 256]> = OnceLock::new();
    static CB_G: OnceLock<[i32; 256]> = OnceLock::new();
    static CB_B: OnceLock<[i32; 256]> = OnceLock::new();
    static CR_G: OnceLock<[i32; 256]> = OnceLock::new();
    static CR_B: OnceLock<[i32; 256]> = OnceLock::new();
    (
        Y_R.get_or_init(|| ycbcr_table(0.299)),
        Y_G.get_or_init(|| ycbcr_table(0.587)),
        Y_B.get_or_init(|| ycbcr_table(0.114)),
        CB_R.get_or_init(|| ycbcr_table(-0.16874)),
        CB_G.get_or_init(|| ycbcr_table(-0.33126)),
        CB_B.get_or_init(|| ycbcr_table(0.5)),
        CR_G.get_or_init(|| ycbcr_table(-0.41869)),
        CR_B.get_or_init(|| ycbcr_table(-0.08131)),
    )
}

fn append_ycbcr_block(
    red: [u8; 8],
    green: [u8; 8],
    blue: [u8; 8],
    active: usize,
    tables: YcbcrTables,
    output: &mut Vec<u8>,
) {
    let (y_r, y_g, y_b, cb_r, cb_g, cb_b, cr_g, cr_b) = tables;
    let y = (i32x8::new(red.map(|value| y_r[usize::from(value)]))
        + i32x8::new(green.map(|value| y_g[usize::from(value)]))
        + i32x8::new(blue.map(|value| y_b[usize::from(value)])))
        >> 6u32;
    let cb = (i32x8::new(red.map(|value| cb_r[usize::from(value)]))
        + i32x8::new(green.map(|value| cb_g[usize::from(value)]))
        + i32x8::new(blue.map(|value| cb_b[usize::from(value)])))
        >> 6u32;
    let cr = (i32x8::new(red.map(|value| cb_b[usize::from(value)]))
        + i32x8::new(green.map(|value| cr_g[usize::from(value)]))
        + i32x8::new(blue.map(|value| cr_b[usize::from(value)])))
        >> 6u32;
    let y = y.to_array().map(|value| value.clamp(0, 255) as u8);
    let cb = cb.to_array().map(|value| (value + 128).clamp(0, 255) as u8);
    let cr = cr.to_array().map(|value| (value + 128).clamp(0, 255) as u8);
    for lane in 0..active {
        output.extend_from_slice(&[y[lane], cb[lane], cr[lane]]);
    }
}

fn append_hsv_block(
    red: [u8; 8],
    green: [u8; 8],
    blue: [u8; 8],
    active: usize,
    output: &mut Vec<u8>,
) {
    let red_vector = f32x8::new(red.map(f32::from));
    let green_vector = f32x8::new(green.map(f32::from));
    let blue_vector = f32x8::new(blue.map(f32::from));
    let max_vector = red_vector.max(green_vector.max(blue_vector));
    let min_vector = red_vector.min(green_vector.min(blue_vector));
    let delta_vector = max_vector - min_vector;
    let safe_delta = delta_vector
        .simd_eq(f32x8::splat(0.0))
        .select(f32x8::splat(1.0), delta_vector);
    let red_ratio = (max_vector - red_vector) / safe_delta;
    let green_ratio = (max_vector - green_vector) / safe_delta;
    let blue_ratio = (max_vector - blue_vector) / safe_delta;
    let red_h = f64x8::new(blue_ratio.to_array().map(f64::from))
        - f64x8::new(green_ratio.to_array().map(f64::from));
    let green_h = f64x8::splat(2.0) + f64x8::new(red_ratio.to_array().map(f64::from))
        - f64x8::new(blue_ratio.to_array().map(f64::from));
    let blue_h = f64x8::splat(4.0) + f64x8::new(green_ratio.to_array().map(f64::from))
        - f64x8::new(red_ratio.to_array().map(f64::from));
    let red_h = red_h.to_array();
    let green_h = green_h.to_array();
    let blue_h = blue_h.to_array();
    let saturation = (delta_vector
        / max_vector
            .simd_eq(f32x8::splat(0.0))
            .select(f32x8::splat(1.0), max_vector))
    .to_array();
    let maximum = max_vector.to_array().map(|value| value as u8);
    let delta = delta_vector.to_array();
    let hue = std::array::from_fn(|lane| {
        let selected = if red[lane] == maximum[lane] {
            red_h[lane]
        } else if green[lane] == maximum[lane] {
            green_h[lane]
        } else {
            blue_h[lane]
        };
        (selected as f32) as f64
    });
    let hue = f64x8::new(hue) / f64x8::splat(6.0) + f64x8::splat(1.0);
    let hue = (hue - hue.floor()).to_array();
    for lane in 0..active {
        if delta[lane] == 0.0 {
            output.extend_from_slice(&[0, 0, maximum[lane]]);
        } else {
            let hue_byte = (f64::from(hue[lane] as f32) * 255.0) as u8;
            let saturation_byte = (f64::from(saturation[lane]) * 255.0) as u8;
            output.extend_from_slice(&[hue_byte, saturation_byte, maximum[lane]]);
        }
    }
}

fn finish(
    output: Vec<u8>,
    expected_bytes: usize,
    vector_blocks: u64,
    source_mode: Mode,
    target_mode: Mode,
) -> Result<(Vec<u8>, u64), PilError> {
    if output.len() != expected_bytes {
        let source_name = source_mode.pillow_name().unwrap_or("unknown");
        let target_name = target_mode.pillow_name().unwrap_or("unknown");
        return Err(PilError::InternalError(format!(
            "{source_name} Convert {target_name} output shape mismatch"
        )));
    }
    Ok((output, vector_blocks))
}
