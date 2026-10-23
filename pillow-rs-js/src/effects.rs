use crate::Image;
use crate::values::*;
use wasm_bindgen::prelude::{JsValue, wasm_bindgen};

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

#[wasm_bindgen(js_name = "effectMandelbrotWithExtent")]
pub fn effect_mandelbrot_with_extent(
    size: JsValue,
    extent: JsValue,
    quality: i32,
) -> Result<Image, JsValue> {
    let size = js_integer_array(&size).ok_or_else(|| {
        err(pillow_rs::PilError::TypeError(
            "argument 1 must be 2-item sequence".to_owned(),
        ))
    })?;
    if size.len() != 2 || size.iter().any(|value| *value < 0) {
        return Err(err(pillow_rs::PilError::ValueError(
            "size must be a non-negative 2-item sequence".to_owned(),
        )));
    }
    let extent_values = js_float_array(&extent);
    let extent_type = js_value_type_name(&extent);
    pillow_rs::image_effect_mandelbrot_with_extent(
        (size[0] as u32, size[1] as u32),
        extent_values.as_deref(),
        &extent_type,
        quality,
    )
    .map(|i| Image { inner: i })
    .map_err(err)
}

#[wasm_bindgen(js_name = "effectNoiseFn")]
pub fn effect_noise(width: u32, height: u32, sigma: f64) -> Result<Image, JsValue> {
    pillow_rs::image_effect_noise_from_size((width, height), sigma)
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
