//! Pillow-compatible image filter operations.
//!
//! Built-in filter names use Pillow `ImageFilter` kernel definitions. The
//! public methods return lazy pipeline images where possible.

use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::PipelineOp;

/// Normalizes the arguments accepted by `ImageFilter.Kernel`.
///
/// The Python facade only converts host values to Rust primitives. Keeping
/// the default kernel, scale derivation, size validation, and offset
/// conversion here means every binding observes the same convolution
/// contract.
pub fn prepare_kernel(
    kernel: Option<Vec<f64>>,
    scale: Option<f64>,
    offset: f64,
    size: (u32, u32),
) -> Result<(Vec<f64>, f64, i32, u32), PilError> {
    let (size_x, size_y) = size;
    if size_x != size_y || (size_x != 3 && size_x != 5) {
        return Err(PilError::ValueError("bad kernel size".into()));
    }
    let numel = (size_x * size_y) as usize;
    let kernel = kernel.unwrap_or_else(|| vec![1.0; numel]);
    validate_kernel_coefficients(Some(&kernel), size)?;
    let scale = scale.unwrap_or_else(|| kernel.iter().sum());
    Ok((kernel, scale, offset as i32, size_x))
}

/// Validates the coefficient count shared by `Kernel` construction and
/// application. Size-shape validation intentionally remains in
/// [`prepare_kernel`], because Pillow defers that particular error until the
/// filter is applied.
pub fn validate_kernel_coefficients(
    kernel: Option<&[f64]>,
    size: (u32, u32),
) -> Result<(), PilError> {
    let expected = (size.0 as usize).saturating_mul(size.1 as usize);
    if kernel.is_some_and(|values| values.len() != expected) {
        return Err(PilError::ValueError(
            "not enough coefficients in kernel".into(),
        ));
    }
    Ok(())
}

/// Pre-defined filter kernels matching PIL's ImageFilter module exactly.
struct FilterKernel {
    kernel: [f32; 9],
    scale: f32,
    offset: i32,
}

impl FilterKernel {
    const fn new(kernel: [f32; 9], scale: f32, offset: i32) -> Self {
        FilterKernel {
            kernel,
            scale,
            offset,
        }
    }
}

/// PIL filter definitions (verified against Pillow 12.2.0 filterargs).
/// BLUR: 5x5 circular blur
const BLUR_5X5: [f32; 25] = [
    1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 0.0,
    1.0, 1.0, 1.0, 1.0, 1.0, 1.0,
];
const BLUR_5X5_SCALE: f32 = 16.0;

/// SMOOTH_MORE: 5x5 Gaussian-like (PIL 12.2.0 runtime filterargs)
const SMOOTH_MORE_5X5: [f32; 25] = [
    1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 5.0, 5.0, 5.0, 1.0, 1.0, 5.0, 44.0, 5.0, 1.0, 1.0, 5.0, 5.0, 5.0,
    1.0, 1.0, 1.0, 1.0, 1.0, 1.0,
];
const SMOOTH_MORE_5X5_SCALE: f32 = 100.0;

const CONTOUR: FilterKernel = FilterKernel::new(
    [-1.0, -1.0, -1.0, -1.0, 8.0, -1.0, -1.0, -1.0, -1.0],
    1.0,
    255,
);

const DETAIL: FilterKernel =
    FilterKernel::new([0.0, -1.0, 0.0, -1.0, 10.0, -1.0, 0.0, -1.0, 0.0], 6.0, 0);

const EDGE_ENHANCE: FilterKernel = FilterKernel::new(
    [-1.0, -1.0, -1.0, -1.0, 10.0, -1.0, -1.0, -1.0, -1.0],
    2.0,
    0,
);

const EDGE_ENHANCE_MORE: FilterKernel = FilterKernel::new(
    [-1.0, -1.0, -1.0, -1.0, 9.0, -1.0, -1.0, -1.0, -1.0],
    1.0,
    0,
);

// PIL 12.2.0 filterargs at runtime: ((-1, 0, 0), (0, 1, 0), (0, 0, 0))
const EMBOSS: FilterKernel =
    FilterKernel::new([-1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0], 1.0, 128);

const FIND_EDGES: FilterKernel = FilterKernel::new(
    [-1.0, -1.0, -1.0, -1.0, 8.0, -1.0, -1.0, -1.0, -1.0],
    1.0,
    0,
);

const SHARPEN: FilterKernel = FilterKernel::new(
    [-2.0, -2.0, -2.0, -2.0, 32.0, -2.0, -2.0, -2.0, -2.0],
    16.0,
    0,
);

const SMOOTH: FilterKernel =
    FilterKernel::new([1.0, 1.0, 1.0, 1.0, 5.0, 1.0, 1.0, 1.0, 1.0], 13.0, 0);

impl Image {
    /// Validates whether a Python-facing filter may run for this image mode.
    pub fn validate_filter(&self, filter_name: &str) -> Result<(), PilError> {
        let mode = self.mode()?;
        if mode == "P" && filter_name != "Mode" {
            return Err(PilError::ValueError("cannot filter palette images".into()));
        }
        // Pillow's ImagingGaussianBlur path rejects PA samples with the
        // generic wrong-mode error instead of expanding the palette. Keep the
        // validation at the core boundary so Python and other bindings do not
        // enter a non-Pillow RGBA fallback.
        if mode == "PA" && filter_name == "GaussianBlur" {
            return Err(PilError::ValueError("image has wrong mode".into()));
        }
        if mode == "F"
            && !matches!(
                filter_name,
                "MaxFilter" | "MinFilter" | "MedianFilter" | "RankFilter"
            )
        {
            return Err(PilError::ValueError("image has wrong mode".into()));
        }
        Ok(())
    }

    /// Applies a named built-in Pillow filter.
    ///
    /// Supported names are the built-in `ImageFilter` kernels such as `"BLUR"`,
    /// `"CONTOUR"`, `"DETAIL"`, `"EDGE_ENHANCE"`, `"FIND_EDGES"`, `"SHARPEN"`,
    /// `"SMOOTH"`, and `"SMOOTH_MORE"`. Modes `"1"`, `"I"`, and `"F"` follow
    /// Pillow's mode-specific conversion rules.
    ///
    /// # Errors
    ///
    /// Returns [`PilError::ValueError`] for an unknown filter name, or another
    /// [`PilError`] when mode conversion or materialization fails.
    pub fn filter(&self, filter_type: &str) -> Result<Image, PilError> {
        self.validate_filter(filter_type)?;
        // I-mode: push the filter op directly — the CPU registry handles I-mode dispatch.
        // by operating on int32 pixel values with no [0,255] clipping.
        if self.explicit_mode() == Some("I") {
            return self.filter_push(filter_type);
        }
        // Mode "1" (binary): stored as Luma8 (0/255), filter applies on L data.
        // No conversion needed.
        self.filter_push(filter_type)
    }

    /// Push the appropriate PipelineOp for a built-in filter, without mode conversion.
    fn filter_push(&self, filter_type: &str) -> Result<Image, PilError> {
        match filter_type {
            "BLUR" => Ok(Image::push_op(
                self,
                PipelineOp::Filter5x5 {
                    kernel: BLUR_5X5,
                    scale: BLUR_5X5_SCALE,
                    offset: 0,
                },
            )),
            "SMOOTH_MORE" => Ok(Image::push_op(
                self,
                PipelineOp::Filter5x5 {
                    kernel: SMOOTH_MORE_5X5,
                    scale: SMOOTH_MORE_5X5_SCALE,
                    offset: 0,
                },
            )),
            name => {
                let k = match name {
                    "CONTOUR" => &CONTOUR,
                    "DETAIL" => &DETAIL,
                    "EDGE_ENHANCE" => &EDGE_ENHANCE,
                    "EDGE_ENHANCE_MORE" => &EDGE_ENHANCE_MORE,
                    "EMBOSS" => &EMBOSS,
                    "FIND_EDGES" => &FIND_EDGES,
                    "SHARPEN" => &SHARPEN,
                    "SMOOTH" => &SMOOTH,
                    _ => {
                        return Err(PilError::NotImplementedError(format!(
                            "Filter '{}' not yet implemented",
                            name
                        )));
                    }
                };
                Ok(Image::push_op(
                    self,
                    PipelineOp::Filter3x3 {
                        kernel: k.kernel,
                        scale: k.scale,
                        offset: k.offset,
                    },
                ))
            }
        }
    }

    /// Applies a custom convolution kernel.
    ///
    /// `size` must be `3` or `5`. `kernel` must contain at least
    /// `size * size` coefficients; extra coefficients are ignored.
    ///
    /// # Errors
    ///
    /// Returns [`PilError::ValueError`] when `size` is unsupported or `kernel`
    /// does not contain enough coefficients.
    pub fn kernel_filter(
        &self,
        kernel: Option<Vec<f64>>,
        scale: Option<f64>,
        offset: f64,
        size: (u32, u32),
    ) -> Result<Image, PilError> {
        self.validate_filter("Kernel")?;
        let (kernel, scale, offset, size) = prepare_kernel(kernel, scale, offset, size)?;
        let scale = (scale as f32).max(0.0001);

        if size == 3 {
            let mut prepared = [0.0f32; 9];
            for (destination, source) in prepared.iter_mut().zip(kernel.iter()) {
                *destination = *source as f32;
            }
            Ok(Image::push_op(
                self,
                PipelineOp::Filter3x3 {
                    kernel: prepared,
                    scale,
                    offset,
                },
            ))
        } else {
            // `prepare_kernel` accepts only square 3x3 and 5x5 kernels, so
            // this branch is the validated 5x5 path.
            let mut prepared = [0.0f32; 25];
            for (destination, source) in prepared.iter_mut().zip(kernel.iter()) {
                *destination = *source as f32;
            }
            Ok(Image::push_op(
                self,
                PipelineOp::Filter5x5 {
                    kernel: prepared,
                    scale,
                    offset,
                },
            ))
        }
    }
}
