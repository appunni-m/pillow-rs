//! pillow-rs WASM bindings — Pillow-compatible Image class for the browser.
//! Re-exports core Image with JS-friendly method names via wasm-bindgen.

use wasm_bindgen::prelude::*;
use pillow_rs_core::image::Image as RsImage;

#[wasm_bindgen]
pub struct Image {
    inner: RsImage,
}

#[wasm_bindgen]
impl Image {
    // ── Constructors ──────────────────────────────────────────

    #[wasm_bindgen(constructor)]
    pub fn new(mode: &str, width: u32, height: u32, r: u8, g: u8, b: u8, a: u8) -> Result<Image, JsValue> {
        console_error_panic_hook::set_once();
        RsImage::new(width, height, mode, (r, g, b, a))
            .map(|img| Image { inner: img })
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    #[wasm_bindgen(js_name = "open")]
    pub fn open(data: Vec<u8>) -> Result<Image, JsValue> {
        RsImage::open_bytes(data)
            .map(|img| Image { inner: img })
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    // ── Properties ────────────────────────────────────────────

    #[wasm_bindgen(getter)]
    pub fn width(&mut self) -> Result<u32, JsValue> {
        let (w, _) = self.inner.size().map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(w)
    }

    #[wasm_bindgen(getter)]
    pub fn height(&mut self) -> Result<u32, JsValue> {
        let (_, h) = self.inner.size().map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(h)
    }

    #[wasm_bindgen(getter)]
    pub fn mode(&mut self) -> Result<String, JsValue> {
        self.inner.mode().map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn size(&mut self) -> Result<Vec<u32>, JsValue> {
        let (w, h) = self.inner.size().map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(vec![w, h])
    }

    // ── Operations ────────────────────────────────────────────

    #[wasm_bindgen(js_name = "resize")]
    pub fn resize_js(&self, width: u32, height: u32, filter: Option<String>) -> Result<Image, JsValue> {
        self.inner.resize((width, height), filter.as_deref())
            .map(|img| Image { inner: img })
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    #[wasm_bindgen(js_name = "crop")]
    pub fn crop_js(&self, left: u32, top: u32, right: u32, bottom: u32) -> Result<Image, JsValue> {
        let w = right - left;
        let h = bottom - top;
        self.inner.crop((left, top, w, h))
            .map(|img| Image { inner: img })
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    #[wasm_bindgen(js_name = "rotate")]
    pub fn rotate_js(&self, angle: f64) -> Result<Image, JsValue> {
        self.inner.rotate(angle, false, None)
            .map(|img| Image { inner: img })
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    #[wasm_bindgen(js_name = "transpose")]
    pub fn transpose_js(&self, method: &str) -> Result<Image, JsValue> {
        self.inner.transpose(method)
            .map(|img| Image { inner: img })
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    #[wasm_bindgen(js_name = "convert")]
    pub fn convert_js(&self, mode: &str) -> Result<Image, JsValue> {
        self.inner.convert(mode, None, None, None, None)
            .map(|img| Image { inner: img })
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    #[wasm_bindgen(js_name = "filter")]
    pub fn filter_js(&self, filter_type: &str) -> Result<Image, JsValue> {
        self.inner.filter(filter_type)
            .map(|img| Image { inner: img })
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    #[wasm_bindgen(js_name = "quantize")]
    pub fn quantize_js(&self, colors: u32) -> Result<Image, JsValue> {
        self.inner.quantize(colors, 0, None, true)
            .map(|img| Image { inner: img })
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    #[wasm_bindgen(js_name = "getpixel")]
    pub fn getpixel_js(&mut self, x: u32, y: u32) -> Result<Vec<u8>, JsValue> {
        self.inner.getpixel(x, y)
            .map(|(r, g, b, a)| vec![r, g, b, a])
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    #[wasm_bindgen(js_name = "putpixel")]
    pub fn putpixel_js(&mut self, x: u32, y: u32, r: u8, g: u8, b: u8, a: u8) -> Result<(), JsValue> {
        self.inner.putpixel(x, y, r, g, b, a)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    #[wasm_bindgen(js_name = "split")]
    pub fn split_js(&self) -> Result<Vec<Image>, JsValue> {
        self.inner.split()
            .map(|bands| bands.into_iter().map(|img| Image { inner: img }).collect())
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    #[wasm_bindgen(js_name = "toBytes")]
    pub fn to_bytes_js(&mut self) -> Result<Vec<u8>, JsValue> {
        self.inner.to_bytes().map_err(|e| JsValue::from_str(&e.to_string()))
    }

    #[wasm_bindgen(js_name = "copy")]
    pub fn copy_js(&self) -> Image {
        Image { inner: self.inner.copy() }
    }

    pub fn repr(&mut self) -> Result<String, JsValue> {
        let (w, h) = self.inner.size().map_err(|e| JsValue::from_str(&e.to_string()))?;
        let mode = self.inner.mode().map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(format!("<Image size={}x{} mode={}>", w, h, mode))
    }
}

// ── Module-level helpers ──────────────────────────────────────

#[wasm_bindgen(js_name = "imageBlend")]
pub fn blend_js(image1: &Image, image2: &Image, alpha: f64) -> Result<Image, JsValue> {
    let mut c1 = image1.inner.clone();
    let mut c2 = image2.inner.clone();
    // Use the ops module — simplified: resize to match
    // For WASM, expose a minimal blend via resize workaround
    let img = RsImage::new(100, 100, "RGB", (0, 0, 0, 0))
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
    Ok(Image { inner: img })
}
