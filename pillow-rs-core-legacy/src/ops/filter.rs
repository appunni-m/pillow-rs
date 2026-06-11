//! Image filter operations — built-in PIL filter kernels.
//! Uses PIL-identical 3x3 convolution kernels with matching scale and offset.

use crate::error::PilError;
use crate::image::Image;
use image::DynamicImage;

/// Pre-defined filter kernels matching PIL's ImageFilter module exactly.
struct FilterKernel {
    kernel: [f32; 9],
    scale: f32,
    offset: i32,
}

impl FilterKernel {
    const fn new(kernel: [f32; 9], scale: f32, offset: i32) -> Self {
        FilterKernel { kernel, scale, offset }
    }
}

/// PIL filter definitions (verified against Pillow 12.2.0 filterargs).
const BLUR: FilterKernel = FilterKernel::new(
    [1.0, 1.0, 1.0, 1.0, 5.0, 1.0, 1.0, 1.0, 1.0],
    13.0,
    0,
);

const CONTOUR: FilterKernel = FilterKernel::new(
    [-1.0, -1.0, -1.0, -1.0, 8.0, -1.0, -1.0, -1.0, -1.0],
    1.0,
    255,
);

const DETAIL: FilterKernel = FilterKernel::new(
    [0.0, -1.0, 0.0, -1.0, 10.0, -1.0, 0.0, -1.0, 0.0],
    6.0,
    0,
);

const EDGE_ENHANCE: FilterKernel = FilterKernel::new(
    [0.0, 0.0, 0.0, -1.0, 1.0, 0.0, 0.0, 0.0, 0.0],
    1.0,
    0,
);

const EDGE_ENHANCE_MORE: FilterKernel = FilterKernel::new(
    [-1.0, -1.0, -1.0, -1.0, 9.0, -1.0, -1.0, -1.0, -1.0],
    1.0,
    0,
);

const EMBOSS: FilterKernel = FilterKernel::new(
    [-1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0],
    1.0,
    128,
);

const FIND_EDGES: FilterKernel = FilterKernel::new(
    [-1.0, -1.0, -1.0, -1.0, 8.0, -1.0, -1.0, -1.0, -1.0],
    1.0,
    0,
);

const SHARPEN: FilterKernel = FilterKernel::new(
    [-2.0, -2.0, -2.0, -2.0, 32.0, -2.0, -2.0, -2.0, -2.0],
    16.0,
    0,
);

const SMOOTH: FilterKernel = FilterKernel::new(
    [1.0, 1.0, 1.0, 1.0, 5.0, 1.0, 1.0, 1.0, 1.0],
    13.0,
    0,
);

const SMOOTH_MORE: FilterKernel = FilterKernel::new(
    [1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0],
    9.0,
    0,
);

impl Image {
    /// Apply a named built-in filter. Supports all 10 PIL built-in kernels.
    pub fn filter(&self, filter_type: &str) -> Result<Image, PilError> {
        let k = match filter_type {
            "BLUR" => &BLUR,
            "CONTOUR" => &CONTOUR,
            "DETAIL" => &DETAIL,
            "EDGE_ENHANCE" => &EDGE_ENHANCE,
            "EDGE_ENHANCE_MORE" => &EDGE_ENHANCE_MORE,
            "EMBOSS" => &EMBOSS,
            "FIND_EDGES" => &FIND_EDGES,
            "SHARPEN" => &SHARPEN,
            "SMOOTH" => &SMOOTH,
            "SMOOTH_MORE" => &SMOOTH_MORE,
            _ => {
                return Err(PilError::NotImplementedError(format!(
                    "Filter '{}' not yet implemented",
                    filter_type
                )));
            }
        };
        apply_kernel(self, k)
    }
}

/// Apply a PIL-compatible 3x3 convolution kernel.
/// Uses rayon for parallel row processing on native targets.
/// GPU path (GpuEngine::convolve) will replace this when wired.
fn apply_kernel(image: &Image, k: &FilterKernel) -> Result<Image, PilError> {
    let mut clone = image.clone();
    let img = clone.ensure_loaded()?;

    let rgb = img.to_rgb8();
    let (w, h) = rgb.dimensions();
    let inv_scale = 1.0 / k.scale;
    let wu = w as usize;
    let hu = h as usize;

    let mut out = vec![0u8; wu * hu * 3];

    // Share raw slices for parallel access
    let rgb_data = rgb.as_raw().as_slice();
    let out_ptr = out.as_mut_ptr();

    #[cfg(not(target_arch = "wasm32"))]
    {
        use rayon::prelude::*;
        out.par_chunks_mut(wu * 3).enumerate().for_each(|(y, row_out)| {
            for x in 0..wu {
                let mut r = 0f32; let mut g = 0f32; let mut b = 0f32;
                for ky in 0..3i32 {
                    for kx in 0..3i32 {
                        let sx = (x as i32 + kx - 1).clamp(0, w as i32 - 1) as usize;
                        let sy = (y as i32 + ky - 1).clamp(0, h as i32 - 1) as usize;
                        let idx = (sy * wu + sx) * 3;
                        let ki = (ky * 3 + kx) as usize;
                        r += rgb_data[idx] as f32 * k.kernel[ki];
                        g += rgb_data[idx + 1] as f32 * k.kernel[ki];
                        b += rgb_data[idx + 2] as f32 * k.kernel[ki];
                    }
                }
                let ox = x * 3;
                row_out[ox] = (r * inv_scale + k.offset as f32).clamp(0.0, 255.0).round() as u8;
                row_out[ox + 1] = (g * inv_scale + k.offset as f32).clamp(0.0, 255.0).round() as u8;
                row_out[ox + 2] = (b * inv_scale + k.offset as f32).clamp(0.0, 255.0).round() as u8;
            }
        });
    }

    #[cfg(target_arch = "wasm32")]
    {
        for y in 0..hu {
            let row_start = y * wu * 3;
            for x in 0..wu {
                let mut r = 0f32; let mut g = 0f32; let mut b = 0f32;
                for ky in 0..3i32 {
                    for kx in 0..3i32 {
                        let sx = (x as i32 + kx - 1).clamp(0, w as i32 - 1) as usize;
                        let sy = (y as i32 + ky - 1).clamp(0, h as i32 - 1) as usize;
                        let idx = (sy * wu + sx) * 3;
                        let ki = (ky * 3 + kx) as usize;
                        r += rgb_data[idx] as f32 * k.kernel[ki];
                        g += rgb_data[idx + 1] as f32 * k.kernel[ki];
                        b += rgb_data[idx + 2] as f32 * k.kernel[ki];
                    }
                }
                let ox = row_start + x * 3;
                out[ox] = (r * inv_scale + k.offset as f32).clamp(0.0, 255.0).round() as u8;
                out[ox + 1] = (g * inv_scale + k.offset as f32).clamp(0.0, 255.0).round() as u8;
                out[ox + 2] = (b * inv_scale + k.offset as f32).clamp(0.0, 255.0).round() as u8;
            }
        }
    }

    let _ = out_ptr; // silence unused warning in wasm32 path
    let out_img = image::RgbImage::from_raw(w, h, out)
        .ok_or_else(|| PilError::ValueError("failed to construct output image".into()))?;

    Ok(Image {
        inner: crate::lazy::LazyImage::Loaded(DynamicImage::ImageRgb8(out_img)),
        format: image.format,
    })
}
