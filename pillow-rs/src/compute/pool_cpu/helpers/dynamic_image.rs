//! Exact-mode image construction shared by CPU operation families.

use crate::raster::{
    DynamicImage, GrayAlphaImage, GrayImage, ImageBuffer, Luma, LumaA, Mode, Rgb, RgbImage, Rgba,
    RgbaImage,
};

pub(crate) fn from_raw_bytes(
    mode: Mode,
    width: u32,
    height: u32,
    bytes: Vec<u8>,
) -> Option<DynamicImage> {
    macro_rules! bytes_image {
        ($variant:ident, $image:ty) => {
            <$image>::from_raw(width, height, bytes).map(DynamicImage::$variant)
        };
    }
    macro_rules! samples_image {
        ($variant:ident, $pixel:ty, $sample:ty) => {{
            let samples = native_samples::<$sample>(&bytes)?;
            ImageBuffer::<$pixel, Vec<$sample>>::from_raw(width, height, samples)
                .map(DynamicImage::$variant)
        }};
    }
    match mode {
        Mode::One => bytes_image!(One, GrayImage),
        Mode::ImageLuma8 => bytes_image!(ImageLuma8, GrayImage),
        Mode::ImagePalette8 => bytes_image!(ImagePalette8, GrayImage),
        Mode::ImageLumaA8 => bytes_image!(ImageLumaA8, GrayAlphaImage),
        Mode::ImageLaPremultiplied8 => bytes_image!(ImageLaPremultiplied8, GrayAlphaImage),
        Mode::ImagePaletteAlpha8 => bytes_image!(ImagePaletteAlpha8, GrayAlphaImage),
        Mode::ImageRgb8 => bytes_image!(ImageRgb8, RgbImage),
        Mode::ImageYCbCr8 => bytes_image!(ImageYCbCr8, RgbImage),
        Mode::ImageLab8 => bytes_image!(ImageLab8, RgbImage),
        Mode::ImageHsv8 => bytes_image!(ImageHsv8, RgbImage),
        Mode::ImageRgba8 => bytes_image!(ImageRgba8, RgbaImage),
        Mode::ImageRgbPremultiplied8 => bytes_image!(ImageRgbPremultiplied8, RgbaImage),
        Mode::ImageRgbx8 => bytes_image!(ImageRgbx8, RgbaImage),
        Mode::ImageCmyk8 => bytes_image!(ImageCmyk8, RgbaImage),
        Mode::ImageI32 => bytes_image!(ImageI32, RgbaImage),
        Mode::ImageF32 => bytes_image!(ImageF32, RgbaImage),
        Mode::ImageI16 => samples_image!(ImageI16, Luma<u16>, u16),
        Mode::ImageI16L => samples_image!(ImageI16L, Luma<u16>, u16),
        Mode::ImageI16B => samples_image!(ImageI16B, Luma<u16>, u16),
        Mode::ImageI16N => samples_image!(ImageI16N, Luma<u16>, u16),
        Mode::ImageLumaA16 => samples_image!(ImageLumaA16, LumaA<u16>, u16),
        Mode::ImageRgb16 => samples_image!(ImageRgb16, Rgb<u16>, u16),
        Mode::ImageRgba16 => samples_image!(ImageRgba16, Rgba<u16>, u16),
        Mode::ImageRgb32F => samples_image!(ImageRgb32F, Rgb<f32>, f32),
        Mode::ImageRgba32F => samples_image!(ImageRgba32F, Rgba<f32>, f32),
    }
}

fn native_samples<T: NativeSample>(bytes: &[u8]) -> Option<Vec<T>> {
    if !bytes.len().is_multiple_of(T::WIDTH) {
        return None;
    }
    Some(bytes.chunks_exact(T::WIDTH).map(T::from_ne_bytes).collect())
}

trait NativeSample: Copy {
    const WIDTH: usize;
    fn from_ne_bytes(bytes: &[u8]) -> Self;
}

impl NativeSample for u16 {
    const WIDTH: usize = 2;

    fn from_ne_bytes(bytes: &[u8]) -> Self {
        Self::from_ne_bytes([bytes[0], bytes[1]])
    }
}

impl NativeSample for f32 {
    const WIDTH: usize = 4;

    fn from_ne_bytes(bytes: &[u8]) -> Self {
        Self::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
    }
}
