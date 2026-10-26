use crate::Image;
use crate::inputs::*;
use crate::values::*;
use wasm_bindgen::prelude::{JsValue, wasm_bindgen};

#[wasm_bindgen]
impl Image {
    // Enhancement
    /// Adjusts image brightness by the supplied enhancement factor.
    #[wasm_bindgen(js_name = "enhanceBrightness")]
    pub fn bright(&self, f: f64) -> Result<Image, JsValue> {
        self.inner
            .enhance_brightness(f)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    /// Adjusts image contrast by the supplied enhancement factor.
    #[wasm_bindgen(js_name = "enhanceContrast")]
    pub fn contrast(&self, f: f64) -> Result<Image, JsValue> {
        self.inner
            .enhance_contrast(f)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    /// Adjusts image color saturation by the supplied enhancement factor.
    #[wasm_bindgen(js_name = "enhanceColor")]
    pub fn color(&self, f: f64) -> Result<Image, JsValue> {
        self.inner
            .enhance_color(f)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    /// Adjusts image sharpness by the supplied enhancement factor.
    #[wasm_bindgen(js_name = "enhanceSharpness")]
    pub fn sharp(&self, f: f64) -> Result<Image, JsValue> {
        self.inner
            .enhance_sharpness(f)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    /// Builds Color's constructor-time degenerate image.
    ///
    /// # Errors
    ///
    /// Returns mode conversion or materialization errors from the core.
    #[wasm_bindgen(js_name = "colorDegenerate")]
    pub fn color_degenerate(&self) -> Result<Image, JsValue> {
        self.inner
            .color_degenerate()
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    /// Builds Contrast's constructor-time mean image.
    ///
    /// # Errors
    ///
    /// Returns mode conversion or materialization errors from the core.
    #[wasm_bindgen(js_name = "contrastDegenerate")]
    pub fn contrast_degenerate(&self) -> Result<Image, JsValue> {
        self.inner
            .contrast_degenerate()
            .map(|i| Image { inner: i })
            .map_err(err)
    }

    // Filters
    /// Applies a Gaussian blur with the given radius.
    #[wasm_bindgen(js_name = "gaussianBlur")]
    pub fn gaussian(&self, r: f32) -> Result<Image, JsValue> {
        self.inner
            .gaussian_blur(r)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    /// Applies a Gaussian blur with independent horizontal and vertical radii.
    #[wasm_bindgen(js_name = "gaussianBlurXY")]
    pub fn gaussian_xy(&self, rx: f32, ry: f32) -> Result<Image, JsValue> {
        self.inner
            .gaussian_blur_xy(rx, ry)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    /// Applies a box blur with the given radius.
    #[wasm_bindgen(js_name = "boxBlur")]
    pub fn boxb(&self, r: f32) -> Result<Image, JsValue> {
        self.inner
            .box_blur(r)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    /// Applies a box blur with independent horizontal and vertical radii.
    #[wasm_bindgen(js_name = "boxBlurXY")]
    pub fn box_xy(&self, rx: f32, ry: f32) -> Result<Image, JsValue> {
        self.inner
            .box_blur_xy(rx, ry)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    /// Sharpens the image using an unsharp mask with the supplied radius, percent,
    /// and threshold.
    #[wasm_bindgen(js_name = "unsharpMask")]
    pub fn unsharp(&self, r: f32, p: i32, t: i32) -> Result<Image, JsValue> {
        self.inner
            .unsharp_mask(r, p, t)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    /// Replaces each pixel with the maximum value in its `size`-wide neighborhood.
    #[wasm_bindgen(js_name = "maxFilter")]
    pub fn maxf(&self, s: u32) -> Result<Image, JsValue> {
        self.inner
            .max_filter(s)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    /// Replaces each pixel with the minimum value in its `size`-wide neighborhood.
    #[wasm_bindgen(js_name = "minFilter")]
    pub fn minf(&self, s: u32) -> Result<Image, JsValue> {
        self.inner
            .min_filter(s)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    /// Replaces each pixel with the median value in its `size`-wide neighborhood.
    #[wasm_bindgen(js_name = "medianFilter")]
    pub fn medianf(&self, s: u32) -> Result<Image, JsValue> {
        self.inner
            .median_filter(s)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    /// Replaces each pixel with the most frequent value in its `size`-wide neighborhood.
    #[wasm_bindgen(js_name = "modeFilter")]
    pub fn modef(&self, s: u32) -> Result<Image, JsValue> {
        self.inner
            .mode_filter(s)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    /// Selects rank `rank` from each `size`-wide neighborhood.
    #[wasm_bindgen(js_name = "rankFilter")]
    pub fn rankf(&self, s: u32, r: u32) -> Result<Image, JsValue> {
        self.inner
            .rank_filter(s, r)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    /// Applies a square convolution kernel with an explicit scale and offset.
    #[wasm_bindgen(js_name = "kernelFilter")]
    pub fn kernelf(
        &self,
        kernel: Vec<f32>,
        scale: f32,
        offset: i32,
        size: u32,
    ) -> Result<Image, JsValue> {
        let kernel = Some(kernel.into_iter().map(f64::from).collect());
        self.inner
            .kernel_filter(
                kernel,
                Some(f64::from(scale)),
                f64::from(offset),
                (size, size),
            )
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    /// Applies a three-dimensional color lookup table.
    ///
    /// The table dimensions and input channel count describe its layout; `target_mode`
    /// optionally selects the output image mode.
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
        let input = pillow_rs::prepare_color3dlut(table, (size_x, size_y, size_z), channels)
            .map_err(err)?;
        self.inner
            .color3dlut(input, target_mode.as_deref())
            .map(|i| Image { inner: i })
            .map_err(err)
    }

    /// Transform a Color3DLUT table using a named parity-corpus callback.
    /// Table traversal, callback-result slice semantics, and final length
    /// validation remain in the shared Rust core; this binding only decodes
    /// the callback asset name used by the JS host.
    #[wasm_bindgen(js_name = "color3DLUTTransform")]
    pub fn color3dlut_transform(
        table: Vec<f64>,
        size_x: u32,
        size_y: u32,
        size_z: u32,
        channels_in: u32,
        channels_out: JsValue,
        with_normals: bool,
        callback: String,
    ) -> Result<Vec<f64>, JsValue> {
        let size = (size_x, size_y, size_z);
        let input = pillow_rs::prepare_color3dlut(table, size, channels_in).map_err(err)?;
        let channels_out = js_optional_i32(&channels_out)?
            .map(|value| {
                u32::try_from(value).map_err(|_| {
                    err(pillow_rs::PilError::ValueError(
                        "Only 3 or 4 output channels are supported".to_owned(),
                    ))
                })
            })
            .transpose()?;
        let output_channels = channels_out.unwrap_or(channels_in);
        let output = pillow_rs::color3dlut_transform_table(
            &input,
            channels_out,
            with_normals,
            |values| match callback.as_str() {
                "color3dlut-transform-identity" => {
                    Ok(values[values.len().saturating_sub(3)..].to_vec())
                }
                "color3dlut-transform-rgba" => {
                    let mut result = values[values.len().saturating_sub(3)..].to_vec();
                    result.push(1.0);
                    Ok(result)
                }
                "color3dlut-short-result" => Ok(values.iter().copied().take(2).collect()),
                _ => Err(err(pillow_rs::PilError::NotImplementedError(format!(
                    "unsupported Color3DLUT callback: {callback}"
                )))),
            },
            err,
        )?;
        pillow_rs::color3dlut_prepare_table(
            pillow_rs::Color3DLutTable::Flat(output.0),
            size,
            output_channels,
        )
        .map_err(err)
    }

    // Quantize/Reduce
    /// Quantizes the image to at most `colors` palette entries using default options.
    #[wasm_bindgen(js_name = "quantize")]
    pub fn quantize(&self, c: u32) -> Result<Image, JsValue> {
        self.inner
            .quantize(c, 0, None, true, 0)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    /// Quantizes the image with optional color count, method, k-means, dither, and
    /// palette inputs.
    #[wasm_bindgen(js_name = "quantizeWithInput")]
    pub fn quantize_with_input(
        &self,
        colors: JsValue,
        method: JsValue,
        kmeans: JsValue,
        dither: JsValue,
        palette: JsValue,
    ) -> Result<Image, JsValue> {
        let colors = js_optional_i32(&colors)?;
        let method = js_optional_i32(&method)?;
        let kmeans = js_optional_i32(&kmeans)?;
        let dither = if dither.is_null() || dither.is_undefined() {
            None
        } else {
            Some(dither.as_bool().ok_or_else(|| {
                err(pillow_rs::PilError::TypeError(
                    "dither must be a boolean".to_owned(),
                ))
            })?)
        };
        let palette = if palette.is_null() || palette.is_undefined() {
            pillow_rs::QuantizePalette::None
        } else {
            pillow_rs::QuantizePalette::Other
        };
        self.inner
            .quantize_with_input(colors, method, kmeans, palette, dither)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    /// Quantizes the image using `palette` as the destination palette.
    #[wasm_bindgen(js_name = "quantizeWithPaletteInput")]
    pub fn quantize_with_palette_input(
        &self,
        colors: JsValue,
        method: JsValue,
        kmeans: JsValue,
        dither: JsValue,
        palette: &Image,
    ) -> Result<Image, JsValue> {
        let colors = js_optional_i32(&colors)?;
        let method = js_optional_i32(&method)?;
        let kmeans = js_optional_i32(&kmeans)?;
        let dither = if dither.is_null() || dither.is_undefined() {
            None
        } else {
            Some(dither.as_bool().ok_or_else(|| {
                err(pillow_rs::PilError::TypeError(
                    "dither must be a boolean".to_owned(),
                ))
            })?)
        };
        self.inner
            .quantize_with_input(
                colors,
                method,
                kmeans,
                pillow_rs::QuantizePalette::Image(Box::new(palette.inner.clone())),
                dither,
            )
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    /// Reduces image dimensions by the same integer factor on both axes.
    #[wasm_bindgen(js_name = "reduce")]
    pub fn reduce(&self, f: u32) -> Result<Image, JsValue> {
        self.inner
            .reduce(f, f)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    /// Reduces the image using a scalar or per-axis factor and an optional source box.
    #[wasm_bindgen(js_name = "reduceWithInput")]
    pub fn reduce_with_input(
        &self,
        factor: JsValue,
        box_coords: JsValue,
    ) -> Result<Image, JsValue> {
        let box_coords = if box_coords.is_null() || box_coords.is_undefined() {
            None
        } else {
            Some(js_reduce_box(&box_coords))
        };
        self.inner
            .reduce_public(js_reduce_factor(&factor), box_coords)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    /// Returns a copy of the image with its palette remapped by `mapping`.
    #[wasm_bindgen(js_name = "remapPalette")]
    pub fn remap(&mut self, m: Vec<u8>) -> Result<Image, JsValue> {
        self.inner
            .remap_palette(&m)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    /// Applies the spread effect using the given pixel distance.
    #[wasm_bindgen(js_name = "effectSpread")]
    pub fn spread(&self, d: u32) -> Result<Image, JsValue> {
        pillow_rs::image_effect_spread(&self.inner, d)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    /// Adds noise with the given standard deviation.
    #[wasm_bindgen(js_name = "effectNoise")]
    pub fn noise(&self, sigma: f64) -> Result<Image, JsValue> {
        pillow_rs::image_effect_noise(&self.inner, sigma)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    /// Applies a lookup table to the image's pixel values.
    #[wasm_bindgen(js_name = "eval")]
    pub fn eval(&self, lut: Vec<u8>) -> Result<Image, JsValue> {
        pillow_rs::image_eval_replicated_for_image(&self.inner, &lut)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    /// Resizes this image in place so it fits within the requested dimensions.
    #[wasm_bindgen(js_name = "thumbnail")]
    pub fn thumb(&mut self, w: u32, h: u32) -> Result<(), JsValue> {
        self.inner
            .thumbnail((i64::from(w), i64::from(h)), None)
            .map_err(err)
    }
    /// Resizes this image in place using Pillow-style size and resampling inputs.
    ///
    /// `size` must contain two integers. An omitted resampling value uses the core
    /// default; invalid values are returned as JavaScript errors.
    #[wasm_bindgen(js_name = "thumbnailWithInput")]
    pub fn thumb_with_input(&mut self, size: JsValue, resample: JsValue) -> Result<(), JsValue> {
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
        self.inner
            .thumbnail((values[0], values[1]), js_optional_resample(&resample)?)
            .map_err(err)
    }

    // Bookkeeping
}
