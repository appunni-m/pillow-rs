//! Indexed color table access and mutation methods for [`Image`].

use std::sync::Arc;

use image_slash_star::{ImageFormat, ImageInfo};

use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::DynamicImage;

use super::{Image, LoadedData, PalettedData, materialization_cache};

pub(super) fn padded_palette(rgb: &[u8]) -> Vec<u8> {
    let mut palette = rgb.to_vec();
    palette.resize(768, 0);
    palette
}

fn split_palette_data(data: &[u8], rawmode: &str) -> Result<(Vec<u8>, Vec<u8>), PilError> {
    match rawmode {
        "RGB" => {
            let complete = data.len() / 3 * 3;
            if complete > 768 {
                return Err(PilError::ValueError("invalid palette size".to_owned()));
            }
            Ok((data[..complete].to_vec(), Vec::new()))
        }
        "RGBA" => {
            let complete = data.len() / 4 * 4;
            if complete > 1024 {
                return Err(PilError::ValueError("invalid palette size".to_owned()));
            }
            let mut rgb = Vec::with_capacity(complete / 4 * 3);
            let mut alpha = Vec::with_capacity(complete / 4);
            for color in data[..complete].chunks_exact(4) {
                rgb.extend_from_slice(&color[..3]);
                alpha.push(color[3]);
            }
            Ok((rgb, alpha))
        }
        "LA" => {
            // Pillow ImagePalette compiles an LA palette to RGB triples with
            // the L channel replicated and the A channel carried as palette
            // alpha (ImagePalette.py `_raw`/mode transforms); getpalette
            // therefore returns (L,L,L,A) per entry.
            let complete = data.len() / 2 * 2;
            if complete > 512 {
                return Err(PilError::ValueError("invalid palette size".to_owned()));
            }
            let mut rgb = Vec::with_capacity(complete / 2 * 3);
            let mut alpha = Vec::with_capacity(complete / 2);
            for color in data[..complete].chunks_exact(2) {
                rgb.extend_from_slice(&[color[0], color[0], color[0]]);
                alpha.push(color[1]);
            }
            Ok((rgb, alpha))
        }
        _ => Err(PilError::ValueError("unrecognized raw mode".to_owned())),
    }
}

pub(crate) fn expand_palette(
    indices: &crate::raster::GrayImage,
    palette: &[u8],
    palette_alpha: &[u8],
) -> DynamicImage {
    if palette_alpha.is_empty() {
        return DynamicImage::ImageRgb8(crate::raster::RgbImage::from_fn(
            indices.width(),
            indices.height(),
            |x, y| {
                let index = usize::from(indices.get_pixel(x, y)[0]);
                let base = index * 3;
                crate::raster::Rgb([
                    palette.get(base).copied().unwrap_or(0),
                    palette.get(base + 1).copied().unwrap_or(0),
                    palette.get(base + 2).copied().unwrap_or(0),
                ])
            },
        ));
    }
    DynamicImage::ImageRgba8(crate::raster::RgbaImage::from_fn(
        indices.width(),
        indices.height(),
        |x, y| {
            let index = usize::from(indices.get_pixel(x, y)[0]);
            let base = index * 3;
            crate::raster::Rgba([
                palette.get(base).copied().unwrap_or(0),
                palette.get(base + 1).copied().unwrap_or(0),
                palette.get(base + 2).copied().unwrap_or(0),
                palette_alpha.get(index).copied().unwrap_or(255),
            ])
        },
    ))
}

/// Expands Pillow `PA` samples through their RGB palette.
///
/// Unlike `P`, `PA` owns alpha per pixel. Pillow therefore ignores any alpha
/// attached to the palette while converting `PA` to `RGBA`.
pub(crate) fn expand_palette_alpha(
    indices_alpha: &crate::raster::GrayAlphaImage,
    palette: &[u8],
) -> DynamicImage {
    let mut rgb_by_index = [[0u8; 3]; 256];
    for (index, rgb) in rgb_by_index.iter_mut().enumerate() {
        let base = index * 3;
        *rgb = [
            palette.get(base).copied().unwrap_or(0),
            palette.get(base + 1).copied().unwrap_or(0),
            palette.get(base + 2).copied().unwrap_or(0),
        ];
    }

    let samples = indices_alpha.as_raw();
    if let Some(output_len) = samples.len().checked_mul(2) {
        let mut output = Vec::with_capacity(output_len);
        for sample in samples.chunks_exact(2) {
            let rgb = rgb_by_index[usize::from(sample[0])];
            output.extend_from_slice(&[rgb[0], rgb[1], rgb[2], sample[1]]);
        }
        if let Some(image) = crate::raster::RgbaImage::from_raw(
            indices_alpha.width(),
            indices_alpha.height(),
            output,
        ) {
            return DynamicImage::ImageRgba8(image);
        }
    }

    // Preserve the image crate's checked construction fallback if dimensions
    // cannot be represented by a flat RGBA allocation.
    DynamicImage::ImageRgba8(crate::raster::RgbaImage::from_fn(
        indices_alpha.width(),
        indices_alpha.height(),
        |x, y| {
            let pixel = indices_alpha.get_pixel(x, y);
            let rgb = rgb_by_index[usize::from(pixel[0])];
            crate::raster::Rgba([rgb[0], rgb[1], rgb[2], pixel[1]])
        },
    ))
}

/// Return mutable palette state without treating internal zero padding as
/// encoded palette entries.
pub(super) fn operational_palette(data: &PalettedData) -> Vec<u8> {
    let Some(source_palette) = data
        .info
        .as_ref()
        .and_then(|info| info.palette.as_ref())
        .map(|palette| &palette.rgb)
    else {
        return data.palette.clone();
    };
    if data.palette.starts_with(source_palette)
        && data.palette[source_palette.len()..]
            .iter()
            .all(|&component| component == 0)
    {
        source_palette.clone()
    } else {
        data.palette.clone()
    }
}

impl Image {
    /// Returns the explicit Pillow mode tag carried by this image.
    ///
    /// Some Pillow modes cannot be represented by the generic raster variants
    /// alone. This method exposes the side-channel mode tag for modes such as
    /// `"1"`, `"P"`, `"CMYK"`, `"HSV"`, `"YCbCr"`, `"I"`, and `"F"`.
    pub fn explicit_mode(&self) -> Option<&str> {
        match self {
            Image::Loaded(LoadedData {
                explicit_mode: Some(m),
                ..
            }) => Some(m.as_str()),
            Image::Paletted(_) => Some("P"),
            Image::Pipeline {
                explicit_mode: Some(m),
                ..
            } => Some(m.as_str()),
            _ => None,
        }
    }

    /// Returns attached palette data as RGB triples.
    ///
    /// A palette contains up to 768 bytes: 256 entries of `R, G, B`. Pipeline
    /// images can carry a copied palette so palette-safe operations preserve
    /// `P` mode.
    pub fn palette(&self) -> Option<Vec<u8>> {
        match self {
            Image::Paletted(data) => Some(data.palette.clone()),
            Image::Loaded(data) => data.palette.clone(),
            Image::Pipeline { palette, .. } => palette.clone(),
            Image::Bytes { info, .. } => info
                .as_ref()
                .and_then(|info| info.palette.as_ref())
                .map(|palette| palette.rgb.clone()),
        }
    }

    /// Returns retained per-entry alpha values for an indexed image.
    pub fn palette_alpha(&self) -> Option<Vec<u8>> {
        match self {
            Image::Paletted(data) => Some(data.palette_alpha.clone()),
            Image::Loaded(data) => data.palette_alpha.clone(),
            Image::Pipeline { palette_alpha, .. } => palette_alpha.clone(),
            Image::Bytes { info, .. } => info
                .as_ref()
                .and_then(|info| info.palette.as_ref())
                .map(|palette| palette.alpha.clone()),
        }
    }

    /// Returns immutable metadata from the original encoded source.
    ///
    /// Pipeline results retain this provenance; current dimensions and mode
    /// are reported by [`Image::size`] and [`Image::mode`] instead.
    pub fn image_info(&self) -> Option<ImageInfo> {
        match self {
            Image::Loaded(data) => data.info.clone(),
            Image::Paletted(data) => data.info.clone(),
            Image::Bytes { info, .. } => info.clone(),
            Image::Pipeline { source, .. } => source.image_info(),
        }
    }

    pub(crate) fn source_format(&self) -> Option<ImageFormat> {
        match self {
            Image::Loaded(data) => data.source_format,
            Image::Paletted(data) => data.source_format,
            Image::Bytes { format, .. } | Image::Pipeline { format, .. } => *format,
        }
    }

    /// Returns the exact retained RGB palette like Pillow's `getpalette`.
    ///
    /// Palette length is structural, not inferred from color values. In
    /// particular, encoded GIF color tables and explicitly attached palettes
    /// retain trailing black entries.
    pub fn getpalette_trimmed(&self) -> Option<Vec<u8>> {
        self.extract_palette()
    }

    /// Returns palette bytes converted to RGBA, trimmed to the retained RGB
    /// palette length.
    pub fn getpalette_rgba(&self) -> Option<Vec<u8>> {
        let palette = self.getpalette_trimmed()?;
        let alpha = if self.pending_palette_transparency().is_none() {
            self.palette_alpha().unwrap_or_default()
        } else {
            Vec::new()
        };
        let mut rgba = Vec::with_capacity(palette.len() / 3 * 4);
        for (index, color) in palette.chunks_exact(3).enumerate() {
            rgba.extend_from_slice(color);
            rgba.push(alpha.get(index).copied().unwrap_or(255));
        }
        Some(rgba)
    }

    /// Converts the retained RGB palette to a Pillow ``getpalette(rawmode)``
    /// layout.
    ///
    /// Pillow's C `getpalette` accepts ``"RGB"``, ``"RGBA"`` (real per-entry
    /// alpha), ``"RGBX"`` (alpha forced to 255, only for palettes without
    /// alpha data), and the single-channel selectors ``"R"``/``"G"``/``"B"``;
    /// every other rawmode raises ``ValueError: unrecognized raw mode``.
    pub fn getpalette_rawmode(&self, rawmode: &str) -> Result<Option<Vec<u8>>, PilError> {
        let Some(palette) = self.getpalette_trimmed() else {
            return Ok(None);
        };
        // Pillow's Image.getpalette reads alpha from the committed palette
        // object, not from pending info["transparency"] decoded alongside a
        // P image. Keep the pending marker available to Image.info and
        // apply_transparency, but do not expose it as RGBA palette bytes yet.
        let palette_alpha = if self.pending_palette_transparency().is_none() {
            self.palette_alpha().unwrap_or_default()
        } else {
            Vec::new()
        };
        let has_alpha = !palette_alpha.is_empty();
        match rawmode {
            "RGB" => Ok(Some(palette)),
            "RGBA" => Ok(self.getpalette_rgba()),
            "RGBX" => {
                // Pillow has no RGBA->RGBX packer, so palettes carrying alpha
                // data raise "unrecognized raw mode" for RGBX; palettes
                // without alpha pack to RGB plus a 255 pad byte.
                if has_alpha {
                    return Err(PilError::ValueError("unrecognized raw mode".into()));
                }
                let mut out = Vec::with_capacity(palette.len() / 3 * 4);
                for color in palette.chunks_exact(3) {
                    out.extend_from_slice(color);
                    out.push(255);
                }
                Ok(Some(out))
            }
            "R" | "G" | "B" => {
                let channel = match rawmode {
                    "R" => 0,
                    "G" => 1,
                    _ => 2,
                };
                Ok(Some(
                    palette
                        .chunks_exact(3)
                        .map(|color| color[channel])
                        .collect(),
                ))
            }
            _ => Err(PilError::ValueError("unrecognized raw mode".into())),
        }
    }

    /// Returns palette data using Pillow's public raw-mode and missing-palette
    /// behavior.
    pub fn getpalette_with_input(
        &self,
        rawmode: Option<&str>,
    ) -> Result<Option<Vec<u8>>, PilError> {
        let rawmode = rawmode.or_else(|| self.palette_mode()).unwrap_or("RGB");
        let palette = self.getpalette_rawmode(rawmode)?;
        if palette.is_none() && matches!(self.mode()?.as_str(), "P" | "PA") {
            return Ok(Some(Vec::new()));
        }
        Ok(palette)
    }

    /// Returns RGB triples for an indexed Qt-compatible color table.
    ///
    /// Qt owns the final `qRgb` packing and `QImage` object construction; the
    /// Rust core owns the mode-aware table values so palette traversal is
    /// shared with every binding.
    pub fn indexed_color_table(&self, mode: &str) -> Result<Vec<(u8, u8, u8)>, PilError> {
        match mode {
            "L" => Ok((0..=u8::MAX).map(|value| (value, value, value)).collect()),
            "P" => Ok(self
                .getpalette_with_input(Some("RGB"))?
                .unwrap_or_default()
                .chunks_exact(3)
                .map(|color| (color[0], color[1], color[2]))
                .collect()),
            _ => Ok(Vec::new()),
        }
    }

    /// Attaches a palette without changing the image's sample bytes.
    ///
    /// Pillow reinterprets `L` samples as palette indices (`P`) and `LA`
    /// samples as index/alpha pairs (`PA`). Existing `P`/`PA` samples are also
    /// retained byte-for-byte while their palette is replaced. The mutation is
    /// stored in this core value rather than binding-only shadow state. `P`
    /// images therefore enter later CPU, SIMD, or GPU work through the existing
    /// shared indexed representation.
    ///
    /// # Errors
    ///
    /// Returns [`PilError::ValueError`] for an illegal image mode, unsupported
    /// raw palette mode, or an oversized palette.
    pub fn putpalette(&mut self, data: &[u8], rawmode: &str) -> Result<(), PilError> {
        let mode = self.mode()?;
        if !matches!(mode.as_str(), "L" | "LA" | "P" | "PA") {
            return Err(PilError::ValueError("illegal image mode".to_owned()));
        }
        let (palette, palette_alpha) = split_palette_data(data, rawmode)?;
        let source_format = self.source_format();
        let info = self.image_info();
        let exif = self.exif_metadata();
        let materialized = self.materialize()?;

        *self = match mode.as_str() {
            "L" | "P" => Image::Paletted(PalettedData {
                indices: materialized.to_luma8(),
                palette,
                palette_alpha,
                source_format,
                info,
                exif,
                materialized: materialization_cache(),
            }),
            "LA" | "PA" => {
                let image = DynamicImage::ImageLumaA8(materialized.to_luma_alpha8());
                let decoded_mode = image.color().into();
                let mode = super::Mode::for_image(&image, Some("PA"));
                Image::Loaded(LoadedData {
                    image: Arc::new(image),
                    mode,
                    known_zero_fill: false,
                    explicit_mode: Some("PA".to_owned()),
                    decoded_mode,
                    palette: Some(palette),
                    palette_alpha: Some(palette_alpha),
                    source_format,
                    info,
                    exif,
                })
            }
            _ => unreachable!("putpalette mode was validated"),
        };
        Ok(())
    }
}

impl Image {
    /// Extract palette from Paletted variant (for Pipeline propagation).
    pub(crate) fn extract_palette(&self) -> Option<Vec<u8>> {
        match self {
            Image::Paletted(data) => Some(operational_palette(data)),
            Image::Loaded(data) => data.palette.clone(),
            Image::Pipeline { palette, .. } => palette.clone(),
            Image::Bytes { info, .. } => info
                .as_ref()
                .and_then(|info| info.palette.as_ref())
                .map(|palette| palette.rgb.clone()),
        }
    }

    /// Remaps palette indices through `dest_map`.
    ///
    /// `P` images operate on palette indices directly. Other modes operate on
    /// RGB color values and return a `P`-tagged pipeline result. Values absent
    /// from `dest_map` map to `0`, matching Pillow's inverse-map behavior.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(Image)`; invalid map behavior is handled during
    /// pipeline execution.
    pub fn remap_palette(&self, dest_map: &[u8]) -> Result<Image, PilError> {
        self.remap_palette_with_source(dest_map, None)
    }

    /// Remaps indices and installs colors selected from an optional source palette.
    ///
    /// Pillow interprets `dest_map[new_index]` as the old index. Pixel samples
    /// therefore use the inverse mapping, while palette triples are copied in
    /// `dest_map` order.
    pub fn remap_palette_with_source(
        &self,
        dest_map: &[u8],
        source_palette: Option<&[u8]>,
    ) -> Result<Image, PilError> {
        let mode = self.mode()?;
        if !matches!(mode.as_str(), "L" | "P") {
            return Err(PilError::ValueError("illegal image mode".to_owned()));
        }
        if dest_map.len() > 256 {
            return Err(PilError::ValueError(
                "byte must be in range(0, 256)".to_owned(),
            ));
        }

        let uses_attached_palette = source_palette.is_none() && mode == "P";
        // Pillow's Image.remap_palette treats every explicit palette longer
        // than 768 bytes as interleaved RGBA, regardless of the source mode.
        let source_bands = if source_palette.is_some_and(|palette| palette.len() > 768) {
            4
        } else {
            3
        };
        let source_palette = source_palette.map_or_else(
            || {
                if mode == "P" {
                    self.extract_palette().unwrap_or_default()
                } else {
                    (0u8..=u8::MAX)
                        .flat_map(|value| [value, value, value])
                        .collect()
                }
            },
            <[u8]>::to_vec,
        );
        let mut remapped_palette = Vec::with_capacity(dest_map.len() * 3);
        let mut explicit_alpha = (source_bands == 4).then(|| Vec::with_capacity(dest_map.len()));
        for &old_index in dest_map {
            let start = usize::from(old_index) * source_bands;
            if let Some(color) = source_palette.get(start..start + source_bands) {
                // Pillow 12.2 PIL.Image.remap_palette passes raw palette bands
                // through unchanged. Core stores the same RGBA layout as RGB
                // triples plus a per-entry alpha sidecar.
                remapped_palette.extend_from_slice(&color[..3]);
                if let Some(alpha) = &mut explicit_alpha {
                    alpha.push(color[3]);
                }
            }
        }

        let remapped_alpha = if source_bands == 4 {
            explicit_alpha
        } else if uses_attached_palette {
            self.palette_alpha().and_then(|alpha| {
                if alpha.is_empty() {
                    return None;
                }
                Some(
                    dest_map
                        .iter()
                        .map(|&index| alpha.get(usize::from(index)).copied().unwrap_or(255))
                        .collect(),
                )
            })
        } else {
            None
        };

        let mut result = Image::push_op(
            self,
            PipelineOp::RemapPalette {
                dest_map: dest_map.to_vec().into(),
            },
        );
        if let Image::Pipeline {
            explicit_mode,
            mode_selector,
            palette,
            palette_alpha,
            ..
        } = &mut result
        {
            *explicit_mode = Some("P".to_owned());
            *mode_selector = Some(super::Mode::ImagePalette8);
            *palette = Some(remapped_palette);
            *palette_alpha = remapped_alpha;
        }
        Ok(result)
    }
}
