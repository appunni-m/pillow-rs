use wasm_bindgen::prelude::{JsValue, wasm_bindgen};

#[wasm_bindgen(js_name = "resolveNewColor")]
pub fn resolve_new_color(
    mode: &str,
    hex: Option<String>,
    single: Option<u8>,
    rgb: Option<Vec<u8>>,
    rgba: Option<Vec<u8>>,
    la: Option<Vec<u8>>,
) -> Result<JsValue, JsValue> {
    let hex = hex.as_deref();
    let rgb = rgb.map(|v| {
        if v.len() == 3 {
            (v[0], v[1], v[2])
        } else {
            (0, 0, 0)
        }
    });
    let rgba = rgba.map(|v| {
        if v.len() == 4 {
            (v[0], v[1], v[2], v[3])
        } else {
            (0, 0, 0, 0)
        }
    });
    let la = la.map(|v| if v.len() == 2 { (v[0], v[1]) } else { (0, 0) });
    let c = pillow_rs::resolve_new_color(mode, hex, single, rgb, rgba, la, None, None)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
    let arr = js_sys::Array::new();
    arr.push(&JsValue::from(c.0));
    arr.push(&JsValue::from(c.1));
    arr.push(&JsValue::from(c.2));
    arr.push(&JsValue::from(c.3));
    Ok(arr.into())
}
