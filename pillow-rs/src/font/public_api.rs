//! Public root wrappers for Pillow's ImageFont and FreeTypeFont API.

use super::{
    FontMaskAndOffset, FontSizeAndOffset, FreeTypeFont, ImageFontFaceInfo, ImageFontLoadOptions,
    ImageFontSourceInput, ImageFontTextInput, ImageFontTextOptions, ImageFontVariantOptions,
    ImageFontVariationAxesInput, ImageFontVariationAxis, ImageFontVariationNameInput,
};
use crate::error::PilError;

/// Load a TrueType/OpenType face from bytes at the requested Pillow point size.
pub fn imagefont_from_bytes(data: Vec<u8>, size: f32) -> Result<FreeTypeFont, PilError> {
    FreeTypeFont::from_bytes(data, size)
}

/// Load a TrueType/OpenType face from bytes with Pillow constructor options.
pub fn imagefont_from_bytes_with_options(
    data: Vec<u8>,
    size: f32,
    options: &ImageFontLoadOptions,
) -> Result<FreeTypeFont, PilError> {
    FreeTypeFont::from_bytes_with_options(data, size, options)
}

/// Load a TrueType/OpenType face from a host-marshaled source.
pub fn imagefont_from_source(
    source: ImageFontSourceInput,
    size: f32,
    options: &ImageFontLoadOptions,
) -> Result<FreeTypeFont, PilError> {
    FreeTypeFont::from_source(source, size, options)
}

/// Loads the same embedded default font subset as Pillow.
pub fn imagefont_load_default(size: f32) -> Result<FreeTypeFont, PilError> {
    FreeTypeFont::load_default(size)
}

/// Return the requested Pillow point size for this FreeType font.
pub fn imagefont_size(font: &FreeTypeFont) -> f32 {
    font.font_size()
}

/// Return the non-negative text mask extent for Pillow-style text layout.
pub fn imagefont_text_bbox(font: &FreeTypeFont, text: &str) -> Result<(u32, u32), PilError> {
    font.text_bbox(text)
}

/// Return the non-negative text mask extent for Pillow byte text.
pub fn imagefont_text_bbox_bytes(font: &FreeTypeFont, text: &[u8]) -> Result<(u32, u32), PilError> {
    font.text_bbox_bytes(text)
}

/// Return Pillow's public `(family, style)` font name tuple.
pub fn imagefont_getname(font: &FreeTypeFont) -> (Option<&str>, Option<&str>) {
    font.getname()
}

/// Return Pillow's raw public name tuple, preserving missing face names.
pub fn imagefont_getname_optional(font: &FreeTypeFont) -> (Option<&str>, Option<&str>) {
    font.getname_optional()
}

/// Return Pillow's public ascent/descent metrics.
pub fn imagefont_getmetrics(font: &FreeTypeFont) -> (u32, u32) {
    font.getmetrics()
}

/// Return Pillow's public text length in pixels.
pub fn imagefont_getlength(font: &FreeTypeFont, text: &str) -> Result<f32, PilError> {
    font.getlength(text)
}

/// Return Pillow native `_imagingft.Font.getlength()` 26.6 advance.
pub fn imagefont_native_getlength_26dot6(font: &FreeTypeFont, text: &str) -> Result<i32, PilError> {
    font.native_getlength_26dot6(text)
}

/// Return Pillow native `_imagingft.Font.getsize()` size and offset tuple.
pub fn imagefont_native_getsize(
    font: &FreeTypeFont,
    text: &str,
) -> Result<FontSizeAndOffset, PilError> {
    font.native_getsize(text)
}

/// Return Pillow native `_imagingft.Font.render()` mask and offset.
pub fn imagefont_native_render(
    font: &FreeTypeFont,
    text: &str,
    options: &ImageFontTextOptions,
) -> Result<FontMaskAndOffset, PilError> {
    font.native_render(text, options)
}

/// Return Pillow native render output for host-classified text input.
pub fn imagefont_native_render_input(
    font: &FreeTypeFont,
    text: ImageFontTextInput,
    options: &ImageFontTextOptions,
) -> Result<FontMaskAndOffset, PilError> {
    font.native_render_input(text, options)
}

/// Return Pillow native `_imagingft.Font` public face attributes.
pub fn imagefont_native_face_attrs(
    font: &FreeTypeFont,
) -> (Option<&str>, Option<&str>, u32, u32, u32, u32, u32, i64) {
    font.native_face_attrs()
}

/// Return the active font face capabilities reported by fontdone.
pub fn imagefont_native_face_info(font: &FreeTypeFont) -> ImageFontFaceInfo<'_> {
    font.native_face_info()
}

/// Return the active font-driver format reported by fontdone.
pub fn imagefont_font_format(font: &FreeTypeFont) -> &'static str {
    font.font_format()
}

/// Return whether the active font face has scalable outlines.
pub fn imagefont_is_scalable(font: &FreeTypeFont) -> bool {
    font.is_scalable()
}

/// Return Pillow's public text length for byte text.
pub fn imagefont_getlength_bytes(font: &FreeTypeFont, text: &[u8]) -> Result<f32, PilError> {
    font.getlength_bytes(text)
}

/// Return Pillow's public text length with optional layout arguments.
pub fn imagefont_getlength_with_options(
    font: &FreeTypeFont,
    text: &str,
    options: &ImageFontTextOptions,
) -> Result<f32, PilError> {
    font.getlength_with_options(text, options)
}

/// Return Pillow's public text length for byte text with optional layout arguments.
pub fn imagefont_getlength_bytes_with_options(
    font: &FreeTypeFont,
    text: &[u8],
    options: &ImageFontTextOptions,
) -> Result<f32, PilError> {
    font.getlength_bytes_with_options(text, options)
}

/// Return whether the font exposes variation axes.
pub fn imagefont_has_variations(font: &FreeTypeFont) -> bool {
    font.has_variations()
}

/// Create a Pillow-compatible variant font.
pub fn imagefont_variant(font: &FreeTypeFont, size: Option<f32>) -> Result<FreeTypeFont, PilError> {
    font.font_variant(size)
}

/// Create a Pillow-compatible variant font with public override arguments.
pub fn imagefont_variant_with_options(
    font: &FreeTypeFont,
    options: &ImageFontVariantOptions,
) -> Result<FreeTypeFont, PilError> {
    font.font_variant_with_options(options)
}

/// Return Pillow's public variation-axis records.
pub fn imagefont_get_variation_axes(
    font: &FreeTypeFont,
) -> Result<Vec<ImageFontVariationAxis>, PilError> {
    font.get_variation_axes()
}

/// Return Pillow's public named-variation style names.
pub fn imagefont_get_variation_names(font: &FreeTypeFont) -> Result<Vec<Vec<u8>>, PilError> {
    font.get_variation_names()
}

/// Set a named variation instance by Pillow-style name bytes.
pub fn imagefont_set_variation_by_name(
    font: &mut FreeTypeFont,
    name: &[u8],
) -> Result<(), PilError> {
    font.set_variation_by_name(name)
}

/// Set a named variation instance after applying Pillow's host input rules.
pub fn imagefont_set_variation_by_name_input(
    font: &mut FreeTypeFont,
    name: ImageFontVariationNameInput,
) -> Result<(), PilError> {
    font.set_variation_by_name_input(name)
}

/// Set variation design coordinates from Pillow-style user coordinates.
pub fn imagefont_set_variation_by_axes(
    font: &mut FreeTypeFont,
    axes: &[f32],
) -> Result<(), PilError> {
    font.set_variation_by_axes(axes)
}

/// Validate and set variation axes using Pillow's list-only public contract.
pub fn imagefont_set_variation_by_axes_input(
    font: &mut FreeTypeFont,
    input: ImageFontVariationAxesInput,
) -> Result<(), PilError> {
    match input {
        ImageFontVariationAxesInput::Values(axes) => imagefont_set_variation_by_axes(font, &axes),
        ImageFontVariationAxesInput::Invalid => {
            Err(PilError::TypeError("argument must be a list".into()))
        }
    }
}

/// Return Pillow native `_imagingft.Font.getvaraxes()` records.
pub fn imagefont_native_getvaraxes(
    font: &FreeTypeFont,
) -> Result<Vec<ImageFontVariationAxis>, PilError> {
    font.native_getvaraxes()
}

/// Return Pillow native `_imagingft.Font.getvarnames()` records.
pub fn imagefont_native_getvarnames(font: &FreeTypeFont) -> Result<Vec<Vec<u8>>, PilError> {
    font.native_getvarnames()
}

/// Set Pillow native `_imagingft.Font` named instance index.
pub fn imagefont_native_setvarname(
    font: &mut FreeTypeFont,
    instance_index: i64,
) -> Result<(), PilError> {
    font.native_setvarname(instance_index)
}

/// Set Pillow native `_imagingft.Font` variation coordinates.
pub fn imagefont_native_setvaraxes(font: &mut FreeTypeFont, axes: &[f32]) -> Result<(), PilError> {
    font.native_setvaraxes(axes)
}

/// Return Pillow's public text bounding box.
pub fn imagefont_getbbox(
    font: &FreeTypeFont,
    text: &str,
) -> Result<(i32, i32, i32, i32), PilError> {
    font.getbbox(text)
}

/// Return Pillow's public text bounding box for byte text.
pub fn imagefont_getbbox_bytes(
    font: &FreeTypeFont,
    text: &[u8],
) -> Result<(i32, i32, i32, i32), PilError> {
    font.getbbox_bytes(text)
}

/// Return Pillow's public binary-mode text bounding box.
#[cfg(feature = "test-api")]
pub fn imagefont_getbbox_binary(
    font: &FreeTypeFont,
    text: &str,
) -> Result<(i32, i32, i32, i32), PilError> {
    font.getbbox_binary(text)
}

/// Return Pillow's public binary-mode text bounding box for byte text.
#[cfg(feature = "test-api")]
pub fn imagefont_getbbox_binary_bytes(
    font: &FreeTypeFont,
    text: &[u8],
) -> Result<(i32, i32, i32, i32), PilError> {
    font.getbbox_binary_bytes(text)
}

/// Return Pillow's public text bounding box with optional layout arguments.
pub fn imagefont_getbbox_with_options(
    font: &FreeTypeFont,
    text: &str,
    options: &ImageFontTextOptions,
) -> Result<(f32, f32, f32, f32), PilError> {
    font.getbbox_with_options(text, options)
}

/// Return a text bounding box using Pillow's optional draw-font rules.
pub fn imagefont_getbbox_with_optional_font(
    font: Option<&FreeTypeFont>,
    size: Option<f32>,
    text: &str,
    options: &ImageFontTextOptions,
) -> Result<(f32, f32, f32, f32), PilError> {
    crate::font::with_text_font(font, size, |font| font.getbbox_with_options(text, options))
}

/// Return an offset text bounding box using Pillow's optional draw-font rules.
pub fn imagefont_textbbox_at_with_optional_font(
    font: Option<&FreeTypeFont>,
    size: Option<f32>,
    xy: (i32, i32),
    text: &str,
    options: &ImageFontTextOptions,
) -> Result<(i32, i32, i32, i32), PilError> {
    let bbox = imagefont_getbbox_with_optional_font(font, size, text, options)?;
    Ok((
        xy.0 + bbox.0 as i32,
        xy.1 + bbox.1 as i32,
        xy.0 + bbox.2 as i32,
        xy.1 + bbox.3 as i32,
    ))
}

/// Return an offset text bounding box using host-classified text input.
pub fn imagefont_textbbox_at_with_optional_font_input(
    font: Option<&FreeTypeFont>,
    size: Option<f32>,
    xy: (i32, i32),
    text: ImageFontTextInput,
    options: &ImageFontTextOptions,
) -> Result<(i32, i32, i32, i32), PilError> {
    let bbox = crate::font::with_text_font(font, size, |font| {
        font.getbbox_input_with_options(text, options)
    })?;
    Ok((
        xy.0 + bbox.0 as i32,
        xy.1 + bbox.1 as i32,
        xy.0 + bbox.2 as i32,
        xy.1 + bbox.3 as i32,
    ))
}

/// Return text length using Pillow's optional draw-font rules.
pub fn imagefont_getlength_with_optional_font(
    font: Option<&FreeTypeFont>,
    size: Option<f32>,
    text: &str,
    options: &ImageFontTextOptions,
) -> Result<f32, PilError> {
    crate::font::with_text_font(font, size, |font| {
        font.getlength_with_options(text, options)
    })
}

/// Return text length using optional-font rules and host-classified text input.
pub fn imagefont_getlength_with_optional_font_input(
    font: Option<&FreeTypeFont>,
    size: Option<f32>,
    text: ImageFontTextInput,
    options: &ImageFontTextOptions,
) -> Result<f32, PilError> {
    crate::font::with_text_font(font, size, |font| {
        font.getlength_input_with_options(text, options)
    })
}

/// Return Pillow's public multiline text bounding box.
pub fn imagefont_multiline_textbbox(
    font: &FreeTypeFont,
    xy: (i32, i32),
    text: &str,
    spacing: i32,
    align: &str,
    options: &ImageFontTextOptions,
) -> Result<(i32, i32, i32, i32), PilError> {
    font.multiline_textbbox(xy, text, spacing, align, options)
}

/// Return a multiline text bounding box using Pillow's optional draw-font rules.
pub fn imagefont_multiline_textbbox_with_optional_font(
    font: Option<&FreeTypeFont>,
    size: Option<f32>,
    xy: (i32, i32),
    text: &str,
    spacing: i32,
    align: &str,
    options: &ImageFontTextOptions,
) -> Result<(i32, i32, i32, i32), PilError> {
    crate::font::with_text_font(font, size, |font| {
        font.multiline_textbbox(xy, text, spacing, align, options)
    })
}

/// Return a multiline text bounding box using host-classified text input.
pub fn imagefont_multiline_textbbox_with_optional_font_input(
    font: Option<&FreeTypeFont>,
    size: Option<f32>,
    xy: (i32, i32),
    text: ImageFontTextInput,
    spacing: i32,
    align: &str,
    options: &ImageFontTextOptions,
) -> Result<(i32, i32, i32, i32), PilError> {
    crate::font::with_text_font(font, size, |font| {
        font.multiline_textbbox_input(xy, text, spacing, align, options)
    })
}

/// Return Pillow's public text bounding box for byte text with optional layout arguments.
pub fn imagefont_getbbox_bytes_with_options(
    font: &FreeTypeFont,
    text: &[u8],
    options: &ImageFontTextOptions,
) -> Result<(f32, f32, f32, f32), PilError> {
    font.getbbox_bytes_with_options(text, options)
}

/// Return Pillow's public text bounding box for host-classified text input.
pub fn imagefont_getbbox_input_with_options(
    font: &FreeTypeFont,
    text: ImageFontTextInput,
    options: &ImageFontTextOptions,
) -> Result<(f32, f32, f32, f32), PilError> {
    font.getbbox_input_with_options(text, options)
}

/// Return the Pillow-compatible grayscale text mask.
pub fn imagefont_getmask(font: &FreeTypeFont, text: &str) -> Result<(u32, u32, Vec<u8>), PilError> {
    font.getmask(text)
}

/// Return the Pillow-compatible grayscale text mask for byte text.
pub fn imagefont_getmask_bytes(
    font: &FreeTypeFont,
    text: &[u8],
) -> Result<(u32, u32, Vec<u8>), PilError> {
    font.getmask_bytes(text)
}

/// Render a Pillow-compatible mask with offset.
pub fn imagefont_getmask2(font: &FreeTypeFont, text: &str) -> Result<FontMaskAndOffset, PilError> {
    font.getmask2(text)
}

/// Render a Pillow-compatible mask with offset for byte text.
pub fn imagefont_getmask2_bytes(
    font: &FreeTypeFont,
    text: &[u8],
) -> Result<FontMaskAndOffset, PilError> {
    font.getmask2_bytes(text)
}

/// Return the Pillow-compatible grayscale text mask with optional render arguments.
pub fn imagefont_getmask_with_options(
    font: &FreeTypeFont,
    text: &str,
    options: &ImageFontTextOptions,
) -> Result<(u32, u32, Vec<u8>), PilError> {
    font.getmask_with_options(text, options)
}

/// Return the Pillow-compatible grayscale text mask for byte text with optional render arguments.
pub fn imagefont_getmask_bytes_with_options(
    font: &FreeTypeFont,
    text: &[u8],
    options: &ImageFontTextOptions,
) -> Result<(u32, u32, Vec<u8>), PilError> {
    font.getmask_bytes_with_options(text, options)
}

/// Return the Pillow-compatible grayscale text mask for host-classified text.
pub fn imagefont_getmask_input_with_options(
    font: &FreeTypeFont,
    text: ImageFontTextInput,
    options: &ImageFontTextOptions,
) -> Result<(u32, u32, Vec<u8>), PilError> {
    font.getmask_input_with_options(text, options)
}

/// Render a Pillow-compatible mask with a fractional raster start.
pub fn imagefont_getmask2_with_start(
    font: &FreeTypeFont,
    text: &str,
    start: (f64, f64),
) -> Result<FontMaskAndOffset, PilError> {
    font.getmask2_with_start(text, start)
}

/// Render a Pillow-compatible byte-text mask with a fractional raster start.
pub fn imagefont_getmask2_bytes_with_start(
    font: &FreeTypeFont,
    text: &[u8],
    start: (f64, f64),
) -> Result<FontMaskAndOffset, PilError> {
    font.getmask2_bytes_with_start(text, start)
}

/// Render a Pillow-compatible mask with optional render arguments.
pub fn imagefont_getmask2_with_options(
    font: &FreeTypeFont,
    text: &str,
    options: &ImageFontTextOptions,
) -> Result<FontMaskAndOffset, PilError> {
    font.getmask2_with_options(text, options)
}

/// Render a Pillow-compatible byte-text mask with optional render arguments.
pub fn imagefont_getmask2_bytes_with_options(
    font: &FreeTypeFont,
    text: &[u8],
    options: &ImageFontTextOptions,
) -> Result<FontMaskAndOffset, PilError> {
    font.getmask2_bytes_with_options(text, options)
}

/// Render a Pillow-compatible mask with offset for host-classified text.
pub fn imagefont_getmask2_input_with_options(
    font: &FreeTypeFont,
    text: ImageFontTextInput,
    options: &ImageFontTextOptions,
) -> Result<FontMaskAndOffset, PilError> {
    font.getmask2_input_with_options(text, options)
}

/// Return text length for host-classified text input with optional layout.
pub fn imagefont_getlength_input_with_options(
    font: &FreeTypeFont,
    text: ImageFontTextInput,
    options: &ImageFontTextOptions,
) -> Result<f32, PilError> {
    font.getlength_input_with_options(text, options)
}

/// Render a font mask and apply Pillow's optional transpose operation.
pub fn imagefont_get_transposed_mask(
    font: &FreeTypeFont,
    text: &str,
    orientation: Option<&str>,
) -> Result<(u32, u32, Vec<u8>), PilError> {
    font.get_transposed_mask(text, orientation)
}

/// Return a transposed mask for host-classified text input.
pub fn imagefont_get_transposed_mask_input(
    font: &FreeTypeFont,
    text: ImageFontTextInput,
    orientation: Option<&str>,
) -> Result<(u32, u32, Vec<u8>), PilError> {
    font.get_transposed_mask_input(text, orientation)
}

/// Return Pillow-compatible binary-mode RGBA text rendering.
#[cfg(feature = "test-api")]
pub fn imagefont_render_text_binary(
    font: &FreeTypeFont,
    text: &str,
    fill: (u8, u8, u8, u8),
    spacing: f32,
) -> Result<(u32, u32, Vec<u8>), PilError> {
    font.render_text_binary(text, fill, spacing)
}
