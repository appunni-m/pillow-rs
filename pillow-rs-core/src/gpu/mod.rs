//! GPU compute dispatcher for image operations.
//!
//! Uses wgpu for native (Vulkan/Metal/DX12/OpenGL) and shares WGSL shaders
//! with WebGPU for WASM targets. Operations are dispatched per-pixel-parallel
//! function (blur, color ops, resampling, blending, convolution).
//!
//! GPU is explicitly enabled via a boolean flag — never auto-detected.
//! Callers control whether GPU path is used.

use crate::error::PilError;
use crate::Image;

/// GPU compute engine. Created only when explicitly enabled.
pub struct GpuEngine {
    #[allow(dead_code)]
    device: wgpu::Device,
    #[allow(dead_code)]
    queue: wgpu::Queue,
}

impl GpuEngine {
    /// Initialize GPU compute. Returns None if GPU is unavailable.
    /// Only called when the caller explicitly passes `gpu_enabled: true`.
    pub async fn new() -> Option<Self> {
        let instance = wgpu::Instance::default();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default(), None)
            .await
            .ok()?;
        Some(Self { device, queue })
    }

    /// Synchronous wrapper for benchmark harness use (pollster).
    pub fn new_sync() -> Option<Self> {
        pollster::block_on(Self::new())
    }

    /// Whether this engine is available (was explicitly and successfully created).
    pub fn is_available(&self) -> bool {
        true
    }

    /// Execute a box blur via GPU compute.
    ///
    /// Dispatches the BLUR_SHADER compute shader. This is a NEW code path —
    /// the CPU blur implementation in `src/ops/filter.rs` is never modified.
    /// Callers select this path when `gpu_enabled` is true.
    pub fn blur(&self, img: &Image, radius: u32) -> Result<Image, PilError> {
        let mut img_mut = img.clone();
        let (w, h) = img_mut.size()?;
        let _rgba = img_mut.ensure_loaded()?.to_rgba8();
        // TODO: actual wgpu pipeline dispatch
        //  1. Create compute pipeline from BLUR_SHADER
        //  2. Upload RGBA pixels as a wgpu texture
        //  3. Bind radius as push constant / uniform
        //  4. Dispatch workgroups covering (w, h)
        //  5. Read back result buffer
        //  6. Construct Image from result pixels
        //
        // For now: placeholder returning CPU-fallback error.
        let _ = (w, h, radius);
        Err(PilError::ValueError(
            "GPU blur not yet wired — use CPU path".into(),
        ))
    }

    /// Apply a sharpen filter via GPU compute.
    ///
    /// Dispatches the CONVOLVE_SHADER with a standard sharpen kernel.
    /// NEW code path — CPU sharpen in `src/ops/filter.rs` is unmodified.
    pub fn sharpen(&self, img: &Image) -> Result<Image, PilError> {
        let mut img_mut = img.clone();
        let _rgba = img_mut.ensure_loaded()?.to_rgba8();
        // Default sharpen kernel:  [ 0, -1,  0]
        //                          [-1,  5, -1]
        //                          [ 0, -1,  0]
        Err(PilError::ValueError(
            "GPU sharpen not yet wired — use CPU path".into(),
        ))
    }

    /// Invert colors via GPU compute.
    ///
    /// Dispatches the COLOR_OPS_SHADER with `op_code = 0` (invert).
    /// NEW code path — CPU invert in `src/ops/color.py` equivalent is unmodified.
    pub fn invert(&self, img: &Image) -> Result<Image, PilError> {
        let mut img_mut = img.clone();
        let _rgba = img_mut.ensure_loaded()?.to_rgba8();
        // Each pixel: out = 255 - in (per channel)
        Err(PilError::ValueError(
            "GPU invert not yet wired — use CPU path".into(),
        ))
    }

    /// Convert to grayscale via GPU compute.
    ///
    /// Dispatches the COLOR_OPS_SHADER with `op_code = 1` (grayscale).
    /// NEW code path — CPU grayscale in `src/ops/color.py` equivalent is unmodified.
    pub fn grayscale(&self, img: &Image) -> Result<Image, PilError> {
        let mut img_mut = img.clone();
        let (w, h) = img_mut.size()?;
        let _rgba = img_mut.ensure_loaded()?.to_rgba8();
        // Standard luminance weights: out = 0.299*R + 0.587*G + 0.114*B
        // Result is L (single-channel) image
        let _ = (w, h);
        Err(PilError::ValueError(
            "GPU grayscale not yet wired — use CPU path".into(),
        ))
    }

    /// Resize via GPU bilinear sampling.
    ///
    /// Dispatches the RESAMPLE_SHADER compute shader.
    /// NEW code path — CPU resize in `src/ops/transform.rs` equivalent is unmodified.
    pub fn resize(
        &self,
        img: &Image,
        dst_w: u32,
        dst_h: u32,
    ) -> Result<Image, PilError> {
        let mut img_mut = img.clone();
        let _rgba = img_mut.ensure_loaded()?.to_rgba8();
        let _ = (dst_w, dst_h);
        Err(PilError::ValueError(
            "GPU resize not yet wired — use CPU path".into(),
        ))
    }

    /// Apply a 3x3 convolution kernel via GPU.
    ///
    /// Dispatches the CONVOLVE_SHADER compute shader with the given kernel,
    /// scale, and offset. NEW code path — CPU convolve in
    /// `src/ops/filter.rs` equivalent is unmodified.
    pub fn convolve(
        &self,
        img: &Image,
        kernel: &[f32; 9],
        scale: f32,
        offset: f32,
    ) -> Result<Image, PilError> {
        let mut img_mut = img.clone();
        let _rgba = img_mut.ensure_loaded()?.to_rgba8();
        let _ = (kernel, scale, offset);
        Err(PilError::ValueError(
            "GPU convolve not yet wired — use CPU path".into(),
        ))
    }

    /// Blend two images via GPU compute.
    ///
    /// Dispatches the BLEND_SHADER compute shader. `op_code` selects the
    /// blend mode (e.g., 0 = normal, 1 = multiply, 2 = screen, etc.).
    /// NEW code path — CPU blend equivalent is unmodified.
    pub fn blend(
        &self,
        img_a: &Image,
        img_b: &Image,
        op_code: u32,
    ) -> Result<Image, PilError> {
        let mut img_a_mut = img_a.clone();
        let mut img_b_mut = img_b.clone();
        let _rgba_a = img_a_mut.ensure_loaded()?.to_rgba8();
        let _rgba_b = img_b_mut.ensure_loaded()?.to_rgba8();
        let _ = op_code;
        Err(PilError::ValueError(
            "GPU blend not yet wired — use CPU path".into(),
        ))
    }
}

// ─── Shader loading ───
// Shaders are embedded at compile time via include_str!.
// Shared between native wgpu and browser WebGPU targets.

pub const BLUR_SHADER: &str = include_str!("shaders/blur.wgsl");
pub const COLOR_OPS_SHADER: &str = include_str!("shaders/color_ops.wgsl");
pub const RESAMPLE_SHADER: &str = include_str!("shaders/resample.wgsl");
pub const BLEND_SHADER: &str = include_str!("shaders/blend.wgsl");
pub const CONVOLVE_SHADER: &str = include_str!("shaders/convolve.wgsl");
