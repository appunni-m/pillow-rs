use super::{ImageDraw, JsValue, wasm_bindgen};
use crate::err;
#[wasm_bindgen]
impl ImageDraw {
    #[wasm_bindgen(js_name = "rectangle")]
    /// Draws a rectangle from integer coordinates and optional RGBA fill and outline.
    #[allow(clippy::too_many_arguments)] // Preserve the existing JavaScript method signature.
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
    #[allow(clippy::too_many_arguments)] // Preserve the existing JavaScript method signature.
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
    #[allow(clippy::too_many_arguments)] // Preserve the existing JavaScript method signature.
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
    #[allow(clippy::too_many_arguments)] // Preserve the existing JavaScript method signature.
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
    #[allow(clippy::too_many_arguments)] // Preserve the existing JavaScript method signature.
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
    #[allow(clippy::too_many_arguments)] // Preserve the existing JavaScript method signature.
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
    #[allow(clippy::too_many_arguments)] // Preserve the existing JavaScript method signature.
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
    #[allow(clippy::too_many_arguments)] // Preserve the existing JavaScript method signature.
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
}
