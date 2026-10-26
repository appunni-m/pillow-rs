use super::{ImageDraw, ImageFont, JsValue, js_text_color_and_kind, wasm_bindgen};
use crate::{JsFontOptionsInput, err, js_font_options, js_font_text_input};
#[wasm_bindgen]
impl ImageDraw {
    #[wasm_bindgen(js_name = "text")]
    /// Draws text at `(x, y)` using a loaded font and RGBA fill color.
    #[allow(clippy::too_many_arguments)] // Preserve the existing JavaScript method signature.
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
        let mut options = js_font_options(JsFontOptionsInput {
            direction,
            features: Some(&features),
            language,
            stroke_width,
            anchor,
            embedded_color,
            anchor_invalid_length_error: true,
            ..JsFontOptionsInput::default()
        });
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
        let mut options = js_font_options(JsFontOptionsInput {
            direction,
            features: Some(&features),
            language,
            stroke_width,
            anchor,
            embedded_color,
            anchor_invalid_length_error: true,
            ..JsFontOptionsInput::default()
        });
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
        let options = js_font_options(JsFontOptionsInput {
            direction,
            features: Some(&features),
            language,
            stroke_width,
            anchor,
            embedded_color,
            anchor_invalid_length_error: true,
            ..JsFontOptionsInput::default()
        });
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
        let options = js_font_options(JsFontOptionsInput {
            direction,
            features: Some(&features),
            language,
            embedded_color,
            ..JsFontOptionsInput::default()
        });
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
        let options = js_font_options(JsFontOptionsInput {
            direction,
            features: Some(&features),
            language,
            stroke_width,
            anchor,
            embedded_color,
            anchor_invalid_length_error: true,
            ..JsFontOptionsInput::default()
        });
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
}
