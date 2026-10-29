//! Fixed-point reducers for native interleaved byte samples.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;

#[derive(Clone, Copy)]
struct Rounding {
    multiplier: u64,
    amend: u32,
}

struct Geometry {
    width: u32,
    x_factor: u32,
    y_factor: u32,
    main_width: u32,
    main_height: u32,
    right_width: u32,
    bottom_height: u32,
    full: Rounding,
    right: Rounding,
    bottom: Rounding,
    corner: Rounding,
}

impl Geometry {
    fn new(width: u32, height: u32, x_factor: u32, y_factor: u32) -> Self {
        let main_width = width / x_factor;
        let main_height = height / y_factor;
        let right_width = width % x_factor;
        let bottom_height = height % y_factor;
        Self {
            width,
            x_factor,
            y_factor,
            main_width,
            main_height,
            right_width,
            bottom_height,
            full: rounding(u64::from(x_factor) * u64::from(y_factor)),
            right: rounding(u64::from(right_width) * u64::from(y_factor)),
            bottom: rounding(u64::from(x_factor) * u64::from(bottom_height)),
            corner: rounding(u64::from(right_width) * u64::from(bottom_height)),
        }
    }

    fn block(&self, x: u32, y: u32) -> (u32, u32, u32, u32, Rounding) {
        let full_x = x < self.main_width;
        let full_y = y < self.main_height;
        let (source_x, block_width) = if full_x {
            (x * self.x_factor, self.x_factor)
        } else {
            (self.main_width * self.x_factor, self.right_width)
        };
        let (source_y, block_height, rounding) = if full_y {
            let rounding = if full_x { self.full } else { self.right };
            (y * self.y_factor, self.y_factor, rounding)
        } else {
            let rounding = if full_x { self.bottom } else { self.corner };
            (
                self.main_height * self.y_factor,
                self.bottom_height,
                rounding,
            )
        };
        (source_x, source_y, block_width, block_height, rounding)
    }
}

fn rounding(divider: u64) -> Rounding {
    let divider = divider.max(1);
    Rounding {
        multiplier: ((1u128 << 32) / (u128::from(divider) * 256)) as u64,
        amend: (divider / 2) as u32,
    }
}

#[inline(always)]
fn average(sum: u64, rounding: Rounding) -> u8 {
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "checked image dimensions bound the native byte sum and reciprocal product"
    )]
    let value = ((sum + u64::from(rounding.amend)) * rounding.multiplier) >> 24;
    #[expect(
        clippy::cast_possible_truncation,
        reason = "Pillow's fixed-point byte average is in the range 0..=255"
    )]
    {
        value as u8
    }
}

#[inline(always)]
fn premultiply(sample: u8, alpha: u8) -> u8 {
    let value = (u16::from(sample) * u16::from(alpha) + 127) / 255;
    #[expect(
        clippy::cast_possible_truncation,
        reason = "premultiplication of two byte samples is bounded to one byte"
    )]
    {
        value as u8
    }
}

#[inline(always)]
fn unpremultiply(sample: u8, alpha: u8) -> u8 {
    if alpha == 0 {
        return sample;
    }
    let value = u16::from(sample) * 255 / u16::from(alpha);
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the established Pillow unpremultiplication path stores a byte"
    )]
    {
        value as u8
    }
}

#[inline(always)]
fn rgba_block_sums(
    source: &[u8],
    width: usize,
    source_x: usize,
    source_y: usize,
    block_width: usize,
    block_height: usize,
) -> [u64; 4] {
    let mut sums = [0u64; 4];
    for dy in 0..block_height {
        let mut index = ((source_y + dy) * width + source_x) * 4;
        for _ in 0..block_width {
            let alpha = source[index + 3];
            sums[0] += u64::from(premultiply(source[index], alpha));
            sums[1] += u64::from(premultiply(source[index + 1], alpha));
            sums[2] += u64::from(premultiply(source[index + 2], alpha));
            sums[3] += u64::from(alpha);
            index += 4;
        }
    }
    sums
}

#[inline(always)]
fn average_rgba_block(
    source: &[u8],
    width: usize,
    source_x: usize,
    source_y: usize,
    block_width: usize,
    block_height: usize,
    rounding: Rounding,
) -> [u8; 4] {
    let sums = rgba_block_sums(
        source,
        width,
        source_x,
        source_y,
        block_width,
        block_height,
    );
    let alpha = average(sums[3], rounding);
    [
        unpremultiply(average(sums[0], rounding), alpha),
        unpremultiply(average(sums[1], rounding), alpha),
        unpremultiply(average(sums[2], rounding), alpha),
        alpha,
    ]
}

#[inline(always)]
fn average_interleaved_block<const CHANNELS: usize, const PREMULTIPLIED: bool>(
    source: &[u8],
    width: usize,
    source_x: usize,
    source_y: usize,
    block_width: usize,
    block_height: usize,
    rounding: Rounding,
) -> [u8; 4] {
    let mut sums = [0u64; 4];
    for dy in 0..block_height {
        for dx in 0..block_width {
            let index = ((source_y + dy) * width + source_x + dx) * CHANNELS;
            let alpha = source[index + CHANNELS - 1];
            for channel in 0..CHANNELS {
                let sample = if PREMULTIPLIED && channel + 1 < CHANNELS {
                    premultiply(source[index + channel], alpha)
                } else {
                    source[index + channel]
                };
                sums[channel] += u64::from(sample);
            }
        }
    }
    let mut output = [0u8; 4];
    let alpha = average(sums[CHANNELS - 1], rounding);
    for channel in 0..CHANNELS {
        let value = average(sums[channel], rounding);
        output[channel] = if PREMULTIPLIED && channel + 1 < CHANNELS {
            unpremultiply(value, alpha)
        } else {
            value
        };
    }
    output
}

/// Prepared native byte geometry shared by Default and architecture kernels.
pub(crate) struct ByteReduceContext<
    'a,
    const CHANNELS: usize,
    const PREMULTIPLIED: bool,
> {
    source: &'a [u8],
    geometry: Geometry,
    output_width: u32,
    output_height: u32,
}

impl<'a, const CHANNELS: usize, const PREMULTIPLIED: bool>
    ByteReduceContext<'a, CHANNELS, PREMULTIPLIED>
{
    pub(crate) fn new(
        source: &'a [u8],
        width: u32,
        height: u32,
        x_factor: u32,
        y_factor: u32,
    ) -> Self {
        let x_factor = x_factor.max(1);
        let y_factor = y_factor.max(1);
        ByteReduceContext {
            source,
            geometry: Geometry::new(width, height, x_factor, y_factor),
            output_width: width.div_ceil(x_factor),
            output_height: height.div_ceil(y_factor),
        }
    }

    pub(crate) const fn output_dimensions(&self) -> (u32, u32) {
        (self.output_width, self.output_height)
    }

    pub(crate) const fn main_dimensions(&self) -> (u32, u32) {
        (self.geometry.main_width, self.geometry.main_height)
    }

    pub(crate) const fn source(&self) -> &[u8] {
        self.source
    }

    pub(crate) const fn source_width(&self) -> u32 {
        self.geometry.width
    }

    pub(crate) fn full_width_rounding(&self, y: u32) -> Option<(u32, u32)> {
        let (_, _, block_width, block_height, rounding) = self.geometry.block(0, y);
        if block_width != self.geometry.x_factor || block_height == 0 {
            return None;
        }
        Some((u32::try_from(rounding.multiplier).ok()?, rounding.amend))
    }

    /// The AVX2 guard bounds each complete block sum to u32.
    pub(crate) fn full_width_sums(&self, x: u32, y: u32) -> Option<[u32; 4]> {
        let (source_x, source_y, block_width, block_height, _) = self.geometry.block(x, y);
        if block_width != self.geometry.x_factor || block_height == 0 {
            return None;
        }
        Some(self.accumulate_block::<u32>(source_x, source_y, block_width, block_height))
    }

    pub(crate) fn pixel_sums(&self, x: u32, y: u32) -> Option<([u32; 4], u32, u32)> {
        let (source_x, source_y, block_width, block_height, rounding) = self.geometry.block(x, y);
        if block_width == 0 || block_height == 0 {
            return None;
        }
        let sums = self.accumulate_block::<u64>(source_x, source_y, block_width, block_height);
        let mut narrowed = [0u32; 4];
        for channel in 0..CHANNELS {
            narrowed[channel] = u32::try_from(sums[channel]).ok()?;
        }
        Some((
            narrowed,
            u32::try_from(rounding.multiplier).ok()?,
            rounding.amend,
        ))
    }

    #[expect(
        clippy::arithmetic_side_effects,
        reason = "typed source bounds and the AVX2 factor guard bound indexing and u32 sums"
    )]
    fn accumulate_block<T>(
        &self,
        source_x: u32,
        source_y: u32,
        block_width: u32,
        block_height: u32,
    ) -> [T; 4]
    where
        T: Copy + From<u8> + std::ops::AddAssign,
    {
        let mut sums = [T::from(0); 4];
        for dy in 0..block_height as usize {
            for dx in 0..block_width as usize {
                let index = (((source_y as usize + dy) * self.geometry.width as usize)
                    + source_x as usize
                    + dx)
                    * CHANNELS;
                let alpha = if PREMULTIPLIED {
                    self.source[index + CHANNELS - 1]
                } else {
                    0
                };
                for channel in 0..CHANNELS {
                    let sample = if PREMULTIPLIED && channel + 1 < CHANNELS {
                        premultiply(self.source[index + channel], alpha)
                    } else {
                        self.source[index + channel]
                    };
                    sums[channel] += T::from(sample);
                }
            }
        }
        sums
    }

    #[expect(
        clippy::arithmetic_side_effects,
        reason = "the checked output row bounds each native destination range"
    )]
    pub(crate) fn write_row_range(
        &self,
        y: u32,
        row: &mut [u8],
        start_x: u32,
        end_x: u32,
    ) {
        for x in start_x..end_x {
            let (source_x, source_y, block_width, block_height, rounding) =
                self.geometry.block(x, y);
            let pixel = if CHANNELS == 4 && PREMULTIPLIED {
                average_rgba_block(
                    self.source,
                    self.geometry.width as usize,
                    source_x as usize,
                    source_y as usize,
                    block_width as usize,
                    block_height as usize,
                    rounding,
                )
            } else {
                average_interleaved_block::<CHANNELS, PREMULTIPLIED>(
                    self.source,
                    self.geometry.width as usize,
                    source_x as usize,
                    source_y as usize,
                    block_width as usize,
                    block_height as usize,
                    rounding,
                )
            };
            let destination = x as usize * CHANNELS;
            row[destination..destination + CHANNELS].copy_from_slice(&pixel[..CHANNELS]);
        }
    }
}

/// Reduces one exact byte-layout strategy without interpreting its mode.
pub(crate) fn reduce_bytes<const CHANNELS: usize, const PREMULTIPLIED: bool>(
    source: &[u8],
    width: u32,
    height: u32,
    x_factor: u32,
    y_factor: u32,
) -> Result<Vec<u8>, PilError> {
    let context = ByteReduceContext::<CHANNELS, PREMULTIPLIED>::new(
        source, width, height, x_factor, y_factor,
    );
    let (output_width, output_height) = context.output_dimensions();
    let mut output = CheckedDims::new(output_width, output_height, CHANNELS as u8)?.alloc_buffer();
    if output_width == 0 || output_height == 0 {
        return Ok(output);
    }
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "CheckedDims validated the complete tightly packed output buffer"
    )]
    let row_stride = output_width as usize * CHANNELS;
    for y in 0..output_height {
        let start = y as usize * row_stride;
        context.write_row_range(
            y,
            &mut output[start..start + row_stride],
            0,
            output_width,
        );
    }
    Ok(output)
}
