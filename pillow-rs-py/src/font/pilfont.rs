use super::{PilError, pilfont_text_input_from_python, pyclass, pymethods};
use crate::{Bound, Py, PyAny, PyBytes, PyImage, PyResult, PyString, Python, map_error};
use pyo3::types::{PyAnyMethods, PyStringMethods};
use std::sync::{Mutex, OnceLock};
#[pyclass(name = "PilFont", unsendable)]
pub(super) struct PyPilFont {
    inner: pillow_rs::PilFont,
    file: Option<String>,
}

#[pymethods]
impl PyPilFont {
    #[staticmethod]
    fn load(filename: &str) -> PyResult<Self> {
        let (inner, file) =
            load_pilfont_from_path(std::path::Path::new(filename)).map_err(map_error)?;
        Ok(Self {
            inner,
            file: Some(file),
        })
    }

    #[staticmethod]
    fn load_path(py: Python<'_>, filename: &str) -> PyResult<Self> {
        let directories = py
            .import("sys")?
            .getattr("path")?
            .extract::<Vec<String>>()?;
        for directory in directories {
            let path = std::path::Path::new(&directory).join(filename);
            match load_pilfont_from_path(&path) {
                Ok((inner, file)) => {
                    return Ok(Self {
                        inner,
                        file: Some(file),
                    });
                }
                Err(error) if pilfont_load_path_catches(&error) => {}
                Err(error) => return Err(map_error(error)),
            }
        }

        let mut message = format!("cannot find font file \"{filename}\" in sys.path");
        if std::path::Path::new(filename).exists() {
            message.push_str(&format!(
                ", did you mean ImageFont.load(\"{filename}\") instead?"
            ));
        }
        Err(pyo3::exceptions::PyOSError::new_err(message))
    }

    #[staticmethod]
    fn load_default() -> PyResult<Self> {
        let inner = pillow_rs::PilFont::load_default().map_err(map_error)?;
        Ok(Self { inner, file: None })
    }

    #[getter]
    fn file(&self) -> Option<&str> {
        self.file.as_deref()
    }

    #[getter]
    fn info(&self, py: Python<'_>) -> Vec<Py<PyBytes>> {
        self.inner
            .info()
            .iter()
            .map(|line| PyBytes::new(py, line).unbind())
            .collect()
    }

    fn getsize(&self, text: &Bound<'_, PyAny>) -> PyResult<(i32, i32)> {
        self.inner
            .getsize_input(pilfont_text_input_from_python(text)?)
            .map_err(map_error)
    }

    fn getbbox(&self, text: &Bound<'_, PyAny>) -> PyResult<(i32, i32, i32, i32)> {
        self.inner
            .getbbox_input(pilfont_text_input_from_python(text)?)
            .map_err(map_error)
    }

    fn getlength(&self, text: &Bound<'_, PyAny>) -> PyResult<i32> {
        if let Ok(text) = text.cast::<PyString>() {
            return self.inner.getlength_str(&text.to_cow()?).map_err(map_error);
        }
        self.inner
            .getlength_input(pilfont_text_input_from_python(text)?)
            .map_err(map_error)
    }

    #[pyo3(signature = (text, mode=""))]
    fn getmask(&self, text: &Bound<'_, PyAny>, mode: &str) -> PyResult<PyImage> {
        let _ = mode;
        let image = self
            .inner
            .getmask_image_input(pilfont_text_input_from_python(text)?)
            .map_err(map_error)?;
        Ok(PyImage { inner: image })
    }
}

fn load_pilfont_from_path(
    path: &std::path::Path,
) -> Result<(pillow_rs::PilFont, String), PilError> {
    let metrics = std::fs::read(path).map_err(PilError::from)?;
    let root = path.with_extension("");

    for extension in [".png", ".gif", ".pbm"] {
        let mut candidate_name = root.as_os_str().to_os_string();
        candidate_name.push(extension);
        let candidate = std::path::PathBuf::from(candidate_name);
        let Ok(bitmap) = std::fs::read(&candidate) else {
            continue;
        };
        if let Some(font) = cached_pilfont_load(&metrics, &bitmap) {
            return Ok((font, candidate.to_string_lossy().into_owned()));
        }
        if let Some(font) = pillow_rs::PilFont::load_default_if_sources_match(&metrics, &bitmap) {
            remember_pilfont_load(&metrics, &bitmap, &font);
            return Ok((font, candidate.to_string_lossy().into_owned()));
        }
        let cache_bitmap =
            pilfont_load_sources_within_cache_limit(&metrics, &bitmap).then(|| bitmap.clone());
        let Ok(image) = pillow_rs::PilFont::open_pilfont_glyph_image(bitmap) else {
            continue;
        };
        let cache_raster = pilfont_glyph_image_within_cache_limit(&image);
        match pilfont_from_glyph_image(&metrics, image) {
            Ok(font) => {
                if cache_raster && let Some(bitmap) = cache_bitmap {
                    remember_pilfont_load(&metrics, &bitmap, &font);
                }
                return Ok((font, candidate.to_string_lossy().into_owned()));
            }
            Err(PilError::TypeError(message)) if message == "invalid font image mode" => continue,
            Err(error) => return Err(error),
        }
    }

    Err(PilError::OsError(format!(
        "cannot find glyph data file {}.{{gif|pbm|png}}",
        root.display()
    )))
}

const PILFONT_LOAD_CACHE_MAX_SOURCE_BYTES: usize = 128 * 1024;
const PILFONT_LOAD_CACHE_MAX_RASTER_PIXELS: u64 = 64 * 1024;

struct PilFontLoadCacheEntry {
    metrics: Vec<u8>,
    bitmap: Vec<u8>,
    font: pillow_rs::PilFont,
}

#[derive(Default)]
pub(crate) struct PilFontLoadCache {
    entry: Option<PilFontLoadCacheEntry>,
}

impl PilFontLoadCache {
    pub(crate) fn get(&self, metrics: &[u8], bitmap: &[u8]) -> Option<pillow_rs::PilFont> {
        let entry = self.entry.as_ref()?;
        (entry.metrics == metrics && entry.bitmap == bitmap).then(|| entry.font.clone())
    }

    pub(crate) fn insert(&mut self, metrics: &[u8], bitmap: &[u8], font: &pillow_rs::PilFont) {
        if !pilfont_load_sources_within_cache_limit(metrics, bitmap) {
            return;
        }
        self.entry = Some(PilFontLoadCacheEntry {
            metrics: metrics.to_vec(),
            bitmap: bitmap.to_vec(),
            font: font.clone(),
        });
    }
}

static PILFONT_LOAD_CACHE: OnceLock<Mutex<PilFontLoadCache>> = OnceLock::new();

fn pilfont_load_sources_within_cache_limit(metrics: &[u8], bitmap: &[u8]) -> bool {
    metrics.len().saturating_add(bitmap.len()) <= PILFONT_LOAD_CACHE_MAX_SOURCE_BYTES
}

fn pilfont_glyph_image_within_cache_limit(image: &pillow_rs::PilFontGlyphImage) -> bool {
    let (width, height) = match image {
        pillow_rs::PilFontGlyphImage::Image(image) => match image.size() {
            Ok(dimensions) => dimensions,
            Err(_) => return false,
        },
        pillow_rs::PilFontGlyphImage::DeferredRenderError { width, height, .. } => {
            (*width, *height)
        }
    };
    u64::from(width) * u64::from(height) <= PILFONT_LOAD_CACHE_MAX_RASTER_PIXELS
}

fn cached_pilfont_load(metrics: &[u8], bitmap: &[u8]) -> Option<pillow_rs::PilFont> {
    if !pilfont_load_sources_within_cache_limit(metrics, bitmap) {
        return None;
    }
    let cache = PILFONT_LOAD_CACHE.get()?;
    let cache = match cache.lock() {
        Ok(cache) => cache,
        Err(poisoned) => poisoned.into_inner(),
    };
    cache.get(metrics, bitmap)
}

fn remember_pilfont_load(metrics: &[u8], bitmap: &[u8], font: &pillow_rs::PilFont) {
    let cache = PILFONT_LOAD_CACHE.get_or_init(|| Mutex::new(PilFontLoadCache::default()));
    let mut cache = match cache.lock() {
        Ok(cache) => cache,
        Err(poisoned) => poisoned.into_inner(),
    };
    cache.insert(metrics, bitmap, font);
}

fn pilfont_from_glyph_image(
    metrics: &[u8],
    image: pillow_rs::PilFontGlyphImage,
) -> Result<pillow_rs::PilFont, PilError> {
    match image {
        pillow_rs::PilFontGlyphImage::Image(image) => {
            pillow_rs::PilFont::from_pilfont_data(metrics, *image)
        }
        deferred @ pillow_rs::PilFontGlyphImage::DeferredRenderError { .. } => {
            pillow_rs::PilFont::from_pilfont_glyph_data(metrics, deferred)
        }
    }
}

fn pilfont_load_path_catches(error: &PilError) -> bool {
    matches!(
        error,
        PilError::IOError(_)
            | PilError::OsError(_)
            | PilError::UnidentifiedImageError(_)
            | PilError::ImageError(_)
            | PilError::Io(_)
    )
}
