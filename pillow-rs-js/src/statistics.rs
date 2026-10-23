use crate::Image;
use crate::values::*;
use wasm_bindgen::prelude::{JsValue, wasm_bindgen};

#[wasm_bindgen]
pub struct ImageStat {
    inner: pillow_rs::StatResult,
}
#[wasm_bindgen]
impl ImageStat {
    #[wasm_bindgen(constructor)]
    pub fn new(img: &Image, mask: Option<Image>) -> Result<ImageStat, JsValue> {
        let s = img
            .inner
            .stat_formatted_with_mask(match mask.as_ref() {
                Some(mask) => pillow_rs::ImageOpsMask::Image(Box::new(mask.inner.clone())),
                None => pillow_rs::ImageOpsMask::None,
            })
            .map_err(err)?;
        Ok(ImageStat { inner: s })
    }
    fn val_to_js(&self, v: &pillow_rs::StatValue) -> JsValue {
        use pillow_rs::StatValue;
        match v {
            StatValue::Int(i) => JsValue::from_f64(*i as f64),
            StatValue::Float(f) => JsValue::from_f64(*f),
            StatValue::IntList(l) => {
                let arr = js_sys::Array::new();
                for &x in l {
                    arr.push(&JsValue::from_f64(x as f64));
                }
                arr.into()
            }
            StatValue::FloatList(l) => {
                let arr = js_sys::Array::new();
                for &x in l {
                    arr.push(&JsValue::from_f64(x));
                }
                arr.into()
            }
            StatValue::ExtremaSingle((min, max)) => {
                let arr = js_sys::Array::new();
                arr.push(&JsValue::from_f64(*min as f64));
                arr.push(&JsValue::from_f64(*max as f64));
                arr.into()
            }
            StatValue::ExtremaList(l) => {
                let arr = js_sys::Array::new();
                for &(min, max) in l {
                    let pair = js_sys::Array::new();
                    pair.push(&JsValue::from_f64(min as f64));
                    pair.push(&JsValue::from_f64(max as f64));
                    arr.push(&pair);
                }
                arr.into()
            }
        }
    }
    pub fn toObject(&self) -> js_sys::Object {
        let obj = js_sys::Object::new();
        js_sys::Reflect::set(&obj, &"count".into(), &self.val_to_js(&self.inner.count)).ok();
        js_sys::Reflect::set(&obj, &"sum".into(), &self.val_to_js(&self.inner.sum)).ok();
        js_sys::Reflect::set(&obj, &"sum2".into(), &self.val_to_js(&self.inner.sum2)).ok();
        js_sys::Reflect::set(&obj, &"mean".into(), &self.val_to_js(&self.inner.mean)).ok();
        js_sys::Reflect::set(&obj, &"median".into(), &self.val_to_js(&self.inner.median)).ok();
        js_sys::Reflect::set(&obj, &"rms".into(), &self.val_to_js(&self.inner.rms)).ok();
        js_sys::Reflect::set(&obj, &"var".into(), &self.val_to_js(&self.inner.var)).ok();
        js_sys::Reflect::set(&obj, &"stddev".into(), &self.val_to_js(&self.inner.stddev)).ok();
        js_sys::Reflect::set(
            &obj,
            &"extrema".into(),
            &self.val_to_js(&self.inner.extrema),
        )
        .ok();
        obj
    }
}

// ── ImageSequence ────────────────────────────────────────────────
#[wasm_bindgen(js_name = "statFromList")]
pub fn stat_from_list(data: Vec<f64>) -> Result<JsValue, JsValue> {
    let (count, sum, mean, min, max) = pillow_rs::stat_from_list(&data);
    let obj = js_sys::Object::new();
    js_sys::Reflect::set(&obj, &JsValue::from_str("count"), &JsValue::from_f64(count))?;
    js_sys::Reflect::set(&obj, &JsValue::from_str("sum"), &JsValue::from_f64(sum))?;
    js_sys::Reflect::set(&obj, &JsValue::from_str("mean"), &JsValue::from_f64(mean))?;
    js_sys::Reflect::set(&obj, &JsValue::from_str("min"), &JsValue::from_f64(min))?;
    js_sys::Reflect::set(&obj, &JsValue::from_str("max"), &JsValue::from_f64(max))?;
    Ok(obj.into())
}

#[wasm_bindgen(js_name = "statFromHistogram")]
pub fn stat_from_histogram(data: Vec<f64>) -> ImageStat {
    ImageStat {
        inner: pillow_rs::stat_from_histogram(&data),
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Draw helpers
// ══════════════════════════════════════════════════════════════════════════════
