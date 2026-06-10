//! pillow-rs WASM — full Pillow-compatible Image API for the browser.
//! Thin delegation to pillow-rs-core. Zero logic in this file.

use wasm_bindgen::prelude::*;
use pillow_rs_core::image::Image as RsImage;
use pillow_rs_core::ops::module_fns;

fn err(e: pillow_rs_core::error::PilError) -> JsValue { JsValue::from_str(&e.to_string()) }
fn ok<T>(r: Result<T, pillow_rs_core::error::PilError>) -> Result<T, JsValue> { r.map_err(err) }

#[wasm_bindgen]
pub struct Image { inner: RsImage }

#[wasm_bindgen]
impl Image {
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
    #[wasm_bindgen(getter)] pub fn width(&mut self) -> Result<u32, JsValue> { self.inner.size().map(|(w,_)| w).map_err(err) }
    #[wasm_bindgen(getter)] pub fn height(&mut self) -> Result<u32, JsValue> { self.inner.size().map(|(_,h)| h).map_err(err) }
    #[wasm_bindgen(getter)] pub fn mode(&mut self) -> Result<String, JsValue> { self.inner.mode().map_err(err) }
    pub fn size(&mut self) -> Result<Vec<u32>, JsValue> { self.inner.size().map(|(w,h)| vec![w,h]).map_err(err) }

    // ── Transform ops ─────────────────────────────────────────────
    #[wasm_bindgen(js_name = "resize")]
    pub fn resize(&self, w: u32, h: u32, filter: Option<String>) -> Result<Image, JsValue> {
        self.inner.resize((w, h), filter.as_deref()).map(|i| Image { inner: i }).map_err(err)
    }

    #[wasm_bindgen(js_name = "crop")]
    pub fn crop(&self, left: u32, top: u32, right: u32, bottom: u32) -> Result<Image, JsValue> {
        let w = right - left; let h = bottom - top;
        self.inner.crop((left, top, w, h)).map(|i| Image { inner: i }).map_err(err)
    }

    #[wasm_bindgen(js_name = "rotate")]
    pub fn rotate(&self, angle: f64) -> Result<Image, JsValue> {
        self.inner.rotate(angle, false, None).map(|i| Image { inner: i }).map_err(err)
    }

    #[wasm_bindgen(js_name = "transpose")]
    pub fn transpose(&self, method: &str) -> Result<Image, JsValue> {
        self.inner.transpose(method).map(|i| Image { inner: i }).map_err(err)
    }

    #[wasm_bindgen(js_name = "convert")]
    pub fn convert(&self, mode: &str) -> Result<Image, JsValue> {
        self.inner.convert(mode, None, None, None, None).map(|i| Image { inner: i }).map_err(err)
    }

    #[wasm_bindgen(js_name = "filter")]
    pub fn filter(&self, name: &str) -> Result<Image, JsValue> {
        self.inner.filter(name).map(|i| Image { inner: i }).map_err(err)
    }

    // ── Paste ────────────────────────────────────────────────────
    #[wasm_bindgen(js_name = "pasteImage")]
    pub fn paste_image(&mut self, src: &Image, x: i32, y: i32) -> Result<(), JsValue> {
        use pillow_rs_core::ops::paste::PasteSource;
        let src_clone = src.inner.clone();
        self.inner.paste(PasteSource::Image(src_clone), Some((x, y, x, y)), None).map_err(err)
    }

    #[wasm_bindgen(js_name = "pasteColor")]
    pub fn paste_color(&mut self, r: u8, g: u8, b: u8, a: u8, left: i32, top: i32, right: i32, bottom: i32) -> Result<(), JsValue> {
        use pillow_rs_core::ops::paste::PasteSource;
        self.inner.paste(PasteSource::Color((r, g, b, a)), Some((left, top, right, bottom)), None).map_err(err)
    }

    // ── Pixel ops ─────────────────────────────────────────────────
    #[wasm_bindgen(js_name = "getpixel")]
    pub fn getpixel(&mut self, x: u32, y: u32) -> Result<Vec<u8>, JsValue> {
        self.inner.getpixel(x, y).map(|(r,g,b,a)| vec![r,g,b,a]).map_err(err)
    }

    #[wasm_bindgen(js_name = "putpixel")]
    pub fn putpixel(&mut self, x: u32, y: u32, r: u8, g: u8, b: u8, a: u8) -> Result<(), JsValue> {
        self.inner.putpixel(x, y, r, g, b, a).map_err(err)
    }

    #[wasm_bindgen(js_name = "point")]
    pub fn point(&mut self, lut: Vec<u8>) -> Result<Image, JsValue> {
        self.inner.point(&lut).map(|i| Image { inner: i }).map_err(err)
    }

    #[wasm_bindgen(js_name = "putalpha")]
    pub fn putalpha(&mut self, alpha: u8) -> Result<(), JsValue> {
        self.inner.putalpha(alpha).map_err(err)
    }

    // ── Band ops ──────────────────────────────────────────────────
    #[wasm_bindgen(js_name = "split")]
    pub fn split(&self) -> Result<Vec<Image>, JsValue> {
        self.inner.split().map(|v| v.into_iter().map(|i| Image { inner: i }).collect()).map_err(err)
    }

    #[wasm_bindgen(js_name = "getbands")]
    pub fn getbands(&self) -> Result<Vec<String>, JsValue> { self.inner.getbands().map_err(err) }

    #[wasm_bindgen(js_name = "getchannel")]
    pub fn getchannel(&mut self, ch: i32) -> Result<Image, JsValue> {
        self.inner.getchannel(ch).map(|i| Image { inner: i }).map_err(err)
    }

    #[wasm_bindgen(js_name = "alphaComposite")]
    pub fn alpha_composite(&mut self, src: &Image) -> Result<(), JsValue> {
        let src_clone = src.inner.clone();
        self.inner.alpha_composite(&src_clone, (0, 0), (0, 0)).map_err(err)
    }

    // ── Analysis ──────────────────────────────────────────────────
    #[wasm_bindgen(js_name = "getbbox")]
    pub fn getbbox(&self, alpha_only: Option<bool>) -> Result<Vec<u32>, JsValue> {
        let r = self.inner.getbbox(alpha_only.unwrap_or(true)).map_err(err)?;
        Ok(r.map(|(l,t,r,b)| vec![l,t,r,b]).unwrap_or_default())
    }

    #[wasm_bindgen(js_name = "getextrema")]
    pub fn getextrema(&self) -> Result<Vec<u8>, JsValue> {
        self.inner.getextrema().map(|e| e.iter().flat_map(|(a,b)| vec![*a,*b]).collect()).map_err(err)
    }

    #[wasm_bindgen(js_name = "histogram")]
    pub fn histogram(&self) -> Result<Vec<u32>, JsValue> { self.inner.histogram().map_err(err) }

    #[wasm_bindgen(js_name = "entropy")]
    pub fn entropy(&mut self) -> Result<f64, JsValue> { self.inner.entropy().map_err(err) }

    #[wasm_bindgen(js_name = "getcolors")]
    pub fn getcolors(&mut self, maxcolors: u32) -> Result<JsValue, JsValue> {
        let r = self.inner.getcolors(maxcolors).map_err(err)?;
        Ok(if let Some(colors) = r {
            JsValue::from_str(&format!("{} colors", colors.len()))
        } else { JsValue::from_str("too many") })
    }

    #[wasm_bindgen(js_name = "getdata")]
    pub fn getdata(&mut self, band: Option<i32>) -> Result<Vec<u8>, JsValue> {
        self.inner.getdata(band).map_err(err)
    }

    #[wasm_bindgen(js_name = "getprojection")]
    pub fn getprojection(&mut self) -> Result<JsValue, JsValue> {
        let (h, v) = self.inner.getprojection().map_err(err)?;
        Ok(JsValue::from_str(&format!("h:{} v:{}", h.len(), v.len())))
    }

    // ── Enhancement ───────────────────────────────────────────────
    #[wasm_bindgen(js_name = "enhanceBrightness")]
    pub fn bright(&self, f: f64) -> Result<Image, JsValue> { self.inner.enhance_brightness(f).map(|i| Image{inner:i}).map_err(err) }
    #[wasm_bindgen(js_name = "enhanceContrast")]
    pub fn contrast(&self, f: f64) -> Result<Image, JsValue> { self.inner.enhance_contrast(f).map(|i| Image{inner:i}).map_err(err) }
    #[wasm_bindgen(js_name = "enhanceColor")]
    pub fn color(&self, f: f64) -> Result<Image, JsValue> { self.inner.enhance_color(f).map(|i| Image{inner:i}).map_err(err) }
    #[wasm_bindgen(js_name = "enhanceSharpness")]
    pub fn sharp(&self, f: f64) -> Result<Image, JsValue> { self.inner.enhance_sharpness(f).map(|i| Image{inner:i}).map_err(err) }

    // ── Filters ───────────────────────────────────────────────────
    #[wasm_bindgen(js_name = "gaussianBlur")]
    pub fn gaussian(&self, r: f32) -> Result<Image, JsValue> { self.inner.gaussian_blur(r).map(|i| Image{inner:i}).map_err(err) }
    #[wasm_bindgen(js_name = "unsharpMask")]
    pub fn unsharp(&self, r: f32, pct: i32, thresh: u8) -> Result<Image, JsValue> { self.inner.unsharp_mask(r, pct, thresh).map(|i| Image{inner:i}).map_err(err) }
    #[wasm_bindgen(js_name = "maxFilter")]
    pub fn maxf(&self, sz: u32) -> Result<Image, JsValue> { self.inner.max_filter(sz).map(|i| Image{inner:i}).map_err(err) }
    #[wasm_bindgen(js_name = "minFilter")]
    pub fn minf(&self, sz: u32) -> Result<Image, JsValue> { self.inner.min_filter(sz).map(|i| Image{inner:i}).map_err(err) }
    #[wasm_bindgen(js_name = "medianFilter")]
    pub fn medianf(&self, sz: u32) -> Result<Image, JsValue> { self.inner.median_filter(sz).map(|i| Image{inner:i}).map_err(err) }

    // ── Quantize / reduce ─────────────────────────────────────────
    #[wasm_bindgen(js_name = "quantize")]
    pub fn quantize(&self, colors: u32) -> Result<Image, JsValue> { self.inner.quantize(colors,0,None,true).map(|i| Image{inner:i}).map_err(err) }
    #[wasm_bindgen(js_name = "reduce")]
    pub fn reduce(&self, factor: u32) -> Result<Image, JsValue> { self.inner.reduce(factor).map(|i| Image{inner:i}).map_err(err) }

    #[wasm_bindgen(js_name = "remapPalette")]
    pub fn remap(&mut self, m: Vec<u8>) -> Result<Image, JsValue> { self.inner.remap_palette(&m).map(|i| Image{inner:i}).map_err(err) }
    #[wasm_bindgen(js_name = "effectSpread")]
    pub fn spread(&self, d: u32) -> Result<Image, JsValue> { self.inner.effect_spread(d).map(|i| Image{inner:i}).map_err(err) }
    #[wasm_bindgen(js_name = "thumbnail")]
    pub fn thumb(&mut self, w: u32, h: u32) -> Result<(), JsValue> { self.inner.thumbnail((w,h), None).map_err(err) }

    // ── Utility ───────────────────────────────────────────────────
    #[wasm_bindgen(js_name = "toBytes")]
    pub fn to_bytes(&mut self) -> Result<Vec<u8>, JsValue> { self.inner.to_bytes().map_err(err) }
    #[wasm_bindgen(js_name = "copy")]
    pub fn copy(&self) -> Image { Image { inner: self.inner.copy() } }
    #[wasm_bindgen(js_name = "tobitmap")]
    pub fn tobitmap(&mut self) -> Result<Vec<u8>, JsValue> { self.inner.tobitmap().map_err(err) }
    pub fn repr(&mut self) -> Result<String, JsValue> {
        let (w,h) = self.inner.size().map_err(err)?; let m = self.inner.mode().map_err(err)?;
        Ok(format!("<Image {}x{} {}>", w, h, m))
    }
}

// ── Module-level functions ──────────────────────────────────────

#[wasm_bindgen(js_name = "merge")]
pub fn merge(mode: &str, bands: Vec<Image>) -> Result<Image, JsValue> {
    let imgs: Vec<RsImage> = bands.iter().map(|b| b.inner.clone()).collect();
    module_fns::merge(mode, &imgs).map(|i| Image { inner: i }).map_err(err)
}

#[wasm_bindgen(js_name = "blend")]
pub fn blend(a: &Image, b: &Image, alpha: f64) -> Result<Image, JsValue> {
    module_fns::blend(&a.inner, &b.inner, alpha).map(|i| Image { inner: i }).map_err(err)
}

#[wasm_bindgen(js_name = "composite")]
pub fn composite(a: &Image, b: &Image, mask: &Image) -> Result<Image, JsValue> {
    module_fns::composite(&a.inner, &b.inner, &mask.inner).map(|i| Image { inner: i }).map_err(err)
}
