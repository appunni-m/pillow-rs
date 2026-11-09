//! Public root wrappers for Pillow's ImageFont and FreeTypeFont API.

use super::{
    FontMaskAndOffset, FontSizeAndOffset, FontTextRef, FreeTypeFont, ImageFontFaceInfo,
    ImageFontLoadOptions, ImageFontSourceInput, ImageFontTextBoundsCallbacks, ImageFontTextInput,
    ImageFontTextLine, ImageFontTextOptions, ImageFontTextPositionCallbacks, ImageFontTextScaling,
    ImageFontTextWrapCallbacks, ImageFontTextWrapResult, ImageFontVariantOptions,
    ImageFontVariationAxesInput, ImageFontVariationAxis, ImageFontVariationNameInput,
};
use crate::error::PilError;
use crate::raster::Mode;

/// Select the font mask mode used by ImageText.Text for one raster identity.
#[doc(hidden)]
pub fn imagefont_text_font_mode(mode: Mode, embedded_color: bool) -> Mode {
    super::text_layout::text_layout_font_mode(mode, embedded_color)
}

/// Resolve Pillow's text-wrap scaling selector into its core strategy.
#[doc(hidden)]
pub fn imagefont_text_scaling(name: &str, limit: f64) -> ImageFontTextScaling {
    if name == "shrink" {
        ImageFontTextScaling::Shrink { limit }
    } else {
        ImageFontTextScaling::Grow { limit }
    }
}

/// Run the Rust-owned wrapping state machine with host callbacks for metrics
/// and Python protocols.
#[doc(hidden)]
pub fn imagefont_text_wrap_with_callbacks<C: ImageFontTextWrapCallbacks>(
    callbacks: &mut C,
    text: ImageFontTextInput,
    has_height: bool,
    has_scaling: bool,
) -> Result<Option<ImageFontTextWrapResult>, C::Error> {
    super::text_wrap::wrap_with_callbacks(callbacks, text, has_height, has_scaling)
}

/// Split text into Pillow-positioned lines using the shared core layout.
#[doc(hidden)]
pub fn imagefont_text_split_positions(
    font: &FreeTypeFont,
    mode: Mode,
    xy: (f64, f64),
    text: &str,
    spacing: f64,
    options: &ImageFontTextOptions,
) -> Result<Vec<ImageFontTextLine>, PilError> {
    let positioned =
        text_split_positions(font, mode, xy, FontTextRef::String(text), spacing, options)?;
    Ok(positioned
        .into_iter()
        .map(|line| ImageFontTextLine {
            x: line.x,
            y: line.y,
            anchor: line.anchor,
            text: match line.text {
                FontTextRef::String(text) => text.to_owned(),
                FontTextRef::Codepoints(_) => unreachable!("string input retains string lines"),
            },
        })
        .collect())
}

/// Split host-classified text into Pillow-positioned lines.
#[doc(hidden)]
pub fn imagefont_text_split_positions_input(
    font: &FreeTypeFont,
    mode: Mode,
    xy: (f64, f64),
    text: ImageFontTextInput,
    spacing: f64,
    options: &ImageFontTextOptions,
) -> Result<Vec<ImageFontTextLine<ImageFontTextInput>>, PilError> {
    let text = text.into_font_text()?;
    let positioned = text_split_positions(font, mode, xy, text.as_ref(), spacing, options)?;
    Ok(positioned
        .into_iter()
        .map(|line| ImageFontTextLine {
            x: line.x,
            y: line.y,
            anchor: line.anchor,
            text: line.text.to_input(),
        })
        .collect())
}

fn text_split_positions<'a>(
    font: &FreeTypeFont,
    mode: Mode,
    xy: (f64, f64),
    text: FontTextRef<'a>,
    spacing: f64,
    options: &ImageFontTextOptions,
) -> Result<Vec<super::text_layout::PositionedTextLine<'a>>, PilError> {
    let lines = text.split('\n');
    let positioned = super::text_layout::multiline_text_positions(
        xy.0, xy.1, &lines, font, spacing, options, mode,
    )?;
    Ok(positioned)
}

/// Compute the positioned multiline text bounds used by ImageText.Text.
#[doc(hidden)]
pub fn imagefont_text_layout_bbox(
    font: &FreeTypeFont,
    mode: Mode,
    xy: (f64, f64),
    text: &str,
    spacing: f64,
    options: &ImageFontTextOptions,
) -> Result<(f64, f64, f64, f64), PilError> {
    text_layout_bbox(font, mode, xy, FontTextRef::String(text), spacing, options)
}

/// Compute positioned multiline text bounds for host-classified text.
#[doc(hidden)]
pub fn imagefont_text_layout_bbox_input(
    font: &FreeTypeFont,
    mode: Mode,
    xy: (f64, f64),
    text: ImageFontTextInput,
    spacing: f64,
    options: &ImageFontTextOptions,
) -> Result<(f64, f64, f64, f64), PilError> {
    let text = text.into_font_text()?;
    text_layout_bbox(font, mode, xy, text.as_ref(), spacing, options)
}

fn text_layout_bbox(
    font: &FreeTypeFont,
    mode: Mode,
    xy: (f64, f64),
    text: FontTextRef<'_>,
    spacing: f64,
    options: &ImageFontTextOptions,
) -> Result<(f64, f64, f64, f64), PilError> {
    let font_mode = super::text_layout::text_layout_font_mode(mode, options.embedded_color);
    let lines = text_split_positions(font, mode, xy, text, spacing, options)?;
    let mut bounds: Option<(f64, f64, f64, f64)> = None;
    for line in lines {
        let mut bbox_options = options.clone();
        bbox_options.mode = Some(font_mode);
        bbox_options.embedded_color = false;
        bbox_options.anchor = Some(line.anchor);
        bbox_options.align = None;
        bbox_options.stroke_fill = None;
        let bbox = font.getbbox_text_with_options(line.text, &bbox_options)?;
        let line_bounds = (
            f64::from(bbox.0) + line.x,
            f64::from(bbox.1) + line.y,
            f64::from(bbox.2) + line.x,
            f64::from(bbox.3) + line.y,
        );
        bounds = Some(match bounds {
            Some(current) => (
                current.0.min(line_bounds.0),
                current.1.min(line_bounds.1),
                current.2.max(line_bounds.2),
                current.3.max(line_bounds.3),
            ),
            None => line_bounds,
        });
    }
    bounds.ok_or_else(|| PilError::InternalError("text layout produced no lines".into()))
}

/// Aggregate callback-provided line bounds using core-owned min/max traversal.
#[doc(hidden)]
pub fn imagefont_text_layout_bbox_with_callbacks<C: ImageFontTextBoundsCallbacks>(
    callbacks: &mut C,
) -> Result<[C::Value; 4], C::Error> {
    super::text_layout::bounds_with_callbacks(callbacks)
}

/// Split text into positioned lines using the Rust-owned layout strategy.
#[doc(hidden)]
pub fn imagefont_text_split_with_callbacks<C: ImageFontTextPositionCallbacks>(
    callbacks: &mut C,
) -> Result<Vec<C::Line>, C::Error> {
    super::text_layout::split_with_callbacks(callbacks)
}

/// Measure a single ImageText.Text line with its mode-specific font mask mode.
#[doc(hidden)]
pub fn imagefont_text_length(
    font: &FreeTypeFont,
    mode: Mode,
    embedded_color: bool,
    text: ImageFontTextInput,
) -> Result<f32, PilError> {
    let text = text.into_font_text()?;
    if text.as_ref().contains('\n') {
        return Err(PilError::ValueError(
            "can't measure length of multiline text".into(),
        ));
    }
    let font_mode = super::text_layout::text_layout_font_mode(mode, embedded_color);
    let options = ImageFontTextOptions {
        mode: Some(font_mode),
        ..ImageFontTextOptions::default()
    };
    font.getlength_text_with_options(text.as_ref(), &options)
}

/// Wrap Unicode or byte ImageText text in the Rust core.
#[doc(hidden)]
#[allow(clippy::too_many_arguments)]
pub fn imagefont_text_wrap(
    font: &FreeTypeFont,
    mode: Mode,
    text: ImageFontTextInput,
    width: f64,
    height: Option<f64>,
    spacing: f64,
    stroke_width: f32,
    embedded_color: bool,
) -> Result<ImageFontTextWrapResult, PilError> {
    super::text_wrap::wrap_text(
        font,
        mode,
        text,
        width,
        height,
        spacing,
        stroke_width,
        embedded_color,
    )
}

/// Apply Pillow's FreeType font-size scaling strategy while wrapping text.
#[doc(hidden)]
#[allow(clippy::too_many_arguments)]
pub fn imagefont_text_wrap_scaled(
    font: &FreeTypeFont,
    mode: Mode,
    text: ImageFontTextInput,
    width: f64,
    height: f64,
    spacing: f64,
    stroke_width: f32,
    embedded_color: bool,
    scaling: ImageFontTextScaling,
) -> Result<Option<(ImageFontTextWrapResult, Option<FreeTypeFont>)>, PilError> {
    super::text_wrap::wrap_text_scaled(
        font,
        mode,
        text,
        width,
        height,
        spacing,
        stroke_width,
        embedded_color,
        scaling,
    )
}

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

/// Return native `_imagingft.Font.getlength()` for host-classified text input.
pub fn imagefont_native_getlength_26dot6_input(
    font: &FreeTypeFont,
    text: ImageFontTextInput,
) -> Result<i32, PilError> {
    font.native_getlength_26dot6_input(text)
}

/// Return Pillow native `_imagingft.Font.getsize()` size and offset tuple.
pub fn imagefont_native_getsize(
    font: &FreeTypeFont,
    text: &str,
) -> Result<FontSizeAndOffset, PilError> {
    font.native_getsize(text)
}

/// Return native `_imagingft.Font.getsize()` for host-classified text input.
pub fn imagefont_native_getsize_input(
    font: &FreeTypeFont,
    text: ImageFontTextInput,
) -> Result<FontSizeAndOffset, PilError> {
    font.native_getsize_input(text)
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
