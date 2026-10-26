//! Shared Resize, aspect-resize, and native-layout admission contracts.

use super::super::{SIMD_RESIZE_NEAREST_BYTES, SimdImageShape, SimdLayout};
use super::{
    native_resize_byte_layout_for_shape, resize_native_channels_for_image,
    resize_nearest_vectorizable,
};
use crate::pipeline::ResampleFilter;
use crate::raster::DynamicImage;

mod aspect;
mod resize;
pub(in crate::compute::pool_simd::ops::adapters) use aspect::*;
pub(in crate::compute::pool_simd::ops::adapters) use resize::*;
