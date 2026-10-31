use super::dispatch::alpha_blend_enabled;
use crate::draw::DrawCanvas;
use crate::pipeline::PipelineOp;
use crate::raster::{ImageBuffer, Luma, Pixel};

/// Direct writer for one exact byte-backed mode strategy.
pub(crate) struct NativeByteCanvas<
    'a,
    P: Pixel<Subpixel = u8>,
    const CHANNELS: usize,
> {
    width: u32,
    height: u32,
    pixels: &'a mut [P],
    alpha_blend_rgb: bool,
}

impl<'a, P: Pixel<Subpixel = u8>, const CHANNELS: usize> NativeByteCanvas<'a, P, CHANNELS>
{
    pub(crate) fn new(width: u32, height: u32, pixels: &'a mut [P]) -> Self {
        Self {
            width,
            height,
            pixels,
            alpha_blend_rgb: false,
        }
    }
}

impl<P: Pixel<Subpixel = u8>, const CHANNELS: usize> DrawCanvas
    for NativeByteCanvas<'_, P, CHANNELS>
{
    fn width(&self) -> u32 {
        self.width
    }

    fn height(&self) -> u32 {
        self.height
    }

    fn put_rgba(&mut self, x: u32, y: u32, color: [u8; 4]) {
        let index = y as usize * self.width as usize + x as usize;
        let alpha_blend_rgb = self.alpha_blend_rgb;
        let channels = self.pixels[index].channels_mut();
        match CHANNELS {
            1 => {
                // Preserve the archived expanded-byte behavior, including for
                // mode `1`: the resolved draw ink is stored without thresholding.
                channels[0] = color[0];
            }
            2 => {
                channels[0] = color[0];
                channels[1] = color[3];
            }
            3 if alpha_blend_rgb => {
                let alpha = u16::from(color[3]);
                let inverse_alpha = 255 - alpha;
                for channel in 0..3 {
                    channels[channel] = ((u16::from(color[channel]) * alpha
                        + u16::from(channels[channel]) * inverse_alpha
                        + 127)
                        / 255) as u8;
                }
            }
            3 => channels[..3].copy_from_slice(&color[..3]),
            4 => channels.copy_from_slice(&color),
            _ => unreachable!("Draw supports byte layouts with one to four channels"),
        }
    }
}

/// Writer for the four Pillow I;16 identities. The exact Mode selects its
/// endian rule before rasterization; the pixel loop only applies that rule.
pub(crate) struct Luma16Canvas<'a, const DECLARED_BIG_ENDIAN: bool> {
    width: u32,
    height: u32,
    pixels: &'a mut [Luma<u16>],
    big_endian_ink: bool,
}

impl<'a, const DECLARED_BIG_ENDIAN: bool> Luma16Canvas<'a, DECLARED_BIG_ENDIAN> {
    pub(crate) fn new(pixels: &'a mut ImageBuffer<Luma<u16>, Vec<u16>>) -> Self {
        Self {
            width: pixels.width(),
            height: pixels.height(),
            pixels: pixels.as_mut(),
            big_endian_ink: DECLARED_BIG_ENDIAN,
        }
    }

    fn set_operation_ink_order(&mut self, op: &PipelineOp) {
        let packed_ink = matches!(op, PipelineOp::DrawLine { .. } | PipelineOp::DrawPoint { .. });
        self.big_endian_ink = DECLARED_BIG_ENDIAN && packed_ink;
    }
}

impl<const DECLARED_BIG_ENDIAN: bool> DrawCanvas for Luma16Canvas<'_, DECLARED_BIG_ENDIAN> {
    fn width(&self) -> u32 {
        self.width
    }

    fn height(&self) -> u32 {
        self.height
    }

    fn put_rgba(&mut self, x: u32, y: u32, color: [u8; 4]) {
        let raw = u16::from_le_bytes([color[0], color[1]]);
        let sample = if self.big_endian_ink {
            raw.swap_bytes()
        } else {
            raw
        };
        self.pixels[y as usize * self.width as usize + x as usize] = Luma([sample]);
    }
}

/// Operation-level state that is configured before entering shared geometry.
pub(crate) trait ConfigureDrawCanvas: DrawCanvas {
    fn configure(&mut self, op: &PipelineOp);
}

impl<P: Pixel<Subpixel = u8>, const CHANNELS: usize> ConfigureDrawCanvas
    for NativeByteCanvas<'_, P, CHANNELS>
{
    fn configure(&mut self, op: &PipelineOp) {
        self.alpha_blend_rgb = CHANNELS == 3 && alpha_blend_enabled(op);
    }
}

impl<const DECLARED_BIG_ENDIAN: bool> ConfigureDrawCanvas
    for Luma16Canvas<'_, DECLARED_BIG_ENDIAN>
{
    fn configure(&mut self, op: &PipelineOp) {
        self.set_operation_ink_order(op);
    }
}
