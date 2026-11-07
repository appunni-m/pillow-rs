//! Indexed color table access and mutation methods for [`Image`].

use std::sync::Arc;

use image_slash_star::{ImageFormat, ImageInfo};

use crate::compute::registry::support::PaletteInput;
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, Mode};

use super::{Image, LoadedData, PalettedData, declared_pipeline_mode, materialization_cache};

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
        "PA" => {
            // Pillow normalizes alpha-bearing palette identities to RGBA,
            // but has no PA-to-RGBA raw unpacker; preserve that rejection.
            Err(PilError::ValueError("unrecognized raw mode".to_owned()))
        }
        _ => Err(PilError::ValueError("unrecognized raw mode".to_owned())),
    }
}

fn clear_pending_palette_alpha(info: &mut Option<ImageInfo>) {
    let Some(palette) = info.as_mut().and_then(|info| info.palette.as_mut()) else {
        return;
    };
    palette.alpha.clear();
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
    /// Returns a mode explicitly carried by this materialized wrapper or
    /// pending pipeline, without projecting it through a Pillow spelling.
    pub(crate) fn explicit_mode_identity(&self) -> Option<crate::raster::Mode> {
        match self {
            Image::Loaded(data) => Some(data.image.mode()),
            Image::Paletted(_) => Some(crate::raster::Mode::ImagePalette8),
            Image::Pipeline { mode, .. } => declared_pipeline_mode(mode),
            _ => None,
        }
    }

    /// Returns the exact Pillow mode carried by materialized pixels or a lazy
    /// pipeline's pending output.
    pub fn explicit_mode(&self) -> Option<&str> {
        self.explicit_mode_identity()
            .and_then(super::mode_boundary::pillow_mode_name)
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
            Image::Pipeline { palette, ops, .. } => palette
                .clone()
                .or_else(|| ops.palette_output()?.palette.clone()),
            Image::Bytes { info, .. } => info
                .as_ref()
                .and_then(|info| info.palette.as_ref())
                .map(|palette| palette.rgb.clone()),
        }
    }

    /// Returns whether palette metadata is attached, including an empty table.
    #[doc(hidden)]
    pub fn has_attached_palette(&self) -> bool {
        match self {
            Image::Paletted(_) => true,
            Image::Loaded(data) => data.palette.is_some(),
            Image::Bytes { info, .. } => info
                .as_ref()
                .is_some_and(|info| info.palette.is_some()),
            Image::Pipeline { palette, ops, .. } => {
                palette.is_some()
                    || ops
                        .palette_output()
                        .is_some_and(|output| output.palette.is_some())
            }
        }
    }

    /// Borrows the resolved palette dependency for mode-specific operations.
    pub(crate) fn palette_input(&self) -> PaletteInput<'_> {
        match self {
            Image::Paletted(data) => PaletteInput {
                rgb: Some(&data.palette),
                alpha: &data.palette_alpha,
            },
            Image::Loaded(data) => PaletteInput {
                rgb: data.palette.as_deref(),
                alpha: data.palette_alpha.as_deref().unwrap_or_default(),
            },
            Image::Bytes { info, .. } => info
                .as_ref()
                .and_then(|info| info.palette.as_ref())
                .map(|palette| PaletteInput {
                    rgb: Some(&palette.rgb),
                    alpha: &palette.alpha,
                })
                .unwrap_or_default(),
            Image::Pipeline {
                palette,
                palette_alpha,
                ops,
                ..
            } => {
                let output = ops.palette_output();
                PaletteInput {
                    rgb: palette
                        .as_deref()
                        .or_else(|| output.and_then(|output| output.palette.as_deref())),
                    alpha: palette_alpha
                        .as_deref()
                        .or_else(|| output.and_then(|output| output.palette_alpha.as_deref()))
                        .unwrap_or_default(),
                }
            }
        }
    }

    /// Returns whether a nonempty RGB palette is attached without cloning it.
    pub(crate) fn has_nonempty_palette(&self) -> bool {
        match self {
            Image::Paletted(data) => !data.palette.is_empty(),
            Image::Loaded(data) => data
                .palette
                .as_ref()
                .is_some_and(|palette| !palette.is_empty()),
            Image::Pipeline { palette, ops, .. } => palette
                .as_ref()
                .or_else(|| ops.palette_output().and_then(|output| output.palette.as_ref()))
                .is_some_and(|palette| !palette.is_empty()),
            Image::Bytes { info, .. } => info
                .as_ref()
                .and_then(|info| info.palette.as_ref())
                .is_some_and(|palette| !palette.rgb.is_empty()),
        }
    }

    /// Returns retained per-entry alpha values for an indexed image.
    pub fn palette_alpha(&self) -> Option<Vec<u8>> {
        match self {
            Image::Paletted(data) => Some(data.palette_alpha.clone()),
            Image::Loaded(data) => data.palette_alpha.clone(),
            Image::Pipeline {
                palette_alpha, ops, ..
            } => palette_alpha.clone().or_else(|| {
                ops.palette_output()
                    .and_then(|output| output.palette_alpha.clone())
            }),
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
            Image::Pipeline { ops, .. } if ops.source_info_suppressed() => None,
            Image::Pipeline { source, ops, .. } => match ops.palette_output() {
                Some(output) => match output.wrapper_metadata {
                    crate::compute::pool_cpu::ops::color::convert::ops::ConvertWrapperMetadata::PreserveSource
                    | crate::compute::pool_cpu::ops::color::convert::ops::ConvertWrapperMetadata::PreserveSourceLoaded => source.image_info(),
                    crate::compute::pool_cpu::ops::color::convert::ops::ConvertWrapperMetadata::ExifOnly
                    | crate::compute::pool_cpu::ops::color::convert::ops::ConvertWrapperMetadata::Drop => None,
                },
                None => source.image_info(),
            },
        }
    }

    pub(crate) fn source_format(&self) -> Option<ImageFormat> {
        match self {
            Image::Loaded(data) => data.source_format,
            Image::Paletted(data) => data.source_format,
            Image::Bytes { format, .. } => *format,
            Image::Pipeline {
                source,
                ops,
                format,
                ..
            } => match ops.palette_output() {
                Some(output) => match output.wrapper_metadata {
                    crate::compute::pool_cpu::ops::color::convert::ops::ConvertWrapperMetadata::PreserveSource
                    | crate::compute::pool_cpu::ops::color::convert::ops::ConvertWrapperMetadata::PreserveSourceLoaded
                        if !ops.source_format_suppressed() =>
                    {
                        source.source_format()
                    }
                    _ => None,
                },
                None => *format,
            },
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
        if palette.is_none()
            && matches!(
                self.mode_identity()?,
                Mode::ImagePalette8 | Mode::ImagePaletteAlpha8
            )
        {
            return Ok(Some(Vec::new()));
        }
        Ok(palette)
    }

    /// Returns RGB triples for an indexed Qt-compatible color table.
    ///
    /// Qt owns the final `qRgb` packing and `QImage` object construction; the
    /// Rust core owns the mode-aware table values so palette traversal is
    /// shared with every binding.
    pub fn indexed_color_table(&self) -> Result<Vec<(u8, u8, u8)>, PilError> {
        match self.mode_identity()? {
            Mode::ImageLuma8 => Ok((0..=u8::MAX).map(|value| (value, value, value)).collect()),
            Mode::ImagePalette8 => Ok(self
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
        let mode = self.mode_identity()?;
        if !matches!(
            mode,
            Mode::ImageLuma8
                | Mode::ImageLumaA8
                | Mode::ImagePalette8
                | Mode::ImagePaletteAlpha8
        ) {
            return Err(PilError::ValueError("illegal image mode".to_owned()));
        }
        let (palette, palette_alpha) = split_palette_data(data, rawmode)?;
        let source_format = self.source_format();
        let info = self.image_info();
        let exif = self.exif_metadata();
        let materialized = self.materialize()?;

        *self = match mode {
            Mode::ImageLuma8 | Mode::ImagePalette8 => Image::Paletted(PalettedData {
                indices: DynamicImage::ImagePalette8(materialized.to_luma8()),
                palette,
                palette_alpha,
                source_format,
                info,
                exif,
                materialized: materialization_cache(),
            }),
            Mode::ImageLumaA8 | Mode::ImagePaletteAlpha8 => {
                let image = DynamicImage::ImagePaletteAlpha8(materialized.to_luma_alpha8());
                Image::Loaded(LoadedData {
                    image: Arc::new(image),
                    known_zero_fill: false,
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

    /// Applies Python ImagePalette transparency to an attached palette.
    ///
    /// This mirrors Pillow's `_putpalettealpha`: alpha storage has 256 slots,
    /// defaults to opaque, and an index may be set beyond the currently
    /// retained color entries. The Python wrapper keeps its public
    /// `info["transparency"]` value; this method clears only the codec-side
    /// pending marker after committing the alpha table.
    pub fn putpalettealpha(&mut self, index: i32, alpha: i32) -> Result<(), PilError> {
        if !self.has_attached_palette() {
            return Err(PilError::ValueError("image has no palette".to_owned()));
        }
        if !(0..256).contains(&index) {
            return Err(PilError::ValueError(
                "palette index out of range".to_owned(),
            ));
        }

        self.load()?;
        let mut palette_alpha = self.palette_alpha().unwrap_or_default();
        palette_alpha.resize(256, 255);
        palette_alpha[index as usize] = alpha as u8;
        self.store_committed_palette_alpha(palette_alpha)
    }

    /// Applies a prefix of Python ImagePalette transparency bytes.
    ///
    /// Existing raw LA palette alpha is preserved after the supplied prefix;
    /// missing entries remain opaque. The Python wrapper keeps its public
    /// `info["transparency"]` value and clears the codec-side pending marker.
    pub fn putpalettealphas(&mut self, values: &[u8]) -> Result<(), PilError> {
        if !self.has_attached_palette() {
            return Err(PilError::ValueError("image has no palette".to_owned()));
        }
        if values.len() > 256 {
            return Err(PilError::ValueError(
                "palette index out of range".to_owned(),
            ));
        }

        self.load()?;
        let mut palette_alpha = self.palette_alpha().unwrap_or_default();
        palette_alpha.resize(256, 255);
        palette_alpha[..values.len()].copy_from_slice(values);
        self.store_committed_palette_alpha(palette_alpha)
    }

    /// Commits alpha already supplied by an LA raw palette.
    pub fn commit_palette_alpha(&mut self) -> Result<(), PilError> {
        if !self.has_attached_palette() {
            return Err(PilError::ValueError("image has no palette".to_owned()));
        }
        self.load()?;
        let mut palette_alpha = self.palette_alpha().unwrap_or_default();
        palette_alpha.resize(256, 255);
        self.store_committed_palette_alpha(palette_alpha)
    }

    fn store_committed_palette_alpha(&mut self, palette_alpha: Vec<u8>) -> Result<(), PilError> {
        match self {
            Image::Paletted(data) => {
                data.palette_alpha = palette_alpha;
                clear_pending_palette_alpha(&mut data.info);
            }
            Image::Loaded(data) if data.palette.is_some() => {
                data.palette_alpha = Some(palette_alpha);
                clear_pending_palette_alpha(&mut data.info);
            }
            _ => return Err(PilError::ValueError("image has no palette".to_owned())),
        }
        Ok(())
    }

    /// Replaces palette metadata on an indexed image without decoding or
    /// materializing its pixel pipeline.
    pub(crate) fn replace_indexed_palette(
        &mut self,
        data: &[u8],
        rawmode: &str,
    ) -> Result<(), PilError> {
        let mode = self.mode_identity()?;
        if !matches!(mode, Mode::ImagePalette8 | Mode::ImagePaletteAlpha8) {
            return Err(PilError::ValueError("illegal image mode".to_owned()));
        }
        let (palette, palette_alpha) = split_palette_data(data, rawmode)?;

        match self {
            Image::Paletted(image) => {
                image.palette = palette;
                image.palette_alpha = palette_alpha;
            }
            Image::Loaded(image) => {
                image.palette = Some(palette);
                image.palette_alpha = Some(palette_alpha);
            }
            Image::Pipeline {
                ops,
                palette: current_palette,
                palette_alpha: current_alpha,
                ..
            } if ops.is_empty() => {
                *current_palette = Some(palette);
                *current_alpha = Some(palette_alpha);
            }
            _ => {
                // Keep earlier operations attached to their original palette;
                // this empty metadata layer applies the updated palette only
                // to operations queued after the color was resolved.
                let source = Arc::new(self.clone());
                *self = Image::Pipeline {
                    format: self.source_format(),
                    backend: self.backend(),
                    source,
                    ops: super::PipelineOps::empty(),
                    palette: Some(palette),
                    palette_alpha: Some(palette_alpha),
                    compatibility_icc_profile: self.compatibility_icc_profile().map(Arc::from),
                    materialized: materialization_cache(),
                    shape: super::pipeline_shape_cache(),
                    mode: super::pipeline_mode_cache_for(Some(mode)),
                };
            }
        }
        Ok(())
    }

    /// Installs the representable portion of a Python-owned Draw palette.
    ///
    /// `ImagePalette.getcolor` retains arbitrary byte lengths until
    /// `Image.load`; drawing only needs the first 256 complete entries to
    /// resolve indexed pixels. The wrapper keeps the original bytes and
    /// performs strict realization at load time.
    pub(crate) fn replace_indexed_palette_from_draw(
        &mut self,
        data: &[u8],
        rawmode: &str,
    ) -> Result<(), PilError> {
        let entry_size = match rawmode {
            "RGB" => 3,
            "RGBA" => 4,
            "LA" => 2,
            _ => return Err(PilError::ValueError("unrecognized raw mode".to_owned())),
        };
        let complete_size = (data.len() / entry_size).min(256) * entry_size;
        self.replace_indexed_palette(&data[..complete_size], rawmode)
    }
}

impl Image {
    /// Extract palette from Paletted variant (for Pipeline propagation).
    pub(crate) fn extract_palette(&self) -> Option<Vec<u8>> {
        match self {
            Image::Paletted(data) => Some(operational_palette(data)),
            Image::Loaded(data) => data.palette.clone(),
            Image::Pipeline { palette, ops, .. } => palette
                .clone()
                .or_else(|| ops.palette_output()?.palette.clone()),
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
    /// Returns [`PilError::ValueError`] when the source is not `L` or `P`, or
    /// when `dest_map` contains more than 256 entries. Errors resolving the
    /// source mode are propagated.
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
        let mode = self.mode_identity()?;
        if !matches!(mode, Mode::ImageLuma8 | Mode::ImagePalette8) {
            return Err(PilError::ValueError("illegal image mode".to_owned()));
        }
        if dest_map.len() > 256 {
            return Err(PilError::ValueError(
                "byte must be in range(0, 256)".to_owned(),
            ));
        }

        let uses_attached_palette = source_palette.is_none() && mode == Mode::ImagePalette8;
        // Pillow's Image.remap_palette treats every explicit palette longer
        // than 768 bytes as interleaved RGBA, regardless of the source mode.
        let source_bands = if source_palette.is_some_and(|palette| palette.len() > 768) {
            4
        } else {
            3
        };
        let source_palette = source_palette.map_or_else(
            || {
                if mode == Mode::ImagePalette8 {
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
            palette,
            palette_alpha,
            ..
        } = &mut result
        {
            *palette = Some(remapped_palette);
            *palette_alpha = remapped_alpha;
        }
        Ok(result)
    }
}
