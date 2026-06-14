//! pillow-rs WASM — full Pillow API for the browser. Thin delegation to pillow-rs-core.
use pillow_rs_core::image::Image as RsImage;
use pillow_rs_core::ops::{chops, imageops, module_fns};
use wasm_bindgen::prelude::*;

fn err(e: pillow_rs_core::error::PilError) -> JsValue {
    JsValue::from_str(&e.to_string())
}

#[wasm_bindgen]
pub struct Image {
    inner: RsImage,
}

#[wasm_bindgen]
impl Image {
    #[wasm_bindgen(constructor)]
    pub fn new(mode: &str, w: u32, h: u32, r: u8, g: u8, b: u8, a: u8) -> Result<Image, JsValue> {
        console_error_panic_hook::set_once();
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
    #[wasm_bindgen(getter)]
    pub fn is_animated(&self) -> bool {
        false
    }
    #[wasm_bindgen(getter)]
    pub fn n_frames(&self) -> u32 {
        1
    }
    #[wasm_bindgen(getter)]
    pub fn has_transparency_data(&self) -> bool {
        false
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
            .crop((l, t, r - l, b - t))
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "rotate")]
    pub fn rotate(
        &self,
        a: f64,
        expand: Option<bool>,
        fill: Option<Vec<u8>>,
    ) -> Result<Image, JsValue> {
        let fill_color = fill.map(|f| {
            (
                f.get(0).copied().unwrap_or(0),
                f.get(1).copied().unwrap_or(0),
                f.get(2).copied().unwrap_or(0),
                f.get(3).copied().unwrap_or(255),
            )
        });
        self.inner
            .rotate(a, expand.unwrap_or(false), fill_color)
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
    pub fn convert(
        &self,
        m: &str,
        dither: Option<String>,
        _palette: Option<Vec<u8>>,
        colors: Option<u32>,
    ) -> Result<Image, JsValue> {
        if m == "I" || m == "F" {
            // Convert to L first (always works), then widen to 32-bit
            let l_img = self
                .inner
                .convert("L", None, None, None, None)
                .map_err(err)?;
            let (w, h) = l_img.size().map_err(err)?;
            let l_bytes = l_img.tobytes().map_err(err)?;
            // Build 4-byte-per-pixel data matching PIL's I/F mode format
            let mut data = Vec::with_capacity(l_bytes.len() * 4);
            if m == "I" {
                // I mode: 32-bit signed int per pixel (little-endian i32)
                for &b in &l_bytes {
                    let val: i32 = b as i32;
                    data.extend_from_slice(&val.to_le_bytes());
                }
            } else {
                // F mode: 32-bit IEEE 754 float per pixel (little-endian f32)
                // L value 128 -> F value 128/255 = 0.50196...
                for &b in &l_bytes {
                    let val: f32 = (b as f32) / 255.0;
                    data.extend_from_slice(&val.to_le_bytes());
                }
            }
            return RsImage::frombytes(m, (w, h), &data)
                .map(|i| Image { inner: i })
                .map_err(err);
        }
        self.inner
            .convert(m, None, dither.as_deref(), None, colors)
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
        use pillow_rs_core::ops::paste::PasteSource;
        self.inner
            .paste(
                PasteSource::Image(src.inner.clone()),
                Some((x, y, x, y)),
                None,
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
        use pillow_rs_core::ops::paste::PasteSource;
        self.inner
            .paste(PasteSource::Color((r, g, b, a)), Some((l, t, rt, bt)), None)
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
        module_fns::eval(&self.inner, &lut)
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
    pub fn getextrema(&self) -> Result<Vec<u8>, JsValue> {
        self.inner
            .getextrema()
            .map(|e| e.iter().flat_map(|(a, b)| vec![*a, *b]).collect())
            .map_err(err)
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
            None => Ok(JsValue::null()),
            Some(colors) => {
                let arr = js_sys::Array::new();
                for (count, color) in &colors {
                    let item = js_sys::Array::new();
                    item.push(&JsValue::from(*count));
                    let color_arr = js_sys::Array::new();
                    for &c in color {
                        color_arr.push(&JsValue::from(c));
                    }
                    item.push(&color_arr);
                    arr.push(&item);
                }
                Ok(JsValue::from(arr))
            }
        }
    }
    #[wasm_bindgen(js_name = "getdata")]
    pub fn getdata(&mut self, b: Option<i32>) -> Result<Vec<u8>, JsValue> {
        self.inner.getdata(b).map_err(err)
    }
    #[wasm_bindgen(js_name = "getprojection")]
    pub fn getprojection(&mut self) -> Result<JsValue, JsValue> {
        self.inner
            .getprojection()
            .map(|(h_proj, v_proj)| {
                let h_arr = js_sys::Array::new();
                for &val in &h_proj {
                    h_arr.push(&JsValue::from(val));
                }
                let v_arr = js_sys::Array::new();
                for &val in &v_proj {
                    v_arr.push(&JsValue::from(val));
                }
                let result = js_sys::Array::new();
                result.push(&h_arr);
                result.push(&v_arr);
                JsValue::from(result)
            })
            .map_err(err)
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
    #[wasm_bindgen(js_name = "boxBlur")]
    pub fn box_blur(&self, radius: f32) -> Result<Image, JsValue> {
        self.inner
            .box_blur(radius)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "modeFilter")]
    pub fn mode_filter(&self, size: u32) -> Result<Image, JsValue> {
        self.inner
            .mode_filter(size)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "rankFilter")]
    pub fn rank_filter(&self, size: u32, rank: u32) -> Result<Image, JsValue> {
        self.inner
            .rank_filter(size, rank)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "kernelFilter")]
    pub fn kernel_filter(
        &self,
        size: Vec<u32>,
        kernel: Vec<f32>,
        scale: Option<f64>,
        offset: Option<f64>,
    ) -> Result<Image, JsValue> {
        let sz = size.get(0).copied().unwrap_or(3);
        let s = scale.unwrap_or(1.0) as f32;
        let o = offset.unwrap_or(0.0) as i32;
        self.inner
            .kernel_filter(&kernel, s, o, sz)
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
        module_fns::effect_spread(&self.inner, d)
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
        self.inner.materialize().map(|_| ()).map_err(err)
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
    pub fn getpalette(&self) -> Result<JsValue, JsValue> {
        let img = self.inner.materialize().map_err(err)?;
        Ok(JsValue::from_str(&format!("{:?}", img.color())))
    }
    #[wasm_bindgen(js_name = "putpalette")]
    pub fn putpalette(&mut self, _data: Vec<u8>) {}
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
        Err(JsValue::from_str("not yet implemented"))
    }
    #[wasm_bindgen(js_name = "applyTransparency")]
    pub fn apply_transparency(&self) {}
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
use pillow_rs_core::draw::Draw;

#[wasm_bindgen]
pub struct ImageDraw {
    draw: Draw,
}

#[wasm_bindgen]
impl ImageDraw {
    #[wasm_bindgen(constructor)]
    pub fn new(img: &Image) -> ImageDraw {
        ImageDraw {
            draw: Draw::new(img.inner.clone()),
        }
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
    ) -> Result<(), JsValue> {
        self.draw.line(x0, y0, x1, y1, (r, g, b, a), 1).map_err(err)
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
    ) -> Result<(), JsValue> {
        let fill = fr.map(|r| (r, fg.unwrap_or(0), fb.unwrap_or(0), fa.unwrap_or(255)));
        let out = or.map(|r| (r, og.unwrap_or(0), ob.unwrap_or(0), oa.unwrap_or(255)));
        self.draw
            .rectangle(x0, y0, x1, y1, fill, out, 1)
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
    ) -> Result<(), JsValue> {
        let fill = fr.map(|r| (r, fg.unwrap_or(0), fb.unwrap_or(0), fa.unwrap_or(255)));
        let out = or.map(|r| (r, og.unwrap_or(0), ob.unwrap_or(0), oa.unwrap_or(255)));
        self.draw.ellipse(x0, y0, x1, y1, fill, out, 1).map_err(err)
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
    ) -> Result<(), JsValue> {
        let pts: Vec<(i32, i32)> = points.chunks(2).map(|c| (c[0], c[1])).collect();
        let fill = fr.map(|r| (r, fg.unwrap_or(0), fb.unwrap_or(0), fa.unwrap_or(255)));
        let out = or.map(|r| (r, og.unwrap_or(0), ob.unwrap_or(0), oa.unwrap_or(255)));
        self.draw.polygon(&pts, fill, out, 1).map_err(err)
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
    ) -> Result<(), JsValue> {
        self.draw
            .arc(x0, y0, x1, y1, start, end, (r, g, b, a), 1)
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
    ) -> Result<(), JsValue> {
        let fill = fr.map(|r| (r, fg.unwrap_or(0), fb.unwrap_or(0), fa.unwrap_or(255)));
        let out = or.map(|r| (r, og.unwrap_or(0), ob.unwrap_or(0), oa.unwrap_or(255)));
        self.draw
            .chord(x0, y0, x1, y1, start, end, fill, out, 1)
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
    ) -> Result<(), JsValue> {
        let fill = fr.map(|r| (r, fg.unwrap_or(0), fb.unwrap_or(0), fa.unwrap_or(255)));
        let out = or.map(|r| (r, og.unwrap_or(0), ob.unwrap_or(0), oa.unwrap_or(255)));
        self.draw
            .pieslice(x0, y0, x1, y1, start, end, fill, out, 1)
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
    ) -> Result<(), JsValue> {
        let fill = fr.map(|r| (r, fg.unwrap_or(0), fb.unwrap_or(0), fa.unwrap_or(255)));
        let out = or.map(|r| (r, og.unwrap_or(0), ob.unwrap_or(0), oa.unwrap_or(255)));
        self.draw
            .circle(cx as i32, cy as i32, radius, fill, out, 1)
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
    ) -> Result<(), JsValue> {
        let fill = fr.map(|r| (r, fg.unwrap_or(0), fb.unwrap_or(0), fa.unwrap_or(255)));
        let out = or.map(|r| (r, og.unwrap_or(0), ob.unwrap_or(0), oa.unwrap_or(255)));
        self.draw
            .rounded_rectangle(x0, y0, x1, y1, radius, fill, out, 1)
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "text")]
    pub fn text(&mut self, x: f64, y: f64, text: &str, font: &ImageFont) -> Result<(), JsValue> {
        self.draw
            .text(x as i32, y as i32, text, &font.font, (0, 0, 0, 255))
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "textFill")]
    pub fn text_fill(
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
        xy: Vec<i32>,
        bitmap: &Image,
        fill: Option<Vec<u8>>,
    ) -> Result<(), JsValue> {
        let x = xy.get(0).copied().unwrap_or(0);
        let y = xy.get(1).copied().unwrap_or(0);
        let fill_color = fill.map(|f| {
            (
                f.get(0).copied().unwrap_or(255),
                f.get(1).copied().unwrap_or(255),
                f.get(2).copied().unwrap_or(255),
                f.get(3).copied().unwrap_or(255),
            )
        });
        self.draw
            .bitmap(x, y, &bitmap.inner, fill_color)
            .map_err(err)
    }
    #[wasm_bindgen(getter)]
    pub fn image(&self) -> Image {
        Image {
            inner: self.draw.image_clone(),
        }
    }
}

// ── ImageFont ────────────────────────────────────────────────────
use pillow_rs_core::font::Font;

#[wasm_bindgen]
pub struct ImageFont {
    font: Font,
}

#[wasm_bindgen]
impl ImageFont {
    #[wasm_bindgen(constructor)]
    pub fn new(data: Vec<u8>, size: f32) -> Result<ImageFont, JsValue> {
        Font::from_bytes(data, size)
            .map(|f| ImageFont { font: f })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "getbbox")]
    pub fn getbbox(&self, text: &str) -> Vec<u32> {
        let (w, h) = self.font.text_bbox(text);
        vec![w, h]
    }
    #[wasm_bindgen(js_name = "getmask")]
    pub fn getmask(&self, text: &str) -> Vec<u8> {
        let (w, h, data) = self.font.getmask(text);
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
        result
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
    #[wasm_bindgen(js_name = "getcolor")]
    pub fn getcolor(&self, index: u32) -> Vec<u8> {
        let idx = (index as usize) * 3;
        if idx + 2 < self.data.len() {
            vec![self.data[idx], self.data[idx + 1], self.data[idx + 2]]
        } else {
            vec![0, 0, 0]
        }
    }
}

// ── ImageStat ────────────────────────────────────────────────────
#[wasm_bindgen]
pub struct ImageStat {
    count: Vec<u32>,
    sum: Vec<f64>,
    mean: Vec<f64>,
    median: Vec<f64>,
    rms: Vec<f64>,
    var: Vec<f64>,
    stddev: Vec<f64>,
    extrema: Vec<u8>,
}
#[wasm_bindgen]
impl ImageStat {
    #[wasm_bindgen(constructor)]
    pub fn new(img: &Image) -> Result<ImageStat, JsValue> {
        let stats = img.inner.stat().map_err(err)?;
        let n_bands = stats.len();
        let mut count = Vec::with_capacity(n_bands);
        let mut sum = Vec::with_capacity(n_bands);
        let mut mean = Vec::with_capacity(n_bands);
        let mut median = Vec::with_capacity(n_bands);
        let mut rms = Vec::with_capacity(n_bands);
        let mut var = Vec::with_capacity(n_bands);
        let mut stddev = Vec::with_capacity(n_bands);
        let mut extrema = Vec::with_capacity(n_bands * 2);
        for band in &stats {
            count.push(band[0] as u32);
            sum.push(band[1]);
            mean.push(band[3]);
            median.push(band[4]);
            rms.push(band[5]);
            var.push(band[6]);
            stddev.push(band[7]);
            extrema.push(band[8] as u8);
            extrema.push(band[9] as u8);
        }
        Ok(ImageStat {
            count,
            sum,
            mean,
            median,
            rms,
            var,
            stddev,
            extrema,
        })
    }
    #[wasm_bindgen(getter)]
    pub fn count(&self) -> Vec<u32> {
        self.count.clone()
    }
    #[wasm_bindgen(getter)]
    pub fn sum(&self) -> Vec<f64> {
        self.sum.clone()
    }
    #[wasm_bindgen(getter)]
    pub fn mean(&self) -> Vec<f64> {
        self.mean.clone()
    }
    #[wasm_bindgen(getter)]
    pub fn median(&self) -> Vec<f64> {
        self.median.clone()
    }
    #[wasm_bindgen(getter)]
    pub fn rms(&self) -> Vec<f64> {
        self.rms.clone()
    }
    #[wasm_bindgen(getter)]
    pub fn var(&self) -> Vec<f64> {
        self.var.clone()
    }
    #[wasm_bindgen(getter)]
    pub fn stddev(&self) -> Vec<f64> {
        self.stddev.clone()
    }
    #[wasm_bindgen(getter)]
    pub fn extrema(&self) -> Vec<u8> {
        self.extrema.clone()
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
        // Uses the image crate's PNG encoder built into pillow-rs-core.
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
    pub fn load_default(size: Option<f32>) -> ImageFont {
        ImageFont {
            font: Font::load_default(size.unwrap_or(10.0)),
        }
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

// ── ImageChops ───────────────────────────────────────────────────
#[wasm_bindgen]
pub struct ImageChops {}
#[wasm_bindgen]
impl ImageChops {
    #[wasm_bindgen(js_name = "add")]
    pub fn add(a: &Image, b: &Image) -> Result<Image, JsValue> {
        chops::add(&a.inner, &b.inner, 1.0, 0.0)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "subtract")]
    pub fn sub(a: &Image, b: &Image) -> Result<Image, JsValue> {
        chops::subtract(&a.inner, &b.inner, 1.0, 0.0)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "multiply")]
    pub fn mul(a: &Image, b: &Image) -> Result<Image, JsValue> {
        chops::multiply(&a.inner, &b.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "screen")]
    pub fn scr(a: &Image, b: &Image) -> Result<Image, JsValue> {
        chops::screen(&a.inner, &b.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "darker")]
    pub fn dark(a: &Image, b: &Image) -> Result<Image, JsValue> {
        chops::darker(&a.inner, &b.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "lighter")]
    pub fn light(a: &Image, b: &Image) -> Result<Image, JsValue> {
        chops::lighter(&a.inner, &b.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "difference")]
    pub fn diff(a: &Image, b: &Image) -> Result<Image, JsValue> {
        chops::difference(&a.inner, &b.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "invert")]
    pub fn inv(img: &Image) -> Result<Image, JsValue> {
        chops::invert(&img.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "hardLight")]
    pub fn hard(a: &Image, b: &Image) -> Result<Image, JsValue> {
        chops::hard_light(&a.inner, &b.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "softLight")]
    pub fn soft(a: &Image, b: &Image) -> Result<Image, JsValue> {
        chops::soft_light(&a.inner, &b.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "overlay")]
    pub fn over(a: &Image, b: &Image) -> Result<Image, JsValue> {
        chops::overlay(&a.inner, &b.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "offset")]
    pub fn off(img: &Image, x: i32, y: i32) -> Result<Image, JsValue> {
        chops::offset(&img.inner, x, y)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "addModulo")]
    pub fn addm(a: &Image, b: &Image) -> Result<Image, JsValue> {
        chops::add_modulo(&a.inner, &b.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "subtractModulo")]
    pub fn subm(a: &Image, b: &Image) -> Result<Image, JsValue> {
        chops::subtract_modulo(&a.inner, &b.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "blend")]
    pub fn blnd(a: &Image, b: &Image, alpha: f64) -> Result<Image, JsValue> {
        module_fns::blend(&a.inner, &b.inner, alpha)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "composite")]
    pub fn comp(a: &Image, b: &Image, m: &Image) -> Result<Image, JsValue> {
        module_fns::composite(&a.inner, &b.inner, &m.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "constant")]
    pub fn cnst(img: &Image, v: u8) -> Result<Image, JsValue> {
        chops::constant(&img.inner, v)
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
        chops::logical_and(&a.inner, &b.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "logicalOr")]
    pub fn lor(a: &Image, b: &Image) -> Result<Image, JsValue> {
        chops::logical_or(&a.inner, &b.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "logicalXor")]
    pub fn lxor(a: &Image, b: &Image) -> Result<Image, JsValue> {
        chops::logical_xor(&a.inner, &b.inner)
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
        imageops::invert(&img.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "flip")]
    pub fn flip(img: &Image) -> Result<Image, JsValue> {
        imageops::flip(&img.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "mirror")]
    pub fn mirror(img: &Image) -> Result<Image, JsValue> {
        imageops::mirror(&img.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "grayscale")]
    pub fn gray(img: &Image) -> Result<Image, JsValue> {
        imageops::grayscale(&img.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "posterize")]
    pub fn post(img: &Image, b: u8) -> Result<Image, JsValue> {
        imageops::posterize(&img.inner, b)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "solarize")]
    pub fn sol(img: &Image, t: u8) -> Result<Image, JsValue> {
        imageops::solarize(&img.inner, t)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "equalize")]
    pub fn eq(img: &Image) -> Result<Image, JsValue> {
        imageops::equalize(&img.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "autocontrast")]
    pub fn auto(img: &Image, c: f64) -> Result<Image, JsValue> {
        imageops::autocontrast(&img.inner, c)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "expand")]
    pub fn expand(img: &Image, border: u32, r: u8, g: u8, b: u8, a: u8) -> Result<Image, JsValue> {
        imageops::expand(&img.inner, border, (r, g, b, a))
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "contain")]
    pub fn contain(img: &Image, w: u32, h: u32) -> Result<Image, JsValue> {
        imageops::contain(&img.inner, w, h, None)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "cover")]
    pub fn cover(img: &Image, w: u32, h: u32) -> Result<Image, JsValue> {
        imageops::cover(&img.inner, w, h, None)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "fit")]
    pub fn fit(img: &Image, w: u32, h: u32) -> Result<Image, JsValue> {
        imageops::fit(&img.inner, w, h, None, 0.0, (0.5, 0.5))
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
        imageops::pad(&img.inner, w, h, None, color, (0.5, 0.5))
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "scale")]
    pub fn scale(img: &Image, factor: f64) -> Result<Image, JsValue> {
        imageops::scale(&img.inner, factor, None)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "crop")]
    pub fn crop_op(img: &Image, border: u32) -> Result<Image, JsValue> {
        imageops::crop(&img.inner, border)
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
        imageops::colorize(
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
    module_fns::merge(mode, &imgs)
        .map(|i| Image { inner: i })
        .map_err(err)
}
#[wasm_bindgen(js_name = "blend")]
pub fn blend(a: &Image, b: &Image, alpha: f64) -> Result<Image, JsValue> {
    module_fns::blend(&a.inner, &b.inner, alpha)
        .map(|i| Image { inner: i })
        .map_err(err)
}
#[wasm_bindgen(js_name = "composite")]
pub fn composite(a: &Image, b: &Image, m: &Image) -> Result<Image, JsValue> {
    module_fns::composite(&a.inner, &b.inner, &m.inner)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "effectNoise")]
pub fn effect_noise_fn(w: u32, h: u32, sigma: f64) -> Result<Image, JsValue> {
    let img = RsImage::new(w, h, "L", (0, 0, 0, 255)).map_err(err)?;
    module_fns::effect_noise(&img, sigma)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "eval")]
pub fn eval_fn(img: &Image, lut: Vec<u8>) -> Result<Image, JsValue> {
    module_fns::eval(&img.inner, &lut)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "imageOpenBytes")]
pub fn image_open_bytes(data: Vec<u8>) -> Result<Image, JsValue> {
    RsImage::open_bytes(data)
        .map(|i| Image { inner: i })
        .map_err(err)
}
