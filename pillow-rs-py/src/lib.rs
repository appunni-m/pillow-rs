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
        let img = RsImage::new(1, 1, "RGB", (0, 0, 0, 0))
            .expect("Default 1x1 RGB image creation should never fail");
        PyImage { inner: img }
    }

    #[classmethod]
    #[pyo3(signature = (mode, size, color=None))]
    fn new(_cls: &Bound<'_, PyType>, mode: &str, size: (u32, u32), color: Option<&Bound<'_, PyAny>>) -> PyResult<Self> {
        // Thin binding: extract Python types, delegate logic to core
        let (hex, single, rgb, rgba, la) = if let Some(val) = color {
            (
                val.extract::<String>().ok(),
                val.extract::<u8>().ok(),
                val.extract::<(u8, u8, u8)>().ok(),
                val.extract::<(u8, u8, u8, u8)>().ok(),
                val.extract::<(u8, u8)>().ok(),
            )
        } else { (None, None, None, None, None) };
        let c = pillow_rs_core::color::resolve_new_color(mode, hex.as_deref(), single, rgb, rgba, la)
            .map_err(map_error)?;
        let img = RsImage::new(size.0, size.1, mode, c).map_err(map_error)?;
        Ok(PyImage { inner: img })
    }

    #[classmethod]
    fn open(_cls: &Bound<'_, PyType>, fp: &Bound<'_, PyAny>) -> PyResult<Self> {
        if let Ok(path) = fp.extract::<String>() {
            let img = RsImage::open(&path, None).map_err(map_error)?;
            Ok(PyImage { inner: img })
        } else if let Ok(bytes) = fp.extract::<Vec<u8>>() {
            let img = RsImage::open_bytes(bytes).map_err(map_error)?;
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
            .map_err(map_error)
    }

    fn resize(&self, size: (u32, u32), resample: Option<String>) -> PyResult<PyImage> {
        let rs = self
            .inner
            .resize(size, resample.as_deref())
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn crop(&self, box_coords: (u32, u32, u32, u32)) -> PyResult<PyImage> {
        let rs = self.inner.crop(box_coords).map_err(map_error)?;
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
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn transpose(&self, method: &str) -> PyResult<PyImage> {
        let rs = self.inner.transpose(method).map_err(map_error)?;
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
            .map_err(map_error)?;
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
        // Thin binding: extract Python types, core handles all logic
        let is_abbreviated = box_coords.map_or(false, |b| b.downcast::<PyImage>().is_ok());
        let effective_mask = if is_abbreviated { box_coords } else { mask };

        let src_image = im.downcast::<PyImage>().ok().map(|p| p.borrow().inner.clone());
        let src_rgb = im.extract::<(u8, u8, u8)>().ok();
        let src_rgba = im.extract::<(u8, u8, u8, u8)>().ok();
        let src_int = im.extract::<u8>().ok();
        let source = if let Some(img) = src_image {
            PasteSource::Image(img)
        } else if let Some((r, g, b, a)) = src_rgba {
            PasteSource::Color((r, g, b, a))
        } else if let Some((r, g, b)) = src_rgb {
            PasteSource::Color((r, g, b, 255))
        } else if let Some(v) = src_int {
            PasteSource::Color((v, v, v, 255))
        } else {
            return Err(pyo3::exceptions::PyTypeError::new_err("im must be Image or color"));
        };

        let parsed_box = if is_abbreviated { None } else {
            box_coords.and_then(|b| {
                b.extract::<(i32,i32,i32,i32)>().ok()
                    .or_else(|| b.extract::<(i32,i32)>().ok().map(|(x, y)| (x, y, x, y)))
            })
        };

        let parsed_mask = effective_mask
            .and_then(|m| m.downcast::<PyImage>().ok())
            .map(|p| p.borrow().inner.clone());

        self.inner.paste(source, parsed_box, parsed_mask.as_ref()).map_err(map_error)
    }

    fn split(&self) -> PyResult<Vec<PyImage>> {
        let bands = self.inner.split().map_err(map_error)?;
        Ok(bands
            .into_iter()
            .map(|img| PyImage { inner: img })
            .collect())
    }

    fn filter(&self, filter_type: &str) -> PyResult<PyImage> {
        let rs = self
            .inner
            .filter(filter_type)
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn copy(&self) -> PyImage {
        PyImage {
            inner: self.inner.copy(),
        }
    }

    fn tobytes(&self) -> PyResult<Vec<u8>> {
        self.inner.tobytes().map_err(map_error)
    }

    fn thumbnail(&mut self, size: (u32, u32), _resample: Option<String>) -> PyResult<()> {
        self.inner
            .thumbnail(size)
            .map_err(map_error)
    }

    fn quantize(&self, colors: Option<u32>, dither: Option<bool>) -> PyResult<PyImage> {
        let rs = self
            .inner
            .quantize(colors.unwrap_or(256), 0, None, dither.unwrap_or(true))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn getbbox(&self, alpha_only: Option<bool>) -> PyResult<Option<(u32, u32, u32, u32)>> {
        self.inner
            .getbbox(alpha_only.unwrap_or(true))
            .map_err(map_error)
    }

    fn getextrema(&self) -> PyResult<Vec<(u8, u8)>> {
        self.inner.getextrema().map_err(map_error)
    }

    fn stat(&self) -> PyResult<Vec<Vec<f64>>> {
        self.inner.stat().map_err(map_error)
    }

    fn histogram(&self) -> PyResult<Vec<u32>> {
        self.inner.histogram().map_err(map_error)
    }

    fn gaussian_blur(&self, radius: Option<f64>) -> PyResult<PyImage> {
        let rs = self
            .inner
            .gaussian_blur(radius.unwrap_or(2.0) as f32)
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn unsharp_mask(&self, radius: Option<f64>, percent: Option<i32>, threshold: Option<u8>) -> PyResult<PyImage> {
        let rs = self
            .inner
            .unsharp_mask(radius.unwrap_or(2.0) as f32, percent.unwrap_or(150), threshold.unwrap_or(3))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn max_filter(&self, size: Option<u32>) -> PyResult<PyImage> {
        let rs = self.inner.max_filter(size.unwrap_or(3))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn min_filter(&self, size: Option<u32>) -> PyResult<PyImage> {
        let rs = self.inner.min_filter(size.unwrap_or(3))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn median_filter(&self, size: Option<u32>) -> PyResult<PyImage> {
        let rs = self.inner.median_filter(size.unwrap_or(3))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn getchannel(&mut self, channel: i32) -> PyResult<PyImage> {
        let rs = self.inner.getchannel(channel).map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn load(&mut self) -> PyResult<()> {
        self.inner.load().map_err(map_error)
    }

    fn putalpha(&mut self, alpha: u8) -> PyResult<()> {
        self.inner.putalpha(alpha).map_err(map_error)
    }

    fn reduce(&self, factor: u32) -> PyResult<PyImage> {
        let rs = self.inner.reduce(factor).map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn alpha_composite(&mut self, im: &Bound<'_, PyImage>) -> PyResult<()> {
        let source = im.borrow().inner.clone();
        self.inner.alpha_composite(&source, (0, 0), (0, 0))
            .map_err(map_error)
    }

    fn getcolors(&mut self, maxcolors: Option<u32>) -> PyResult<Option<Vec<(u32, Vec<u8>)>>> {
        self.inner.getcolors(maxcolors.unwrap_or(256)).map_err(map_error)
    }

    fn getdata(&mut self, band: Option<i32>) -> PyResult<Vec<u8>> {
        self.inner.getdata(band).map_err(map_error)
    }

    fn getprojection(&mut self) -> PyResult<(Vec<u32>, Vec<u32>)> {
        self.inner.getprojection().map_err(map_error)
    }

    fn entropy(&mut self) -> PyResult<f64> {
        self.inner.entropy().map_err(map_error)
    }

    fn seek(&mut self, frame: u32) -> PyResult<()> {
        self.inner.seek(frame).map_err(map_error)
    }

    fn tell(&self) -> u32 {
        self.inner.tell()
    }

    fn point(&self, lut: Vec<u8>) -> PyResult<PyImage> {
        pillow_rs_core::ops::module_fns::eval(&self.inner, &lut).map(|i| PyImage { inner: i }).map_err(map_error)
    }

    fn effect_spread(&self, distance: u32) -> PyResult<PyImage> {
        pillow_rs_core::ops::module_fns::effect_spread(&self.inner, distance).map(|i| PyImage { inner: i }).map_err(map_error)
    }

    #[pyo3(signature = (size, method, data=None, resample=0, fill=1, fillcolor=None))]
    fn transform(&self, size: (u32, u32), method: &str, data: Option<Vec<f64>>,
                 resample: Option<i32>, fill: Option<i32>, fillcolor: Option<&Bound<'_, PyAny>>) -> PyResult<PyImage> {
        let _ = (resample, fill);
        let fill = if let Some(fc) = fillcolor {
            if let Ok((r, g, b)) = fc.extract::<(u8, u8, u8)>() { (r, g, b, 255) }
            else if let Ok((r, g, b, a)) = fc.extract::<(u8, u8, u8, u8)>() { (r, g, b, a) }
            else if let Ok(i) = fc.extract::<u8>() { (i, i, i, 255) }
            else { (0, 0, 0, 255) }
        } else { (0, 0, 0, 255) };

        match method {
            "AFFINE" => {
                let matrix = data.ok_or_else(|| pyo3::exceptions::PyValueError::new_err("AFFINE requires data"))?;
                self.inner.transform_affine(size, &matrix, fill)
                    .map(|i| PyImage { inner: i }).map_err(map_error)
            }
            _ => Err(pyo3::exceptions::PyNotImplementedError::new_err(
                format!("Transform method '{}' not yet implemented", method)
            ))
        }
    }

    #[staticmethod]
    fn frombytes(mode: &str, size: (u32, u32), data: Vec<u8>) -> PyResult<PyImage> {
        pillow_rs_core::image::Image::frombytes(mode, size, &data)
            .map(|img| PyImage { inner: img })
            .map_err(map_error)
    }

    fn remap_palette(&mut self, dest_map: Vec<u8>) -> PyResult<PyImage> {
        self.inner.remap_palette(&dest_map).map(|i| PyImage { inner: i }).map_err(map_error)
    }

    fn tobitmap(&mut self) -> PyResult<Vec<u8>> {
        self.inner.tobitmap().map_err(map_error)
    }

    fn effect_noise(&self, sigma: Option<f64>) -> PyResult<PyImage> {
        pillow_rs_core::ops::module_fns::effect_noise(&self.inner, sigma.unwrap_or(10.0))
            .map(|img| PyImage { inner: img })
            .map_err(map_error)
    }

    #[classmethod]
    fn blend(
        _cls: &Bound<'_, PyType>,
        image1: &Bound<'_, PyImage>,
        image2: &Bound<'_, PyImage>,
        alpha: f64,
    ) -> PyResult<PyImage> {
        let im1 = image1.borrow();
        let im2 = image2.borrow();
        pillow_rs_core::ops::module_fns::blend(&im1.inner, &im2.inner, alpha)
            .map(|img| PyImage { inner: img })
            .map_err(map_error)
    }

    #[classmethod]
    fn composite(
        _cls: &Bound<'_, PyType>,
        image1: &Bound<'_, PyImage>,
        image2: &Bound<'_, PyImage>,
        mask: &Bound<'_, PyImage>,
    ) -> PyResult<PyImage> {
        let im1 = image1.borrow();
        let im2 = image2.borrow();
        let m = mask.borrow();
        pillow_rs_core::ops::module_fns::composite(&im1.inner, &im2.inner, &m.inner)
            .map(|img| PyImage { inner: img })
            .map_err(map_error)
    }

    #[classmethod]
    fn merge(
        _cls: &Bound<'_, PyType>,
        mode: &str,
        bands: &Bound<'_, PyAny>,
    ) -> PyResult<PyImage> {
        let mut images = Vec::new();
        for item in bands.iter()? {
            let obj = item?;
            let py_img = obj.downcast::<PyImage>().map_err(|_| {
                pyo3::exceptions::PyTypeError::new_err("bands must be a sequence of Image objects")
            })?;
            images.push(py_img.borrow().inner.clone());
        }
        pillow_rs_core::ops::module_fns::merge(mode, &images)
            .map(|img| PyImage { inner: img })
            .map_err(map_error)
    }

    fn close(&self) -> PyResult<()> {
        // No-op: Rust's Drop handles cleanup
        Ok(())
    }

    fn verify(&self) -> PyResult<()> {
        // Verify image data integrity
        self.inner.materialize().map_err(map_error)?;
        Ok(())
    }

    fn enhance_brightness(&self, factor: f64) -> PyResult<PyImage> {
        let inner = self.inner.clone();
        let rs = Python::with_gil(|py| {
            py.allow_threads(|| inner.enhance_brightness(factor))
        }).map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn enhance_contrast(&self, factor: f64) -> PyResult<PyImage> {
        let inner = self.inner.clone();
        let rs = Python::with_gil(|py| {
            py.allow_threads(|| inner.enhance_contrast(factor))
        }).map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn enhance_color(&self, factor: f64) -> PyResult<PyImage> {
        let inner = self.inner.clone();
        let rs = Python::with_gil(|py| {
            py.allow_threads(|| inner.enhance_color(factor))
        }).map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn enhance_sharpness(&self, factor: f64) -> PyResult<PyImage> {
        let inner = self.inner.clone();
        let rs = Python::with_gil(|py| {
            py.allow_threads(|| inner.enhance_sharpness(factor))
        }).map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn getpixel(&mut self, xy: (u32, u32)) -> PyResult<(u8, u8, u8, u8)> {
        self.inner.getpixel(xy.0, xy.1).map_err(map_error)
    }

    fn putdata(&mut self, data: &Bound<'_, PyAny>) -> PyResult<()> {
        // Flatten sequence in Rust — handles ints and tuples
        let mut flat: Vec<u8> = Vec::new();
        for item in data.iter()? {
            let obj = item?;
            if let Ok(t) = obj.extract::<(u8, u8, u8, u8)>() {
                flat.extend_from_slice(&[t.0, t.1, t.2, t.3]);
            } else if let Ok(t) = obj.extract::<(u8, u8, u8)>() {
                flat.extend_from_slice(&[t.0, t.1, t.2]);
            } else if let Ok(t) = obj.extract::<(u8, u8)>() {
                flat.extend_from_slice(&[t.0, t.1]);
            } else if let Ok(v) = obj.extract::<u8>() {
                flat.push(v);
            } else if let Ok(v) = obj.extract::<i64>() {
                flat.push(v.clamp(0, 255) as u8);
            }
        }
        self.inner.putdata(&flat).map_err(map_error)
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
            .map_err(map_error)
    }

    #[getter]
    fn size(&mut self) -> PyResult<(u32, u32)> {
        self.inner.size().map_err(map_error)
    }

    #[getter]
    fn width(&mut self) -> PyResult<u32> {
        let (w, _) = self.inner.size().map_err(map_error)?;
        Ok(w)
    }

    #[getter]
    fn height(&mut self) -> PyResult<u32> {
        let (_, h) = self.inner.size().map_err(map_error)?;
        Ok(h)
    }

    #[getter]
    fn mode(&mut self) -> PyResult<String> {
        self.inner.mode().map_err(map_error)
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
    m.add_class::<PyFont>()?;

    // ImageOps functions
    m.add_function(wrap_pyfunction!(ops_autocontrast, m)?)?;
    m.add_function(wrap_pyfunction!(ops_equalize, m)?)?;
    m.add_function(wrap_pyfunction!(ops_invert, m)?)?;
    m.add_function(wrap_pyfunction!(ops_flip, m)?)?;
    m.add_function(wrap_pyfunction!(ops_mirror, m)?)?;
    m.add_function(wrap_pyfunction!(ops_posterize, m)?)?;
    m.add_function(wrap_pyfunction!(ops_solarize, m)?)?;
    m.add_function(wrap_pyfunction!(ops_grayscale, m)?)?;
    m.add_function(wrap_pyfunction!(ops_colorize, m)?)?;

    // ImageChops functions
    m.add_function(wrap_pyfunction!(chops_add, m)?)?;
    m.add_function(wrap_pyfunction!(chops_subtract, m)?)?;
    m.add_function(wrap_pyfunction!(chops_multiply, m)?)?;
    m.add_function(wrap_pyfunction!(chops_screen, m)?)?;
    m.add_function(wrap_pyfunction!(chops_darker, m)?)?;
    m.add_function(wrap_pyfunction!(chops_lighter, m)?)?;
    m.add_function(wrap_pyfunction!(chops_difference, m)?)?;
    m.add_function(wrap_pyfunction!(chops_invert, m)?)?;

    // More ImageChops
    m.add_function(wrap_pyfunction!(chops_add_modulo, m)?)?;
    m.add_function(wrap_pyfunction!(chops_subtract_modulo, m)?)?;
    m.add_function(wrap_pyfunction!(chops_constant, m)?)?;
    m.add_function(wrap_pyfunction!(chops_hard_light, m)?)?;
    m.add_function(wrap_pyfunction!(chops_soft_light, m)?)?;
    m.add_function(wrap_pyfunction!(chops_overlay, m)?)?;
    m.add_function(wrap_pyfunction!(chops_logical_and, m)?)?;
    m.add_function(wrap_pyfunction!(chops_logical_or, m)?)?;
    m.add_function(wrap_pyfunction!(chops_logical_xor, m)?)?;
    m.add_function(wrap_pyfunction!(chops_offset, m)?)?;

    // ImageColor
    m.add_function(wrap_pyfunction!(getrgb, m)?)?;

    // Image module functions
    m.add_function(wrap_pyfunction!(image_merge, m)?)?;
    m.add_function(wrap_pyfunction!(image_blend, m)?)?;
    m.add_function(wrap_pyfunction!(image_composite, m)?)?;

    Ok(())
}

// --- ImageFont ---

#[pyclass(name = "ImageFont")]
pub struct PyFont {
    inner: pillow_rs_core::font::Font,
}

#[pymethods]
impl PyFont {
    #[staticmethod]
    fn truetype(fp: &str, size: f64) -> PyResult<Self> {
        let data = std::fs::read(fp)
            .map_err(|e| pyo3::exceptions::PyOSError::new_err(format!("Cannot read font file: {}", e)))?;
        let font = pillow_rs_core::font::Font::from_bytes(data, size as f32)
            .map_err(map_error)?;
        Ok(PyFont { inner: font })
    }

    #[staticmethod]
    fn load_default() -> PyResult<Self> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "load_default(): use ImageFont.truetype('path.ttf', size) instead"
        ))
    }

    fn getbbox(&self, text: &str) -> PyResult<(u32, u32)> {
        Ok(self.inner.text_bbox(text))
    }

    fn getmask_alpha(&self, text: &str) -> PyResult<(u32, u32, Vec<u8>)> {
        Ok(self.inner.getmask(text))
    }

    fn get_size(&self) -> f32 {
        self.inner.font_size()
    }
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
                .map_err(map_error)?;
        }
        Ok(())
    }

    fn rectangle(&mut self, xy: (i32, i32, i32, i32), fill: Option<&Bound<'_, PyAny>>,
                 outline: Option<&Bound<'_, PyAny>>, width: Option<u32>) -> PyResult<()> {
        let fill_color = if let Some(_f) = fill { Some(parse_draw_color(fill)?) } else { None };
        let out_color = if let Some(_o) = outline { Some(parse_draw_color(outline)?) } else { Some((0, 0, 0, 255)) };
        self.draw.rectangle(xy.0, xy.1, xy.2, xy.3, fill_color, out_color, width.unwrap_or(1))
            .map_err(map_error)
    }

    fn ellipse(&mut self, xy: (i32, i32, i32, i32), fill: Option<&Bound<'_, PyAny>>,
               outline: Option<&Bound<'_, PyAny>>, width: Option<u32>) -> PyResult<()> {
        let fill_color = if let Some(_f) = fill { Some(parse_draw_color(fill)?) } else { None };
        let out_color = if let Some(_o) = outline { Some(parse_draw_color(outline)?) } else { Some((0, 0, 0, 255)) };
        self.draw.ellipse(xy.0, xy.1, xy.2, xy.3, fill_color, out_color, width.unwrap_or(1))
            .map_err(map_error)
    }

    fn regular_polygon(&mut self, bounding_circle: &Bound<'_, PyAny>,
                        n_sides: u32, rotation: Option<f64>,
                        fill: Option<&Bound<'_, PyAny>>,
                        outline: Option<&Bound<'_, PyAny>>, width: Option<u32>) -> PyResult<()> {
        use std::f64::consts::TAU;
        let (cx, cy, r): (f64, f64, f64) = if let Ok((x, y, r)) = bounding_circle.extract::<(f64, f64, f64)>() {
            (x, y, r)
        } else if let Ok(((x, y), r)) = bounding_circle.extract::<((f64, f64), f64)>() {
            (x, y, r)
        } else if let Ok((x, y, r)) = bounding_circle.extract::<(i32, i32, i32)>() {
            (x as f64, y as f64, r as f64)
        } else {
            return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>("bounding_circle must be (x,y,r) or ((x,y),r)"));
        };
        let rot = rotation.unwrap_or(0.0).to_radians();
        let mut pts = Vec::with_capacity(n_sides as usize);
        for i in 0..n_sides {
            let angle = rot + TAU * i as f64 / n_sides as f64;
            pts.push(((cx + r * angle.cos()) as i32, (cy + r * angle.sin()) as i32));
        }
        let fill_color = if let Some(_f) = fill { Some(parse_draw_color(fill)?) } else { None };
        let out_color = if let Some(_o) = outline { Some(parse_draw_color(outline)?) } else { Some((0, 0, 0, 255)) };
        self.draw.polygon(&pts, fill_color, out_color, width.unwrap_or(1)).map_err(map_error)
    }

    fn polygon(&mut self, xy: Vec<(i32, i32)>, fill: Option<&Bound<'_, PyAny>>,
               outline: Option<&Bound<'_, PyAny>>, width: Option<u32>) -> PyResult<()> {
        let fill_color = if let Some(_f) = fill { Some(parse_draw_color(fill)?) } else { None };
        let out_color = if let Some(_o) = outline { Some(parse_draw_color(outline)?) } else { Some((0, 0, 0, 255)) };
        self.draw.polygon(&xy, fill_color, out_color, width.unwrap_or(1))
            .map_err(map_error)
    }

    fn point(&mut self, xy: Vec<(i32, i32)>, fill: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        let color = parse_draw_color(fill)?;
        self.draw.point(&xy, color).map_err(map_error)
    }

    #[pyo3(signature = (xy, start, end, fill=None, width=1))]
    fn arc(&mut self, xy: (i32, i32, i32, i32), start: f64, end: f64,
           fill: Option<&Bound<'_, PyAny>>, width: Option<u32>) -> PyResult<()> {
        let color = parse_draw_color(fill)?;
        self.draw.arc(xy.0, xy.1, xy.2, xy.3, start, end, color, width.unwrap_or(1))
            .map_err(map_error)
    }

    #[pyo3(signature = (xy, start, end, fill=None, outline=None, width=1))]
    fn chord(&mut self, xy: (i32, i32, i32, i32), start: f64, end: f64,
             fill: Option<&Bound<'_, PyAny>>, outline: Option<&Bound<'_, PyAny>>, width: Option<u32>) -> PyResult<()> {
        let fc = fill.map(|_| parse_draw_color(fill).unwrap_or((0,0,0,255)));
        let oc = outline.map(|_| parse_draw_color(outline).unwrap_or((0,0,0,255)));
        self.draw.chord(xy.0, xy.1, xy.2, xy.3, start, end, fc, oc, width.unwrap_or(1))
            .map_err(map_error)
    }

    #[pyo3(signature = (xy, start, end, fill=None, outline=None, width=1))]
    fn pieslice(&mut self, xy: (i32, i32, i32, i32), start: f64, end: f64,
                fill: Option<&Bound<'_, PyAny>>, outline: Option<&Bound<'_, PyAny>>, width: Option<u32>) -> PyResult<()> {
        let fc = fill.map(|_| parse_draw_color(fill).unwrap_or((0,0,0,255)));
        let oc = outline.map(|_| parse_draw_color(outline).unwrap_or((0,0,0,255)));
        self.draw.pieslice(xy.0, xy.1, xy.2, xy.3, start, end, fc, oc, width.unwrap_or(1))
            .map_err(map_error)
    }

    #[pyo3(signature = (xy, radius, fill=None, outline=None, width=1))]
    fn circle(&mut self, xy: (f64, f64), radius: f64,
              fill: Option<&Bound<'_, PyAny>>, outline: Option<&Bound<'_, PyAny>>, width: Option<u32>) -> PyResult<()> {
        let fc = fill.map(|_| parse_draw_color(fill).unwrap_or((0,0,0,255)));
        let oc = outline.map(|_| parse_draw_color(outline).unwrap_or((0,0,0,255)));
        self.draw.circle(xy.0 as i32, xy.1 as i32, radius, fc, oc, width.unwrap_or(1))
            .map_err(map_error)
    }

    #[pyo3(signature = (xy, radius=0.0, fill=None, outline=None, width=1))]
    fn rounded_rectangle(&mut self, xy: (i32, i32, i32, i32), radius: f64,
                         fill: Option<&Bound<'_, PyAny>>, outline: Option<&Bound<'_, PyAny>>, width: Option<u32>) -> PyResult<()> {
        let fc = fill.map(|_| parse_draw_color(fill).unwrap_or((0,0,0,255)));
        let oc = outline.map(|_| parse_draw_color(outline).unwrap_or((0,0,0,255)));
        self.draw.rounded_rectangle(xy.0, xy.1, xy.2, xy.3, radius, fc, oc, width.unwrap_or(1))
            .map_err(map_error)
    }

    fn text(&mut self, xy: (f64, f64), text: String, fill: Option<&Bound<'_, PyAny>>,
            font: Option<&Bound<'_, PyFont>>) -> PyResult<()> {
        let color = parse_draw_color(fill)?;
        if let Some(pyfont) = font {
            let borrowed = pyfont.borrow();
            self.draw.text(xy.0 as i32, xy.1 as i32, &text, &borrowed.inner, color)
                .map_err(map_error)
        } else {
            Err(pyo3::exceptions::PyNotImplementedError::new_err("text() requires a font"))
        }
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
        pillow_rs_core::color::parse_color_str(&s).map_err(map_error)
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
    let inner = image.borrow().inner.clone();
    let c = cutoff.unwrap_or(0.0);
    let rs = Python::with_gil(|py| {
        py.allow_threads(|| pillow_rs_core::ops::imageops::autocontrast(&inner, c))
    }).map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn ops_equalize(image: &Bound<'_, PyImage>) -> PyResult<PyImage> {
    let inner = image.borrow().inner.clone();
    let rs = Python::with_gil(|py| {
        py.allow_threads(|| pillow_rs_core::ops::imageops::equalize(&inner))
    }).map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn ops_invert(image: &Bound<'_, PyImage>) -> PyResult<PyImage> {
    let inner = image.borrow().inner.clone();
    let rs = Python::with_gil(|py| {
        py.allow_threads(|| pillow_rs_core::ops::imageops::invert(&inner))
    }).map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn ops_flip(image: &Bound<'_, PyImage>) -> PyResult<PyImage> {
    let inner = image.borrow().inner.clone();
    let rs = Python::with_gil(|py| {
        py.allow_threads(|| pillow_rs_core::ops::imageops::flip(&inner))
    }).map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn ops_mirror(image: &Bound<'_, PyImage>) -> PyResult<PyImage> {
    let inner = image.borrow().inner.clone();
    let rs = Python::with_gil(|py| {
        py.allow_threads(|| pillow_rs_core::ops::imageops::mirror(&inner))
    }).map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn ops_posterize(image: &Bound<'_, PyImage>, bits: u8) -> PyResult<PyImage> {
    let inner = image.borrow().inner.clone();
    let rs = Python::with_gil(|py| {
        py.allow_threads(|| pillow_rs_core::ops::imageops::posterize(&inner, bits))
    }).map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn ops_solarize(image: &Bound<'_, PyImage>, threshold: Option<u8>) -> PyResult<PyImage> {
    let inner = image.borrow().inner.clone();
    let t = threshold.unwrap_or(128);
    let rs = Python::with_gil(|py| {
        py.allow_threads(|| pillow_rs_core::ops::imageops::solarize(&inner, t))
    }).map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn ops_grayscale(image: &Bound<'_, PyImage>) -> PyResult<PyImage> {
    let inner = image.borrow().inner.clone();
    let rs = Python::with_gil(|py| {
        py.allow_threads(|| pillow_rs_core::ops::imageops::grayscale(&inner))
    }).map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn ops_colorize(image: &Bound<'_, PyImage>, black: (u8, u8, u8), white: (u8, u8, u8)) -> PyResult<PyImage> {
    let inner = image.borrow().inner.clone();
    let rs = Python::with_gil(|py| {
        py.allow_threads(|| pillow_rs_core::ops::imageops::colorize(&inner, black, white))
    }).map_err(map_error)?;
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
    let b1 = image1.borrow().inner.clone();
    let b2 = image2.borrow().inner.clone();
    let rs = Python::with_gil(|py| {
        py.allow_threads(|| pillow_rs_core::ops::chops::add(&b1, &b2, scale, offset))
    }).map_err(map_error)?;
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
    let b1 = image1.borrow().inner.clone();
    let b2 = image2.borrow().inner.clone();
    let rs = Python::with_gil(|py| {
        py.allow_threads(|| pillow_rs_core::ops::chops::subtract(&b1, &b2, scale, offset))
    }).map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn chops_multiply(
    image1: &Bound<'_, PyImage>,
    image2: &Bound<'_, PyImage>,
) -> PyResult<PyImage> {
    let b1 = image1.borrow().inner.clone();
    let b2 = image2.borrow().inner.clone();
    let rs = Python::with_gil(|py| {
        py.allow_threads(|| pillow_rs_core::ops::chops::multiply(&b1, &b2))
    }).map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn chops_screen(
    image1: &Bound<'_, PyImage>,
    image2: &Bound<'_, PyImage>,
) -> PyResult<PyImage> {
    let b1 = image1.borrow().inner.clone();
    let b2 = image2.borrow().inner.clone();
    let rs = Python::with_gil(|py| {
        py.allow_threads(|| pillow_rs_core::ops::chops::screen(&b1, &b2))
    }).map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn chops_darker(
    image1: &Bound<'_, PyImage>,
    image2: &Bound<'_, PyImage>,
) -> PyResult<PyImage> {
    let b1 = image1.borrow().inner.clone();
    let b2 = image2.borrow().inner.clone();
    let rs = Python::with_gil(|py| {
        py.allow_threads(|| pillow_rs_core::ops::chops::darker(&b1, &b2))
    }).map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn chops_lighter(
    image1: &Bound<'_, PyImage>,
    image2: &Bound<'_, PyImage>,
) -> PyResult<PyImage> {
    let b1 = image1.borrow().inner.clone();
    let b2 = image2.borrow().inner.clone();
    let rs = Python::with_gil(|py| {
        py.allow_threads(|| pillow_rs_core::ops::chops::lighter(&b1, &b2))
    }).map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn chops_difference(
    image1: &Bound<'_, PyImage>,
    image2: &Bound<'_, PyImage>,
) -> PyResult<PyImage> {
    let b1 = image1.borrow().inner.clone();
    let b2 = image2.borrow().inner.clone();
    let rs = Python::with_gil(|py| {
        py.allow_threads(|| pillow_rs_core::ops::chops::difference(&b1, &b2))
    }).map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn chops_invert(image: &Bound<'_, PyImage>) -> PyResult<PyImage> {
    let borrowed = image.borrow();
    let rs = pillow_rs_core::ops::chops::invert(&borrowed.inner)
        .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn chops_add_modulo(image1: &Bound<'_, PyImage>, image2: &Bound<'_, PyImage>) -> PyResult<PyImage> {
    let b1 = image1.borrow(); let b2 = image2.borrow();
    pillow_rs_core::ops::chops::add_modulo(&b1.inner, &b2.inner).map(|i| PyImage { inner: i }).map_err(map_error)
}

#[pyfunction]
fn chops_subtract_modulo(image1: &Bound<'_, PyImage>, image2: &Bound<'_, PyImage>) -> PyResult<PyImage> {
    let b1 = image1.borrow(); let b2 = image2.borrow();
    pillow_rs_core::ops::chops::subtract_modulo(&b1.inner, &b2.inner).map(|i| PyImage { inner: i }).map_err(map_error)
}

#[pyfunction]
fn chops_constant(image: &Bound<'_, PyImage>, value: u8) -> PyResult<PyImage> {
    let b = image.borrow();
    pillow_rs_core::ops::chops::constant(&b.inner, value).map(|i| PyImage { inner: i }).map_err(map_error)
}

#[pyfunction]
fn chops_hard_light(image1: &Bound<'_, PyImage>, image2: &Bound<'_, PyImage>) -> PyResult<PyImage> {
    let b1 = image1.borrow(); let b2 = image2.borrow();
    pillow_rs_core::ops::chops::hard_light(&b1.inner, &b2.inner).map(|i| PyImage { inner: i }).map_err(map_error)
}

#[pyfunction]
fn chops_soft_light(image1: &Bound<'_, PyImage>, image2: &Bound<'_, PyImage>) -> PyResult<PyImage> {
    let b1 = image1.borrow(); let b2 = image2.borrow();
    pillow_rs_core::ops::chops::soft_light(&b1.inner, &b2.inner).map(|i| PyImage { inner: i }).map_err(map_error)
}

#[pyfunction]
fn chops_overlay(image1: &Bound<'_, PyImage>, image2: &Bound<'_, PyImage>) -> PyResult<PyImage> {
    let b1 = image1.borrow(); let b2 = image2.borrow();
    pillow_rs_core::ops::chops::overlay(&b1.inner, &b2.inner).map(|i| PyImage { inner: i }).map_err(map_error)
}

#[pyfunction]
fn chops_logical_and(image1: &Bound<'_, PyImage>, image2: &Bound<'_, PyImage>) -> PyResult<PyImage> {
    let b1 = image1.borrow(); let b2 = image2.borrow();
    pillow_rs_core::ops::chops::logical_and(&b1.inner, &b2.inner).map(|i| PyImage { inner: i }).map_err(map_error)
}

#[pyfunction]
fn chops_logical_or(image1: &Bound<'_, PyImage>, image2: &Bound<'_, PyImage>) -> PyResult<PyImage> {
    let b1 = image1.borrow(); let b2 = image2.borrow();
    pillow_rs_core::ops::chops::logical_or(&b1.inner, &b2.inner).map(|i| PyImage { inner: i }).map_err(map_error)
}

#[pyfunction]
fn chops_logical_xor(image1: &Bound<'_, PyImage>, image2: &Bound<'_, PyImage>) -> PyResult<PyImage> {
    let b1 = image1.borrow(); let b2 = image2.borrow();
    pillow_rs_core::ops::chops::logical_xor(&b1.inner, &b2.inner).map(|i| PyImage { inner: i }).map_err(map_error)
}

#[pyfunction]
fn chops_offset(image: &Bound<'_, PyImage>, xoffset: i32, yoffset: i32) -> PyResult<PyImage> {
    let b = image.borrow();
    pillow_rs_core::ops::chops::offset(&b.inner, xoffset, yoffset).map(|i| PyImage { inner: i }).map_err(map_error)
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
        .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn image_blend(image1: &Bound<'_, PyImage>, image2: &Bound<'_, PyImage>, alpha: f64) -> PyResult<PyImage> {
    let b1 = image1.borrow();
    let b2 = image2.borrow();
    let rs = pillow_rs_core::ops::module_fns::blend(&b1.inner, &b2.inner, alpha)
        .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn image_composite(image1: &Bound<'_, PyImage>, image2: &Bound<'_, PyImage>, mask: &Bound<'_, PyImage>) -> PyResult<PyImage> {
    let b1 = image1.borrow();
    let b2 = image2.borrow();
    let bm = mask.borrow();
    let rs = pillow_rs_core::ops::module_fns::composite(&b1.inner, &b2.inner, &bm.inner)
        .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

// --- ImageColor ---

#[pyfunction]
fn getrgb(color: &str) -> PyResult<(u8, u8, u8)> {
    pillow_rs_core::color::parse_color_str(color)
        .map(|(r, g, b, _a)| (r, g, b))
        .map_err(map_error)
}

#[pyfunction]
fn palette_search(palette: Vec<u8>, r: u8, g: u8, b: u8) -> Option<usize> {
    pillow_rs_core::color::palette_getcolor(&palette, r, g, b)
}

#[pyfunction]
fn getcolor(color: &str, mode: &str) -> PyResult<PyObject> {
    let (r, g, b) = pillow_rs_core::color::parse_color_str(color)
        .map(|(r, g, b, _a)| (r, g, b))
        .map_err(map_error)?;
    let result = pillow_rs_core::color::getcolor(r, g, b, mode).map_err(map_error)?;
    Python::with_gil(|py| {
        match mode {
            "L" | "1" => Ok(result.0.to_object(py)),
            "LA" => Ok((result.0, result.3).to_object(py)),
            "RGB" => Ok((result.0, result.1, result.2).to_object(py)),
            "RGBA" => Ok((result.0, result.1, result.2, result.3).to_object(py)),
            _ => Ok((result.0, result.1, result.2).to_object(py)),
        }
    })
}
