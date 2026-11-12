//! Reusable imageops kernels and strategy declarations.

pub(crate) mod autocontrast;
pub(crate) mod colorize;
pub(crate) mod equalize;
pub(crate) mod expand;
pub(crate) mod expand_edges;
pub(crate) mod expand_vector;
pub(crate) mod hidden_lanes;
pub(crate) mod histogram;
pub(crate) mod native_bytes;
pub(crate) mod pixel;
pub(crate) mod point_lut;
pub(crate) mod point_support;
pub(crate) mod quantize;
pub(crate) mod quantize_rgb_source;
pub(crate) mod strategy;
pub(crate) mod wrapper_routes;

pub(crate) use wrapper_routes::{
    IMAGEOPS_COLORIZE_CHILDREN, IMAGEOPS_COLORIZE_LUT_CHILDREN,
    IMAGEOPS_INVERT_CHILDREN, IMAGEOPS_LUT_CHILDREN,
};
