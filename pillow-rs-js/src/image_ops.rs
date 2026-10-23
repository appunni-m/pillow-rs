use crate::Image;
use crate::inputs::*;
use crate::values::*;
use wasm_bindgen::prelude::{JsValue, wasm_bindgen};

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
    /// Applies the exact host-computed `sample < threshold` solarize table.
    ///
    /// # Errors
    ///
    /// Returns an error when the table does not contain 256 entries or the
    /// image mode is unsupported by Pillow's `ImageOps.solarize`.
    #[wasm_bindgen(js_name = "solarizeWithComparisons")]
    pub fn sol_with_comparisons(img: &Image, below: Vec<u8>) -> Result<Image, JsValue> {
        if below.len() != 256 {
            return Err(err(pillow_rs::PilError::ValueError(
                "solarize threshold comparisons must contain 256 entries".into(),
            )));
        }
        let mut comparisons = Box::new([false; 256]);
        for (output, input) in comparisons.iter_mut().zip(below) {
            *output = input != 0;
        }
        pillow_rs::imageops_solarize_with_input(
            &img.inner,
            pillow_rs::SolarizeInput::Comparisons(comparisons),
        )
        .map(|i| Image { inner: i })
        .map_err(err)
    }
    #[wasm_bindgen(js_name = "equalize")]
    pub fn eq(img: &Image) -> Result<Image, JsValue> {
        pillow_rs::imageops_equalize(&img.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "equalizeWithInput")]
    pub fn eq_with_input(img: &Image, mask: &Image) -> Result<Image, JsValue> {
        pillow_rs::imageops_equalize_with_mask(
            &img.inner,
            pillow_rs::ImageOpsMask::Image(Box::new(mask.inner.clone())),
        )
        .map(|i| Image { inner: i })
        .map_err(err)
    }
    #[wasm_bindgen(js_name = "autocontrast")]
    pub fn auto(img: &Image, c: f64) -> Result<Image, JsValue> {
        pillow_rs::imageops_autocontrast(&img.inner, c)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "autocontrastWithMask")]
    pub fn auto_with_mask(img: &Image, c: f64, mask: &Image) -> Result<Image, JsValue> {
        pillow_rs::imageops_autocontrast_with_mask(
            &img.inner,
            c,
            pillow_rs::ImageOpsMask::Image(Box::new(mask.inner.clone())),
        )
        .map(|i| Image { inner: i })
        .map_err(err)
    }
    #[wasm_bindgen(js_name = "autocontrastInvalidMask")]
    pub fn auto_with_invalid_mask(img: &Image, c: f64, type_name: &str) -> Result<Image, JsValue> {
        let mask = pillow_rs::ImageOpsMask::Invalid(type_name.to_owned());
        pillow_rs::imageops_autocontrast_with_mask(&img.inner, c, mask)
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
    #[wasm_bindgen(js_name = "containWithInput")]
    pub fn contain_with_input(
        img: &Image,
        w: u32,
        h: u32,
        method: JsValue,
    ) -> Result<Image, JsValue> {
        pillow_rs::imageops_contain_with_input(&img.inner, w, h, js_optional_resample(&method)?)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "cover")]
    pub fn cover(img: &Image, w: u32, h: u32) -> Result<Image, JsValue> {
        pillow_rs::imageops_cover(&img.inner, w, h, None)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "coverWithInput")]
    pub fn cover_with_input(
        img: &Image,
        w: u32,
        h: u32,
        method: JsValue,
    ) -> Result<Image, JsValue> {
        pillow_rs::imageops_cover_with_input(&img.inner, w, h, js_optional_resample(&method)?)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "fit")]
    pub fn fit(img: &Image, w: u32, h: u32) -> Result<Image, JsValue> {
        pillow_rs::imageops_fit(&img.inner, w, h, None, 0.0, (0.5, 0.5))
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "fitWithInput")]
    pub fn fit_with_input(
        img: &Image,
        w: u32,
        h: u32,
        method: JsValue,
        bleed: f64,
        centering: JsValue,
    ) -> Result<Image, JsValue> {
        pillow_rs::imageops_fit_with_input(
            &img.inner,
            w,
            h,
            js_optional_resample(&method)?,
            bleed,
            js_centering(&centering),
        )
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
    #[wasm_bindgen(js_name = "padWithInput")]
    pub fn pad_with_input(
        img: &Image,
        w: u32,
        h: u32,
        method: JsValue,
        color: JsValue,
        centering: JsValue,
    ) -> Result<Image, JsValue> {
        pillow_rs::imageops_pad_with_input(
            &img.inner,
            w,
            h,
            js_optional_resample(&method)?,
            js_imageops_color(&color),
            js_centering(&centering),
        )
        .map(|i| Image { inner: i })
        .map_err(err)
    }
    #[wasm_bindgen(js_name = "scale")]
    pub fn scale(img: &Image, factor: f64) -> Result<Image, JsValue> {
        pillow_rs::imageops_scale_with_input(&img.inner, factor, None)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "scaleWithInput")]
    pub fn scale_with_input(img: &Image, factor: f64, method: JsValue) -> Result<Image, JsValue> {
        pillow_rs::imageops_scale_with_input(&img.inner, factor, js_optional_resample(&method)?)
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
            None,
            0,
            127,
            255,
        )
        .map(|i| Image { inner: i })
        .map_err(err)
    }
}

#[wasm_bindgen(js_name = "colorizeFn")]
pub fn colorize_fn(
    img: &Image,
    black: JsValue,
    white: JsValue,
    mid: JsValue,
    blackpoint: i32,
    midpoint: i32,
    whitepoint: i32,
) -> Result<Image, JsValue> {
    let black = js_color_triplet(&black)?;
    let white = js_color_triplet(&white)?;
    let mid = if mid.is_null() || mid.is_undefined() {
        None
    } else {
        Some(js_color_triplet(&mid)?)
    };
    if !(0..=255).contains(&blackpoint)
        || !(0..=255).contains(&midpoint)
        || !(0..=255).contains(&whitepoint)
    {
        return Err(err(pillow_rs::PilError::AssertionError(
            "colorize points out of range".to_owned(),
        )));
    }
    pillow_rs::imageops_colorize(
        &img.inner,
        black,
        white,
        mid,
        blackpoint as u8,
        midpoint as u8,
        whitepoint as u8,
    )
    .map(|i| Image { inner: i })
    .map_err(err)
}

// ── Module functions ─────────────────────────────────────────────
#[wasm_bindgen(js_name = "color3DLUTCheckSize")]
pub fn color3dlut_check_size(size: Vec<f64>) -> Result<Vec<u32>, JsValue> {
    pillow_rs::color3dlut_check_size(&size)
        .map(|(x, y, z)| vec![x, y, z])
        .map_err(err)
}

#[wasm_bindgen(js_name = "color3DLUTNew")]
pub fn color3dlut_new(
    table: JsValue,
    size_x: u32,
    size_y: u32,
    size_z: u32,
    channels: u32,
) -> Result<Vec<f64>, JsValue> {
    pillow_rs::color3dlut_prepare_table(
        js_color3dlut_table(&table)?,
        (size_x, size_y, size_z),
        channels,
    )
    .map_err(err)
}

#[wasm_bindgen(js_name = "color3DLUTGenerate")]
pub fn color3dlut_generate(
    size_x: u32,
    size_y: u32,
    size_z: u32,
    channels: u32,
    callback: String,
) -> Result<Vec<f64>, JsValue> {
    pillow_rs::color3dlut_generate_table(
        (size_x, size_y, size_z),
        channels,
        |values| match callback.as_str() {
            "color3dlut-generate-identity" => Ok(values.to_vec()),
            "color3dlut-short-result" => Ok(values.iter().copied().take(2).collect()),
            _ => Err(err(pillow_rs::PilError::NotImplementedError(format!(
                "unsupported Color3DLUT callback: {callback}"
            )))),
        },
        err,
    )
}

#[wasm_bindgen(js_name = "color3DLUTRepr")]
pub fn color3dlut_repr(
    table_type: String,
    size_x: u32,
    size_y: u32,
    size_z: u32,
    channels: u32,
    target_mode: Option<String>,
) -> String {
    pillow_rs::color3dlut_repr(
        &table_type,
        (size_x, size_y, size_z),
        channels,
        target_mode.as_deref(),
    )
}

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
    pillow_rs::imageops_scale_with_input(&img.inner, factor, None)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "alphaCompositeFn")]
pub fn alpha_composite_fn(a: &Image, b: &Image) -> Result<Image, JsValue> {
    pillow_rs::image_alpha_composite(&a.inner, &b.inner)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "exifTransposeFn")]
pub fn exif_transpose_fn(img: &mut Image, in_place: bool) -> Result<JsValue, JsValue> {
    let result = pillow_rs::imageops_exif_transpose(&img.inner, in_place).map_err(err)?;
    if in_place {
        if let Some(transposed) = result {
            img.inner = transposed;
        }
        return Ok(JsValue::NULL);
    }
    Ok(result
        .map(|inner| Image { inner })
        .map_or(JsValue::NULL, Into::into))
}
