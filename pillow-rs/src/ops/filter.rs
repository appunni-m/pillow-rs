//! Pillow-compatible image filter operations.
//!
//! Built-in filter names use Pillow `ImageFilter` kernel definitions. The
//! public methods return lazy pipeline images where possible.

use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::PipelineOp;

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
        // I-mode: push the filter op directly — execute_op handles I-mode dispatch
        // by operating on int32 pixel values with no [0,255] clipping.
        if self.explicit_mode() == Some("I") {
            return self.filter_push(filter_type);
        }
        // F-mode: convert to L, apply filter, convert back to F
        if self.explicit_mode() == Some("F") {
            let l_img = self.convert("L", None, None, None, None)?;
            let filtered = l_img.filter(filter_type)?;
            return filtered.convert("F", None, None, None, None);
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
                        )))
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
        kernel: &[f32],
        scale: f32,
        offset: i32,
        size: u32,
    ) -> Result<Image, PilError> {
        match size {
            3 => {
                let mut k = [0.0f32; 9];
                if kernel.len() < 9 {
                    return Err(PilError::ValueError(
                        "Kernel must have at least 9 elements for 3x3".into(),
                    ));
                }
                k.copy_from_slice(&kernel[..9]);
                Ok(Image::push_op(
                    self,
                    PipelineOp::Filter3x3 {
                        kernel: k,
                        scale: scale.max(0.0001),
                        offset,
                    },
                ))
            }
            5 => {
                let mut k = [0.0f32; 25];
                if kernel.len() < 25 {
                    return Err(PilError::ValueError(
                        "Kernel must have at least 25 elements for 5x5".into(),
                    ));
                }
                k.copy_from_slice(&kernel[..25]);
                Ok(Image::push_op(
                    self,
                    PipelineOp::Filter5x5 {
                        kernel: k,
                        scale: scale.max(0.0001),
                        offset,
                    },
                ))
            }
            _ => Err(PilError::ValueError("Kernel size must be 3 or 5".into())),
        }
    }
}
