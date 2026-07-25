//! Emit the compute registry's declared backend support as deterministic JSON.
#![allow(unused_crate_dependencies)]

use pillow_rs::compute::registry;
use serde_json::json;

fn main() {
    let registry = registry::registry();
    let mut names: Vec<_> = registry.keys().copied().collect();
    names.sort_unstable();

    let operations: Vec<_> = names
        .iter()
        .map(|name| {
            let entry = &registry[name];
            json!({
                "operation": name,
                "cpu": entry.cpu_fn.is_some(),
                "simd_pool": entry.simd_fn.is_some(),
                "gpu_shader": entry.gpu_shader.is_some(),
            })
        })
        .collect();
    let cpu = registry
        .values()
        .filter(|entry| entry.cpu_fn.is_some())
        .count();
    let simd_pool = registry
        .values()
        .filter(|entry| entry.simd_fn.is_some())
        .count();
    let gpu_shader = registry
        .values()
        .filter(|entry| entry.gpu_shader.is_some())
        .count();
    let cpu_without_simd: Vec<_> = names
        .iter()
        .filter(|name| {
            let entry = &registry[*name];
            entry.cpu_fn.is_some() && entry.simd_fn.is_none()
        })
        .copied()
        .collect();

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "totals": {
                "operations": registry.len(),
                "cpu": cpu,
                "simd_pool": simd_pool,
                "gpu_shader": gpu_shader,
                "cpu_without_simd": cpu_without_simd.len(),
            },
            "cpu_without_simd": cpu_without_simd,
            "operations": operations,
        }))
        .expect("backend support matrix serializes")
    );
}
