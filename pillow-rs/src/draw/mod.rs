//! Pillow-compatible `ImageDraw` primitives.
//!
//! [`Draw`] records drawing operations against an [`Image`] and keeps enough
//! mode metadata to convert the drawn result back to the original Pillow mode.
//! Coordinates are integer pixel coordinates. Colors are normalized RGBA byte
//! tuples before mode-specific drawing rules are applied.

use crate::raster::{DynamicImage, Rgba, RgbaImage};

use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::PipelineOp;

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
    width.filter(|value| *value > 0).unwrap_or(1)
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

fn draw_float_color_error(mode: &str) -> PilError {
    if matches!(mode, "1" | "L" | "P" | "I") {
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
            DrawColorInput::None => {
                if mode == "PA" {
                    Ok((255, 255, 255, 255))
                } else {
                    Ok((0, 0, 0, 255))
                }
            }
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
    ) -> (Option<(u8, u8, u8, u8)>, Option<(u8, u8, u8, u8)>) {
        if fill.is_none() && outline.is_none() && self.orig_mode.as_deref() == Some("PA") {
            (None, Some((255, 255, 255, 255)))
        } else {
            (fill, outline)
        }
    }

    /// Return the ink Pillow's experimental `Outline` API uses when both
    /// `fill` and `outline` are omitted.
    fn default_shape_ink(&self) -> Option<(u8, u8, u8, u8)> {
        match self.effective_mode().as_str() {
            // ImageDraw._getink() starts an RGB-family outline with
            // draw_ink(-1), which resolves to an all-255 sample.
            "RGB" | "RGBA" | "LA" | "PA" | "CMYK" | "YCbCr" | "HSV" => Some((255, 255, 255, 255)),
            // I/F contexts initialize their ink with draw_ink(1). These
            // tuples are the native little-endian representations carried by
            // the explicit-mode RGBA canvas used by this crate.
            "I" => Some((1, 0, 0, 0)),
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
        self.image = Image::from_dynamic(DynamicImage::ImageRgba8(canvas), explicit);
    }

    /// Draws a line from `(x0, y0)` to `(x1, y1)`.
    ///
    /// `fill` is an RGBA byte tuple and `width` is the stroke width in pixels.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(())`; deferred pipeline execution reports later
    /// materialization failures.
    pub fn line(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        fill: (u8, u8, u8, u8),
        width: u32,
    ) -> Result<(), PilError> {
        self.image = Image::push_op(
            &self.image,
            PipelineOp::DrawLine {
                x0,
                y0,
                x1,
                y1,
                fill,
                width,
                alpha_blend_rgb: self.alpha_blend_rgb(),
            },
        );
        Ok(())
    }

    /// Draws consecutive line segments through `points`.
    ///
    /// # Errors
    ///
    /// Returns [`PilError::ValueError`] when fewer than two points are given.
    /// Deferred pipeline execution reports materialization failures later.
    pub fn polyline(
        &mut self,
        points: &[(i32, i32)],
        fill: (u8, u8, u8, u8),
        width: u32,
    ) -> Result<(), PilError> {
        if points.len() < 2 {
            return Err(PilError::ValueError(
                "wrong number of coordinates".to_owned(),
            ));
        }
        for segment in points.windows(2) {
            self.line(
                segment[0].0,
                segment[0].1,
                segment[1].0,
                segment[1].1,
                fill,
                width,
            )?;
        }
        Ok(())
    }

    /// Normalizes and draws a Python-facing line coordinate sequence.
    pub fn polyline_with_input(
        &mut self,
        input: DrawPointsInput,
        fill: (u8, u8, u8, u8),
        width: u32,
    ) -> Result<(), PilError> {
        let points = normalize_draw_points(input, true)?;
        if points.len() < 2 {
            return Ok(());
        }
        self.polyline(&points, fill, width)
    }

    /// Draws a rectangle bounded by `(x0, y0, x1, y1)`.
    ///
    /// `fill` paints the interior when present. `outline` paints the border
    /// when present. `width` controls outline thickness in pixels.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(())`; deferred pipeline execution reports later
    /// materialization failures.
    pub fn rectangle(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        fill: Option<(u8, u8, u8, u8)>,
        outline: Option<(u8, u8, u8, u8)>,
        width: u32,
    ) -> Result<(), PilError> {
        let (fill, outline) = self.shape_inks(fill, outline);
        self.image = Image::push_op(
            &self.image,
            PipelineOp::DrawRectangle {
                x0,
                y0,
                x1,
                y1,
                fill,
                outline,
                width,
                alpha_blend_rgb: self.alpha_blend_rgb(),
            },
        );
        Ok(())
    }

    /// Draws an ellipse inside `(x0, y0, x1, y1)`.
    ///
    /// Fill, outline, and width follow Pillow `ImageDraw.ellipse` semantics.
    /// The backend uses Pillow's Bresenham-style quarter-ellipse algorithm.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(())`; deferred pipeline execution reports later
    /// materialization failures.
    pub fn ellipse(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        fill: Option<(u8, u8, u8, u8)>,
        outline: Option<(u8, u8, u8, u8)>,
        _width: u32,
    ) -> Result<(), PilError> {
        // Pillow's _imaging ellipse dispatch rejects a reversed bounding box
        // before reaching ImagingDrawEllipse; keep that public validation in
        // core so every binding reports the same error.
        if x1 < x0 {
            return Err(PilError::ValueError(
                "x1 must be greater than or equal to x0".into(),
            ));
        }
        if y1 < y0 {
            return Err(PilError::ValueError(
                "y1 must be greater than or equal to y0".into(),
            ));
        }
        let (fill, outline) = self.shape_inks(fill, outline);
        self.image = Image::push_op(
            &self.image,
            PipelineOp::DrawEllipse {
                x0,
                y0,
                x1,
                y1,
                fill,
                outline,
                width: _width,
                alpha_blend_rgb: self.alpha_blend_rgb(),
            },
        );
        Ok(())
    }

    /// Draws a polygon from integer vertices.
    ///
    /// Fewer than three points is a no-op. `fill` paints the interior and
    /// `outline` paints the boundary when present.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(())`; deferred pipeline execution reports later
    /// materialization failures.
    pub fn polygon(
        &mut self,
        points: &[(i32, i32)],
        fill: Option<(u8, u8, u8, u8)>,
        outline: Option<(u8, u8, u8, u8)>,
        _width: u32,
    ) -> Result<(), PilError> {
        let (fill, outline) = self.shape_inks(fill, outline);
        if points.len() < 2 {
            return Ok(());
        }
        self.image = Image::push_op(
            &self.image,
            PipelineOp::DrawPolygon {
                points: points.to_vec(),
                fill,
                outline,
                width: _width,
                alpha_blend_rgb: self.alpha_blend_rgb(),
            },
        );
        Ok(())
    }

    /// Normalizes and draws a Python-facing polygon coordinate sequence.
    pub fn polygon_with_input(
        &mut self,
        input: DrawPointsInput,
        fill: Option<(u8, u8, u8, u8)>,
        outline: Option<(u8, u8, u8, u8)>,
        width: u32,
    ) -> Result<(), PilError> {
        let points = normalize_draw_points(input, false)?;
        self.polygon(&points, fill, outline, width)
    }

    /// Normalize and draw a public `ImageDraw.shape` point sequence.
    pub fn shape_with_input(
        &mut self,
        input: DrawPointsInput,
        fill: Option<(u8, u8, u8, u8)>,
        outline: Option<(u8, u8, u8, u8)>,
    ) -> Result<(), PilError> {
        let points = normalize_draw_point_input(input)?;
        self.shape(&points, fill, outline)
    }

    /// Draws a regular polygon from Pillow's bounding-circle representation.
    ///
    /// Vertex generation, Pillow's two-decimal rounding, and side-count
    /// validation live in core so every binding uses the same geometry.
    pub fn regular_polygon(
        &mut self,
        bounding_circle: RegularPolygonCircle,
        n_sides: RegularPolygonSides,
        rotation: f64,
        fill: Option<(u8, u8, u8, u8)>,
        outline: Option<(u8, u8, u8, u8)>,
        width: u32,
    ) -> Result<(), PilError> {
        let n_sides = match n_sides {
            RegularPolygonSides::Value(value) if value > 2 => usize::try_from(value)
                .map_err(|_| PilError::ValueError("n_sides should be an int > 2".into()))?,
            RegularPolygonSides::Value(_) | RegularPolygonSides::Invalid => {
                return Err(PilError::ValueError("n_sides should be an int > 2".into()));
            }
        };
        let (cx, cy, radius) = match bounding_circle {
            RegularPolygonCircle::Flat(x, y, radius)
            | RegularPolygonCircle::Nested(x, y, radius) => (x, y, radius),
            RegularPolygonCircle::Invalid => {
                return Err(PilError::ValueError(
                    "bounding_circle should contain 2D coordinates and a radius (e.g. (x, y, r) or ((x, y), r) )".into(),
                ));
            }
        };

        // Match PIL's _compute_regular_polygon_vertices exactly: start from
        // (radius, 0), rotate by (270 - 0.5*degrees-per-side + rotation),
        // round each coordinate to two decimals, then truncate to integers.
        let n = n_sides as f64;
        let degrees_per_side = 360.0 / n;
        let start_angle = 270.0 - 0.5 * degrees_per_side + rotation;
        let mut points = Vec::with_capacity(n_sides);
        for index in 0..n_sides {
            let angle = start_angle + degrees_per_side * index as f64;
            let angle = if angle > 360.0 { angle - 360.0 } else { angle };
            let theta = (360.0 - angle).to_radians();
            let x = ((radius * theta.cos() + cx) * 100.0).round() / 100.0;
            let y = ((radius * theta.sin() + cy) * 100.0).round() / 100.0;
            points.push((x as i32, y as i32));
        }
        self.polygon(&points, fill, outline, width)
    }

    /// Fills a closed outline using Pillow's `ImageDraw.shape` ink order.
    ///
    /// Pillow draws `fill` first and `outline` last, but its outline primitive
    /// fills the complete path. Therefore `outline`, when present, is the
    /// effective color for the whole shape.
    pub fn shape(
        &mut self,
        points: &[(i32, i32)],
        fill: Option<(u8, u8, u8, u8)>,
        outline: Option<(u8, u8, u8, u8)>,
    ) -> Result<(), PilError> {
        // Pillow's ImageDraw._getink() supplies the context's default ink
        // when both arguments are omitted. This is deliberately shape-only:
        // the experimental API calls draw_outline directly, and its default
        // differs from the wrapper's ordinary color parser.
        let Some(ink) = outline.or(fill).or_else(|| self.default_shape_ink()) else {
            return Ok(());
        };
        self.polygon(points, Some(ink), None, 1)
    }

    /// Draws one or more individual points.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(())`; deferred pipeline execution reports later
    /// materialization failures.
    pub fn point(&mut self, points: &[(i32, i32)], fill: (u8, u8, u8, u8)) -> Result<(), PilError> {
        self.image = Image::push_op(
            &self.image,
            PipelineOp::DrawPoint {
                points: points.to_vec(),
                fill,
                alpha_blend_rgb: self.alpha_blend_rgb(),
            },
        );
        Ok(())
    }

    /// Normalizes and draws a Python-facing point coordinate sequence.
    pub fn point_with_input(
        &mut self,
        input: DrawPointsInput,
        fill: (u8, u8, u8, u8),
    ) -> Result<(), PilError> {
        let points = normalize_draw_point_input(input)?;
        self.point(&points, fill)
    }

    /// Normalizes and draws a Python-facing rounded-rectangle box.
    pub fn rounded_rectangle_with_input(
        &mut self,
        input: DrawBoxInput,
        radius: f64,
        fill: Option<(u8, u8, u8, u8)>,
        outline: Option<(u8, u8, u8, u8)>,
        width: u32,
    ) -> Result<(), PilError> {
        let (x0, y0, x1, y1) = normalize_rounded_rectangle_box(input)?;
        self.rounded_rectangle(x0, y0, x1, y1, radius, fill, outline, width)
    }

    /// Draws a bitmap mask at `(x, y)` using `fill`.
    ///
    /// The bitmap acts as a transparency mask. Valid bitmap modes:
    /// - "1": binary mask (non-zero → fill)
    /// - "L": alpha mask (0-255 opacity)
    /// - "RGBA"/"RGBa": alpha channel at byte offset +3
    ///
    /// # Errors
    ///
    /// Returns [`PilError::ValueError`] when the bitmap mode is not a valid mask
    /// mode. Returns other [`PilError`] values when mode, size, or data lookup
    /// fails.
    pub fn bitmap(
        &mut self,
        x: i32,
        y: i32,
        bitmap: &Image,
        fill: Option<(u8, u8, u8, u8)>,
    ) -> Result<(), PilError> {
        let color = fill.unwrap_or((255, 255, 255, 255));
        let bmp_mode = bitmap.mode()?;
        // Validate mask mode — PIL only accepts "1", "L", "RGBA", "RGBa"
        let is_valid_mask = matches!(bmp_mode.as_str(), "1" | "L" | "RGBA" | "RGBa");
        if !is_valid_mask {
            return Err(PilError::ValueError("bad transparency mask".to_string()));
        }
        let (bmp_w, bmp_h) = bitmap.size()?;
        let raw_data = bitmap.getdata(None)?;
        let bmp_stride: usize = if matches!(bmp_mode.as_str(), "1" | "L") {
            1
        } else {
            4
        };

        // `Image::getdata` returns the complete canonical raster for a
        // materialized image, so the dimensions above make these indexes
        // exact. Keep this hot path branch-free for valid mask modes; the
        // public mode check above rejects every other mode before iteration.
        let mask_val = |px: u32, py: u32, data: &[u8]| -> u8 {
            let idx = (py * bmp_w + px) as usize;
            match bmp_mode.as_str() {
                "1" => {
                    if data[idx] > 0 {
                        255
                    } else {
                        0
                    }
                }
                "L" => data[idx],
                "RGBA" | "RGBa" => {
                    let pixel_idx = idx * bmp_stride;
                    data[pixel_idx + 3]
                }
                _ => unreachable!("bitmap mode was validated before mask iteration"),
            }
        };

        // PIL's BLEND: DIV255(a * (255 - mask) + b * mask)
        let pil_blend = |bg: u8, fg: u8, m: u8| -> u8 {
            if m == 255 {
                return fg;
            }
            ((bg as u16 * (255u16 - m as u16) + fg as u16 * m as u16 + 127u16) / 255u16) as u8
        };

        let mode = self.effective_mode();

        match mode.as_str() {
            "RGB" | "RGBA" => {
                let img = self.image.materialize()?;
                let (img_w, img_h) = (img.width(), img.height());
                let mut canvas = img.to_rgba8();

                for py in 0..bmp_h {
                    for px in 0..bmp_w {
                        let m = mask_val(px, py, &raw_data);
                        if m == 0 {
                            continue;
                        }
                        let dx = x + px as i32;
                        let dy = y + py as i32;
                        if dx >= 0 && dy >= 0 && (dx as u32) < img_w && (dy as u32) < img_h {
                            let existing = canvas.get_pixel(dx as u32, dy as u32);
                            let r = pil_blend(existing[0], color.0, m);
                            let g = pil_blend(existing[1], color.1, m);
                            let b = pil_blend(existing[2], color.2, m);
                            let a = pil_blend(existing[3], color.3, m);
                            canvas.put_pixel(
                                dx as u32,
                                dy as u32,
                                crate::raster::Rgba([r, g, b, a]),
                            );
                        }
                    }
                }

                self.set_image(canvas);
                Ok(())
            }
            "1" => {
                let img = self.image.materialize()?;
                let (img_w, img_h) = (img.width(), img.height());
                let mut luma = img.to_luma8();
                let ink = color.0;
                for py in 0..bmp_h {
                    for px in 0..bmp_w {
                        let m = mask_val(px, py, &raw_data);
                        if m == 0 {
                            continue;
                        }
                        let dx = x + px as i32;
                        let dy = y + py as i32;
                        if dx >= 0 && dy >= 0 && (dx as u32) < img_w && (dy as u32) < img_h {
                            let v = if m == 255 {
                                ink
                            } else {
                                pil_blend(luma.get_pixel(dx as u32, dy as u32)[0], ink, m)
                            };
                            luma.put_pixel(dx as u32, dy as u32, crate::raster::Luma([v]));
                        }
                    }
                }
                self.image = Image::from_dynamic(
                    crate::raster::DynamicImage::ImageLuma8(luma),
                    Some("1".to_string()),
                );
                Ok(())
            }
            "L" => {
                let img = self.image.materialize()?;
                let (img_w, img_h) = (img.width(), img.height());
                let mut luma = img.to_luma8();
                let ink = color.0;
                for py in 0..bmp_h {
                    for px in 0..bmp_w {
                        let m = mask_val(px, py, &raw_data);
                        if m == 0 {
                            continue;
                        }
                        let dx = x + px as i32;
                        let dy = y + py as i32;
                        if dx >= 0 && dy >= 0 && (dx as u32) < img_w && (dy as u32) < img_h {
                            let v = if m == 255 {
                                ink
                            } else {
                                pil_blend(luma.get_pixel(dx as u32, dy as u32)[0], ink, m)
                            };
                            luma.put_pixel(dx as u32, dy as u32, crate::raster::Luma([v]));
                        }
                    }
                }
                self.image =
                    Image::from_dynamic(crate::raster::DynamicImage::ImageLuma8(luma), None);
                Ok(())
            }
            "LA" => {
                let img = self.image.materialize()?;
                let (img_w, img_h) = (img.width(), img.height());
                let mut la = img.to_luma_alpha8();
                let ink_l = color.0;
                let ink_a = color.3;
                for py in 0..bmp_h {
                    for px in 0..bmp_w {
                        let m = mask_val(px, py, &raw_data);
                        if m == 0 {
                            continue;
                        }
                        let dx = x + px as i32;
                        let dy = y + py as i32;
                        if dx >= 0 && dy >= 0 && (dx as u32) < img_w && (dy as u32) < img_h {
                            let existing = la.get_pixel(dx as u32, dy as u32);
                            // Pillow's `fill_mask_L` adjusts the L-channel
                            // coverage against the destination alpha: with a
                            // fully transparent destination the L ink is
                            // written directly (`src/libImaging/Paste.c`).
                            let l_mask = if existing[1] == 0 { 255 } else { m };
                            let l = pil_blend(existing[0], ink_l, l_mask);
                            let a = pil_blend(existing[1], ink_a, m);
                            la.put_pixel(dx as u32, dy as u32, crate::raster::LumaA([l, a]));
                        }
                    }
                }
                self.image = Image::from_dynamic(
                    crate::raster::DynamicImage::ImageLumaA8(la),
                    Some("LA".to_string()),
                );
                Ok(())
            }
            "CMYK" => {
                let img = self.image.materialize()?;
                let (img_w, img_h) = (img.width(), img.height());
                let mut rgba = img.to_rgba8();
                let ink = [color.0, color.1, color.2, color.3];
                for py in 0..bmp_h {
                    for px in 0..bmp_w {
                        let m = mask_val(px, py, &raw_data);
                        if m == 0 {
                            continue;
                        }
                        let dx = x + px as i32;
                        let dy = y + py as i32;
                        if dx >= 0 && dy >= 0 && (dx as u32) < img_w && (dy as u32) < img_h {
                            let existing = rgba.get_pixel(dx as u32, dy as u32);
                            let c = if m == 255 {
                                ink[0]
                            } else {
                                pil_blend(existing[0], ink[0], m)
                            };
                            let m_ch = if m == 255 {
                                ink[1]
                            } else {
                                pil_blend(existing[1], ink[1], m)
                            };
                            let y_ch = if m == 255 {
                                ink[2]
                            } else {
                                pil_blend(existing[2], ink[2], m)
                            };
                            let k = if m == 255 {
                                ink[3]
                            } else {
                                pil_blend(existing[3], ink[3], m)
                            };
                            rgba.put_pixel(
                                dx as u32,
                                dy as u32,
                                crate::raster::Rgba([c, m_ch, y_ch, k]),
                            );
                        }
                    }
                }
                self.image = Image::from_dynamic(
                    crate::raster::DynamicImage::ImageRgba8(rgba),
                    Some("CMYK".to_string()),
                );
                Ok(())
            }
            "P" => {
                // Raw P images use an empty retained palette to mean that no
                // palette has been attached yet; only a non-empty palette
                // needs the indexed reconstruction path below.
                if let Some(palette) = self.image.palette().filter(|palette| !palette.is_empty()) {
                    // Pillow ImageDraw mutates the existing ImagingCore, so the
                    // encoded format and pending `info` metadata stay attached.
                    // Carry them across our immediate indexed-buffer rebuild.
                    let palette_alpha = self.image.palette_alpha().unwrap_or_default();
                    let source_format = self.image.source_format();
                    let info = self.image.image_info();
                    let img = self.image.materialize()?;
                    let luma = img.to_luma8();
                    let (img_w, img_h) = luma.dimensions();
                    let mut indices = crate::raster::GrayImage::new(img_w, img_h);
                    for (op, ip) in indices.pixels_mut().zip(luma.pixels()) {
                        op[0] = ip[0];
                    }
                    let ink = color.0;
                    for py in 0..bmp_h {
                        for px in 0..bmp_w {
                            let m = mask_val(px, py, &raw_data);
                            if m == 0 {
                                continue;
                            }
                            let dx = x + px as i32;
                            let dy = y + py as i32;
                            if dx >= 0 && dy >= 0 && (dx as u32) < img_w && (dy as u32) < img_h {
                                let v = if m == 255 {
                                    ink
                                } else {
                                    pil_blend(indices.get_pixel(dx as u32, dy as u32)[0], ink, m)
                                };
                                indices.put_pixel(dx as u32, dy as u32, crate::raster::Luma([v]));
                            }
                        }
                    }
                    self.image = Image::Paletted(crate::image::PalettedData {
                        indices,
                        palette,
                        palette_alpha,
                        source_format,
                        info,
                        exif: self.image.exif_metadata(),
                        materialized: crate::image::materialization_cache(),
                    });
                } else {
                    let img = self.image.materialize()?;
                    let (img_w, img_h) = (img.width(), img.height());
                    let mut luma = img.to_luma8();
                    let ink = color.0;
                    for py in 0..bmp_h {
                        for px in 0..bmp_w {
                            let m = mask_val(px, py, &raw_data);
                            if m == 0 {
                                continue;
                            }
                            let dx = x + px as i32;
                            let dy = y + py as i32;
                            if dx >= 0 && dy >= 0 && (dx as u32) < img_w && (dy as u32) < img_h {
                                let v = if m == 255 {
                                    ink
                                } else {
                                    pil_blend(luma.get_pixel(dx as u32, dy as u32)[0], ink, m)
                                };
                                luma.put_pixel(dx as u32, dy as u32, crate::raster::Luma([v]));
                            }
                        }
                    }
                    self.image = Image::from_dynamic(
                        crate::raster::DynamicImage::ImageLuma8(luma),
                        Some("P".to_string()),
                    );
                }
                Ok(())
            }
            "I" | "F" => {
                let img = self.image.materialize()?;
                let (img_w, img_h) = (img.width(), img.height());
                let mut rgba = img.to_rgba8();
                // Write all 4 bytes of the LE representation produced by the
                // mode-aware color normalization above.
                let ink = [color.0, color.1, color.2, color.3];
                for py in 0..bmp_h {
                    for px in 0..bmp_w {
                        let m = mask_val(px, py, &raw_data);
                        if m == 0 {
                            continue;
                        }
                        let dx = x + px as i32;
                        let dy = y + py as i32;
                        if dx >= 0 && dy >= 0 && (dx as u32) < img_w && (dy as u32) < img_h {
                            let existing = rgba.get_pixel(dx as u32, dy as u32);
                            let b0 = if m == 255 {
                                ink[0]
                            } else {
                                pil_blend(existing[0], ink[0], m)
                            };
                            let b1 = if m == 255 {
                                ink[1]
                            } else {
                                pil_blend(existing[1], ink[1], m)
                            };
                            let b2 = if m == 255 {
                                ink[2]
                            } else {
                                pil_blend(existing[2], ink[2], m)
                            };
                            let b3 = if m == 255 {
                                ink[3]
                            } else {
                                pil_blend(existing[3], ink[3], m)
                            };
                            rgba.put_pixel(
                                dx as u32,
                                dy as u32,
                                crate::raster::Rgba([b0, b1, b2, b3]),
                            );
                        }
                    }
                }
                self.image = Image::from_dynamic(
                    crate::raster::DynamicImage::ImageRgba8(rgba),
                    Some(mode.to_string()),
                );
                Ok(())
            }
            _ => {
                // Fallback: RGBA pipeline
                let img = self.image.materialize()?;
                let (img_w, img_h) = (img.width(), img.height());
                let mut canvas = img.to_rgba8();
                for py in 0..bmp_h {
                    for px in 0..bmp_w {
                        let m = mask_val(px, py, &raw_data);
                        if m == 0 {
                            continue;
                        }
                        let dx = x + px as i32;
                        let dy = y + py as i32;
                        if dx >= 0 && dy >= 0 && (dx as u32) < img_w && (dy as u32) < img_h {
                            let existing = canvas.get_pixel(dx as u32, dy as u32);
                            let r = if m == 255 {
                                color.0
                            } else {
                                pil_blend(existing[0], color.0, m)
                            };
                            let g = if m == 255 {
                                color.1
                            } else {
                                pil_blend(existing[1], color.1, m)
                            };
                            let b = if m == 255 {
                                color.2
                            } else {
                                pil_blend(existing[2], color.2, m)
                            };
                            let a = if m == 255 {
                                color.3
                            } else {
                                pil_blend(existing[3], color.3, m)
                            };
                            canvas.put_pixel(
                                dx as u32,
                                dy as u32,
                                crate::raster::Rgba([r, g, b, a]),
                            );
                        }
                    }
                }
                // YCbCr/HSV and other 3-band fallback modes store no alpha;
                // rebuild the canvas as RGB tagged with the source mode so
                // the resulting image keeps Pillow's 3-byte pixel layout.
                self.image = Image::from_dynamic(
                    crate::raster::DynamicImage::ImageRgb8(
                        crate::raster::DynamicImage::ImageRgba8(canvas).to_rgb8(),
                    ),
                    Some(mode.to_string()),
                );
                Ok(())
            }
        }
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

    /// Draws an arc along an ellipse boundary.
    ///
    /// Angles are in degrees, following Pillow's coordinate convention.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(())`; deferred pipeline execution reports later
    /// materialization failures.
    pub fn arc(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        start: f64,
        end: f64,
        fill: (u8, u8, u8, u8),
        _width: u32,
    ) -> Result<(), PilError> {
        self.image = Image::push_op(
            &self.image,
            PipelineOp::DrawArc {
                x0,
                y0,
                x1,
                y1,
                start,
                end,
                fill: Some(fill),
                width: _width,
                alpha_blend_rgb: self.alpha_blend_rgb(),
            },
        );
        Ok(())
    }

    /// Draws a chord inside an ellipse bounding box.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(())`; deferred pipeline execution reports later
    /// materialization failures.
    pub fn chord(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        start: f64,
        end: f64,
        fill: Option<(u8, u8, u8, u8)>,
        outline: Option<(u8, u8, u8, u8)>,
        _width: u32,
    ) -> Result<(), PilError> {
        let (fill, outline) = self.shape_inks(fill, outline);
        self.image = Image::push_op(
            &self.image,
            PipelineOp::DrawChord {
                x0,
                y0,
                x1,
                y1,
                start,
                end,
                fill,
                outline,
                width: _width,
                alpha_blend_rgb: self.alpha_blend_rgb(),
            },
        );
        Ok(())
    }

    /// Draws a pieslice inside an ellipse bounding box.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(())`; deferred pipeline execution reports later
    /// materialization failures.
    pub fn pieslice(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        start: f64,
        end: f64,
        fill: Option<(u8, u8, u8, u8)>,
        outline: Option<(u8, u8, u8, u8)>,
        _width: u32,
    ) -> Result<(), PilError> {
        let (fill, outline) = self.shape_inks(fill, outline);
        self.image = Image::push_op(
            &self.image,
            PipelineOp::DrawPieslice {
                x0,
                y0,
                x1,
                y1,
                start,
                end,
                fill,
                outline,
                width: _width,
                alpha_blend_rgb: self.alpha_blend_rgb(),
            },
        );
        Ok(())
    }

    /// Draws a circle centered at `(cx, cy)`.
    ///
    /// `radius` is rounded to an integer pixel radius for pipeline execution.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(())`; deferred pipeline execution reports later
    /// materialization failures.
    pub fn circle(
        &mut self,
        cx: i32,
        cy: i32,
        radius: f64,
        fill: Option<(u8, u8, u8, u8)>,
        outline: Option<(u8, u8, u8, u8)>,
        _width: u32,
    ) -> Result<(), PilError> {
        let (fill, outline) = self.shape_inks(fill, outline);
        self.image = Image::push_op(
            &self.image,
            PipelineOp::DrawCircle {
                cx,
                cy,
                radius: radius as i32,
                fill,
                outline,
                width: _width,
                alpha_blend_rgb: self.alpha_blend_rgb(),
            },
        );
        Ok(())
    }

    /// Normalizes and draws a Python-facing circle center.
    pub fn circle_with_input(
        &mut self,
        input: DrawCircleCenterInput,
        radius: f64,
        fill: Option<(u8, u8, u8, u8)>,
        outline: Option<(u8, u8, u8, u8)>,
        width: u32,
    ) -> Result<(), PilError> {
        let (cx, cy) = normalize_draw_circle_center(input)?;
        self.circle(cx as i32, cy as i32, radius, fill, outline, width)
    }

    /// Draws a rounded rectangle.
    ///
    /// `radius` is rounded to pixels. Non-positive radii or degenerate boxes
    /// fall back to a normal rectangle.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(())`; deferred pipeline execution reports later
    /// materialization failures.
    pub fn rounded_rectangle(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        radius: f64,
        fill: Option<(u8, u8, u8, u8)>,
        outline: Option<(u8, u8, u8, u8)>,
        _width: u32,
    ) -> Result<(), PilError> {
        let (fill, outline) = self.shape_inks(fill, outline);
        let r = radius.round() as i32;
        let d = r * 2;
        if d <= 0 || x1 <= x0 + 1 || y1 <= y0 + 1 {
            // No corner curve, just draw rectangle
            self.image = Image::push_op(
                &self.image,
                PipelineOp::DrawRectangle {
                    x0,
                    y0,
                    x1,
                    y1,
                    fill,
                    outline,
                    width: 1,
                    alpha_blend_rgb: self.alpha_blend_rgb(),
                },
            );
            return Ok(());
        }

        self.image = Image::push_op(
            &self.image,
            PipelineOp::DrawRoundedRect {
                x0,
                y0,
                x1,
                y1,
                radius,
                fill,
                outline,
                width: _width,
                alpha_blend_rgb: self.alpha_blend_rgb(),
            },
        );
        Ok(())
    }

    /// Draws text at `(x, y)` using a loaded font.
    ///
    /// For RGB and RGBA modes, uses the standard RGBA compositing pipeline.
    /// For other modes (1, L, LA, CMYK, P, I, F), renders directly in the
    /// mode's native pixel format, matching PIL's `draw_bitmap` behavior:
    /// - Integer fill values go to the first channel only; other channels get 0.
    /// - Binary modes (1, P, I, F) use PIL's fontmode="1": binary glyphs (coverage >= 128 → 255).
    /// - Anti-aliased modes (L, LA, CMYK) use PIL's BLEND (truncation) per channel.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when mode detection, font rendering, or destination
    /// materialization fails.
    pub fn text(
        &mut self,
        x: i32,
        y: i32,
        text: &str,
        font: &crate::font::FreeTypeFont,
        fill: (u8, u8, u8, u8),
    ) -> Result<(), PilError> {
        self.text_with_options_inner(
            x,
            y,
            text,
            font,
            fill,
            &crate::font::ImageFontTextOptions::default(),
        )
    }

    /// Draws text at `(x, y)` using Pillow-compatible text options.
    ///
    /// Libraqm-dependent options (`direction`, `features`, `language`) are
    /// validated by the `ImageFont` adapter and return
    /// [`PilError::UnsupportedLibraqm`] in no-libraqm builds.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when text option validation, font rendering, or
    /// destination materialization fails.
    pub fn text_with_options(
        &mut self,
        x: i32,
        y: i32,
        text: &str,
        font: &crate::font::FreeTypeFont,
        fill: (u8, u8, u8, u8),
        options: &crate::font::ImageFontTextOptions,
    ) -> Result<(), PilError> {
        self.text_with_options_inner(x, y, text, font, fill, options)
    }

    /// Draw text after applying Pillow's host-neutral text input rules.
    pub fn text_with_options_input(
        &mut self,
        x: i32,
        y: i32,
        text: crate::font::ImageFontTextInput,
        font: &crate::font::FreeTypeFont,
        fill: (u8, u8, u8, u8),
        options: &crate::font::ImageFontTextOptions,
    ) -> Result<(), PilError> {
        let text = text.into_text();
        self.text_with_options(x, y, &text, font, fill, options)
    }

    /// Draw text using Pillow's optional-font and `font_size` rules.
    pub fn text_with_optional_font(
        &mut self,
        x: i32,
        y: i32,
        text: &str,
        font: Option<&crate::font::FreeTypeFont>,
        fill: (u8, u8, u8, u8),
        size: Option<f32>,
        options: &crate::font::ImageFontTextOptions,
    ) -> Result<(), PilError> {
        crate::font::with_text_font(font, size, |font| {
            self.text_with_options(x, y, text, font, fill, options)
        })
    }

    /// Draw text with optional-font rules and host-neutral text input.
    pub fn text_with_optional_font_input(
        &mut self,
        x: i32,
        y: i32,
        text: crate::font::ImageFontTextInput,
        font: Option<&crate::font::FreeTypeFont>,
        fill: (u8, u8, u8, u8),
        size: Option<f32>,
        options: &crate::font::ImageFontTextOptions,
    ) -> Result<(), PilError> {
        crate::font::with_text_font(font, size, |font| {
            self.text_with_options_input(x, y, text, font, fill, options)
        })
    }

    /// Draws multiline text using the same line stepping as Pillow's public
    /// `ImageDraw.multiline_text` path.
    pub fn multiline_text_with_options(
        &mut self,
        x: f64,
        y: f64,
        text: &str,
        font: &crate::font::FreeTypeFont,
        fill: (u8, u8, u8, u8),
        spacing: f64,
        options: &crate::font::ImageFontTextOptions,
    ) -> Result<(), PilError> {
        let mut line_y = y;
        for line in text.split('\n') {
            if line.is_empty() {
                line_y += spacing + 10.0;
                continue;
            }
            self.text_with_options(x as i32, line_y as i32, line, font, fill, options)?;
            let (_, height) = font.text_bbox(line)?;
            line_y += height as f64 + spacing;
        }
        Ok(())
    }

    /// Draw multiline text using Pillow's optional-font and `font_size` rules.
    pub fn multiline_text_with_optional_font(
        &mut self,
        x: f64,
        y: f64,
        text: &str,
        font: Option<&crate::font::FreeTypeFont>,
        fill: (u8, u8, u8, u8),
        spacing: f64,
        size: Option<f32>,
        options: &crate::font::ImageFontTextOptions,
    ) -> Result<(), PilError> {
        crate::font::with_text_font(font, size, |font| {
            self.multiline_text_with_options(x, y, text, font, fill, spacing, options)
        })
    }

    /// Draw multiline text with optional-font rules and host-neutral text
    /// input. Byte text is interpreted as one Latin-1 code point per byte,
    /// preserving Pillow's public font contract before line splitting.
    pub fn multiline_text_with_optional_font_input(
        &mut self,
        x: f64,
        y: f64,
        text: crate::font::ImageFontTextInput,
        font: Option<&crate::font::FreeTypeFont>,
        fill: (u8, u8, u8, u8),
        spacing: f64,
        size: Option<f32>,
        options: &crate::font::ImageFontTextOptions,
    ) -> Result<(), PilError> {
        crate::font::with_text_font(font, size, |font| {
            let text = text.into_text();
            self.multiline_text_with_options(x, y, &text, font, fill, spacing, options)
        })
    }

    fn text_with_options_inner(
        &mut self,
        x: i32,
        y: i32,
        text: &str,
        font: &crate::font::FreeTypeFont,
        fill: (u8, u8, u8, u8),
        options: &crate::font::ImageFontTextOptions,
    ) -> Result<(), PilError> {
        let mode = self.effective_mode();
        self.validate_text_options(options)?;
        let binary = matches!(mode.as_str(), "1" | "P" | "I" | "F");
        let mut options = options.clone();
        if binary && options.mode.is_none() {
            options.mode = Some("1".to_string());
        }
        if options.stroke_width != 0.0 {
            // Pillow's ImageDraw.text path requests the outside-border
            // variant of the FreeType stroke helper. Keep this policy in the
            // Rust draw core so Python/JS bindings only forward public text
            // arguments and cannot silently select the getmask2 default.
            options.stroke_filled = true;
        }
        let color_mask = options.uses_color_mask();
        if color_mask && options.ink.is_none() {
            // Pillow's RGBA font mask stores the draw fill in the mask's RGB
            // channels and reserves alpha for glyph coverage. Keep this
            // packing in the Rust core so Python only forwards the fill.
            options.ink = Some(Self::pack_text_ink(fill));
        }

        // ImageFont rendering always uses alpha=255 so glyph coverage is preserved.
        // Mode-specific alpha handling (e.g., LA alpha=0 for int fills) is done
        // in text_compose_direct / text_compose_rgba.
        let render_fill = (fill.0, fill.1, fill.2, 255u8);
        let (w, h, mask, offset) = font.getmask2_with_options(text, &options)?;
        let pixels = if color_mask {
            mask
        } else {
            text_mask_to_rgba(mask, render_fill)
        };
        if w == 0 || h == 0 {
            return Ok(());
        }
        let draw_x = x.saturating_add(offset.0);
        let draw_y = y.saturating_add(offset.1);

        match mode.as_str() {
            "RGB" | "RGBA" => self.text_compose_rgba(
                draw_x,
                draw_y,
                w,
                h,
                &pixels,
                fill,
                self.alpha_blend_rgb(),
                color_mask,
            ),
            _ => self.text_compose_direct(draw_x, draw_y, w, h, &pixels, &mode, fill),
        }
    }

    fn pack_text_ink(fill: (u8, u8, u8, u8)) -> i64 {
        i64::from(fill.0) | (i64::from(fill.1) << 8) | (i64::from(fill.2) << 16)
    }

    /// RGBA compositing for text (used for RGB and RGBA modes).
    ///
    /// Pixels from the font renderer have the glyph coverage in the alpha channel
    /// and the fill color in the RGB channels. Pillow's mask paste path blends
    /// each stored channel by glyph coverage. RGB output keeps alpha fixed at
    /// 255; RGBA output blends the alpha channel too.
    fn text_compose_rgba(
        &mut self,
        x: i32,
        y: i32,
        w: u32,
        h: u32,
        pixels: &[u8],
        fill: (u8, u8, u8, u8),
        // Whether the requested RGBA context is compositing onto an RGB
        // destination rather than preserving an RGBA destination.
        rgba_blend_rgb: bool,
        color_mask: bool,
    ) -> Result<(), PilError> {
        let img = self.image.materialize()?;
        let mut canvas = img.to_rgba8();
        let (img_w, img_h) = (canvas.width(), canvas.height());
        if img_w == 0 || img_h == 0 {
            return Ok(());
        }
        let mode = self.effective_mode();
        // Every font mask constructor returns exactly `w * h * 4` bytes for
        // this drawing path (grayscale masks are expanded before composing,
        // and color masks are produced as RGBA). Index the validated raster
        // directly in the hot loop instead of carrying a defensive length
        // branch that cannot occur for a core-owned mask.
        for py in 0..h {
            for px in 0..w {
                let off = ((py * w + px) * 4) as usize;
                let sa = pixels[off + 3];
                if sa == 0 {
                    continue;
                }
                // Pillow's ImageDraw text compositor clips glyph pixels
                // outside the destination. Keep the coordinate signed until
                // bounds are checked; casting a negative anchor or stroke
                // offset to u32 previously wrapped and panicked.
                let dx_signed = i64::from(x) + i64::from(px);
                let dy_signed = i64::from(y) + i64::from(py);
                if dx_signed < 0
                    || dy_signed < 0
                    || dx_signed >= i64::from(img_w)
                    || dy_signed >= i64::from(img_h)
                {
                    continue;
                }
                let dx = dx_signed as u32;
                let dy = dy_signed as u32;
                let dp = canvas.get_pixel(dx, dy);
                // Pillow's `src/libImaging/Paste.c` RGB mask path uses
                // glyph coverage as the RGB blend mask. The alpha byte
                // of an RGBA ink is intentionally ignored on an RGB
                // destination; RGBA destinations blend it separately.
                let channel_alpha = sa;
                let inv = 255u16 - u16::from(channel_alpha);
                let alpha = if mode == "RGBA" && !rgba_blend_rgb {
                    blend_u8(fill.3, dp[3], sa, inv)
                } else {
                    255
                };
                let source_rgb = if color_mask {
                    (pixels[off], pixels[off + 1], pixels[off + 2])
                } else {
                    (fill.0, fill.1, fill.2)
                };
                let (r, g, b) = if mode == "RGBA" && !rgba_blend_rgb && !color_mask {
                    // Pillow's RGBA text writes the fill RGB directly and
                    // blends only the alpha channel with glyph coverage.
                    (fill.0, fill.1, fill.2)
                } else {
                    (
                        blend_u8(source_rgb.0, dp[0], channel_alpha, inv),
                        blend_u8(source_rgb.1, dp[1], channel_alpha, inv),
                        blend_u8(source_rgb.2, dp[2], channel_alpha, inv),
                    )
                };
                canvas.put_pixel(dx, dy, Rgba([r, g, b, alpha]));
            }
        }
        if matches!(mode.as_str(), "YCbCr" | "HSV") {
            // Three-band canvases store no alpha; rebuild as RGB tagged with
            // the source mode so the result keeps Pillow's 3-byte layout.
            self.image = Image::from_dynamic(
                crate::raster::DynamicImage::ImageRgb8(
                    crate::raster::DynamicImage::ImageRgba8(canvas).to_rgb8(),
                ),
                Some(mode),
            );
        } else {
            self.set_image(canvas);
        }
        Ok(())
    }

    /// Direct per-pixel text compositing for non-standard modes.
    ///
    /// Matches PIL's `fill_mask_1` (binary) and `fill_mask_L` (anti-aliased)
    /// behavior from Paste.c. Integer fill values go to the first channel;
    /// other channels are zeroed.
    fn text_compose_direct(
        &mut self,
        x: i32,
        y: i32,
        w: u32,
        h: u32,
        pixels: &[u8],
        mode: &str,
        fill: (u8, u8, u8, u8),
    ) -> Result<(), PilError> {
        let img = self.image.materialize()?;
        let (img_w, img_h) = (img.width(), img.height());

        // For integer fills (all channels equal and alpha=255), treat as
        // single-channel: fill.0 goes to first channel, others get 0.
        // For tuple fills, use channel values directly.
        let is_int_fill = fill.0 == fill.1 && fill.0 == fill.2 && fill.3 == 255;

        match mode {
            "1" => {
                // Binary: write 255 where coverage > 0. PIL thresholds non-zero to 255.
                let mut luma = img.to_luma8();
                let ink = if fill.0 > 0 { 255u8 } else { 0u8 };
                for py in 0..h {
                    for px in 0..w {
                        let off = ((py * w + px) * 4) as usize;
                        if pixels[off + 3] > 0 {
                            let dx = x as i64 + px as i64;
                            let dy = y as i64 + py as i64;
                            if dx < 0 || dy < 0 || dx >= i64::from(img_w) || dy >= i64::from(img_h)
                            {
                                continue;
                            }
                            let (dx, dy) = (dx as u32, dy as u32);
                            luma.put_pixel(dx, dy, crate::raster::Luma([ink]));
                        }
                    }
                }
                self.image = Image::from_dynamic(
                    crate::raster::DynamicImage::ImageLuma8(luma),
                    Some("1".to_string()),
                );
                Ok(())
            }
            "L" => {
                // Anti-aliased: blend fill.0 with background using coverage.
                // Uses PIL's signed truncation: bg + (fg - bg) * cov / 255
                let mut luma = img.to_luma8();
                let ink = fill.0;
                for py in 0..h {
                    for px in 0..w {
                        let off = ((py * w + px) * 4) as usize;
                        let cov = pixels[off + 3];
                        if cov == 0 {
                            continue;
                        }
                        let dx = x as i64 + px as i64;
                        let dy = y as i64 + py as i64;
                        if dx < 0 || dy < 0 || dx >= i64::from(img_w) || dy >= i64::from(img_h) {
                            continue;
                        }
                        let (dx, dy) = (dx as u32, dy as u32);
                        let bg = luma.get_pixel(dx, dy)[0];
                        let result = pil_blend(ink, bg, cov);
                        luma.put_pixel(dx, dy, crate::raster::Luma([result]));
                    }
                }
                self.image =
                    Image::from_dynamic(crate::raster::DynamicImage::ImageLuma8(luma), None);
                Ok(())
            }
            "LA" => {
                // Anti-aliased per channel: the A channel blends the fill
                // alpha by glyph coverage, and the L channel coverage is
                // adjusted against the destination alpha — with a fully
                // transparent destination the L ink is written directly
                // (`src/libImaging/Paste.c::fill_mask_L`).
                let mut la = img.to_luma_alpha8();
                let ink_l = fill.0;
                let ink_a = fill.3;
                for py in 0..h {
                    for px in 0..w {
                        let off = ((py * w + px) * 4) as usize;
                        let cov = pixels[off + 3];
                        if cov == 0 {
                            continue;
                        }
                        let dx = x as i64 + px as i64;
                        let dy = y as i64 + py as i64;
                        if dx < 0 || dy < 0 || dx >= i64::from(img_w) || dy >= i64::from(img_h) {
                            continue;
                        }
                        let (dx, dy) = (dx as u32, dy as u32);
                        let bg = la.get_pixel(dx, dy);
                        let l_cov = if bg[1] == 0 { 255 } else { cov };
                        let new_l = pil_blend(ink_l, bg[0], l_cov);
                        let new_a = pil_blend(ink_a, bg[1], cov);
                        la.put_pixel(dx, dy, crate::raster::LumaA([new_l, new_a]));
                    }
                }
                self.image = Image::from_dynamic(
                    crate::raster::DynamicImage::ImageLumaA8(la),
                    Some("LA".to_string()),
                );
                Ok(())
            }
            "CMYK" => {
                // Anti-aliased per channel:
                //   C channel = fill.0 or tuple C, M=tuple M, Y=tuple Y, K=tuple K.
                //   For integer fill: C=fill.0, M=Y=K=0.
                // Uses PIL's signed truncation: bg + (fg - bg) * cov / 255
                let mut rgba = img.to_rgba8(); // CMYK stored as Rgba8 internally
                let ink = if is_int_fill {
                    [fill.0, 0u8, 0u8, 0u8]
                } else {
                    [fill.0, fill.1, fill.2, fill.3]
                };
                for py in 0..h {
                    for px in 0..w {
                        let off = ((py * w + px) * 4) as usize;
                        let cov = pixels[off + 3];
                        if cov == 0 {
                            continue;
                        }
                        let dx = x as i64 + px as i64;
                        let dy = y as i64 + py as i64;
                        if dx < 0 || dy < 0 || dx >= i64::from(img_w) || dy >= i64::from(img_h) {
                            continue;
                        }
                        let (dx, dy) = (dx as u32, dy as u32);
                        let bg = rgba.get_pixel(dx, dy);
                        let new_pix = if cov == 255 {
                            Rgba(ink)
                        } else {
                            Rgba([
                                pil_blend(ink[0], bg[0], cov),
                                pil_blend(ink[1], bg[1], cov),
                                pil_blend(ink[2], bg[2], cov),
                                pil_blend(ink[3], bg[3], cov),
                            ])
                        };
                        rgba.put_pixel(dx, dy, new_pix);
                    }
                }
                self.image = Image::from_dynamic(
                    crate::raster::DynamicImage::ImageRgba8(rgba),
                    Some("CMYK".to_string()),
                );
                Ok(())
            }
            "P" => {
                // Binary: write palette index where coverage > 0. fontmode="1".
                if let Some(palette) = self.image.palette().filter(|palette| !palette.is_empty()) {
                    // Pillow's in-place text draw preserves format/info. The
                    // Rust path rebuilds PalettedData, so copy both explicitly.
                    let palette_alpha = self.image.palette_alpha().unwrap_or_default();
                    let source_format = self.image.source_format();
                    let info = self.image.image_info();
                    let img_loaded = img.to_luma8();
                    let (w_i, h_i) = img_loaded.dimensions();
                    let mut indices = crate::raster::GrayImage::new(w_i, h_i);
                    for (op, ip) in indices.pixels_mut().zip(img_loaded.pixels()) {
                        op[0] = ip[0];
                    }
                    let ink = fill.0; // palette index
                    for py in 0..h.min(h_i) {
                        for px in 0..w.min(w_i) {
                            let off = ((py * w + px) * 4) as usize;
                            if pixels[off + 3] > 0 {
                                let dx = x as i64 + px as i64;
                                let dy = y as i64 + py as i64;
                                if dx < 0 || dy < 0 || dx >= i64::from(w_i) || dy >= i64::from(h_i)
                                {
                                    continue;
                                }
                                let (dx, dy) = (dx as u32, dy as u32);
                                indices.put_pixel(dx, dy, crate::raster::Luma([ink]));
                            }
                        }
                    }
                    self.image = Image::Paletted(crate::image::PalettedData {
                        indices,
                        palette,
                        palette_alpha,
                        source_format,
                        info,
                        exif: self.image.exif_metadata(),
                        materialized: crate::image::materialization_cache(),
                    });
                } else {
                    // Fallback: just modify luma8
                    let mut luma = img.to_luma8();
                    let ink = fill.0;
                    for py in 0..h {
                        for px in 0..w {
                            let off = ((py * w + px) * 4) as usize;
                            if pixels[off + 3] > 0 {
                                let dx = x as i64 + px as i64;
                                let dy = y as i64 + py as i64;
                                if dx < 0
                                    || dy < 0
                                    || dx >= i64::from(img_w)
                                    || dy >= i64::from(img_h)
                                {
                                    continue;
                                }
                                let (dx, dy) = (dx as u32, dy as u32);
                                luma.put_pixel(dx, dy, crate::raster::Luma([ink]));
                            }
                        }
                    }
                    self.image = Image::from_dynamic(
                        crate::raster::DynamicImage::ImageLuma8(luma),
                        Some("P".to_string()),
                    );
                }
                Ok(())
            }
            "I" | "F" => {
                // Binary: write full 4-byte LE representation. fontmode="1".
                // Stored internally as Rgba8 with explicit mode.
                let mut rgba = img.to_rgba8();
                let ink = [fill.0, fill.1, fill.2, fill.3];
                for py in 0..h {
                    for px in 0..w {
                        let off = ((py * w + px) * 4) as usize;
                        if pixels[off + 3] > 0 {
                            let dx = x as i64 + px as i64;
                            let dy = y as i64 + py as i64;
                            if dx < 0 || dy < 0 || dx >= i64::from(img_w) || dy >= i64::from(img_h)
                            {
                                continue;
                            }
                            let (dx, dy) = (dx as u32, dy as u32);
                            rgba.put_pixel(dx, dy, Rgba(ink));
                        }
                    }
                }
                self.image = Image::from_dynamic(
                    crate::raster::DynamicImage::ImageRgba8(rgba),
                    Some(mode.to_string()),
                );
                Ok(())
            }
            _ => {
                // Fallback: RGBA pipeline
                self.text_compose_rgba(x, y, w, h, pixels, fill, false, false)
            }
        }
    }

    /// Consume the drawing context and return the modified image.
    pub fn into_image(self) -> Image {
        self.image
    }
}

// ── Drawing primitives ──────────────────────────────────────────────

/// Minimal native-pixel canvas used by the shared rasterizers.
///
/// Implementations retain their original storage layout; drawing code never
/// needs to convert an `L`, `LA`, `RGB`, or indexed buffer through `RGBA`.
pub(crate) trait DrawCanvas {
    /// Canvas width in pixels.
    fn width(&self) -> u32;
    /// Canvas height in pixels.
    fn height(&self) -> u32;
    /// Writes one normalized RGBA color using the canvas's native channels.
    fn put_rgba(&mut self, x: u32, y: u32, color: [u8; 4]);
}

impl DrawCanvas for RgbaImage {
    fn width(&self) -> u32 {
        self.width()
    }

    fn height(&self) -> u32 {
        self.height()
    }

    fn put_rgba(&mut self, x: u32, y: u32, color: [u8; 4]) {
        self.put_pixel(x, y, Rgba(color));
    }
}

/// Bresenham's line algorithm with clamping.
pub(crate) fn bresenham_line<C: DrawCanvas>(
    canvas: &mut C,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    color: (u8, u8, u8, u8),
    w: u32,
    h: u32,
    raw: bool,
) {
    // Match Pillow src/libImaging/Draw.c::{line8,line32,line32rgba}.
    // The C primitive omits its final endpoint because draw_lines adds it
    // once after the segment chain; this helper represents one complete
    // high-level segment, so it appends that endpoint below.
    let mut x = i64::from(x0);
    let mut y = i64::from(y0);
    let target_x = i64::from(x1);
    let target_y = i64::from(y1);
    let mut dx = target_x - x;
    let step_x = if dx < 0 {
        dx = -dx;
        -1
    } else {
        1
    };
    let mut dy = target_y - y;
    let step_y = if dy < 0 {
        dy = -dy;
        -1
    } else {
        1
    };

    if dx == 0 {
        for _ in 0..dy {
            plot(canvas, x as i32, y as i32, color, w, h, raw);
            y += step_y;
        }
    } else if dy == 0 {
        for _ in 0..dx {
            plot(canvas, x as i32, y as i32, color, w, h, raw);
            x += step_x;
        }
    } else if dx > dy {
        let steps = dx;
        dy += dy;
        let mut error = dy - dx;
        dx += dx;
        for _ in 0..steps {
            plot(canvas, x as i32, y as i32, color, w, h, raw);
            if error >= 0 {
                y += step_y;
                error -= dx;
            }
            error += dy;
            x += step_x;
        }
    } else {
        let steps = dy;
        dx += dx;
        let mut error = dx - dy;
        dy += dy;
        for _ in 0..steps {
            plot(canvas, x as i32, y as i32, color, w, h, raw);
            if error >= 0 {
                x += step_x;
                error -= dy;
            }
            error += dx;
            y += step_y;
        }
    }
    plot(canvas, x1, y1, color, w, h, raw);
}

/// Plot a single pixel with bounds checking.
///
/// When `raw` is true (F/I mode), writes the 4 bytes directly as-is without any
/// alpha blending — the 4-byte chunk represents a raw float32 or int32 LE value.
///
/// When `raw` is false, writes the normalized color directly. Pillow's default
/// drawing context uses the native-mode `point8`/`point32` primitives; alpha is
/// a stored channel, not an implicit blend factor. RGBA-on-RGB drawing uses a
/// separate explicit blend mode at the binding/API layer.
#[inline]
pub(crate) fn plot<C: DrawCanvas>(
    canvas: &mut C,
    x: i32,
    y: i32,
    color: (u8, u8, u8, u8),
    w: u32,
    h: u32,
    raw: bool,
) {
    if x < 0 || y < 0 || x as u32 >= w || y as u32 >= h {
        return;
    }
    let (x, y) = (x as u32, y as u32);
    let _ = raw;
    canvas.put_rgba(x, y, [color.0, color.1, color.2, color.3]);
}

#[inline]
pub(crate) fn blend_u8(src: u8, dst: u8, alpha: u8, inv_alpha: u16) -> u8 {
    let a = alpha as u16;
    (((src as u16 * a) + (dst as u16 * inv_alpha) + 127) / 255) as u8
}

/// PIL-style single-channel blend:
///   BLEND(mask, dst, src) = DIV255(dst * (255 - mask) + src * mask)
/// where DIV255(x) = (x + 127) / 255  (round-to-nearest via +127 before /255 truncation)
///
/// This exactly matches PIL's ImagingFill2 → fill_mask_L C implementation.
/// Using the simpler unsigned formula (fg*cov + bg*(255-cov))/255 truncates,
/// which differs by 1 from PIL's rounded result for some cov values.
#[inline]
pub(crate) fn pil_blend(fg: u8, bg: u8, cov: u8) -> u8 {
    let x = (bg as u32) * (255u32 - cov as u32) + (fg as u32) * (cov as u32);
    // DIV255 with rounding: (x + 127) / 255
    // Note: `(x + 127 + (x >> 8)) >> 8` is NOT used — it is an approximation
    // that differs from the exact /255 for some inputs (e.g., x=37104 gives 145 vs 146).
    ((x + 127) / 255) as u8
}

fn text_mask_to_rgba(mask: Vec<u8>, fill: (u8, u8, u8, u8)) -> Vec<u8> {
    let mut pixels = vec![0u8; mask.len() * 4];
    for (index, coverage) in mask.into_iter().enumerate() {
        if coverage == 0 {
            continue;
        }
        let offset = index * 4;
        pixels[offset] = fill.0;
        pixels[offset + 1] = fill.1;
        pixels[offset + 2] = fill.2;
        pixels[offset + 3] = coverage;
    }
    pixels
}

/// Compute cubic Bezier curve subdivision points.
/// Returns a flat list of (x, y) integer pairs for the curve from t=1..steps.
/// `control_points` must have at least 8 elements: [x0, y0, x1, y1, x2, y2, x3, y3].
/// Matches Pillow's `src/libImaging/Draw.c::ImagingOutlineCurve` algorithm.
pub fn outline_curve_points(control_points: &[f64], steps: u32) -> Vec<(i32, i32)> {
    if control_points.len() < 8 || steps == 0 {
        return vec![];
    }
    // Pillow receives these values as C `float`s, not doubles. Keep the
    // subdivision arithmetic in f32 so points near a half-pixel boundary
    // follow the oracle's rounding decisions.
    let x0 = control_points[0] as f32;
    let y0 = control_points[1] as f32;
    let x1 = control_points[2] as f32;
    let y1 = control_points[3] as f32;
    let x2 = control_points[4] as f32;
    let y2 = control_points[5] as f32;
    let x3 = control_points[6] as f32;
    let y3 = control_points[7] as f32;

    let mut points = Vec::with_capacity(steps as usize);
    for i in 1..=steps {
        let t = i as f32 / steps as f32;
        let t2 = t * t;
        let t3 = t2 * t;
        let u = 1.0_f32 - t;
        let u2 = u * u;
        let u3 = u2 * u;
        // C adds 0.5 and casts to int (truncating toward zero), rather than
        // using the host language's nearest-integer rounding rule.
        let x = x0 * u3 + 3.0 * (x1 * t * u2 + x2 * t2 * u) + x3 * t3 + 0.5;
        let y = y0 * u3 + 3.0 * (y1 * t * u2 + y2 * t2 * u) + y3 * t3 + 0.5;
        points.push((x.trunc() as i32, y.trunc() as i32));
    }
    points
}

/// PIL-style ROUND_UP: away-from-zero rounding at 0.5.
pub(crate) fn round_up(f: f64) -> i32 {
    if f >= 0.0 {
        (f + 0.5).floor() as i32
    } else {
        -((-f) + 0.5).floor() as i32
    }
}

/// PIL-style ROUND_DOWN: toward-zero rounding at 0.5.
pub(crate) fn round_down(f: f64) -> i32 {
    if f >= 0.0 {
        (f - 0.5).ceil() as i32
    } else {
        -((-f) - 0.5).ceil() as i32
    }
}

/// PIL-identical scanline polygon fill.
///
/// Uses PIL's edge-table / scanline algorithm from Draw.c:
/// 1. Build edges with inverse slope (dx = Δx/Δy)
/// 2. For each scanline, compute x-intersections from active edges
/// 3. Sort intersections and fill between pairs using ROUND_UP/ROUND_DOWN
/// 4. Horizontal edges drawn directly as filled lines
pub(crate) fn scanline_polygon_fill<C: DrawCanvas>(
    canvas: &mut C,
    points: &[(i32, i32)],
    color: (u8, u8, u8, u8),
    img_w: u32,
    img_h: u32,
    _raw: bool,
) {
    let n = points.len();
    if n < 2 {
        return;
    }

    // Edge descriptor matching PIL's Edge struct
    #[derive(Clone, Copy)]
    struct ScanEdge {
        x0: i32,
        y0: i32,
        xmin: i32,
        xmax: i32,
        ymin: i32,
        ymax: i32,
        dx: f32,
    }

    let make_edge = |x0: i32, y0: i32, x1: i32, y1: i32| ScanEdge {
        x0,
        y0,
        xmin: x0.min(x1),
        xmax: x0.max(x1),
        ymin: y0.min(y1),
        ymax: y0.max(y1),
        dx: if y0 == y1 {
            0.0
        } else {
            (x1 - x0) as f32 / (y1 - y0) as f32
        },
    };

    // Build Pillow's edge list, including its consecutive-horizontal-edge
    // coalescing. That detail affects vertex parity on scanlines that touch a
    // run of collinear polygon points.
    let mut edges: Vec<ScanEdge> = Vec::with_capacity(n);
    for i in 0..n.saturating_sub(1) {
        let (x0, y0) = points[i];
        let (x1, y1) = points[i + 1];
        if y0 == y1 && i != 0 && y0 == points[i - 1].1 {
            let previous_x = points[i - 1].0;
            if let Some(last) = edges.last_mut() {
                if x1 > x0 && x0 > previous_x {
                    last.xmax = x1;
                    continue;
                }
                if x1 < x0 && x0 < previous_x {
                    last.xmin = x1;
                    continue;
                }
            }
        }
        edges.push(make_edge(x0, y0, x1, y1));
    }
    if points[n - 1] != points[0] {
        let (x0, y0) = points[n - 1];
        let (x1, y1) = points[0];
        edges.push(make_edge(x0, y0, x1, y1));
    }

    if edges.is_empty() {
        return;
    }

    // Draw horizontal edges immediately (matching PIL's hline in non-alpha mode)
    let iw = img_w as i32;
    let ih = img_h as i32;
    let rgba = [color.0, color.1, color.2, color.3];
    for e in &edges {
        if e.ymin == e.ymax && e.ymin >= 0 && e.ymin < ih {
            let x_start = e.xmin.max(0);
            let x_end = e.xmax.min(iw - 1);
            for x in x_start..=x_end {
                canvas.put_rgba(x as u32, e.ymin as u32, rgba);
            }
        }
    }

    // Find global y bounds
    let mut global_ymin = i32::MAX;
    let mut global_ymax = i32::MIN;
    for e in &edges {
        global_ymin = global_ymin.min(e.ymin);
        global_ymax = global_ymax.max(e.ymax);
    }
    global_ymin = global_ymin.max(0);
    global_ymax = global_ymax.min(ih - 1);
    if global_ymin > global_ymax {
        return;
    }

    // Edge table: only non-horizontal edges (matching PIL's edge_table)
    let edge_table: Vec<&ScanEdge> = edges.iter().filter(|e| e.ymin != e.ymax).collect();
    if edge_table.is_empty() {
        return;
    }

    // Pre-allocate x-intersection array
    let mut xx: Vec<f32> = Vec::with_capacity(edge_table.len() * 2);

    // Scanline sweep
    for y in global_ymin..=global_ymax {
        xx.clear();
        let yf = y as f32;

        for (edge_index, edge) in edge_table.iter().enumerate() {
            if y >= edge.ymin && y <= edge.ymax {
                let mut x = (yf - edge.y0 as f32) * edge.dx + edge.x0 as f32;
                xx.push(x);

                // PIL duplicate at ymax (vertex parity)
                if y == edge.ymax && y < global_ymax {
                    xx.push(x);
                } else if (y == edge.ymin || y == edge.ymax) && edge.dx != 0.0 {
                    // Pillow connects discontiguous corners by looking one row
                    // into the two incident edges and nudging the shared
                    // intersection when both edges leave in the same direction.
                    for other in edge_table.iter().take(edge_index) {
                        if (y != other.ymin && y != other.ymax) || other.dx == 0.0 {
                            continue;
                        }
                        let other_x = (yf - other.y0 as f32) * other.dx + other.x0 as f32;
                        if x.round() != other_x.round() {
                            continue;
                        }
                        let offset = if y == edge.ymax { -1 } else { 1 };
                        let adjacent_x = ((y + offset - edge.y0) as f32) * edge.dx + edge.x0 as f32;
                        if y + offset < other.ymin || y + offset > other.ymax {
                            continue;
                        }
                        let adjacent_other_x =
                            ((y + offset - other.y0) as f32) * other.dx + other.x0 as f32;
                        if x > adjacent_x + 1.0 && x > adjacent_other_x + 1.0 {
                            x = adjacent_x.max(adjacent_other_x).round() + 1.0;
                        } else if x < adjacent_x - 1.0 && x < adjacent_other_x - 1.0 {
                            x = adjacent_x.min(adjacent_other_x).round() - 1.0;
                        }
                        if let Some(current) = xx.last_mut() {
                            *current = x;
                        }
                        break;
                    }
                }
            }
        }

        if xx.is_empty() {
            continue;
        }

        xx.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        // Fill pairs (0-1, 2-3, ...) matching PIL's pair fill
        let mut i = 1;
        while i < xx.len() {
            let x_start = round_up(f64::from(xx[i - 1]));
            let x_end = round_down(f64::from(xx[i]));
            if x_end < 0 || x_start >= iw {
                i += 2;
                continue;
            }
            let x_start = x_start.max(0);
            let x_end = x_end.min(iw - 1);
            if x_start <= x_end {
                for x in x_start..=x_end {
                    canvas.put_rgba(x as u32, y as u32, rgba);
                }
            }
            i += 2;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Draw, Image};

    #[test]
    fn rgb_text_uses_glyph_coverage_for_rgba_ink() {
        let image = Image::new(3, 3, "RGB", (0, 0, 0, 255)).expect("RGB image");
        let mut draw = Draw::new(image, Some("RGBA".to_owned()));
        let mask_pixel = [255, 0, 0, 128];

        draw.text_compose_rgba(1, 1, 1, 1, &mask_pixel, (255, 0, 0, 128), true, false)
            .expect("text composition");

        let image = draw.image_clone().expect("restored RGB image");
        let materialized = image.materialize().expect("materialized RGB image");
        let rgb = materialized.to_rgb8();
        let pixel = rgb.get_pixel(1, 1);
        assert_eq!(pixel[0], 128);
        assert_eq!(pixel[1], 0);
        assert_eq!(pixel[2], 0);
    }
}
