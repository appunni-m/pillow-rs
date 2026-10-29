//! Pillow-compatible `ImageDraw` primitives.
//!
//! [`Draw`] records drawing operations against an [`Image`] and keeps enough
//! mode metadata to convert the drawn result back to the original Pillow mode.
//! Coordinates are integer pixel coordinates. Colors are normalized RGBA byte
//! tuples before mode-specific drawing rules are applied.

use crate::raster::{DynamicImage, RgbaImage};

use crate::error::PilError;
use crate::image::Image;

type DrawColor = (u8, u8, u8, u8);
type ShapeInks = (Option<DrawColor>, Option<DrawColor>);

/// Drawing context for Pillow-style image mutation.
///
/// This is the Rust equivalent of `ImageDraw.Draw(image)`. Methods queue or
/// apply drawing operations and [`Draw::image_clone`] returns the updated image
/// with the original mode restored where possible.
#[derive(Debug)]
pub struct Draw {
    image: Image,
    /// Original image mode used to restore the public image on `image_clone()`.
    orig_mode: Option<String>,
    /// Requested drawing mode, which may differ from `orig_mode` for Pillow's
    /// RGBA-on-RGB alpha-compositing context.
    draw_mode: Option<String>,
}

/// Host-neutral coordinate input for ImageDraw point-based primitives.
#[derive(Debug, Clone)]
pub enum DrawPointsInput {
    /// Flat `x0, y0, x1, y1, ...` values.
    Flat(Vec<i32>),
    /// Nested `(x, y)` point values.
    Nested(Vec<Vec<i32>>),
    /// A list/tuple sequence whose elements are not supported coordinates.
    InvalidSequence,
    /// A value that could not be represented as either supported sequence.
    Invalid,
}

/// Host-neutral center input for `ImageDraw.circle`.
#[derive(Debug, Clone)]
pub enum DrawCircleCenterInput {
    /// A sequence whose first two values are the center coordinates.
    Values(Vec<f64>),
    /// An integer is accepted by the host boundary but has no subscriptable
    /// center, matching Pillow's direct `xy[0]` diagnostic.
    Integer,
    /// Text reaches Pillow's arithmetic path after indexing the first
    /// character, so preserve its type-specific subtraction diagnostic.
    Text,
    /// A mapping is indexed by integer zero before Pillow performs arithmetic;
    /// an empty mapping therefore raises `KeyError(0)`.
    Mapping,
    /// A value that could not be represented as a numeric sequence.
    Invalid,
}

/// Host-neutral coordinate input for ImageDraw rectangle-like primitives.
#[derive(Debug, Clone)]
pub enum DrawBoxInput {
    /// Flat `x0, y0, x1, y1` values.
    Flat(Vec<i32>),
    /// Nested `((x0, y0), (x1, y1))` values.
    Nested(Vec<Vec<i32>>),
    /// A value that could not be represented as a supported sequence.
    Invalid,
}

/// Host-neutral color input for ImageDraw operations.
#[derive(Debug, Clone)]
pub enum DrawColorInput {
    /// No fill or outline was supplied.
    None,
    /// A Pillow color name or hexadecimal color string.
    String(String),
    /// An integer color value.
    Integer(i64),
    /// A floating-point color value, used by `F` mode.
    Float(f64),
    /// A tuple/list of color components.
    Components(Vec<i64>),
    /// A value that is not a supported color form.
    Invalid,
}

/// Normalize Pillow's optional draw width at the core boundary.
pub fn normalize_draw_width(width: Option<u32>) -> u32 {
    // An omitted width defaults to one, but an explicit zero is preserved.
    // Pillow forwards that distinction to the C primitive: line-like paths
    // handle zero as their own default while outline paths treat it as a
    // no-op.  Collapsing `Some(0)` here makes those public contracts
    // indistinguishable before the backend can apply the operation-specific
    // rule.
    width.unwrap_or(1)
}

/// Normalizes a flat or nested ImageDraw bounding box.
pub fn normalize_draw_box(input: DrawBoxInput) -> Result<(i32, i32, i32, i32), PilError> {
    let error = || PilError::TypeError("coordinate list must contain exactly 2 coordinates".into());
    match input {
        DrawBoxInput::Flat(values) if values.len() == 4 => {
            Ok((values[0], values[1], values[2], values[3]))
        }
        DrawBoxInput::Nested(values)
            if values.len() == 2 && values.iter().all(|point| point.len() == 2) =>
        {
            Ok((values[0][0], values[0][1], values[1][0], values[1][1]))
        }
        // Pillow's nested coordinate parser reports a malformed point as a
        // value error, while a flat sequence with the wrong number of values
        // keeps the generic coordinate-list type error.
        DrawBoxInput::Nested(values)
            if values.len() == 2 && values.iter().any(|point| point.len() != 2) =>
        {
            Err(PilError::ValueError("incorrect coordinate type".into()))
        }
        // Pillow's three-value flat parser reports an arity mismatch as a
        // value error; other flat forms and nested sequences retain the
        // generic type diagnostic used by its coordinate unpacker.
        DrawBoxInput::Flat(values) if values.len() == 3 => {
            Err(PilError::ValueError("wrong number of coordinates".into()))
        }
        DrawBoxInput::Flat(_) | DrawBoxInput::Nested(_) | DrawBoxInput::Invalid => Err(error()),
    }
}

/// Normalizes the rounded-rectangle box, preserving its two-scalar Pillow
/// diagnostic before falling back to the shared box validation.
pub fn normalize_rounded_rectangle_box(
    input: DrawBoxInput,
) -> Result<(i32, i32, i32, i32), PilError> {
    if matches!(&input, DrawBoxInput::Flat(values) if values.len() == 2) {
        return Err(PilError::ValueError(
            "not enough values to unpack (expected 4, got 2)".into(),
        ));
    }
    normalize_draw_box(input)
}

fn normalize_draw_points(
    input: DrawPointsInput,
    allow_short: bool,
) -> Result<Vec<(i32, i32)>, PilError> {
    // Pillow's _imaging.c coordinate parser distinguishes an odd flat list
    // ("wrong number of coordinates") from a malformed nested point
    // ("incorrect coordinate type"); preserve that public split here.
    let too_few =
        || PilError::TypeError("coordinate list must contain at least 2 coordinates".into());
    let invalid_input = || PilError::TypeError("argument must be sequence".into());
    let wrong_number = || PilError::ValueError("wrong number of coordinates".into());
    let wrong_point = || PilError::ValueError("incorrect coordinate type".into());
    match input {
        DrawPointsInput::Flat(values) => {
            if values.len() % 2 != 0 {
                return Err(wrong_number());
            }
            if allow_short && values.len() < 4 {
                return Ok(Vec::new());
            }
            if values.len() < 4 {
                return Err(too_few());
            }
            Ok(values
                .chunks_exact(2)
                .map(|point| (point[0], point[1]))
                .collect())
        }
        DrawPointsInput::Nested(values) => {
            if values.iter().any(|point| point.len() != 2) {
                return Err(wrong_point());
            }
            if values.len() < 2 {
                if allow_short {
                    return Ok(Vec::new());
                }
                return Err(too_few());
            }
            Ok(values
                .into_iter()
                .map(|point| (point[0], point[1]))
                .collect())
        }
        DrawPointsInput::InvalidSequence => Err(wrong_point()),
        DrawPointsInput::Invalid => Err(invalid_input()),
    }
}

fn normalize_draw_point_input(input: DrawPointsInput) -> Result<Vec<(i32, i32)>, PilError> {
    let invalid_input = || PilError::TypeError("argument must be sequence".into());
    let wrong_number = || PilError::ValueError("wrong number of coordinates".into());
    let wrong_point = || PilError::ValueError("incorrect coordinate type".into());
    match input {
        DrawPointsInput::Flat(values) => {
            if values.is_empty() {
                return Ok(Vec::new());
            }
            if values.len() % 2 != 0 {
                return Err(wrong_number());
            }
            Ok(values
                .chunks_exact(2)
                .map(|point| (point[0], point[1]))
                .collect())
        }
        DrawPointsInput::Nested(values) => {
            if values.iter().any(|point| point.len() != 2) {
                return Err(wrong_point());
            }
            Ok(values
                .into_iter()
                .map(|point| (point[0], point[1]))
                .collect())
        }
        DrawPointsInput::InvalidSequence => Err(wrong_point()),
        DrawPointsInput::Invalid => Err(invalid_input()),
    }
}

/// Normalizes the center accepted by Pillow's circle wrapper.
pub fn normalize_draw_circle_center(input: DrawCircleCenterInput) -> Result<(f64, f64), PilError> {
    match input {
        DrawCircleCenterInput::Values(values) if values.len() >= 2 => Ok((values[0], values[1])),
        // Pillow's circle wrapper indexes the first two sequence elements
        // directly; a short numeric sequence therefore raises IndexError
        // rather than the TypeError used by its other coordinate parsers.
        DrawCircleCenterInput::Values(_) => {
            Err(PilError::IndexError("tuple index out of range".into()))
        }
        DrawCircleCenterInput::Integer => Err(PilError::TypeError(
            "'int' object is not subscriptable".into(),
        )),
        DrawCircleCenterInput::Text => Err(PilError::TypeError(
            "unsupported operand type(s) for -: 'str' and 'int'".into(),
        )),
        DrawCircleCenterInput::Mapping => Err(PilError::KeyErrorInt(0)),
        DrawCircleCenterInput::Invalid => Err(PilError::TypeError(
            "'NoneType' object is not subscriptable".into(),
        )),
    }
}

/// Host-neutral `ImageDraw.regular_polygon` bounding-circle input.
#[derive(Debug, Clone, Copy)]
pub enum RegularPolygonCircle {
    /// `(x, y, radius)` form.
    Flat(f64, f64, f64),
    /// `((x, y), radius)` form.
    Nested(f64, f64, f64),
    /// A value that was not one of the accepted forms.
    Invalid,
}

/// Host-neutral side-count input for `ImageDraw.regular_polygon`.
#[derive(Debug, Clone, Copy)]
pub enum RegularPolygonSides {
    /// The integer supplied by the caller.
    Value(i64),
    /// A non-integer value.
    Invalid,
}

fn draw_color_error() -> PilError {
    PilError::TypeError(
        // Pillow's ImageDraw._getink() reports this exact message for a
        // component sequence with an unsupported arity.
        "color must be int, or tuple of one, three or four elements".to_owned(),
    )
}

fn is_l16_mode(mode: &str) -> bool {
    matches!(mode, "I;16" | "I;16L" | "I;16B" | "I;16N")
}

fn draw_float_color_error(mode: &str) -> PilError {
    if matches!(
        mode,
        "1" | "L" | "P" | "I" | "I;16" | "I;16L" | "I;16B" | "I;16N"
    ) {
        PilError::TypeError("color must be int or single-element tuple".to_owned())
    } else {
        PilError::TypeError("color must be int or tuple".to_owned())
    }
}

fn draw_byte(value: i64) -> u8 {
    // Pillow clamps tuple components to the byte range before handing them to
    // the ImagingDraw backend; integer inks use a separate packed path below.
    value.clamp(0, 255) as u8
}

fn resolve_integer_color(mode: &str, value: i64) -> Result<(u8, u8, u8, u8), PilError> {
    if mode == "F" {
        let bytes = (value as f32).to_le_bytes();
        return Ok((bytes[0], bytes[1], bytes[2], bytes[3]));
    }
    if mode == "I" {
        let value = i32::try_from(value).map_err(|_| draw_color_error())?;
        let bytes = value.to_le_bytes();
        return Ok((bytes[0], bytes[1], bytes[2], bytes[3]));
    }
    if is_l16_mode(mode) {
        // Pillow's I;16* draw path packs an integer as a native 32-bit ink
        // and stores its low two bytes. Keep the packed little-endian bytes
        // here; the native canvas applies the declared I;16B byte-order tag
        // when turning them into its host-native u16 sample.
        let bytes = (value as i32 as u16).to_le_bytes();
        return Ok((bytes[0], bytes[1], 0, 255));
    }
    let signed_value = value;
    let packed = i32::try_from(value)
        .map_err(|_| draw_color_error())?
        .to_le_bytes();
    let value = draw_byte(value);
    Ok(match mode {
        // Multi-band integer inks are packed little-endian by Pillow's
        // ImagingDraw implementation, including values outside one byte.
        "RGB" | "YCbCr" | "HSV" => (packed[0], packed[1], packed[2], 255),
        "RGBA" => (packed[0], packed[1], packed[2], packed[3]),
        "LA" => (
            packed[0],
            packed[0],
            packed[0],
            if signed_value < 0 { 255 } else { 0 },
        ),
        "CMYK" => (packed[0], packed[1], packed[2], packed[3]),
        _ => (value, value, value, 255),
    })
}

fn resolve_component_color(mode: &str, values: &[i64]) -> Result<(u8, u8, u8, u8), PilError> {
    if is_l16_mode(mode) {
        if values.len() != 1 {
            return Err(PilError::TypeError(
                "color must be int or single-element tuple".to_owned(),
            ));
        }
        let bytes = (values[0] as i32 as u16).to_le_bytes();
        return Ok((bytes[0], bytes[1], 0, 255));
    }
    let components = || values.iter().copied().map(draw_byte).collect::<Vec<_>>();
    if mode == "PA" {
        let components = components();
        return match components.as_slice() {
            [value] => Ok((*value, *value, *value, 0)),
            [value, alpha] => Ok((*value, *value, *value, *alpha)),
            _ => Err(PilError::TypeError(
                "color must be int, or tuple of one or two elements".to_owned(),
            )),
        };
    }
    if mode == "LA" {
        let components = components();
        return match components.as_slice() {
            [value, alpha] => Ok((*value, *value, *value, *alpha)),
            _ => Err(PilError::TypeError(
                "color must be int, or tuple of one or two elements".to_owned(),
            )),
        };
    }
    let components = components();
    match components.as_slice() {
        [value] if matches!(mode, "L" | "1" | "P") => Ok((*value, *value, *value, 255)),
        [r, g, b] => Ok((*r, *g, *b, 255)),
        [r, g, b, a] => Ok((*r, *g, *b, *a)),
        _ => Err(draw_color_error()),
    }
}

impl Draw {
    /// Creates a drawing context for `image`.
    ///
    /// `explicit_mode` is an optional PIL mode override for cases where the
    /// image's raw DynamicImage mode differs from the logical PIL mode
    /// (e.g. "P" stored as Luma8, "CMYK" stored as Rgba8).
    pub fn new(image: Image, explicit_mode: Option<String>) -> Self {
        // TODO(cpu-migration): accept `Option<Mode>` and keep string parsing
        // in the binding adapter.
        let original_mode = image.mode().ok();
        let draw_mode = explicit_mode.or_else(|| original_mode.clone());
        Draw {
            image,
            orig_mode: original_mode,
            draw_mode,
        }
    }

    /// Validate Pillow's optional draw-mode override against the destination.
    ///
    /// RGB images may be drawn through an RGBA context for alpha blending. All
    /// other explicit modes must match the destination image mode exactly.
    pub fn validate_mode(&self) -> Result<(), PilError> {
        let Some(requested) = self.draw_mode.as_deref() else {
            return Ok(());
        };
        let actual = self.image.mode()?;
        if requested != actual && !(requested == "RGBA" && actual == "RGB") {
            return Err(PilError::ValueError("mode mismatch".to_owned()));
        }
        Ok(())
    }

    /// Return the effective PIL mode for drawing operations.
    /// Uses the explicit mode if set, otherwise falls back to the image's mode.
    fn effective_mode(&self) -> String {
        self.draw_mode
            .clone()
            .or_else(|| self.image.mode().ok())
            .unwrap_or_else(|| "RGBA".to_string())
    }

    /// Whether Pillow requested an RGBA drawing context over an RGB image.
    fn alpha_blend_rgb(&self) -> bool {
        self.draw_mode.as_deref() == Some("RGBA") && self.orig_mode.as_deref() == Some("RGB")
    }

    /// Validate text options against the destination's Pillow mode.
    pub fn validate_text_options(
        &self,
        options: &crate::font::ImageFontTextOptions,
    ) -> Result<(), PilError> {
        if options.embedded_color && !matches!(self.effective_mode().as_str(), "RGB" | "RGBA") {
            return Err(PilError::ValueError(
                "Embedded color supported only in RGB and RGBA modes".into(),
            ));
        }
        Ok(())
    }

    /// Resolves a host-neutral color according to Pillow's drawing-mode rules.
    pub fn color_with_input(&self, input: DrawColorInput) -> Result<(u8, u8, u8, u8), PilError> {
        let mode = self.effective_mode();
        match input {
            DrawColorInput::None => Ok(self.default_shape_ink().unwrap_or((0, 0, 0, 255))),
            DrawColorInput::String(value) => crate::color::parse_color_str(&value),
            DrawColorInput::Integer(value) => resolve_integer_color(&mode, value),
            DrawColorInput::Float(value) if mode == "F" => {
                let bytes = (value as f32).to_le_bytes();
                Ok((bytes[0], bytes[1], bytes[2], bytes[3]))
            }
            DrawColorInput::Float(_) => Err(draw_float_color_error(&mode)),
            DrawColorInput::Components(values) => resolve_component_color(&mode, &values),
            DrawColorInput::Invalid => Err(draw_color_error()),
        }
    }

    /// Resolve an ImageDraw text ink according to Pillow's mode-specific
    /// default. ImageDraw initializes text's implicit ink from
    /// `draw_ink(-1)` (or `draw_ink(1)` for `I`/`F`), which is not the same
    /// as the geometric primitive default represented by `None` here.
    pub fn text_color_with_input(
        &self,
        input: DrawColorInput,
    ) -> Result<(u8, u8, u8, u8), PilError> {
        if !matches!(input, DrawColorInput::None) {
            return self.color_with_input(input);
        }

        match self.effective_mode().as_str() {
            "RGB" | "RGBA" | "RGBa" | "LA" | "PA" | "CMYK" | "YCbCr" | "HSV" => {
                Ok((255, 255, 255, 255))
            }
            "I" => Ok((1, 0, 0, 0)),
            "F" => Ok((0, 0, 128, 63)),
            _ => Ok((0, 0, 0, 255)),
        }
    }

    /// Validates a bitmap fill and delegates the complete operation to core.
    pub fn bitmap_with_input(
        &mut self,
        x: i32,
        y: i32,
        bitmap: &Image,
        input: DrawColorInput,
    ) -> Result<(), PilError> {
        let mode = self.effective_mode();
        if matches!(input, DrawColorInput::None) {
            return self.bitmap(x, y, bitmap, None);
        }
        if mode == "P"
            && matches!(&input, DrawColorInput::Components(values) if values.len() > 1)
            && self
                .image
                .palette()
                .is_none_or(|palette| palette.is_empty())
        {
            // Without a palette, Pillow's P-mode bitmap ink resolver does
            // not turn an RGB(A) tuple into an index. It returns no ink and
            // leaves the destination unchanged; using the first component
            // here would incorrectly paint that byte as a palette index.
            return Ok(());
        }
        if let DrawColorInput::Components(values) = &input {
            if mode.len() == 1 && mode != "P" && values.len() != 1 {
                if mode == "F" {
                    return Err(PilError::TypeError(
                        "must be real number, not tuple".to_owned(),
                    ));
                }
                return Err(PilError::TypeError(
                    "color must be int or single-element tuple".to_owned(),
                ));
            }
            if mode.len() == 2 && !matches!(values.len(), 1 | 2) {
                return Err(PilError::TypeError(
                    "color must be int, or tuple of one or two elements".to_owned(),
                ));
            }
        }
        let color = self.color_with_input(input)?;
        self.bitmap(x, y, bitmap, Some(color))
    }

    /// Returns the original Pillow mode of the drawing target.
    pub fn mode(&self) -> Option<&str> {
        self.orig_mode.as_deref()
    }

    fn shape_inks(
        &self,
        fill: Option<(u8, u8, u8, u8)>,
        outline: Option<(u8, u8, u8, u8)>,
    ) -> ShapeInks {
        if fill.is_none() && outline.is_none() {
            if self.orig_mode.as_deref() == Some("PA") {
                return (None, Some((255, 255, 255, 255)));
            }
            // ImageDraw geometric primitives default to the context's outline
            // ink, not a filled interior.  Rectangle/ellipse/polygon and
            // their relatives therefore retain a one-pixel white outline on
            // RGB-family images when both color arguments are omitted.
            return (None, self.default_shape_ink());
        }
        (fill, outline)
    }

    /// Return the ink Pillow's experimental `Outline` API uses when both
    /// `fill` and `outline` are omitted.
    fn default_shape_ink(&self) -> Option<(u8, u8, u8, u8)> {
        match self.effective_mode().as_str() {
            // ImageDraw._getink() starts an RGB-family outline with
            // draw_ink(-1), which resolves to an all-255 sample.
            "RGB" | "RGBA" | "RGBX" | "LA" | "PA" | "CMYK" | "YCbCr" | "HSV" => {
                Some((255, 255, 255, 255))
            }
            // I/F contexts initialize their ink with draw_ink(1). These
            // tuples are the native little-endian representations carried by
            // the explicit-mode RGBA canvas used by this crate.
            "I" => Some((1, 0, 0, 0)),
            // I;16* uses Pillow's draw_ink(-1) default. The native canvas
            // consumes the low two packed bytes as the full 65535 sample.
            "I;16" | "I;16L" | "I;16B" | "I;16N" => Some((255, 255, 255, 255)),
            "F" => Some((0, 0, 128, 63)),
            // Pillow's draw_ink(-1) is not a valid ink for these single-byte
            // and palette-indexed modes, so shape() remains a no-op.
            "1" | "L" | "P" => None,
            _ => None,
        }
    }

    /// Set the output image from a drawn RGBA canvas.
    /// image_clone() handles RGBA→native mode conversion for standard modes.
    /// Only F/I/CMYK need explicit_mode tagging (their RGBA data IS the final format).
    fn set_image(&mut self, canvas: RgbaImage) {
        let explicit = match self.orig_mode.as_deref() {
            Some("F") | Some("I") | Some("CMYK") => self.orig_mode.clone(),
            _ => None,
        };
        self.image = Image::from_dynamic(
            DynamicImage::ImageRgba8(canvas),
            explicit.as_deref().and_then(crate::image::mode_boundary::parse_pillow_mode),
        );
    }

    /// Returns the current drawn image with original mode semantics restored.
    ///
    /// Standard modes are converted from the internal RGBA drawing canvas back
    /// to their original layout. All other public draw branches either write
    /// the destination's native layout directly or retain the original mode
    /// as an explicit tag on the RGBA backing buffer. The only public path
    /// that leaves a logical mode mismatch is an explicit RGBA context over
    /// an RGB image.
    pub fn image_clone(&self) -> Result<Image, PilError> {
        let img = self.image.clone();
        let orig = self.orig_mode.as_deref().unwrap_or_default();
        let current = img.mode().unwrap_or_default();
        if orig != "RGB" || current == orig {
            return Ok(img);
        }

        // The RGBA draw context over RGB is the one public path whose backing
        // storage intentionally differs from the logical destination mode.
        let img_loaded = img.materialize()?;
        Ok(Image::from_dynamic(
            DynamicImage::ImageRgb8(img_loaded.to_rgb8()),
            None,
        ))
    }

    /// Consume the drawing context and return the modified image.
    pub fn into_image(self) -> Image {
        self.image
    }
}

mod bitmap;
mod curves;
mod geometry;
mod primitives;
mod text;

pub use geometry::outline_curve_points;
pub(crate) use geometry::{
    DrawCanvas, blend_u8, bresenham_line, for_each_bresenham_point, for_each_polygon_fill_span,
    pil_blend, plot, scanline_polygon_fill, wide_line_polygon_points,
};

#[cfg(test)]
mod tests {
    use super::{Draw, for_each_polygon_fill_span, wide_line_polygon_points};
    use crate::image::{FormattedPixelValue, Image};

    #[test]
    fn rounded_rectangle_rejects_reversed_axes_in_pillow_order() {
        let image = Image::new(8, 8, "L", (0, 0, 0, 255)).expect("test image");
        let mut draw = Draw::new(image, None);

        let error = draw
            .rounded_rectangle(1, 5, 5, 1, 0.0, None, None, 1)
            .expect_err("reversed y axis must fail");
        assert_eq!(error.to_string(), "y1 must be greater than or equal to y0");

        let error = draw
            .rounded_rectangle(5, 5, 1, 1, 0.0, None, None, 1)
            .expect_err("reversed x axis must fail");
        assert_eq!(error.to_string(), "x1 must be greater than or equal to x0");

        draw.rounded_rectangle(1, 1, 5, 5, 0.0, None, None, 1)
            .expect("a non-reversed box remains valid");
    }

    #[test]
    fn i16_geometry_uses_native_samples_and_declared_byte_order() {
        for mode in ["I;16", "I;16L", "I;16B", "I;16N"] {
            let image = Image::new(4, 4, mode, (0, 0, 0, 0)).expect("test image");
            let mut draw = Draw::new(image, None);
            draw.shape(&[(0, 0), (2, 0), (0, 2)], None, None)
                .expect("default shape ink");
            let image = draw.into_image();
            assert_eq!(image.mode().expect("mode"), mode);
            assert_eq!(
                image.getpixel_formatted(0, 0).expect("shape pixel"),
                FormattedPixelValue::Integer(65535)
            );

            let image = Image::new(1, 1, mode, (0, 0, 0, 0)).expect("test image");
            let mut draw = Draw::new(image, None);
            draw.point(&[(0, 0)], (1, 0, 0, 255))
                .expect("packed point ink");
            let image = draw.into_image();
            let expected = if mode == "I;16B" { 256 } else { 1 };
            assert_eq!(
                image.getpixel_formatted(0, 0).expect("point pixel"),
                FormattedPixelValue::Integer(expected)
            );
        }
    }

    #[test]
    fn wide_line_bottom_edge_keeps_pillow_corner_intersection() {
        let points = wide_line_polygon_points(0, 5, 7, 0, 2).expect("non-zero line");
        let mut spans = Vec::new();
        for_each_polygon_fill_span(&points, 8, 6, |x_start, x_end, y| {
            spans.push((x_start, x_end, y));
        });

        assert!(spans.contains(&(0, 1, 5)));
    }
}
