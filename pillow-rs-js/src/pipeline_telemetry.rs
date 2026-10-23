use wasm_bindgen::prelude::{JsValue, wasm_bindgen};

fn backend_name(backend: pillow_rs::Backend) -> String {
    format!("{backend:?}").to_lowercase()
}

fn set_pipeline_telemetry_field(object: &js_sys::Object, name: &str, value: &JsValue) {
    let _ = js_sys::Reflect::set(object, &JsValue::from_str(name), value);
}

fn pipeline_resource_telemetry_to_js(resource: pillow_rs::PipelineResourceTelemetry) -> JsValue {
    let object = js_sys::Object::new();
    set_pipeline_telemetry_field(
        &object,
        "upload_bytes",
        &JsValue::from_f64(resource.upload_bytes as f64),
    );
    set_pipeline_telemetry_field(
        &object,
        "readback_bytes",
        &JsValue::from_f64(resource.readback_bytes as f64),
    );
    set_pipeline_telemetry_field(
        &object,
        "auxiliary_bytes",
        &JsValue::from_f64(resource.auxiliary_bytes as f64),
    );
    set_pipeline_telemetry_field(
        &object,
        "parameter_bytes",
        &JsValue::from_f64(resource.parameter_bytes as f64),
    );
    set_pipeline_telemetry_field(
        &object,
        "retained_cache_bytes",
        &JsValue::from_f64(resource.retained_cache_bytes as f64),
    );
    set_pipeline_telemetry_field(
        &object,
        "full_frame_copy_count",
        &JsValue::from_f64(resource.full_frame_copy_count as f64),
    );
    set_pipeline_telemetry_field(
        &object,
        "mode_conversion_count",
        &JsValue::from_f64(resource.mode_conversion_count as f64),
    );
    set_pipeline_telemetry_field(
        &object,
        "host_buffer_count",
        &JsValue::from_f64(resource.host_buffer_count as f64),
    );
    set_pipeline_telemetry_field(
        &object,
        "host_buffer_bytes",
        &JsValue::from_f64(resource.host_buffer_bytes as f64),
    );
    set_pipeline_telemetry_field(
        &object,
        "peak_live_host_bytes",
        &JsValue::from_f64(resource.peak_live_host_bytes as f64),
    );
    set_pipeline_telemetry_field(
        &object,
        "fused_operation_count",
        &JsValue::from_f64(resource.fused_operation_count as f64),
    );
    set_pipeline_telemetry_field(
        &object,
        "host_allocation_count",
        &JsValue::from_f64(resource.host_allocation_count as f64),
    );
    set_pipeline_telemetry_field(
        &object,
        "host_allocated_bytes",
        &JsValue::from_f64(resource.host_allocated_bytes as f64),
    );
    object.into()
}

fn pipeline_operation_telemetry_to_js(
    samples: Vec<pillow_rs::PipelineOperationTelemetry>,
) -> JsValue {
    let values = js_sys::Array::new();
    for sample in samples {
        let value = js_sys::Object::new();
        set_pipeline_telemetry_field(&value, "operation", &JsValue::from_str(sample.operation));
        set_pipeline_telemetry_field(&value, "path", &JsValue::from_str(sample.path));
        set_pipeline_telemetry_field(
            &value,
            "vector_block_count",
            &JsValue::from_f64(sample.vector_block_count as f64),
        );
        set_pipeline_telemetry_field(
            &value,
            "scalar_tail_count",
            &JsValue::from_f64(sample.scalar_tail_count as f64),
        );
        set_pipeline_telemetry_field(
            &value,
            "mode_conversion_count",
            &JsValue::from_f64(sample.mode_conversion_count as f64),
        );
        set_pipeline_telemetry_field(
            &value,
            "handoff_count",
            &JsValue::from_f64(sample.handoff_count as f64),
        );
        values.push(&value);
    }
    values.into()
}

/// Enable bounded WASM image-pipeline execution telemetry for parity evidence.
#[wasm_bindgen(js_name = "setPipelineTelemetry")]
pub fn set_pipeline_telemetry(enabled: bool) -> bool {
    pillow_rs::Backend::set_pipeline_telemetry_enabled(enabled)
}

/// Take the most recent completed WASM image-pipeline receipt, or `null`.
#[wasm_bindgen(js_name = "takePipelineTelemetry")]
pub fn take_pipeline_telemetry() -> JsValue {
    let operation_telemetry = pillow_rs::Backend::take_pipeline_operation_telemetry();
    let Some((
        requested_backend,
        actual_backend,
        operation_count,
        route_ns,
        validation_ns,
        backend_ns,
        dispatch_count,
        fallback_reason,
        resource,
        resize_coeff_cache_hits,
        resize_coeff_cache_misses,
    )) = pillow_rs::Backend::take_pipeline_telemetry()
    else {
        if operation_telemetry.is_empty() {
            return JsValue::NULL;
        }
        let object = js_sys::Object::new();
        let operation_telemetry = pipeline_operation_telemetry_to_js(operation_telemetry);
        set_pipeline_telemetry_field(&object, "operation_telemetry", &operation_telemetry);
        return object.into();
    };

    let object = js_sys::Object::new();
    let requested = requested_backend
        .map(backend_name)
        .map_or(JsValue::NULL, |value| JsValue::from_str(&value));
    set_pipeline_telemetry_field(&object, "requested_backend", &requested);
    set_pipeline_telemetry_field(
        &object,
        "actual_backend",
        &JsValue::from_str(&backend_name(actual_backend)),
    );
    set_pipeline_telemetry_field(
        &object,
        "operation_count",
        &JsValue::from_f64(operation_count as f64),
    );
    set_pipeline_telemetry_field(&object, "route_ns", &JsValue::from_f64(route_ns as f64));
    set_pipeline_telemetry_field(
        &object,
        "validation_ns",
        &JsValue::from_f64(validation_ns as f64),
    );
    set_pipeline_telemetry_field(&object, "backend_ns", &JsValue::from_f64(backend_ns as f64));
    let dispatch = dispatch_count.map_or(JsValue::NULL, |value| JsValue::from_f64(value as f64));
    set_pipeline_telemetry_field(&object, "dispatch_count", &dispatch);
    let fallback = fallback_reason.map_or(JsValue::NULL, |value| JsValue::from_str(&value));
    set_pipeline_telemetry_field(&object, "fallback_reason", &fallback);
    let resource = resource.map_or(JsValue::NULL, pipeline_resource_telemetry_to_js);
    set_pipeline_telemetry_field(&object, "resource", &resource);
    set_pipeline_telemetry_field(
        &object,
        "resize_coeff_cache_hits",
        &JsValue::from_f64(resize_coeff_cache_hits as f64),
    );
    set_pipeline_telemetry_field(
        &object,
        "resize_coeff_cache_misses",
        &JsValue::from_f64(resize_coeff_cache_misses as f64),
    );
    let operation_telemetry = pipeline_operation_telemetry_to_js(operation_telemetry);
    set_pipeline_telemetry_field(&object, "operation_telemetry", &operation_telemetry);
    object.into()
}
