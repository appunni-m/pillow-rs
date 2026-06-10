//! Image quantization — reduce color palette using NeuQuant algorithm.

use image::{DynamicImage, RgbImage};
use color_quant::NeuQuant;

use crate::error::PilError;
use crate::image::Image;

impl Image {
    /// Reduce the number of colors in the image using median cut or NeuQuant.
    /// PIL-compatible: `quantize(colors=256, method=None, kmeans=0, palette=None, dither=1)`.
    pub fn quantize(
        &self,
        colors: u32,
        _kmeans: u32,
        _palette: Option<&Image>,
        dither: bool,
    ) -> Result<Image, PilError> {
        let colors = colors.clamp(2, 256) as usize;
        let mut clone = self.clone();
        let img = clone.ensure_loaded()?;

        let rgb = img.to_rgb8();
        let (w, h) = rgb.dimensions();
        let rgba: Vec<u8> = rgb
            .pixels()
            .flat_map(|p| [p[0], p[1], p[2], 255u8])
            .collect();

        let nq = NeuQuant::new(10, colors, &rgba);
        let palette = nq.color_map_rgb();

        let mut out = RgbImage::new(w, h);
        for (out_px, in_px) in out.pixels_mut().zip(rgb.pixels()) {
            let (idx, _) = nearest_color(&palette, in_px[0], in_px[1], in_px[2]);
            let base = (idx as usize) * 3;
            *out_px = image::Rgb([
                palette[base],
                palette[base + 1],
                palette[base + 2],
            ]);
        }

        Ok(Image {
            inner: crate::lazy::LazyImage::Loaded(DynamicImage::ImageRgb8(out)),
            format: self.format,
        })
    }
}

/// Find the nearest palette color using Euclidean distance in RGB space.
fn nearest_color(palette: &[u8], r: u8, g: u8, b: u8) -> (u8, (u8, u8, u8)) {
    let mut min_dist = u32::MAX;
    let mut best_idx = 0u8;
    let mut best_color = (0u8, 0u8, 0u8);

    for (i, chunk) in palette.chunks(3).enumerate() {
        let dr = r as i32 - chunk[0] as i32;
        let dg = g as i32 - chunk[1] as i32;
        let db = b as i32 - chunk[2] as i32;
        let dist = (dr * dr + dg * dg + db * db) as u32;
        if dist < min_dist {
            min_dist = dist;
            best_idx = i as u8;
            best_color = (chunk[0], chunk[1], chunk[2]);
        }
    }
    (best_idx, best_color)
}
