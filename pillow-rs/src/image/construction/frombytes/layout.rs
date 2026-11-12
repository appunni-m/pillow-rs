use crate::error::PilError;
use crate::raster::Mode;

#[path = "layout/rgb_alpha.rs"]
mod rgb_alpha;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RawDecoderTransform {
    Native,
    ChannelOrder {
        source_lanes: [u8; 4],
        source_lanes_per_pixel: usize,
        output_lanes_per_pixel: usize,
    },
    ChannelOrderWithPadding {
        source_lanes: [u8; 3],
        source_bytes_per_pixel: usize,
    },
    InterleavedTwoBandLine,
    InterleavedThreeBandLine,
    InterleavedThreeBandLineWithPadding { padding: u8 },
    InterleavedFourBandLine,
    RgbTriplets { source_lanes: [u8; 3] },
    LumaAlphaToRgba {
        source_bytes_per_pixel: usize,
        luma_high_byte_offset: usize,
        alpha_high_byte_offset: usize,
    },
    UnpremultiplyRgba {
        source_bytes_per_pixel: usize,
        source_lanes: [u8; 4],
        sample_bytes: usize,
        high_byte_offset: usize,
    },
    InvertRgbKeepAlpha,
    CmykToRgb,
    /// Raw YCC triples are converted to RGB; four-lane outputs pad with 255.
    YccToRgb { output_lanes: usize },
    /// Raw YCCA pixels convert their first three lanes and preserve alpha.
    YccaToRgba,
    LabBias,
    PackBilevelBytes,
    PackedLuma { bits: u8, reverse: bool, invert: bool },
    PackedPalette { bits: u8, bit_planes: u8 },
    HighByte16 { offset: usize },
    HighBytes16 {
        source_lanes: [u8; 4],
        source_bytes_per_pixel: usize,
        output_lanes: usize,
        high_byte_offset: usize,
        padding: Option<u8>,
    },
    PackedColor16 {
        format: PackedColorFormat,
        output_lanes: usize,
    },
    Unpack12To16,
    ChannelBand {
        source_bytes_per_sample: usize,
        source_offset: usize,
        output_lane: usize,
        output_lanes: usize,
        invert: bool,
    },
    DecodeScalar {
        sample_bytes: u8,
        byte_order: RawByteOrder,
        sample_type: RawScalarType,
        output_type: RawScalarOutput,
    },
    SwapBytes16,
    InvertBytes,
    ReverseBits,
    InvertReverseBits,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RawByteOrder {
    Little,
    Big,
    Native,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RawScalarType {
    Integer { signed: bool },
    Float32,
    Float64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RawScalarOutput {
    Integer32,
    Float32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PackedColorFormat {
    Rgb555,
    Bgr555,
    Rgb565,
    Bgr565,
    Rgb444,
    Rgba5551,
    Bgra5551,
    Bgra5551InvertedAlpha,
    Rgba4444,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RawDecoderHiddenLane {
    Default,
    SourcePixelByte {
        source_bytes_per_pixel: usize,
        offset: usize,
    },
    SourcePixelWordByte {
        source_bytes_per_pixel: usize,
        component: usize,
        high_byte_offset: usize,
    },
    PackedAlphaBit15 { inverted: bool },
    SourcePlaneByte { plane: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RawDecoderLayout {
    pub(super) input_bits: u8,
    pub(super) bit_planes: u8,
    pub(super) transform: RawDecoderTransform,
    pub(super) hidden_lane: RawDecoderHiddenLane,
}

impl RawDecoderLayout {
    const fn new(input_bits: u8, bit_planes: u8, transform: RawDecoderTransform) -> Self {
        Self {
            input_bits,
            bit_planes,
            transform,
            hidden_lane: RawDecoderHiddenLane::Default,
        }
    }

    const fn with_hidden_lane(mut self, hidden_lane: RawDecoderHiddenLane) -> Self {
        self.hidden_lane = hidden_lane;
        self
    }
}

pub(super) fn raw_layout(
    mode: Mode,
    rawmode: &str,
    native_rawmode: &str,
) -> Result<RawDecoderLayout, PilError> {
    if rawmode == native_rawmode && mode != Mode::ImageLab8 {
        return Ok(RawDecoderLayout::new(
            super::native_input_bits(mode),
            1,
            RawDecoderTransform::Native,
        ));
    }
    let layout = match mode {
        Mode::One => one_raw_layout(rawmode),
        Mode::ImageLuma8 => luma_raw_layout(rawmode),
        Mode::ImagePalette8 => palette_raw_layout(rawmode),
        Mode::ImageLumaA8 | Mode::ImageLaPremultiplied8 | Mode::ImagePaletteAlpha8 => {
            alpha_luma_raw_layout(mode, rawmode)
        }
        Mode::ImageRgb8 | Mode::ImageYCbCr8 | Mode::ImageLab8 | Mode::ImageHsv8 => {
            rgb_family_raw_layout(mode, rawmode)
        }
        Mode::ImageRgba8 | Mode::ImageRgbPremultiplied8 | Mode::ImageRgbx8 => {
            rgba_family_raw_layout(mode, rawmode)
        }
        Mode::ImageCmyk8 => cmyk_raw_layout(rawmode),
        Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N => {
            i16_raw_layout(mode, rawmode)
        }
        Mode::ImageI32 | Mode::ImageF32 => scalar32_raw_layout(mode, rawmode),
        Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => None,
    };
    layout.ok_or_else(|| {
        // TODO(pillow-rs#3ptb): finish the full Pillow 13 mode/rawmode-pair
        // parity sweep, including stride and orientation variants.
        PilError::ValueError("unknown raw mode for given image mode".into())
    })
}

fn one_raw_layout(rawmode: &str) -> Option<RawDecoderLayout> {
    let layout = match rawmode {
        "1;I" => RawDecoderLayout::new(1, 1, RawDecoderTransform::InvertBytes),
        "1;R" => RawDecoderLayout::new(1, 1, RawDecoderTransform::ReverseBits),
        "1;IR" => RawDecoderLayout::new(1, 1, RawDecoderTransform::InvertReverseBits),
        "1;8" | "L" => RawDecoderLayout::new(8, 1, RawDecoderTransform::PackBilevelBytes),
        _ => return None,
    };
    Some(layout)
}

fn luma_raw_layout(rawmode: &str) -> Option<RawDecoderLayout> {
    let packed = |bits, reverse, invert| {
        RawDecoderLayout::new(
            bits,
            1,
            RawDecoderTransform::PackedLuma {
                bits,
                reverse,
                invert,
            },
        )
    };
    let layout = match rawmode {
        // NO_PILLOW_SUPPORT: Pillow declares L;1 but has no Unpack.c decoder entry.
        "L;1" => packed(1, false, false),
        "L;2" => packed(2, false, false),
        "L;2I" => packed(2, false, true),
        "L;2R" => packed(2, true, false),
        "L;2IR" => packed(2, true, true),
        "L;4" => packed(4, false, false),
        "L;4I" => packed(4, false, true),
        "L;4R" => packed(4, true, false),
        "L;4IR" => packed(4, true, true),
        "L;I" => RawDecoderLayout::new(8, 1, RawDecoderTransform::InvertBytes),
        "L;R" => RawDecoderLayout::new(8, 1, RawDecoderTransform::ReverseBits),
        "L;16" => RawDecoderLayout::new(16, 1, RawDecoderTransform::HighByte16 { offset: 1 }),
        "L;16B" | "LX" => {
            RawDecoderLayout::new(16, 1, RawDecoderTransform::HighByte16 { offset: 0 })
        }
        _ => return None,
    };
    Some(layout)
}

fn palette_raw_layout(rawmode: &str) -> Option<RawDecoderLayout> {
    let packed = |bits, bit_planes| {
        RawDecoderLayout::new(
            bits,
            bit_planes,
            RawDecoderTransform::PackedPalette { bits, bit_planes },
        )
    };
    let layout = match rawmode {
        "P;1" => packed(1, 1),
        "P;2" => packed(2, 1),
        "P;2L" => packed(2, 2),
        "P;4" => packed(4, 1),
        "P;4L" => packed(4, 4),
        "P;R" => RawDecoderLayout::new(8, 1, RawDecoderTransform::ReverseBits),
        "L" => RawDecoderLayout::new(8, 1, RawDecoderTransform::Native),
        "PX" => RawDecoderLayout::new(16, 1, RawDecoderTransform::HighByte16 { offset: 0 }),
        _ => return None,
    };
    Some(layout)
}

fn alpha_luma_raw_layout(mode: Mode, rawmode: &str) -> Option<RawDecoderLayout> {
    let transform = match (mode, rawmode) {
        (Mode::ImageLumaA8, "LA;L") | (Mode::ImagePaletteAlpha8, "PA;L") => {
            RawDecoderTransform::InterleavedTwoBandLine
        }
        (Mode::ImagePaletteAlpha8, "LA") => RawDecoderTransform::Native,
        _ => return None,
    };
    Some(RawDecoderLayout::new(16, 1, transform))
}

fn rgb_family_raw_layout(mode: Mode, rawmode: &str) -> Option<RawDecoderLayout> {
    match mode {
        Mode::ImageRgb8 => rgb_raw_layout(rawmode),
        Mode::ImageYCbCr8 => ycbcr_raw_layout(rawmode),
        Mode::ImageLab8 => lab_raw_layout(rawmode),
        Mode::ImageHsv8 => hsv_raw_layout(rawmode),
        _ => None,
    }
}

fn rgb_raw_layout(rawmode: &str) -> Option<RawDecoderLayout> {
    rgb_byte_layout(rawmode)
        .or_else(|| rgb_word_layout(rawmode))
        .or_else(|| rgb_packed_layout(rawmode))
        .or_else(|| rgb_band_or_special_layout(rawmode))
}

fn rgb_byte_layout(rawmode: &str) -> Option<RawDecoderLayout> {
    let layout = match rawmode {
        "RGB;L" => RawDecoderLayout::new(
            24,
            1,
            RawDecoderTransform::InterleavedThreeBandLine,
        ),
        "RGB;R" => {
            RawDecoderLayout::new(24, 1, RawDecoderTransform::ReverseBits)
        }
        "BGR" => RawDecoderLayout::new(
            24,
            1,
            RawDecoderTransform::ChannelOrder {
                source_lanes: [2, 1, 0, 3],
                source_lanes_per_pixel: 3,
                output_lanes_per_pixel: 3,
            },
        ),
        "RGBX" => channel_layout(
            32,
            [0, 1, 2, 3],
            4,
            3,
            RawDecoderHiddenLane::SourcePixelByte {
                source_bytes_per_pixel: 4,
                offset: 3,
            },
        ),
        "BGRX" => channel_layout(
            32,
            [2, 1, 0, 3],
            4,
            3,
            RawDecoderHiddenLane::Default,
        ),
        "BGXR" => channel_layout(
            32,
            [3, 1, 0, 2],
            4,
            3,
            RawDecoderHiddenLane::Default,
        ),
        "XRGB" => channel_layout(
            32,
            [1, 2, 3, 0],
            4,
            3,
            RawDecoderHiddenLane::Default,
        ),
        "XBGR" => channel_layout(
            32,
            [3, 2, 1, 0],
            4,
            3,
            RawDecoderHiddenLane::Default,
        ),
        "RGBXX" | "RGBXXX" => {
            let (input_bits, source_bytes_per_pixel) = if rawmode == "RGBXX" {
                (40, 5)
            } else {
                (48, 6)
            };
            channel_layout(
                input_bits,
                [0, 1, 2, 3],
                source_bytes_per_pixel,
                3,
                RawDecoderHiddenLane::SourcePixelByte {
                    source_bytes_per_pixel,
                    offset: 3,
                },
            )
        }
        "RGBX;L" | "RGBA;L" => RawDecoderLayout::new(
            32,
            1,
            RawDecoderTransform::InterleavedThreeBandLine,
        )
        .with_hidden_lane(RawDecoderHiddenLane::SourcePlaneByte { plane: 3 }),
        _ => return None,
    };
    Some(layout)
}

fn rgb_word_layout(rawmode: &str) -> Option<RawDecoderLayout> {
    let layout = match rawmode {
        "RGB;16L" | "RGB;16B" | "RGB;16N" => {
            high_bytes_layout(3, 3, rawmode_byte_order(rawmode, "RGB;16L", "RGB;16B"))
        }
        "RGBX;16L" | "RGBX;16B" | "RGBX;16N" => {
            let order = rawmode_byte_order(rawmode, "RGBX;16L", "RGBX;16B");
            high_bytes_layout(4, 3, order).with_hidden_lane(RawDecoderHiddenLane::SourcePixelWordByte {
                source_bytes_per_pixel: 8,
                component: 3,
                high_byte_offset: high_byte_offset(order),
            })
        }
        "R;16L" | "G;16L" | "B;16L" => {
            channel_band_layout(16, 1, 3, false, rgb_band(rawmode))
        }
        "R;16B" | "G;16B" | "B;16B" => {
            channel_band_layout(16, 0, 3, false, rgb_band(rawmode))
        }
        "R;16N" | "G;16N" | "B;16N" => channel_band_layout(
            16,
            high_byte_offset(RawByteOrder::Native),
            3,
            false,
            rgb_band(rawmode),
        ),
        _ => return None,
    };
    Some(layout)
}

fn rgb_packed_layout(rawmode: &str) -> Option<RawDecoderLayout> {
    let layout = match rawmode {
        "RGB;15" => packed_color_layout(PackedColorFormat::Rgb555, 3),
        "BGR;15" | "BGR;5" => packed_color_layout(PackedColorFormat::Bgr555, 3),
        "RGB;16" => packed_color_layout(PackedColorFormat::Rgb565, 3),
        "BGR;16" => packed_color_layout(PackedColorFormat::Bgr565, 3),
        "RGB;4B" => packed_color_layout(PackedColorFormat::Rgb444, 3),
        "RGBA;15" => packed_color_layout(PackedColorFormat::Rgba5551, 3)
            .with_hidden_lane(RawDecoderHiddenLane::PackedAlphaBit15 { inverted: false }),
        _ => return None,
    };
    Some(layout)
}

fn rgb_band_or_special_layout(rawmode: &str) -> Option<RawDecoderLayout> {
    let layout = match rawmode {
        "CMYK" => RawDecoderLayout::new(32, 1, RawDecoderTransform::CmykToRgb),
        "YCC;P" => RawDecoderLayout::new(
            24,
            1,
            RawDecoderTransform::YccToRgb { output_lanes: 3 },
        ),
        "R" | "G" | "B" => channel_band_layout(8, 0, 3, false, rgb_band(rawmode)),
        _ => return None,
    };
    Some(layout)
}

fn ycbcr_raw_layout(rawmode: &str) -> Option<RawDecoderLayout> {
    match rawmode {
        "YCbCr;L" => Some(RawDecoderLayout::new(
            24,
            1,
            RawDecoderTransform::InterleavedThreeBandLine,
        )),
        "YCbCrX" | "YCbCrK" => Some(channel_layout(
            32,
            [0, 1, 2, 3],
            4,
            3,
            RawDecoderHiddenLane::SourcePixelByte {
                source_bytes_per_pixel: 4,
                offset: 3,
            },
        )),
        _ => None,
    }
}

fn lab_raw_layout(rawmode: &str) -> Option<RawDecoderLayout> {
    match rawmode {
        "LAB" => Some(RawDecoderLayout::new(24, 1, RawDecoderTransform::LabBias)),
        "L" => Some(channel_band_layout(8, 0, 3, false, 0)),
        "A" => Some(channel_band_layout(8, 0, 3, false, 1)),
        "B" => Some(channel_band_layout(8, 0, 3, false, 2)),
        _ => None,
    }
}

fn hsv_raw_layout(rawmode: &str) -> Option<RawDecoderLayout> {
    match rawmode {
        "H" => Some(channel_band_layout(8, 0, 3, false, 0)),
        "S" => Some(channel_band_layout(8, 0, 3, false, 1)),
        "V" => Some(channel_band_layout(8, 0, 3, false, 2)),
        "HSV" => Some(RawDecoderLayout::new(24, 1, RawDecoderTransform::Native)),
        _ => None,
    }
}

fn channel_band_layout(
    input_bits: u8,
    source_offset: usize,
    output_lanes: usize,
    invert: bool,
    output_lane: usize,
) -> RawDecoderLayout {
    RawDecoderLayout::new(
        input_bits,
        1,
        RawDecoderTransform::ChannelBand {
            source_bytes_per_sample: usize::from(input_bits.div_ceil(8)),
            source_offset,
            output_lane,
            output_lanes,
            invert,
        },
    )
}

fn rgb_band(rawmode: &str) -> usize {
    match rawmode.as_bytes()[0] {
        b'R' => 0,
        b'G' => 1,
        _ => 2,
    }
}

fn packed_color_layout(format: PackedColorFormat, output_lanes: usize) -> RawDecoderLayout {
    RawDecoderLayout::new(
        16,
        1,
        RawDecoderTransform::PackedColor16 {
            format,
            output_lanes,
        },
    )
}

fn high_bytes_layout(channels: usize, output_lanes: usize, order: RawByteOrder) -> RawDecoderLayout {
    high_bytes_layout_ordered(
        channels,
        [0, 1, 2, 3],
        output_lanes,
        order,
        (output_lanes > channels).then_some(u8::MAX),
    )
}

fn high_bytes_layout_ordered(
    channels: usize,
    source_lanes: [u8; 4],
    output_lanes: usize,
    order: RawByteOrder,
    padding: Option<u8>,
) -> RawDecoderLayout {
    RawDecoderLayout::new(
        (channels * 16) as u8,
        1,
        RawDecoderTransform::HighBytes16 {
            source_lanes,
            source_bytes_per_pixel: channels * 2,
            output_lanes,
            high_byte_offset: high_byte_offset(order),
            padding,
        },
    )
}

fn rawmode_byte_order(rawmode: &str, little: &str, big: &str) -> RawByteOrder {
    if rawmode == little {
        RawByteOrder::Little
    } else if rawmode == big {
        RawByteOrder::Big
    } else {
        RawByteOrder::Native
    }
}

fn high_byte_offset(order: RawByteOrder) -> usize {
    match order {
        RawByteOrder::Little => 1,
        RawByteOrder::Big => 0,
        RawByteOrder::Native if cfg!(target_endian = "little") => 1,
        RawByteOrder::Native => 0,
    }
}

fn channel_layout(
    input_bits: u8,
    source_lanes: [u8; 4],
    source_lanes_per_pixel: usize,
    output_lanes_per_pixel: usize,
    hidden_lane: RawDecoderHiddenLane,
) -> RawDecoderLayout {
    RawDecoderLayout::new(
        input_bits,
        1,
        RawDecoderTransform::ChannelOrder {
            source_lanes,
            source_lanes_per_pixel,
            output_lanes_per_pixel,
        },
    )
    .with_hidden_lane(hidden_lane)
}

fn rgba_family_raw_layout(mode: Mode, rawmode: &str) -> Option<RawDecoderLayout> {
    rgb_alpha::rgba_family_raw_layout(mode, rawmode)
}

fn cmyk_raw_layout(rawmode: &str) -> Option<RawDecoderLayout> {
    match rawmode {
        "CMYK;I" => Some(RawDecoderLayout::new(
            32,
            1,
            RawDecoderTransform::InvertBytes,
        )),
        "CMYKX" | "CMYKXX" => {
            let source_bytes_per_pixel = if rawmode == "CMYKX" { 5 } else { 6 };
            Some(RawDecoderLayout::new(
                (source_bytes_per_pixel * 8) as u8,
                1,
                RawDecoderTransform::ChannelOrder {
                    source_lanes: [0, 1, 2, 3],
                    source_lanes_per_pixel: source_bytes_per_pixel,
                    output_lanes_per_pixel: 4,
                },
            ))
        }
        "CMYK;L" => Some(RawDecoderLayout::new(
            32,
            1,
            RawDecoderTransform::InterleavedFourBandLine,
        )),
        "CMYK;16L" | "CMYK;16B" | "CMYK;16N" => {
            let order = rawmode_byte_order(rawmode, "CMYK;16L", "CMYK;16B");
            Some(high_bytes_layout(4, 4, order))
        }
        "C" | "M" | "Y" | "K" => {
            Some(channel_band_layout(8, 0, 4, false, cmyk_band(rawmode)))
        }
        "C;I" | "M;I" | "Y;I" | "K;I" => {
            Some(channel_band_layout(8, 0, 4, true, cmyk_band(rawmode)))
        }
        _ => None,
    }
}

fn cmyk_band(rawmode: &str) -> usize {
    match rawmode.as_bytes()[0] {
        b'C' => 0,
        b'M' => 1,
        b'Y' => 2,
        _ => 3,
    }
}

fn i16_raw_layout(mode: Mode, rawmode: &str) -> Option<RawDecoderLayout> {
    // The four public I;16 identities retain their declared byte order.
    match (mode, rawmode) {
        (Mode::ImageI16, "I;16B") if cfg!(target_endian = "little") => {
            Some(RawDecoderLayout::new(16, 1, RawDecoderTransform::SwapBytes16))
        }
        (Mode::ImageI16, "I;16B" | "I;16N") => {
            Some(RawDecoderLayout::new(16, 1, RawDecoderTransform::Native))
        }
        (Mode::ImageI16, "I;16R") => {
            Some(RawDecoderLayout::new(16, 1, RawDecoderTransform::ReverseBits))
        }
        (Mode::ImageI16, "I;12") => {
            Some(RawDecoderLayout::new(12, 1, RawDecoderTransform::Unpack12To16))
        }
        (Mode::ImageI16B, "I;16N") if cfg!(target_endian = "little") => {
            Some(RawDecoderLayout::new(16, 1, RawDecoderTransform::SwapBytes16))
        }
        (Mode::ImageI16L, "I;16N") if cfg!(target_endian = "big") => {
            Some(RawDecoderLayout::new(16, 1, RawDecoderTransform::SwapBytes16))
        }
        (Mode::ImageI16L, "I;16N") => {
            Some(RawDecoderLayout::new(16, 1, RawDecoderTransform::Native))
        }
        (Mode::ImageI16B, "I;16N") => {
            Some(RawDecoderLayout::new(16, 1, RawDecoderTransform::Native))
        }
        _ => None,
    }
}

fn scalar32_raw_layout(mode: Mode, rawmode: &str) -> Option<RawDecoderLayout> {
    match mode {
        Mode::ImageI32 => integer32_raw_layout(rawmode),
        Mode::ImageF32 => float32_raw_layout(rawmode),
        _ => None,
    }
}

fn integer32_raw_layout(rawmode: &str) -> Option<RawDecoderLayout> {
    let (sample_bytes, byte_order, signed) = match rawmode {
        "I;8" => (1, RawByteOrder::Native, false),
        "I;8S" => (1, RawByteOrder::Native, true),
        "I;16" => (2, RawByteOrder::Little, false),
        "I;16S" => (2, RawByteOrder::Little, true),
        "I;16B" => (2, RawByteOrder::Big, false),
        "I;16BS" => (2, RawByteOrder::Big, true),
        "I;16N" => (2, RawByteOrder::Native, false),
        "I;16NS" => (2, RawByteOrder::Native, true),
        "I;32" => (4, RawByteOrder::Little, false),
        "I;32S" => (4, RawByteOrder::Little, true),
        "I;32B" => (4, RawByteOrder::Big, false),
        "I;32BS" => (4, RawByteOrder::Big, true),
        "I;32N" => (4, RawByteOrder::Native, false),
        "I;32NS" => (4, RawByteOrder::Native, true),
        _ => return None,
    };
    Some(scalar_layout(
        sample_bytes,
        byte_order,
        RawScalarType::Integer { signed },
        RawScalarOutput::Integer32,
    ))
}

fn float32_raw_layout(rawmode: &str) -> Option<RawDecoderLayout> {
    let (sample_bytes, byte_order, sample_type) = match rawmode {
        "F;8" => (1, RawByteOrder::Native, RawScalarType::Integer { signed: false }),
        "F;8S" => (1, RawByteOrder::Native, RawScalarType::Integer { signed: true }),
        "F;16" => (2, RawByteOrder::Little, RawScalarType::Integer { signed: false }),
        "F;16S" => (2, RawByteOrder::Little, RawScalarType::Integer { signed: true }),
        "F;16B" => (2, RawByteOrder::Big, RawScalarType::Integer { signed: false }),
        "F;16BS" => (2, RawByteOrder::Big, RawScalarType::Integer { signed: true }),
        "F;16N" => (2, RawByteOrder::Native, RawScalarType::Integer { signed: false }),
        "F;16NS" => (2, RawByteOrder::Native, RawScalarType::Integer { signed: true }),
        "F;32" => (4, RawByteOrder::Little, RawScalarType::Integer { signed: false }),
        "F;32S" => (4, RawByteOrder::Little, RawScalarType::Integer { signed: true }),
        "F;32B" => (4, RawByteOrder::Big, RawScalarType::Integer { signed: false }),
        "F;32BS" => (4, RawByteOrder::Big, RawScalarType::Integer { signed: true }),
        "F;32N" => (4, RawByteOrder::Native, RawScalarType::Integer { signed: false }),
        "F;32NS" => (4, RawByteOrder::Native, RawScalarType::Integer { signed: true }),
        "F;32F" => (4, RawByteOrder::Little, RawScalarType::Float32),
        "F;32BF" => (4, RawByteOrder::Big, RawScalarType::Float32),
        "F;32NF" => (4, RawByteOrder::Native, RawScalarType::Float32),
        "F;64F" => (8, RawByteOrder::Little, RawScalarType::Float64),
        "F;64BF" => (8, RawByteOrder::Big, RawScalarType::Float64),
        "F;64NF" => (8, RawByteOrder::Native, RawScalarType::Float64),
        _ => return None,
    };
    Some(scalar_layout(
        sample_bytes,
        byte_order,
        sample_type,
        RawScalarOutput::Float32,
    ))
}

fn scalar_layout(
    sample_bytes: u8,
    byte_order: RawByteOrder,
    sample_type: RawScalarType,
    output_type: RawScalarOutput,
) -> RawDecoderLayout {
    RawDecoderLayout::new(
        sample_bytes * 8,
        1,
        RawDecoderTransform::DecodeScalar {
            sample_bytes,
            byte_order,
            sample_type,
            output_type,
        },
    )
}
