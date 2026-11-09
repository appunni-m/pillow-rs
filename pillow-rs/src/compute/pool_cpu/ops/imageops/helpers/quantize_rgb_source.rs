//! RGB quantization kernels over statically dispatched native mode samples.

use crate::compute::pool_cpu::helpers::quantization::{
    RgbPixelSource, RgbaPixelSource, kmeans_refine_source, map_native_to_fixed_palette,
    maxcoverage_quantize_source, median_cut_quantize_source, quantize_octree_rgb_source,
    quantize_rgba_source,
};
use crate::compute::registry::support::PaletteInput;
use crate::error::PilError;
use crate::raster::color::FromPrimitive;

/// Projects one mode-native sample into an 8-bit palette component.
pub(crate) trait QuantizeSampleToU8: Copy {
    /// Projects one native sample into the byte palette's component range.
    fn to_palette_u8(self) -> u8;
}

impl QuantizeSampleToU8 for u8 {
    #[inline(always)]
    fn to_palette_u8(self) -> u8 {
        self
    }
}

impl QuantizeSampleToU8 for u16 {
    #[inline(always)]
    fn to_palette_u8(self) -> u8 {
        u8::from_primitive(self)
    }
}

impl QuantizeSampleToU8 for f32 {
    #[inline(always)]
    fn to_palette_u8(self) -> u8 {
        u8::from_primitive(self)
    }
}

/// Reads interleaved native mode samples without materializing a converted raster.
struct InterleavedRgbSamples<'a, T, const CHANNELS: usize> {
    samples: &'a [T],
}

/// Runs the exact RGB quantizer over a native interleaved mode buffer.
pub(crate) fn execute_interleaved<T, const CHANNELS: usize>(
    samples: &[T],
    width: u32,
    height: u32,
    colors: usize,
    kmeans: u32,
    method: u32,
) -> Result<(Vec<u8>, Vec<u8>, Option<Vec<u8>>), PilError>
where
    T: QuantizeSampleToU8,
{
    let source = InterleavedRgbSamples::<T, CHANNELS>::new(samples);
    execute(&source, width, height, colors, kmeans, method)
}

/// Maps native RGB lanes from an interleaved mode carrier to a fixed palette.
pub(crate) fn execute_interleaved_with_palette<T, const CHANNELS: usize>(
    samples: &[T],
    width: u32,
    height: u32,
    palette: &[[u8; 3]],
    dither: bool,
) -> Result<Vec<u8>, PilError>
where
    T: QuantizeSampleToU8,
{
    let source = InterleavedRgbSamples::<T, CHANNELS>::new(samples);
    map_native_to_fixed_palette(&source, width, height, palette, dither)
}

/// Runs quantization over I;16 samples using its clipped byte projection.
pub(crate) fn execute_clipped_luma16(
    samples: &[u16],
    width: u32,
    height: u32,
    colors: usize,
    kmeans: u32,
    method: u32,
) -> Result<(Vec<u8>, Vec<u8>, Option<Vec<u8>>), PilError> {
    let source = ClippedLuma16Samples::new(samples);
    execute(&source, width, height, colors, kmeans, method)
}

/// Quantizes native I-mode scalars using a clamped byte-grayscale projection.
pub(crate) fn execute_i32_scalar(
    samples: &[u8],
    width: u32,
    height: u32,
    colors: usize,
    kmeans: u32,
    method: u32,
) -> Result<(Vec<u8>, Vec<u8>, Option<Vec<u8>>), PilError> {
    execute(
        &I32ScalarSamples(samples),
        width,
        height,
        colors,
        kmeans,
        method,
    )
}

/// Quantizes native F-mode scalars using the raster float-to-byte projection.
pub(crate) fn execute_f32_scalar(
    samples: &[u8],
    width: u32,
    height: u32,
    colors: usize,
    kmeans: u32,
    method: u32,
) -> Result<(Vec<u8>, Vec<u8>, Option<Vec<u8>>), PilError> {
    execute(
        &F32ScalarSamples(samples),
        width,
        height,
        colors,
        kmeans,
        method,
    )
}

/// Quantizes two- or four-channel native samples into an alpha palette.
pub(crate) fn execute_rgba_interleaved<T, const CHANNELS: usize>(
    samples: &[T],
    width: u32,
    height: u32,
    colors: usize,
) -> Result<(Vec<u8>, Vec<u8>, Option<Vec<u8>>), PilError>
where
    T: QuantizeSampleToU8,
{
    let source = InterleavedRgbaSamples::<T, CHANNELS>::new(samples);
    execute_rgba_source(&source, width, height, colors)
}

/// Quantizes native premultiplied luma-alpha samples without a staging raster.
pub(crate) fn execute_premultiplied_luma_alpha(
    samples: &[u8],
    width: u32,
    height: u32,
    colors: usize,
) -> Result<(Vec<u8>, Vec<u8>, Option<Vec<u8>>), PilError> {
    let source = PremultipliedLumaAlphaSamples(samples);
    execute_rgba_source(&source, width, height, colors)
}

/// Quantizes native RGBa samples after per-pixel unpremultiplication.
pub(crate) fn execute_premultiplied_rgba(
    samples: &[u8],
    width: u32,
    height: u32,
    colors: usize,
) -> Result<(Vec<u8>, Vec<u8>, Option<Vec<u8>>), PilError> {
    let source = PremultipliedRgbaSamples(samples);
    execute_rgba_source(&source, width, height, colors)
}

/// Quantizes PA indices with attached palette colors and per-pixel alpha.
pub(crate) fn execute_palette_alpha(
    samples: &[u8],
    palette: PaletteInput<'_>,
    width: u32,
    height: u32,
    colors: usize,
) -> Result<(Vec<u8>, Vec<u8>, Option<Vec<u8>>), PilError> {
    let source = PaletteAlphaSamples::new(samples, palette);
    execute_rgba_source(&source, width, height, colors)
}

impl<'a, T, const CHANNELS: usize> InterleavedRgbSamples<'a, T, CHANNELS> {
    /// Creates a statically shaped view over a native image buffer.
    pub(crate) const fn new(samples: &'a [T]) -> Self {
        Self { samples }
    }
}

impl<T: QuantizeSampleToU8, const CHANNELS: usize> RgbPixelSource
    for InterleavedRgbSamples<'_, T, CHANNELS>
{
    #[inline(always)]
    fn pixel_count(&self) -> usize {
        self.samples.len() / CHANNELS
    }

    #[inline(always)]
    fn rgb_at(&self, index: usize) -> [u8; 3] {
        let offset = index * CHANNELS;
        let first = self.samples[offset].to_palette_u8();
        if CHANNELS < 3 {
            [first, first, first]
        } else {
            [
                first,
                self.samples[offset + 1].to_palette_u8(),
                self.samples[offset + 2].to_palette_u8(),
            ]
        }
    }
}

struct InterleavedRgbaSamples<'a, T, const CHANNELS: usize> {
    samples: &'a [T],
}

impl<'a, T, const CHANNELS: usize> InterleavedRgbaSamples<'a, T, CHANNELS> {
    const fn new(samples: &'a [T]) -> Self {
        Self { samples }
    }
}

impl<T: QuantizeSampleToU8, const CHANNELS: usize> RgbaPixelSource
    for InterleavedRgbaSamples<'_, T, CHANNELS>
{
    #[inline(always)]
    fn pixel_count(&self) -> usize {
        self.samples.len() / CHANNELS
    }

    #[inline(always)]
    fn rgba_at(&self, index: usize) -> [u8; 4] {
        let offset = index * CHANNELS;
        let first = self.samples[offset].to_palette_u8();
        if CHANNELS == 2 {
            [
                first,
                first,
                first,
                self.samples[offset + 1].to_palette_u8(),
            ]
        } else {
            [
                first,
                self.samples[offset + 1].to_palette_u8(),
                self.samples[offset + 2].to_palette_u8(),
                self.samples[offset + 3].to_palette_u8(),
            ]
        }
    }
}

struct PremultipliedLumaAlphaSamples<'a>(&'a [u8]);

impl RgbaPixelSource for PremultipliedLumaAlphaSamples<'_> {
    #[inline(always)]
    fn pixel_count(&self) -> usize {
        self.0.len() / 2
    }

    #[inline(always)]
    fn rgba_at(&self, index: usize) -> [u8; 4] {
        let offset = index * 2;
        let alpha = self.0[offset + 1];
        let luma = crate::compute::pool_cpu::helpers::bytes::unpremultiply::unpremultiply_byte(
            self.0[offset],
            alpha,
        );
        [luma, luma, luma, alpha]
    }
}

struct PremultipliedRgbaSamples<'a>(&'a [u8]);

impl RgbaPixelSource for PremultipliedRgbaSamples<'_> {
    #[inline(always)]
    fn pixel_count(&self) -> usize {
        self.0.len() / 4
    }

    #[inline(always)]
    fn rgba_at(&self, index: usize) -> [u8; 4] {
        let offset = index * 4;
        let alpha = self.0[offset + 3];
        let unpremultiply =
            crate::compute::pool_cpu::helpers::bytes::unpremultiply::unpremultiply_byte;
        [
            unpremultiply(self.0[offset], alpha),
            unpremultiply(self.0[offset + 1], alpha),
            unpremultiply(self.0[offset + 2], alpha),
            alpha,
        ]
    }
}

struct PaletteAlphaSamples<'a> {
    samples: &'a [u8],
    palette: [[u8; 3]; 256],
    palette_alpha: &'a [u8],
}

impl<'a> PaletteAlphaSamples<'a> {
    fn new(samples: &'a [u8], palette: PaletteInput<'a>) -> Self {
        let mut colors = [[0_u8; 3]; 256];
        let rgb = palette.rgb.unwrap_or_default();
        for (index, color) in colors.iter_mut().enumerate() {
            let offset = index * 3;
            *color = [
                rgb.get(offset).copied().unwrap_or(0),
                rgb.get(offset + 1).copied().unwrap_or(0),
                rgb.get(offset + 2).copied().unwrap_or(0),
            ];
        }
        Self {
            samples,
            palette: colors,
            palette_alpha: palette.alpha,
        }
    }
}

impl RgbaPixelSource for PaletteAlphaSamples<'_> {
    #[inline(always)]
    fn pixel_count(&self) -> usize {
        self.samples.len() / 2
    }

    #[inline(always)]
    fn rgba_at(&self, index: usize) -> [u8; 4] {
        let offset = index * 2;
        let palette_index = usize::from(self.samples[offset]);
        let color = self.palette[palette_index];
        let sample_alpha = self.samples[offset + 1];
        let palette_alpha = self
            .palette_alpha
            .get(palette_index)
            .copied()
            .unwrap_or(u8::MAX);
        let alpha = ((u16::from(sample_alpha) * u16::from(palette_alpha) + 127) / 255) as u8;
        [color[0], color[1], color[2], alpha]
    }
}

fn execute_rgba_source<S: RgbaPixelSource + ?Sized>(
    source: &S,
    width: u32,
    height: u32,
    colors: usize,
) -> Result<(Vec<u8>, Vec<u8>, Option<Vec<u8>>), PilError> {
    let (indices, palette, alpha) = quantize_rgba_source(source, width, height, colors)?;
    Ok((indices, palette, Some(alpha)))
}

/// Reads Pillow I;16 samples with its byte-clipping conversion semantics.
struct ClippedLuma16Samples<'a>(&'a [u16]);

impl<'a> ClippedLuma16Samples<'a> {
    /// Creates a view that clips native I;16 values to the byte range.
    pub(crate) const fn new(samples: &'a [u16]) -> Self {
        Self(samples)
    }
}

impl RgbPixelSource for ClippedLuma16Samples<'_> {
    #[inline(always)]
    fn pixel_count(&self) -> usize {
        self.0.len()
    }

    #[inline(always)]
    fn rgb_at(&self, index: usize) -> [u8; 3] {
        let sample = self.0[index].min(u16::from(u8::MAX)) as u8;
        [sample, sample, sample]
    }
}

struct I32ScalarSamples<'a>(&'a [u8]);

impl RgbPixelSource for I32ScalarSamples<'_> {
    #[inline(always)]
    fn pixel_count(&self) -> usize {
        self.0.len() / 4
    }

    #[inline(always)]
    fn rgb_at(&self, index: usize) -> [u8; 3] {
        let offset = index * 4;
        let value = i32::from_le_bytes([
            self.0[offset],
            self.0[offset + 1],
            self.0[offset + 2],
            self.0[offset + 3],
        ]);
        let sample = value.clamp(0, i32::from(u8::MAX)) as u8;
        [sample, sample, sample]
    }
}

struct F32ScalarSamples<'a>(&'a [u8]);

impl RgbPixelSource for F32ScalarSamples<'_> {
    #[inline(always)]
    fn pixel_count(&self) -> usize {
        self.0.len() / 4
    }

    #[inline(always)]
    fn rgb_at(&self, index: usize) -> [u8; 3] {
        let offset = index * 4;
        let value = f32::from_le_bytes([
            self.0[offset],
            self.0[offset + 1],
            self.0[offset + 2],
            self.0[offset + 3],
        ]);
        let sample = value.to_palette_u8();
        [sample, sample, sample]
    }
}

/// Runs the shared RGB algorithms over one mode's operation-defined samples.
pub(crate) fn execute<S>(
    source: &S,
    width: u32,
    height: u32,
    colors: usize,
    kmeans: u32,
    method: u32,
) -> Result<(Vec<u8>, Vec<u8>, Option<Vec<u8>>), PilError>
where
    S: RgbPixelSource + ?Sized,
{
    match method {
        2 => {
            let (indices, palette) = quantize_octree_rgb_source(source, width, height, colors)?;
            Ok((indices, rgba_palette_rgb(&palette), None))
        }
        1 => {
            let (indices, palette) = maxcoverage_quantize_source(source, colors, kmeans);
            Ok((indices, flatten_rgb_palette(&palette), None))
        }
        0 => {
            let (indices, mut palette) = median_cut_quantize_source(source, colors);
            let indices = refine_median_cut_kmeans(source, indices, &mut palette, kmeans);
            Ok((indices, flatten_rgb_palette(&palette), None))
        }
        3 => Err(PilError::ValueError(
            "dependency required by this method was not enabled at compile time".into(),
        )),
        _ => Err(PilError::ValueError("quantization error".into())),
    }
}

fn refine_median_cut_kmeans<S>(
    source: &S,
    indices: Vec<u8>,
    palette: &mut [[u8; 3]],
    kmeans: u32,
) -> Vec<u8>
where
    S: RgbPixelSource + ?Sized,
{
    if kmeans == 0 {
        return indices;
    }
    let mut indices: Vec<u32> = indices.into_iter().map(u32::from).collect();
    kmeans_refine_source(source, palette, &mut indices, kmeans - 1);
    indices.into_iter().map(|index| index as u8).collect()
}

fn flatten_rgb_palette(palette: &[[u8; 3]]) -> Vec<u8> {
    palette
        .iter()
        .flat_map(|color| [color[0], color[1], color[2]])
        .collect()
}

fn rgba_palette_rgb(palette: &[u8]) -> Vec<u8> {
    palette
        .chunks_exact(4)
        .flat_map(|color| [color[0], color[1], color[2]])
        .collect()
}
