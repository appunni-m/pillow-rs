use crate::draw::DrawCanvas;
use crate::raster::{
    DynamicImage, GrayAlphaImage, GrayImage, ImageBuffer, Luma, RgbImage, RgbaImage,
};

pub(super) enum NativeDrawCanvas {
    L(GrayImage),
    LA(GrayAlphaImage),
    Rgb(RgbImage, bool),
    Rgba(RgbaImage),
    /// Unsigned 16-bit luma storage used by Pillow's I;16* draw modes.
    ///
    /// The raster keeps host-native samples. Pillow's draw backend, however,
    /// writes the packed little-endian ink bytes before the logical mode's
    /// byte-order adapter is applied, so retain that mode bit while drawing.
    L16(ImageBuffer<Luma<u16>, Vec<u16>>, bool),
}

impl NativeDrawCanvas {
    pub(super) fn from_image(
        image: &DynamicImage,
        alpha_blend_rgb: bool,
        mode: Option<&str>,
    ) -> Self {
        match image {
            DynamicImage::ImageLuma8(pixels) => Self::L(pixels.clone()),
            DynamicImage::ImageLumaA8(pixels) => Self::LA(pixels.clone()),
            DynamicImage::ImageRgb8(pixels) => Self::Rgb(pixels.clone(), alpha_blend_rgb),
            DynamicImage::ImageRgba8(pixels) => Self::Rgba(pixels.clone()),
            DynamicImage::ImageLuma16(pixels) => {
                Self::L16(pixels.clone(), l16_draw_uses_big_endian(mode))
            }
            // TODO(pillow-rs#3ptb): Add native draw strategies for these
            // storage modes. Do not widen them to RGBA to share the draw path.
            _ => Self::Rgba(image.to_rgba8()),
        }
    }

    pub(super) fn into_image(self) -> DynamicImage {
        match self {
            Self::L(pixels) => DynamicImage::ImageLuma8(pixels),
            Self::LA(pixels) => DynamicImage::ImageLumaA8(pixels),
            Self::Rgb(pixels, _) => DynamicImage::ImageRgb8(pixels),
            Self::Rgba(pixels) => DynamicImage::ImageRgba8(pixels),
            Self::L16(pixels, _) => DynamicImage::ImageLuma16(pixels),
        }
    }

    pub(super) fn set_alpha_blend_rgb(&mut self, enabled: bool) {
        if let Self::Rgb(_, alpha_blend_rgb) = self {
            *alpha_blend_rgb = enabled;
        }
    }

    pub(super) fn set_l16_ink_order(&mut self, mode: Option<&str>, packed_bytes: bool) {
        if let Self::L16(_, big_endian) = self {
            // Line/point paths in Pillow write the packed little-endian ink
            // bytes verbatim, while the area/arc paths normalize the ink to
            // the destination's declared I;16 byte order. Keep that small C
            // backend distinction explicit at the canvas boundary.
            *big_endian = packed_bytes && l16_draw_uses_big_endian(mode);
        }
    }
}

impl DrawCanvas for NativeDrawCanvas {
    fn width(&self) -> u32 {
        match self {
            Self::L(pixels) => pixels.width(),
            Self::LA(pixels) => pixels.width(),
            Self::Rgb(pixels, _) => pixels.width(),
            Self::Rgba(pixels) => pixels.width(),
            Self::L16(pixels, _) => pixels.width(),
        }
    }

    fn height(&self) -> u32 {
        match self {
            Self::L(pixels) => pixels.height(),
            Self::LA(pixels) => pixels.height(),
            Self::Rgb(pixels, _) => pixels.height(),
            Self::Rgba(pixels) => pixels.height(),
            Self::L16(pixels, _) => pixels.height(),
        }
    }

    fn put_rgba(&mut self, x: u32, y: u32, color: [u8; 4]) {
        match self {
            Self::L(pixels) => {
                pixels.put_pixel(x, y, crate::raster::Luma([color[0]]));
            }
            Self::LA(pixels) => {
                pixels.put_pixel(x, y, crate::raster::LumaA([color[0], color[3]]));
            }
            Self::Rgb(pixels, alpha_blend_rgb) => {
                if *alpha_blend_rgb {
                    let background = pixels.get_pixel(x, y);
                    let alpha = u16::from(color[3]);
                    let inverse_alpha = 255u16 - alpha;
                    let blend = |source: u8, destination: u8| {
                        ((u16::from(source) * alpha + u16::from(destination) * inverse_alpha + 127)
                            / 255) as u8
                    };
                    pixels.put_pixel(
                        x,
                        y,
                        crate::raster::Rgb([
                            blend(color[0], background[0]),
                            blend(color[1], background[1]),
                            blend(color[2], background[2]),
                        ]),
                    );
                } else {
                    pixels.put_pixel(x, y, crate::raster::Rgb([color[0], color[1], color[2]]));
                }
            }
            Self::Rgba(pixels) => {
                pixels.put_pixel(x, y, crate::raster::Rgba(color));
            }
            Self::L16(pixels, big_endian) => {
                // ImageDraw's I;16* color parser supplies the low two bytes
                // of the packed integer ink. The C canvas writes those bytes
                // verbatim; convert them into the host-native u16 sample so
                // the public mode serializer can restore the declared order.
                let raw = u16::from_le_bytes([color[0], color[1]]);
                let sample = if *big_endian { raw.swap_bytes() } else { raw };
                pixels.put_pixel(x, y, crate::raster::Luma([sample]));
            }
        }
    }
}

pub(super) fn l16_draw_uses_big_endian(mode: Option<&str>) -> bool {
    match mode {
        Some("I;16B") => true,
        Some("I;16") | Some("I;16N") => cfg!(target_endian = "big"),
        _ => false,
    }
}

/// Draw directly in the destination's native byte layout.
pub(super) fn draw_native<F>(
    img: &DynamicImage,
    alpha_blend_rgb: bool,
    mode: Option<&str>,
    draw_fn: F,
) -> DynamicImage
where
    F: Fn(&mut NativeDrawCanvas),
{
    let mut canvas = NativeDrawCanvas::from_image(img, alpha_blend_rgb, mode);
    draw_fn(&mut canvas);
    canvas.into_image()
}
