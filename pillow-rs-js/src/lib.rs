// AS PER DESIGN — DO NOT REMOVE: Deferred lint cleanup. See CODEBASE_AUDIT.md Fix 2.
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_in_result)]
#![allow(clippy::map_unwrap_or)]
#![allow(clippy::redundant_clone)]
// WASM binding conventions differ from standard Rust naming
#![allow(non_snake_case)]
#![allow(dead_code)]
#![allow(unused_variables)]

//! pillow-rs WASM — full Pillow API for the browser. Thin delegation to pillow-rs.
use pillow_rs::Image as RsImage;
use wasm_bindgen::prelude::JsValue;
use wasm_bindgen::prelude::wasm_bindgen;

fn err(e: pillow_rs::PilError) -> JsValue {
    let name = match &e {
        pillow_rs::PilError::IOError(_)
        | pillow_rs::PilError::OsError(_)
        | pillow_rs::PilError::Io(_) => "OSError",
        pillow_rs::PilError::AssertionError(_) => "AssertionError",
        pillow_rs::PilError::IndexError(_) => "IndexError",
        pillow_rs::PilError::KeyError(_) | pillow_rs::PilError::UnsupportedLibraqm(_) => "KeyError",
        pillow_rs::PilError::ValueError(_) => "ValueError",
        pillow_rs::PilError::TypeError(_) => "TypeError",
        pillow_rs::PilError::SystemError(_) => "SystemError",
        pillow_rs::PilError::SyntaxError(_) => "SyntaxError",
        pillow_rs::PilError::NotImplementedError(_) => "NotImplementedError",
        pillow_rs::PilError::UnidentifiedImageError(_) => "UnidentifiedImageError",
        _ => "Error",
    };
    let error = js_sys::Error::new(&e.to_string());
    error.set_name(name);
    error.into()
}

#[wasm_bindgen]
pub struct Image {
    inner: RsImage,
}

#[wasm_bindgen]
pub struct ArrayDescriptorLayout {
    mode: String,
    raw_mode: String,
    width: usize,
    height: usize,
    dimensions: usize,
    mode_reinterprets_dtype: bool,
}

#[wasm_bindgen]
impl ArrayDescriptorLayout {
    #[wasm_bindgen(getter)]
    pub fn mode(&self) -> String {
        self.mode.clone()
    }

    #[wasm_bindgen(getter, js_name = "rawMode")]
    pub fn raw_mode(&self) -> String {
        self.raw_mode.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn width(&self) -> usize {
        self.width
    }

    #[wasm_bindgen(getter)]
    pub fn height(&self) -> usize {
        self.height
    }

    #[wasm_bindgen(getter)]
    pub fn dimensions(&self) -> usize {
        self.dimensions
    }

    #[wasm_bindgen(getter, js_name = "modeReinterpretsDtype")]
    pub fn mode_reinterprets_dtype(&self) -> bool {
        self.mode_reinterprets_dtype
    }
}

#[wasm_bindgen(js_name = "resolveArrayLayout")]
pub fn resolve_array_layout(
    shape: Vec<usize>,
    typestr: &str,
    mode: Option<String>,
) -> Result<ArrayDescriptorLayout, JsValue> {
    let layout = pillow_rs::resolve_array_layout(&shape, typestr, mode.as_deref()).map_err(err)?;
    Ok(ArrayDescriptorLayout {
        mode: layout.mode,
        raw_mode: layout.raw_mode,
        width: layout.width,
        height: layout.height,
        dimensions: layout.dimensions,
        mode_reinterprets_dtype: layout.mode_reinterprets_dtype,
    })
}

#[wasm_bindgen]
impl Image {
    #[wasm_bindgen(constructor)]
    pub fn new(mode: &str, w: u32, h: u32, r: u8, g: u8, b: u8, a: u8) -> Result<Image, JsValue> {
        #[cfg(feature = "debug-hooks")]
        console_error_panic_hook::set_once();
        // Initialize console_log with a conservative default (Warn).
        // Users can change the level at runtime via setLogLevel().
        #[cfg(feature = "debug-hooks")]
        console_log::init_with_level(log::Level::Warn).ok();
        RsImage::new(w, h, mode, (r, g, b, a))
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "open")]
    pub fn open(data: Vec<u8>) -> Result<Image, JsValue> {
        RsImage::open_bytes(data)
            .map(|i| Image { inner: i })
            .map_err(err)
    }

    // Properties
    #[wasm_bindgen(getter)]
    pub fn width(&mut self) -> Result<u32, JsValue> {
        self.inner.size().map(|(w, _)| w).map_err(err)
    }
    #[wasm_bindgen(getter)]
    pub fn height(&mut self) -> Result<u32, JsValue> {
        self.inner.size().map(|(_, h)| h).map_err(err)
    }
    #[wasm_bindgen(getter)]
    pub fn mode(&mut self) -> Result<String, JsValue> {
        self.inner.mode().map_err(err)
    }
    pub fn size(&mut self) -> Result<Vec<u32>, JsValue> {
        self.inner.size().map(|(w, h)| vec![w, h]).map_err(err)
    }

    // Transforms
    #[wasm_bindgen(js_name = "resize")]
    pub fn resize(&self, w: u32, h: u32, f: Option<String>) -> Result<Image, JsValue> {
        self.inner
            .resize((w, h), f.as_deref())
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "crop")]
    pub fn crop(&self, l: u32, t: u32, r: u32, b: u32) -> Result<Image, JsValue> {
        self.inner
            .crop_box(l, t, r, b)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "rotate")]
    pub fn rotate(&self, a: f64) -> Result<Image, JsValue> {
        self.inner
            .rotate(a, false, None)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "transpose")]
    pub fn transpose(&self, m: &str) -> Result<Image, JsValue> {
        self.inner
            .transpose(m)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "convert")]
    pub fn convert(&self, m: &str, dither: Option<String>) -> Result<Image, JsValue> {
        self.inner
            .convert(m, None, dither.as_deref(), None, None)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "filter")]
    pub fn filter(&self, n: &str) -> Result<Image, JsValue> {
        self.inner
            .filter(n)
            .map(|i| Image { inner: i })
            .map_err(err)
    }

    // Paste
    #[wasm_bindgen(js_name = "pasteImage")]
    pub fn paste_image(&mut self, src: &Image, x: i32, y: i32) -> Result<(), JsValue> {
        use pillow_rs::PasteSource;
        self.inner
            .paste_at(PasteSource::Image(src.inner.clone()), Some((x, y)), None)
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "pasteImageMasked")]
    pub fn paste_image_masked(
        &mut self,
        src: &Image,
        x: i32,
        y: i32,
        mask: &Image,
    ) -> Result<(), JsValue> {
        use pillow_rs::PasteSource;
        self.inner
            .paste_at(
                PasteSource::Image(src.inner.clone()),
                Some((x, y)),
                Some(&mask.inner),
            )
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "pasteImageRegion")]
    pub fn paste_image_region(
        &mut self,
        src: &Image,
        l: i32,
        t: i32,
        r: i32,
        b: i32,
    ) -> Result<(), JsValue> {
        use pillow_rs::PasteSource;
        self.inner
            .paste(
                PasteSource::Image(src.inner.clone()),
                Some((l, t, r, b)),
                None,
            )
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "pasteImageRegionMasked")]
    pub fn paste_image_region_masked(
        &mut self,
        src: &Image,
        l: i32,
        t: i32,
        r: i32,
        b: i32,
        mask: &Image,
    ) -> Result<(), JsValue> {
        use pillow_rs::PasteSource;
        self.inner
            .paste(
                PasteSource::Image(src.inner.clone()),
                Some((l, t, r, b)),
                Some(&mask.inner),
            )
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "pasteColor")]
    pub fn paste_color(
        &mut self,
        r: u8,
        g: u8,
        b: u8,
        a: u8,
        l: i32,
        t: i32,
        rt: i32,
        bt: i32,
    ) -> Result<(), JsValue> {
        use pillow_rs::PasteSource;
        self.inner
            .paste(PasteSource::Rgba(r, g, b, a), Some((l, t, rt, bt)), None)
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "pasteScalarRegion")]
    pub fn paste_scalar_region(
        &mut self,
        value: u8,
        l: i32,
        t: i32,
        r: i32,
        b: i32,
    ) -> Result<(), JsValue> {
        use pillow_rs::PasteSource;
        self.inner
            .paste(PasteSource::Scalar(value), Some((l, t, r, b)), None)
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "pasteScalarAt")]
    pub fn paste_scalar_at(&mut self, value: u8, x: i32, y: i32) -> Result<(), JsValue> {
        use pillow_rs::PasteSource;
        self.inner
            .paste_at(PasteSource::Scalar(value), Some((x, y)), None)
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "pasteLumaAlphaRegion")]
    pub fn paste_luma_alpha_region(
        &mut self,
        luma: u8,
        alpha: u8,
        l: i32,
        t: i32,
        r: i32,
        b: i32,
    ) -> Result<(), JsValue> {
        use pillow_rs::PasteSource;
        self.inner
            .paste(
                PasteSource::LumaAlpha(luma, alpha),
                Some((l, t, r, b)),
                None,
            )
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "pasteRgbAt")]
    pub fn paste_rgb_at(
        &mut self,
        r: u8,
        g: u8,
        b: u8,
        x: i32,
        y: i32,
        mask: &Image,
    ) -> Result<(), JsValue> {
        use pillow_rs::PasteSource;
        self.inner
            .paste_at(PasteSource::Rgb(r, g, b), Some((x, y)), Some(&mask.inner))
            .map_err(err)
    }

    // Pixels
    #[wasm_bindgen(js_name = "getpixel")]
    pub fn getpixel(&mut self, x: u32, y: u32) -> Result<Vec<u8>, JsValue> {
        self.inner
            .getpixel(x, y)
            .map(|(r, g, b, a)| vec![r, g, b, a])
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "putpixel")]
    pub fn putpixel(&mut self, x: u32, y: u32, r: u8, g: u8, b: u8, a: u8) -> Result<(), JsValue> {
        self.inner.putpixel(x, y, r, g, b, a).map_err(err)
    }
    #[wasm_bindgen(js_name = "point")]
    pub fn point(&self, lut: Vec<u8>) -> Result<Image, JsValue> {
        pillow_rs::image_eval(&self.inner, &lut)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "putalpha")]
    pub fn putalpha(&mut self, a: u8) -> Result<(), JsValue> {
        self.inner.putalpha(a).map_err(err)
    }

    // Bands
    #[wasm_bindgen(js_name = "split")]
    pub fn split(&self) -> Result<Vec<Image>, JsValue> {
        self.inner
            .split()
            .map(|v| v.into_iter().map(|i| Image { inner: i }).collect())
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "getbands")]
    pub fn getbands(&self) -> Result<Vec<String>, JsValue> {
        self.inner.getbands().map_err(err)
    }
    #[wasm_bindgen(js_name = "getchannel")]
    pub fn getchannel(&mut self, ch: i32) -> Result<Image, JsValue> {
        self.inner
            .getchannel(ch)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "alphaComposite")]
    pub fn alpha_composite(&mut self, src: &Image) -> Result<(), JsValue> {
        self.inner
            .alpha_composite(&src.inner, (0, 0), (0, 0))
            .map_err(err)
    }

    // Analysis
    #[wasm_bindgen(js_name = "getbbox")]
    pub fn getbbox(&self, a: Option<bool>) -> Result<Vec<u32>, JsValue> {
        self.inner
            .getbbox(a.unwrap_or(true))
            .map(|r| r.map(|(l, t, r, b)| vec![l, t, r, b]).unwrap_or_default())
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "getextrema")]
    pub fn getextrema(&self) -> Result<js_sys::Array, JsValue> {
        let extrema = self.inner.getextrema().map_err(err)?;
        let arr = js_sys::Array::new();
        for (a, b) in &extrema {
            let pair = js_sys::Array::new();
            pair.push(&JsValue::from(*a));
            pair.push(&JsValue::from(*b));
            arr.push(&pair);
        }
        Ok(arr)
    }
    #[wasm_bindgen(js_name = "histogram")]
    pub fn histogram(&self) -> Result<Vec<u32>, JsValue> {
        self.inner.histogram().map_err(err)
    }
    #[wasm_bindgen(js_name = "entropy")]
    pub fn entropy(&mut self) -> Result<f64, JsValue> {
        self.inner.entropy().map_err(err)
    }
    #[wasm_bindgen(js_name = "getcolors")]
    pub fn getcolors(&mut self, m: u32) -> Result<JsValue, JsValue> {
        match self.inner.getcolors(m).map_err(err)? {
            Some(colors) => {
                let arr = js_sys::Array::new();
                for (count, color_bytes) in &colors {
                    let entry = js_sys::Array::new();
                    entry.push(&JsValue::from(*count));
                    let color_arr = js_sys::Array::new();
                    for b in color_bytes {
                        color_arr.push(&JsValue::from(*b));
                    }
                    entry.push(&color_arr);
                    arr.push(&entry);
                }
                Ok(arr.into())
            }
            None => Ok(JsValue::null()),
        }
    }
    #[wasm_bindgen(js_name = "getdata")]
    pub fn getdata(&mut self, b: Option<i32>) -> Result<Vec<u8>, JsValue> {
        self.inner.getdata(b).map_err(err)
    }
    #[wasm_bindgen(js_name = "getprojection")]
    pub fn getprojection(&mut self) -> Result<js_sys::Array, JsValue> {
        let (h_proj, v_proj) = self.inner.getprojection().map_err(err)?;
        let h_arr = js_sys::Array::new();
        for val in &h_proj {
            h_arr.push(&JsValue::from(*val));
        }
        let v_arr = js_sys::Array::new();
        for val in &v_proj {
            v_arr.push(&JsValue::from(*val));
        }
        let result = js_sys::Array::new();
        result.push(&h_arr);
        result.push(&v_arr);
        Ok(result)
    }

    // Enhancement
    #[wasm_bindgen(js_name = "enhanceBrightness")]
    pub fn bright(&self, f: f64) -> Result<Image, JsValue> {
        self.inner
            .enhance_brightness(f)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "enhanceContrast")]
    pub fn contrast(&self, f: f64) -> Result<Image, JsValue> {
        self.inner
            .enhance_contrast(f)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "enhanceColor")]
    pub fn color(&self, f: f64) -> Result<Image, JsValue> {
        self.inner
            .enhance_color(f)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "enhanceSharpness")]
    pub fn sharp(&self, f: f64) -> Result<Image, JsValue> {
        self.inner
            .enhance_sharpness(f)
            .map(|i| Image { inner: i })
            .map_err(err)
    }

    // Filters
    #[wasm_bindgen(js_name = "gaussianBlur")]
    pub fn gaussian(&self, r: f32) -> Result<Image, JsValue> {
        self.inner
            .gaussian_blur(r)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "boxBlur")]
    pub fn boxb(&self, r: f32) -> Result<Image, JsValue> {
        self.inner
            .box_blur(r)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "unsharpMask")]
    pub fn unsharp(&self, r: f32, p: i32, t: u8) -> Result<Image, JsValue> {
        self.inner
            .unsharp_mask(r, p, t)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "maxFilter")]
    pub fn maxf(&self, s: u32) -> Result<Image, JsValue> {
        self.inner
            .max_filter(s)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "minFilter")]
    pub fn minf(&self, s: u32) -> Result<Image, JsValue> {
        self.inner
            .min_filter(s)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "medianFilter")]
    pub fn medianf(&self, s: u32) -> Result<Image, JsValue> {
        self.inner
            .median_filter(s)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "modeFilter")]
    pub fn modef(&self, s: u32) -> Result<Image, JsValue> {
        self.inner
            .mode_filter(s)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "rankFilter")]
    pub fn rankf(&self, s: u32, r: u32) -> Result<Image, JsValue> {
        self.inner
            .rank_filter(s, r)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "kernelFilter")]
    pub fn kernelf(
        &self,
        kernel: Vec<f32>,
        scale: f32,
        offset: i32,
        size: u32,
    ) -> Result<Image, JsValue> {
        self.inner
            .kernel_filter(&kernel, scale, offset, size)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "color3DLUT")]
    pub fn color3dlut(
        &self,
        size_x: u32,
        size_y: u32,
        size_z: u32,
        table: Vec<f64>,
        channels: u32,
        target_mode: Option<String>,
    ) -> Result<Image, JsValue> {
        self.inner
            .color3dlut(
                (size_x, size_y, size_z),
                table,
                channels,
                target_mode.as_deref(),
            )
            .map(|i| Image { inner: i })
            .map_err(err)
    }

    // Quantize/Reduce
    #[wasm_bindgen(js_name = "quantize")]
    pub fn quantize(&self, c: u32) -> Result<Image, JsValue> {
        self.inner
            .quantize(c, 0, None, true)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "reduce")]
    pub fn reduce(&self, f: u32) -> Result<Image, JsValue> {
        self.inner
            .reduce(f)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "remapPalette")]
    pub fn remap(&mut self, m: Vec<u8>) -> Result<Image, JsValue> {
        self.inner
            .remap_palette(&m)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "effectSpread")]
    pub fn spread(&self, d: u32) -> Result<Image, JsValue> {
        pillow_rs::image_effect_spread(&self.inner, d)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "effectNoise")]
    pub fn noise(&self, sigma: f64) -> Result<Image, JsValue> {
        pillow_rs::image_effect_noise(&self.inner, sigma)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "eval")]
    pub fn eval(&self, lut: Vec<u8>) -> Result<Image, JsValue> {
        pillow_rs::image_eval_replicated_for_image(&self.inner, &lut)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "thumbnail")]
    pub fn thumb(&mut self, w: u32, h: u32) -> Result<(), JsValue> {
        self.inner.thumbnail((w, h), None).map_err(err)
    }

    // Bookkeeping
    #[wasm_bindgen(js_name = "seek")]
    pub fn seek(&mut self, f: u32) -> Result<(), JsValue> {
        self.inner.seek(f).map_err(err)
    }
    #[wasm_bindgen(js_name = "tell")]
    pub fn tell_js(&self) -> u32 {
        self.inner.tell()
    }
    #[wasm_bindgen(js_name = "load")]
    pub fn load(&mut self) -> Result<(), JsValue> {
        self.inner.load().map_err(err)
    }
    #[wasm_bindgen(js_name = "verify")]
    pub fn verify(&self) -> Result<(), JsValue> {
        self.inner.verify().map_err(err)
    }
    #[wasm_bindgen(js_name = "fromBytes")]
    pub fn frombytes(&self, m: &str, w: u32, h: u32, d: Vec<u8>) -> Result<Image, JsValue> {
        RsImage::frombytes(m, (w, h), &d)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "putdata")]
    pub fn putdata(&mut self, d: Vec<u8>) -> Result<(), JsValue> {
        self.inner.putdata(&d).map_err(err)
    }
    #[wasm_bindgen(js_name = "transform")]
    pub fn transform(&self, sz: Vec<u32>, d: Vec<f64>) -> Result<Image, JsValue> {
        self.inner
            .transform_affine((sz[0], sz[1]), &d, (0, 0, 0, 255))
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "toBytes")]
    pub fn tobytes(&self) -> Result<Vec<u8>, JsValue> {
        self.inner.tobytes().map_err(err)
    }
    #[wasm_bindgen(js_name = "toBytesEncoded")]
    pub fn tobytes_encoded(
        &self,
        encoder_name: &str,
        args: Vec<String>,
    ) -> Result<Vec<u8>, JsValue> {
        let mode = self.inner.mode().map_err(err)?;
        self.inner
            .tobytes_encoded(&mode, encoder_name, &args)
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "copy")]
    pub fn copy(&self) -> Image {
        Image {
            inner: self.inner.copy(),
        }
    }
    #[wasm_bindgen(js_name = "tobitmap")]
    pub fn tobitmap(&mut self) -> Result<Vec<u8>, JsValue> {
        self.inner.tobitmap().map_err(err)
    }
    // More methods
    #[wasm_bindgen(js_name = "getpalette")]
    pub fn getpalette(&self) -> Result<Vec<u8>, JsValue> {
        self.inner
            .palette()
            .ok_or_else(|| JsValue::from_str("no palette"))
    }
    #[wasm_bindgen(js_name = "putpalette")]
    pub fn putpalette(&mut self, data: Vec<u8>, rawmode: Option<String>) -> Result<(), JsValue> {
        self.inner
            .putpalette(&data, rawmode.as_deref().unwrap_or("RGB"))
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "getexif")]
    pub fn getexif(&self) -> JsValue {
        JsValue::from_str("{}")
    }
    #[wasm_bindgen(js_name = "getxmp")]
    pub fn getxmp(&self) -> JsValue {
        JsValue::from_str("{}")
    }
    #[wasm_bindgen(js_name = "getChildImages")]
    pub fn get_child_images(&self) -> Vec<Image> {
        vec![]
    }
    #[wasm_bindgen(js_name = "getFlattenedData")]
    pub fn get_flattened(&self) -> Result<Vec<u8>, JsValue> {
        self.inner.tobytes().map_err(err)
    }
    #[wasm_bindgen(js_name = "applyTransparency")]
    pub fn apply_transparency(&mut self) -> Result<(), JsValue> {
        self.inner.apply_transparency().map_err(err)
    }
    #[wasm_bindgen(js_name = "paletteMode")]
    pub fn palette_mode(&self) -> Option<String> {
        self.inner.palette_mode().map(str::to_owned)
    }
    #[wasm_bindgen(js_name = "paletteRgba")]
    pub fn palette_rgba(&self) -> Option<Vec<u8>> {
        self.inner.getpalette_rgba()
    }
    #[wasm_bindgen(js_name = "pendingTransparencyIndex")]
    pub fn pending_transparency_index(&self) -> Option<u8> {
        match self.inner.pending_palette_transparency() {
            Some(pillow_rs::PaletteTransparency::Index(index)) => Some(index),
            _ => None,
        }
    }
    #[wasm_bindgen(js_name = "pendingTransparencyTable")]
    pub fn pending_transparency_table(&self) -> Option<Vec<u8>> {
        match self.inner.pending_palette_transparency() {
            Some(pillow_rs::PaletteTransparency::Table(alpha)) => Some(alpha),
            _ => None,
        }
    }
    #[wasm_bindgen(js_name = "hasTransparencyData")]
    pub fn has_transparency_data(&self) -> bool {
        self.inner.has_transparency_data()
    }
    #[wasm_bindgen(js_name = "draft")]
    pub fn draft(&self) -> Image {
        Image {
            inner: self.inner.clone(),
        }
    }
    #[wasm_bindgen(js_name = "putpixelRaw")]
    pub fn putpixel_raw(
        &mut self,
        x: u32,
        y: u32,
        r: u8,
        g: u8,
        b: u8,
        a: u8,
    ) -> Result<(), JsValue> {
        self.inner.putpixel(x, y, r, g, b, a).map_err(err)
    }

    pub fn repr(&mut self) -> Result<String, JsValue> {
        let (w, h) = self.inner.size().map_err(err)?;
        let m = self.inner.mode().map_err(err)?;
        Ok(format!("<Image {}x{} {}>", w, h, m))
    }
}

// ── ImageDraw ────────────────────────────────────────────────────
use pillow_rs::Draw;

#[wasm_bindgen]
pub struct ImageDraw {
    draw: Draw,
}

#[wasm_bindgen]
impl ImageDraw {
    #[wasm_bindgen(constructor)]
    pub fn new(img: &Image) -> Result<ImageDraw, JsValue> {
        let mode = img.inner.mode().map_err(err)?;
        Ok(ImageDraw {
            draw: Draw::new(img.inner.clone(), Some(mode)),
        })
    }

    #[wasm_bindgen(js_name = "line")]
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
    #[wasm_bindgen(js_name = "rectangle")]
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
        let pts: Vec<(i32, i32)> = points.chunks(2).map(|c| (c[0], c[1])).collect();
        let fill = fr.map(|r| (r, fg.unwrap_or(0), fb.unwrap_or(0), fa.unwrap_or(255)));
        let out = or.map(|r| (r, og.unwrap_or(0), ob.unwrap_or(0), oa.unwrap_or(255)));
        self.draw
            .polygon(&pts, fill, out, width.unwrap_or(1))
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "point")]
    pub fn point(&mut self, pts: Vec<i32>, r: u8, g: u8, b: u8, a: u8) -> Result<(), JsValue> {
        let pp: Vec<(i32, i32)> = pts.chunks(2).map(|c| (c[0], c[1])).collect();
        self.draw.point(&pp, (r, g, b, a)).map_err(err)
    }
    #[wasm_bindgen(js_name = "arc")]
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
    #[wasm_bindgen(js_name = "bitmap")]
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
    #[wasm_bindgen(getter)]
    pub fn image(&self) -> Result<Image, JsValue> {
        // Core image_clone() already handles mode preservation
        // (RGB→RGB, RGBA→RGBA, L→L, etc.)
        Ok(Image {
            inner: self.draw.image_clone().map_err(err)?,
        })
    }
}

// ── ImageFont ────────────────────────────────────────────────────
use pillow_rs::ImageFont as RsImageFont;

#[wasm_bindgen]
pub struct ImageFont {
    font: RsImageFont,
}

#[wasm_bindgen]
pub struct ImageFontMask {
    width: u32,
    height: u32,
    offset_x: i32,
    offset_y: i32,
    pixels: Vec<u8>,
}

#[wasm_bindgen]
impl ImageFontMask {
    #[wasm_bindgen(getter)]
    pub fn width(&self) -> u32 {
        self.width
    }

    #[wasm_bindgen(getter)]
    pub fn height(&self) -> u32 {
        self.height
    }

    #[wasm_bindgen(getter, js_name = "offsetX")]
    pub fn offset_x(&self) -> i32 {
        self.offset_x
    }

    #[wasm_bindgen(getter, js_name = "offsetY")]
    pub fn offset_y(&self) -> i32 {
        self.offset_y
    }

    #[wasm_bindgen(getter)]
    pub fn pixels(&self) -> Vec<u8> {
        self.pixels.clone()
    }
}

#[wasm_bindgen]
impl ImageFont {
    #[wasm_bindgen(constructor)]
    pub fn new(data: Vec<u8>, size: f32) -> Result<ImageFont, JsValue> {
        pillow_rs::imagefont_from_bytes(data, size)
            .map(|f| ImageFont { font: f })
            .map_err(err)
    }

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
            offset_x: offset.0,
            offset_y: offset.1,
            pixels,
        })
    }

    #[wasm_bindgen(js_name = "getname")]
    pub fn getname(&self) -> Vec<String> {
        let (family, style) = pillow_rs::imagefont_getname(&self.font);
        vec![family.to_owned(), style.to_owned()]
    }

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

    #[wasm_bindgen(js_name = "getVariationNames")]
    pub fn get_variation_names(&self) -> Result<js_sys::Array, JsValue> {
        let names = pillow_rs::imagefont_get_variation_names(&self.font).map_err(err)?;
        let result = js_sys::Array::new();
        for name in names {
            result.push(&js_sys::Uint8Array::from(name.as_slice()));
        }
        Ok(result)
    }

    #[wasm_bindgen(js_name = "setVariationByName")]
    pub fn set_variation_by_name(&mut self, name: Vec<u8>) -> Result<(), JsValue> {
        pillow_rs::imagefont_set_variation_by_name(&mut self.font, &name).map_err(err)
    }

    #[wasm_bindgen(js_name = "setVariationByAxes")]
    pub fn set_variation_by_axes(&mut self, axes: Vec<f32>) -> Result<(), JsValue> {
        pillow_rs::imagefont_set_variation_by_axes(&mut self.font, &axes).map_err(err)
    }

    #[wasm_bindgen(js_name = "fontVariant")]
    pub fn font_variant(&self, size: Option<f32>) -> Result<ImageFont, JsValue> {
        pillow_rs::imagefont_variant(&self.font, size)
            .map(|font| ImageFont { font })
            .map_err(err)
    }

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
            offset_x: 0,
            offset_y: 0,
            pixels,
        })
    }

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

    #[wasm_bindgen(js_name = "getTransposedLength")]
    pub fn get_transposed_length(
        &self,
        text: &str,
        orientation: Option<String>,
    ) -> Result<f32, JsValue> {
        pillow_rs::validate_transposed_length(orientation.as_deref()).map_err(err)?;
        pillow_rs::imagefont_getlength(&self.font, text).map_err(err)
    }

    #[wasm_bindgen(js_name = "getbbox")]
    pub fn getbbox(&self, text: &str) -> Result<Vec<u32>, JsValue> {
        let (w, h) = pillow_rs::imagefont_text_bbox(&self.font, text).map_err(err)?;
        Ok(vec![w, h])
    }
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
pub struct ImagePalette {
    mode: String,
    data: Vec<u8>,
}
#[wasm_bindgen]
impl ImagePalette {
    #[wasm_bindgen(constructor)]
    pub fn new(mode: &str) -> ImagePalette {
        ImagePalette {
            mode: mode.to_string(),
            data: vec![],
        }
    }
    #[wasm_bindgen(js_name = "copy")]
    pub fn copy(&self) -> ImagePalette {
        ImagePalette {
            mode: self.mode.clone(),
            data: self.data.clone(),
        }
    }
    #[wasm_bindgen(js_name = "tobytes")]
    pub fn tobytes(&self) -> Vec<u8> {
        self.data.clone()
    }
    #[wasm_bindgen(js_name = "getdata")]
    pub fn getdata(&self) -> JsValue {
        JsValue::from_str(&self.mode)
    }
    #[wasm_bindgen(js_name = "save")]
    pub fn save(&self) -> JsValue {
        JsValue::from_str("palette")
    }
}

// ── ImageStat ────────────────────────────────────────────────────
#[wasm_bindgen]
pub struct ImageStat {
    inner: pillow_rs::StatResult,
}
#[wasm_bindgen]
impl ImageStat {
    #[wasm_bindgen(constructor)]
    pub fn new(img: &Image) -> Result<ImageStat, JsValue> {
        let s = img.inner.stat_formatted().map_err(err)?;
        Ok(ImageStat { inner: s })
    }
    fn val_to_js(&self, v: &pillow_rs::StatValue) -> JsValue {
        use pillow_rs::StatValue;
        match v {
            StatValue::Int(i) => JsValue::from_f64(*i as f64),
            StatValue::Float(f) => JsValue::from_f64(*f),
            StatValue::IntList(l) => {
                let arr = js_sys::Array::new();
                for &x in l {
                    arr.push(&JsValue::from_f64(x as f64));
                }
                arr.into()
            }
            StatValue::FloatList(l) => {
                let arr = js_sys::Array::new();
                for &x in l {
                    arr.push(&JsValue::from_f64(x));
                }
                arr.into()
            }
            StatValue::ExtremaSingle((min, max)) => {
                let arr = js_sys::Array::new();
                arr.push(&JsValue::from_f64(*min as f64));
                arr.push(&JsValue::from_f64(*max as f64));
                arr.into()
            }
            StatValue::ExtremaList(l) => {
                let arr = js_sys::Array::new();
                for &(min, max) in l {
                    let pair = js_sys::Array::new();
                    pair.push(&JsValue::from_f64(min as f64));
                    pair.push(&JsValue::from_f64(max as f64));
                    arr.push(&pair);
                }
                arr.into()
            }
        }
    }
    pub fn toObject(&self) -> js_sys::Object {
        let obj = js_sys::Object::new();
        js_sys::Reflect::set(&obj, &"count".into(), &self.val_to_js(&self.inner.count)).ok();
        js_sys::Reflect::set(&obj, &"sum".into(), &self.val_to_js(&self.inner.sum)).ok();
        js_sys::Reflect::set(&obj, &"mean".into(), &self.val_to_js(&self.inner.mean)).ok();
        js_sys::Reflect::set(&obj, &"median".into(), &self.val_to_js(&self.inner.median)).ok();
        js_sys::Reflect::set(&obj, &"rms".into(), &self.val_to_js(&self.inner.rms)).ok();
        js_sys::Reflect::set(&obj, &"var".into(), &self.val_to_js(&self.inner.var)).ok();
        js_sys::Reflect::set(&obj, &"stddev".into(), &self.val_to_js(&self.inner.stddev)).ok();
        js_sys::Reflect::set(
            &obj,
            &"extrema".into(),
            &self.val_to_js(&self.inner.extrema),
        )
        .ok();
        obj
    }
}

// ── ImageSequence ────────────────────────────────────────────────
#[wasm_bindgen]
pub struct ImageSequence {}
#[wasm_bindgen]
impl ImageSequence {
    #[wasm_bindgen(constructor)]
    pub fn new(img: &Image) -> ImageSequence {
        ImageSequence {}
    }
    #[wasm_bindgen(js_name = "next")]
    pub fn next(&self) -> Option<Image> {
        None
    }
}

// ── Remaining stubs (WASM equivalents for file-I/O functions) ────
#[wasm_bindgen]
impl Image {
    #[wasm_bindgen(js_name = "save")]
    pub fn save(&mut self) -> Result<Vec<u8>, JsValue> {
        // Returns PNG-encoded bytes for download (browser) or fs.writeFile (server).
        // Uses the image crate's PNG encoder built into pillow-rs.
        self.inner.to_png_bytes().map_err(err)
    }

    /// Encode DynamicImage to PNG bytes
    fn encode_png(img: &mut RsImage) -> Result<Vec<u8>, JsValue> {
        img.to_png_bytes()
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }
    #[wasm_bindgen(js_name = "show")]
    pub fn show(&self) -> JsValue {
        JsValue::from_str("show: use toBytes() for display")
    }
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
#[wasm_bindgen]
impl ImageFont {
    #[wasm_bindgen(js_name = "load")]
    pub fn load(_path: &str, _size: f32) -> Result<ImageFont, JsValue> {
        Err(JsValue::from_str(
            "Use new ImageFont(data, size) with font bytes",
        ))
    }
    #[wasm_bindgen(js_name = "loadPath")]
    pub fn load_path(_path: &str, _size: f32) -> Result<ImageFont, JsValue> {
        Err(JsValue::from_str(
            "Use new ImageFont(data, size) with font bytes",
        ))
    }
    #[wasm_bindgen(js_name = "loadDefault")]
    pub fn load_default() -> Result<ImageFont, JsValue> {
        pillow_rs::imagefont_load_default(10.0)
            .map(|font| ImageFont { font })
            .map_err(err)
    }
}
#[wasm_bindgen(js_name = "imageOpen")]
pub fn image_open_path(_path: &str) -> Result<Image, JsValue> {
    Err(JsValue::from_str(
        "Use Image.open(bytes) instead of file path in WASM",
    ))
}
#[wasm_bindgen(js_name = "imageNew")]
pub fn image_new(mode: &str, w: u32, h: u32, r: u8, g: u8, b: u8, a: u8) -> Result<Image, JsValue> {
    RsImage::new(w, h, mode, (r, g, b, a))
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "imageNewPaletteIndex")]
pub fn image_new_palette_index(w: u32, h: u32, index: u8) -> Image {
    Image {
        inner: RsImage::new_palette_index(w, h, index),
    }
}

// ── ImageChops ───────────────────────────────────────────────────
#[wasm_bindgen]
pub struct ImageChops {}
#[wasm_bindgen]
impl ImageChops {
    #[wasm_bindgen(js_name = "add")]
    pub fn add(a: &Image, b: &Image) -> Result<Image, JsValue> {
        pillow_rs::chops_add(&a.inner, &b.inner, 1.0, 0.0)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "subtract")]
    pub fn sub(a: &Image, b: &Image) -> Result<Image, JsValue> {
        pillow_rs::chops_subtract(&a.inner, &b.inner, 1.0, 0.0)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "multiply")]
    pub fn mul(a: &Image, b: &Image) -> Result<Image, JsValue> {
        pillow_rs::chops_multiply(&a.inner, &b.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "screen")]
    pub fn scr(a: &Image, b: &Image) -> Result<Image, JsValue> {
        pillow_rs::chops_screen(&a.inner, &b.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "darker")]
    pub fn dark(a: &Image, b: &Image) -> Result<Image, JsValue> {
        pillow_rs::chops_darker(&a.inner, &b.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "lighter")]
    pub fn light(a: &Image, b: &Image) -> Result<Image, JsValue> {
        pillow_rs::chops_lighter(&a.inner, &b.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "difference")]
    pub fn diff(a: &Image, b: &Image) -> Result<Image, JsValue> {
        pillow_rs::chops_difference(&a.inner, &b.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "invert")]
    pub fn inv(img: &Image) -> Result<Image, JsValue> {
        pillow_rs::chops_invert(&img.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "hardLight")]
    pub fn hard(a: &Image, b: &Image) -> Result<Image, JsValue> {
        pillow_rs::chops_hard_light(&a.inner, &b.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "softLight")]
    pub fn soft(a: &Image, b: &Image) -> Result<Image, JsValue> {
        pillow_rs::chops_soft_light(&a.inner, &b.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "overlay")]
    pub fn over(a: &Image, b: &Image) -> Result<Image, JsValue> {
        pillow_rs::chops_overlay(&a.inner, &b.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "offset")]
    pub fn off(img: &Image, x: i32, y: i32) -> Result<Image, JsValue> {
        pillow_rs::chops_offset(&img.inner, x, y)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "addModulo")]
    pub fn addm(a: &Image, b: &Image) -> Result<Image, JsValue> {
        pillow_rs::chops_add_modulo(&a.inner, &b.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "subtractModulo")]
    pub fn subm(a: &Image, b: &Image) -> Result<Image, JsValue> {
        pillow_rs::chops_subtract_modulo(&a.inner, &b.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "blend")]
    pub fn blnd(a: &Image, b: &Image, alpha: f64) -> Result<Image, JsValue> {
        pillow_rs::image_blend(&a.inner, &b.inner, alpha)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "composite")]
    pub fn comp(a: &Image, b: &Image, m: &Image) -> Result<Image, JsValue> {
        pillow_rs::image_composite(&a.inner, &b.inner, &m.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "constant")]
    pub fn cnst(img: &Image, v: u8) -> Result<Image, JsValue> {
        pillow_rs::chops_constant(&img.inner, v)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "duplicate")]
    pub fn dup(img: &Image) -> Image {
        Image {
            inner: img.inner.clone(),
        }
    }
    #[wasm_bindgen(js_name = "logicalAnd")]
    pub fn land(a: &Image, b: &Image) -> Result<Image, JsValue> {
        pillow_rs::chops_logical_and(&a.inner, &b.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "logicalOr")]
    pub fn lor(a: &Image, b: &Image) -> Result<Image, JsValue> {
        pillow_rs::chops_logical_or(&a.inner, &b.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "logicalXor")]
    pub fn lxor(a: &Image, b: &Image) -> Result<Image, JsValue> {
        pillow_rs::chops_logical_xor(&a.inner, &b.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
}

// ── ImageOps ─────────────────────────────────────────────────────
#[wasm_bindgen]
pub struct ImageOps {}
#[wasm_bindgen]
impl ImageOps {
    #[wasm_bindgen(js_name = "invert")]
    pub fn inv(img: &Image) -> Result<Image, JsValue> {
        pillow_rs::imageops_invert(&img.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "flip")]
    pub fn flip(img: &Image) -> Result<Image, JsValue> {
        pillow_rs::imageops_flip(&img.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "mirror")]
    pub fn mirror(img: &Image) -> Result<Image, JsValue> {
        pillow_rs::imageops_mirror(&img.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "grayscale")]
    pub fn gray(img: &Image) -> Result<Image, JsValue> {
        pillow_rs::imageops_grayscale(&img.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "posterize")]
    pub fn post(img: &Image, b: u8) -> Result<Image, JsValue> {
        pillow_rs::imageops_posterize(&img.inner, b)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "solarize")]
    pub fn sol(img: &Image, t: u8) -> Result<Image, JsValue> {
        pillow_rs::imageops_solarize(&img.inner, t)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "equalize")]
    pub fn eq(img: &Image) -> Result<Image, JsValue> {
        pillow_rs::imageops_equalize(&img.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "autocontrast")]
    pub fn auto(img: &Image, c: f64) -> Result<Image, JsValue> {
        pillow_rs::imageops_autocontrast(&img.inner, c)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "expand")]
    pub fn expand(img: &Image, border: u32, r: u8, g: u8, b: u8, a: u8) -> Result<Image, JsValue> {
        pillow_rs::imageops_expand(&img.inner, border, (r, g, b, a))
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "contain")]
    pub fn contain(img: &Image, w: u32, h: u32) -> Result<Image, JsValue> {
        pillow_rs::imageops_contain(&img.inner, w, h, None)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "cover")]
    pub fn cover(img: &Image, w: u32, h: u32) -> Result<Image, JsValue> {
        pillow_rs::imageops_cover(&img.inner, w, h, None)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "fit")]
    pub fn fit(img: &Image, w: u32, h: u32) -> Result<Image, JsValue> {
        pillow_rs::imageops_fit(&img.inner, w, h, None, 0.0, (0.5, 0.5))
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "pad")]
    pub fn pad(
        img: &Image,
        w: u32,
        h: u32,
        r: Option<u8>,
        g: Option<u8>,
        b: Option<u8>,
        a: Option<u8>,
    ) -> Result<Image, JsValue> {
        let color = r.map(|cr| (cr, g.unwrap_or(0), b.unwrap_or(0), a.unwrap_or(255)));
        pillow_rs::imageops_pad(&img.inner, w, h, None, color, (0.5, 0.5))
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "scale")]
    pub fn scale(img: &Image, factor: f64) -> Result<Image, JsValue> {
        pillow_rs::imageops_scale(&img.inner, factor, None)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "crop")]
    pub fn crop(img: &Image, border: u32) -> Result<Image, JsValue> {
        pillow_rs::imageops_crop(&img.inner, border)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "colorize")]
    pub fn colorize(
        img: &Image,
        black_r: u8,
        black_g: u8,
        black_b: u8,
        white_r: u8,
        white_g: u8,
        white_b: u8,
    ) -> Result<Image, JsValue> {
        pillow_rs::imageops_colorize(
            &img.inner,
            (black_r, black_g, black_b),
            (white_r, white_g, white_b),
        )
        .map(|i| Image { inner: i })
        .map_err(err)
    }
}

// ── Module functions ─────────────────────────────────────────────
#[wasm_bindgen(js_name = "merge")]
pub fn merge(mode: &str, bands: Vec<Image>) -> Result<Image, JsValue> {
    let imgs: Vec<RsImage> = bands.iter().map(|b| b.inner.clone()).collect();
    pillow_rs::image_merge(mode, &imgs)
        .map(|i| Image { inner: i })
        .map_err(err)
}
#[wasm_bindgen(js_name = "blend")]
pub fn blend(a: &Image, b: &Image, alpha: f64) -> Result<Image, JsValue> {
    pillow_rs::image_blend(&a.inner, &b.inner, alpha)
        .map(|i| Image { inner: i })
        .map_err(err)
}
#[wasm_bindgen(js_name = "composite")]
pub fn composite(a: &Image, b: &Image, m: &Image) -> Result<Image, JsValue> {
    pillow_rs::image_composite(&a.inner, &b.inner, &m.inner)
        .map(|i| Image { inner: i })
        .map_err(err)
}

/// Activate a compute backend. Returns true if the backend exists.
#[wasm_bindgen]
pub fn enable_backend(name: &str) -> Result<bool, JsValue> {
    let backend = pillow_rs::Backend::parse(name).ok_or_else(|| {
        err(pillow_rs::PilError::ValueError(format!(
            "unknown backend: {name}"
        )))
    })?;
    pillow_rs::enable_backend(backend).map_err(err)
}

/// Deactivate a compute backend. Returns true if it was active.
#[wasm_bindgen]
pub fn disable_backend(name: &str) -> Result<bool, JsValue> {
    let backend = pillow_rs::Backend::parse(name).ok_or_else(|| {
        err(pillow_rs::PilError::ValueError(format!(
            "unknown backend: {name}"
        )))
    })?;
    pillow_rs::disable_backend(backend).map_err(err)
}

/// List backends that exist on this machine.
#[wasm_bindgen]
pub fn available_backends() -> Vec<String> {
    pillow_rs::available_backends()
        .iter()
        .map(|b| format!("{:?}", b).to_lowercase())
        .collect()
}

/// List currently active backends (priority order).
#[wasm_bindgen]
pub fn active_backends() -> Result<Vec<String>, JsValue> {
    Ok(pillow_rs::active_backends()
        .map_err(err)?
        .into_iter()
        .map(|b| format!("{:?}", b).to_lowercase())
        .collect())
}

/// Check if a specific backend is active.
#[wasm_bindgen]
pub fn backend_enabled(name: &str) -> Result<bool, JsValue> {
    let backend = pillow_rs::Backend::parse(name).ok_or_else(|| {
        err(pillow_rs::PilError::ValueError(format!(
            "unknown backend: {name}"
        )))
    })?;
    pillow_rs::backend_enabled(backend).map_err(err)
}

/// Set the maximum log level shown in the browser console.
/// Levels (ascending): 0=off, 1=error, 2=warn, 3=info, 4=debug, 5=trace.
#[wasm_bindgen(js_name = "setLogLevel")]
pub fn set_log_level(level: u8) {
    #[cfg(feature = "debug-hooks")]
    {
        let lvl = match level {
            0 => log::LevelFilter::Off,
            1 => log::LevelFilter::Error,
            2 => log::LevelFilter::Warn,
            3 => log::LevelFilter::Info,
            4 => log::LevelFilter::Debug,
            5 => log::LevelFilter::Trace,
            _ => log::LevelFilter::Warn,
        };
        log::set_max_level(lvl);
    }

    #[cfg(not(feature = "debug-hooks"))]
    let _ = level;
}

// ══════════════════════════════════════════════════════════════════════════════
// ImageChops — per-pixel channel operations (thin wrappers)
// ══════════════════════════════════════════════════════════════════════════════

#[wasm_bindgen(js_name = "addModulo")]
pub fn add_modulo(a: &Image, b: &Image) -> Result<Image, JsValue> {
    pillow_rs::chops_add_modulo(&a.inner, &b.inner)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "constant")]
pub fn constant(img: &Image, value: u8) -> Result<Image, JsValue> {
    pillow_rs::chops_constant(&img.inner, value)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "darker")]
pub fn darker(a: &Image, b: &Image) -> Result<Image, JsValue> {
    pillow_rs::chops_darker(&a.inner, &b.inner)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "hardLight")]
pub fn hard_light(a: &Image, b: &Image) -> Result<Image, JsValue> {
    pillow_rs::chops_hard_light(&a.inner, &b.inner)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "lighter")]
pub fn lighter(a: &Image, b: &Image) -> Result<Image, JsValue> {
    pillow_rs::chops_lighter(&a.inner, &b.inner)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "logicalAnd")]
pub fn logical_and(a: &Image, b: &Image) -> Result<Image, JsValue> {
    pillow_rs::chops_logical_and(&a.inner, &b.inner)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "logicalOr")]
pub fn logical_or(a: &Image, b: &Image) -> Result<Image, JsValue> {
    pillow_rs::chops_logical_or(&a.inner, &b.inner)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "logicalXor")]
pub fn logical_xor(a: &Image, b: &Image) -> Result<Image, JsValue> {
    pillow_rs::chops_logical_xor(&a.inner, &b.inner)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "multiply")]
pub fn multiply(a: &Image, b: &Image) -> Result<Image, JsValue> {
    pillow_rs::chops_multiply(&a.inner, &b.inner)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "offset")]
pub fn offset(img: &Image, xoffset: i32, yoffset: i32) -> Result<Image, JsValue> {
    pillow_rs::chops_offset(&img.inner, xoffset, yoffset)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "overlay")]
pub fn overlay(a: &Image, b: &Image) -> Result<Image, JsValue> {
    pillow_rs::chops_overlay(&a.inner, &b.inner)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "screenFn")]
pub fn screen(a: &Image, b: &Image) -> Result<Image, JsValue> {
    pillow_rs::chops_screen(&a.inner, &b.inner)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "softLight")]
pub fn soft_light(a: &Image, b: &Image) -> Result<Image, JsValue> {
    pillow_rs::chops_soft_light(&a.inner, &b.inner)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "subtractModulo")]
pub fn subtract_modulo(a: &Image, b: &Image) -> Result<Image, JsValue> {
    pillow_rs::chops_subtract_modulo(&a.inner, &b.inner)
        .map(|i| Image { inner: i })
        .map_err(err)
}

// ══════════════════════════════════════════════════════════════════════════════
// ImageOps — high-level image operations
// ══════════════════════════════════════════════════════════════════════════════

#[wasm_bindgen(js_name = "autocontrastFn")]
pub fn autocontrast(img: &Image, cutoff: f64) -> Result<Image, JsValue> {
    pillow_rs::imageops_autocontrast(&img.inner, cutoff)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "equalizeFn")]
pub fn equalize(img: &Image) -> Result<Image, JsValue> {
    pillow_rs::imageops_equalize(&img.inner)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "flipFn")]
pub fn flip(img: &Image) -> Result<Image, JsValue> {
    pillow_rs::imageops_flip(&img.inner)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "mirrorFn")]
pub fn mirror(img: &Image) -> Result<Image, JsValue> {
    pillow_rs::imageops_mirror(&img.inner)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "posterizeFn")]
pub fn posterize(img: &Image, bits: u8) -> Result<Image, JsValue> {
    pillow_rs::imageops_posterize(&img.inner, bits)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "solarizeFn")]
pub fn solarize(img: &Image, threshold: u8) -> Result<Image, JsValue> {
    pillow_rs::imageops_solarize(&img.inner, threshold)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "grayscaleFn")]
pub fn grayscale(img: &Image) -> Result<Image, JsValue> {
    pillow_rs::imageops_grayscale(&img.inner)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "expand")]
pub fn expand(
    img: &Image,
    border: u32,
    fill_r: u8,
    fill_g: u8,
    fill_b: u8,
    fill_a: u8,
) -> Result<Image, JsValue> {
    pillow_rs::imageops_expand(&img.inner, border, (fill_r, fill_g, fill_b, fill_a))
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "containFn")]
pub fn contain(img: &Image, w: u32, h: u32) -> Result<Image, JsValue> {
    pillow_rs::imageops_contain(&img.inner, w, h, None)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "coverFn")]
pub fn cover(img: &Image, w: u32, h: u32) -> Result<Image, JsValue> {
    pillow_rs::imageops_cover(&img.inner, w, h, None)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "fitFn")]
pub fn fit(img: &Image, w: u32, h: u32) -> Result<Image, JsValue> {
    pillow_rs::imageops_fit(&img.inner, w, h, None, 0.0, (0.5, 0.5))
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "padFn")]
pub fn pad(img: &Image, w: u32, h: u32, color: Vec<u8>) -> Result<Image, JsValue> {
    let c = match color.len() {
        3 => Some((color[0], color[1], color[2], 255)),
        4 => Some((color[0], color[1], color[2], color[3])),
        _ => None,
    };
    pillow_rs::imageops_pad(&img.inner, w, h, None, c, (0.5, 0.5))
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "cropFn")]
pub fn crop_border(img: &Image, border: u32) -> Result<Image, JsValue> {
    pillow_rs::imageops_crop(&img.inner, border)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "scaleFn")]
pub fn scale(img: &Image, factor: f64) -> Result<Image, JsValue> {
    pillow_rs::imageops_scale(&img.inner, factor, None)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "exifOrientation")]
pub fn exif_orientation(raw: Vec<u8>) -> Option<u32> {
    pillow_rs::exif_get_orientation(&raw)
}

#[wasm_bindgen(js_name = "exifRemoveOrientation")]
pub fn exif_remove_orientation(raw: Vec<u8>) -> Vec<u8> {
    pillow_rs::exif_remove_orientation(&raw)
}

// ══════════════════════════════════════════════════════════════════════════════
// ImageModule — module-level operations
// ══════════════════════════════════════════════════════════════════════════════

#[wasm_bindgen(js_name = "effectMandelbrot")]
pub fn effect_mandelbrot(
    w: u32,
    h: u32,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    quality: u32,
) -> Result<Image, JsValue> {
    let quality = quality.try_into().map_err(|_| {
        err(pillow_rs::PilError::ValueError(
            "quality exceeds supported Mandelbrot iteration range".to_string(),
        ))
    })?;
    pillow_rs::image_effect_mandelbrot((w, h), (x0, y0, x1, y1), quality)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "effectNoiseFn")]
pub fn effect_noise(width: u32, height: u32, sigma: f64) -> Result<Image, JsValue> {
    let blank = RsImage::new(width, height, "L", (0, 0, 0, 255)).map_err(err)?;
    pillow_rs::image_effect_noise(&blank, sigma)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "effectSpreadFn")]
pub fn effect_spread(img: &Image, distance: u32) -> Result<Image, JsValue> {
    pillow_rs::image_effect_spread(&img.inner, distance)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "evalFn")]
pub fn eval_fn(img: &Image, lut: Vec<u8>, _n_bands: usize) -> Result<Image, JsValue> {
    pillow_rs::image_eval_replicated_for_image(&img.inner, &lut)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "linearGradientFn")]
pub fn linear_gradient(mode: &str) -> Result<Image, JsValue> {
    pillow_rs::image_linear_gradient(mode)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "radialGradientFn")]
pub fn radial_gradient(mode: &str) -> Result<Image, JsValue> {
    pillow_rs::image_radial_gradient(mode)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "mergeFn")]
pub fn merge_fn(mode: &str, bands: Vec<Image>) -> Result<Image, JsValue> {
    let inner_bands: Vec<pillow_rs::Image> = bands.iter().map(|b| b.inner.clone()).collect();
    pillow_rs::image_merge(mode, &inner_bands)
        .map(|i| Image { inner: i })
        .map_err(err)
}

// ══════════════════════════════════════════════════════════════════════════════
// ImageColor — color resolution
// ══════════════════════════════════════════════════════════════════════════════

#[wasm_bindgen(js_name = "getColor")]
pub fn getcolor(color: &str, mode: &str) -> Result<JsValue, JsValue> {
    let (r, g, b, a) =
        pillow_rs::parse_color_str(color).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let (r, g, b, a) =
        pillow_rs::getcolor(r, g, b, mode).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let arr = js_sys::Array::new();
    arr.push(&JsValue::from(r));
    if mode == "LA" || mode == "RGBA" {
        arr.push(&JsValue::from(a));
    }
    if mode != "L" && mode != "1" && mode != "LA" {
        arr.push(&JsValue::from(g));
        arr.push(&JsValue::from(b));
        if mode == "RGBA" {
            arr.push(&JsValue::from(a));
        }
    }
    Ok(arr.into())
}

// ══════════════════════════════════════════════════════════════════════════════
// ImagePalette — palette operations
// ══════════════════════════════════════════════════════════════════════════════

#[wasm_bindgen(js_name = "paletteGetColor")]
pub fn palette_getcolor(palette: Vec<u8>, r: u8, g: u8, b: u8) -> Option<usize> {
    pillow_rs::palette_getcolor(&palette, r, g, b)
}

#[wasm_bindgen(js_name = "paletteGetColorAppend")]
pub fn palette_getcolor_append(
    palette: Vec<u8>,
    r: u8,
    g: u8,
    b: u8,
    a: u8,
    mode: &str,
) -> Result<usize, JsValue> {
    let mut pal = palette;
    pillow_rs::palette_getcolor_append(&mut pal, r, g, b, a, mode)
        .map_err(|e| JsValue::from_str(&e))
}

#[wasm_bindgen(js_name = "paletteGetColorValidate")]
pub fn palette_getcolor_validate(
    palette: Vec<u8>,
    color: Vec<u8>,
    mode: &str,
) -> Result<usize, JsValue> {
    pillow_rs::palette_getcolor_validate(&mut palette.clone(), &color, mode)
        .map_err(|e| JsValue::from_str(&e.to_string()))
}

#[wasm_bindgen(js_name = "paletteToText")]
pub fn palette_to_text(palette: Vec<u8>, mode: &str) -> String {
    pillow_rs::palette_to_text(&palette, mode)
}

// ══════════════════════════════════════════════════════════════════════════════
// ImageStat — statistics
// ══════════════════════════════════════════════════════════════════════════════

#[wasm_bindgen(js_name = "statFromList")]
pub fn stat_from_list(data: Vec<f64>) -> Result<JsValue, JsValue> {
    let (count, sum, mean, min, max) = pillow_rs::stat_from_list(&data);
    let obj = js_sys::Object::new();
    js_sys::Reflect::set(&obj, &JsValue::from_str("count"), &JsValue::from_f64(count))?;
    js_sys::Reflect::set(&obj, &JsValue::from_str("sum"), &JsValue::from_f64(sum))?;
    js_sys::Reflect::set(&obj, &JsValue::from_str("mean"), &JsValue::from_f64(mean))?;
    js_sys::Reflect::set(&obj, &JsValue::from_str("min"), &JsValue::from_f64(min))?;
    js_sys::Reflect::set(&obj, &JsValue::from_str("max"), &JsValue::from_f64(max))?;
    Ok(obj.into())
}

// ══════════════════════════════════════════════════════════════════════════════
// Draw helpers
// ══════════════════════════════════════════════════════════════════════════════

#[wasm_bindgen(js_name = "outlineCurve")]
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

#[wasm_bindgen(js_name = "resolveNewColor")]
pub fn resolve_new_color(
    mode: &str,
    hex: Option<String>,
    single: Option<u8>,
    rgb: Option<Vec<u8>>,
    rgba: Option<Vec<u8>>,
    la: Option<Vec<u8>>,
) -> Result<JsValue, JsValue> {
    let hex = hex.as_deref();
    let rgb = rgb.map(|v| {
        if v.len() == 3 {
            (v[0], v[1], v[2])
        } else {
            (0, 0, 0)
        }
    });
    let rgba = rgba.map(|v| {
        if v.len() == 4 {
            (v[0], v[1], v[2], v[3])
        } else {
            (0, 0, 0, 0)
        }
    });
    let la = la.map(|v| if v.len() == 2 { (v[0], v[1]) } else { (0, 0) });
    let c = pillow_rs::resolve_new_color(mode, hex, single, rgb, rgba, la, None, None)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
    let arr = js_sys::Array::new();
    arr.push(&JsValue::from(c.0));
    arr.push(&JsValue::from(c.1));
    arr.push(&JsValue::from(c.2));
    arr.push(&JsValue::from(c.3));
    Ok(arr.into())
}
