use crate::Image;
use crate::js_chops_offset;
use crate::values::*;
use pillow_rs::Image as RsImage;
use wasm_bindgen::prelude::{JsValue, wasm_bindgen};

#[wasm_bindgen]
/// Namespace for per-pixel arithmetic and blend operations on images.
pub struct ImageChops {}
#[wasm_bindgen]
impl ImageChops {
    /// Adds corresponding pixels, applying an optional scale and offset.
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
    /// Subtracts corresponding pixels, applying an optional scale and offset.
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
    /// Multiplies corresponding pixels from two images.
    #[wasm_bindgen(js_name = "multiply")]
    pub fn mul(a: &Image, b: &Image) -> Result<Image, JsValue> {
        pillow_rs::chops_multiply(&a.inner, &b.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    /// Combines two images with the screen blend operation.
    #[wasm_bindgen(js_name = "screen")]
    pub fn scr(a: &Image, b: &Image) -> Result<Image, JsValue> {
        pillow_rs::chops_screen(&a.inner, &b.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    /// Selects the darker pixel at each position.
    #[wasm_bindgen(js_name = "darker")]
    pub fn dark(a: &Image, b: &Image) -> Result<Image, JsValue> {
        pillow_rs::chops_darker(&a.inner, &b.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    /// Selects the lighter pixel at each position.
    #[wasm_bindgen(js_name = "lighter")]
    pub fn light(a: &Image, b: &Image) -> Result<Image, JsValue> {
        pillow_rs::chops_lighter(&a.inner, &b.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    /// Returns the absolute difference between corresponding pixels.
    #[wasm_bindgen(js_name = "difference")]
    pub fn diff(a: &Image, b: &Image) -> Result<Image, JsValue> {
        pillow_rs::chops_difference(&a.inner, &b.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    /// Inverts the image's pixel values.
    #[wasm_bindgen(js_name = "invert")]
    pub fn inv(img: &Image) -> Result<Image, JsValue> {
        pillow_rs::chops_invert(&img.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    /// Combines two images with the hard-light blend operation.
    #[wasm_bindgen(js_name = "hardLight")]
    pub fn hard(a: &Image, b: &Image) -> Result<Image, JsValue> {
        pillow_rs::chops_hard_light(&a.inner, &b.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    /// Combines two images with the soft-light blend operation.
    #[wasm_bindgen(js_name = "softLight")]
    pub fn soft(a: &Image, b: &Image) -> Result<Image, JsValue> {
        pillow_rs::chops_soft_light(&a.inner, &b.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    /// Combines two images with the overlay blend operation.
    #[wasm_bindgen(js_name = "overlay")]
    pub fn over(a: &Image, b: &Image) -> Result<Image, JsValue> {
        pillow_rs::chops_overlay(&a.inner, &b.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    /// Shifts image pixels by `x` and optional `y` offsets.
    #[wasm_bindgen(js_name = "offset")]
    pub fn off(img: &Image, x: i32, y: Option<i32>) -> Result<Image, JsValue> {
        pillow_rs::chops_offset_with_default(&img.inner, x, y)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    /// Adds corresponding pixels with channel values wrapping modulo the channel range.
    #[wasm_bindgen(js_name = "addModulo")]
    pub fn addm(a: &Image, b: &Image) -> Result<Image, JsValue> {
        pillow_rs::chops_add_modulo(&a.inner, &b.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    /// Subtracts corresponding pixels with channel values wrapping modulo the channel range.
    #[wasm_bindgen(js_name = "subtractModulo")]
    pub fn subm(a: &Image, b: &Image) -> Result<Image, JsValue> {
        pillow_rs::chops_subtract_modulo(&a.inner, &b.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    /// Blends two images using `alpha` as the interpolation weight.
    #[wasm_bindgen(js_name = "blend")]
    pub fn blnd(a: &Image, b: &Image, alpha: f64) -> Result<Image, JsValue> {
        pillow_rs::image_blend(&a.inner, &b.inner, alpha)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    /// Selects pixels from `a` or `b` according to the grayscale mask `m`.
    #[wasm_bindgen(js_name = "composite")]
    pub fn comp(a: &Image, b: &Image, m: &Image) -> Result<Image, JsValue> {
        pillow_rs::image_composite(&a.inner, &b.inner, &m.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    /// Creates an image filled with the supplied constant value.
    #[wasm_bindgen(js_name = "constant")]
    pub fn cnst(img: &Image, v: u8) -> Result<Image, JsValue> {
        pillow_rs::chops_constant(&img.inner, v)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    /// Returns a clone of the image.
    #[wasm_bindgen(js_name = "duplicate")]
    pub fn dup(img: &Image) -> Image {
        Image {
            inner: img.inner.clone(),
        }
    }
    /// Applies a logical AND to corresponding image pixels.
    #[wasm_bindgen(js_name = "logicalAnd")]
    pub fn land(a: &Image, b: &Image) -> Result<Image, JsValue> {
        pillow_rs::chops_logical_and(&a.inner, &b.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    /// Applies a logical OR to corresponding image pixels.
    #[wasm_bindgen(js_name = "logicalOr")]
    pub fn lor(a: &Image, b: &Image) -> Result<Image, JsValue> {
        pillow_rs::chops_logical_or(&a.inner, &b.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    /// Applies a logical XOR to corresponding image pixels.
    #[wasm_bindgen(js_name = "logicalXor")]
    pub fn lxor(a: &Image, b: &Image) -> Result<Image, JsValue> {
        pillow_rs::chops_logical_xor(&a.inner, &b.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
}

// ── ImageOps ─────────────────────────────────────────────────────
#[wasm_bindgen(js_name = "merge")]
/// Merges image bands into a new image in the requested mode.
pub fn merge(mode: &str, bands: Vec<Image>) -> Result<Image, JsValue> {
    let imgs: Vec<RsImage> = bands.iter().map(|b| b.inner.clone()).collect();
    pillow_rs::image_merge(mode, &imgs)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "kernelPrepare")]
/// Validates and prepares a convolution kernel with the requested dimensions.
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
/// Merges bands while preserving invalid-input and band-count validation behavior.
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
/// Blends two images using `alpha` as the interpolation weight.
pub fn blend(a: &Image, b: &Image, alpha: f64) -> Result<Image, JsValue> {
    pillow_rs::image_blend(&a.inner, &b.inner, alpha)
        .map(|i| Image { inner: i })
        .map_err(err)
}
#[wasm_bindgen(js_name = "composite")]
/// Selects pixels from `a` or `b` according to the grayscale mask `m`.
pub fn composite(a: &Image, b: &Image, m: &Image) -> Result<Image, JsValue> {
    pillow_rs::image_composite(&a.inner, &b.inner, &m.inner)
        .map(|i| Image { inner: i })
        .map_err(err)
}

/// Enable a backend route for future automatic image routing.
/// Returns true if this call changed the backend from inactive to active.
#[wasm_bindgen(js_name = "addModulo")]
/// Adds corresponding pixels with channel values wrapping modulo the channel range.
pub fn add_modulo(a: &Image, b: &Image) -> Result<Image, JsValue> {
    pillow_rs::chops_add_modulo(&a.inner, &b.inner)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "constant")]
/// Creates an image filled with the supplied constant value.
pub fn constant(img: &Image, value: u8) -> Result<Image, JsValue> {
    pillow_rs::chops_constant(&img.inner, value)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "darker")]
/// Selects the darker pixel at each position.
pub fn darker(a: &Image, b: &Image) -> Result<Image, JsValue> {
    pillow_rs::chops_darker(&a.inner, &b.inner)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "hardLight")]
/// Combines two images with the hard-light blend operation.
pub fn hard_light(a: &Image, b: &Image) -> Result<Image, JsValue> {
    pillow_rs::chops_hard_light(&a.inner, &b.inner)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "lighter")]
/// Selects the lighter pixel at each position.
pub fn lighter(a: &Image, b: &Image) -> Result<Image, JsValue> {
    pillow_rs::chops_lighter(&a.inner, &b.inner)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "logicalAnd")]
/// Applies a logical AND to corresponding image pixels.
pub fn logical_and(a: &Image, b: &Image) -> Result<Image, JsValue> {
    pillow_rs::chops_logical_and(&a.inner, &b.inner)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "logicalOr")]
/// Applies a logical OR to corresponding image pixels.
pub fn logical_or(a: &Image, b: &Image) -> Result<Image, JsValue> {
    pillow_rs::chops_logical_or(&a.inner, &b.inner)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "logicalXor")]
/// Applies a logical XOR to corresponding image pixels.
pub fn logical_xor(a: &Image, b: &Image) -> Result<Image, JsValue> {
    pillow_rs::chops_logical_xor(&a.inner, &b.inner)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "multiply")]
/// Multiplies corresponding pixels from two images.
pub fn multiply(a: &Image, b: &Image) -> Result<Image, JsValue> {
    pillow_rs::chops_multiply(&a.inner, &b.inner)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "offset")]
/// Shifts image pixels by the requested horizontal and vertical offsets.
pub fn offset(img: &Image, xoffset: i32, yoffset: i32) -> Result<Image, JsValue> {
    pillow_rs::chops_offset(&img.inner, xoffset, yoffset)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "overlay")]
/// Combines two images with the overlay blend operation.
pub fn overlay(a: &Image, b: &Image) -> Result<Image, JsValue> {
    pillow_rs::chops_overlay(&a.inner, &b.inner)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "screenFn")]
/// Combines two images with the screen blend operation.
pub fn screen(a: &Image, b: &Image) -> Result<Image, JsValue> {
    pillow_rs::chops_screen(&a.inner, &b.inner)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "softLight")]
/// Combines two images with the soft-light blend operation.
pub fn soft_light(a: &Image, b: &Image) -> Result<Image, JsValue> {
    pillow_rs::chops_soft_light(&a.inner, &b.inner)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "subtractModulo")]
/// Subtracts corresponding pixels with channel values wrapping modulo the channel range.
pub fn subtract_modulo(a: &Image, b: &Image) -> Result<Image, JsValue> {
    pillow_rs::chops_subtract_modulo(&a.inner, &b.inner)
        .map(|i| Image { inner: i })
        .map_err(err)
}

// ══════════════════════════════════════════════════════════════════════════════
// ImageOps — high-level image operations
// ══════════════════════════════════════════════════════════════════════════════
