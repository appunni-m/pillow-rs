//! Read-only palette accessors and metadata views for [`Image`].

use image_slash_star::{ImageFormat, ImageInfo};

use crate::compute::registry::support::PaletteInput;
use crate::error::PilError;
use crate::raster::Mode;

use super::super::{Image, PaletteData, declared_pipeline_mode};

impl Image {
    /// Returns a mode explicitly carried by this materialized wrapper or
    /// pending pipeline, without projecting it through a Pillow spelling.
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
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
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    pub fn explicit_mode(&self) -> Option<&str> {
        self.explicit_mode_identity()
            .and_then(super::super::mode_boundary::pillow_mode_name)
    }

    /// Returns attached palette data as RGB triples.
    ///
    /// A palette contains up to 768 bytes: 256 entries of `R, G, B`. Pipeline
    /// images can carry a copied palette so palette-safe operations preserve
    /// `P` mode.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    pub fn palette(&self) -> Option<Vec<u8>> {
        match self {
            Image::Paletted(data) => data
                .palette_attached
                .then(|| super::operational_palette(data)),
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

    /// Returns the retained Pillow palette model, when one is attached.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    pub(crate) fn palette_data(&self) -> Option<&PaletteData> {
        match self {
            Image::Paletted(data) => data.palette_attached.then_some(&data.palette_data),
            Image::Loaded(data) if data.palette.is_some() => data.palette_data.as_ref(),
            Image::Loaded(_) => None,
            Image::Bytes { .. } => None,
            Image::Pipeline { .. } if !self.has_attached_palette() => None,
            Image::Pipeline {
                source,
                palette_data,
                ops,
                ..
            } => palette_data
                .as_ref()
                .or_else(|| {
                    ops.palette_output()
                        .and_then(|output| output.palette_data.as_ref())
                })
                .or_else(|| source.palette_data()),
        }
    }

    /// Returns whether palette metadata is attached, including an empty table.
    #[doc(hidden)]
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    pub fn has_attached_palette(&self) -> bool {
        match self {
            Image::Paletted(data) => data.palette_attached,
            Image::Loaded(data) => data.palette.is_some(),
            Image::Bytes { info, .. } => info.as_ref().is_some_and(|info| info.palette.is_some()),
            Image::Pipeline { palette, ops, .. } => {
                palette.is_some()
                    || ops
                        .palette_output()
                        .is_some_and(|output| output.palette.is_some())
            }
        }
    }

    /// Borrows the resolved palette dependency for mode-specific operations.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    pub(crate) fn palette_input(&self) -> PaletteInput<'_> {
        match self {
            Image::Paletted(data) if !data.palette_attached => PaletteInput::default(),
            Image::Paletted(data) => PaletteInput {
                rgb: Some(&data.palette),
                alpha: &data.palette_alpha,
                data: Some(&data.palette_data),
            },
            Image::Loaded(data) if data.palette.is_none() => PaletteInput::default(),
            Image::Loaded(data) => PaletteInput {
                rgb: data.palette.as_deref(),
                alpha: data.palette_alpha.as_deref().unwrap_or_default(),
                data: data.palette_data.as_ref(),
            },
            Image::Bytes { info, .. } => info
                .as_ref()
                .and_then(|info| info.palette.as_ref())
                .map(|palette| PaletteInput {
                    rgb: Some(&palette.rgb),
                    alpha: &palette.alpha,
                    data: None,
                })
                .unwrap_or_default(),
            Image::Pipeline {
                source,
                palette,
                palette_alpha,
                palette_data,
                ops,
                ..
            } => {
                let output = ops.palette_output();
                let rgb = palette
                    .as_deref()
                    .or_else(|| output.and_then(|output| output.palette.as_deref()));
                PaletteInput {
                    rgb,
                    alpha: if rgb.is_some() {
                        palette_alpha
                            .as_deref()
                            .or_else(|| output.and_then(|output| output.palette_alpha.as_deref()))
                            .unwrap_or_default()
                    } else {
                        &[]
                    },
                    data: rgb.and_then(|_| {
                        palette_data
                            .as_ref()
                            .or_else(|| output.and_then(|output| output.palette_data.as_ref()))
                            .or_else(|| source.palette_data())
                    }),
                }
            }
        }
    }

    /// Returns whether a nonempty RGB palette is attached without cloning it.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    pub(crate) fn has_nonempty_palette(&self) -> bool {
        match self {
            Image::Paletted(data) => data.palette_attached && !data.palette.is_empty(),
            Image::Loaded(data) => data
                .palette
                .as_ref()
                .is_some_and(|palette| !palette.is_empty()),
            Image::Pipeline { palette, ops, .. } => palette
                .as_ref()
                .or_else(|| {
                    ops.palette_output()
                        .and_then(|output| output.palette.as_ref())
                })
                .is_some_and(|palette| !palette.is_empty()),
            Image::Bytes { info, .. } => info
                .as_ref()
                .and_then(|info| info.palette.as_ref())
                .is_some_and(|palette| !palette.rgb.is_empty()),
        }
    }

    /// Returns retained per-entry alpha values for an indexed image.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    pub fn palette_alpha(&self) -> Option<Vec<u8>> {
        match self {
            Image::Paletted(data) => data.palette_attached.then(|| data.palette_alpha.clone()),
            Image::Loaded(data) if data.palette.is_some() => data.palette_alpha.clone(),
            Image::Loaded(_) => None,
            Image::Pipeline { .. } if !self.has_attached_palette() => None,
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
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    pub fn image_info(&self) -> Option<ImageInfo> {
        match self {
            Image::Loaded(data) => data.info.clone(),
            Image::Paletted(data) => data.info.clone(),
            Image::Bytes { info, .. } => info.clone(),
            Image::Pipeline { ops, .. } if ops.source_info_suppressed() => None,
            Image::Pipeline { source, ops, .. } => match ops.palette_output() {
                Some(output) => match output.wrapper_metadata {
                    crate::compute::pool_cpu::ops::color::ops::ConvertWrapperMetadata::PreserveSource
                    | crate::compute::pool_cpu::ops::color::ops::ConvertWrapperMetadata::PreserveSourceLoaded => source.image_info(),
                    crate::compute::pool_cpu::ops::color::ops::ConvertWrapperMetadata::ExifOnly
                    | crate::compute::pool_cpu::ops::color::ops::ConvertWrapperMetadata::Drop => None,
                },
                None => source.image_info(),
            },
        }
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending

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
                    crate::compute::pool_cpu::ops::color::ops::ConvertWrapperMetadata::PreserveSource
                    | crate::compute::pool_cpu::ops::color::ops::ConvertWrapperMetadata::PreserveSourceLoaded
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
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    pub fn getpalette_trimmed(&self) -> Option<Vec<u8>> {
        self.extract_palette()
    }

    /// Returns palette bytes converted to RGBA, trimmed to the retained RGB
    /// palette length.
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    pub fn getpalette_rgba(&self) -> Option<Vec<u8>> {
        if matches!(self.palette_data(), Some(PaletteData::Cmyk(_))) {
            return None;
        }
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

    /// Converts the retained palette to a Pillow ``getpalette(rawmode)``
    /// layout.
    ///
    /// Pillow's C `getpalette` accepts ``"RGB"``, ``"RGBA"`` (real per-entry
    /// alpha), ``"RGBX"`` (alpha forced to 255, only for palettes without
    /// alpha data), and the single-channel selectors ``"R"``/``"G"``/``"B"``;
    /// every other rawmode raises ``ValueError: unrecognized raw mode``.
    /// Native CMYK palette metadata also exposes its complete entries,
    /// inverted and planar layouts, RGB projection, and individual channels.
    /// Incomplete input bytes do not contribute to the palette's entry count.
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    pub fn getpalette_rawmode(&self, rawmode: &str) -> Result<Option<Vec<u8>>, PilError> {
        let Some(palette) = self.getpalette_trimmed() else {
            return Ok(None);
        };
        if let Some(PaletteData::Cmyk(entries)) = self.palette_data() {
            let raw_palette = match rawmode {
                "CMYK" => entries.clone(),
                "CMYK;I" => entries[..entries.len() / 4 * 4]
                    .iter()
                    .map(|value| u8::MAX - value)
                    .collect(),
                "CMYK;L" => {
                    let count = entries.len() / 4;
                    let mut planar = vec![0; count * 4];
                    for (index, color) in entries[..count * 4].chunks_exact(4).enumerate() {
                        for channel in 0..4 {
                            planar[channel * count + index] = color[channel];
                        }
                    }
                    planar
                }
                "RGB" => palette,
                "C" | "M" | "Y" | "K" => {
                    let channel = match rawmode {
                        "C" => 0,
                        "M" => 1,
                        "Y" => 2,
                        _ => 3,
                    };
                    entries
                        .chunks_exact(4)
                        .map(|entry| entry[channel])
                        .collect()
                }
                _ => return Err(PilError::ValueError("unrecognized raw mode".into())),
            };
            return Ok(Some(raw_palette));
        }
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
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    pub fn getpalette_with_input(
        &self,
        rawmode: Option<&str>,
    ) -> Result<Option<Vec<u8>>, PilError> {
        let rawmode = rawmode.or_else(|| self.palette_mode()).unwrap_or("RGB");
        let palette = self.getpalette_rawmode(rawmode)?;
        Ok(palette)
    }

    /// Returns RGB triples for an indexed Qt-compatible color table.
    ///
    /// Qt owns the final `qRgb` packing and `QImage` object construction; the
    /// Rust core owns the mode-aware table values so palette traversal is
    /// shared with every binding.
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
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
}
