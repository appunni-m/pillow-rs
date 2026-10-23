use crate::error::PilError;
use crate::pipeline::PixelMode;
use crate::raster::{DynamicImage, GenericImageView, RgbaImage};

fn table_index_3d(x: usize, y: usize, z: usize, sx: usize, sxy: usize) -> usize {
    x + y * sx + z * sxy
}

fn color_lut_interpolate(a: i16, b: i16, shift: i32) -> i16 {
    const SHIFT_BITS: i32 = 15;
    let value = (i64::from(a) * i64::from((1 << SHIFT_BITS) - shift)
        + i64::from(b) * i64::from(shift))
        >> SHIFT_BITS;
    value as i16
}

pub fn op_color3dlut(
    img: &DynamicImage,
    size: (u32, u32, u32),
    table: &[f64],
    channels: u32,
    source_mode: PixelMode,
    target_mode: PixelMode,
) -> Result<DynamicImage, PilError> {
    let (sx, sy, sz) = (size.0 as usize, size.1 as usize, size.2 as usize);
    let ch = channels as usize;
    let sxy = sx * sy;

    let (w, h) = img.dimensions();
    const PRECISION_BITS: i32 = 6;
    const SCALE_BITS: u32 = 18;
    const SCALE_MASK: u32 = (1 << SCALE_BITS) - 1;
    const SHIFT_BITS: u32 = 15;
    let scales = [
        ((sx - 1) as f64 / 255.0 * f64::from(1 << SCALE_BITS)) as u32,
        ((sy - 1) as f64 / 255.0 * f64::from(1 << SCALE_BITS)) as u32,
        ((sz - 1) as f64 / 255.0 * f64::from(1 << SCALE_BITS)) as u32,
    ];
    // Pillow converts Python sequences to float32 before preparing the signed
    // 10.6 fixed-point entries in `_prepare_lut_table`.
    let prepared: Vec<i16> = table
        .iter()
        .map(|value| {
            let item = *value as f32;
            let scaled = item * ((255 << PRECISION_BITS) as f32);
            if scaled >= i16::MAX as f32 - 0.5 {
                i16::MAX
            } else if scaled <= i16::MIN as f32 + 0.5 {
                i16::MIN
            } else if item < 0.0 {
                (scaled - 0.5) as i16
            } else {
                (scaled + 0.5) as i16
            }
        })
        .collect();

    let mut out = vec![0u8; (w * h) as usize * 4];

    for y in 0..h {
        for x in 0..w {
            let out_idx = ((y * w + x) as usize) * 4;
            let px = img.get_pixel(x, y).0;

            let indices = [
                u32::from(px[0]) * scales[0],
                u32::from(px[1]) * scales[1],
                u32::from(px[2]) * scales[2],
            ];
            let shifts =
                indices.map(|index| ((SCALE_MASK & index) >> (SCALE_BITS - SHIFT_BITS)) as i32);
            let base = table_index_3d(
                (indices[0] >> SCALE_BITS) as usize,
                (indices[1] >> SCALE_BITS) as usize,
                (indices[2] >> SCALE_BITS) as usize,
                sx,
                sxy,
            ) * ch;

            for c in 0..ch {
                let left_left =
                    color_lut_interpolate(prepared[base + c], prepared[base + ch + c], shifts[0]);
                let left_right = color_lut_interpolate(
                    prepared[base + sx * ch + c],
                    prepared[base + sx * ch + ch + c],
                    shifts[0],
                );
                let left = color_lut_interpolate(left_left, left_right, shifts[1]);
                let right_left = color_lut_interpolate(
                    prepared[base + sxy * ch + c],
                    prepared[base + sxy * ch + ch + c],
                    shifts[0],
                );
                let right_right = color_lut_interpolate(
                    prepared[base + sxy * ch + sx * ch + c],
                    prepared[base + sxy * ch + sx * ch + ch + c],
                    shifts[0],
                );
                let right = color_lut_interpolate(right_left, right_right, shifts[1]);
                let result = color_lut_interpolate(left, right, shifts[2]);
                out[out_idx + c] = ((i32::from(result) + (1 << (PRECISION_BITS - 1)))
                    >> PRECISION_BITS)
                    .clamp(0, 255) as u8;
            }
            if ch == 3 {
                out[out_idx + 3] = if source_mode.channels() == 4 {
                    px[3]
                } else {
                    255
                };
            }
        }
    }

    let rgba = RgbaImage::from_raw(w, h, out)
        .ok_or_else(|| PilError::InternalError("color3dlut output size mismatch".into()))?;
    match target_mode {
        PixelMode::RGB => Ok(DynamicImage::ImageRgb8(crate::raster::RgbImage::from_fn(
            w,
            h,
            |x, y| {
                let pixel = rgba.get_pixel(x, y);
                crate::raster::Rgb([pixel[0], pixel[1], pixel[2]])
            },
        ))),
        PixelMode::RGBA | PixelMode::CMYK => Ok(DynamicImage::ImageRgba8(rgba)),
        _ => Err(PilError::InternalError(
            "validated color3dlut target mode was not RGB, RGBA, or CMYK".into(),
        )),
    }
}
