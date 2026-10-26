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
    #[wasm_bindgen(js_name = "rectangle")]
    /// Draws a rectangle from integer coordinates and optional RGBA fill and outline.
    pub fn rect(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        fr: Option<u8>,
        fg: Option<u8>,
        fb: Option<u8>,
        fa: Option<u8>,
        or: Option<u8>,
        og: Option<u8>,
        ob: Option<u8>,
        oa: Option<u8>,
        width: Option<u32>,
    ) -> Result<(), JsValue> {
        let fill = fr.map(|r| (r, fg.unwrap_or(0), fb.unwrap_or(0), fa.unwrap_or(255)));
        let out = or.map(|r| (r, og.unwrap_or(0), ob.unwrap_or(0), oa.unwrap_or(255)));
        self.draw
            .rectangle(x0, y0, x1, y1, fill, out, width.unwrap_or(1))
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "ellipse")]
    /// Draws an ellipse from integer bounds and optional RGBA fill and outline.
    pub fn ellipse(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        fr: Option<u8>,
        fg: Option<u8>,
        fb: Option<u8>,
        fa: Option<u8>,
        or: Option<u8>,
        og: Option<u8>,
        ob: Option<u8>,
        oa: Option<u8>,
        width: Option<u32>,
    ) -> Result<(), JsValue> {
        let fill = fr.map(|r| (r, fg.unwrap_or(0), fb.unwrap_or(0), fa.unwrap_or(255)));
        let out = or.map(|r| (r, og.unwrap_or(0), ob.unwrap_or(0), oa.unwrap_or(255)));
        self.draw
            .ellipse(x0, y0, x1, y1, fill, out, width.unwrap_or(1))
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "polygon")]
    /// Draws a polygon from flattened integer coordinates and optional colors.
    pub fn polygon(
        &mut self,
        points: Vec<i32>,
        fr: Option<u8>,
        fg: Option<u8>,
        fb: Option<u8>,
        fa: Option<u8>,
        or: Option<u8>,
        og: Option<u8>,
        ob: Option<u8>,
        oa: Option<u8>,
        width: Option<u32>,
    ) -> Result<(), JsValue> {
        let fill = fr.map(|r| (r, fg.unwrap_or(0), fb.unwrap_or(0), fa.unwrap_or(255)));
        let out = or.map(|r| (r, og.unwrap_or(0), ob.unwrap_or(0), oa.unwrap_or(255)));
        self.draw
            .polygon_with_input(
                pillow_rs::DrawPointsInput::Flat(points),
                fill,
                out,
                width.unwrap_or(1),
            )
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "point")]
    /// Draws points from flattened integer coordinates using an RGBA color.
    pub fn point(&mut self, pts: Vec<i32>, r: u8, g: u8, b: u8, a: u8) -> Result<(), JsValue> {
        self.draw
            .point_with_input(pillow_rs::DrawPointsInput::Flat(pts), (r, g, b, a))
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "arc")]
    /// Draws an arc inside integer bounds from `start` to `end` degrees.
    pub fn arc(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        start: f64,
        end: f64,
        r: u8,
        g: u8,
        b: u8,
        a: u8,
        width: Option<u32>,
    ) -> Result<(), JsValue> {
        self.draw
            .arc(x0, y0, x1, y1, start, end, (r, g, b, a), width.unwrap_or(1))
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "chord")]
    /// Draws a chord inside integer bounds with optional RGBA fill and outline colors.
    pub fn chord(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        start: f64,
        end: f64,
        fr: Option<u8>,
        fg: Option<u8>,
        fb: Option<u8>,
        fa: Option<u8>,
        or: Option<u8>,
        og: Option<u8>,
        ob: Option<u8>,
        oa: Option<u8>,
        width: Option<u32>,
    ) -> Result<(), JsValue> {
        let fill = fr.map(|r| (r, fg.unwrap_or(0), fb.unwrap_or(0), fa.unwrap_or(255)));
        let out = or.map(|r| (r, og.unwrap_or(0), ob.unwrap_or(0), oa.unwrap_or(255)));
        self.draw
            .chord(x0, y0, x1, y1, start, end, fill, out, width.unwrap_or(1))
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "pieslice")]
    /// Draws a pie slice inside integer bounds with optional RGBA fill and outline colors.
    pub fn pieslice(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        start: f64,
        end: f64,
        fr: Option<u8>,
        fg: Option<u8>,
        fb: Option<u8>,
        fa: Option<u8>,
        or: Option<u8>,
        og: Option<u8>,
        ob: Option<u8>,
        oa: Option<u8>,
        width: Option<u32>,
    ) -> Result<(), JsValue> {
        let fill = fr.map(|r| (r, fg.unwrap_or(0), fb.unwrap_or(0), fa.unwrap_or(255)));
        let out = or.map(|r| (r, og.unwrap_or(0), ob.unwrap_or(0), oa.unwrap_or(255)));
        self.draw
            .pieslice(x0, y0, x1, y1, start, end, fill, out, width.unwrap_or(1))
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "circle")]
    /// Draws a circle from its center, radius, and optional RGBA colors.
    pub fn circle(
        &mut self,
        cx: f64,
        cy: f64,
        radius: f64,
        fr: Option<u8>,
        fg: Option<u8>,
        fb: Option<u8>,
        fa: Option<u8>,
        or: Option<u8>,
        og: Option<u8>,
        ob: Option<u8>,
        oa: Option<u8>,
        width: Option<u32>,
    ) -> Result<(), JsValue> {
        let fill = fr.map(|r| (r, fg.unwrap_or(0), fb.unwrap_or(0), fa.unwrap_or(255)));
        let out = or.map(|r| (r, og.unwrap_or(0), ob.unwrap_or(0), oa.unwrap_or(255)));
        self.draw
            .circle(cx as i32, cy as i32, radius, fill, out, width.unwrap_or(1))
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "roundedRectangle")]
    /// Draws a rounded rectangle from integer bounds and a corner radius.
    pub fn rounded_rect(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        radius: f64,
        fr: Option<u8>,
        fg: Option<u8>,
        fb: Option<u8>,
        fa: Option<u8>,
        or: Option<u8>,
        og: Option<u8>,
        ob: Option<u8>,
        oa: Option<u8>,
        width: Option<u32>,
    ) -> Result<(), JsValue> {
        let fill = fr.map(|r| (r, fg.unwrap_or(0), fb.unwrap_or(0), fa.unwrap_or(255)));
        let out = or.map(|r| (r, og.unwrap_or(0), ob.unwrap_or(0), oa.unwrap_or(255)));
        self.draw
            .rounded_rectangle(x0, y0, x1, y1, radius, fill, out, width.unwrap_or(1))
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "text")]
    /// Draws text at `(x, y)` using a loaded font and RGBA fill color.
    pub fn text(
        &mut self,
        x: f64,
        y: f64,
        text: &str,
        font: &ImageFont,
        r: u8,
        g: u8,
        b: u8,
        a: u8,
    ) -> Result<(), JsValue> {
        self.draw
            .text(x as i32, y as i32, text, &font.font, (r, g, b, a))
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "textWithInput")]
    /// Draws text using JavaScript text, font, fill, shaping, and stroke inputs.
    #[allow(clippy::too_many_arguments)]
    pub fn text_with_input(
        &mut self,
        x: f64,
        y: f64,
        text: JsValue,
        font: Option<ImageFont>,
        fill: JsValue,
        direction: Option<String>,
        features: JsValue,
        language: Option<String>,
        stroke_width: f64,
        anchor: Option<String>,
        embedded_color: bool,
        font_size: Option<f64>,
        stroke_fill: JsValue,
    ) -> Result<(), JsValue> {
        let mut options = js_font_options(
            None,
            direction,
            &features,
            language,
            stroke_width,
            anchor,
            None,
            &JsValue::NULL,
            embedded_color,
            false,
            false,
            false,
            true,
        );
        let (color, text_fill_is_integer) = js_text_color_and_kind(&self.draw, &fill)?;
        options.text_fill_is_integer = text_fill_is_integer;
        if !stroke_fill.is_null() && !stroke_fill.is_undefined() {
            let (color, is_integer) = js_text_color_and_kind(&self.draw, &stroke_fill)?;
            options.stroke_fill = Some(color);
            options.stroke_fill_is_integer = is_integer;
        }
        self.draw
            .text_with_optional_font_input(
                x as i32,
                y as i32,
                js_font_text_input(&text),
                font.as_ref().map(|font| &font.font),
                color,
                font_size.map(|size| size as f32),
                &options,
            )
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "multilineTextWithInput")]
    /// Draws multiline text with explicit spacing and shaping options.
    #[allow(clippy::too_many_arguments)]
    pub fn multiline_text_with_input(
        &mut self,
        x: f64,
        y: f64,
        text: JsValue,
        font: Option<ImageFont>,
        fill: JsValue,
        spacing: f64,
        direction: Option<String>,
        features: JsValue,
        language: Option<String>,
        stroke_width: f64,
        anchor: Option<String>,
        embedded_color: bool,
        font_size: Option<f64>,
        stroke_fill: JsValue,
    ) -> Result<(), JsValue> {
        let mut options = js_font_options(
            None,
            direction,
            &features,
            language,
            stroke_width,
            anchor,
            None,
            &JsValue::NULL,
            embedded_color,
            false,
            false,
            false,
            true,
        );
        let (color, text_fill_is_integer) = js_text_color_and_kind(&self.draw, &fill)?;
        options.text_fill_is_integer = text_fill_is_integer;
        if !stroke_fill.is_null() && !stroke_fill.is_undefined() {
            let (color, is_integer) = js_text_color_and_kind(&self.draw, &stroke_fill)?;
            options.stroke_fill = Some(color);
            options.stroke_fill_is_integer = is_integer;
        }
        self.draw
            .multiline_text_with_optional_font_input(
                x,
                y,
                js_font_text_input(&text),
                font.as_ref().map(|font| &font.font),
                color,
                spacing,
                font_size.map(|size| size as f32),
                &options,
            )
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "textbboxWithInput")]
    /// Returns the text bounding box at `(x, y)` using the supplied font options.
    #[allow(clippy::too_many_arguments)]
    pub fn textbbox_with_input(
        &mut self,
        x: i32,
        y: i32,
        text: JsValue,
        font: Option<ImageFont>,
        direction: Option<String>,
        features: JsValue,
        language: Option<String>,
        stroke_width: f64,
        anchor: Option<String>,
        embedded_color: bool,
        font_size: Option<f64>,
    ) -> Result<Vec<i32>, JsValue> {
        let options = js_font_options(
            None,
            direction,
            &features,
            language,
            stroke_width,
            anchor,
            None,
            &JsValue::NULL,
            embedded_color,
            false,
            false,
            false,
            true,
        );
        self.draw.validate_text_options(&options).map_err(err)?;
        let bbox = pillow_rs::imagefont_textbbox_at_with_optional_font_input(
            font.as_ref().map(|font| &font.font),
            font_size.map(|size| size as f32),
            (x, y),
            js_font_text_input(&text),
            &options,
        )
        .map_err(err)?;
        Ok(vec![bbox.0, bbox.1, bbox.2, bbox.3])
    }
    #[wasm_bindgen(js_name = "textlengthWithInput")]
    /// Measures text advance using the supplied font and shaping options.
    #[allow(clippy::too_many_arguments)]
    pub fn textlength_with_input(
        &mut self,
        text: JsValue,
        font: Option<ImageFont>,
        direction: Option<String>,
        features: JsValue,
        language: Option<String>,
        embedded_color: bool,
        font_size: Option<f64>,
    ) -> Result<f64, JsValue> {
        let options = js_font_options(
            None,
            direction,
            &features,
            language,
            0.0,
            None,
            None,
            &JsValue::NULL,
            embedded_color,
            false,
            false,
            false,
            false,
        );
        self.draw.validate_text_options(&options).map_err(err)?;
        pillow_rs::imagefont_getlength_with_optional_font_input(
            font.as_ref().map(|font| &font.font),
            font_size.map(|size| size as f32),
            js_font_text_input(&text),
            &options,
        )
        .map(|value| value as f64)
        .map_err(err)
    }
    #[wasm_bindgen(js_name = "multilineTextbboxWithInput")]
    /// Returns the bounding box of multiline text with the supplied layout options.
    #[allow(clippy::too_many_arguments)]
    pub fn multiline_textbbox_with_input(
        &mut self,
        x: i32,
        y: i32,
        text: JsValue,
        font: Option<ImageFont>,
        spacing: i32,
        align: String,
        direction: Option<String>,
        features: JsValue,
        language: Option<String>,
        stroke_width: f64,
        anchor: Option<String>,
        embedded_color: bool,
        font_size: Option<f64>,
    ) -> Result<Vec<i32>, JsValue> {
        let options = js_font_options(
            None,
            direction,
            &features,
            language,
            stroke_width,
            anchor,
            None,
            &JsValue::NULL,
            embedded_color,
            false,
            false,
            false,
            true,
        );
        self.draw.validate_text_options(&options).map_err(err)?;
        let bbox = pillow_rs::imagefont_multiline_textbbox_with_optional_font_input(
            font.as_ref().map(|font| &font.font),
            font_size.map(|size| size as f32),
            (x, y),
            js_font_text_input(&text),
            spacing,
            &align,
            &options,
        )
        .map_err(err)?;
        Ok(vec![bbox.0, bbox.1, bbox.2, bbox.3])
    }
    #[wasm_bindgen(js_name = "bitmap")]
    /// Draws a bitmap image at `(x, y)` with an optional RGBA foreground color.
    pub fn bitmap(
        &mut self,
        x: i32,
        y: i32,
        bitmap: &Image,
        fr: Option<u8>,
        fg: Option<u8>,
        fb: Option<u8>,
        fa: Option<u8>,
    ) -> Result<(), JsValue> {
        let fill = fr.map(|r| (r, fg.unwrap_or(0), fb.unwrap_or(0), fa.unwrap_or(255)));
        self.draw.bitmap(x, y, &bitmap.inner, fill).map_err(err)
    }
    #[wasm_bindgen(js_name = "bitmapWithInput")]
    /// Draws a bitmap image at `(x, y)` using a JavaScript color value.
    pub fn bitmap_with_input(
        &mut self,
        x: i32,
        y: i32,
        bitmap: &Image,
        fill: JsValue,
    ) -> Result<(), JsValue> {
        self.draw
            .bitmap_with_input(x, y, &bitmap.inner, js_draw_color_input(&fill))
            .map_err(err)
    }
    #[wasm_bindgen(getter)]
    /// Returns a clone of the image containing the current drawing operations.
    pub fn image(&self) -> Result<Image, JsValue> {
        // Core image_clone() already handles mode preservation
        // (RGB→RGB, RGBA→RGBA, L→L, etc.)
        Ok(Image {
            inner: self.draw.image_clone().map_err(err)?,
        })
    }
}

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
