use crate::inputs::*;
use crate::values::*;
use pillow_rs::Image as RsImage;
use wasm_bindgen::prelude::{JsValue, wasm_bindgen};

#[wasm_bindgen]
pub struct Image {
    pub(crate) inner: RsImage,
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
    #[wasm_bindgen(getter)]
    pub fn format(&self) -> Option<String> {
        self.inner.format_name()
    }
    #[wasm_bindgen(js_name = "compatibilityInfo")]
    pub fn compatibility_info(&self) -> JsValue {
        image_info_to_js(self.inner.compatibility_info())
    }
    #[wasm_bindgen(js_name = "convertedCompatibilityInfo")]
    pub fn converted_compatibility_info(&self, target_mode: &str) -> JsValue {
        image_info_to_js(self.inner.converted_compatibility_info(target_mode))
    }
    #[wasm_bindgen(js_name = "replaceFrom")]
    pub fn replace_from(&mut self, other: &Image) {
        self.inner = other.inner.clone();
    }
    pub fn size(&mut self) -> Result<Vec<u32>, JsValue> {
        self.inner.size().map(|(w, h)| vec![w, h]).map_err(err)
    }

    // Transforms
    #[wasm_bindgen(js_name = "resize")]
    pub fn resize(&self, w: u32, h: u32, f: Option<String>) -> Result<Image, JsValue> {
        let filter = f.map(pillow_rs::ResampleInput::Name);
        self.inner
            .resize((i64::from(w), i64::from(h)), filter, None)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "resizeWithInput")]
    pub fn resize_with_input(
        &self,
        size: JsValue,
        resample: JsValue,
        box_coords: JsValue,
        reducing_gap: JsValue,
    ) -> Result<Image, JsValue> {
        let values = js_integer_array(&size).ok_or_else(|| {
            err(pillow_rs::PilError::TypeError(
                "size must be a two-item sequence".to_owned(),
            ))
        })?;
        if values.len() != 2 {
            return Err(err(pillow_rs::PilError::TypeError(format!(
                "size must be a two-item sequence, not {}",
                values.len()
            ))));
        }
        let filter = js_optional_resample(&resample)?;
        let box_coords = js_resize_box(&box_coords)?;
        let reducing_gap = if reducing_gap.is_null() || reducing_gap.is_undefined() {
            None
        } else {
            Some(reducing_gap.as_f64().ok_or_else(|| {
                err(pillow_rs::PilError::TypeError(
                    "reducing_gap must be a real number".to_owned(),
                ))
            })?)
        };
        self.inner
            .resize_with_options((values[0], values[1]), filter, box_coords, reducing_gap)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "crop")]
    pub fn crop(&self, l: u32, t: u32, r: u32, b: u32) -> Result<Image, JsValue> {
        self.inner
            .crop_unsigned((l, t, r, b))
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "cropWithInput")]
    pub fn crop_with_input(&self, box_value: JsValue) -> Result<Image, JsValue> {
        let box_coords = if box_value.is_null() || box_value.is_undefined() {
            None
        } else {
            let values = js_float_array(&box_value).ok_or_else(|| {
                err(pillow_rs::PilError::TypeError(
                    "box must be a 4-item sequence".to_owned(),
                ))
            })?;
            if values.len() != 4 {
                return Err(err(pillow_rs::PilError::TypeError(format!(
                    "box must be a 4-item sequence, not {}",
                    values.len()
                ))));
            }
            Some((values[0], values[1], values[2], values[3]))
        };
        self.inner
            .crop_float(box_coords)
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
    #[wasm_bindgen(js_name = "rotateWithInput")]
    pub fn rotate_with_input(
        &self,
        angle: f64,
        resample: JsValue,
        expand: JsValue,
        center: JsValue,
        translate: JsValue,
        fillcolor: JsValue,
    ) -> Result<Image, JsValue> {
        self.inner
            .rotate_with_input(
                angle,
                js_rotate_resample(&resample),
                js_rotate_expand(&expand),
                js_rotate_point(&center),
                js_rotate_point(&translate),
                js_imageops_color(&fillcolor),
            )
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
    #[wasm_bindgen(js_name = "convertWithInput")]
    pub fn convert_with_input(
        &self,
        mode: JsValue,
        matrix: JsValue,
        dither: JsValue,
        palette: JsValue,
        colors: JsValue,
    ) -> Result<Image, JsValue> {
        let matrix = js_optional_matrix(&matrix)?;
        let colors = js_optional_i32(&colors)?.map(|value| {
            u32::try_from(value).map_err(|_| {
                err(pillow_rs::PilError::OverflowError(
                    "colors is outside the supported range".to_owned(),
                ))
            })
        });
        let colors = colors.transpose()?;
        self.inner
            .convert_with_input(
                js_convert_mode(&mode),
                matrix,
                js_python_dither(&dither),
                js_convert_palette(&palette),
                colors,
            )
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
}

mod filters;
mod pixels;
