//! Pillow-compatible font loading and text rendering.
//!
//! TrueType loading, metadata, glyph rendering, advance, and kerning go
//! through the `_imagingft` adapter so this public module does not expose
//! FreeType-core helper APIs.

use crate::error::PilError;

mod default_aileron;
pub(crate) mod imagingft;
pub(crate) mod pilfont;

/// Pillow `FreeTypeFont`-compatible handle backed by the pure-Rust FreeType path.
pub struct Font {
    engine: imagingft::TrueTypeEngine,
}

/// One Pillow `FreeTypeFont.get_variation_axes()` axis record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FontVariationAxis {
    /// Minimum design coordinate as Pillow reports it.
    pub minimum: i32,
    /// Default design coordinate as Pillow reports it.
    pub default: i32,
    /// Maximum design coordinate as Pillow reports it.
    pub maximum: i32,
    /// Axis name bytes after Pillow's null-byte cleanup.
    pub name: Vec<u8>,
}

/// Optional Pillow `FreeTypeFont` text-layout/render arguments.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FontTextOptions {
    /// Pillow `mode` argument. BASIC layout ignores most values, but `RGBA`
    /// changes `getmask/getmask2` allocation behavior.
    pub mode: Option<String>,
    /// Pillow `direction` argument. Requires libraqm in Pillow.
    pub direction: Option<String>,
    /// Pillow OpenType feature list. Requires libraqm in Pillow.
    pub features: Option<Vec<String>>,
    /// Pillow language tag. Requires libraqm in Pillow.
    pub language: Option<String>,
    /// Pillow text stroke width in pixels.
    pub stroke_width: f32,
    /// Pillow two-character anchor code.
    pub anchor: Option<String>,
    /// Pillow fractional rendering start.
    pub start: Option<(f64, f64)>,
}

impl Font {
    /// Load a TrueType/OpenType face from bytes at the requested Pillow point size.
    pub fn from_bytes(data: Vec<u8>, size: f32) -> Result<Self, PilError> {
        imagingft::load_truetype(data, size)
    }

    /// Loads the same embedded Aileron Regular subset as Pillow 12.2.0.
    ///
    /// Pillow opens this subset with the BASIC layout engine. The regular
    /// TrueType constructor is used here so default fonts and caller-supplied
    /// fonts share the same pure-Rust `fontdone` pipeline.
    pub fn load_default(size: f32) -> Result<Self, PilError> {
        Self::from_bytes(default_aileron::decode()?, size)
    }

    /// Return the requested Pillow point size for this FreeType font.
    pub fn font_size(&self) -> f32 {
        self.engine.size_pt
    }

    /// Create a variant copy of this FreeType font, overriding the size when provided.
    pub fn font_variant(&self, size: Option<f32>) -> Result<Self, PilError> {
        imagingft::font_variant(self, size)
    }

    /// Return the non-negative text mask extent for Pillow-style text layout.
    pub fn text_bbox(&self, text: &str) -> Result<(u32, u32), PilError> {
        let bbox = imagingft::getbbox(self, text)?;
        let w = (bbox.2 - bbox.0).max(0) as u32;
        let h = (bbox.3 - bbox.1).max(0) as u32;
        Ok((w, h))
    }

    /// Return the Pillow-compatible grayscale text mask.
    pub fn getmask(&self, text: &str) -> Result<(u32, u32, Vec<u8>), PilError> {
        imagingft::getmask(self, text)
    }

    /// Return Pillow's public `getmask` result using optional render arguments.
    pub fn getmask_with_options(
        &self,
        text: &str,
        options: &FontTextOptions,
    ) -> Result<(u32, u32, Vec<u8>), PilError> {
        imagingft::getmask_with_options(self, text, options)
    }

    /// Return Pillow's public `(family, style)` font name tuple.
    pub fn getname(&self) -> (&str, &str) {
        let (family, style) = imagingft::getname_optional(self);
        (family.unwrap_or("Unknown"), style.unwrap_or("Regular"))
    }

    /// Return Pillow's raw public name tuple, preserving missing face names.
    #[cfg(feature = "test-api")]
    pub fn getname_optional(&self) -> (Option<&str>, Option<&str>) {
        imagingft::getname_optional(self)
    }

    /// Return Pillow's public ascent/descent metrics.
    pub fn getmetrics(&self) -> (u32, u32) {
        imagingft::getmetrics(self)
    }

    /// Return Pillow's public text length in pixels.
    pub fn getlength(&self, text: &str) -> Result<f32, PilError> {
        imagingft::getlength(self, text)
    }

    /// Return Pillow's public text length using optional layout arguments.
    pub fn getlength_with_options(
        &self,
        text: &str,
        options: &FontTextOptions,
    ) -> Result<f32, PilError> {
        imagingft::getlength_with_options(self, text, options)
    }

    /// Return whether the font exposes variation axes.
    pub fn has_variations(&self) -> bool {
        imagingft::has_variations(self)
    }

    /// Return Pillow's public variation-axis records.
    pub fn get_variation_axes(&self) -> Result<Vec<FontVariationAxis>, PilError> {
        imagingft::get_variation_axes(self)
    }

    /// Return Pillow's public named-variation style names.
    pub fn get_variation_names(&self) -> Result<Vec<Vec<u8>>, PilError> {
        imagingft::get_variation_names(self)
    }

    /// Set a named variation instance by Pillow-style name bytes.
    pub fn set_variation_by_name(&mut self, name: &[u8]) -> Result<(), PilError> {
        imagingft::set_variation_by_name(self, name)
    }

    /// Set variation design coordinates from Pillow-style user coordinates.
    pub fn set_variation_by_axes(&mut self, axes: &[f32]) -> Result<(), PilError> {
        imagingft::set_variation_by_axes(self, axes)
    }

    /// Return Pillow's public text bounding box.
    pub fn getbbox(&self, text: &str) -> Result<(i32, i32, i32, i32), PilError> {
        imagingft::getbbox(self, text)
    }

    /// Return Pillow's public text bounding box using optional layout arguments.
    pub fn getbbox_with_options(
        &self,
        text: &str,
        options: &FontTextOptions,
    ) -> Result<(f32, f32, f32, f32), PilError> {
        imagingft::getbbox_with_options(self, text, options)
    }

    /// Return Pillow's public binary-mode text bounding box.
    #[cfg(feature = "test-api")]
    pub fn getbbox_binary(&self, text: &str) -> Result<(i32, i32, i32, i32), PilError> {
        imagingft::getbbox_binary(self, text)
    }

    /// Return Pillow's public `getmask2` mask and offset tuple.
    pub fn getmask2(&self, text: &str) -> Result<(u32, u32, Vec<u8>, (i32, i32)), PilError> {
        imagingft::getmask2(self, text)
    }

    /// Return Pillow's public `getmask2` result using optional render arguments.
    pub fn getmask2_with_options(
        &self,
        text: &str,
        options: &FontTextOptions,
    ) -> Result<(u32, u32, Vec<u8>, (i32, i32)), PilError> {
        imagingft::getmask2_with_options(self, text, options)
    }

    /// `getmask2` variant with Pillow's fractional start parameter.
    pub fn getmask2_with_start(
        &self,
        text: &str,
        start: (f64, f64),
    ) -> Result<(u32, u32, Vec<u8>, (i32, i32)), PilError> {
        imagingft::getmask2_with_start(self, text, start)
    }

    /// Return a transposed Pillow-compatible grayscale text mask.
    pub fn get_transposed_mask(
        &self,
        text: &str,
        orientation: Option<&str>,
    ) -> Result<(u32, u32, Vec<u8>), PilError> {
        imagingft::get_transposed_mask(self, text, orientation)
    }

    /// Return Pillow-compatible binary-mode RGBA text rendering.
    #[cfg(feature = "test-api")]
    pub fn render_text_binary(
        &self,
        text: &str,
        fill: (u8, u8, u8, u8),
        spacing: f32,
    ) -> Result<(u32, u32, Vec<u8>), PilError> {
        imagingft::render_text_binary(self, text, fill, spacing)
    }
}

/// Normalize a wrapped font bounding box using Pillow's `TransposedFont` rules.
pub fn transposed_bbox(
    bbox: (i32, i32, i32, i32),
    orientation: Option<&str>,
) -> (i32, i32, i32, i32) {
    imagingft::transposed_bbox(bbox, orientation)
}

/// Validate whether Pillow defines text length for a transposed font.
pub fn validate_transposed_length(orientation: Option<&str>) -> Result<(), PilError> {
    imagingft::validate_transposed_length(orientation)
}
