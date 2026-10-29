//! Public Image quantization methods and algorithm selection.

use std::sync::Arc;

use crate::error::PilError;
use crate::image::{Image, PalettedData, PipelineOps};
use crate::raster::DynamicImage;

use super::input::QuantizePalette;
use super::{
    kmeans_refine, map_pixels_to_fixed_palette, maxcoverage_quantize_rgb, median_cut_quantize_rgb,
    quantize_octree_rgb, quantize_octree_rgba,
};

// ── Image method ──

impl Image {
    /// Applies Pillow's public quantize defaults and validation before the
    /// algorithm-specific implementation runs.
    pub fn quantize_with_input(
        &self,
        colors: Option<i32>,
        method: Option<i32>,
        kmeans: Option<i32>,
        palette: QuantizePalette,
        dither: Option<bool>,
    ) -> Result<Image, PilError> {
        let colors = colors.unwrap_or(256);
        if !(1..=256).contains(&colors) {
            return Err(PilError::ValueError("bad number of colors".to_owned()));
        }
        let source_mode = self.mode()?;
        let method = match method {
            Some(method) => method,
            None if source_mode == "RGBA" => 2,
            None => 0,
        };
        if source_mode == "RGBA" && method != 2 && method != 3 {
            return Err(PilError::ValueError(
                "Fast Octree (method == 2) and libimagequant (method == 3) \
                 are the only valid methods for quantizing RGBA images"
                    .to_owned(),
            ));
        }
        if let QuantizePalette::Image(palette_image) = &palette {
            if palette_image.mode()? != "P" {
                return Err(PilError::ValueError(
                    "bad mode for palette image".to_owned(),
                ));
            }
            if !matches!(source_mode.as_str(), "RGB" | "L") {
                return Err(PilError::ValueError(
                    "only RGB or L mode images can be quantized to a palette".to_owned(),
                ));
            }
            return self.quantize_to_palette(palette_image, dither.unwrap_or(true));
        }
        let kmeans = kmeans.unwrap_or(0);
        if kmeans < 0 {
            return Err(PilError::ValueError(
                "kmeans must not be negative".to_owned(),
            ));
        }
        self.quantize(
            colors as u32,
            kmeans as u32,
            None,
            dither.unwrap_or(true),
            method as u32,
        )
    }

    /// Maps an RGB or L image to the explicitly supplied P-mode palette.
    ///
    /// Pillow's `Image.quantize(palette=...)` preserves the reference palette
    /// and uses its entries as the destination colors. The public wrapper only
    /// classifies the host object; all mode checks, palette copying, and pixel
    /// mapping stay here in the core implementation.
    fn quantize_to_palette(&self, palette_image: &Image, dither: bool) -> Result<Image, PilError> {
        let image = self.materialize()?;
        let rgb = image.to_rgb8();
        let (width, height) = rgb.dimensions();
        let palette_bytes = palette_image.palette().unwrap_or_default();
        let palette: Vec<[u8; 3]> = palette_bytes
            .chunks_exact(3)
            .take(256)
            .map(|entry| [entry[0], entry[1], entry[2]])
            .collect();
        let pixels = rgb.into_raw();
        let indices = map_pixels_to_fixed_palette(&pixels, width, height, &palette, dither)?;
        let indices = crate::raster::GrayImage::from_raw(width, height, indices)
            .ok_or_else(|| PilError::ValueError("palette conversion failed".to_owned()))?;

        Ok(Image::Paletted(PalettedData {
            indices,
            palette: palette_bytes,
            palette_alpha: palette_image.palette_alpha().unwrap_or_default(),
            source_format: None,
            info: None,
            exif: None,
            materialized: crate::image::materialization_cache(),
        }))
    }

    /// Reduces the image to a `P` image with at most `colors` palette entries.
    ///
    /// RGB-family images use Pillow-compatible median cut. RGBA images use the
    /// default Pillow fast-octree path. The returned image carries palette index
    /// bytes plus RGB or RGBA palette data matching the selected path.
    ///
    /// # Errors
    ///
    /// Returns [`PilError::ValueError`] when `colors` is outside 1..=256, or
    /// another [`PilError`] when materialization fails.
    pub fn quantize(
        &self,
        colors: u32,
        kmeans: u32,
        _palette: Option<&Image>,
        _dither: bool,
        method: u32,
    ) -> Result<Image, PilError> {
        validate_color_count(colors)?;
        let image = self.materialize()?;
        let (width, height) = (image.width(), image.height());
        let is_rgba = matches!(image.color(), crate::raster::ColorType::Rgba8)
            || self.explicit_mode() == Some("RGBA");
        validate_method(is_rgba, method)?;
        if method == 3 {
            return Err(PilError::ValueError(
                "dependency required by this method was not enabled at compile time".into(),
            ));
        }

        let quantized = quantize_by_method(
            &image,
            self.explicit_mode(),
            width,
            height,
            colors as usize,
            kmeans,
            is_rgba,
            method,
        )?;
        build_paletted_image(width, height, quantized)
    }
}

struct QuantizedColors {
    indices: Vec<u8>,
    palette: Vec<u8>,
    alpha: Option<Vec<u8>>,
}

fn validate_color_count(colors: u32) -> Result<(), PilError> {
    if !(1..=256).contains(&colors) {
        return Err(PilError::ValueError("bad number of colors".to_owned()));
    }
    Ok(())
}

fn validate_method(is_rgba: bool, method: u32) -> Result<(), PilError> {
    if is_rgba && method != 2 && method != 3 {
        return Err(PilError::ValueError(
            "Fast Octree (method == 2) and libimagequant (method == 3) \
             are the only valid methods for quantizing RGBA images"
                .to_owned(),
        ));
    }
    Ok(())
}

fn quantize_by_method(
    image: &DynamicImage,
    explicit_mode: Option<&str>,
    width: u32,
    height: u32,
    n_colors: usize,
    kmeans: u32,
    is_rgba: bool,
    method: u32,
) -> Result<QuantizedColors, PilError> {
    if is_rgba {
        return quantize_rgba(image, width, height, n_colors);
    }
    match method {
        2 => quantize_octree(image, explicit_mode, width, height, n_colors),
        1 => quantize_maxcoverage(image, n_colors, kmeans),
        _ => quantize_median_cut(image, explicit_mode, n_colors, kmeans),
    }
}

fn quantize_rgba(
    image: &DynamicImage,
    width: u32,
    height: u32,
    n_colors: usize,
) -> Result<QuantizedColors, PilError> {
    let (indices, palette) = match image {
        DynamicImage::ImageRgba8(rgba) => {
            quantize_octree_rgba(rgba.as_raw(), width, height, n_colors, true)?
        }
        _ => {
            let rgba = image.to_rgba8();
            quantize_octree_rgba(rgba.as_raw(), width, height, n_colors, true)?
        }
    };
    Ok(QuantizedColors {
        indices,
        palette: rgba_palette_rgb(&palette),
        alpha: Some(rgba_palette_alpha(&palette)),
    })
}

fn quantize_octree(
    image: &DynamicImage,
    explicit_mode: Option<&str>,
    width: u32,
    height: u32,
    n_colors: usize,
) -> Result<QuantizedColors, PilError> {
    let (indices, palette) = match image {
        DynamicImage::ImageRgb8(rgb) if matches!(explicit_mode, None | Some("RGB")) => {
            quantize_octree_rgb(rgb.as_raw(), width, height, n_colors)?
        }
        _ => {
            let rgba = image.to_rgba8();
            quantize_octree_rgba(rgba.as_raw(), width, height, n_colors, false)?
        }
    };
    Ok(QuantizedColors {
        indices,
        palette: rgba_palette_rgb(&palette),
        alpha: None,
    })
}

fn rgba_palette_rgb(palette: &[u8]) -> Vec<u8> {
    palette
        .chunks_exact(4)
        .flat_map(|color| [color[0], color[1], color[2]])
        .collect()
}

fn rgba_palette_alpha(palette: &[u8]) -> Vec<u8> {
    palette.chunks_exact(4).map(|color| color[3]).collect()
}

fn quantize_maxcoverage(
    image: &DynamicImage,
    n_colors: usize,
    kmeans: u32,
) -> Result<QuantizedColors, PilError> {
    let rgb = image.to_rgb8();
    let (indices, palette) = maxcoverage_quantize_rgb(&rgb.into_raw(), n_colors, kmeans);
    Ok(QuantizedColors {
        indices,
        palette: flatten_rgb_palette(&palette),
        alpha: None,
    })
}

fn quantize_median_cut(
    image: &DynamicImage,
    explicit_mode: Option<&str>,
    n_colors: usize,
    kmeans: u32,
) -> Result<QuantizedColors, PilError> {
    let native_rgb =
        matches!(image, DynamicImage::ImageRgb8(_)) && matches!(explicit_mode, None | Some("RGB"));
    let converted_rgb = (!native_rgb).then(|| image.to_rgb8());
    let pixels = match converted_rgb.as_ref() {
        Some(rgb) => rgb.as_raw().as_slice(),
        None => image.as_bytes(),
    };
    let pixel_list: Vec<[u8; 3]> = if kmeans > 0 {
        pixels
            .chunks_exact(3)
            .map(|pixel| [pixel[0], pixel[1], pixel[2]])
            .collect()
    } else {
        Vec::new()
    };
    let (indices, mut palette) = median_cut_quantize_rgb(pixels, n_colors);
    let indices = refine_median_cut_kmeans(indices, &pixel_list, &mut palette, kmeans);
    Ok(QuantizedColors {
        indices,
        palette: flatten_rgb_palette(&palette),
        alpha: None,
    })
}

fn refine_median_cut_kmeans(
    indices: Vec<u8>,
    pixels: &[[u8; 3]],
    palette: &mut [[u8; 3]],
    kmeans: u32,
) -> Vec<u8> {
    if kmeans == 0 {
        return indices;
    }
    let mut indices: Vec<u32> = indices.into_iter().map(u32::from).collect();
    kmeans_refine(pixels, palette, &mut indices, kmeans - 1);
    indices.into_iter().map(|index| index as u8).collect()
}

fn flatten_rgb_palette(palette: &[[u8; 3]]) -> Vec<u8> {
    palette
        .iter()
        .flat_map(|color| [color[0], color[1], color[2]])
        .collect()
}

fn build_paletted_image(
    width: u32,
    height: u32,
    quantized: QuantizedColors,
) -> Result<Image, PilError> {
    let mut output = crate::raster::GrayImage::new(width, height);
    for (index, pixel) in output.pixels_mut().enumerate() {
        pixel[0] = quantized.indices.get(index).copied().unwrap_or(0);
    }
    Ok(Image::Pipeline {
        source: Arc::new(Image::from_dynamic(
            DynamicImage::ImageLuma8(output),
            Some("P".to_string()),
        )),
        ops: PipelineOps::empty(),
        format: None,
        explicit_mode: Some("P".to_string()),
        mode_selector: Some(crate::image::Mode::ImagePalette8),
        backend: None,
        palette: Some(quantized.palette),
        palette_alpha: quantized.alpha,
        materialized: crate::image::materialization_cache(),
        shape: crate::image::pipeline_shape_cache(),
        mode: crate::image::pipeline_mode_cache(),
    })
}
