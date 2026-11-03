//! Original byte-prefix replacement, with source and payload strategies already chosen.

use crate::compute::registry::support::{PaletteKnowledge, PayloadOutput, RequestFacts, StrategyInputs};
use crate::error::PilError;
use crate::raster::buffer::HiddenPixelLanes;
use crate::raster::{DynamicImage, GenericImageView, ImageBuffer, Mode, Pixel};

pub(crate) const UNSUPPORTED_PAYLOAD: &str = "putdata payload mode is not supported";

macro_rules! declare_payloads {
    ($($channels:literal, $compatible:ident, $output:ident {
        $($layout:ident { $($mode:ident),+ $(,)? });+ $(;)?
    });+ $(;)?) => {
        #[derive(Clone, Copy)]
        pub(crate) enum PayloadLayout { $($($layout,)+)+ }

        pub(crate) const PAYLOAD_MODES: &[Mode] = &[$($($(Mode::$mode,)+)+)+];
        $(pub(crate) const $compatible: &[Mode] = &[$($(Mode::$mode,)+)+];)+

        impl PayloadLayout {
            pub(crate) fn new(mode: Mode) -> Result<Self, PilError> {
                match mode {
                    $($($(Mode::$mode)|+ => Ok(Self::$layout),)+)+
                    Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N
                    | Mode::ImageLumaA16 | Mode::ImageRgb16 | Mode::ImageRgba16
                    | Mode::ImageRgb32F | Mode::ImageRgba32F =>
                        Err(PilError::ValueError(UNSUPPORTED_PAYLOAD.into())),
                }
            }
        }

        pub(crate) const fn outputs(source: Mode, native_channels: u8) -> [PayloadOutput; 16] {
            [$($($(PayloadOutput {
                payload: Mode::$mode,
                output: if native_channels == $channels { source } else { Mode::$output },
            },)+)+)+]
        }
    };
}

declare_payloads! {
    1, ONE_BYTE_PAYLOADS, ImageLuma8 {
        Luma { ImageLuma8 };
        Index { One, ImagePalette8 };
    };
    2, TWO_BYTE_PAYLOADS, ImageLumaA8 {
        LumaAlpha { ImageLumaA8, ImageLaPremultiplied8 };
        IndexAlpha { ImagePaletteAlpha8 };
    };
    3, THREE_BYTE_PAYLOADS, ImageRgb8 {
        Rgb { ImageRgb8 };
        Color3 { ImageYCbCr8, ImageLab8, ImageHsv8 };
    };
    4, FOUR_BYTE_PAYLOADS, ImageRgba8 {
        Rgba { ImageRgba8, ImageRgbPremultiplied8, ImageRgbx8 };
        Raw4 { ImageCmyk8, ImageI32, ImageF32 };
    };
}

pub(crate) struct DataInput<'a, P: Pixel> {
    pub(crate) image: &'a ImageBuffer<P, Vec<P::Subpixel>>,
    pub(crate) data: &'a [u8],
    pub(crate) payload: PayloadLayout,
}

/// Deferred state needed to restore compact-carrier lanes after a kernel has
/// rebuilt the logical pixel buffer.
pub(crate) struct HiddenLaneUpdate<'a> {
    pub(crate) source: &'a DynamicImage,
    pub(crate) payload_mode: Mode,
    pub(crate) data_len: usize,
    pub(crate) lane_data: Option<bytes::Bytes>,
}

impl<'a> HiddenLaneUpdate<'a> {
    pub(crate) fn new(
        source: &'a DynamicImage,
        payload_mode: Mode,
        data_len: usize,
        lane_data: Option<bytes::Bytes>,
    ) -> Self {
        Self {
            source,
            payload_mode,
            data_len,
            lane_data,
        }
    }
}

/// Restores raw physical lanes after the selected mode and architecture kernel.
pub(crate) fn apply_hidden_lane_update(
    output: &mut DynamicImage,
    update: HiddenLaneUpdate,
) -> Result<(), PilError> {
    let has_explicit_lanes = update.lane_data.is_some();
    if has_explicit_lanes
        && (output.mode() != update.source.mode() || update.payload_mode != update.source.mode())
    {
        return Err(PilError::InternalError(
            "PutData hidden lanes did not match the output mode".into(),
        ));
    }
    let source_lanes = (output.mode() == update.source.mode())
        .then(|| update.source.hidden_pixel_lanes().cloned())
        .flatten();
    let lane_data = update.lane_data.as_deref().unwrap_or_default();

    match output {
        DynamicImage::ImageLumaA8(image)
        | DynamicImage::ImageLaPremultiplied8(image)
        | DynamicImage::ImagePaletteAlpha8(image) => {
            apply_hidden_lanes::<crate::raster::LumaA<u8>, _>(
                image,
                source_lanes,
                update.data_len,
                lane_data,
                2,
                |pixel| [pixel.0[0], pixel.0[0]],
            )
        }
        DynamicImage::ImageRgb8(image)
        | DynamicImage::ImageYCbCr8(image)
        | DynamicImage::ImageLab8(image)
        | DynamicImage::ImageHsv8(image) => apply_hidden_lanes::<crate::raster::Rgb<u8>, _>(
            image,
            source_lanes,
            update.data_len,
            lane_data,
            1,
            |_| [u8::MAX, 0],
        ),
        _ if has_explicit_lanes => Err(PilError::InternalError(
            "PutData hidden lanes did not match a compact raster mode".into(),
        )),
        _ => Ok(()),
    }
}

fn apply_hidden_lanes<P, F>(
    image: &mut ImageBuffer<P, Vec<u8>>,
    source_lanes: Option<HiddenPixelLanes>,
    data_len: usize,
    lane_data: &[u8],
    lane_count: usize,
    fallback: F,
) -> Result<(), PilError>
where
    P: Pixel<Subpixel = u8>,
    F: Fn(&P) -> [u8; 2],
{
    let channels = usize::from(P::CHANNEL_COUNT);
    if channels == 0 || !(1..=2).contains(&lane_count) {
        return Err(PilError::InternalError(
            "PutData hidden lane layout was invalid".into(),
        ));
    }
    let pixel_count = (image.width() as usize)
        .checked_mul(image.height() as usize)
        .ok_or_else(|| PilError::DimensionError("hidden pixel lane extent overflow".into()))?;
    let touched_bytes = data_len.min(image.as_raw().len());
    let touched_pixels = touched_bytes.div_ceil(channels).min(pixel_count);
    if lane_data.len() % lane_count != 0 || lane_data.len() / lane_count > touched_pixels {
        return Err(PilError::InternalError(
            "PutData hidden lane payload did not match its byte prefix".into(),
        ));
    }

    let complete_lane_bytes = pixel_count
        .checked_mul(lane_count)
        .ok_or_else(|| PilError::DimensionError("hidden pixel lane size overflow".into()))?;
    if touched_pixels == pixel_count && lane_data.len() == complete_lane_bytes {
        return replace_complete_hidden_lanes(image, pixel_count, lane_count, lane_data);
    }

    apply_partial_hidden_lanes(
        image,
        source_lanes,
        touched_pixels,
        lane_data,
        lane_count,
        fallback,
    )
}

fn replace_complete_hidden_lanes<P: Pixel<Subpixel = u8>>(
    image: &mut ImageBuffer<P, Vec<u8>>,
    pixel_count: usize,
    lane_count: usize,
    lane_data: &[u8],
) -> Result<(), PilError> {
    let lanes = if pixel_count == 0 {
        None
    } else {
        let first = [lane_data[0], lane_data.get(1).copied().unwrap_or_default()];
        let uniform = lane_data
            .chunks_exact(lane_count)
            .all(|pixel| pixel == &first[..lane_count]);
        let lanes = if uniform {
            HiddenPixelLanes::constant(lane_count, pixel_count, first).ok_or_else(|| {
                PilError::InternalError("PutData hidden lane layout was invalid".into())
            })?
        } else {
            HiddenPixelLanes::per_pixel(lane_count, pixel_count, lane_data.to_vec()).ok_or_else(
                || {
                    PilError::InternalError(
                        "PutData hidden lane extent did not match its image".into(),
                    )
                },
            )?
        };
        Some(lanes)
    };
    if !image.replace_hidden_pixel_lanes(lanes) {
        return Err(PilError::InternalError(
            "PutData hidden lane extent did not match its image".into(),
        ));
    }
    Ok(())
}

fn apply_partial_hidden_lanes<P, F>(
    image: &mut ImageBuffer<P, Vec<u8>>,
    source_lanes: Option<HiddenPixelLanes>,
    touched_pixels: usize,
    lane_data: &[u8],
    lane_count: usize,
    fallback: F,
) -> Result<(), PilError>
where
    P: Pixel<Subpixel = u8>,
    F: Fn(&P) -> [u8; 2],
{
    let mut hidden_lanes = source_lanes;
    for pixel_index in 0..touched_pixels {
        let lane_offset = pixel_index * lane_count;
        let has_lane_data = lane_offset + lane_count <= lane_data.len();
        let default = if has_lane_data && hidden_lanes.is_some() {
            [0; 2]
        } else {
            let pixel = image.get_pixel(
                (pixel_index % image.width() as usize) as u32,
                (pixel_index / image.width() as usize) as u32,
            );
            fallback(&pixel)
        };
        let values = if has_lane_data {
            let mut values = [0; 2];
            values[..lane_count]
                .copy_from_slice(&lane_data[lane_offset..lane_offset + lane_count]);
            values
        } else {
            default
        };
        if let Some(hidden_lanes) = &mut hidden_lanes {
            if hidden_lanes.lane_count() != lane_count
                || hidden_lanes.set_pixel(pixel_index, values).is_none()
            {
                return Err(PilError::InternalError(
                    "PutData hidden lane payload did not match its image".into(),
                ));
            }
        } else {
            super::put_pixel::update_hidden_lanes(
                image,
                &mut hidden_lanes,
                pixel_index,
                lane_count,
                values,
                default,
                &fallback,
            )?;
        }
    }
    if !image.replace_hidden_pixel_lanes(hidden_lanes) {
        return Err(PilError::InternalError(
            "PutData hidden lane extent did not match its image".into(),
        ));
    }
    Ok(())
}

pub(crate) fn facts<P: Pixel>(
    image: &ImageBuffer<P, Vec<P::Subpixel>>, data: &[u8], payload: Mode,
) -> StrategyInputs {
    StrategyInputs {
        width: image.width(), height: image.height(),
        byte_len: Some(core::mem::size_of_val(image.as_raw().as_slice())),
        palette: PaletteKnowledge::Unknown,
        request: RequestFacts::PutData { payload_mode: payload, data_len: data.len() },
    }
}

struct Prefix {
    width: u32,
    height: u32,
    expected: usize,
    copy_len: usize,
}

impl Prefix {
    fn new(width: u32, height: u32, channels: u8, data_len: usize) -> Self {
        let expected = width as usize * height as usize * usize::from(channels);
        Self { width, height, expected, copy_len: data_len.min(expected) }
    }

    fn apply<P: Pixel<Subpixel = u8>>(
        self, original: ImageBuffer<P, Vec<u8>>, data: &[u8],
    ) -> Result<ImageBuffer<P, Vec<u8>>, PilError> {
        let mut pixels = original.into_raw();
        pixels[..self.copy_len].copy_from_slice(&data[..self.copy_len]);
        ImageBuffer::from_raw(self.width, self.height, pixels)
            .ok_or_else(|| PilError::ValueError("putdata: buffer error".into()))
    }
}

#[inline]
pub(crate) fn converted<P: Pixel<Subpixel = u8>>(
    width: u32, height: u32, data: &[u8], convert: impl FnOnce() -> ImageBuffer<P, Vec<u8>>,
) -> Result<ImageBuffer<P, Vec<u8>>, PilError> {
    let prefix = Prefix::new(width, height, P::CHANNEL_COUNT, data.len());
    prefix.apply(convert(), data)
}

#[inline]
pub(crate) fn native<P: Pixel<Subpixel = u8>>(
    image: &ImageBuffer<P, Vec<u8>>, data: &[u8],
) -> Result<ImageBuffer<P, Vec<u8>>, PilError> {
    let prefix = Prefix::new(image.width(), image.height(), P::CHANNEL_COUNT, data.len());
    if prefix.expected != 0 && prefix.copy_len == prefix.expected
        && let Some(output) = ImageBuffer::from_raw(
            image.width(), image.height(), data[..prefix.expected].to_vec(),
        ) {
        return Ok(output);
    }
    prefix.apply(image.clone(), data)
}

pub(crate) fn clipped<P: Pixel<Subpixel = u8>>(
    image: &ImageBuffer<crate::raster::Luma<u16>, Vec<u16>>,
    pixel: impl Fn(u8) -> P,
) -> ImageBuffer<P, Vec<u8>> {
    ImageBuffer::from_fn(image.width(), image.height(), |x, y| {
        pixel(image.get_pixel(x, y)[0].min(255) as u8)
    })
}
