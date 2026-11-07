//! Canonical raster identity list shared by `Mode` and `DynamicImage`.

macro_rules! for_each_raster_identity {
    ($callback:ident) => {
        for_each_raster_identity! {@dispatch $callback, default}
    };
    ($callback:ident, $argument:expr) => {
        for_each_raster_identity! {@dispatch $callback, $argument}
    };
    (@dispatch $callback:ident, $argument:tt) => {
        $callback! {
            $argument;
            /// Pillow's bilevel `1` mode, expanded to one byte per pixel.
            One(GrayImage),
            /// Pillow's 8-bit luma (`L`) mode.
            ImageLuma8(GrayImage),
            /// Pillow's 8-bit luma with alpha (`LA`) mode.
            ImageLumaA8(GrayAlphaImage),
            /// Pillow's premultiplied luma with alpha (`La`) mode.
            ImageLaPremultiplied8(GrayAlphaImage),
            /// Pillow's 8-bit palette-index (`P`) mode.
            ImagePalette8(GrayImage),
            /// Pillow's palette-index with alpha (`PA`) mode.
            ImagePaletteAlpha8(GrayAlphaImage),
            /// Pillow's 8-bit RGB mode.
            ImageRgb8(RgbImage),
            /// Pillow's 8-bit RGBA mode.
            ImageRgba8(RgbaImage),
            /// Pillow's premultiplied RGBA (`RGBa`) mode.
            ImageRgbPremultiplied8(RgbaImage),
            /// Pillow's 8-bit RGB with an unused padding byte (`RGBX`) mode.
            ImageRgbx8(RgbaImage),
            /// Pillow's 8-bit CMYK mode, stored in four raw bytes per pixel.
            ImageCmyk8(RgbaImage),
            /// Pillow's 8-bit YCbCr mode.
            ImageYCbCr8(RgbImage),
            /// Pillow's 8-bit LAB mode.
            ImageLab8(RgbImage),
            /// Pillow's 8-bit HSV mode.
            ImageHsv8(RgbImage),
            /// Pillow's signed 32-bit integer (`I`) samples in the existing raw carrier.
            ImageI32(RgbaImage),
            /// Pillow's 32-bit float (`F`) samples in the existing raw carrier.
            ImageF32(RgbaImage),
            /// Pillow's native-endian unsigned 16-bit (`I;16`) mode.
            ImageI16(Luma16Image),
            /// Pillow's little-endian unsigned 16-bit (`I;16L`) mode.
            ImageI16L(Luma16Image),
            /// Pillow's big-endian unsigned 16-bit (`I;16B`) mode.
            ImageI16B(Luma16Image),
            /// Pillow's native-order unsigned 16-bit (`I;16N`) mode.
            ImageI16N(Luma16Image),
            /// Internal 16-bit luma-alpha storage, not a Pillow mode identity.
            ImageLumaA16(LumaA16Image),
            /// Internal 16-bit RGB storage, not a Pillow mode identity.
            ImageRgb16(Rgb16Image),
            /// Internal 16-bit RGBA storage, not a Pillow mode identity.
            ImageRgba16(Rgba16Image),
            /// Internal 32-bit float RGB storage, not a Pillow mode identity.
            ImageRgb32F(Rgb32FImage),
            /// Internal 32-bit float RGBA storage, not a Pillow mode identity.
            ImageRgba32F(Rgba32FImage),
        }
    };
}
