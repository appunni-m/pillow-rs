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
pub struct FreeTypeFont {
    engine: imagingft::TrueTypeEngine,
}

/// Pillow `ImageFont.ImageFont`-compatible bitmap font handle.
///
/// The implementation type remains [`pilfont::PilFont`] internally because a
/// Pillow bitmap font is loaded from `.pil` metrics plus a sibling glyph image.
/// The root public alias uses Pillow's base class name.
pub type ImageFont = pilfont::PilFont;

/// Optional Pillow `ImageFont.truetype()` constructor arguments.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ImageFontLoadOptions {
    /// Pillow `index` argument, selecting a face from a collection.
    pub index: Option<usize>,
    /// Pillow `encoding` argument. The BASIC Unicode-compatible Rust path
    /// preserves this as a public API option while selecting the Unicode
    /// charmap, matching the active Pillow oracle rows.
    pub encoding: Option<String>,
    /// Pillow `layout_engine` argument. In the no-raqm oracle configuration,
    /// Pillow accepts this and falls back to BASIC layout.
    pub layout_engine: Option<String>,
}

/// One Pillow `FreeTypeFont.get_variation_axes()` axis record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageFontVariationAxis {
    /// Minimum design coordinate as Pillow reports it.
    pub minimum: i32,
    /// Default design coordinate as Pillow reports it.
    pub default: i32,
    /// Maximum design coordinate as Pillow reports it.
    pub maximum: i32,
    /// Axis name bytes after Pillow's null-byte cleanup.
    pub name: Vec<u8>,
}

/// A Pillow-compatible scalar from a font bounding box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ImageFontBBoxValue {
    /// An integral coordinate is exposed as an integer.
    Integer(i64),
    /// A non-integral coordinate remains a floating-point value.
    Float(f64),
}

/// Normalizes font bounding-box coordinates to Pillow's scalar types.
pub fn normalize_font_bbox(bbox: (f64, f64, f64, f64)) -> [ImageFontBBoxValue; 4] {
    [bbox.0, bbox.1, bbox.2, bbox.3].map(|value| {
        if value.fract() == 0.0 {
            ImageFontBBoxValue::Integer(value as i64)
        } else {
            ImageFontBBoxValue::Float(value)
        }
    })
}

/// Normalize Pillow's public layout-engine selector for the no-raqm build.
///
/// Pillow accepts BASIC, RAQM, and invalid values here. This build exposes
/// only the BASIC engine, so every selector maps to BASIC while the caller can
/// preserve the RAQM compatibility warning for the Python API.
pub fn normalize_layout_engine(value: Option<i64>) -> (&'static str, bool) {
    ("BASIC", value == Some(1))
}

/// Optional Pillow `FreeTypeFont` text-layout/render arguments.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ImageFontTextOptions {
    /// Pillow `mode` argument. BASIC layout ignores most values, but `RGBA`
    /// changes `getmask/getmask2` allocation behavior.
    pub mode: Option<String>,
    /// Pillow `direction` argument. Requires libraqm in Pillow.
    pub direction: Option<String>,
    /// Pillow OpenType feature list. Requires libraqm in Pillow.
    pub features: Option<Vec<String>>,
    /// Whether the host supplied an unrepresentable feature-list value.
    /// Validation of the resulting layout error remains in the core.
    pub features_invalid: bool,
    /// Pillow language tag. Requires libraqm in Pillow.
    pub language: Option<String>,
    /// Pillow text stroke width in pixels.
    pub stroke_width: f32,
    /// Pillow `getmask2(..., stroke_filled=True)` keyword.
    ///
    /// Pillow routes this through `FT_Glyph_StrokeBorder`; the Rust adapter
    /// keeps it explicit so the path cannot be silently treated as the default
    /// filled stroke.
    pub stroke_filled: bool,
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
pub struct ImageFontVariantOptions {
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

/// Host-neutral input for Pillow's variation-axis list requirement.
#[derive(Clone, Debug, PartialEq)]
pub enum ImageFontVariationAxesInput {
    /// A Python list converted to Rust axis values.
    Values(Vec<f32>),
    /// A value that was not a Python list of numeric axis values.
    Invalid,
}

impl FreeTypeFont {
    /// Computes Pillow's multiline text bounding box.
    ///
    /// Line splitting, alignment, line advance, and bbox union are core font
    /// behavior so bindings only marshal the font and text options.
    pub fn multiline_textbbox(
        &self,
        xy: (i32, i32),
        text: &str,
        spacing: i32,
        align: &str,
        options: &ImageFontTextOptions,
    ) -> Result<(i32, i32, i32, i32), PilError> {
        let lines: Vec<&str> = text.split('\n').collect();
        if lines.len() == 1 {
            let bbox = self.getbbox_with_options(text, options)?;
            return Ok((
                xy.0 + bbox.0 as i32,
                xy.1 + bbox.1 as i32,
                xy.0 + bbox.2 as i32,
                xy.1 + bbox.3 as i32,
            ));
        }

        // Pillow ImageText.Text::_split advances by the bottom of "A"'s
        // FreeType bbox, then unions each line's full bbox.
        let line_height = spacing + self.getbbox_with_options("A", options)?.3 as i32;
        let widths = lines
            .iter()
            .map(|line| self.getlength_with_options(line, options))
            .collect::<Result<Vec<_>, _>>()?;
        let max_width = widths.iter().copied().fold(0.0_f32, f32::max);
        let x0 = xy.0 as f64;
        let y0 = xy.1 as f64;
        let mut left = f64::MAX;
        let mut top = f64::MAX;
        let mut right = f64::MIN;
        let mut bottom = f64::MIN;
        for (index, line) in lines.iter().enumerate() {
            let line_y = y0 + index as f64 * line_height as f64;
            let line_x = match align {
                "center" => x0 + (max_width as f64 - widths[index] as f64) / 2.0,
                "right" => x0 + max_width as f64 - widths[index] as f64,
                _ => x0,
            };
            let bbox = self.getbbox_with_options(line, options)?;
            left = left.min(line_x + bbox.0 as f64);
            top = top.min(line_y + bbox.1 as f64);
            right = right.max(line_x + bbox.2 as f64);
            bottom = bottom.max(line_y + bbox.3 as f64);
        }
        Ok((left as i32, top as i32, right as i32, bottom as i32))
    }

    /// Load a TrueType/OpenType face from bytes at the requested Pillow point size.
    pub fn from_bytes(data: Vec<u8>, size: f32) -> Result<Self, PilError> {
        imagingft::load_truetype(data, size)
    }

    /// Load a TrueType/OpenType face from bytes using Pillow constructor options.
    pub fn from_bytes_with_options(
        data: Vec<u8>,
        size: f32,
        options: &ImageFontLoadOptions,
    ) -> Result<Self, PilError> {
        if options.index.is_some_and(|index| index != 0) {
            return Err(PilError::OsError("invalid argument".into()));
        }
        imagingft::load_truetype_with_options(data, size, options)
    }

    /// Loads the same embedded Aileron Regular subset as Pillow 12.2.0.
    ///
    /// Pillow opens this subset with the BASIC layout engine. The regular
    /// TrueType constructor is used here so default fonts and caller-supplied
    /// fonts share the same pure-Rust `fontdone` pipeline.
    pub fn load_default(size: f32) -> Result<Self, PilError> {
        Self::from_bytes(default_aileron::decode(), size)
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
        options: &ImageFontVariantOptions,
    ) -> Result<Self, PilError> {
        if options.index.is_some_and(|index| index != 0) {
            return Err(PilError::OsError("invalid argument".into()));
        }
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
        options: &ImageFontTextOptions,
    ) -> Result<(u32, u32, Vec<u8>), PilError> {
        imagingft::getmask_with_options(self, text, options)
    }

    /// Return Pillow's public `getmask` result for byte text using optional render arguments.
    pub fn getmask_bytes_with_options(
        &self,
        text: &[u8],
        options: &ImageFontTextOptions,
    ) -> Result<(u32, u32, Vec<u8>), PilError> {
        let text = pillow_bytes_to_text(text);
        self.getmask_with_options(&text, options)
    }

    /// Return Pillow's public `(family, style)` font name tuple.
    pub fn getname(&self) -> (Option<&str>, Option<&str>) {
        imagingft::getname_optional(self)
    }

    /// Return Pillow's raw public name tuple, preserving missing face names.
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

    /// Return Pillow native `_imagingft.Font.getlength()` 26.6 advance.
    pub fn native_getlength_26dot6(&self, text: &str) -> Result<i32, PilError> {
        imagingft::native_getlength_26dot6(self, text)
    }

    /// Return Pillow native `_imagingft.Font.getsize()` size and offset tuple.
    pub fn native_getsize(&self, text: &str) -> Result<((i32, i32), (i32, i32)), PilError> {
        imagingft::native_getsize(self, text)
    }

    /// Return Pillow native `_imagingft.Font.render()` mask and offset.
    pub fn native_render(
        &self,
        text: &str,
        options: &ImageFontTextOptions,
    ) -> Result<(u32, u32, Vec<u8>, (i32, i32)), PilError> {
        imagingft::native_render(self, text, options)
    }

    /// Return Pillow native `_imagingft.Font` public face attributes.
    pub fn native_face_attrs(&self) -> (Option<&str>, Option<&str>, u32, u32, u32, u32, u32, i64) {
        imagingft::native_face_attrs(self)
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
        options: &ImageFontTextOptions,
    ) -> Result<f32, PilError> {
        imagingft::getlength_with_options(self, text, options)
    }

    /// Return Pillow's public text length for byte text using optional layout arguments.
    pub fn getlength_bytes_with_options(
        &self,
        text: &[u8],
        options: &ImageFontTextOptions,
    ) -> Result<f32, PilError> {
        let text = pillow_bytes_to_text(text);
        self.getlength_with_options(&text, options)
    }

    /// Return whether the font exposes variation axes.
    pub fn has_variations(&self) -> bool {
        imagingft::has_variations(self)
    }

    /// Return Pillow's public variation-axis records.
    pub fn get_variation_axes(&self) -> Result<Vec<ImageFontVariationAxis>, PilError> {
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

    /// Return Pillow native `_imagingft.Font.getvaraxes()` records.
    pub fn native_getvaraxes(&self) -> Result<Vec<ImageFontVariationAxis>, PilError> {
        imagingft::native_getvaraxes(self)
    }

    /// Return Pillow native `_imagingft.Font.getvarnames()` records.
    pub fn native_getvarnames(&self) -> Result<Vec<Vec<u8>>, PilError> {
        imagingft::native_getvarnames(self)
    }

    /// Set Pillow native `_imagingft.Font` named instance index.
    pub fn native_setvarname(&mut self, instance_index: i64) -> Result<(), PilError> {
        imagingft::native_setvarname(self, instance_index)
    }

    /// Set Pillow native `_imagingft.Font` variation coordinates.
    pub fn native_setvaraxes(&mut self, axes: &[f32]) -> Result<(), PilError> {
        imagingft::native_setvaraxes(self, axes)
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
        options: &ImageFontTextOptions,
    ) -> Result<(f32, f32, f32, f32), PilError> {
        imagingft::getbbox_with_options(self, text, options)
    }

    /// Return Pillow's public text bounding box for byte text using optional layout arguments.
    pub fn getbbox_bytes_with_options(
        &self,
        text: &[u8],
        options: &ImageFontTextOptions,
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
        options: &ImageFontTextOptions,
    ) -> Result<(u32, u32, Vec<u8>, (i32, i32)), PilError> {
        imagingft::getmask2_with_options(self, text, options)
    }

    /// Return Pillow's public `getmask2` result for byte text using optional render arguments.
    pub fn getmask2_bytes_with_options(
        &self,
        text: &[u8],
        options: &ImageFontTextOptions,
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
