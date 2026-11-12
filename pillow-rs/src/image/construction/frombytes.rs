//! Prepared raw decoder layout and row handling for `Image.frombytes`.

use std::borrow::Cow;
use std::sync::Arc;

use crate::error::PilError;
use crate::raster::buffer::HiddenPixelLanes;
use crate::raster::Mode;

use super::{FromBytesMode, Image};

#[path = "frombytes/layout.rs"]
mod layout;
#[path = "frombytes/transform.rs"]
mod transform;

use layout::{RawDecoderHiddenLane, RawDecoderTransform, raw_layout};
use transform::transform_row;

/// Validated raw decoder settings for one exact raster mode.
///
/// `rawmode` identifies the byte layout presented to the raw decoder. It does
/// not change the resulting [`Mode`]. Pillow's additional raw-layout aliases
/// are kept separate from the image mode selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawDecoderPlan {
    mode: Mode,
    input_bits: u8,
    bit_planes: u8,
    transform: RawDecoderTransform,
    hidden_lane: RawDecoderHiddenLane,
    stride: i32,
    orientation: i32,
}

impl RawDecoderPlan {
    /// Prepares Pillow's raw decoder for a target mode and optional raw layout.
    ///
    /// A supplied `rawmode` selects a mode-specific raw layout. Row stride and
    /// orientation are applied while decoding and never change the output mode.
    pub fn new(
        mode: Mode,
        rawmode: Option<&str>,
        stride: i32,
        orientation: i32,
    ) -> Result<Self, PilError> {
        validate_mode(mode)?;
        let native_rawmode = mode.pillow_name().or_else(|| mode.rust_name());
        let Some(native_rawmode) = native_rawmode else {
            return Err(PilError::ValueError("unrecognized image mode".into()));
        };
        let layout = raw_layout(mode, rawmode.unwrap_or(native_rawmode), native_rawmode)?;
        Ok(Self {
            mode,
            input_bits: layout.input_bits,
            bit_planes: layout.bit_planes,
            transform: layout.transform,
            hidden_lane: layout.hidden_lane,
            stride,
            orientation,
        })
    }

    /// Returns the exact output mode selected when this plan was prepared.
    #[must_use]
    pub const fn mode(self) -> Mode {
        self.mode
    }

    /// Decodes an immutable input buffer using this prepared row layout.
    pub fn decode(self, size: (u32, u32), data: &[u8]) -> Result<Image, PilError> {
        self.decode_prepared(size, Cow::Borrowed(data))
    }

    /// Decodes an owned input buffer, preserving ownership when rows are tight.
    pub fn decode_owned(self, size: (u32, u32), data: Vec<u8>) -> Result<Image, PilError> {
        self.decode_prepared(size, Cow::Owned(data))
    }

    fn decode_prepared<'a>(
        self,
        (width, height): (u32, u32),
        data: Cow<'a, [u8]>,
    ) -> Result<Image, PilError> {
        let prepared = self.prepare_data((width, height), data)?;
        let mut image = Image::frombytes_with_data(self.mode, (width, height), prepared.data)?;
        if let Some(lanes) = prepared.hidden_lanes {
            attach_raw_hidden_lanes(&mut image, lanes)?;
        }
        Ok(image)
    }

    fn prepare_data<'a>(
        self,
        (width, height): (u32, u32),
        data: Cow<'a, [u8]>,
    ) -> Result<PreparedRawData<'a>, PilError> {
        if width == 0 || height == 0 {
            return Ok(PreparedRawData {
                data,
                hidden_lanes: None,
            });
        }
        let input_row_bytes = decoder_input_row_bytes(
            width,
            self.input_bits,
            self.bit_planes,
        )?;
        let output_row_bytes = raw_row_bytes(self.mode, width)?;
        let stride = effective_stride(self.stride, input_row_bytes)?;
        let required = required_input_bytes(input_row_bytes, stride, height)?;
        if data.len() < required {
            return Err(PilError::ValueError("not enough image data".into()));
        }
        let hidden_lanes = self.prepare_hidden_lanes(
            data.as_ref(),
            width as usize,
            height as usize,
            input_row_bytes,
            stride,
        )?;
        let packed_bytes = packed_image_bytes(output_row_bytes, height)?;
        if input_row_bytes == output_row_bytes
            && stride == input_row_bytes
            && self.orientation >= 0
            && self.transform == RawDecoderTransform::Native
        {
            return Ok(PreparedRawData {
                data: trim_data(data, packed_bytes),
                hidden_lanes,
            });
        }
        let data = pack_rows(
            data.as_ref(),
            input_row_bytes,
            output_row_bytes,
            width as usize,
            stride,
            height,
            self.orientation,
            packed_bytes,
            self.transform,
        )?;
        Ok(PreparedRawData {
            data: Cow::Owned(data),
            hidden_lanes,
        })
    }

    fn prepare_hidden_lanes(
        self,
        data: &[u8],
        width: usize,
        height: usize,
        input_row_bytes: usize,
        stride: usize,
    ) -> Result<Option<HiddenPixelLanes>, PilError> {
        if self.hidden_lane == RawDecoderHiddenLane::Default {
            return Ok(None);
        }
        let pixel_count = width
            .checked_mul(height)
            .ok_or_else(|| PilError::DimensionError("physical pixel lane count overflows".into()))?;
        let lane_bytes = pixel_count
            .checked_mul(1)
            .ok_or_else(|| PilError::DimensionError("physical pixel lane size overflows".into()))?;
        let mut lanes = Vec::new();
        lanes
            .try_reserve_exact(lane_bytes)
            .map_err(|_| PilError::MemoryError("unable to allocate raw image lanes".into()))?;
        for output_row in 0..height {
            let input_row = if self.orientation < 0 {
                height - 1 - output_row
            } else {
                output_row
            };
            let row_start = input_row * stride;
            let row = &data[row_start..row_start + input_row_bytes];
            for x in 0..width {
                lanes.push(self.hidden_lane.value(row, width, x).ok_or_else(|| {
                    PilError::InternalError("raw hidden lane did not match its input row".into())
                })?);
            }
        }
        HiddenPixelLanes::per_pixel(1, pixel_count, lanes)
            .map(Some)
            .ok_or_else(|| PilError::InternalError("invalid raw hidden lane layout".into()))
    }
}

impl RawDecoderHiddenLane {
    fn value(self, row: &[u8], width: usize, x: usize) -> Option<u8> {
        match self {
            Self::Default => None,
            Self::SourcePixelByte {
                source_bytes_per_pixel,
                offset,
            } => row.get(x.checked_mul(source_bytes_per_pixel)?.checked_add(offset)?).copied(),
            Self::SourcePixelWordByte {
                source_bytes_per_pixel,
                component,
                high_byte_offset,
            } => row
                .get(
                    x.checked_mul(source_bytes_per_pixel)?
                        .checked_add(component.checked_mul(2)?)?
                        .checked_add(high_byte_offset)?,
                )
                .copied(),
            Self::PackedAlphaBit15 { inverted } => {
                let start = x.checked_mul(2)?;
                let pixel = u16::from_le_bytes([*row.get(start)?, *row.get(start + 1)?]);
                let opaque = pixel & 0x8000 != 0;
                Some(if opaque ^ inverted { u8::MAX } else { 0 })
            }
            Self::SourcePlaneByte { plane } => row.get(plane.checked_mul(width)?.checked_add(x)?).copied(),
        }
    }
}

struct PreparedRawData<'a> {
    data: Cow<'a, [u8]>,
    hidden_lanes: Option<HiddenPixelLanes>,
}

fn native_input_bits(mode: Mode) -> u8 {
    match mode {
        Mode::One => 1,
        Mode::ImageLuma8 | Mode::ImagePalette8 => 8,
        Mode::ImageLumaA8 | Mode::ImageLaPremultiplied8 | Mode::ImagePaletteAlpha8 => 16,
        Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N => 16,
        Mode::ImageRgb8 | Mode::ImageLab8 | Mode::ImageHsv8 | Mode::ImageYCbCr8 => 24,
        Mode::ImageRgba8
        | Mode::ImageRgbPremultiplied8
        | Mode::ImageRgbx8
        | Mode::ImageCmyk8
        | Mode::ImageI32
        | Mode::ImageF32 => 32,
        Mode::ImageLumaA16 => 32,
        Mode::ImageRgb16 => 48,
        Mode::ImageRgba16 => 64,
        Mode::ImageRgb32F => 96,
        Mode::ImageRgba32F => 128,
    }
}

fn validate_mode(mode: Mode) -> Result<(), PilError> {
    if mode.pillow_name().is_some() {
        FromBytesMode::from_mode(mode).map(|_| ())
    } else if matches!(
        mode,
        Mode::ImageLumaA16
            | Mode::ImageRgb16
            | Mode::ImageRgba16
            | Mode::ImageRgb32F
            | Mode::ImageRgba32F
    ) {
        Ok(())
    } else {
        Err(PilError::ValueError("unrecognized image mode".into()))
    }
}

fn raw_row_bytes(mode: Mode, width: u32) -> Result<usize, PilError> {
    let width = width as usize;
    let bytes_per_pixel = match mode {
        Mode::One => return Ok(width.div_ceil(8)),
        Mode::ImageLuma8 | Mode::ImagePalette8 => 1,
        Mode::ImageLumaA8 | Mode::ImageLaPremultiplied8 | Mode::ImagePaletteAlpha8 => 2,
        Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N => 2,
        Mode::ImageRgb8 | Mode::ImageLab8 | Mode::ImageHsv8 | Mode::ImageYCbCr8 => 3,
        Mode::ImageRgba8
        | Mode::ImageRgbPremultiplied8
        | Mode::ImageRgbx8
        | Mode::ImageCmyk8
        | Mode::ImageI32
        | Mode::ImageF32 => 4,
        Mode::ImageLumaA16 => 4,
        Mode::ImageRgb16 => 6,
        Mode::ImageRgba16 => 8,
        Mode::ImageRgb32F => 12,
        Mode::ImageRgba32F => 16,
    };
    width
        .checked_mul(bytes_per_pixel)
        .ok_or_else(|| PilError::DimensionError("image buffer size overflow".into()))
}

fn decoder_input_row_bytes(
    width: u32,
    input_bits: u8,
    bit_planes: u8,
) -> Result<usize, PilError> {
    let width = width as usize;
    let row_bytes = if bit_planes > 1 {
        width
            .div_ceil(8)
            .checked_mul(usize::from(bit_planes))
    } else {
        width
            .checked_mul(usize::from(input_bits))
            .map(|bits| bits.div_ceil(8))
    };
    row_bytes.ok_or_else(|| PilError::DimensionError("image buffer size overflow".into()))
}

fn effective_stride(stride: i32, row_bytes: usize) -> Result<usize, PilError> {
    if stride == 0 {
        return Ok(row_bytes);
    }
    let stride = usize::try_from(stride)
        .map_err(|_| PilError::ValueError("cannot decode image data".into()))?;
    if stride < row_bytes {
        return Err(PilError::ValueError("cannot decode image data".into()));
    }
    Ok(stride)
}

fn required_input_bytes(
    row_bytes: usize,
    stride: usize,
    height: u32,
) -> Result<usize, PilError> {
    (height as usize - 1)
        .checked_mul(stride)
        .and_then(|rows| rows.checked_add(row_bytes))
        .ok_or_else(|| PilError::DimensionError("image buffer size overflow".into()))
}

fn packed_image_bytes(row_bytes: usize, height: u32) -> Result<usize, PilError> {
    row_bytes
        .checked_mul(height as usize)
        .ok_or_else(|| PilError::DimensionError("image buffer size overflow".into()))
}

fn attach_raw_hidden_lanes(image: &mut Image, lanes: HiddenPixelLanes) -> Result<(), PilError> {
    let Image::Loaded(data) = image else {
        return Err(PilError::InternalError(
            "raw hidden lanes require a loaded image".into(),
        ));
    };
    if !Arc::make_mut(&mut data.image).replace_hidden_pixel_lanes(Some(lanes)) {
        return Err(PilError::InternalError(
            "raw decoder mode did not match its hidden pixel lanes".into(),
        ));
    }
    Ok(())
}

fn trim_data<'a>(data: Cow<'a, [u8]>, expected: usize) -> Cow<'a, [u8]> {
    match data {
        Cow::Borrowed(data) => Cow::Borrowed(&data[..expected]),
        Cow::Owned(mut data) => {
            data.truncate(expected);
            Cow::Owned(data)
        }
    }
}

fn pack_rows(
    data: &[u8],
    input_row_bytes: usize,
    output_row_bytes: usize,
    width: usize,
    stride: usize,
    height: u32,
    orientation: i32,
    packed_bytes: usize,
    transform: RawDecoderTransform,
) -> Result<Vec<u8>, PilError> {
    let mut packed = Vec::new();
    packed
        .try_reserve_exact(packed_bytes)
        .map_err(|_| PilError::MemoryError("unable to allocate raw image data".into()))?;
    packed.resize(packed_bytes, 0);
    let rows = height as usize;
    for output_row in 0..rows {
        let input_row = if orientation < 0 {
            rows - 1 - output_row
        } else {
            output_row
        };
        let input_start = input_row * stride;
        let output_start = output_row * output_row_bytes;
        let input = &data[input_start..input_start + input_row_bytes];
        let output = &mut packed[output_start..output_start + output_row_bytes];
        transform_row(transform, width, input, output);
    }
    Ok(packed)
}
