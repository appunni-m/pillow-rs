use crate::values::*;
use wasm_bindgen::prelude::{JsValue, wasm_bindgen};

#[wasm_bindgen]
/// Activates a compiled backend for subsequent automatic image routing.
pub fn enable_backend(name: &str) -> Result<bool, JsValue> {
    let backend = pillow_rs::Backend::parse(name).ok_or_else(|| {
        err(pillow_rs::PilError::ValueError(format!(
            "unknown backend: {name}"
        )))
    })?;
    pillow_rs::enable_backend(backend).map_err(err)
}

/// Deactivate a compute backend. Returns true if it was active.
#[wasm_bindgen]
pub fn disable_backend(name: &str) -> Result<bool, JsValue> {
    let backend = pillow_rs::Backend::parse(name).ok_or_else(|| {
        err(pillow_rs::PilError::ValueError(format!(
            "unknown backend: {name}"
        )))
    })?;
    pillow_rs::disable_backend(backend).map_err(err)
}

/// List backend implementations compiled into this module.
#[wasm_bindgen]
pub fn available_backends() -> Vec<String> {
    pillow_rs::available_backends()
        .iter()
        .map(|b| format!("{:?}", b).to_lowercase())
        .collect()
}

/// List currently active backends (priority order).
#[wasm_bindgen]
pub fn active_backends() -> Result<Vec<String>, JsValue> {
    Ok(pillow_rs::active_backends()
        .map_err(err)?
        .into_iter()
        .map(|b| format!("{:?}", b).to_lowercase())
        .collect())
}

/// Check if a specific backend is active.
#[wasm_bindgen]
pub fn backend_enabled(name: &str) -> Result<bool, JsValue> {
    let backend = pillow_rs::Backend::parse(name).ok_or_else(|| {
        err(pillow_rs::PilError::ValueError(format!(
            "unknown backend: {name}"
        )))
    })?;
    pillow_rs::backend_enabled(backend).map_err(err)
}

#[wasm_bindgen(js_name = "setLogLevel")]
/// Set the maximum log level shown in the browser console.
/// Levels (ascending): 0=off, 1=error, 2=warn, 3=info, 4=debug, 5=trace.
pub fn set_log_level(level: u8) {
    #[cfg(feature = "debug-hooks")]
    {
        let lvl = match level {
            0 => log::LevelFilter::Off,
            1 => log::LevelFilter::Error,
            2 => log::LevelFilter::Warn,
            3 => log::LevelFilter::Info,
            4 => log::LevelFilter::Debug,
            5 => log::LevelFilter::Trace,
            _ => log::LevelFilter::Warn,
        };
        log::set_max_level(lvl);
    }

    #[cfg(not(feature = "debug-hooks"))]
    let _ = level;
}

// ══════════════════════════════════════════════════════════════════════════════
// ImageChops — per-pixel channel operations (thin wrappers)
// ══════════════════════════════════════════════════════════════════════════════
