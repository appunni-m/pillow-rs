//! Shared Color3DLUT interpolation and mode-specific output kernels.

#[path = "kernels/default.rs"]
mod default;
#[path = "kernels/rgb.rs"]
mod rgb;

pub(super) use default::execute as execute_default;
pub(super) use rgb::project_rgb;
