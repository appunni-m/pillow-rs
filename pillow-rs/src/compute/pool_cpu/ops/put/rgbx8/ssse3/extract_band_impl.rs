//! Original ssse3 native-band implementation for `rgbx8`.

use crate::error::PilError;
use crate::raster::DynamicImage;
use super::super::default::extract_band_impl as scalar;
use super::super::super::helpers::extract_band_vector as vector;

pub(crate) type Kernel<'a> = scalar::Kernel<'a>;

#[inline]
pub(crate) fn prepare(input: scalar::Kernel<'_>) -> Kernel<'_> {
    input
}

/// # Safety
///
/// Preparation must establish SSE2/SSSE3 and the exact native source/index guard.
#[allow(unsafe_code)]
pub(crate) unsafe fn execute(input: Kernel<'_>) -> Result<DynamicImage, PilError> {
    #[cfg(target_arch = "x86_64")]
    {
        // SAFETY: the mode strategy established the documented capability,
        // native layout, source bounds, and index before retaining this kernel.
        unsafe { vector::ssse3::<crate::raster::Rgba<u8>, 4>(input) }
    }
    #[cfg(target_arch = "x86")]
    {
        vector::portable::<crate::raster::Rgba<u8>, 4>(input)
    }
}
