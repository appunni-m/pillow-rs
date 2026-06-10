use pyo3::prelude::*;
use pyo3::types::PyType;
use pillow_rs_core::error::PilError;
use pillow_rs_core::image::Image as RsImage;

#[pyclass(name = "Image")]
pub struct PyImage {
    inner: RsImage,
}

#[pymethods]
impl PyImage {
    #[new]
    fn py_new() -> Self {
        // Default 1x1 RGB image for compatibility
        let img = RsImage::new(1, 1, "RGB", (0, 0, 0, 0)).unwrap();
        PyImage { inner: img }
    }

    #[classmethod]
    #[pyo3(signature = (mode, size, color=None))]
    fn new(_cls: &Bound<'_, PyType>, mode: &str, size: (u32, u32), color: Option<&Bound<'_, PyAny>>) -> PyResult<Self> {
        let c = if let Some(val) = color {
            if let Ok(hex_str) = val.extract::<String>() {
                pillow_rs_core::color::parse_color_str(&hex_str).map_err(|e| map_error(e))?
            } else if let Ok(i) = val.extract::<u8>() {
                // PIL: single int fills only the first channel for multi-band modes
                if mode == "L" || mode == "LA" {
                    (i, i, i, 255)
                } else {
                    (i, 0, 0, 255)
                }
            } else if let Ok((r, g, b)) = val.extract::<(u8, u8, u8)>() {
                (r, g, b, 255)
            } else if let Ok((r, g, b, a)) = val.extract::<(u8, u8, u8, u8)>() {
                (r, g, b, a)
            } else if let Ok((l,)) = val.extract::<(u8,)>() {
                // PIL: single-element tuple same as single int
                if mode == "L" || mode == "LA" {
                    (l, l, l, 255)
                } else {
                    (l, 0, 0, 255)
                }
            } else {
                return Err(pyo3::exceptions::PyTypeError::new_err(
                    "color must be int, tuple, or string",
                ));
            }
        } else {
            (0, 0, 0, 0)
        };
        let img = RsImage::new(size.0, size.1, mode, c).map_err(|e| map_error(e))?;
        Ok(PyImage { inner: img })
    }

    #[classmethod]
    fn open(_cls: &Bound<'_, PyType>, fp: &Bound<'_, PyAny>) -> PyResult<Self> {
        if let Ok(path) = fp.extract::<String>() {
            let img = RsImage::open_path(&path).map_err(|e| map_error(e))?;
            Ok(PyImage { inner: img })
        } else if let Ok(bytes) = fp.extract::<Vec<u8>>() {
            let img = RsImage::open_bytes(bytes).map_err(|e| map_error(e))?;
            Ok(PyImage { inner: img })
        } else {
            Err(pyo3::exceptions::PyTypeError::new_err(
                "Expected str or bytes",
            ))
        }
    }

    fn save(&mut self, fp: &str, format: Option<String>) -> PyResult<()> {
        self.inner
            .save(fp, format.as_deref())
            .map_err(|e| map_error(e))
    }

    fn resize(&self, size: (u32, u32), resample: Option<String>) -> PyResult<PyImage> {
        let rs = self
            .inner
            .resize(size, resample.as_deref())
            .map_err(|e| map_error(e))?;
        Ok(PyImage { inner: rs })
    }

    fn crop(&self, box_coords: (u32, u32, u32, u32)) -> PyResult<PyImage> {
        let rs = self.inner.crop(box_coords).map_err(|e| map_error(e))?;
        Ok(PyImage { inner: rs })
    }

    #[pyo3(signature = (angle, expand=false, fillcolor=None))]
    fn rotate(
        &self,
        angle: f64,
        expand: Option<bool>,
        fillcolor: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyImage> {
        let _ = fillcolor;
        let rs = self
            .inner
            .rotate(angle, expand.unwrap_or(false), None)
            .map_err(|e| map_error(e))?;
        Ok(PyImage { inner: rs })
    }

    fn transpose(&self, method: &str) -> PyResult<PyImage> {
        let rs = self.inner.transpose(method).map_err(|e| map_error(e))?;
        Ok(PyImage { inner: rs })
    }

    #[pyo3(signature = (mode, matrix=None, dither=None, palette=None, colors=None))]
    fn convert(
        &self,
        mode: &str,
        matrix: Option<Vec<f64>>,
        dither: Option<String>,
        palette: Option<String>,
        colors: Option<u32>,
    ) -> PyResult<PyImage> {
        let rs = self
            .inner
            .convert(mode, matrix, dither.as_deref(), palette.as_deref(), colors)
            .map_err(|e| map_error(e))?;
        Ok(PyImage { inner: rs })
    }

    #[pyo3(signature = (im, box_coords=None, mask=None))]
    fn paste(
        &mut self,
        im: &Bound<'_, PyAny>,
        box_coords: Option<&Bound<'_, PyAny>>,
        mask: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        use pillow_rs_core::ops::paste::PasteSource;

        // Handle abbreviated syntax: paste(im, mask) where box is actually mask
        let (effective_box, effective_mask): (Option<&Bound<'_, PyAny>>, Option<&Bound<'_, PyAny>>) =
            if let Some(box_val) = box_coords {
                if box_val.downcast::<PyImage>().is_ok() {
                    // Abbreviated: paste(im, mask) — box_val is actually mask
                    (None, Some(box_val))
                } else {
                    (Some(box_val), mask)
                }
            } else {
                (None, mask)
            };

        // Parse source: Image or color
        let source = if let Ok(py_img) = im.downcast::<PyImage>() {
            let borrowed = py_img.borrow();
            PasteSource::Image(borrowed.inner.clone())
        } else if let Ok((r, g, b)) = im.extract::<(u8, u8, u8)>() {
            PasteSource::Color((r, g, b, 255))
        } else if let Ok((r, g, b, a)) = im.extract::<(u8, u8, u8, u8)>() {
            PasteSource::Color((r, g, b, a))
        } else if let Ok(val) = im.extract::<u8>() {
            PasteSource::Color((val, val, val, 255))
        } else {
            return Err(pyo3::exceptions::PyTypeError::new_err(
                "im must be Image or color tuple",
            ));
        };

        // Parse box: 2-tuple or 4-tuple
        let parsed_box = if let Some(box_val) = effective_box {
            if let Ok((x1, y1, x2, y2)) = box_val.extract::<(i32, i32, i32, i32)>() {
                Some((x1, y1, x2, y2))
            } else if let Ok((x, y)) = box_val.extract::<(i32, i32)>() {
                Some((x, y, x, y))
            } else {
                return Err(pyo3::exceptions::PyTypeError::new_err(
                    "box must be (x,y) or (left,upper,right,lower)",
                ));
            }
        } else {
            None
        };

        // Parse mask
        let parsed_mask = if let Some(mask_val) = effective_mask {
            let py_img = mask_val.downcast::<PyImage>().map_err(|_| {
                pyo3::exceptions::PyTypeError::new_err("mask must be an Image")
            })?;
            let borrowed = py_img.borrow();
            Some(borrowed.inner.clone())
        } else {
            None
        };

        let mask_ref = parsed_mask.as_ref();
        self.inner
            .paste(source, parsed_box, mask_ref)
            .map_err(|e| map_error(e))
    }

    fn split(&self) -> PyResult<Vec<PyImage>> {
        let bands = self.inner.split().map_err(|e| map_error(e))?;
        Ok(bands
            .into_iter()
            .map(|img| PyImage { inner: img })
            .collect())
    }

    fn filter(&self, filter_type: &str) -> PyResult<PyImage> {
        let rs = self
            .inner
            .filter(filter_type)
            .map_err(|e| map_error(e))?;
        Ok(PyImage { inner: rs })
    }

    fn copy(&self) -> PyImage {
        PyImage {
            inner: self.inner.copy(),
        }
    }

    fn tobytes(&mut self) -> PyResult<Vec<u8>> {
        self.inner.to_bytes().map_err(|e| map_error(e))
    }

    fn thumbnail(&mut self, size: (u32, u32), resample: Option<String>) -> PyResult<()> {
        self.inner
            .thumbnail(size, resample.as_deref())
            .map_err(|e| map_error(e))
    }

    fn quantize(&self, colors: Option<u32>, dither: Option<bool>) -> PyResult<PyImage> {
        let rs = self
            .inner
            .quantize(colors.unwrap_or(256), 0, None, dither.unwrap_or(true))
            .map_err(|e| map_error(e))?;
        Ok(PyImage { inner: rs })
    }

    fn getbbox(&self, alpha_only: Option<bool>) -> PyResult<Option<(u32, u32, u32, u32)>> {
        self.inner
            .getbbox(alpha_only.unwrap_or(true))
            .map_err(|e| map_error(e))
    }

    fn getextrema(&self) -> PyResult<Vec<(u8, u8)>> {
        self.inner.getextrema().map_err(|e| map_error(e))
    }

    fn histogram(&self) -> PyResult<Vec<u32>> {
        self.inner.histogram().map_err(|e| map_error(e))
    }

    fn gaussian_blur(&self, radius: Option<f64>) -> PyResult<PyImage> {
        let rs = self
            .inner
            .gaussian_blur(radius.unwrap_or(2.0) as f32)
            .map_err(|e| map_error(e))?;
        Ok(PyImage { inner: rs })
    }

    fn unsharp_mask(&self, radius: Option<f64>, percent: Option<i32>, threshold: Option<u8>) -> PyResult<PyImage> {
        let rs = self
            .inner
            .unsharp_mask(radius.unwrap_or(2.0) as f32, percent.unwrap_or(150), threshold.unwrap_or(3))
            .map_err(|e| map_error(e))?;
        Ok(PyImage { inner: rs })
    }

    fn max_filter(&self, size: Option<u32>) -> PyResult<PyImage> {
        let rs = self.inner.max_filter(size.unwrap_or(3))
            .map_err(|e| map_error(e))?;
        Ok(PyImage { inner: rs })
    }

    fn min_filter(&self, size: Option<u32>) -> PyResult<PyImage> {
        let rs = self.inner.min_filter(size.unwrap_or(3))
            .map_err(|e| map_error(e))?;
        Ok(PyImage { inner: rs })
    }

    fn median_filter(&self, size: Option<u32>) -> PyResult<PyImage> {
        let rs = self.inner.median_filter(size.unwrap_or(3))
            .map_err(|e| map_error(e))?;
        Ok(PyImage { inner: rs })
    }

    fn getchannel(&mut self, channel: i32) -> PyResult<PyImage> {
        let rs = self.inner.getchannel(channel).map_err(|e| map_error(e))?;
        Ok(PyImage { inner: rs })
    }

    fn load(&mut self) -> PyResult<()> {
        self.inner.load().map_err(|e| map_error(e))
    }

    fn putalpha(&mut self, alpha: u8) -> PyResult<()> {
        self.inner.putalpha(alpha).map_err(|e| map_error(e))
    }

    fn reduce(&self, factor: u32) -> PyResult<PyImage> {
        let rs = self.inner.reduce(factor).map_err(|e| map_error(e))?;
        Ok(PyImage { inner: rs })
    }

    fn close(&self) -> PyResult<()> {
        // No-op: Rust's Drop handles cleanup
        Ok(())
    }

    fn verify(&self) -> PyResult<()> {
        // Verify image data integrity
        let mut clone = self.inner.clone();
        clone.ensure_loaded().map_err(|e| map_error(e))?;
        Ok(())
    }

    fn enhance_brightness(&self, factor: f64) -> PyResult<PyImage> {
        let rs = self
            .inner
            .enhance_brightness(factor)
            .map_err(|e| map_error(e))?;
        Ok(PyImage { inner: rs })
    }

    fn enhance_contrast(&self, factor: f64) -> PyResult<PyImage> {
        let rs = self
            .inner
            .enhance_contrast(factor)
            .map_err(|e| map_error(e))?;
        Ok(PyImage { inner: rs })
    }

    fn enhance_color(&self, factor: f64) -> PyResult<PyImage> {
        let rs = self
            .inner
            .enhance_color(factor)
            .map_err(|e| map_error(e))?;
        Ok(PyImage { inner: rs })
    }

    fn enhance_sharpness(&self, factor: f64) -> PyResult<PyImage> {
        let rs = self
            .inner
            .enhance_sharpness(factor)
            .map_err(|e| map_error(e))?;
        Ok(PyImage { inner: rs })
    }

    fn getpixel(&mut self, xy: (u32, u32)) -> PyResult<(u8, u8, u8, u8)> {
        self.inner.getpixel(xy.0, xy.1).map_err(|e| map_error(e))
    }

    fn putpixel(&mut self, xy: (u32, u32), value: &Bound<'_, PyAny>) -> PyResult<()> {
        let (r, g, b, a) = if let Ok((r, g, b)) = value.extract::<(u8, u8, u8)>() {
            (r, g, b, 255)
        } else if let Ok((r, g, b, a)) = value.extract::<(u8, u8, u8, u8)>() {
            (r, g, b, a)
        } else if let Ok(val) = value.extract::<u8>() {
            (val, val, val, 255)
        } else {
            return Err(pyo3::exceptions::PyTypeError::new_err(
                "value must be int, RGB tuple, or RGBA tuple",
            ));
        };
        self.inner
            .putpixel(xy.0, xy.1, r, g, b, a)
            .map_err(|e| map_error(e))
    }

    #[getter]
    fn size(&mut self) -> PyResult<(u32, u32)> {
        self.inner.size().map_err(|e| map_error(e))
    }

    #[getter]
    fn width(&mut self) -> PyResult<u32> {
        let (w, _) = self.inner.size().map_err(|e| map_error(e))?;
        Ok(w)
    }

    #[getter]
    fn height(&mut self) -> PyResult<u32> {
        let (_, h) = self.inner.size().map_err(|e| map_error(e))?;
        Ok(h)
    }

    #[getter]
    fn mode(&mut self) -> PyResult<String> {
        self.inner.mode().map_err(|e| map_error(e))
    }

    #[getter]
    fn format(&self) -> Option<String> {
        self.inner.format_name()
    }

    fn __repr__(&mut self) -> String {
        match self.inner.size() {
            Ok((w, h)) => {
                let mode = self.inner.mode().unwrap_or_else(|_| "?".into());
                let fmt = self
                    .inner
                    .format_name()
                    .unwrap_or_else(|| "Unknown".into());
                format!("<Image size={}x{} mode={} format={}>", w, h, mode, fmt)
            }
            Err(_) => "<Image [error loading]>".into(),
        }
    }
}

fn map_error(e: PilError) -> PyErr {
    match e {
        PilError::IOError(msg) => pyo3::exceptions::PyOSError::new_err(msg),
        PilError::UnidentifiedImageError(msg) => pyo3::exceptions::PyValueError::new_err(msg),
        PilError::ValueError(msg) => pyo3::exceptions::PyValueError::new_err(msg),
        PilError::TypeError(msg) => pyo3::exceptions::PyTypeError::new_err(msg),
        PilError::ImageError(err) => pyo3::exceptions::PyException::new_err(err.to_string()),
        PilError::NotImplementedError(msg) => {
            pyo3::exceptions::PyNotImplementedError::new_err(msg)
        }
        PilError::UnknownFormat(msg) => pyo3::exceptions::PyValueError::new_err(msg),
        PilError::Io(err) => pyo3::exceptions::PyOSError::new_err(err.to_string()),
    }
}

#[pymodule]
fn _core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyImage>()?;
    m.add_class::<PyDraw>()?;

    // ImageOps functions
    m.add_function(wrap_pyfunction!(ops_autocontrast, m)?)?;
    m.add_function(wrap_pyfunction!(ops_equalize, m)?)?;
    m.add_function(wrap_pyfunction!(ops_invert, m)?)?;
    m.add_function(wrap_pyfunction!(ops_flip, m)?)?;
    m.add_function(wrap_pyfunction!(ops_mirror, m)?)?;
    m.add_function(wrap_pyfunction!(ops_posterize, m)?)?;
    m.add_function(wrap_pyfunction!(ops_solarize, m)?)?;
    m.add_function(wrap_pyfunction!(ops_grayscale, m)?)?;

    // ImageChops functions
    m.add_function(wrap_pyfunction!(chops_add, m)?)?;
    m.add_function(wrap_pyfunction!(chops_subtract, m)?)?;
    m.add_function(wrap_pyfunction!(chops_multiply, m)?)?;
    m.add_function(wrap_pyfunction!(chops_screen, m)?)?;
    m.add_function(wrap_pyfunction!(chops_darker, m)?)?;
    m.add_function(wrap_pyfunction!(chops_lighter, m)?)?;
    m.add_function(wrap_pyfunction!(chops_difference, m)?)?;
    m.add_function(wrap_pyfunction!(chops_invert, m)?)?;

    // ImageColor
    m.add_function(wrap_pyfunction!(getrgb, m)?)?;

    // Image module functions
    m.add_function(wrap_pyfunction!(image_merge, m)?)?;
    m.add_function(wrap_pyfunction!(image_blend, m)?)?;
    m.add_function(wrap_pyfunction!(image_composite, m)?)?;

    Ok(())
}

// --- ImageDraw ---

#[pyclass(name = "ImageDraw")]
pub struct PyDraw {
    draw: pillow_rs_core::draw::Draw,
}

#[pymethods]
impl PyDraw {
    #[new]
    fn new(image: &Bound<'_, PyImage>) -> PyResult<Self> {
        let borrowed = image.borrow();
        let draw = pillow_rs_core::draw::Draw::new(borrowed.inner.clone());
        Ok(PyDraw { draw })
    }

    fn line(&mut self, xy: Vec<(i32, i32)>, fill: Option<&Bound<'_, PyAny>>, width: Option<u32>) -> PyResult<()> {
        let color = parse_draw_color(fill)?;
        for i in 0..xy.len() - 1 {
            let (x0, y0) = xy[i];
            let (x1, y1) = xy[i + 1];
            self.draw.line(x0, y0, x1, y1, color, width.unwrap_or(1))
                .map_err(|e| map_error(e))?;
        }
        Ok(())
    }

    fn rectangle(&mut self, xy: (i32, i32, i32, i32), fill: Option<&Bound<'_, PyAny>>,
                 outline: Option<&Bound<'_, PyAny>>, width: Option<u32>) -> PyResult<()> {
        let fill_color = if let Some(ref _f) = fill { Some(parse_draw_color(fill)?) } else { None };
        let out_color = if let Some(ref _o) = outline { Some(parse_draw_color(outline)?) } else { Some((0, 0, 0, 255)) };
        self.draw.rectangle(xy.0, xy.1, xy.2, xy.3, fill_color, out_color, width.unwrap_or(1))
            .map_err(|e| map_error(e))
    }

    fn ellipse(&mut self, xy: (i32, i32, i32, i32), fill: Option<&Bound<'_, PyAny>>,
               outline: Option<&Bound<'_, PyAny>>, width: Option<u32>) -> PyResult<()> {
        let fill_color = if let Some(ref _f) = fill { Some(parse_draw_color(fill)?) } else { None };
        let out_color = if let Some(ref _o) = outline { Some(parse_draw_color(outline)?) } else { Some((0, 0, 0, 255)) };
        self.draw.ellipse(xy.0, xy.1, xy.2, xy.3, fill_color, out_color, width.unwrap_or(1))
            .map_err(|e| map_error(e))
    }

    fn polygon(&mut self, xy: Vec<(i32, i32)>, fill: Option<&Bound<'_, PyAny>>,
               outline: Option<&Bound<'_, PyAny>>, width: Option<u32>) -> PyResult<()> {
        let fill_color = if let Some(ref _f) = fill { Some(parse_draw_color(fill)?) } else { None };
        let out_color = if let Some(ref _o) = outline { Some(parse_draw_color(outline)?) } else { Some((0, 0, 0, 255)) };
        self.draw.polygon(&xy, fill_color, out_color, width.unwrap_or(1))
            .map_err(|e| map_error(e))
    }

    fn point(&mut self, xy: Vec<(i32, i32)>, fill: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        let color = parse_draw_color(fill)?;
        self.draw.point(&xy, color).map_err(|e| map_error(e))
    }

    #[getter]
    fn image(&self) -> PyImage {
        // Return a copy of the current image state
        PyImage { inner: self.draw_get_image() }
    }
}

impl PyDraw {
    fn draw_get_image(&self) -> pillow_rs_core::image::Image {
        self.draw.image_clone()
    }
}

fn parse_draw_color(val: Option<&Bound<'_, PyAny>>) -> PyResult<(u8, u8, u8, u8)> {
    let v = match val {
        Some(v) => v,
        None => return Ok((0, 0, 0, 255)), // default black
    };
    if let Ok(s) = v.extract::<String>() {
        pillow_rs_core::color::parse_color_str(&s).map_err(|e| map_error(e))
    } else if let Ok((r, g, b)) = v.extract::<(u8, u8, u8)>() {
        Ok((r, g, b, 255))
    } else if let Ok((r, g, b, a)) = v.extract::<(u8, u8, u8, u8)>() {
        Ok((r, g, b, a))
    } else if let Ok(i) = v.extract::<u8>() {
        Ok((i, i, i, 255))
    } else {
        Err(pyo3::exceptions::PyTypeError::new_err("Expected color tuple, int, or string"))
    }
}

// --- ImageOps module-level functions ---

#[pyfunction]
fn ops_autocontrast(image: &Bound<'_, PyImage>, cutoff: Option<f64>) -> PyResult<PyImage> {
    let borrowed = image.borrow();
    let rs = pillow_rs_core::ops::imageops::autocontrast(&borrowed.inner, cutoff.unwrap_or(0.0))
        .map_err(|e| map_error(e))?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn ops_equalize(image: &Bound<'_, PyImage>) -> PyResult<PyImage> {
    let borrowed = image.borrow();
    let rs = pillow_rs_core::ops::imageops::equalize(&borrowed.inner)
        .map_err(|e| map_error(e))?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn ops_invert(image: &Bound<'_, PyImage>) -> PyResult<PyImage> {
    let borrowed = image.borrow();
    let rs = pillow_rs_core::ops::imageops::invert(&borrowed.inner)
        .map_err(|e| map_error(e))?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn ops_flip(image: &Bound<'_, PyImage>) -> PyResult<PyImage> {
    let borrowed = image.borrow();
    let rs = pillow_rs_core::ops::imageops::flip(&borrowed.inner)
        .map_err(|e| map_error(e))?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn ops_mirror(image: &Bound<'_, PyImage>) -> PyResult<PyImage> {
    let borrowed = image.borrow();
    let rs = pillow_rs_core::ops::imageops::mirror(&borrowed.inner)
        .map_err(|e| map_error(e))?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn ops_posterize(image: &Bound<'_, PyImage>, bits: u8) -> PyResult<PyImage> {
    let borrowed = image.borrow();
    let rs = pillow_rs_core::ops::imageops::posterize(&borrowed.inner, bits)
        .map_err(|e| map_error(e))?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn ops_solarize(image: &Bound<'_, PyImage>, threshold: Option<u8>) -> PyResult<PyImage> {
    let borrowed = image.borrow();
    let rs = pillow_rs_core::ops::imageops::solarize(&borrowed.inner, threshold.unwrap_or(128))
        .map_err(|e| map_error(e))?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn ops_grayscale(image: &Bound<'_, PyImage>) -> PyResult<PyImage> {
    let borrowed = image.borrow();
    let rs = pillow_rs_core::ops::imageops::grayscale(&borrowed.inner)
        .map_err(|e| map_error(e))?;
    Ok(PyImage { inner: rs })
}

// --- ImageChops module-level functions ---

#[pyfunction]
#[pyo3(signature = (image1, image2, scale=1.0, offset=0.0))]
fn chops_add(
    image1: &Bound<'_, PyImage>,
    image2: &Bound<'_, PyImage>,
    scale: f64,
    offset: f64,
) -> PyResult<PyImage> {
    let b1 = image1.borrow();
    let b2 = image2.borrow();
    let rs = pillow_rs_core::ops::chops::add(&b1.inner, &b2.inner, scale, offset)
        .map_err(|e| map_error(e))?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
#[pyo3(signature = (image1, image2, scale=1.0, offset=0.0))]
fn chops_subtract(
    image1: &Bound<'_, PyImage>,
    image2: &Bound<'_, PyImage>,
    scale: f64,
    offset: f64,
) -> PyResult<PyImage> {
    let b1 = image1.borrow();
    let b2 = image2.borrow();
    let rs = pillow_rs_core::ops::chops::subtract(&b1.inner, &b2.inner, scale, offset)
        .map_err(|e| map_error(e))?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn chops_multiply(
    image1: &Bound<'_, PyImage>,
    image2: &Bound<'_, PyImage>,
) -> PyResult<PyImage> {
    let b1 = image1.borrow();
    let b2 = image2.borrow();
    let rs = pillow_rs_core::ops::chops::multiply(&b1.inner, &b2.inner)
        .map_err(|e| map_error(e))?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn chops_screen(
    image1: &Bound<'_, PyImage>,
    image2: &Bound<'_, PyImage>,
) -> PyResult<PyImage> {
    let b1 = image1.borrow();
    let b2 = image2.borrow();
    let rs = pillow_rs_core::ops::chops::screen(&b1.inner, &b2.inner)
        .map_err(|e| map_error(e))?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn chops_darker(
    image1: &Bound<'_, PyImage>,
    image2: &Bound<'_, PyImage>,
) -> PyResult<PyImage> {
    let b1 = image1.borrow();
    let b2 = image2.borrow();
    let rs = pillow_rs_core::ops::chops::darker(&b1.inner, &b2.inner)
        .map_err(|e| map_error(e))?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn chops_lighter(
    image1: &Bound<'_, PyImage>,
    image2: &Bound<'_, PyImage>,
) -> PyResult<PyImage> {
    let b1 = image1.borrow();
    let b2 = image2.borrow();
    let rs = pillow_rs_core::ops::chops::lighter(&b1.inner, &b2.inner)
        .map_err(|e| map_error(e))?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn chops_difference(
    image1: &Bound<'_, PyImage>,
    image2: &Bound<'_, PyImage>,
) -> PyResult<PyImage> {
    let b1 = image1.borrow();
    let b2 = image2.borrow();
    let rs = pillow_rs_core::ops::chops::difference(&b1.inner, &b2.inner)
        .map_err(|e| map_error(e))?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn chops_invert(image: &Bound<'_, PyImage>) -> PyResult<PyImage> {
    let borrowed = image.borrow();
    let rs = pillow_rs_core::ops::chops::invert(&borrowed.inner)
        .map_err(|e| map_error(e))?;
    Ok(PyImage { inner: rs })
}

// --- Image module functions ---

#[pyfunction]
fn image_merge(mode: &str, bands: &Bound<'_, PyAny>) -> PyResult<PyImage> {
    let mut band_images: Vec<pillow_rs_core::image::Image> = Vec::new();
    for item in bands.iter()? {
        let obj = item?;
        let py_img = obj.downcast::<PyImage>().map_err(|_| {
            pyo3::exceptions::PyTypeError::new_err("bands must be a sequence of Image objects")
        })?;
        band_images.push(py_img.borrow().inner.clone());
    }
    let rs = pillow_rs_core::ops::module_fns::merge(mode, &band_images)
        .map_err(|e| map_error(e))?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn image_blend(image1: &Bound<'_, PyImage>, image2: &Bound<'_, PyImage>, alpha: f64) -> PyResult<PyImage> {
    let b1 = image1.borrow();
    let b2 = image2.borrow();
    let rs = pillow_rs_core::ops::module_fns::blend(&b1.inner, &b2.inner, alpha)
        .map_err(|e| map_error(e))?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn image_composite(image1: &Bound<'_, PyImage>, image2: &Bound<'_, PyImage>, mask: &Bound<'_, PyImage>) -> PyResult<PyImage> {
    let b1 = image1.borrow();
    let b2 = image2.borrow();
    let bm = mask.borrow();
    let rs = pillow_rs_core::ops::module_fns::composite(&b1.inner, &b2.inner, &bm.inner)
        .map_err(|e| map_error(e))?;
    Ok(PyImage { inner: rs })
}

// --- ImageColor ---

#[pyfunction]
fn getrgb(color: &str) -> PyResult<(u8, u8, u8)> {
    pillow_rs_core::color::parse_color_str(color)
        .map(|(r, g, b, _a)| (r, g, b))
        .map_err(|e| map_error(e))
}
