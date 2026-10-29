//! Expand kernels over the image's native sample representation.

mod default;

pub(super) use default::{expand_f32, expand_native_bytes, expand_u16, expanded_dimensions};
