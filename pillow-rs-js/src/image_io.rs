//! WASM image save and display bindings.

use super::*;
use pillow_rs::Image as RsImage;
use wasm_bindgen::JsCast;
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
        if !option_pairs.len().is_multiple_of(2) {
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

#[wasm_bindgen]
impl Image {
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
    #[wasm_bindgen(js_name = "fromBytesInPlace")]
    pub fn frombytes_in_place(
        &mut self,
        m: &str,
        w: u32,
        h: u32,
        d: Vec<u8>,
    ) -> Result<(), JsValue> {
        self.inner = RsImage::frombytes(m, (w, h), &d).map_err(err)?;
        Ok(())
    }
    #[wasm_bindgen(js_name = "pasteValue")]
    pub fn paste_value(
        &mut self,
        source: JsValue,
        box_value: Option<JsValue>,
        mask: Option<JsValue>,
    ) -> Result<(), JsValue> {
        self.inner
            .paste_with_input(
                js_paste_source(&source),
                js_paste_box(box_value.as_ref()),
                js_paste_mask(mask.as_ref()),
            )
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "pasteValueMasked")]
    pub fn paste_value_masked(
        &mut self,
        source: JsValue,
        box_value: Option<JsValue>,
        mask: &Image,
    ) -> Result<(), JsValue> {
        self.inner
            .paste_with_input(
                js_paste_source(&source),
                js_paste_box(box_value.as_ref()),
                pillow_rs::PythonPasteMask::Image(Box::new(mask.inner.clone())),
            )
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "putdataValues")]
    pub fn putdata_values(
        &mut self,
        data: JsValue,
        scale: Option<f64>,
        offset: Option<f64>,
    ) -> Result<(), JsValue> {
        if data.is_instance_of::<js_sys::Uint8Array>() {
            let bytes = js_sys::Uint8Array::new(&data).to_vec();
            let scale = scale.unwrap_or(1.0);
            let offset = offset.unwrap_or(0.0);
            if self
                .inner
                .putdata_bytes_fast_path(&bytes, bytes.len(), scale, offset)
                .map_err(err)?
            {
                return Ok(());
            }
            let kind = self.inner.putdata_value_kind().map_err(err)?;
            let values = bytes
                .into_iter()
                .map(|value| match kind {
                    pillow_rs::PutDataValueKind::Numeric => {
                        pillow_rs::PutDataValue::Number(f64::from(value))
                    }
                    pillow_rs::PutDataValueKind::Components { .. } => {
                        pillow_rs::PutDataValue::Packed(i64::from(value))
                    }
                })
                .collect::<Vec<_>>();
            return self
                .inner
                .putdata_values(&values, scale, offset)
                .map_err(err);
        }
        if !data.is_array() {
            return Err(err(pillow_rs::PilError::TypeError(
                "argument must be a sequence".to_owned(),
            )));
        }
        let kind = self.inner.putdata_value_kind().map_err(err)?;
        let values = js_sys::Array::from(&data)
            .iter()
            .map(|value| {
                if let Some(number) = value.as_f64() {
                    return match kind {
                        pillow_rs::PutDataValueKind::Numeric => {
                            Ok(pillow_rs::PutDataValue::Number(number))
                        }
                        pillow_rs::PutDataValueKind::Components { .. }
                            if number.is_finite() && number.fract() == 0.0 =>
                        {
                            Ok(pillow_rs::PutDataValue::Packed(number as i64))
                        }
                        pillow_rs::PutDataValueKind::Components { .. } => {
                            Ok(pillow_rs::PutDataValue::Number(number))
                        }
                    };
                }
                if let Some(values) = js_integer_array(&value) {
                    if values.len() == 1 {
                        return Ok(pillow_rs::PutDataValue::Packed(values[0]));
                    }
                    return Ok(pillow_rs::PutDataValue::Components(
                        values.into_iter().map(i128::from).collect(),
                    ));
                }
                if value.is_object()
                    && !value.is_array()
                    && matches!(
                        kind,
                        pillow_rs::PutDataValueKind::Components { channels: 2 }
                    )
                    && js_sys::Reflect::has(
                        &value,
                        &JsValue::from_str("__pillow_rs_putdata_custom_index__"),
                    )
                    .unwrap_or(false)
                {
                    return Err(pillow_rs::PilError::TypeError(
                        "color must be int or tuple".to_owned(),
                    ));
                }
                if value.is_array() && js_sys::Array::from(&value).length() == 1 {
                    if matches!(
                        kind,
                        pillow_rs::PutDataValueKind::Components { channels: 2 }
                    ) {
                        return Err(pillow_rs::PilError::SystemError(
                            "new style getargs format but argument is not a tuple".to_owned(),
                        ));
                    }
                    return Err(pillow_rs::PilError::TypeError(
                        "color must be int, or tuple of one, three or four elements".to_owned(),
                    ));
                }
                Err(pillow_rs::PilError::TypeError(
                    "color must be int or tuple".to_owned(),
                ))
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(err)?;
        self.inner
            .putdata_values(&values, scale.unwrap_or(1.0), offset.unwrap_or(0.0))
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "putdata")]
    pub fn putdata(&mut self, d: Vec<u8>) -> Result<(), JsValue> {
        self.inner.putdata(&d).map_err(err)
    }
    #[wasm_bindgen(js_name = "transform")]
    pub fn transform(&self, sz: Vec<u32>, d: Vec<f64>) -> Result<Image, JsValue> {
        let [width, height] = sz.as_slice() else {
            return Err(err(pillow_rs::PilError::TypeError(
                "size must be a two-item sequence".to_owned(),
            )));
        };
        self.inner
            .transform_public(
                (*width, *height),
                0,
                Some(pillow_rs::TransformData::Affine(d)),
                0,
                1,
                None,
            )
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "transformWithInput")]
    pub fn transform_with_input(
        &self,
        size: JsValue,
        method: i32,
        data: JsValue,
        resample: i32,
        fill: i32,
        fillcolor: JsValue,
    ) -> Result<Image, JsValue> {
        let size = js_integer_array(&size).ok_or_else(|| {
            err(pillow_rs::PilError::TypeError(
                "size must be a two-item sequence".to_owned(),
            ))
        })?;
        if size.len() != 2 {
            return Err(err(pillow_rs::PilError::TypeError(format!(
                "size must be a two-item sequence, not {}",
                size.len()
            ))));
        }
        let size = (
            u32::try_from(size[0]).map_err(|_| {
                err(pillow_rs::PilError::ValueError(
                    "width and height must be >= 0".to_owned(),
                ))
            })?,
            u32::try_from(size[1]).map_err(|_| {
                err(pillow_rs::PilError::ValueError(
                    "width and height must be >= 0".to_owned(),
                ))
            })?,
        );
        let mode = self.inner.mode().map_err(err)?;
        let fillcolor = js_transform_fill(&fillcolor, &mode)?;
        self.inner
            .transform_public(
                size,
                method,
                js_transform_data(&data)?,
                resample,
                fill,
                fillcolor,
            )
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
    pub fn getpalette(&self, rawmode: Option<String>) -> Result<Option<Vec<u8>>, JsValue> {
        self.inner
            .getpalette_with_input(rawmode.as_deref())
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "putpalette")]
    pub fn putpalette(&mut self, data: Vec<u8>, rawmode: Option<String>) -> Result<(), JsValue> {
        self.inner
            .putpalette(&data, rawmode.as_deref().unwrap_or("RGB"))
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "getexif")]
    pub fn getexif(&self) -> Vec<u8> {
        self.inner.getexif()
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

#[wasm_bindgen]
pub struct ImageSequence {
    image: Image,
    yielded: bool,
}
#[wasm_bindgen]
impl ImageSequence {
    #[wasm_bindgen(constructor)]
    pub fn new(img: &Image) -> ImageSequence {
        ImageSequence {
            image: Image {
                inner: img.inner.clone(),
            },
            yielded: false,
        }
    }
    #[wasm_bindgen(js_name = "next")]
    pub fn next_frame(&mut self) -> Option<Image> {
        if self.yielded {
            return None;
        }
        self.yielded = true;
        Some(Image {
            inner: self.image.inner.clone(),
        })
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

#[wasm_bindgen(js_name = "imageNewWithInput")]
pub fn image_new_with_input(mode: &str, w: u32, h: u32, color: JsValue) -> Result<Image, JsValue> {
    let input = js_new_color_input(
        mode,
        (!color.is_null() && !color.is_undefined()).then_some(&color),
    )?;
    RsImage::new_with_input(w, h, mode, input)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "fromBytesFn")]
pub fn from_bytes_fn(
    mode: &str,
    w: u32,
    h: u32,
    data: Vec<u8>,
    decoder_name: Option<String>,
) -> Result<Image, JsValue> {
    pillow_rs::image_frombytes(
        mode,
        (w, h),
        &data,
        decoder_name.as_deref().unwrap_or("raw"),
    )
    .map(|i| Image { inner: i })
    .map_err(err)
}

#[wasm_bindgen(js_name = "fromArrayFn")]
pub fn from_array_fn(
    shape: Vec<f64>,
    typestr: &str,
    mode: Option<String>,
    data: Vec<u8>,
) -> Result<Image, JsValue> {
    let shape = shape
        .into_iter()
        .map(|value| {
            if !value.is_finite() || value < 0.0 || value.fract() != 0.0 {
                return Err(err(pillow_rs::PilError::OverflowError(
                    "signed integer is greater than maximum".into(),
                )));
            }
            usize::try_from(value as u128).map_err(|_| {
                err(pillow_rs::PilError::OverflowError(
                    "signed integer is greater than maximum".into(),
                ))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let layout = pillow_rs::resolve_array_layout(&shape, typestr, mode.as_deref()).map_err(err)?;
    pillow_rs::from_resolved_array_interface(&layout, &data)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "openFn")]
pub fn open_fn(data: Vec<u8>, mode: JsValue, formats: JsValue) -> Result<Image, JsValue> {
    let mode = if mode.is_null() || mode.is_undefined() {
        pillow_rs::PythonOpenModeInput::None
    } else if let Some(name) = mode.as_string() {
        pillow_rs::PythonOpenModeInput::Name(name)
    } else {
        pillow_rs::PythonOpenModeInput::Invalid(js_value_display(&mode))
    };
    let formats = if formats.is_null() || formats.is_undefined() {
        pillow_rs::PythonOpenFormatsInput::None
    } else if formats.is_array() {
        let values = js_sys::Array::from(&formats)
            .iter()
            .map(|value| value.as_string())
            .collect::<Option<Vec<_>>>();
        match values {
            Some(values) => pillow_rs::PythonOpenFormatsInput::Names(values),
            None => pillow_rs::PythonOpenFormatsInput::Invalid(js_value_type_name(&formats)),
        }
    } else {
        pillow_rs::PythonOpenFormatsInput::Invalid(js_value_type_name(&formats))
    };
    let format_names = pillow_rs::validate_python_open_inputs(mode, formats).map_err(err)?;
    let format_refs = format_names
        .as_ref()
        .map(|values| values.iter().map(String::as_str).collect::<Vec<_>>());
    RsImage::open_bytes_with_formats(data, format_refs.as_deref())
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "validateOpenInputs")]
pub fn validate_open_inputs(mode: JsValue, formats: JsValue) -> Result<(), JsValue> {
    let mode = if mode.is_null() || mode.is_undefined() {
        pillow_rs::PythonOpenModeInput::None
    } else if let Some(name) = mode.as_string() {
        pillow_rs::PythonOpenModeInput::Name(name)
    } else {
        pillow_rs::PythonOpenModeInput::Invalid(js_value_display(&mode))
    };
    let formats = if formats.is_null() || formats.is_undefined() {
        pillow_rs::PythonOpenFormatsInput::None
    } else if formats.is_array() {
        let values = js_sys::Array::from(&formats)
            .iter()
            .map(|value| value.as_string())
            .collect::<Option<Vec<_>>>();
        match values {
            Some(values) => pillow_rs::PythonOpenFormatsInput::Names(values),
            None => pillow_rs::PythonOpenFormatsInput::Invalid(js_value_type_name(&formats)),
        }
    } else {
        pillow_rs::PythonOpenFormatsInput::Invalid(js_value_type_name(&formats))
    };
    pillow_rs::validate_python_open_inputs(mode, formats)
        .map(|_| ())
        .map_err(err)
}

#[wasm_bindgen(js_name = "validateOpenSource")]
pub fn validate_open_source(data: Vec<u8>) -> Result<(), JsValue> {
    pillow_rs::validate_python_open_source_bytes(&data)
        .map(|_| ())
        .map_err(err)
}

#[wasm_bindgen(js_name = "imageNewPaletteIndex")]
pub fn image_new_palette_index(w: u32, h: u32, index: u8) -> Image {
    Image {
        inner: RsImage::new_palette_index(w, h, index),
    }
}

// ── ImageChops ───────────────────────────────────────────────────

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
