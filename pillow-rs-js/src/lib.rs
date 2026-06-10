//! pillow-rs WASM bindings — full Pillow-compatible Image API for the browser.
//! Pure delegation to core — zero logic in this file.
//! JS-friendly method names via #[wasm_bindgen(js_name = "...")].

use wasm_bindgen::prelude::*;
use pillow_rs_core::image::Image as RsImage;

#[wasm_bindgen]
pub struct Image { inner: RsImage }

// ── Helper to map errors ────────────────────────────────────────
fn err(e: pillow_rs_core::error::PilError) -> JsValue { JsValue::from_str(&e.to_string()) }

#[wasm_bindgen]
impl Image {
    // ── Constructors ──────────────────────────────────────────────
    #[wasm_bindgen(constructor)]
    pub fn new(mode: &str, w: u32, h: u32, r: u8, g: u8, b: u8, a: u8) -> Result<Image, JsValue> {
        console_error_panic_hook::set_once();
        RsImage::new(w, h, mode, (r, g, b, a)).map(|i| Image { inner: i }).map_err(err)
    }

    #[wasm_bindgen(js_name = "open")]
    pub fn open(data: Vec<u8>) -> Result<Image, JsValue> {
        RsImage::open_bytes(data).map(|i| Image { inner: i }).map_err(err)
    }

    // ── Properties ────────────────────────────────────────────────
    #[wasm_bindgen(getter)] pub fn width(&mut self) -> Result<u32, JsValue> { let (w, _) = self.inner.size().map_err(err)?; Ok(w) }
    #[wasm_bindgen(getter)] pub fn height(&mut self) -> Result<u32, JsValue> { let (_, h) = self.inner.size().map_err(err)?; Ok(h) }
    #[wasm_bindgen(getter)] pub fn mode(&mut self) -> Result<String, JsValue> { self.inner.mode().map_err(err) }
    pub fn size(&mut self) -> Result<Vec<u32>, JsValue> { self.inner.size().map(|(w,h)| vec![w,h]).map_err(err) }

    // ── Core operations ───────────────────────────────────────────
    #[wasm_bindgen(js_name = "resize")]
    pub fn resize_js(&self, w: u32, h: u32, filter: Option<String>) -> Result<Image, JsValue> {
        self.inner.resize((w, h), filter.as_deref()).map(|i| Image { inner: i }).map_err(err)
    }

    #[wasm_bindgen(js_name = "crop")]
    pub fn crop_js(&self, x: u32, y: u32, w: u32, h: u32) -> Result<Image, JsValue> {
        self.inner.crop((x, y, w, h)).map(|i| Image { inner: i }).map_err(err)
    }

    #[wasm_bindgen(js_name = "rotate")]
    pub fn rotate_js(&self, angle: f64) -> Result<Image, JsValue> {
        self.inner.rotate(angle, false, None).map(|i| Image { inner: i }).map_err(err)
    }

    #[wasm_bindgen(js_name = "transpose")]
    pub fn transpose_js(&self, method: &str) -> Result<Image, JsValue> {
        self.inner.transpose(method).map(|i| Image { inner: i }).map_err(err)
    }

    #[wasm_bindgen(js_name = "convert")]
    pub fn convert_js(&self, mode: &str) -> Result<Image, JsValue> {
        self.inner.convert(mode, None, None, None, None).map(|i| Image { inner: i }).map_err(err)
    }

    #[wasm_bindgen(js_name = "filter")]
    pub fn filter_js(&self, name: &str) -> Result<Image, JsValue> {
        self.inner.filter(name).map(|i| Image { inner: i }).map_err(err)
    }

    #[wasm_bindgen(js_name = "quantize")]
    pub fn quantize_js(&self, colors: u32) -> Result<Image, JsValue> {
        self.inner.quantize(colors, 0, None, true).map(|i| Image { inner: i }).map_err(err)
    }

    // ── Pixel access ──────────────────────────────────────────────
    #[wasm_bindgen(js_name = "getpixel")]
    pub fn getpixel_js(&mut self, x: u32, y: u32) -> Result<Vec<u8>, JsValue> {
        self.inner.getpixel(x, y).map(|(r,g,b,a)| vec![r,g,b,a]).map_err(err)
    }

    #[wasm_bindgen(js_name = "putpixel")]
    pub fn putpixel_js(&mut self, x: u32, y: u32, r: u8, g: u8, b: u8, a: u8) -> Result<(), JsValue> {
        self.inner.putpixel(x, y, r, g, b, a).map_err(err)
    }

    // ── Band operations ───────────────────────────────────────────
    #[wasm_bindgen(js_name = "split")]
    pub fn split_js(&self) -> Result<Vec<Image>, JsValue> {
        self.inner.split().map(|v| v.into_iter().map(|i| Image { inner: i }).collect()).map_err(err)
    }

    #[wasm_bindgen(js_name = "getbands")]
    pub fn getbands_js(&self) -> Result<Vec<String>, JsValue> {
        self.inner.getbands().map_err(err)
    }

    #[wasm_bindgen(js_name = "getchannel")]
    pub fn getchannel_js(&mut self, channel: i32) -> Result<Image, JsValue> {
        self.inner.getchannel(channel).map(|i| Image { inner: i }).map_err(err)
    }

    // ── Analysis ──────────────────────────────────────────────────
    #[wasm_bindgen(js_name = "getbbox")]
    pub fn getbbox_js(&self, alpha_only: Option<bool>) -> Result<Vec<u32>, JsValue> {
        let result = self.inner.getbbox(alpha_only.unwrap_or(true)).map_err(err)?;
        Ok(match result {
            Some((l,t,r,b)) => vec![l, t, r, b],
            None => vec![],
        })
    }

    #[wasm_bindgen(js_name = "getextrema")]
    pub fn getextrema_js(&self) -> Result<Vec<u8>, JsValue> {
        let ext = self.inner.getextrema().map_err(err)?;
        Ok(ext.iter().flat_map(|(a,b)| vec![*a,*b]).collect())
    }

    #[wasm_bindgen(js_name = "histogram")]
    pub fn histogram_js(&self) -> Result<Vec<u32>, JsValue> {
        self.inner.histogram().map_err(err)
    }

    #[wasm_bindgen(js_name = "entropy")]
    pub fn entropy_js(&mut self) -> Result<f64, JsValue> {
        self.inner.entropy().map_err(err)
    }

    // ── Enhancement ───────────────────────────────────────────────
    #[wasm_bindgen(js_name = "enhanceBrightness")]
    pub fn enhance_brightness_js(&self, factor: f64) -> Result<Image, JsValue> {
        self.inner.enhance_brightness(factor).map(|i| Image { inner: i }).map_err(err)
    }

    #[wasm_bindgen(js_name = "enhanceContrast")]
    pub fn enhance_contrast_js(&self, factor: f64) -> Result<Image, JsValue> {
        self.inner.enhance_contrast(factor).map(|i| Image { inner: i }).map_err(err)
    }

    #[wasm_bindgen(js_name = "enhanceColor")]
    pub fn enhance_color_js(&self, factor: f64) -> Result<Image, JsValue> {
        self.inner.enhance_color(factor).map(|i| Image { inner: i }).map_err(err)
    }

    #[wasm_bindgen(js_name = "enhanceSharpness")]
    pub fn enhance_sharpness_js(&self, factor: f64) -> Result<Image, JsValue> {
        self.inner.enhance_sharpness(factor).map(|i| Image { inner: i }).map_err(err)
    }

    // ── Utility ───────────────────────────────────────────────────
    #[wasm_bindgen(js_name = "toBytes")]
    pub fn to_bytes_js(&mut self) -> Result<Vec<u8>, JsValue> { self.inner.to_bytes().map_err(err) }

    #[wasm_bindgen(js_name = "copy")]
    pub fn copy_js(&self) -> Image { Image { inner: self.inner.copy() } }

    #[wasm_bindgen(js_name = "reduce")]
    pub fn reduce_js(&self, factor: u32) -> Result<Image, JsValue> {
        self.inner.reduce(factor).map(|i| Image { inner: i }).map_err(err)
    }

    #[wasm_bindgen(js_name = "putalpha")]
    pub fn putalpha_js(&mut self, alpha: u8) -> Result<(), JsValue> { self.inner.putalpha(alpha).map_err(err) }

    pub fn repr(&mut self) -> Result<String, JsValue> {
        let (w, h) = self.inner.size().map_err(err)?;
        let m = self.inner.mode().map_err(err)?;
        Ok(format!("<Image size={}x{} mode={}>", w, h, m))
    }
}

// ── Module-level functions (JS-side wrappers, thin delegation) ──
// Complex module fns (blend, composite, ops) are implemented in JS wrapper.
// WASM exposes only the Image class; JS builds higher-level API on top.
