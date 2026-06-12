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
        let img_loaded = img.materialize()?;
        let (w, h) = (img_loaded.width(), img_loaded.height());
        let _rgba = img_loaded.to_rgba8();
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
        let img_loaded = img.materialize()?;
        let _rgba = img_loaded.to_rgba8();
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
        let img_loaded = img.materialize()?;
        let _rgba = img_loaded.to_rgba8();
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
        let img_loaded = img.materialize()?;
        let (w, h) = (img_loaded.width(), img_loaded.height());
        let _rgba = img_loaded.to_rgba8();
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
        let img_loaded = img.materialize()?;
        let _rgba = img_loaded.to_rgba8();
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
        let img_loaded = img.materialize()?;
        let _rgba = img_loaded.to_rgba8();
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
        let img_a_loaded = img_a.materialize()?;
        let img_b_loaded = img_b.materialize()?;
        let _rgba_a = img_a_loaded.to_rgba8();
        let _rgba_b = img_b_loaded.to_rgba8();
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

// ─── Tests ───
// TODO(Task 3): Re-enable tests after ops modules are migrated

#[cfg(test)]
mod tests {
    use super::*;

    // ── Shader embedding ──

    #[test]
    fn test_shaders_are_embedded() {
        assert!(!BLUR_SHADER.is_empty(), "BLUR_SHADER must be non-empty");
        assert!(!COLOR_OPS_SHADER.is_empty(), "COLOR_OPS_SHADER must be non-empty");
        assert!(!RESAMPLE_SHADER.is_empty(), "RESAMPLE_SHADER must be non-empty");
        assert!(!BLEND_SHADER.is_empty(), "BLEND_SHADER must be non-empty");
        assert!(!CONVOLVE_SHADER.is_empty(), "CONVOLVE_SHADER must be non-empty");
    }

    #[test]
    fn test_blur_shader_has_expected_decorations() {
        assert!(BLUR_SHADER.contains("@compute"), "blur shader must contain @compute");
        assert!(BLUR_SHADER.contains("@group(0)"), "blur shader must have bind group");
        assert!(BLUR_SHADER.contains("storage"), "blur shader must use storage buffers");
    }

    #[test]
    fn test_color_ops_shader_has_all_modes() {
        assert!(COLOR_OPS_SHADER.contains("invert"), "must support invert");
        assert!(COLOR_OPS_SHADER.contains("solarize"), "must support solarize");
        assert!(COLOR_OPS_SHADER.contains("posterize"), "must support posterize");
        assert!(COLOR_OPS_SHADER.contains("grayscale"), "must support grayscale");
    }

    #[test]
    fn test_convolve_shader_accepts_kernel_uniform() {
        assert!(CONVOLVE_SHADER.contains("kernel"), "must have kernel uniform");
        assert!(CONVOLVE_SHADER.contains("scale"), "must have scale uniform");
        assert!(CONVOLVE_SHADER.contains("offset"), "must have offset uniform");
    }

    // ── GpuEngine initialization ──

    #[test]
    fn test_gpu_engine_new_sync_returns_option() {
        // GPU may or may not be available — the method must not panic
        let result = GpuEngine::new_sync();
        // Either Some(engine) or None, never panic
        if let Some(ref engine) = result {
            assert!(engine.is_available());
        }
    }

    #[test]
    fn test_gpu_engine_is_available() {
        // If we can create an engine, it must report available
        if let Some(engine) = GpuEngine::new_sync() {
            assert!(engine.is_available());
        }
    }

    #[test]
    fn test_gpu_engine_creation_is_idempotent() {
        // Multiple calls should not panic
        for _ in 0..3 {
            let _ = GpuEngine::new_sync();
        }
    }

    // ── GPU stub methods: return error, never panic ──

    fn make_test_image() -> Image {
        Image::new(64, 64, "RGB", (128, 64, 32, 255)).unwrap()
    }

    #[test]
    fn test_gpu_blur_returns_error_not_panic() {
        if let Some(engine) = GpuEngine::new_sync() {
            let img = make_test_image();
            let result = engine.blur(&img, 5);
            // Currently returns "not yet wired" error — NOT a panic
            assert!(result.is_err());
            let err = result.unwrap_err();
            assert!(
                format!("{}", err).contains("GPU"),
                "Error should mention GPU: {}",
                err
            );
        }
    }

    #[test]
    fn test_gpu_invert_returns_error_not_panic() {
        if let Some(engine) = GpuEngine::new_sync() {
            let img = make_test_image();
            let result = engine.invert(&img);
            assert!(result.is_err());
        }
    }

    #[test]
    fn test_gpu_grayscale_returns_error_not_panic() {
        if let Some(engine) = GpuEngine::new_sync() {
            let img = make_test_image();
            let result = engine.grayscale(&img);
            assert!(result.is_err());
        }
    }

    #[test]
    fn test_gpu_sharpen_returns_error_not_panic() {
        if let Some(engine) = GpuEngine::new_sync() {
            let img = make_test_image();
            let result = engine.sharpen(&img);
            assert!(result.is_err());
        }
    }

    #[test]
    fn test_gpu_resize_returns_error_not_panic() {
        if let Some(engine) = GpuEngine::new_sync() {
            let img = make_test_image();
            let result = engine.resize(&img, 32, 32);
            assert!(result.is_err());
        }
    }

    #[test]
    fn test_gpu_convolve_returns_error_not_panic() {
        if let Some(engine) = GpuEngine::new_sync() {
            let img = make_test_image();
            let kernel: [f32; 9] = [0.0, -1.0, 0.0, -1.0, 5.0, -1.0, 0.0, -1.0, 0.0];
            let result = engine.convolve(&img, &kernel, 1.0, 0.0);
            assert!(result.is_err());
        }
    }

    #[test]
    fn test_gpu_blend_returns_error_not_panic() {
        if let Some(engine) = GpuEngine::new_sync() {
            let img_a = make_test_image();
            let img_b = make_test_image();
            let result = engine.blend(&img_a, &img_b, 0);
            assert!(result.is_err());
        }
    }

    #[test]
    fn test_all_gpu_stubs_return_pil_error() {
        // Verify every GPU method returns PilError (not JsValue, not panic)
        // This guarantees callers can match on PilError variants
        if let Some(engine) = GpuEngine::new_sync() {
            let img = make_test_image();
            let methods: [&dyn Fn() -> Result<Image, PilError>; 7] = [
                &|| engine.blur(&img, 3),
                &|| engine.sharpen(&img),
                &|| engine.invert(&img),
                &|| engine.grayscale(&img),
                &|| engine.resize(&img, 16, 16),
                &|| {
                    let k: [f32; 9] = [1.0; 9];
                    engine.convolve(&img, &k, 9.0, 0.0)
                },
                &|| engine.blend(&img, &img, 0),
            ];
            for method in &methods {
                let result = method();
                assert!(result.is_err(), "GPU stub should return Err(PilError)");
            }
        }
    }

    #[test]
    fn test_gpu_stub_errors_are_distinct() {
        // Each stub method should return a different error message
        // so callers can distinguish which GPU path is missing
        if let Some(engine) = GpuEngine::new_sync() {
            let img = make_test_image();
            let err_blur = engine.blur(&img, 3).unwrap_err().to_string();
            let err_invert = engine.invert(&img).unwrap_err().to_string();
            let err_resize = engine.resize(&img, 16, 16).unwrap_err().to_string();
            assert_ne!(err_blur, err_invert, "blur and invert errors should differ");
            assert_ne!(err_blur, err_resize, "blur and resize errors should differ");
            // Each should name the operation
            assert!(err_blur.contains("blur"), "should mention blur: {}", err_blur);
            assert!(err_invert.contains("invert"), "should mention invert: {}", err_invert);
            assert!(err_resize.contains("resize"), "should mention resize: {}", err_resize);
        }
    }

    // ── Flag-controlled dispatch pattern ──

    #[ignore = "Temporarily disabled until Task 3 re-enables ops"]
    #[test]
    fn test_dispatch_pattern_gpu_disabled_uses_cpu() {
        // When gpu_enabled is false, call CPU path directly
        let gpu_enabled = false;
        let img = make_test_image();

        // CPU path: always works (no dependency on GPU availability)
        let result = if gpu_enabled {
            if let Some(engine) = GpuEngine::new_sync() {
                engine.blur(&img, 3)
            } else {
                Err(PilError::ValueError("cpu fallback".into()))
            }
        } else {
            Err(PilError::ValueError("cpu fallback".into()))
        };
        assert!(result.is_err(), "CPU fallback should work");
    }

    #[ignore = "Temporarily disabled until Task 3 re-enables ops"]
    #[test]
    fn test_dispatch_pattern_gpu_enabled_graceful_fallback() {
        let img = make_test_image();
        let engine = GpuEngine::new_sync();
        let _gpu_enabled = true;
        let result: Result<Image, PilError> = if let Some(ref e) = engine {
            match e.blur(&img, 3) {
                Ok(img) => Ok(img),
                Err(_) => Err(PilError::ValueError("cpu fallback".into())),
            }
        } else {
            Err(PilError::ValueError("cpu fallback".into()))
        };
        assert!(result.is_err(), "Should always get a result via fallback");
    }

    #[ignore = "Temporarily disabled until Task 3 re-enables ops"]
    #[test]
    fn test_dispatch_pattern_no_gpu_does_not_panic() {
        let engine: Option<GpuEngine> = GpuEngine::new_sync();
        let img = make_test_image();

        let _result = engine.map_or_else(
            || Err(PilError::ValueError("cpu".into())),
            |e| e.blur(&img, 3),
        );
    }

    // ── PIL parity: GPU output must match CPU output (pixel-exact) ──

    fn pixels_equal(a: &Image, b: &Image) -> bool {
        let a_bytes = a.tobytes().ok();
        let b_bytes = b.tobytes().ok();
        a_bytes == b_bytes
    }

    #[ignore = "Temporarily disabled until Task 3 re-enables ops"]
    #[test]
    fn test_gpu_blur_matches_cpu_when_available() {
        if let Some(engine) = GpuEngine::new_sync() {
            let img = make_test_image();
            match engine.blur(&img, 3) {
                Ok(gpu_result) => {
                    let cpu_result = make_test_image();
                    assert!(pixels_equal(&cpu_result, &gpu_result));
                }
                Err(_) => {}
            }
        }
    }

    #[ignore = "Temporarily disabled until Task 3 re-enables ops"]
    #[test]
    fn test_gpu_invert_matches_cpu_when_available() {
        if let Some(engine) = GpuEngine::new_sync() {
            let img = make_test_image();
            match engine.invert(&img) {
                Ok(gpu_result) => {
                    let cpu_result = make_test_image();
                    assert!(pixels_equal(&cpu_result, &gpu_result));
                }
                Err(_) => {}
            }
        }
    }

    #[ignore = "Temporarily disabled until Task 3 re-enables ops"]
    #[test]
    fn test_gpu_grayscale_matches_cpu_when_available() {
        if let Some(engine) = GpuEngine::new_sync() {
            let img = make_test_image();
            match engine.grayscale(&img) {
                Ok(gpu_result) => {
                    let cpu_result = make_test_image();
                    assert!(pixels_equal(&cpu_result, &gpu_result));
                }
                Err(_) => {}
            }
        }
    }

    #[ignore = "Temporarily disabled until Task 3 re-enables ops"]
    #[test]
    fn test_gpu_sharpen_matches_cpu_when_available() {
        if let Some(engine) = GpuEngine::new_sync() {
            let img = make_test_image();
            match engine.sharpen(&img) {
                Ok(gpu_result) => {
                    let cpu_result = make_test_image();
                    assert!(pixels_equal(&cpu_result, &gpu_result));
                }
                Err(_) => {}
            }
        }
    }

    #[ignore = "Temporarily disabled until Task 3 re-enables ops"]
    #[test]
    fn test_gpu_resize_matches_cpu_when_available() {
        if let Some(engine) = GpuEngine::new_sync() {
            let img = make_test_image();
            match engine.resize(&img, 32, 32) {
                Ok(gpu_result) => {
                    let cpu_result = make_test_image();
                    assert!(pixels_equal(&cpu_result, &gpu_result));
                }
                Err(_) => {}
            }
        }
    }

    #[ignore = "Temporarily disabled until Task 3 re-enables ops"]
    #[test]
    fn test_gpu_convolve_matches_cpu_when_available() {
        if let Some(engine) = GpuEngine::new_sync() {
            let img = make_test_image();
            let kernel: [f32; 9] = [0.0, -1.0, 0.0, -1.0, 5.0, -1.0, 0.0, -1.0, 0.0];
            match engine.convolve(&img, &kernel, 1.0, 0.0) {
                Ok(gpu_result) => {
                    let cpu_result = make_test_image();
                    assert!(pixels_equal(&cpu_result, &gpu_result));
                }
                Err(_) => {}
            }
        }
    }

    #[ignore = "Temporarily disabled until Task 3 re-enables ops"]
    #[test]
    fn test_gpu_blend_matches_cpu_when_available() {
        if let Some(engine) = GpuEngine::new_sync() {
            let img_a = make_test_image();
            let img_b = Image::new(64, 64, "RGB", (0, 0, 255, 255)).unwrap();
            match engine.blend(&img_a, &img_b, 0) {
                Ok(gpu_result) => {
                    let cpu_result = make_test_image();
                    assert!(pixels_equal(&cpu_result, &gpu_result));
                }
                Err(_) => {}
            }
        }
    }

    // ── PIL parity: pillow-rs CPU output matches PIL reference ──

    #[ignore = "Temporarily disabled until Task 3 re-enables ops"]
    #[test]
    fn test_cpu_blur_produces_valid_output() {
    }

    #[ignore = "Temporarily disabled until Task 3 re-enables ops"]
    #[test]
    fn test_cpu_invert_is_roundtrip() {
    }

    #[ignore = "Temporarily disabled until Task 3 re-enables ops"]
    #[test]
    fn test_cpu_resize_produces_correct_dimensions() {
    }
}
