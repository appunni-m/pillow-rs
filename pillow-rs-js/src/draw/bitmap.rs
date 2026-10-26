use super::{Image, ImageDraw, JsValue, wasm_bindgen};
use crate::{err, js_draw_color_input};
#[wasm_bindgen]
impl ImageDraw {
    #[wasm_bindgen(js_name = "bitmap")]
    /// Draws a bitmap image at `(x, y)` with an optional RGBA foreground color.
    #[allow(clippy::too_many_arguments)] // Preserve the existing JavaScript method signature.
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
