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
    /// Pillow foreground ink for mask rendering. Grayscale BASIC masks accept
    /// the integer but render coverage bytes independent of its value.
    pub ink: Option<i64>,
    /// Whether Pillow-compatible variadic `getmask2` arguments were supplied.
    /// The BASIC C path ignores them; this field preserves public signature
    /// coverage without moving argument interpretation into tests.
    pub has_args: bool,
    /// Whether Pillow-compatible extra `getmask2` keyword arguments were
    /// supplied. Unknown keywords are ignored by Pillow's public wrapper.
    pub has_kwargs: bool,
}

/// Optional Pillow `FreeTypeFont.font_variant()` override arguments.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FontVariantOptions {
    /// Replacement font bytes for Pillow's `font` argument.
    pub font_bytes: Option<Vec<u8>>,
    /// Replacement point size for Pillow's `size` argument.
    pub size: Option<f32>,
    /// Replacement face index for Pillow's `index` argument.
    pub index: Option<usize>,
    /// Pillow `encoding` argument. BASIC Unicode-compatible rows preserve this
    /// for public signature parity while fontdone selects the Unicode charmap.
    pub encoding: Option<String>,
    /// Pillow `layout_engine` argument. In the no-raqm oracle configuration,
    /// Pillow accepts this argument and falls back to BASIC layout.
    pub layout_engine: Option<String>,
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

    /// Create a variant copy of this FreeType font with Pillow-style overrides.
    pub fn font_variant_with_options(
        &self,
        options: &FontVariantOptions,
    ) -> Result<Self, PilError> {
        imagingft::font_variant_with_options(self, options)
    }

    /// Return the non-negative text mask extent for Pillow-style text layout.
    pub fn text_bbox(&self, text: &str) -> Result<(u32, u32), PilError> {
        let bbox = imagingft::getbbox(self, text)?;
        let w = (bbox.2 - bbox.0).max(0) as u32;
        let h = (bbox.3 - bbox.1).max(0) as u32;
        Ok((w, h))
    }

    /// Return the non-negative text mask extent for a Python `bytes` text argument.
    pub fn text_bbox_bytes(&self, text: &[u8]) -> Result<(u32, u32), PilError> {
        let text = pillow_bytes_to_text(text);
        self.text_bbox(&text)
    }

    /// Return the Pillow-compatible grayscale text mask.
    pub fn getmask(&self, text: &str) -> Result<(u32, u32, Vec<u8>), PilError> {
        imagingft::getmask(self, text)
    }

    /// Return the Pillow-compatible grayscale text mask for byte text.
    pub fn getmask_bytes(&self, text: &[u8]) -> Result<(u32, u32, Vec<u8>), PilError> {
        let text = pillow_bytes_to_text(text);
        self.getmask(&text)
    }

    /// Return Pillow's public `getmask` result using optional render arguments.
    pub fn getmask_with_options(
        &self,
        text: &str,
        options: &FontTextOptions,
    ) -> Result<(u32, u32, Vec<u8>), PilError> {
        imagingft::getmask_with_options(self, text, options)
    }

    /// Return Pillow's public `getmask` result for byte text using optional render arguments.
    pub fn getmask_bytes_with_options(
        &self,
        text: &[u8],
        options: &FontTextOptions,
    ) -> Result<(u32, u32, Vec<u8>), PilError> {
        let text = pillow_bytes_to_text(text);
        self.getmask_with_options(&text, options)
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

    /// Return Pillow's public text length for a Python `bytes` text argument.
    pub fn getlength_bytes(&self, text: &[u8]) -> Result<f32, PilError> {
        let text = pillow_bytes_to_text(text);
        self.getlength(&text)
    }

    /// Return Pillow's public text length using optional layout arguments.
    pub fn getlength_with_options(
        &self,
        text: &str,
        options: &FontTextOptions,
    ) -> Result<f32, PilError> {
        imagingft::getlength_with_options(self, text, options)
    }

    /// Return Pillow's public text length for byte text using optional layout arguments.
    pub fn getlength_bytes_with_options(
        &self,
        text: &[u8],
        options: &FontTextOptions,
    ) -> Result<f32, PilError> {
        let text = pillow_bytes_to_text(text);
        self.getlength_with_options(&text, options)
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

    /// Return Pillow's public text bounding box for a Python `bytes` text argument.
    pub fn getbbox_bytes(&self, text: &[u8]) -> Result<(i32, i32, i32, i32), PilError> {
        let text = pillow_bytes_to_text(text);
        self.getbbox(&text)
    }

    /// Return Pillow's public text bounding box using optional layout arguments.
    pub fn getbbox_with_options(
        &self,
        text: &str,
        options: &FontTextOptions,
    ) -> Result<(f32, f32, f32, f32), PilError> {
        imagingft::getbbox_with_options(self, text, options)
    }

    /// Return Pillow's public text bounding box for byte text using optional layout arguments.
    pub fn getbbox_bytes_with_options(
        &self,
        text: &[u8],
        options: &FontTextOptions,
    ) -> Result<(f32, f32, f32, f32), PilError> {
        let text = pillow_bytes_to_text(text);
        self.getbbox_with_options(&text, options)
    }

    /// Return Pillow's public binary-mode text bounding box.
    #[cfg(feature = "test-api")]
    pub fn getbbox_binary(&self, text: &str) -> Result<(i32, i32, i32, i32), PilError> {
        imagingft::getbbox_binary(self, text)
    }

    /// Return Pillow's public binary-mode text bounding box for byte text.
    #[cfg(feature = "test-api")]
    pub fn getbbox_binary_bytes(&self, text: &[u8]) -> Result<(i32, i32, i32, i32), PilError> {
        let text = pillow_bytes_to_text(text);
        self.getbbox_binary(&text)
    }

    /// Return Pillow's public `getmask2` mask and offset tuple.
    pub fn getmask2(&self, text: &str) -> Result<(u32, u32, Vec<u8>, (i32, i32)), PilError> {
        imagingft::getmask2(self, text)
    }

    /// Return Pillow's public `getmask2` mask and offset tuple for byte text.
    pub fn getmask2_bytes(&self, text: &[u8]) -> Result<(u32, u32, Vec<u8>, (i32, i32)), PilError> {
        let text = pillow_bytes_to_text(text);
        self.getmask2(&text)
    }

    /// Return Pillow's public `getmask2` result using optional render arguments.
    pub fn getmask2_with_options(
        &self,
        text: &str,
        options: &FontTextOptions,
    ) -> Result<(u32, u32, Vec<u8>, (i32, i32)), PilError> {
        imagingft::getmask2_with_options(self, text, options)
    }

    /// Return Pillow's public `getmask2` result for byte text using optional render arguments.
    pub fn getmask2_bytes_with_options(
        &self,
        text: &[u8],
        options: &FontTextOptions,
    ) -> Result<(u32, u32, Vec<u8>, (i32, i32)), PilError> {
        let text = pillow_bytes_to_text(text);
        self.getmask2_with_options(&text, options)
    }

    /// `getmask2` variant with Pillow's fractional start parameter.
    pub fn getmask2_with_start(
        &self,
        text: &str,
        start: (f64, f64),
    ) -> Result<(u32, u32, Vec<u8>, (i32, i32)), PilError> {
        imagingft::getmask2_with_start(self, text, start)
    }

    /// `getmask2` byte-text variant with Pillow's fractional start parameter.
    pub fn getmask2_bytes_with_start(
        &self,
        text: &[u8],
        start: (f64, f64),
    ) -> Result<(u32, u32, Vec<u8>, (i32, i32)), PilError> {
        let text = pillow_bytes_to_text(text);
        self.getmask2_with_start(&text, start)
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

fn pillow_bytes_to_text(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| char::from(*byte)).collect()
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
