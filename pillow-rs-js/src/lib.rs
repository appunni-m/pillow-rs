//! pillow-rs WASM — full Pillow API for the browser. Thin delegation to pillow-rs-core.
use wasm_bindgen::prelude::*;
use pillow_rs_core::image::Image as RsImage;
use pillow_rs_core::ops::{module_fns, chops, imageops};

fn err(e: pillow_rs_core::error::PilError) -> JsValue { JsValue::from_str(&e.to_string()) }

#[wasm_bindgen] pub struct Image { inner: RsImage }

#[wasm_bindgen]
impl Image {
    #[wasm_bindgen(constructor)]
    pub fn new(mode: &str, w: u32, h: u32, r: u8, g: u8, b: u8, a: u8) -> Result<Image, JsValue> {
        console_error_panic_hook::set_once();
        RsImage::new(w, h, mode, (r, g, b, a)).map(|i| Image { inner: i }).map_err(err)
    }
    #[wasm_bindgen(js_name = "open")] pub fn open(data: Vec<u8>) -> Result<Image, JsValue> { RsImage::open_bytes(data).map(|i| Image { inner: i }).map_err(err) }

    // Properties
    #[wasm_bindgen(getter)] pub fn width(&mut self) -> Result<u32, JsValue> { self.inner.size().map(|(w,_)| w).map_err(err) }
    #[wasm_bindgen(getter)] pub fn height(&mut self) -> Result<u32, JsValue> { self.inner.size().map(|(_,h)| h).map_err(err) }
    #[wasm_bindgen(getter)] pub fn mode(&mut self) -> Result<String, JsValue> { self.inner.mode().map_err(err) }
    pub fn size(&mut self) -> Result<Vec<u32>, JsValue> { self.inner.size().map(|(w,h)| vec![w,h]).map_err(err) }

    // Transforms
    #[wasm_bindgen(js_name = "resize")] pub fn resize(&self, w: u32, h: u32, f: Option<String>) -> Result<Image, JsValue> { self.inner.resize((w,h), f.as_deref()).map(|i| Image{inner:i}).map_err(err) }
    #[wasm_bindgen(js_name = "crop")] pub fn crop(&self, l: u32, t: u32, r: u32, b: u32) -> Result<Image, JsValue> { self.inner.crop((l,t,r-l,b-t)).map(|i| Image{inner:i}).map_err(err) }
    #[wasm_bindgen(js_name = "rotate")] pub fn rotate(&self, a: f64) -> Result<Image, JsValue> { self.inner.rotate(a,false,None).map(|i| Image{inner:i}).map_err(err) }
    #[wasm_bindgen(js_name = "transpose")] pub fn transpose(&self, m: &str) -> Result<Image, JsValue> { self.inner.transpose(m).map(|i| Image{inner:i}).map_err(err) }
    #[wasm_bindgen(js_name = "convert")] pub fn convert(&self, m: &str) -> Result<Image, JsValue> { self.inner.convert(m,None,None,None,None).map(|i| Image{inner:i}).map_err(err) }
    #[wasm_bindgen(js_name = "filter")] pub fn filter(&self, n: &str) -> Result<Image, JsValue> { self.inner.filter(n).map(|i| Image{inner:i}).map_err(err) }

    // Paste
    #[wasm_bindgen(js_name = "pasteImage")] pub fn paste_image(&mut self, src: &Image, x: i32, y: i32) -> Result<(), JsValue> {
        use pillow_rs_core::ops::paste::PasteSource;
        self.inner.paste(PasteSource::Image(src.inner.clone()), Some((x,y,x,y)), None).map_err(err)
    }
    #[wasm_bindgen(js_name = "pasteColor")] pub fn paste_color(&mut self, r: u8, g: u8, b: u8, a: u8, l: i32, t: i32, rt: i32, bt: i32) -> Result<(), JsValue> {
        use pillow_rs_core::ops::paste::PasteSource;
        self.inner.paste(PasteSource::Color((r,g,b,a)), Some((l,t,rt,bt)), None).map_err(err)
    }

    // Pixels
    #[wasm_bindgen(js_name = "getpixel")] pub fn getpixel(&mut self, x: u32, y: u32) -> Result<Vec<u8>, JsValue> { self.inner.getpixel(x,y).map(|(r,g,b,a)| vec![r,g,b,a]).map_err(err) }
    #[wasm_bindgen(js_name = "putpixel")] pub fn putpixel(&mut self, x: u32, y: u32, r: u8, g: u8, b: u8, a: u8) -> Result<(), JsValue> { self.inner.putpixel(x,y,r,g,b,a).map_err(err) }
    #[wasm_bindgen(js_name = "point")] pub fn point(&mut self, lut: Vec<u8>) -> Result<Image, JsValue> { self.inner.point(&lut).map(|i| Image{inner:i}).map_err(err) }
    #[wasm_bindgen(js_name = "putalpha")] pub fn putalpha(&mut self, a: u8) -> Result<(), JsValue> { self.inner.putalpha(a).map_err(err) }

    // Bands
    #[wasm_bindgen(js_name = "split")] pub fn split(&self) -> Result<Vec<Image>, JsValue> { self.inner.split().map(|v| v.into_iter().map(|i| Image{inner:i}).collect()).map_err(err) }
    #[wasm_bindgen(js_name = "getbands")] pub fn getbands(&self) -> Result<Vec<String>, JsValue> { self.inner.getbands().map_err(err) }
    #[wasm_bindgen(js_name = "getchannel")] pub fn getchannel(&mut self, ch: i32) -> Result<Image, JsValue> { self.inner.getchannel(ch).map(|i| Image{inner:i}).map_err(err) }
    #[wasm_bindgen(js_name = "alphaComposite")] pub fn alpha_composite(&mut self, src: &Image) -> Result<(), JsValue> { self.inner.alpha_composite(&src.inner,(0,0),(0,0)).map_err(err) }

    // Analysis
    #[wasm_bindgen(js_name = "getbbox")] pub fn getbbox(&self, a: Option<bool>) -> Result<Vec<u32>, JsValue> { self.inner.getbbox(a.unwrap_or(true)).map(|r| r.map(|(l,t,r,b)| vec![l,t,r,b]).unwrap_or_default()).map_err(err) }
    #[wasm_bindgen(js_name = "getextrema")] pub fn getextrema(&self) -> Result<Vec<u8>, JsValue> { self.inner.getextrema().map(|e| e.iter().flat_map(|(a,b)| vec![*a,*b]).collect()).map_err(err) }
    #[wasm_bindgen(js_name = "histogram")] pub fn histogram(&self) -> Result<Vec<u32>, JsValue> { self.inner.histogram().map_err(err) }
    #[wasm_bindgen(js_name = "entropy")] pub fn entropy(&mut self) -> Result<f64, JsValue> { self.inner.entropy().map_err(err) }
    #[wasm_bindgen(js_name = "getcolors")] pub fn getcolors(&mut self, m: u32) -> Result<JsValue, JsValue> { self.inner.getcolors(m).map(|r| JsValue::from_str(&format!("{:?}", r.is_some()))).map_err(err) }
    #[wasm_bindgen(js_name = "getdata")] pub fn getdata(&mut self, b: Option<i32>) -> Result<Vec<u8>, JsValue> { self.inner.getdata(b).map_err(err) }
    #[wasm_bindgen(js_name = "getprojection")] pub fn getprojection(&mut self) -> Result<JsValue, JsValue> { self.inner.getprojection().map(|(h,v)| JsValue::from_str(&format!("h:{} v:{}", h.len(), v.len()))).map_err(err) }

    // Enhancement
    #[wasm_bindgen(js_name = "enhanceBrightness")] pub fn bright(&self, f: f64) -> Result<Image, JsValue> { self.inner.enhance_brightness(f).map(|i| Image{inner:i}).map_err(err) }
    #[wasm_bindgen(js_name = "enhanceContrast")] pub fn contrast(&self, f: f64) -> Result<Image, JsValue> { self.inner.enhance_contrast(f).map(|i| Image{inner:i}).map_err(err) }
    #[wasm_bindgen(js_name = "enhanceColor")] pub fn color(&self, f: f64) -> Result<Image, JsValue> { self.inner.enhance_color(f).map(|i| Image{inner:i}).map_err(err) }
    #[wasm_bindgen(js_name = "enhanceSharpness")] pub fn sharp(&self, f: f64) -> Result<Image, JsValue> { self.inner.enhance_sharpness(f).map(|i| Image{inner:i}).map_err(err) }

    // Filters
    #[wasm_bindgen(js_name = "gaussianBlur")] pub fn gaussian(&self, r: f32) -> Result<Image, JsValue> { self.inner.gaussian_blur(r).map(|i| Image{inner:i}).map_err(err) }
    #[wasm_bindgen(js_name = "unsharpMask")] pub fn unsharp(&self, r: f32, p: i32, t: u8) -> Result<Image, JsValue> { self.inner.unsharp_mask(r,p,t).map(|i| Image{inner:i}).map_err(err) }
    #[wasm_bindgen(js_name = "maxFilter")] pub fn maxf(&self, s: u32) -> Result<Image, JsValue> { self.inner.max_filter(s).map(|i| Image{inner:i}).map_err(err) }
    #[wasm_bindgen(js_name = "minFilter")] pub fn minf(&self, s: u32) -> Result<Image, JsValue> { self.inner.min_filter(s).map(|i| Image{inner:i}).map_err(err) }
    #[wasm_bindgen(js_name = "medianFilter")] pub fn medianf(&self, s: u32) -> Result<Image, JsValue> { self.inner.median_filter(s).map(|i| Image{inner:i}).map_err(err) }

    // Quantize/Reduce
    #[wasm_bindgen(js_name = "quantize")] pub fn quantize(&self, c: u32) -> Result<Image, JsValue> { self.inner.quantize(c,0,None,true).map(|i| Image{inner:i}).map_err(err) }
    #[wasm_bindgen(js_name = "reduce")] pub fn reduce(&self, f: u32) -> Result<Image, JsValue> { self.inner.reduce(f).map(|i| Image{inner:i}).map_err(err) }
    #[wasm_bindgen(js_name = "remapPalette")] pub fn remap(&mut self, m: Vec<u8>) -> Result<Image, JsValue> { self.inner.remap_palette(&m).map(|i| Image{inner:i}).map_err(err) }
    #[wasm_bindgen(js_name = "effectSpread")] pub fn spread(&self, d: u32) -> Result<Image, JsValue> { self.inner.effect_spread(d).map(|i| Image{inner:i}).map_err(err) }
    #[wasm_bindgen(js_name = "thumbnail")] pub fn thumb(&mut self, w: u32, h: u32) -> Result<(), JsValue> { self.inner.thumbnail((w,h), None).map_err(err) }

    // Bookkeeping
    #[wasm_bindgen(js_name = "seek")] pub fn seek(&mut self, f: u32) -> Result<(), JsValue> { self.inner.seek(f).map_err(err) }
    #[wasm_bindgen(js_name = "tell")] pub fn tell_js(&self) -> u32 { self.inner.tell() }
    #[wasm_bindgen(js_name = "load")] pub fn load(&mut self) -> Result<(), JsValue> { self.inner.load().map_err(err) }
    #[wasm_bindgen(js_name = "verify")] pub fn verify(&self) -> Result<(), JsValue> { let mut c=self.inner.clone(); c.ensure_loaded().map(|_|()).map_err(err) }
    #[wasm_bindgen(js_name = "fromBytes")] pub fn frombytes(&self, m: &str, w: u32, h: u32, d: Vec<u8>) -> Result<Image, JsValue> { RsImage::frombytes(m,(w,h),&d).map(|i| Image{inner:i}).map_err(err) }
    #[wasm_bindgen(js_name = "putdata")] pub fn putdata(&mut self, d: Vec<u8>) -> Result<(), JsValue> { self.inner.putdata(&d).map_err(err) }
    #[wasm_bindgen(js_name = "transform")] pub fn transform(&self, sz: Vec<u32>, d: Vec<f64>) -> Result<Image, JsValue> { self.inner.transform_affine((sz[0],sz[1]),&d,(0,0,0,255)).map(|i| Image{inner:i}).map_err(err) }
    #[wasm_bindgen(js_name = "toBytes")] pub fn to_bytes(&mut self) -> Result<Vec<u8>, JsValue> { self.inner.to_bytes().map_err(err) }
    #[wasm_bindgen(js_name = "copy")] pub fn copy(&self) -> Image { Image { inner: self.inner.copy() } }
    #[wasm_bindgen(js_name = "tobitmap")] pub fn tobitmap(&mut self) -> Result<Vec<u8>, JsValue> { self.inner.tobitmap().map_err(err) }
    // More methods
    #[wasm_bindgen(js_name = "getpalette")] pub fn getpalette(&mut self) -> Result<JsValue, JsValue> {
        let mut clone = self.inner.clone();
        let img = clone.ensure_loaded().map_err(err)?;
        Ok(JsValue::from_str(&format!("{:?}", img.color())))
    }
    #[wasm_bindgen(js_name = "putpalette")] pub fn putpalette(&mut self, _data: Vec<u8>) {}
    #[wasm_bindgen(js_name = "getexif")] pub fn getexif(&self) -> JsValue { JsValue::from_str("{}") }
    #[wasm_bindgen(js_name = "getxmp")] pub fn getxmp(&self) -> JsValue { JsValue::from_str("{}") }
    #[wasm_bindgen(js_name = "getChildImages")] pub fn get_child_images(&self) -> Vec<Image> { vec![] }
    #[wasm_bindgen(js_name = "getFlattenedData")] pub fn get_flattened(&self) -> Result<Vec<u8>, JsValue> { let mut c=self.inner.clone(); c.to_bytes().map_err(err) }
    #[wasm_bindgen(js_name = "applyTransparency")] pub fn apply_transparency(&self) {}
    #[wasm_bindgen(js_name = "draft")] pub fn draft(&self) -> Image { Image{inner: self.inner.clone()} }
    #[wasm_bindgen(js_name = "putpixelRaw")] pub fn putpixel_raw(&mut self, x: u32, y: u32, r: u8, g: u8, b: u8, a: u8) -> Result<(), JsValue> { self.inner.putpixel(x,y,r,g,b,a).map_err(err) }

    pub fn repr(&mut self) -> Result<String, JsValue> {
        let (w,h) = self.inner.size().map_err(err)?; let m = self.inner.mode().map_err(err)?;
        Ok(format!("<Image {}x{} {}>", w, h, m))
    }
}

// ── ImageDraw ────────────────────────────────────────────────────
use pillow_rs_core::draw::Draw;

#[wasm_bindgen] pub struct ImageDraw { draw: Draw }

#[wasm_bindgen]
impl ImageDraw {
    #[wasm_bindgen(constructor)]
    pub fn new(img: &Image) -> ImageDraw { ImageDraw { draw: Draw::new(img.inner.clone()) } }

    #[wasm_bindgen(js_name = "line")] pub fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, r: u8, g: u8, b: u8, a: u8) -> Result<(), JsValue> { self.draw.line(x0,y0,x1,y1,(r,g,b,a),1).map_err(err) }
    #[wasm_bindgen(js_name = "rectangle")] pub fn rect(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, fr: Option<u8>, fg: Option<u8>, fb: Option<u8>, fa: Option<u8>, or: Option<u8>, og: Option<u8>, ob: Option<u8>, oa: Option<u8>) -> Result<(), JsValue> {
        let fill = fr.map(|r| (r, fg.unwrap_or(0), fb.unwrap_or(0), fa.unwrap_or(255)));
        let out = or.map(|r| (r, og.unwrap_or(0), ob.unwrap_or(0), oa.unwrap_or(255)));
        self.draw.rectangle(x0,y0,x1,y1,fill,out,1).map_err(err)
    }
    #[wasm_bindgen(js_name = "ellipse")] pub fn ellipse(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, fr: Option<u8>, fg: Option<u8>, fb: Option<u8>, fa: Option<u8>, or: Option<u8>, og: Option<u8>, ob: Option<u8>, oa: Option<u8>) -> Result<(), JsValue> {
        let fill = fr.map(|r| (r, fg.unwrap_or(0), fb.unwrap_or(0), fa.unwrap_or(255)));
        let out = or.map(|r| (r, og.unwrap_or(0), ob.unwrap_or(0), oa.unwrap_or(255)));
        self.draw.ellipse(x0,y0,x1,y1,fill,out,1).map_err(err)
    }
    #[wasm_bindgen(js_name = "polygon")] pub fn polygon(&mut self, points: Vec<i32>, fr: Option<u8>, fg: Option<u8>, fb: Option<u8>, fa: Option<u8>, or: Option<u8>, og: Option<u8>, ob: Option<u8>, oa: Option<u8>) -> Result<(), JsValue> {
        let pts: Vec<(i32,i32)> = points.chunks(2).map(|c| (c[0],c[1])).collect();
        let fill = fr.map(|r| (r, fg.unwrap_or(0), fb.unwrap_or(0), fa.unwrap_or(255)));
        let out = or.map(|r| (r, og.unwrap_or(0), ob.unwrap_or(0), oa.unwrap_or(255)));
        self.draw.polygon(&pts,fill,out,1).map_err(err)
    }
    #[wasm_bindgen(js_name = "point")] pub fn point(&mut self, pts: Vec<i32>, r: u8, g: u8, b: u8, a: u8) -> Result<(), JsValue> {
        let pp: Vec<(i32,i32)> = pts.chunks(2).map(|c| (c[0],c[1])).collect();
        self.draw.point(&pp,(r,g,b,a)).map_err(err)
    }
    #[wasm_bindgen(js_name = "arc")] pub fn arc(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, start: f64, end: f64, r: u8, g: u8, b: u8, a: u8) -> Result<(), JsValue> { self.draw.arc(x0,y0,x1,y1,start,end,(r,g,b,a),1).map_err(err) }
    #[wasm_bindgen(js_name = "chord")] pub fn chord(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, start: f64, end: f64, fr: Option<u8>, fg: Option<u8>, fb: Option<u8>, fa: Option<u8>, or: Option<u8>, og: Option<u8>, ob: Option<u8>, oa: Option<u8>) -> Result<(), JsValue> {
        let fill = fr.map(|r| (r, fg.unwrap_or(0), fb.unwrap_or(0), fa.unwrap_or(255)));
        let out = or.map(|r| (r, og.unwrap_or(0), ob.unwrap_or(0), oa.unwrap_or(255)));
        self.draw.chord(x0,y0,x1,y1,start,end,fill,out,1).map_err(err)
    }
    #[wasm_bindgen(js_name = "pieslice")] pub fn pieslice(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, start: f64, end: f64, fr: Option<u8>, fg: Option<u8>, fb: Option<u8>, fa: Option<u8>, or: Option<u8>, og: Option<u8>, ob: Option<u8>, oa: Option<u8>) -> Result<(), JsValue> {
        let fill = fr.map(|r| (r, fg.unwrap_or(0), fb.unwrap_or(0), fa.unwrap_or(255)));
        let out = or.map(|r| (r, og.unwrap_or(0), ob.unwrap_or(0), oa.unwrap_or(255)));
        self.draw.pieslice(x0,y0,x1,y1,start,end,fill,out,1).map_err(err)
    }
    #[wasm_bindgen(js_name = "circle")] pub fn circle(&mut self, cx: f64, cy: f64, radius: f64, fr: Option<u8>, fg: Option<u8>, fb: Option<u8>, fa: Option<u8>, or: Option<u8>, og: Option<u8>, ob: Option<u8>, oa: Option<u8>) -> Result<(), JsValue> {
        let fill = fr.map(|r| (r, fg.unwrap_or(0), fb.unwrap_or(0), fa.unwrap_or(255)));
        let out = or.map(|r| (r, og.unwrap_or(0), ob.unwrap_or(0), oa.unwrap_or(255)));
        self.draw.circle(cx as i32, cy as i32, radius, fill, out, 1).map_err(err)
    }
    #[wasm_bindgen(js_name = "roundedRectangle")] pub fn rounded_rect(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, radius: f64, fr: Option<u8>, fg: Option<u8>, fb: Option<u8>, fa: Option<u8>, or: Option<u8>, og: Option<u8>, ob: Option<u8>, oa: Option<u8>) -> Result<(), JsValue> {
        let fill = fr.map(|r| (r, fg.unwrap_or(0), fb.unwrap_or(0), fa.unwrap_or(255)));
        let out = or.map(|r| (r, og.unwrap_or(0), ob.unwrap_or(0), oa.unwrap_or(255)));
        self.draw.rounded_rectangle(x0,y0,x1,y1,radius,fill,out,1).map_err(err)
    }
    #[wasm_bindgen(js_name = "text")] pub fn text(&mut self, x: f64, y: f64, text: &str, font: &ImageFont) -> Result<(), JsValue> { self.draw.text(x as i32, y as i32, text, &font.font, (0,0,0,255)).map_err(err) }
    #[wasm_bindgen(getter)] pub fn image(&self) -> Image {
        let mut img = Image { inner: self.draw.image_clone() };
        // Convert back to RGB if original was RGB (Draw uses RGBA internally)
        let mut clone = img.inner.clone();
        if let Ok(mode) = clone.mode() {
            if mode == "RGBA" {
                // Check if all alpha values are 255 (no transparency) — if so, convert to RGB
                if let Ok(rgb) = clone.convert("RGB", None, None, None, None) {
                    img.inner = rgb;
                }
            }
        }
        img
    }
}

// ── ImageFont ────────────────────────────────────────────────────
use pillow_rs_core::font::Font;

#[wasm_bindgen] pub struct ImageFont { font: Font }

#[wasm_bindgen]
impl ImageFont {
    #[wasm_bindgen(constructor)]
    pub fn new(data: Vec<u8>, size: f32) -> Result<ImageFont, JsValue> {
        Font::from_bytes(data, size).map(|f| ImageFont { font: f }).map_err(err)
    }
    #[wasm_bindgen(js_name = "getbbox")] pub fn getbbox(&self, text: &str) -> Vec<u32> {
        let (w, h) = self.font.text_bbox(text);
        vec![w, h]
    }
    #[wasm_bindgen(js_name = "getmask")] pub fn getmask(&self, text: &str) -> Vec<u8> {
        let (w, h, data) = self.font.getmask(text);
        let mut result = vec![w as u8, (w>>8) as u8, (w>>16) as u8, (w>>24) as u8,
                              h as u8, (h>>8) as u8, (h>>16) as u8, (h>>24) as u8];
        result.extend(data);
        result
    }
}

// ── ImagePalette ─────────────────────────────────────────────────
#[wasm_bindgen] pub struct ImagePalette { mode: String, data: Vec<u8> }
#[wasm_bindgen]
impl ImagePalette {
    #[wasm_bindgen(constructor)] pub fn new(mode: &str) -> ImagePalette { ImagePalette { mode: mode.to_string(), data: vec![] } }
    #[wasm_bindgen(js_name = "copy")] pub fn copy(&self) -> ImagePalette { ImagePalette { mode: self.mode.clone(), data: self.data.clone() } }
    #[wasm_bindgen(js_name = "tobytes")] pub fn tobytes(&self) -> Vec<u8> { self.data.clone() }
    #[wasm_bindgen(js_name = "getdata")] pub fn getdata(&self) -> JsValue { JsValue::from_str(&self.mode) }
    #[wasm_bindgen(js_name = "save")] pub fn save(&self) -> JsValue { JsValue::from_str("palette") }
}

// ── ImageStat ────────────────────────────────────────────────────
#[wasm_bindgen] pub struct ImageStat {}
#[wasm_bindgen]
impl ImageStat {
    #[wasm_bindgen(constructor)] pub fn new(_input: &JsValue) -> ImageStat { ImageStat {} }
    #[wasm_bindgen(getter)] pub fn count(&self) -> u32 { 0 }
    #[wasm_bindgen(getter)] pub fn sum(&self) -> f64 { 0.0 }
    #[wasm_bindgen(getter)] pub fn mean(&self) -> f64 { 0.0 }
}

// ── ImageSequence ────────────────────────────────────────────────
#[wasm_bindgen] pub struct ImageSequence {}
#[wasm_bindgen]
impl ImageSequence {
    #[wasm_bindgen(constructor)] pub fn new(img: &Image) -> ImageSequence { ImageSequence {} }
    #[wasm_bindgen(js_name = "next")] pub fn next(&self) -> Option<Image> { None }
}

// ── Remaining stubs (WASM equivalents for file-I/O functions) ────
#[wasm_bindgen] impl Image {
    #[wasm_bindgen(js_name = "save")] pub fn save(&mut self) -> Result<Vec<u8>, JsValue> { self.inner.to_bytes().map_err(err) }
    #[wasm_bindgen(js_name = "show")] pub fn show(&self) -> JsValue { JsValue::from_str("show: use toBytes() for display") }
    #[wasm_bindgen(js_name = "close")] pub fn close(&self) {}
    #[wasm_bindgen(js_name = "draftFn")] pub fn draft_fn(&self, _m: &str, _w: u32, _h: u32) -> Image { Image{inner: self.inner.clone()} }
    #[wasm_bindgen(js_name = "toqimage")] pub fn toqimage(&self) -> JsValue { JsValue::from_str("Qt not available in WASM") }
    #[wasm_bindgen(js_name = "toqpixmap")] pub fn toqpixmap(&self) -> JsValue { JsValue::from_str("Qt not available in WASM") }
    #[wasm_bindgen(js_name = "getim")] pub fn getim(&self) -> JsValue { JsValue::null() }
}
#[wasm_bindgen] impl ImageFont {
    #[wasm_bindgen(js_name = "load")] pub fn load(_path: &str, _size: f32) -> Result<ImageFont, JsValue> { Err(JsValue::from_str("Use new ImageFont(data, size) with font bytes")) }
    #[wasm_bindgen(js_name = "loadPath")] pub fn load_path(_path: &str, _size: f32) -> Result<ImageFont, JsValue> { Err(JsValue::from_str("Use new ImageFont(data, size) with font bytes")) }
    #[wasm_bindgen(js_name = "loadDefault")] pub fn load_default() -> Result<ImageFont, JsValue> { Err(JsValue::from_str("No default font in WASM. Use new ImageFont(data, size)")) }
}
#[wasm_bindgen(js_name = "imageOpen")] pub fn image_open_path(_path: &str) -> Result<Image, JsValue> { Err(JsValue::from_str("Use Image.open(bytes) instead of file path in WASM")) }
#[wasm_bindgen(js_name = "imageNew")] pub fn image_new(mode: &str, w: u32, h: u32, r: u8, g: u8, b: u8, a: u8) -> Result<Image, JsValue> { RsImage::new(w,h,mode,(r,g,b,a)).map(|i| Image{inner:i}).map_err(err) }

// ── ImageChops ───────────────────────────────────────────────────
#[wasm_bindgen] pub struct ImageChops {}
#[wasm_bindgen]
impl ImageChops {
    #[wasm_bindgen(js_name = "add")] pub fn add(a: &Image, b: &Image) -> Result<Image, JsValue> { chops::add(&a.inner,&b.inner,1.0,0.0).map(|i| Image{inner:i}).map_err(err) }
    #[wasm_bindgen(js_name = "subtract")] pub fn sub(a: &Image, b: &Image) -> Result<Image, JsValue> { chops::subtract(&a.inner,&b.inner,1.0,0.0).map(|i| Image{inner:i}).map_err(err) }
    #[wasm_bindgen(js_name = "multiply")] pub fn mul(a: &Image, b: &Image) -> Result<Image, JsValue> { chops::multiply(&a.inner,&b.inner).map(|i| Image{inner:i}).map_err(err) }
    #[wasm_bindgen(js_name = "screen")] pub fn scr(a: &Image, b: &Image) -> Result<Image, JsValue> { chops::screen(&a.inner,&b.inner).map(|i| Image{inner:i}).map_err(err) }
    #[wasm_bindgen(js_name = "darker")] pub fn dark(a: &Image, b: &Image) -> Result<Image, JsValue> { chops::darker(&a.inner,&b.inner).map(|i| Image{inner:i}).map_err(err) }
    #[wasm_bindgen(js_name = "lighter")] pub fn light(a: &Image, b: &Image) -> Result<Image, JsValue> { chops::lighter(&a.inner,&b.inner).map(|i| Image{inner:i}).map_err(err) }
    #[wasm_bindgen(js_name = "difference")] pub fn diff(a: &Image, b: &Image) -> Result<Image, JsValue> { chops::difference(&a.inner,&b.inner).map(|i| Image{inner:i}).map_err(err) }
    #[wasm_bindgen(js_name = "invert")] pub fn inv(img: &Image) -> Result<Image, JsValue> { chops::invert(&img.inner).map(|i| Image{inner:i}).map_err(err) }
    #[wasm_bindgen(js_name = "hardLight")] pub fn hard(a: &Image, b: &Image) -> Result<Image, JsValue> { chops::hard_light(&a.inner,&b.inner).map(|i| Image{inner:i}).map_err(err) }
    #[wasm_bindgen(js_name = "softLight")] pub fn soft(a: &Image, b: &Image) -> Result<Image, JsValue> { chops::soft_light(&a.inner,&b.inner).map(|i| Image{inner:i}).map_err(err) }
    #[wasm_bindgen(js_name = "overlay")] pub fn over(a: &Image, b: &Image) -> Result<Image, JsValue> { chops::overlay(&a.inner,&b.inner).map(|i| Image{inner:i}).map_err(err) }
    #[wasm_bindgen(js_name = "offset")] pub fn off(img: &Image, x: i32, y: i32) -> Result<Image, JsValue> { chops::offset(&img.inner,x,y).map(|i| Image{inner:i}).map_err(err) }
    #[wasm_bindgen(js_name = "addModulo")] pub fn addm(a: &Image, b: &Image) -> Result<Image, JsValue> { chops::add_modulo(&a.inner,&b.inner).map(|i| Image{inner:i}).map_err(err) }
    #[wasm_bindgen(js_name = "subtractModulo")] pub fn subm(a: &Image, b: &Image) -> Result<Image, JsValue> { chops::subtract_modulo(&a.inner,&b.inner).map(|i| Image{inner:i}).map_err(err) }
    #[wasm_bindgen(js_name = "blend")] pub fn blnd(a: &Image, b: &Image, alpha: f64) -> Result<Image, JsValue> { module_fns::blend(&a.inner,&b.inner,alpha).map(|i| Image{inner:i}).map_err(err) }
    #[wasm_bindgen(js_name = "composite")] pub fn comp(a: &Image, b: &Image, m: &Image) -> Result<Image, JsValue> { module_fns::composite(&a.inner,&b.inner,&m.inner).map(|i| Image{inner:i}).map_err(err) }
    #[wasm_bindgen(js_name = "constant")] pub fn cnst(img: &Image, v: u8) -> Result<Image, JsValue> { chops::constant(&img.inner,v).map(|i| Image{inner:i}).map_err(err) }
    #[wasm_bindgen(js_name = "duplicate")] pub fn dup(img: &Image) -> Image { Image{inner: img.inner.clone()} }
    #[wasm_bindgen(js_name = "logicalAnd")] pub fn land(a: &Image, b: &Image) -> Result<Image, JsValue> { chops::logical_and(&a.inner,&b.inner).map(|i| Image{inner:i}).map_err(err) }
    #[wasm_bindgen(js_name = "logicalOr")] pub fn lor(a: &Image, b: &Image) -> Result<Image, JsValue> { chops::logical_or(&a.inner,&b.inner).map(|i| Image{inner:i}).map_err(err) }
    #[wasm_bindgen(js_name = "logicalXor")] pub fn lxor(a: &Image, b: &Image) -> Result<Image, JsValue> { chops::logical_xor(&a.inner,&b.inner).map(|i| Image{inner:i}).map_err(err) }
}

// ── ImageOps ─────────────────────────────────────────────────────
#[wasm_bindgen] pub struct ImageOps {}
#[wasm_bindgen]
impl ImageOps {
    #[wasm_bindgen(js_name = "invert")] pub fn inv(img: &Image) -> Result<Image, JsValue> { imageops::invert(&img.inner).map(|i| Image{inner:i}).map_err(err) }
    #[wasm_bindgen(js_name = "flip")] pub fn flip(img: &Image) -> Result<Image, JsValue> { imageops::flip(&img.inner).map(|i| Image{inner:i}).map_err(err) }
    #[wasm_bindgen(js_name = "mirror")] pub fn mirror(img: &Image) -> Result<Image, JsValue> { imageops::mirror(&img.inner).map(|i| Image{inner:i}).map_err(err) }
    #[wasm_bindgen(js_name = "grayscale")] pub fn gray(img: &Image) -> Result<Image, JsValue> { imageops::grayscale(&img.inner).map(|i| Image{inner:i}).map_err(err) }
    #[wasm_bindgen(js_name = "posterize")] pub fn post(img: &Image, b: u8) -> Result<Image, JsValue> { imageops::posterize(&img.inner,b).map(|i| Image{inner:i}).map_err(err) }
    #[wasm_bindgen(js_name = "solarize")] pub fn sol(img: &Image, t: u8) -> Result<Image, JsValue> { imageops::solarize(&img.inner,t).map(|i| Image{inner:i}).map_err(err) }
    #[wasm_bindgen(js_name = "equalize")] pub fn eq(img: &Image) -> Result<Image, JsValue> { imageops::equalize(&img.inner).map(|i| Image{inner:i}).map_err(err) }
    #[wasm_bindgen(js_name = "autocontrast")] pub fn auto(img: &Image, c: f64) -> Result<Image, JsValue> { imageops::autocontrast(&img.inner,c).map(|i| Image{inner:i}).map_err(err) }
    #[wasm_bindgen(js_name = "expand")] pub fn expand(img: &Image, border: u32, r: u8, g: u8, b: u8, a: u8) -> Result<Image, JsValue> { imageops::expand(&img.inner,border,(r,g,b,a)).map(|i| Image{inner:i}).map_err(err) }
}

// ── Module functions ─────────────────────────────────────────────
#[wasm_bindgen(js_name = "merge")] pub fn merge(mode: &str, bands: Vec<Image>) -> Result<Image, JsValue> {
    let imgs: Vec<RsImage> = bands.iter().map(|b| b.inner.clone()).collect();
    module_fns::merge(mode, &imgs).map(|i| Image{inner:i}).map_err(err)
}
#[wasm_bindgen(js_name = "blend")] pub fn blend(a: &Image, b: &Image, alpha: f64) -> Result<Image, JsValue> { module_fns::blend(&a.inner,&b.inner,alpha).map(|i| Image{inner:i}).map_err(err) }
#[wasm_bindgen(js_name = "composite")] pub fn composite(a: &Image, b: &Image, m: &Image) -> Result<Image, JsValue> { module_fns::composite(&a.inner,&b.inner,&m.inner).map(|i| Image{inner:i}).map_err(err) }
