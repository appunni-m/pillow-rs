//! GPU device ownership, adapter selection, and lazy-engine construction.

use super::{BufferPool, CachedPipeline, PilError, StagingBuffer};
use crate::compute::GpuAdapterInfo;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub(super) struct GpuDeviceState {
    pub(super) lost: Option<String>,
    pub(super) uncaptured_error: Option<String>,
}

/// Internal GPU engine. Initialized once and stored in a static OnceLock.
pub(super) struct GpuInner {
    pub(super) device: wgpu::Device,
    pub(super) queue: wgpu::Queue,
    pub(super) adapter_info: GpuAdapterInfo,
    /// Pipelines are compiled on first use. Values are shared so a resolved
    /// batch can keep every pipeline alive for the full compute-pass lifetime
    /// without holding the cache lock while commands are encoded.
    pub(super) pipelines: Mutex<HashMap<&'static str, Arc<CachedPipeline>>>,
    pub(super) device_state: Arc<Mutex<GpuDeviceState>>,
    pub(super) available_buffers: Mutex<Vec<BufferPool>>,
    pub(super) available_staging: Mutex<Vec<StagingBuffer>>,
    pub(super) direct_primary_readback: bool,
}

#[cfg(not(target_arch = "wasm32"))]
fn requested_adapter_index() -> Result<Option<usize>, PilError> {
    let Ok(value) = std::env::var("MIGRATION_TARGET_GPU_ADAPTER_INDEX") else {
        return Ok(None);
    };
    value.parse::<usize>().map(Some).map_err(|_| {
        PilError::ValueError(format!(
            "MIGRATION_TARGET_GPU_ADAPTER_INDEX must be a non-negative integer, got {value:?}"
        ))
    })
}

fn adapter_identity(selection_index: Option<usize>, info: wgpu::AdapterInfo) -> GpuAdapterInfo {
    GpuAdapterInfo {
        selection_index,
        name: info.name,
        vendor: info.vendor,
        device: info.device,
        device_type: format!("{:?}", info.device_type).to_lowercase(),
        backend: format!("{:?}", info.backend).to_lowercase(),
        driver: info.driver,
        driver_info: info.driver_info,
    }
}

pub(super) fn adapter_inventory() -> Vec<GpuAdapterInfo> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let descriptor = wgpu::InstanceDescriptor::from_env_or_default();
        let instance = wgpu::Instance::new(&descriptor);
        instance
            .enumerate_adapters(descriptor.backends)
            .into_iter()
            .enumerate()
            .map(|(index, adapter)| adapter_identity(Some(index), adapter.get_info()))
            .collect()
    }
    #[cfg(target_arch = "wasm32")]
    {
        Vec::new()
    }
}

#[cfg(target_arch = "wasm32")]
fn requested_adapter_index() -> Result<Option<usize>, PilError> {
    Ok(None)
}

pub(super) fn direct_primary_readback_supported(
    backend: wgpu::Backend,
    device_type: wgpu::DeviceType,
    features: wgpu::Features,
) -> bool {
    // Limit the first transport implementation to a native backend whose
    // integrated adapter classification denotes shared CPU/GPU memory.
    cfg!(not(target_arch = "wasm32"))
        && backend == wgpu::Backend::Metal
        && device_type == wgpu::DeviceType::IntegratedGpu
        && features.contains(wgpu::Features::MAPPABLE_PRIMARY_BUFFERS)
}

#[cfg(not(target_arch = "wasm32"))]
fn format_adapter_inventory(adapters: &[wgpu::Adapter]) -> String {
    let infos = adapters
        .iter()
        .map(wgpu::Adapter::get_info)
        .collect::<Vec<_>>();
    format!("enumerated={} adapters={infos:?}", adapters.len())
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
fn select_adapter(
    instance: &wgpu::Instance,
    instance_descriptor: &wgpu::InstanceDescriptor,
    requested_adapter_index: Option<usize>,
) -> Result<wgpu::Adapter, PilError> {
    let request_options = wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface: None,
    };

    #[cfg(not(target_arch = "wasm32"))]
    let enumerated_adapters = instance.enumerate_adapters(instance_descriptor.backends);
    #[cfg(not(target_arch = "wasm32"))]
    let adapter_inventory = format_adapter_inventory(&enumerated_adapters);

    // Prefer wgpu's normal selection because it applies power preference and
    // compatibility rules. Some embedded/native-library contexts have been
    // observed to return None even though direct enumeration succeeds.
    let adapter = match requested_adapter_index {
        Some(index) => {
            #[cfg(not(target_arch = "wasm32"))]
            {
                enumerated_adapters.into_iter().nth(index)
            }
            #[cfg(target_arch = "wasm32")]
            {
                let _ = index;
                None
            }
        }
        None => pollster::block_on(instance.request_adapter(&request_options)).or_else(|| {
            #[cfg(not(target_arch = "wasm32"))]
            {
                enumerated_adapters.into_iter().next()
            }
            #[cfg(target_arch = "wasm32")]
            {
                None
            }
        }),
    };
    adapter.ok_or_else(|| {
        #[cfg(not(target_arch = "wasm32"))]
        {
            PilError::ValueError(format!(
                "GPU adapter not available: requested={:?}, enabled={:?}, {}",
                instance_descriptor.backends,
                wgpu::Instance::enabled_backend_features(),
                adapter_inventory,
            ))
        }
        #[cfg(target_arch = "wasm32")]
        {
            PilError::ValueError(format!(
                "GPU adapter not available: requested={:?}",
                instance_descriptor.backends,
            ))
        }
    })
}

impl GpuInner {
    // VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
    pub(super) fn new() -> Result<Self, PilError> {
        // `Instance::default()` does not apply WGPU_BACKEND. Use the explicit
        // descriptor so backend selection is deterministic and debuggable in
        // the Python extension as well as in standalone Rust binaries.
        let instance_descriptor = wgpu::InstanceDescriptor::from_env_or_default();
        let instance = wgpu::Instance::new(&instance_descriptor);
        let requested_adapter_index = requested_adapter_index()?;
        let adapter = select_adapter(&instance, &instance_descriptor, requested_adapter_index)?;
        let selected_info = adapter.get_info();
        gpu_log!("[GPU] adapter selected: {:?}", selected_info);
        let direct_primary_readback = direct_primary_readback_supported(
            selected_info.backend,
            selected_info.device_type,
            adapter.features(),
        );
        let adapter_info = adapter_identity(requested_adapter_index, selected_info);
        let required_features = if direct_primary_readback {
            wgpu::Features::MAPPABLE_PRIMARY_BUFFERS
        } else {
            wgpu::Features::empty()
        };
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("pillow-rs-gpu"),
                required_features,
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::Performance,
            },
            None,
        ))
        .map_err(|error| {
            PilError::ValueError(format!("GPU device initialization failed: {error}"))
        })?;

        let device_state = Arc::new(Mutex::new(GpuDeviceState::default()));
        let lost_state = Arc::clone(&device_state);
        device.set_device_lost_callback(move |reason, message| {
            let detail = format!("{reason:?}: {message}");
            if let Ok(mut state) = lost_state.lock() {
                state.lost = Some(detail.clone());
            }
            log::error!(target: "compute::gpu", "[GPU] device lost: {detail}");
        });
        let error_state = Arc::clone(&device_state);
        device.on_uncaptured_error(Box::new(move |error| {
            let detail = error.to_string();
            if let Ok(mut state) = error_state.lock() {
                state.uncaptured_error = Some(detail.clone());
            }
            log::error!(target: "compute::gpu", "[GPU] uncaptured device error: {detail}");
        }));

        Ok(GpuInner {
            device,
            queue,
            adapter_info,
            pipelines: Mutex::new(HashMap::new()),
            device_state,
            available_buffers: Mutex::new(Vec::new()),
            available_staging: Mutex::new(Vec::new()),
            direct_primary_readback,
        })
    }
}
