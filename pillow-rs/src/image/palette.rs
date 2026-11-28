//! Indexed color table access and mutation methods for [`Image`].

use image_slash_star::ImageInfo;

use crate::error::PilError;
use crate::raster::DynamicImage;
use crate::raster::Mode;

use super::PalettedData;

/// Palette color model; independent of the indexed raster's [`Mode`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PaletteModel {
    /// Three channels per palette entry.
    Rgb,
    /// RGB plus alpha per palette entry.
    Rgba,
    /// Cyan, magenta, yellow, and black per palette entry.
    Cmyk,
}

impl PaletteModel {
    /// Parses one of Pillow's palette model names at a host boundary.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    #[must_use]
    pub fn from_pillow_name(name: &str) -> Option<Self> {
        match name {
            "RGB" => Some(Self::Rgb),
            "RGBA" => Some(Self::Rgba),
            "CMYK" => Some(Self::Cmyk),
            _ => None,
        }
    }

    /// Applies Pillow's high-level rawmode-to-palette-model rule.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    #[must_use]
    pub(crate) fn inferred_from_rawmode(rawmode: &str) -> Self {
        if rawmode.starts_with("CMYK") {
            Self::Cmyk
        } else if rawmode.contains('A') {
            Self::Rgba
        } else {
            Self::Rgb
        }
    }

    /// Returns Pillow's public palette model spelling.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    #[must_use]
    pub const fn pillow_name(self) -> &'static str {
        match self {
            Self::Rgb => "RGB",
            Self::Rgba => "RGBA",
            Self::Cmyk => "CMYK",
        }
    }

    /// Returns the entry slice width used by Pillow-compatible palette remap.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    #[must_use]
    pub const fn remap_palette_bands(self) -> usize {
        match self {
            Self::Rgba => 4,
            // Pillow selects three source bytes per CMYK map entry before
            // installing those selected bytes as a CMYK palette.
            Self::Rgb | Self::Cmyk => 3,
        }
    }

    /// Returns the raw layout Pillow uses when installing a remap index table.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    #[must_use]
    pub const fn remap_mapping_rawmode(self) -> &'static str {
        match self {
            Self::Rgb => "RGB;L",
            Self::Rgba => "RGBA;L",
            Self::Cmyk => "CMYK;L",
        }
    }

    /// Builds a typed color table whose luma value is each remapped index.
    ///
    /// RemapPalette installs this intermediate table before converting the
    /// copied indexed image to L. CMYK uses zero C/M/Y and inverse K so its
    /// RGB projection is the same grayscale value as the mapping index.
    // TODO(pillow-rs#3ptb): compare CMYK outputs with Pillow's three-byte map
    // stride and four-plane CMYK;L install before deciding output policy.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn remap_mapping_palette(self, mapping: &[u8]) -> Vec<u8> {
        let bands = match self {
            Self::Rgb => 3,
            Self::Rgba | Self::Cmyk => 4,
        };
        let mut palette = Vec::with_capacity(mapping.len() * bands);
        match self {
            Self::Rgb | Self::Rgba => {
                for _ in 0..bands {
                    palette.extend_from_slice(mapping);
                }
            }
            Self::Cmyk => {
                palette.resize(mapping.len() * 3, 0);
                palette.extend(mapping.iter().map(|&index| u8::MAX - index));
            }
        }
        palette
    }

    /// Returns the Pillow raster identity used to decode this palette model.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    #[must_use]
    pub(crate) const fn raster_mode(self) -> Mode {
        match self {
            Self::Rgb => Mode::ImageRgb8,
            Self::Rgba => Mode::ImageRgba8,
            Self::Cmyk => Mode::ImageCmyk8,
        }
    }
}

/// Pillow-owned palette color model, independent of the indexed raster mode.
///
/// RGB and RGBA bytes remain in the existing RGB/alpha fields. CMYK retains
/// its complete native entries here, while `PalettedData::palette` holds the
/// corresponding RGB projection for operations whose output requires RGB.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaletteData {
    /// Three byte color entries.
    Rgb,
    /// RGB color entries with a separate per-entry alpha table.
    Rgba,
    /// Complete native CMYK palette entries, retained alongside the RGB
    /// projection. Pillow's palette entry count ignores a partial input tail.
    Cmyk(Vec<u8>),
}

impl PaletteData {
    /// Returns the typed color model retained by this palette.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    #[must_use]
    pub(crate) const fn model(&self) -> PaletteModel {
        match self {
            Self::Rgb => PaletteModel::Rgb,
            Self::Rgba => PaletteModel::Rgba,
            Self::Cmyk(_) => PaletteModel::Cmyk,
        }
    }

    /// Infers the attached RGB palette model from its separate alpha table.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    #[must_use]
    pub(crate) fn from_alpha(alpha: &[u8]) -> Self {
        if alpha.is_empty() {
            Self::Rgb
        } else {
            Self::Rgba
        }
    }

    /// Returns Pillow's public palette model name.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    #[must_use]
    pub const fn mode_name(&self) -> &'static str {
        self.model().pillow_name()
    }

    /// Returns the native CMYK palette byte buffer when this palette uses it.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    #[must_use]
    pub fn cmyk_entries(&self) -> Option<&[u8]> {
        match self {
            Self::Cmyk(entries) => Some(entries),
            Self::Rgb | Self::Rgba => None,
        }
    }

    /// Applies Pillow's four-byte palette alpha transition.
    ///
    /// Pillow relabels the native four-byte entries as RGBA. For CMYK this
    /// means C, M, Y become RGB and K becomes alpha; incomplete trailing input
    /// bytes are outside the native palette entry count and are discarded.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    pub(crate) fn transition_to_rgba(&mut self, rgb: &mut Vec<u8>, alpha: &mut Vec<u8>) {
        if let Self::Cmyk(entries) = self {
            let mut channels = Vec::with_capacity(entries.len() / 4 * 3);
            let mut palette_alpha = Vec::with_capacity(entries.len() / 4);
            for entry in entries.chunks_exact(4) {
                channels.extend_from_slice(&entry[..3]);
                palette_alpha.push(entry[3]);
            }
            *rgb = channels;
            *alpha = palette_alpha;
        }
        *self = Self::Rgba;
    }
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
pub(super) fn padded_palette(rgb: &[u8]) -> Vec<u8> {
    let mut palette = rgb.to_vec();
    palette.resize(768, 0);
    palette
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
fn split_palette_data(
    data: &[u8],
    model: PaletteModel,
    rawmode: &str,
) -> Result<(Vec<u8>, Vec<u8>, PaletteData), PilError> {
    // The raw decoder applies Pillow's complete-entry count, 256-entry limit,
    // and partial-tail handling for native and converted layouts alike.
    let normalized =
        crate::image::RawDecoderPlan::decode_palette_data(model.raster_mode(), rawmode, data)?;
    Ok(split_normalized_palette_data(normalized, model))
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
fn split_normalized_palette_data(
    normalized: Vec<u8>,
    model: PaletteModel,
) -> (Vec<u8>, Vec<u8>, PaletteData) {
    match model {
        PaletteModel::Rgb => (normalized, Vec::new(), PaletteData::Rgb),
        PaletteModel::Rgba => {
            let mut rgb = Vec::with_capacity(normalized.len() / 4 * 3);
            let mut alpha = Vec::with_capacity(normalized.len() / 4);
            for color in normalized.chunks_exact(4) {
                rgb.extend_from_slice(&color[..3]);
                alpha.push(color[3]);
            }
            (rgb, alpha, PaletteData::Rgba)
        }
        PaletteModel::Cmyk => {
            let mut rgb = Vec::with_capacity(normalized.len() / 4 * 3);
            for color in normalized.chunks_exact(4) {
                rgb.extend_from_slice(&crate::color::cmyk_pixel_to_rgb([
                    color[0], color[1], color[2], color[3],
                ]));
            }
            (rgb, Vec::new(), PaletteData::Cmyk(normalized))
        }
    }
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
fn clear_pending_palette_alpha(info: &mut Option<ImageInfo>) {
    let Some(palette) = info.as_mut().and_then(|info| info.palette.as_mut()) else {
        return;
    };
    palette.alpha.clear();
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
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
// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
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
// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
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

mod access;
mod mutation;
mod remap;
