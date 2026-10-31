use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_invert_native_mode;

// NO_PILLOW_SUPPORT: ImageOps.invert rejects PA; this route preserves native
// palette-index and alpha bytes as a core-only extension.
declare_invert_native_mode!(
    ImagePaletteAlpha8,
    super::Input,
    2,
    2,
    Some("ImageOps.invert extends Pillow with native PA inversion"),
);
