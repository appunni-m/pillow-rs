//! Image filter operations — built-in PIL filter kernels.
//! Uses PIL-identical 3x3 convolution kernels with matching scale and offset.

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
    /// Apply a named built-in filter. Supports all 10 PIL built-in kernels.
    pub fn filter(&self, filter_type: &str) -> Result<Image, PilError> {
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

    /// Apply a generic kernel filter (custom convolution).
    /// `size` is the kernel dimension (3 or 5).
    /// `kernel` must have `size*size` elements.
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
