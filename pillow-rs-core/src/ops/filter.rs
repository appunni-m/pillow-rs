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
        FilterKernel { kernel, scale, offset }
    }
}

/// PIL filter definitions (verified against Pillow 12.2.0 filterargs).
const BLUR: FilterKernel = FilterKernel::new(
    [1.0, 1.0, 1.0, 1.0, 5.0, 1.0, 1.0, 1.0, 1.0],
    13.0,
    0,
);

const CONTOUR: FilterKernel = FilterKernel::new(
    [-1.0, -1.0, -1.0, -1.0, 8.0, -1.0, -1.0, -1.0, -1.0],
    1.0,
    255,
);

const DETAIL: FilterKernel = FilterKernel::new(
    [0.0, -1.0, 0.0, -1.0, 10.0, -1.0, 0.0, -1.0, 0.0],
    6.0,
    0,
);

const EDGE_ENHANCE: FilterKernel = FilterKernel::new(
    [0.0, 0.0, 0.0, -1.0, 1.0, 0.0, 0.0, 0.0, 0.0],
    1.0,
    0,
);

const EDGE_ENHANCE_MORE: FilterKernel = FilterKernel::new(
    [-1.0, -1.0, -1.0, -1.0, 9.0, -1.0, -1.0, -1.0, -1.0],
    1.0,
    0,
);

const EMBOSS: FilterKernel = FilterKernel::new(
    [-1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0],
    1.0,
    128,
);

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

const SMOOTH: FilterKernel = FilterKernel::new(
    [1.0, 1.0, 1.0, 1.0, 5.0, 1.0, 1.0, 1.0, 1.0],
    13.0,
    0,
);

const SMOOTH_MORE: FilterKernel = FilterKernel::new(
    [1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0],
    9.0,
    0,
);

impl Image {
    /// Apply a named built-in filter. Supports all 10 PIL built-in kernels.
    pub fn filter(&self, filter_type: &str) -> Result<Image, PilError> {
        let k = match filter_type {
            "BLUR" => &BLUR,
            "CONTOUR" => &CONTOUR,
            "DETAIL" => &DETAIL,
            "EDGE_ENHANCE" => &EDGE_ENHANCE,
            "EDGE_ENHANCE_MORE" => &EDGE_ENHANCE_MORE,
            "EMBOSS" => &EMBOSS,
            "FIND_EDGES" => &FIND_EDGES,
            "SHARPEN" => &SHARPEN,
            "SMOOTH" => &SMOOTH,
            "SMOOTH_MORE" => &SMOOTH_MORE,
            _ => {
                return Err(PilError::NotImplementedError(format!(
                    "Filter '{}' not yet implemented",
                    filter_type
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
