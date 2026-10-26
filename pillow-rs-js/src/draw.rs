use crate::inputs::*;
use crate::values::*;
use crate::{Image, ImageFont};
use wasm_bindgen::prelude::{JsValue, wasm_bindgen};

type DrawTextColorAndKind = ((u8, u8, u8, u8), bool);

// ── ImageDraw ────────────────────────────────────────────────────
use pillow_rs::Draw;

fn js_text_color_and_kind(draw: &Draw, value: &JsValue) -> Result<DrawTextColorAndKind, JsValue> {
    let input = js_draw_color_input(value);
    let is_integer = matches!(&input, pillow_rs::DrawColorInput::Integer(_));
    draw.text_color_with_input(input)
        .map(|color| (color, is_integer))
        .map_err(err)
}

#[wasm_bindgen]
/// Mutable drawing context for painting shapes and text into an image.
pub struct ImageDraw {
    draw: Draw,
}

#[wasm_bindgen]
impl ImageDraw {
    /// Creates a drawing context for `img`, optionally selecting a drawing mode.
    #[wasm_bindgen(constructor)]
    pub fn new(img: &Image, mode: Option<String>) -> Result<ImageDraw, JsValue> {
        let draw = Draw::new(img.inner.clone(), mode);
        draw.validate_mode().map_err(err)?;
        Ok(ImageDraw { draw })
    }

    #[wasm_bindgen(js_name = "line")]
    /// Draws a line segment between two points using the supplied RGBA color.
    #[allow(clippy::too_many_arguments)] // Preserve the existing JavaScript method signature.
    pub fn line(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        r: u8,
        g: u8,
        b: u8,
        a: u8,
        width: Option<u32>,
    ) -> Result<(), JsValue> {
        self.draw
            .line(x0, y0, x1, y1, (r, g, b, a), width.unwrap_or(1))
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "lineWithInput")]
    /// Draws connected points from a flat or nested JavaScript sequence.
    #[allow(clippy::too_many_arguments)] // Preserve the existing JavaScript method signature.
    pub fn line_with_input(
        &mut self,
        points: JsValue,
        r: u8,
        g: u8,
        b: u8,
        a: u8,
        width: Option<u32>,
        joint: Option<String>,
    ) -> Result<(), JsValue> {
        let input = if !points.is_array() {
            pillow_rs::DrawPointsInput::Invalid
        } else {
            let values = js_sys::Array::from(&points).iter().collect::<Vec<_>>();
            if let Some(flat) = values
                .iter()
                .map(|value| value.as_f64())
                .collect::<Option<Vec<_>>>()
            {
                pillow_rs::DrawPointsInput::Flat(
                    flat.into_iter().map(|value| value as i32).collect(),
                )
            } else {
                let nested = values
                    .iter()
                    .map(|value| {
                        if !value.is_array() {
                            return None;
                        }
                        js_sys::Array::from(value)
                            .iter()
                            .map(|item| item.as_f64().map(|number| number as i32))
                            .collect::<Option<Vec<_>>>()
                    })
                    .collect::<Option<Vec<_>>>();
                nested.map_or(
                    pillow_rs::DrawPointsInput::InvalidSequence,
                    pillow_rs::DrawPointsInput::Nested,
                )
            }
        };
        self.draw
            .polyline_with_input_joint(input, (r, g, b, a), width.unwrap_or(1), joint.as_deref())
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "lineWithColorInput")]
    /// Draws connected points using a mode-aware JavaScript color value.
    pub fn line_with_color_input(
        &mut self,
        points: JsValue,
        fill: JsValue,
        width: Option<u32>,
        joint: Option<String>,
    ) -> Result<(), JsValue> {
        let color = self
            .draw
            .color_with_input(js_draw_color_input(&fill))
            .map_err(err)?;
        self.draw
            .polyline_with_input_joint(
                js_draw_points_input(&points),
                color,
                width.unwrap_or(1),
                joint.as_deref(),
            )
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "rectangleWithInput")]
    /// Draws a rectangle from a JavaScript box with optional fill and outline colors.
    pub fn rectangle_with_input(
        &mut self,
        xy: JsValue,
        fill: JsValue,
        outline: JsValue,
        width: Option<u32>,
    ) -> Result<(), JsValue> {
        let (x0, y0, x1, y1) = js_draw_box_coordinates(&xy)?;
        let fill = js_draw_optional_color(&self.draw, &fill)?;
        let outline = js_draw_optional_color(&self.draw, &outline)?;
        self.draw
            .rectangle(x0, y0, x1, y1, fill, outline, width.unwrap_or(1))
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "ellipseWithInput")]
    /// Draws an ellipse inside a JavaScript box with optional fill and outline colors.
    pub fn ellipse_with_input(
        &mut self,
        xy: JsValue,
        fill: JsValue,
        outline: JsValue,
        width: Option<u32>,
    ) -> Result<(), JsValue> {
        let (x0, y0, x1, y1) = js_draw_box_coordinates(&xy)?;
        let fill = js_draw_optional_color(&self.draw, &fill)?;
        let outline = js_draw_optional_color(&self.draw, &outline)?;
        self.draw
            .ellipse(x0, y0, x1, y1, fill, outline, width.unwrap_or(1))
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "arcWithInput")]
    /// Draws an arc inside a JavaScript box from `start` to `end` degrees.
    pub fn arc_with_input(
        &mut self,
        xy: JsValue,
        start: f64,
        end: f64,
        fill: JsValue,
        width: Option<u32>,
    ) -> Result<(), JsValue> {
        let (x0, y0, x1, y1) = js_draw_box_coordinates(&xy)?;
        let fill = self
            .draw
            .color_with_input(js_draw_color_input(&fill))
            .map_err(err)?;
        self.draw
            .arc(x0, y0, x1, y1, start, end, fill, width.unwrap_or(1))
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "chordWithInput")]
    /// Draws a filled chord bounded by an ellipse and angular endpoints.
    pub fn chord_with_input(
        &mut self,
        xy: JsValue,
        start: f64,
        end: f64,
        fill: JsValue,
        outline: JsValue,
        width: Option<u32>,
    ) -> Result<(), JsValue> {
        let (x0, y0, x1, y1) = js_draw_box_coordinates(&xy)?;
        let fill = js_draw_optional_color(&self.draw, &fill)?;
        let outline = js_draw_optional_color(&self.draw, &outline)?;
        self.draw
            .chord(
                x0,
                y0,
                x1,
                y1,
                start,
                end,
                fill,
                outline,
                width.unwrap_or(1),
            )
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "piesliceWithInput")]
    /// Draws a pie slice bounded by an ellipse and angular endpoints.
    pub fn pieslice_with_input(
        &mut self,
        xy: JsValue,
        start: f64,
        end: f64,
        fill: JsValue,
        outline: JsValue,
        width: Option<u32>,
    ) -> Result<(), JsValue> {
        let (x0, y0, x1, y1) = js_draw_box_coordinates(&xy)?;
        let fill = js_draw_optional_color(&self.draw, &fill)?;
        let outline = js_draw_optional_color(&self.draw, &outline)?;
        self.draw
            .pieslice(
                x0,
                y0,
                x1,
                y1,
                start,
                end,
                fill,
                outline,
                width.unwrap_or(1),
            )
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "circleWithInput")]
    /// Draws a circle from a JavaScript center value and radius.
    pub fn circle_with_input(
        &mut self,
        xy: JsValue,
        radius: f64,
        fill: JsValue,
        outline: JsValue,
        width: Option<u32>,
    ) -> Result<(), JsValue> {
        let center = js_draw_circle_center_input(&xy);
        let fill = js_draw_optional_color(&self.draw, &fill)?;
        let outline = js_draw_optional_color(&self.draw, &outline)?;
        self.draw
            .circle_with_input(center, radius, fill, outline, width.unwrap_or(1))
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "roundedRectangleWithInput")]
    /// Draws a rounded rectangle from a JavaScript box and corner radius.
    pub fn rounded_rectangle_with_input(
        &mut self,
        xy: JsValue,
        radius: f64,
        fill: JsValue,
        outline: JsValue,
        width: Option<u32>,
    ) -> Result<(), JsValue> {
        let input = js_draw_box_input(&xy);
        let fill = js_draw_optional_color(&self.draw, &fill)?;
        let outline = js_draw_optional_color(&self.draw, &outline)?;
        self.draw
            .rounded_rectangle_with_input(input, radius, fill, outline, width.unwrap_or(1))
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "polygonWithInput")]
    /// Draws a polygon from JavaScript point input with optional fill and outline.
    pub fn polygon_with_input(
        &mut self,
        points: JsValue,
        fill: JsValue,
        outline: JsValue,
        width: Option<u32>,
    ) -> Result<(), JsValue> {
        let fill = js_draw_optional_color(&self.draw, &fill)?;
        let outline = js_draw_optional_color(&self.draw, &outline)?;
        self.draw
            .polygon_with_input(
                js_draw_points_input(&points),
                fill,
                outline,
                width.unwrap_or(1),
            )
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "pointWithInput")]
    /// Draws points from a JavaScript coordinate sequence using a mode-aware color.
    pub fn point_with_input(&mut self, points: JsValue, fill: JsValue) -> Result<(), JsValue> {
        let fill = self
            .draw
            .color_with_input(js_draw_color_input(&fill))
            .map_err(err)?;
        self.draw
            .point_with_input(js_draw_points_input(&points), fill)
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "shapeWithInput")]
    /// Draws a sequence of points as a shape with optional fill and outline colors.
    pub fn shape_with_input(
        &mut self,
        points: JsValue,
        fill: JsValue,
        outline: JsValue,
    ) -> Result<(), JsValue> {
        let fill = js_draw_optional_color(&self.draw, &fill)?;
        let outline = js_draw_optional_color(&self.draw, &outline)?;
        self.draw
            .shape_with_input(js_draw_points_input(&points), fill, outline)
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "regularPolygonWithInput")]
    /// Draws a regular polygon inside a bounding circle.
    pub fn regular_polygon_with_input(
        &mut self,
        bounding_circle: JsValue,
        n_sides: JsValue,
        rotation: f64,
        fill: JsValue,
        outline: JsValue,
        width: Option<u32>,
    ) -> Result<(), JsValue> {
        let circle = if let Some(values) = js_float_array(&bounding_circle) {
            if values.len() == 3 {
                pillow_rs::RegularPolygonCircle::Flat(values[0], values[1], values[2])
            } else {
                pillow_rs::RegularPolygonCircle::Invalid
            }
        } else if bounding_circle.is_array() {
            let values = js_sys::Array::from(&bounding_circle);
            if values.length() == 2 {
                let center = js_float_array(&values.get(0));
                let radius = values.get(1).as_f64();
                match (center, radius) {
                    (Some(center), Some(radius)) if center.len() == 2 => {
                        pillow_rs::RegularPolygonCircle::Nested(center[0], center[1], radius)
                    }
                    _ => pillow_rs::RegularPolygonCircle::Invalid,
                }
            } else {
                pillow_rs::RegularPolygonCircle::Invalid
            }
        } else {
            pillow_rs::RegularPolygonCircle::Invalid
        };
        let sides = js_integer(&n_sides).map_or(
            pillow_rs::RegularPolygonSides::Invalid,
            pillow_rs::RegularPolygonSides::Value,
        );
        let fill = js_draw_optional_color(&self.draw, &fill)?;
        let outline = js_draw_optional_color(&self.draw, &outline)?;
        self.draw
            .regular_polygon(circle, sides, rotation, fill, outline, width.unwrap_or(1))
            .map_err(err)
    }
}

mod bitmap;
mod shapes;
mod text;

// ── ImageFont ────────────────────────────────────────────────────

#[wasm_bindgen(js_name = "outlineCurve")]
/// Samples a curve from flattened coordinate pairs using the requested step count.
pub fn outline_curve(points: Vec<f64>, steps: i32) -> Result<Vec<i32>, JsValue> {
    let steps = u32::try_from(steps)
        .map_err(|_| JsValue::from_str("steps must be greater than or equal to 0"))?;
    let pts = pillow_rs::outline_curve_points(&points, steps);
    let mut flat = Vec::with_capacity(pts.len() * 2);
    for (x, y) in pts {
        flat.push(x);
        flat.push(y);
    }
    Ok(flat)
}

// ══════════════════════════════════════════════════════════════════════════════
// Color helpers
// ══════════════════════════════════════════════════════════════════════════════
