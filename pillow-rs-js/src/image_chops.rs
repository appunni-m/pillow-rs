use crate::Image;
use crate::js_chops_offset;
use crate::values::*;
use pillow_rs::Image as RsImage;
use wasm_bindgen::prelude::{JsValue, wasm_bindgen};

#[wasm_bindgen]
pub struct ImageChops {}
#[wasm_bindgen]
impl ImageChops {
    #[wasm_bindgen(js_name = "add")]
    pub fn add(
        a: &Image,
        b: &Image,
        scale: Option<f64>,
        offset: JsValue,
    ) -> Result<Image, JsValue> {
        let offset = js_chops_offset(&offset)?;
        pillow_rs::chops_add(&a.inner, &b.inner, scale.unwrap_or(1.0), f64::from(offset))
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "subtract")]
    pub fn sub(
        a: &Image,
        b: &Image,
        scale: Option<f64>,
        offset: JsValue,
    ) -> Result<Image, JsValue> {
        let offset = js_chops_offset(&offset)?;
        pillow_rs::chops_subtract(&a.inner, &b.inner, scale.unwrap_or(1.0), f64::from(offset))
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
    pub fn off(img: &Image, x: i32, y: Option<i32>) -> Result<Image, JsValue> {
        pillow_rs::chops_offset_with_default(&img.inner, x, y)
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
#[wasm_bindgen(js_name = "merge")]
pub fn merge(mode: &str, bands: Vec<Image>) -> Result<Image, JsValue> {
    let imgs: Vec<RsImage> = bands.iter().map(|b| b.inner.clone()).collect();
    pillow_rs::image_merge(mode, &imgs)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "kernelPrepare")]
pub fn kernel_prepare(
    kernel: Vec<f64>,
    scale: Option<f64>,
    offset: f64,
    size: Vec<u32>,
) -> Result<(), JsValue> {
    let size = match size.as_slice() {
        [width, height] => (*width, *height),
        _ => {
            return Err(err(pillow_rs::PilError::ValueError(
                "bad kernel size".to_owned(),
            )));
        }
    };
    pillow_rs::prepare_kernel(Some(kernel), scale, offset, size)
        .map(|_| ())
        .map_err(err)
}

#[wasm_bindgen(js_name = "mergeWithInput")]
pub fn merge_with_input(
    mode: &str,
    bands: Vec<Image>,
    band_count: u32,
    invalid_type: Option<String>,
) -> Result<Image, JsValue> {
    // Rust-exported wasm-bindgen structs do not implement JsCast. The
    // workflow therefore supplies image handles as a typed Vec and preserves
    // the first non-image's public type separately; validation/order remains
    // in the shared core contract.
    let mut inputs = bands
        .into_iter()
        .map(|image| pillow_rs::MergeInput::Image(Box::new(image.inner)))
        .collect::<Vec<_>>();
    if let Some(type_name) = invalid_type {
        inputs.push(pillow_rs::MergeInput::Invalid(type_name));
    }
    while inputs.len() < band_count as usize {
        inputs.push(pillow_rs::MergeInput::Invalid("object".to_owned()));
    }
    inputs.truncate(band_count as usize);
    pillow_rs::image_merge_inputs(mode, &inputs)
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

/// Enable a backend route for future automatic image routing.
/// Returns true if this call changed the backend from inactive to active.
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
