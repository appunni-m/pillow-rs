//! Original byte-prefix replacement, with source and payload strategies already chosen.

use crate::compute::registry::support::{PaletteKnowledge, PayloadOutput, RequestFacts, StrategyInputs};
use crate::error::PilError;
use crate::raster::{ImageBuffer, Mode, Pixel};

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
                    Mode::ImageLaPremultiplied8 | Mode::ImageLab8
                    | Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N
                    | Mode::ImageLumaA16 | Mode::ImageRgb16 | Mode::ImageRgba16
                    | Mode::ImageRgb32F | Mode::ImageRgba32F =>
                        Err(PilError::ValueError(UNSUPPORTED_PAYLOAD.into())),
                }
            }
        }

        pub(crate) const fn outputs(source: Mode, native_channels: u8) -> [PayloadOutput; 14] {
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
        LumaAlpha { ImageLumaA8 };
        IndexAlpha { ImagePaletteAlpha8 };
    };
    3, THREE_BYTE_PAYLOADS, ImageRgb8 {
        Rgb { ImageRgb8 };
        Color3 { ImageYCbCr8, ImageHsv8 };
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

pub(crate) fn facts<P: Pixel>(
    image: &ImageBuffer<P, Vec<P::Subpixel>>, data: &[u8], payload: Mode,
) -> StrategyInputs {
    StrategyInputs {
        width: image.width(), height: image.height(),
        byte_len: Some(core::mem::size_of_val(image.as_raw().as_slice())),
        palette: PaletteKnowledge::Unknown,
        request: RequestFacts::PutData { mode: payload, data_len: data.len() },
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
