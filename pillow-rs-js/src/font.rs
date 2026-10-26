use crate::inputs::*;
use crate::values::*;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::{JsValue, wasm_bindgen};

use pillow_rs::FreeTypeFont as RsImageFont;

#[wasm_bindgen]
/// A TrueType or OpenType font loaded by the Rust font engine.
pub struct ImageFont {
    pub(crate) font: RsImageFont,
}

#[wasm_bindgen]
/// Rasterized font mask with its mode, dimensions, and placement offset.
pub struct ImageFontMask {
    width: u32,
    height: u32,
    mode: String,
    offset_x: i32,
    offset_y: i32,
    pixels: Vec<u8>,
}

#[wasm_bindgen]
impl ImageFontMask {
    /// Returns the mask width in pixels.
    #[wasm_bindgen(getter)]
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Returns the mask height in pixels.
    #[wasm_bindgen(getter)]
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Returns the mask pixel mode, such as `L` or `RGBA`.
    #[wasm_bindgen(getter)]
    pub fn mode(&self) -> String {
        self.mode.clone()
    }

    /// Returns the mask's horizontal placement offset.
    #[wasm_bindgen(getter, js_name = "offsetX")]
    pub fn offset_x(&self) -> i32 {
        self.offset_x
    }

    /// Returns the mask's vertical placement offset.
    #[wasm_bindgen(getter, js_name = "offsetY")]
    pub fn offset_y(&self) -> i32 {
        self.offset_y
    }

    /// Returns the mask's packed pixel bytes.
    #[wasm_bindgen(getter)]
    pub fn pixels(&self) -> Vec<u8> {
        self.pixels.clone()
    }
}

#[wasm_bindgen]
impl ImageFont {
    /// Loads a font from bytes at the requested pixel size.
    #[wasm_bindgen(constructor)]
    pub fn new(data: Vec<u8>, size: f32) -> Result<ImageFont, JsValue> {
        pillow_rs::imagefont_from_bytes(data, size)
            .map(|f| ImageFont { font: f })
            .map_err(err)
    }

    /// Loads a font from bytes with optional face index, encoding, and layout engine.
    #[wasm_bindgen(js_name = "fromBytes")]
    pub fn from_bytes(
        data: Vec<u8>,
        size: f32,
        index: Option<u32>,
        encoding: Option<String>,
        layout_engine: Option<String>,
    ) -> Result<ImageFont, JsValue> {
        let options = pillow_rs::ImageFontLoadOptions {
            index: index.map(|value| value as usize),
            encoding: encoding.filter(|value| !value.is_empty()),
            layout_engine,
        };
        pillow_rs::imagefont_from_bytes_with_options(data, size, &options)
            .map(|font| ImageFont { font })
            .map_err(err)
    }

    /// Rasterizes `text` to an `L` mask and returns its placement offset.
    #[wasm_bindgen(js_name = "getmask2")]
    pub fn getmask2(
        &self,
        text: &str,
        start_x: Option<f64>,
        start_y: Option<f64>,
    ) -> Result<ImageFontMask, JsValue> {
        let (width, height, pixels, offset) = pillow_rs::imagefont_getmask2_with_start(
            &self.font,
            text,
            (start_x.unwrap_or(0.0), start_y.unwrap_or(0.0)),
        )
        .map_err(err)?;
        Ok(ImageFontMask {
            width,
            height,
            mode: "L".to_owned(),
            offset_x: offset.0,
            offset_y: offset.1,
            pixels,
        })
    }

    /// Returns the text bounding box using the supplied direction, features, language,
    /// stroke, and anchor options.
    #[wasm_bindgen(js_name = "getbboxWithOptions")]
    #[allow(clippy::too_many_arguments)]
    pub fn getbbox_with_options(
        &self,
        text: JsValue,
        mode: Option<String>,
        direction: Option<String>,
        features: JsValue,
        language: Option<String>,
        stroke_width: f64,
        anchor: Option<String>,
    ) -> Result<Vec<f64>, JsValue> {
        let options = js_font_options(JsFontOptionsInput {
            mode,
            direction,
            features: Some(&features),
            language,
            stroke_width,
            anchor,
            ..JsFontOptionsInput::default()
        });
        let bbox = pillow_rs::imagefont_getbbox_input_with_options(
            &self.font,
            js_font_text_input(&text),
            &options,
        )
        .map_err(err)?;
        Ok(vec![
            bbox.0 as f64,
            bbox.1 as f64,
            bbox.2 as f64,
            bbox.3 as f64,
        ])
    }

    /// Measures text advance using the supplied shaping options.
    #[wasm_bindgen(js_name = "getlengthWithOptions")]
    pub fn getlength_with_options(
        &self,
        text: JsValue,
        mode: Option<String>,
        direction: Option<String>,
        features: JsValue,
        language: Option<String>,
    ) -> Result<f64, JsValue> {
        let options = js_font_options(JsFontOptionsInput {
            mode,
            direction,
            features: Some(&features),
            language,
            ..JsFontOptionsInput::default()
        });
        pillow_rs::imagefont_getlength_input_with_options(
            &self.font,
            js_font_text_input(&text),
            &options,
        )
        .map(|value| value as f64)
        .map_err(err)
    }

    /// Returns the font ascent and descent as a two-item array.
    #[wasm_bindgen(js_name = "getMetrics")]
    pub fn get_metrics(&self) -> Vec<u32> {
        let (ascent, descent) = pillow_rs::imagefont_getmetrics(&self.font);
        vec![ascent, descent]
    }

    /// Rasterizes text with explicit shaping, stroke, anchor, ink, and start-position options.
    #[wasm_bindgen(js_name = "getmaskWithOptions")]
    #[allow(clippy::too_many_arguments)]
    pub fn getmask_with_options(
        &self,
        text: JsValue,
        mode: Option<String>,
        direction: Option<String>,
        features: JsValue,
        language: Option<String>,
        stroke_width: f64,
        anchor: Option<String>,
        ink: JsValue,
        start: JsValue,
    ) -> Result<ImageFontMask, JsValue> {
        let ink = js_font_ink(&ink)?;
        let options = js_font_options(JsFontOptionsInput {
            mode,
            direction,
            features: Some(&features),
            language,
            stroke_width,
            anchor,
            ink,
            start: Some(&start),
            ..JsFontOptionsInput::default()
        });
        let (width, height, pixels) = pillow_rs::imagefont_getmask_input_with_options(
            &self.font,
            js_font_text_input(&text),
            &options,
        )
        .map_err(err)?;
        Ok(ImageFontMask {
            width,
            height,
            mode: if options.uses_color_mask() {
                "RGBA".to_owned()
            } else {
                "L".to_owned()
            },
            offset_x: 0,
            offset_y: 0,
            pixels,
        })
    }

    /// Rasterizes text and returns its mask dimensions, mode, pixels, and placement offset.
    #[wasm_bindgen(js_name = "getmask2WithOptions")]
    #[allow(clippy::too_many_arguments)]
    pub fn getmask2_with_options(
        &self,
        text: JsValue,
        mode: Option<String>,
        direction: Option<String>,
        features: JsValue,
        language: Option<String>,
        stroke_width: f64,
        anchor: Option<String>,
        ink: JsValue,
        start: JsValue,
        stroke_filled: bool,
        has_args: bool,
        has_kwargs: bool,
    ) -> Result<ImageFontMask, JsValue> {
        let ink = js_font_ink(&ink)?;
        let options = js_font_options(JsFontOptionsInput {
            mode,
            direction,
            features: Some(&features),
            language,
            stroke_width,
            anchor,
            ink,
            start: Some(&start),
            stroke_filled,
            has_args,
            has_kwargs,
            ..JsFontOptionsInput::default()
        });
        let (width, height, pixels, offset) = pillow_rs::imagefont_getmask2_input_with_options(
            &self.font,
            js_font_text_input(&text),
            &options,
        )
        .map_err(err)?;
        Ok(ImageFontMask {
            width,
            height,
            mode: if options.uses_color_mask() {
                "RGBA".to_owned()
            } else {
                "L".to_owned()
            },
            offset_x: offset.0,
            offset_y: offset.1,
            pixels,
        })
    }

    /// Returns the font family and style names as a two-item array.
    #[wasm_bindgen(js_name = "getname")]
    pub fn getname(&self) -> js_sys::Array {
        let (family, style) = pillow_rs::imagefont_getname(&self.font);
        let result = js_sys::Array::new();
        result.push(&family.map_or(JsValue::NULL, JsValue::from_str));
        result.push(&style.map_or(JsValue::NULL, JsValue::from_str));
        result
    }

    /// Returns the variable-font axes and their minimum, default, maximum, and names.
    #[wasm_bindgen(js_name = "getVariationAxes")]
    pub fn get_variation_axes(&self) -> Result<js_sys::Array, JsValue> {
        let axes = pillow_rs::imagefont_get_variation_axes(&self.font).map_err(err)?;
        let result = js_sys::Array::new();
        for axis in axes {
            let object = js_sys::Object::new();
            js_sys::Reflect::set(
                &object,
                &JsValue::from_str("minimum"),
                &JsValue::from_f64(axis.minimum as f64),
            )?;
            js_sys::Reflect::set(
                &object,
                &JsValue::from_str("default"),
                &JsValue::from_f64(axis.default as f64),
            )?;
            js_sys::Reflect::set(
                &object,
                &JsValue::from_str("maximum"),
                &JsValue::from_f64(axis.maximum as f64),
            )?;
            js_sys::Reflect::set(
                &object,
                &JsValue::from_str("name"),
                &js_sys::Uint8Array::from(axis.name.as_slice()).into(),
            )?;
            result.push(&object);
        }
        Ok(result)
    }

    /// Returns the named variation presets available in the font.
    #[wasm_bindgen(js_name = "getVariationNames")]
    pub fn get_variation_names(&self) -> Result<js_sys::Array, JsValue> {
        let names = pillow_rs::imagefont_get_variation_names(&self.font).map_err(err)?;
        let result = js_sys::Array::new();
        for name in names {
            result.push(&js_sys::Uint8Array::from(name.as_slice()));
        }
        Ok(result)
    }

    /// Selects a variable-font preset by its encoded name bytes.
    #[wasm_bindgen(js_name = "setVariationByName")]
    pub fn set_variation_by_name(&mut self, name: Vec<u8>) -> Result<(), JsValue> {
        pillow_rs::imagefont_set_variation_by_name(&mut self.font, &name).map_err(err)
    }

    /// Selects a variable-font preset from a JavaScript name input.
    #[wasm_bindgen(js_name = "setVariationByNameWithInput")]
    pub fn set_variation_by_name_with_input(&mut self, name: JsValue) -> Result<(), JsValue> {
        pillow_rs::imagefont_set_variation_by_name_input(
            &mut self.font,
            js_font_variation_name_input(&name),
        )
        .map_err(err)
    }

    /// Sets the variable-font axis coordinates in axis order.
    #[wasm_bindgen(js_name = "setVariationByAxes")]
    pub fn set_variation_by_axes(&mut self, axes: Vec<f32>) -> Result<(), JsValue> {
        pillow_rs::imagefont_set_variation_by_axes(&mut self.font, &axes).map_err(err)
    }

    /// Sets variable-font axes from a JavaScript sequence input.
    #[wasm_bindgen(js_name = "setVariationByAxesWithInput")]
    pub fn set_variation_by_axes_with_input(&mut self, axes: JsValue) -> Result<(), JsValue> {
        pillow_rs::imagefont_set_variation_by_axes_input(
            &mut self.font,
            js_font_variation_axes_input(&axes),
        )
        .map_err(err)
    }

    /// Creates a font variant, optionally changing its pixel size.
    #[wasm_bindgen(js_name = "fontVariant")]
    pub fn font_variant(&self, size: Option<f32>) -> Result<ImageFont, JsValue> {
        pillow_rs::imagefont_variant(&self.font, size)
            .map(|font| ImageFont { font })
            .map_err(err)
    }

    /// Creates a font variant with optional replacement font bytes and load options.
    #[wasm_bindgen(js_name = "fontVariantWithOptions")]
    pub fn font_variant_with_options(
        &self,
        font: JsValue,
        size: Option<f64>,
        index: Option<u32>,
        encoding: Option<String>,
        layout_engine: Option<String>,
    ) -> Result<ImageFont, JsValue> {
        let font_bytes = if font.is_null() || font.is_undefined() {
            None
        } else if font.is_instance_of::<js_sys::Uint8Array>() {
            Some(js_sys::Uint8Array::new(&font).to_vec())
        } else {
            return Err(err(pillow_rs::PilError::TypeError(
                "font must be a file path or file-like object".to_owned(),
            )));
        };
        let options = pillow_rs::ImageFontVariantOptions {
            font_bytes,
            size: size.map(|value| value as f32),
            index: index.map(|value| value as usize),
            encoding,
            layout_engine,
        };
        pillow_rs::imagefont_variant_with_options(&self.font, &options)
            .map(|font| ImageFont { font })
            .map_err(err)
    }

    /// Rasterizes text after applying the requested text orientation.
    #[wasm_bindgen(js_name = "getTransposedMask")]
    pub fn get_transposed_mask(
        &self,
        text: &str,
        orientation: Option<String>,
    ) -> Result<ImageFontMask, JsValue> {
        let (width, height, pixels) =
            pillow_rs::imagefont_get_transposed_mask(&self.font, text, orientation.as_deref())
                .map_err(err)?;
        Ok(ImageFontMask {
            width,
            height,
            mode: "L".to_owned(),
            offset_x: 0,
            offset_y: 0,
            pixels,
        })
    }

    /// Returns the text bounding box after applying the requested orientation.
    #[wasm_bindgen(js_name = "getTransposedBbox")]
    pub fn get_transposed_bbox(
        &self,
        text: &str,
        orientation: Option<String>,
    ) -> Result<Vec<i32>, JsValue> {
        let bbox = pillow_rs::transposed_bbox(
            pillow_rs::imagefont_getbbox(&self.font, text).map_err(err)?,
            orientation.as_deref(),
        );
        Ok(vec![bbox.0, bbox.1, bbox.2, bbox.3])
    }

    /// Measures text advance after validating the requested orientation.
    #[wasm_bindgen(js_name = "getTransposedLength")]
    pub fn get_transposed_length(
        &self,
        text: &str,
        orientation: Option<String>,
    ) -> Result<f32, JsValue> {
        pillow_rs::validate_transposed_length(orientation.as_deref()).map_err(err)?;
        pillow_rs::imagefont_getlength(&self.font, text).map_err(err)
    }

    /// Returns the text mask dimensions as `[width, height]`.
    #[wasm_bindgen(js_name = "getbbox")]
    pub fn getbbox(&self, text: &str) -> Result<Vec<u32>, JsValue> {
        let (w, h) = pillow_rs::imagefont_text_bbox(&self.font, text).map_err(err)?;
        Ok(vec![w, h])
    }
    /// Returns mask bytes prefixed by 32-bit little-endian width and height values.
    #[wasm_bindgen(js_name = "getmask")]
    pub fn getmask(&self, text: &str) -> Result<Vec<u8>, JsValue> {
        let (w, h, data) = pillow_rs::imagefont_getmask(&self.font, text).map_err(err)?;
        let mut result = vec![
            w as u8,
            (w >> 8) as u8,
            (w >> 16) as u8,
            (w >> 24) as u8,
            h as u8,
            (h >> 8) as u8,
            (h >> 16) as u8,
            (h >> 24) as u8,
        ];
        result.extend(data);
        Ok(result)
    }
}

// ── ImagePalette ─────────────────────────────────────────────────
#[wasm_bindgen]
impl ImageFont {
    #[wasm_bindgen(js_name = "load")]
    /// Returns an error because WASM cannot load fonts from filesystem paths.
    pub fn load(_path: &str, _size: f32) -> Result<ImageFont, JsValue> {
        Err(JsValue::from_str(
            "Use new ImageFont(data, size) with font bytes",
        ))
    }
    #[wasm_bindgen(js_name = "loadPath")]
    /// Returns an error because WASM cannot load fonts from filesystem paths.
    pub fn load_path(_path: &str, _size: f32) -> Result<ImageFont, JsValue> {
        Err(JsValue::from_str(
            "Use new ImageFont(data, size) with font bytes",
        ))
    }
    #[wasm_bindgen(js_name = "loadDefault")]
    /// Loads the built-in default font at its default size.
    pub fn load_default() -> Result<ImageFont, JsValue> {
        pillow_rs::imagefont_load_default(10.0)
            .map(|font| ImageFont { font })
            .map_err(err)
    }
}

// ── Legacy PILfont ──────────────────────────────────────────────
#[wasm_bindgen]
/// Legacy bitmap font with metrics and glyph-image data.
pub struct PilFont {
    font: pillow_rs::PilFont,
}

#[wasm_bindgen]
impl PilFont {
    #[wasm_bindgen(js_name = "fromBytes")]
    /// Creates a bitmap font from its metrics bytes and encoded glyph image.
    pub fn from_bytes(metrics: Vec<u8>, glyph_image: Vec<u8>) -> Result<PilFont, JsValue> {
        let glyph_image = pillow_rs::PilFont::open_pilfont_glyph_image(glyph_image).map_err(err)?;
        pillow_rs::PilFont::from_pilfont_glyph_data(&metrics, glyph_image)
            .map(|font| PilFont { font })
            .map_err(err)
    }

    #[wasm_bindgen(js_name = "loadDefault")]
    /// Loads the built-in default bitmap font.
    pub fn load_default() -> Result<PilFont, JsValue> {
        pillow_rs::PilFont::load_default()
            .map(|font| PilFont { font })
            .map_err(err)
    }

    #[wasm_bindgen(js_name = "getbboxWithInput")]
    /// Returns the bounding box for text supplied using the legacy input contract.
    pub fn getbbox_with_input(&self, text: JsValue) -> Result<Vec<i32>, JsValue> {
        let bbox = self
            .font
            .getbbox_input(js_pilfont_text_input(&text))
            .map_err(err)?;
        Ok(vec![bbox.0, bbox.1, bbox.2, bbox.3])
    }

    #[wasm_bindgen(js_name = "getlengthWithInput")]
    /// Returns the advance length for text supplied using the legacy input contract.
    pub fn getlength_with_input(&self, text: JsValue) -> Result<i32, JsValue> {
        self.font
            .getlength_input(js_pilfont_text_input(&text))
            .map_err(err)
    }

    #[wasm_bindgen(js_name = "getmaskWithInput")]
    /// Rasterizes text supplied using the legacy input contract into a font mask.
    pub fn getmask_with_input(&self, text: JsValue) -> Result<ImageFontMask, JsValue> {
        let mask = self
            .font
            .getmask_input(js_pilfont_text_input(&text))
            .map_err(err)?;
        Ok(ImageFontMask {
            width: mask.width,
            height: mask.height,
            mode: mask.mode.as_str().to_owned(),
            offset_x: 0,
            offset_y: 0,
            pixels: mask.pixels,
        })
    }
}
