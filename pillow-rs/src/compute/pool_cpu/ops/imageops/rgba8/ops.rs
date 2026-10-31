use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_invert_native_mode;

// NO_PILLOW_SUPPORT: ImageOps.invert rejects RGBA; this route preserves the
// native carrier as a core-only extension.
declare_invert_native_mode!(
    ImageRgba8,
    super::Input,
    4,
    4,
    Some("ImageOps.invert extends Pillow with native RGBA inversion"),
);
