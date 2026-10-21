//! WASM image save and display bindings.

use super::{Image, err, js_optional_string};
use pillow_rs::Image as RsImage;
use wasm_bindgen::prelude::{JsValue, wasm_bindgen};

#[wasm_bindgen]
impl Image {
    #[wasm_bindgen(js_name = "save")]
    pub fn save(&mut self) -> Result<Vec<u8>, JsValue> {
        // Returns PNG-encoded bytes for download (browser) or fs.writeFile (server).
        // Uses the image crate's PNG encoder built into pillow-rs.
        self.inner.to_png_bytes().map_err(err)
    }

    #[wasm_bindgen(js_name = "saveWithInput")]
    pub fn save_with_input(
        &mut self,
        format: JsValue,
        extension: JsValue,
    ) -> Result<Vec<u8>, JsValue> {
        let format = js_optional_string(&format, "format")?;
        let extension = js_optional_string(&extension, "extension")?;
        let resolved =
            RsImage::resolve_save_format(format.as_deref(), extension.as_deref()).map_err(err)?;
        self.inner.encode(&resolved).map_err(err)
    }

    /// Encode the image in the selected format with Pillow-style save options.
    #[wasm_bindgen(js_name = "saveWithOptions")]
    pub fn save_with_options(
        &self,
        format: &str,
        option_pairs: Vec<String>,
    ) -> Result<Vec<u8>, JsValue> {
        if option_pairs.len() % 2 != 0 {
            return Err(err(pillow_rs::PilError::ValueError(
                "encoder options must be key/value pairs".to_owned(),
            )));
        }
        let options: Vec<(String, String)> = option_pairs
            .chunks_exact(2)
            .map(|pair| {
                let key = if format.eq_ignore_ascii_case("PNG") && pair[0] == "compress_level" {
                    "compression".to_owned()
                } else {
                    pair[0].clone()
                };
                (key, pair[1].clone())
            })
            .collect();
        self.inner
            .encode_with_options(format, &options)
            .map_err(err)
    }

    /// Encode DynamicImage to PNG bytes.
    fn encode_png(img: &mut RsImage) -> Result<Vec<u8>, JsValue> {
        img.to_png_bytes()
            .map_err(|error| JsValue::from_str(&error.to_string()))
    }

    #[wasm_bindgen(js_name = "show")]
    pub fn show(&self) {}

    #[wasm_bindgen(js_name = "close")]
    pub fn close(&self) {}

    #[wasm_bindgen(js_name = "draftFn")]
    pub fn draft_fn(&self, _m: &str, _w: u32, _h: u32) -> Image {
        Image {
            inner: self.inner.clone(),
        }
    }

    #[wasm_bindgen(js_name = "toqimage")]
    pub fn toqimage(&self) -> JsValue {
        JsValue::from_str("Qt not available in WASM")
    }

    #[wasm_bindgen(js_name = "toqpixmap")]
    pub fn toqpixmap(&self) -> JsValue {
        JsValue::from_str("Qt not available in WASM")
    }

    #[wasm_bindgen(js_name = "getim")]
    pub fn getim(&self) -> JsValue {
        JsValue::null()
    }
}
