use crate::Image;
use crate::inputs::*;
use crate::values::*;
use wasm_bindgen::prelude::{JsValue, wasm_bindgen};

#[wasm_bindgen]
impl Image {
    #[wasm_bindgen(js_name = "pasteImage")]
    pub fn paste_image(&mut self, src: &Image, x: i32, y: i32) -> Result<(), JsValue> {
        use pillow_rs::PasteSource;
        self.inner
            .paste_at(
                PasteSource::Image(Box::new(src.inner.clone())),
                Some((x, y)),
                None,
            )
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
                PasteSource::Image(Box::new(src.inner.clone())),
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
                PasteSource::Image(Box::new(src.inner.clone())),
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
                PasteSource::Image(Box::new(src.inner.clone())),
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
    #[wasm_bindgen(js_name = "getpixelFormatted")]
    pub fn getpixel_formatted(&self, x: u32, y: u32) -> Result<JsValue, JsValue> {
        self.inner
            .getpixel_formatted(x, y)
            .map(formatted_pixel_to_js)
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "putpixel")]
    pub fn putpixel(&mut self, x: u32, y: u32, r: u8, g: u8, b: u8, a: u8) -> Result<(), JsValue> {
        self.inner.putpixel(x, y, r, g, b, a).map_err(err)
    }
    #[wasm_bindgen(js_name = "putpixelValue")]
    pub fn putpixel_value(&mut self, x: u32, y: u32, value: JsValue) -> Result<(), JsValue> {
        self.inner
            .putpixel_value(x, y, js_putpixel_value(&value))
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "point")]
    pub fn point(&self, lut: Vec<u8>) -> Result<Image, JsValue> {
        pillow_rs::image_eval_validated(&self.inner, &lut)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    /// Applies the floating-output form of Pillow's `Image.point` operation.
    ///
    /// The ordinary `point` export remains byte-oriented for compatibility.
    /// Keep the output mode explicit here because a `Float64Array` is needed
    /// to preserve fractional LUT values across the WASM boundary.
    #[wasm_bindgen(js_name = "pointWithMode")]
    pub fn point_with_mode(&self, lut: Vec<f64>, mode: &str) -> Result<Image, JsValue> {
        if mode != "F" {
            return Err(JsValue::from_str(
                "only Image.point output mode 'F' is supported by this export",
            ));
        }
        pillow_rs::image_eval_float(&self.inner, &lut)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "pointWithTransform")]
    pub fn point_with_transform(&self, scale: f64, offset: f64) -> Result<Image, JsValue> {
        pillow_rs::image_eval_point_transform(&self.inner, scale, offset)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "putalpha")]
    pub fn putalpha(&mut self, a: u8) -> Result<(), JsValue> {
        self.inner.putalpha(a).map_err(err)
    }
    #[wasm_bindgen(js_name = "putalphaInput")]
    pub fn putalpha_input(&mut self, alpha: JsValue) -> Result<(), JsValue> {
        let input = if let Some(value) = alpha.as_f64() {
            if !value.is_finite() || value.fract() != 0.0 {
                pillow_rs::PutAlphaInput::Invalid("float".to_owned())
            } else {
                pillow_rs::PutAlphaInput::Integer(value as i64)
            }
        } else {
            pillow_rs::PutAlphaInput::Invalid(js_value_type_name(&alpha))
        };
        self.inner.putalpha_with_input(input).map_err(err)
    }
    #[wasm_bindgen(js_name = "putalphaImageInput")]
    pub fn putalpha_image_input(&mut self, alpha: &Image) -> Result<(), JsValue> {
        self.inner
            .putalpha_with_input(pillow_rs::PutAlphaInput::Image(Box::new(
                alpha.inner.clone(),
            )))
            .map_err(err)
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
    pub fn alpha_composite(
        &mut self,
        src: &Image,
        dest: Option<JsValue>,
        source: Option<JsValue>,
    ) -> Result<(), JsValue> {
        let normalize_box = |value: Option<&JsValue>, default: Vec<i64>| match value {
            None => pillow_rs::AlphaCompositeBox::Values(default),
            Some(value) if value.is_null() || value.is_undefined() => {
                pillow_rs::AlphaCompositeBox::Values(default)
            }
            Some(value) => js_integer_array(value).map_or(
                pillow_rs::AlphaCompositeBox::Invalid,
                pillow_rs::AlphaCompositeBox::Values,
            ),
        };
        self.inner
            .alpha_composite_public(
                &src.inner,
                normalize_box(dest.as_ref(), vec![0, 0]),
                normalize_box(source.as_ref(), vec![0, 0]),
            )
            .map_err(err)
    }

    // Analysis
    #[wasm_bindgen(js_name = "getbbox")]
    pub fn getbbox(&self, a: Option<bool>) -> Result<JsValue, JsValue> {
        self.inner
            .getbbox(a.unwrap_or(true))
            .map(|r| match r {
                Some((left, top, right, bottom)) => {
                    let value = js_sys::Array::new();
                    value.push(&JsValue::from(left));
                    value.push(&JsValue::from(top));
                    value.push(&JsValue::from(right));
                    value.push(&JsValue::from(bottom));
                    value.into()
                }
                None => JsValue::NULL,
            })
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
    #[wasm_bindgen(js_name = "getextremaFormatted")]
    pub fn getextrema_formatted(&self) -> Result<JsValue, JsValue> {
        self.inner
            .getextrema_formatted()
            .map(formatted_extrema_to_js)
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "histogram")]
    pub fn histogram(&self) -> Result<Vec<u32>, JsValue> {
        self.inner.histogram().map_err(err)
    }
    #[wasm_bindgen(js_name = "histogramWithInput")]
    pub fn histogram_with_input(&self, mask: &Image) -> Result<Vec<u32>, JsValue> {
        self.inner
            .histogram_with_input(pillow_rs::ImageAnalysisMask::Image(Box::new(
                mask.inner.clone(),
            )))
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "histogramInvalidInput")]
    pub fn histogram_invalid_input(&self, type_name: &str) -> Result<Vec<u32>, JsValue> {
        self.inner
            .histogram_with_input(pillow_rs::ImageAnalysisMask::Invalid(type_name.to_owned()))
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "entropy")]
    pub fn entropy(&mut self) -> Result<f64, JsValue> {
        self.inner.entropy().map_err(err)
    }
    #[wasm_bindgen(js_name = "entropyWithInput")]
    pub fn entropy_with_input(&mut self, mask: &Image) -> Result<f64, JsValue> {
        self.inner.entropy_with_mask(Some(&mask.inner)).map_err(err)
    }
    #[wasm_bindgen(js_name = "getcolors")]
    pub fn getcolors(&mut self, m: i32) -> Result<JsValue, JsValue> {
        match self.inner.getcolors_with_signed_limit(m).map_err(err)? {
            Some(colors) => {
                let arr = js_sys::Array::new();
                for (count, color) in &colors {
                    let entry = js_sys::Array::new();
                    entry.push(&JsValue::from(*count));
                    match color {
                        pillow_rs::FormattedPixelValue::Scalar(value) => {
                            entry.push(&JsValue::from(*value));
                        }
                        pillow_rs::FormattedPixelValue::Integer(value) => {
                            entry.push(&JsValue::from(*value));
                        }
                        pillow_rs::FormattedPixelValue::Float(value) => {
                            entry.push(&JsValue::from(*value));
                        }
                        pillow_rs::FormattedPixelValue::Components(values) => {
                            let color_arr = js_sys::Array::new();
                            for value in values {
                                color_arr.push(&JsValue::from(*value));
                            }
                            entry.push(&color_arr);
                        }
                        pillow_rs::FormattedPixelValue::InlineComponents { bytes, bands } => {
                            let color_arr = js_sys::Array::new();
                            for value in bytes.iter().copied().take(usize::from(*bands)) {
                                color_arr.push(&JsValue::from(value));
                            }
                            entry.push(&color_arr);
                        }
                    }
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
    #[wasm_bindgen(js_name = "getdataFormatted")]
    pub fn getdata_formatted(&self, b: Option<i32>) -> Result<JsValue, JsValue> {
        self.inner
            .getdata_formatted(b)
            .map(formatted_image_data_to_js)
            .map_err(err)
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
}
