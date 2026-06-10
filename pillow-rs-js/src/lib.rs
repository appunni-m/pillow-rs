use wasm_bindgen::prelude::*;
use pillow_rs_core::image::Image as RsImage;

#[wasm_bindgen]
pub struct Image {
    inner: RsImage,
}

#[wasm_bindgen]
impl Image {
    #[wasm_bindgen(constructor)]
    pub fn new(mode: &str, width: u32, height: u32, r: u8, g: u8, b: u8, a: u8) -> Result<Image, JsValue> {
        console_error_panic_hook::set_once();
        let img = RsImage::new(width, height, mode, (r, g, b, a))
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(Image { inner: img })
    }

    #[wasm_bindgen(js_name = "open")]
    pub fn open(data: Vec<u8>) -> Result<Image, JsValue> {
        let img = RsImage::open_bytes(data)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(Image { inner: img })
    }

    #[wasm_bindgen(js_name = "resize")]
    pub fn resize_js(&self, width: u32, height: u32, filter: Option<String>) -> Result<Image, JsValue> {
        let rs = self.inner.resize((width, height), filter.as_deref())
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(Image { inner: rs })
    }

    #[wasm_bindgen(js_name = "crop")]
    pub fn crop_js(&self, left: u32, top: u32, right: u32, bottom: u32) -> Result<Image, JsValue> {
        let rs = self.inner.crop((left, top, right, bottom))
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(Image { inner: rs })
    }

    #[wasm_bindgen(js_name = "rotate")]
    pub fn rotate_js(&self, angle: f64) -> Result<Image, JsValue> {
        let rs = self.inner.rotate(angle, false, None)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(Image { inner: rs })
    }

    #[wasm_bindgen(js_name = "convert")]
    pub fn convert_js(&self, mode: &str) -> Result<Image, JsValue> {
        let rs = self.inner.convert(mode, None, None, None, None)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(Image { inner: rs })
    }

    #[wasm_bindgen(js_name = "toBytes")]
    pub fn to_bytes_js(&mut self) -> Result<Vec<u8>, JsValue> {
        self.inner.to_bytes().map_err(|e| JsValue::from_str(&e.to_string()))
    }

    #[wasm_bindgen(js_name = "copy")]
    pub fn copy_js(&self) -> Image {
        Image { inner: self.inner.copy() }
    }

    #[wasm_bindgen(getter)]
    pub fn width_js(&mut self) -> Result<u32, JsValue> {
        let (w, _) = self.inner.size().map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(w)
    }

    #[wasm_bindgen(getter)]
    pub fn height_js(&mut self) -> Result<u32, JsValue> {
        let (_, h) = self.inner.size().map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(h)
    }

    #[wasm_bindgen(getter)]
    pub fn mode_js(&mut self) -> Result<String, JsValue> {
        self.inner.mode().map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn size(&mut self) -> Result<Vec<u32>, JsValue> {
        let (w, h) = self.inner.size().map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(vec![w, h])
    }

    pub fn repr(&mut self) -> Result<String, JsValue> {
        let (w, h) = self.inner.size().map_err(|e| JsValue::from_str(&e.to_string()))?;
        let mode = self.inner.mode().map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(format!("<Image size={}x{} mode={}>", w, h, mode))
    }
}
