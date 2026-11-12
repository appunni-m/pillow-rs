use super::{
    Mode, PackedColorFormat, RawByteOrder, RawDecoderLayout, RawDecoderTransform,
    channel_band_layout, high_byte_offset, high_bytes_layout, high_bytes_layout_ordered,
    packed_color_layout, rawmode_byte_order,
};

pub(super) fn rgba_family_raw_layout(mode: Mode, rawmode: &str) -> Option<RawDecoderLayout> {
    match mode {
        Mode::ImageRgba8 => rgba_raw_layout(rawmode),
        Mode::ImageRgbPremultiplied8 => rgb_premultiplied_raw_layout(rawmode),
        Mode::ImageRgbx8 => rgbx_raw_layout(rawmode),
        _ => None,
    }
}

fn rgba_raw_layout(rawmode: &str) -> Option<RawDecoderLayout> {
    rgba_channel_order_layout(rawmode)
        .or_else(|| rgba_packed_layout(rawmode))
        .or_else(|| rgba_alpha_input_layout(rawmode))
        .or_else(|| rgba_typed_layout(rawmode))
}

fn rgba_channel_order_layout(rawmode: &str) -> Option<RawDecoderLayout> {
    let source_lanes = match rawmode {
        "BGRA" => [2, 1, 0, 3],
        "ARGB" => [1, 2, 3, 0],
        "ABGR" => [3, 2, 1, 0],
        "BGAR" => [3, 1, 0, 2],
        _ => return None,
    };
    Some(RawDecoderLayout::new(
        32,
        1,
        RawDecoderTransform::ChannelOrder {
            source_lanes,
            source_lanes_per_pixel: 4,
            output_lanes_per_pixel: 4,
        },
    ))
}

fn rgba_packed_layout(rawmode: &str) -> Option<RawDecoderLayout> {
    let format = match rawmode {
        "RGBA;15" => PackedColorFormat::Rgba5551,
        "BGRA;15" => PackedColorFormat::Bgra5551,
        "BGRA;15Z" => PackedColorFormat::Bgra5551InvertedAlpha,
        "RGBA;4B" => PackedColorFormat::Rgba4444,
        _ => return None,
    };
    Some(packed_color_layout(format, 4))
}

fn rgba_alpha_input_layout(rawmode: &str) -> Option<RawDecoderLayout> {
    match rawmode {
        "LA" => Some(RawDecoderLayout::new(
            16,
            1,
            RawDecoderTransform::LumaAlphaToRgba {
                source_bytes_per_pixel: 2,
                luma_high_byte_offset: 0,
                alpha_high_byte_offset: 0,
            },
        )),
        "LA;16B" => Some(RawDecoderLayout::new(
            32,
            1,
            RawDecoderTransform::LumaAlphaToRgba {
                source_bytes_per_pixel: 4,
                luma_high_byte_offset: 0,
                alpha_high_byte_offset: 0,
            },
        )),
        "RGBAX" | "RGBAXX" => raw_rgba_four_lane_skip(rawmode),
        "RGBa" | "RGBaX" | "RGBaXX" => raw_rgba_unpremultiply(rawmode),
        "RGBa;16L" | "RGBa;16B" | "RGBa;16N" => {
            Some(raw_rgba_unpremultiply_16(rawmode))
        }
        "BGR" => Some(RawDecoderLayout::new(
            24,
            1,
            RawDecoderTransform::RgbTriplets {
                source_lanes: [2, 1, 0],
            },
        )),
        "BGRa" => Some(RawDecoderLayout::new(
            32,
            1,
            RawDecoderTransform::UnpremultiplyRgba {
                source_bytes_per_pixel: 4,
                source_lanes: [2, 1, 0, 3],
                sample_bytes: 1,
                high_byte_offset: 0,
            },
        )),
        _ => None,
    }
}

fn raw_rgba_four_lane_skip(rawmode: &str) -> Option<RawDecoderLayout> {
    let source_bytes_per_pixel = if rawmode == "RGBAX" { 5 } else { 6 };
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

fn raw_rgba_unpremultiply(rawmode: &str) -> Option<RawDecoderLayout> {
    let source_bytes_per_pixel = match rawmode {
        "RGBa" => 4,
        "RGBaX" => 5,
        _ => 6,
    };
    Some(RawDecoderLayout::new(
        (source_bytes_per_pixel * 8) as u8,
        1,
        RawDecoderTransform::UnpremultiplyRgba {
            source_bytes_per_pixel,
            source_lanes: [0, 1, 2, 3],
            sample_bytes: 1,
            high_byte_offset: 0,
        },
    ))
}

fn raw_rgba_unpremultiply_16(rawmode: &str) -> RawDecoderLayout {
    let order = rawmode_byte_order(rawmode, "RGBa;16L", "RGBa;16B");
    RawDecoderLayout::new(
        64,
        1,
        RawDecoderTransform::UnpremultiplyRgba {
            source_bytes_per_pixel: 8,
            source_lanes: [0, 1, 2, 3],
            sample_bytes: 2,
            high_byte_offset: high_byte_offset(order),
        },
    )
}

fn rgba_typed_layout(rawmode: &str) -> Option<RawDecoderLayout> {
    match rawmode {
        "RGBA;I" => Some(RawDecoderLayout::new(
            32,
            1,
            RawDecoderTransform::InvertRgbKeepAlpha,
        )),
        "RGBA;L" => Some(RawDecoderLayout::new(
            32,
            1,
            RawDecoderTransform::InterleavedFourBandLine,
        )),
        "RGBA;16L" | "RGBA;16B" | "RGBA;16N" => {
            let order = rawmode_byte_order(rawmode, "RGBA;16L", "RGBA;16B");
            Some(high_bytes_layout(4, 4, order))
        }
        "BGRA;16L" | "BGRA;16B" => {
            let order = if rawmode == "BGRA;16L" {
                RawByteOrder::Little
            } else {
                RawByteOrder::Big
            };
            Some(high_bytes_layout_ordered(
                4,
                [2, 1, 0, 3],
                4,
                order,
                None,
            ))
        }
        "R" | "G" | "B" | "A" => {
            Some(channel_band_layout(8, 0, 4, false, rgba_band(rawmode)))
        }
        "R;16L" | "G;16L" | "B;16L" | "A;16L" => {
            Some(channel_band_layout(16, 1, 4, false, rgba_band(rawmode)))
        }
        "R;16B" | "G;16B" | "B;16B" | "A;16B" => {
            Some(channel_band_layout(16, 0, 4, false, rgba_band(rawmode)))
        }
        "R;16N" | "G;16N" | "B;16N" | "A;16N" => Some(channel_band_layout(
            16,
            high_byte_offset(RawByteOrder::Native),
            4,
            false,
            rgba_band(rawmode),
        )),
        "YCCA;P" => Some(RawDecoderLayout::new(
            32,
            1,
            RawDecoderTransform::YccaToRgba,
        )),
        _ => None,
    }
}

fn rgba_band(rawmode: &str) -> usize {
    match rawmode.as_bytes()[0] {
        b'R' => 0,
        b'G' => 1,
        b'B' => 2,
        _ => 3,
    }
}

fn rgb_premultiplied_raw_layout(rawmode: &str) -> Option<RawDecoderLayout> {
    let source_lanes = match rawmode {
        "BGRa" => [2, 1, 0, 3],
        "aRGB" => [1, 2, 3, 0],
        "aBGR" => [3, 2, 1, 0],
        _ => return None,
    };
    Some(RawDecoderLayout::new(
        32,
        1,
        RawDecoderTransform::ChannelOrder {
            source_lanes,
            source_lanes_per_pixel: 4,
            output_lanes_per_pixel: 4,
        },
    ))
}

fn rgbx_raw_layout(rawmode: &str) -> Option<RawDecoderLayout> {
    rgbx_byte_layout(rawmode)
        .or_else(|| rgbx_word_layout(rawmode))
        .or_else(|| rgbx_packed_layout(rawmode))
}

fn rgbx_byte_layout(rawmode: &str) -> Option<RawDecoderLayout> {
    match rawmode {
        "BGRX" => Some(RawDecoderLayout::new(
            32,
            1,
            RawDecoderTransform::ChannelOrderWithPadding {
                source_lanes: [2, 1, 0],
                source_bytes_per_pixel: 4,
            },
        )),
        "XRGB" => Some(RawDecoderLayout::new(
            32,
            1,
            RawDecoderTransform::ChannelOrderWithPadding {
                source_lanes: [1, 2, 3],
                source_bytes_per_pixel: 4,
            },
        )),
        "XBGR" => Some(RawDecoderLayout::new(
            32,
            1,
            RawDecoderTransform::ChannelOrderWithPadding {
                source_lanes: [3, 2, 1],
                source_bytes_per_pixel: 4,
            },
        )),
        "RGB" => Some(RawDecoderLayout::new(
            24,
            1,
            RawDecoderTransform::RgbTriplets {
                source_lanes: [0, 1, 2],
            },
        )),
        "YCC;P" => Some(RawDecoderLayout::new(
            24,
            1,
            RawDecoderTransform::YccToRgb { output_lanes: 4 },
        )),
        "BGR" => Some(RawDecoderLayout::new(
            24,
            1,
            RawDecoderTransform::RgbTriplets {
                source_lanes: [2, 1, 0],
            },
        )),
        "RGB;L" => Some(RawDecoderLayout::new(
            24,
            1,
            RawDecoderTransform::InterleavedThreeBandLineWithPadding {
                padding: u8::MAX,
            },
        )),
        "RGBX;L" => Some(RawDecoderLayout::new(
            32,
            1,
            RawDecoderTransform::InterleavedFourBandLine,
        )),
        "RGBXX" | "RGBXXX" => {
            let source_bytes_per_pixel = if rawmode == "RGBXX" { 5 } else { 6 };
            return Some(RawDecoderLayout::new(
                (source_bytes_per_pixel * 8) as u8,
                1,
                RawDecoderTransform::ChannelOrder {
                    source_lanes: [0, 1, 2, 3],
                    source_lanes_per_pixel: source_bytes_per_pixel,
                    output_lanes_per_pixel: 4,
                },
            ));
        }
        "R" | "G" | "B" | "X" => {
            Some(channel_band_layout(8, 0, 4, false, rgba_band(rawmode)))
        }
        _ => None,
    }
}

fn rgbx_word_layout(rawmode: &str) -> Option<RawDecoderLayout> {
    match rawmode {
        "RGBX;16L" | "RGBX;16B" | "RGBX;16N" => {
            let order = rawmode_byte_order(rawmode, "RGBX;16L", "RGBX;16B");
            Some(high_bytes_layout(4, 4, order))
        }
        "RGB;16B" => Some(high_bytes_layout(3, 4, RawByteOrder::Big)),
        _ => None,
    }
}

fn rgbx_packed_layout(rawmode: &str) -> Option<RawDecoderLayout> {
    let format = match rawmode {
        "RGB;15" => PackedColorFormat::Rgb555,
        "BGR;15" | "BGR;5" => PackedColorFormat::Bgr555,
        "RGB;16" => PackedColorFormat::Rgb565,
        "BGR;16" => PackedColorFormat::Bgr565,
        "RGB;4B" => PackedColorFormat::Rgb444,
        _ => return None,
    };
    Some(packed_color_layout(format, 4))
}
