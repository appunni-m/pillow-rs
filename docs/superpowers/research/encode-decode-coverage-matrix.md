# Unified Decode + Encode Coverage Matrix

Each row = one test case mapped to one assertion.
Decode: input asset → decode → compare bytes with PIL reference.
Encode: decode reference → encode with params → decode → compare bytes.

Status: ✅ pass | ❌ fail | ⬜ no asset | 🔧 not wired (encode params)

## JPEG (31 decode + 33 encode = 64 total)

| # | Type | ID | Variation | Asset/Params | Status |
|---|------|----|-----------|-------------|--------|
| 1 | DEC | subsampling_444 | 4:4:4 chroma subsampling (no subsampling) | baseline_444.jpg | ✅ |
| 2 | DEC | subsampling_422 | 4:2:2 horizontal chroma subsampling | baseline_422.jpg | ✅ |
| 3 | DEC | subsampling_420 | 4:2:0 chroma subsampling (most common) | baseline_420.jpg | ✅ |
| 4 | DEC | subsampling_411 | 4:1:1 chroma subsampling | baseline_411.jpg | ✅ |
| 5 | DEC | subsampling_gray | Grayscale (no chroma components) | baseline_gray.jpg | ✅ |
| 6 | DEC | color_ycbcr | YCbCr color space (standard) | baseline_ycbcr.jpg | ✅ |
| 7 | DEC | color_rgb | RGB color space (rare, Adobe) | baseline_rgb_jpeg.jpg | ✅ |
| 8 | DEC | color_cmyk | CMYK color space (Adobe) | baseline_cmyk.jpg | ✅ |
| 9 | DEC | huffman_default | Default Huffman tables | baseline_default.jpg | ✅ |
| 10 | DEC | huffman_optimized | Optimized/custom Huffman tables | baseline_optimized.jpg | ✅ |
| 11 | DEC | huffman_progressive | Progressive JPEG (multiple scans) | progressive.jpg | ❌ |
| 12 | DEC | quality_100 | Quality 100 (minimal compression) | q100.jpg | ✅ |
| 13 | DEC | quality_50 | Quality 50 (medium compression) | q50.jpg | ✅ |
| 14 | DEC | quality_10 | Quality 10 (heavy compression, artifacts) | q10.jpg | ✅ |
| 15 | DEC | quality_1 | Quality 1 (maximum compression) | q1.jpg | ✅ |
| 16 | DEC | size_minimal | Smallest valid JPEG (8x8 minimum) | 8x8.jpg | ❌ |
| 17 | DEC | size_odd | Odd dimensions requiring edge MCU handling | 17x17.jpg | ✅ |
| 18 | DEC | size_large | Large image (4096x4096) | large.jpg | ✅ |
| 19 | DEC | size_1x1 | Single pixel | 1x1.jpg | ✅ |
| 20 | DEC | restart_none | No restart markers | baseline_default.jpg | ✅ |
| 21 | DEC | restart_interval | Restart markers every N MCUs | restart.jpg | ✅ |
| 22 | DEC | exif_orientation | JPEG with EXIF orientation tag | exif_orientation.jpg | ✅ |
| 23 | DEC | exif_thumbnail | JPEG with embedded thumbnail | exif_thumbnail.jpg | ✅ |
| 24 | DEC | no_exif | JPEG without EXIF | no_exif.jpg | ✅ |
| 25 | DEC | trailing_data | JPEG with extra data after EOI marker | trailing_data.jpg | ✅ |
| 26 | DEC | multiple_eoi | Multiple EOI markers | multiple_eoi.jpg | ✅ |
| 27 | DEC | zero_length | Empty file (error expected) | empty.jpg | ✅ |
| 28 | DEC | truncated | Truncated JPEG (incomplete) | truncated.jpg | ✅ |
| 29 | DEC | corrupt_header | Corrupt JPEG header | corrupt.jpg | ✅ |
| 30 | DEC | baseline_standard | Baseline DCT sequential | baseline.jpg | ✅ |
| 31 | DEC | progressive_spectral | Progressive with spectral selection | progressive_spectral.jpg | ❌ |
| 32 | ENC | enc_q100 | None | quality=100 | 🔧 |
| 33 | ENC | enc_q85 | None | quality=85 | 🔧 |
| 34 | ENC | enc_q75 | None | quality=75 | 🔧 |
| 35 | ENC | enc_q50 | None | quality=50 | 🔧 |
| 36 | ENC | enc_q25 | None | quality=25 | 🔧 |
| 37 | ENC | enc_q10 | None | quality=10 | 🔧 |
| 38 | ENC | enc_q1 | None | quality=1 | 🔧 |
| 39 | ENC | enc_sub_444 | None | subsampling=444 | 🔧 |
| 40 | ENC | enc_sub_422 | None | subsampling=422 | 🔧 |
| 41 | ENC | enc_sub_420 | None | subsampling=420 | 🔧 |
| 42 | ENC | enc_progressive | None | progressive=True | 🔧 |
| 43 | ENC | enc_baseline | None | progressive=False | 🔧 |
| 44 | ENC | enc_grayscale | None | grayscale=True | 🔧 |
| 45 | ENC | enc_rgb | None | grayscale=False | 🔧 |
| 46 | ENC | enc_1x1 | None | size=[1, 1] | 🔧 |
| 47 | ENC | enc_8x8 | None | size=[8, 8] | 🔧 |
| 48 | ENC | enc_odd_size | None | size=[17, 17] | 🔧 |
| 49 | ENC | enc_exif | None | exif=True | 🔧 |
| 50 | ENC | enc_no_exif | None | exif=False | 🔧 |
| 51 | ENC | enc_restart | None | restart_interval=4 | 🔧 |
| 52 | ENC | enc_no_restart | None | restart_interval=0 | 🔧 |
| 53 | ENC | enc_dct_fast | None | dct_method=fast | 🔧 |
| 54 | ENC | enc_dct_slow | None | dct_method=slow | 🔧 |
| 55 | ENC | enc_optimize | None | optimize=True | 🔧 |
| 56 | ENC | enc_no_optimize | None | optimize=False | 🔧 |
| 57 | ENC | enc_quality_100 | Quality 100 (max) | quality=100 | 🔧 |
| 58 | ENC | enc_quality_85 | Quality 85 (default) | quality=85 | 🔧 |
| 59 | ENC | enc_quality_50 | Quality 50 | quality=50 | 🔧 |
| 60 | ENC | enc_quality_10 | Quality 10 (min) | quality=10 | 🔧 |
| 61 | ENC | enc_subsample_444 | 4:4:4 chroma | subsampling=444 | 🔧 |
| 62 | ENC | enc_subsample_420 | 4:2:0 chroma | subsampling=420 | 🔧 |
| 63 | ENC | enc_progressive | Progressive JPEG | progressive=True | 🔧 |
| 64 | ENC | enc_grayscale | Grayscale JPEG | grayscale=True | 🔧 |

## PNG (38 decode + 37 encode = 75 total)

| # | Type | ID | Variation | Asset/Params | Status |
|---|------|----|-----------|-------------|--------|
| 1 | DEC | color_gray | Grayscale (color type 0) | gray.png | ✅ |
| 2 | DEC | color_gray_alpha | Grayscale + alpha (color type 4) | gray_alpha.png | ✅ |
| 3 | DEC | color_rgb | RGB (color type 2) | rgb.png | ✅ |
| 4 | DEC | color_rgba | RGBA (color type 6) | rgba.png | ✅ |
| 5 | DEC | color_indexed | Indexed/Palette (color type 3) | indexed.png | ✅ |
| 6 | DEC | color_indexed_alpha | Indexed with tRNS transparency | indexed_alpha.png | ✅ |
| 7 | DEC | depth_1 | 1-bit grayscale | 1bit.png | ✅ |
| 8 | DEC | depth_2 | 2-bit grayscale | 2bit.png | ❌ |
| 9 | DEC | depth_4 | 4-bit grayscale | 4bit.png | ❌ |
| 10 | DEC | depth_8 | 8-bit per channel (standard) | 8bit.png | ✅ |
| 11 | DEC | depth_16 | 16-bit per channel | 16bit.png | ❌ |
| 12 | DEC | no_interlace | No interlacing (Adam7 off) | no_interlace.png | ✅ |
| 13 | DEC | interlace_adam7 | Adam7 interlacing | adam7.png | ✅ |
| 14 | DEC | filter_none | No filter | filter_none.png | ✅ |
| 15 | DEC | filter_sub | Sub filter | filter_sub.png | ✅ |
| 16 | DEC | filter_up | Up filter | filter_up.png | ✅ |
| 17 | DEC | filter_average | Average filter | filter_average.png | ✅ |
| 18 | DEC | filter_paeth | Paeth filter | filter_paeth.png | ✅ |
| 19 | DEC | filter_mixed | Adaptive per-scanline filtering | filter_mixed.png | ✅ |
| 20 | DEC | compress_default | Default zlib compression (level 6) | compress_default.png | ✅ |
| 21 | DEC | compress_none | No compression (store only) | compress_none.png | ✅ |
| 22 | DEC | compress_max | Maximum compression (level 9) | compress_max.png | ✅ |
| 23 | DEC | chunk_gama | PNG with gAMA chunk (gamma) | gama.png | ✅ |
| 24 | DEC | chunk_srgb | PNG with sRGB chunk | srgb.png | ✅ |
| 25 | DEC | chunk_iccp | PNG with iCCP color profile | iccp.png | ✅ |
| 26 | DEC | chunk_text | PNG with tEXt/zTXt/iTXt metadata | text_chunks.png | ✅ |
| 27 | DEC | chunk_time | PNG with tIME chunk | time_chunk.png | ✅ |
| 28 | DEC | chunk_background | PNG with bKGD chunk | bkgd.png | ✅ |
| 29 | DEC | chunk_phys | PNG with pHYs chunk (physical dimensions) | phys.png | ✅ |
| 30 | DEC | size_1x1 | Single pixel | 1x1.png | ✅ |
| 31 | DEC | size_small | Small image (16x16) | 16x16.png | ✅ |
| 32 | DEC | size_large | Large image (4096x4096) | large.png | ✅ |
| 33 | DEC | size_odd | Non-power-of-2 dimensions | odd_size.png | ✅ |
| 34 | DEC | error_truncated | Truncated PNG file | truncated.png | ✅ |
| 35 | DEC | error_corrupt_crc | Corrupt chunk CRC | bad_crc.png | ✅ |
| 36 | DEC | error_wrong_magic | File with wrong PNG signature | not_a_png.png | ✅ |
| 37 | DEC | apng_static | APNG with single frame (backward compatible) | apng_static.png | ✅ |
| 38 | DEC | apng_animated | APNG with multiple frames | apng_animated.png | ✅ |
| 39 | ENC | enc_compress_0 | None | compression=0 | 🔧 |
| 40 | ENC | enc_compress_3 | None | compression=3 | 🔧 |
| 41 | ENC | enc_compress_6 | None | compression=6 | 🔧 |
| 42 | ENC | enc_compress_9 | None | compression=9 | 🔧 |
| 43 | ENC | enc_l8 | None | color_type=L | 🔧 |
| 44 | ENC | enc_la8 | None | color_type=LA | 🔧 |
| 45 | ENC | enc_rgb8 | None | color_type=RGB | 🔧 |
| 46 | ENC | enc_rgba8 | None | color_type=RGBA | 🔧 |
| 47 | ENC | enc_indexed | None | color_type=P | 🔧 |
| 48 | ENC | enc_1bit | None | color_type=1 | 🔧 |
| 49 | ENC | enc_adam7 | None | interlace=True | 🔧 |
| 50 | ENC | enc_no_interlace | None | interlace=False | 🔧 |
| 51 | ENC | enc_8bit | None | bit_depth=8 | 🔧 |
| 52 | ENC | enc_16bit | None | bit_depth=16 | 🔧 |
| 53 | ENC | enc_filter_none | None | filter=none | 🔧 |
| 54 | ENC | enc_filter_sub | None | filter=sub | 🔧 |
| 55 | ENC | enc_filter_up | None | filter=up | 🔧 |
| 56 | ENC | enc_filter_avg | None | filter=average | 🔧 |
| 57 | ENC | enc_filter_paeth | None | filter=paeth | 🔧 |
| 58 | ENC | enc_filter_adaptive | None | filter=adaptive | 🔧 |
| 59 | ENC | enc_chunk_text | None | text_chunks=True | 🔧 |
| 60 | ENC | enc_chunk_gamma | None | gamma=True | 🔧 |
| 61 | ENC | enc_chunk_srgb | None | srgb=True | 🔧 |
| 62 | ENC | enc_chunk_phys | None | physical=True | 🔧 |
| 63 | ENC | enc_chunk_time | None | time=True | 🔧 |
| 64 | ENC | enc_1x1 | None | size=[1, 1] | 🔧 |
| 65 | ENC | enc_odd_size | None | size=[17, 17] | 🔧 |
| 66 | ENC | enc_compress_default | Default compression | compression=default | 🔧 |
| 67 | ENC | enc_compress_max | Max compression (9) | compression=max | 🔧 |
| 68 | ENC | enc_compress_none | No compression | compression=none | 🔧 |
| 69 | ENC | enc_interlaced | Adam7 interlaced | interlaced=True | 🔧 |
| 70 | ENC | enc_rgb | RGB output | color=rgb | 🔧 |
| 71 | ENC | enc_rgba | RGBA output | color=rgba | 🔧 |
| 72 | ENC | enc_grayscale | Grayscale output | color=gray | 🔧 |
| 73 | ENC | enc_grayscale_alpha | Grayscale+alpha output | color=gray_alpha | 🔧 |
| 74 | ENC | enc_indexed | Palette output | color=indexed | 🔧 |
| 75 | ENC | enc_1bit | 1-bit bilevel | color=1bit | 🔧 |

## GIF (9 decode + 16 encode = 25 total)

| # | Type | ID | Variation | Asset/Params | Status |
|---|------|----|-----------|-------------|--------|
| 1 | DEC | static_gif | Single frame GIF | static.gif | ✅ |
| 2 | DEC | animated_gif | Animated GIF (decode first frame) | animated.gif | ✅ |
| 3 | DEC | transparent | GIF with transparency index | transparent.gif | ✅ |
| 4 | DEC | interlaced | Interlaced GIF | interlaced.gif | ✅ |
| 5 | DEC | color_table_global | Global color table only | global_ct.gif | ✅ |
| 6 | DEC | color_table_local | Local per-frame color table | local_ct.gif | ✅ |
| 7 | DEC | extension_gce | Graphic Control Extension (delay, disposal) | gce.gif | ✅ |
| 8 | DEC | size_1x1 | Single pixel GIF | 1x1.gif | ✅ |
| 9 | DEC | error_empty | Empty file | empty.gif | ✅ |
| 10 | ENC | enc_static | None | animated=False | 🔧 |
| 11 | ENC | enc_animated | None | animated=True, frames=2 | 🔧 |
| 12 | ENC | enc_animated_loop | None | animated=True, frames=3, loop=True | 🔧 |
| 13 | ENC | enc_opaque | None | transparency=False | 🔧 |
| 14 | ENC | enc_transparent | None | transparency=True | 🔧 |
| 15 | ENC | enc_interlaced | None | interlace=True | 🔧 |
| 16 | ENC | enc_non_interlaced | None | interlace=False | 🔧 |
| 17 | ENC | enc_dispose_none | None | disposal=none | 🔧 |
| 18 | ENC | enc_dispose_background | None | disposal=background | 🔧 |
| 19 | ENC | enc_dispose_previous | None | disposal=previous | 🔧 |
| 20 | ENC | enc_global_ct | None | color_table=global | 🔧 |
| 21 | ENC | enc_local_ct | None | color_table=local | 🔧 |
| 22 | ENC | enc_1x1 | None | size=[1, 1] | 🔧 |
| 23 | ENC | enc_static | Static GIF | animated=False | 🔧 |
| 24 | ENC | enc_transparent | Transparent GIF | transparency=True | 🔧 |
| 25 | ENC | enc_interlaced | Interlaced GIF | interlaced=True | 🔧 |

## BMP (18 decode + 21 encode = 39 total)

| # | Type | ID | Variation | Asset/Params | Status |
|---|------|----|-----------|-------------|--------|
| 1 | DEC | depth_1 | 1-bit monochrome BMP | 1bit.bmp | ✅ |
| 2 | DEC | depth_4 | 4-bit indexed BMP | 4bit.bmp | ✅ |
| 3 | DEC | depth_8 | 8-bit indexed BMP | 8bit.bmp | ✅ |
| 4 | DEC | depth_16 | 16-bit (RGB555) | 16bit.bmp | ✅ |
| 5 | DEC | depth_24 | 24-bit RGB (most common) | 24bit.bmp | ✅ |
| 6 | DEC | depth_32 | 32-bit RGBA | 32bit.bmp | ✅ |
| 7 | DEC | compression_none | BI_RGB (no compression) | uncompressed.bmp | ✅ |
| 8 | DEC | compression_rle8 | BI_RLE8 (8-bit RLE) | rle8.bmp | ✅ |
| 9 | DEC | compression_rle4 | BI_RLE4 (4-bit RLE) | rle4.bmp | ✅ |
| 10 | DEC | compression_bitfields | BI_BITFIELDS (16/32 bit with masks) | bitfields.bmp | ✅ |
| 11 | DEC | top_down | Top-down scanline order (negative height) | top_down.bmp | ✅ |
| 12 | DEC | bottom_up | Bottom-up scanline order (standard) | bottom_up.bmp | ✅ |
| 13 | DEC | os2_v1 | OS/2 BMP v1 header | os2v1.bmp | ✅ |
| 14 | DEC | v4_header | BITMAPV4HEADER (color space info) | v4header.bmp | ✅ |
| 15 | DEC | v5_header | BITMAPV5HEADER (ICC profile) | v5header.bmp | ✅ |
| 16 | DEC | size_1x1 | Single pixel | 1x1.bmp | ✅ |
| 17 | DEC | size_odd | Odd width (row padding required) | odd_width.bmp | ✅ |
| 18 | DEC | error_not_bmp | File without BM magic | not_bmp.bmp | ✅ |
| 19 | ENC | enc_1bit | None | bit_depth=1 | 🔧 |
| 20 | ENC | enc_4bit | None | bit_depth=4 | 🔧 |
| 21 | ENC | enc_8bit | None | bit_depth=8 | 🔧 |
| 22 | ENC | enc_16bit | None | bit_depth=16 | 🔧 |
| 23 | ENC | enc_24bit | None | bit_depth=24 | 🔧 |
| 24 | ENC | enc_32bit | None | bit_depth=32 | 🔧 |
| 25 | ENC | enc_bi_rgb | None | compression=BI_RGB | 🔧 |
| 26 | ENC | enc_bi_rle8 | None | compression=BI_RLE8 | 🔧 |
| 27 | ENC | enc_bi_rle4 | None | compression=BI_RLE4 | 🔧 |
| 28 | ENC | enc_bi_bitfields | None | compression=BI_BITFIELDS | 🔧 |
| 29 | ENC | enc_top_down | None | top_down=True | 🔧 |
| 30 | ENC | enc_bottom_up | None | top_down=False | 🔧 |
| 31 | ENC | enc_v3_header | None | header=V3 | 🔧 |
| 32 | ENC | enc_v4_header | None | header=V4 | 🔧 |
| 33 | ENC | enc_v5_header | None | header=V5 | 🔧 |
| 34 | ENC | enc_1x1 | None | size=[1, 1] | 🔧 |
| 35 | ENC | enc_odd_width | None | size=[17, 16] | 🔧 |
| 36 | ENC | enc_24bit | 24-bit BMP | bit_depth=24 | 🔧 |
| 37 | ENC | enc_32bit | 32-bit BMP with alpha | bit_depth=32 | 🔧 |
| 38 | ENC | enc_8bit | 8-bit indexed BMP | bit_depth=8 | 🔧 |
| 39 | ENC | enc_1bit | 1-bit bilevel | bit_depth=1 | 🔧 |

## WEBP (13 decode + 24 encode = 37 total)

| # | Type | ID | Variation | Asset/Params | Status |
|---|------|----|-----------|-------------|--------|
| 1 | DEC | lossy_vp8 | Lossy WebP (VP8 codec) | lossy.webp | ✅ |
| 2 | DEC | lossless_vp8l | Lossless WebP (VP8L codec) | lossless.webp | ✅ |
| 3 | DEC | extended | Extended WebP (VP8X container) | extended.webp | ✅ |
| 4 | DEC | no_alpha | WebP without alpha | no_alpha.webp | ✅ |
| 5 | DEC | with_alpha | WebP with alpha channel | with_alpha.webp | ❌ |
| 6 | DEC | alpha_lossless | Lossless WebP with alpha | alpha_lossless.webp | ✅ |
| 7 | DEC | animated | Animated WebP (decode first frame) | animated.webp | ✅ |
| 8 | DEC | size_small | Small image (16x16) | 16x16.webp | ❌ |
| 9 | DEC | size_odd | Odd dimensions | odd.webp | ✅ |
| 10 | DEC | icc_profile | WebP with embedded ICC profile | icc.webp | ✅ |
| 11 | DEC | xmp_metadata | WebP with XMP metadata | xmp.webp | ✅ |
| 12 | DEC | exif_metadata | WebP with EXIF metadata | exif.webp | ✅ |
| 13 | DEC | error_truncated | Truncated RIFF chunk | truncated.webp | ✅ |
| 14 | ENC | enc_lossy_q100 | None | lossless=False, quality=100 | 🔧 |
| 15 | ENC | enc_lossy_q80 | None | lossless=False, quality=80 | 🔧 |
| 16 | ENC | enc_lossy_q50 | None | lossless=False, quality=50 | 🔧 |
| 17 | ENC | enc_lossy_q10 | None | lossless=False, quality=10 | 🔧 |
| 18 | ENC | enc_lossy_q1 | None | lossless=False, quality=1 | 🔧 |
| 19 | ENC | enc_lossless | None | lossless=True | 🔧 |
| 20 | ENC | enc_lossy_alpha | None | alpha=True, lossless=False | 🔧 |
| 21 | ENC | enc_lossless_alpha | None | alpha=True, lossless=True | 🔧 |
| 22 | ENC | enc_no_alpha | None | alpha=False | 🔧 |
| 23 | ENC | enc_hint_photo | None | hint=photo | 🔧 |
| 24 | ENC | enc_hint_graph | None | hint=graph | 🔧 |
| 25 | ENC | enc_hint_picture | None | hint=picture | 🔧 |
| 26 | ENC | enc_method_0 | None | method=0 | 🔧 |
| 27 | ENC | enc_method_6 | None | method=6 | 🔧 |
| 28 | ENC | enc_exif | None | exif=True | 🔧 |
| 29 | ENC | enc_xmp | None | xmp=True | 🔧 |
| 30 | ENC | enc_icc | None | icc=True | 🔧 |
| 31 | ENC | enc_1x1 | None | size=[1, 1] | 🔧 |
| 32 | ENC | enc_lossy | Lossy WebP | lossless=False | 🔧 |
| 33 | ENC | enc_lossless | Lossless WebP | lossless=True | 🔧 |
| 34 | ENC | enc_lossy_alpha | Lossy with alpha | alpha=True, lossless=False | 🔧 |
| 35 | ENC | enc_lossless_alpha | Lossless with alpha | alpha=True, lossless=True | 🔧 |
| 36 | ENC | enc_lossy_quality_100 | Lossy quality 100 | lossless=False, quality=100 | 🔧 |
| 37 | ENC | enc_lossy_quality_10 | Lossy quality 10 | lossless=False, quality=10 | 🔧 |

## TIFF (22 decode + 30 encode = 52 total)

| # | Type | ID | Variation | Asset/Params | Status |
|---|------|----|-----------|-------------|--------|
| 1 | DEC | byte_order_le | Little-endian TIFF (II) | le.tiff | ✅ |
| 2 | DEC | byte_order_be | Big-endian TIFF (MM) | be.tiff | ✅ |
| 3 | DEC | compression_none | No compression | uncompressed.tiff | ✅ |
| 4 | DEC | compression_lzw | LZW compression | lzw.tiff | ✅ |
| 5 | DEC | compression_deflate | Deflate/ZIP compression | deflate.tiff | ✅ |
| 6 | DEC | compression_packbits | PackBits compression | packbits.tiff | ✅ |
| 7 | DEC | bilevel | Black and white (PHOTOMETRIC_MINISWHITE/BLACK) | bilevel.tiff | ✅ |
| 8 | DEC | grayscale | Grayscale | gray.tiff | ✅ |
| 9 | DEC | palette | Palette/indexed color | palette.tiff | ✅ |
| 10 | DEC | rgb | RGB | rgb.tiff | ✅ |
| 11 | DEC | rgba | RGBA with extrasamples | rgba.tiff | ✅ |
| 12 | DEC | cmyk | CMYK | cmyk.tiff | ✅ |
| 13 | DEC | ycbcr | YCbCr | ycbcr.tiff | ✅ |
| 14 | DEC | depth_1 | 1-bit | 1bit.tiff | ✅ |
| 15 | DEC | depth_8 | 8-bit per channel | 8bit.tiff | ✅ |
| 16 | DEC | depth_16 | 16-bit per channel | 16bit.tiff | ✅ |
| 17 | DEC | depth_float | 32-bit float (F-mode) | float32.tiff | ✅ |
| 18 | DEC | stripped | Striped organization | stripped.tiff | ✅ |
| 19 | DEC | tiled | Tiled organization | tiled.tiff | ✅ |
| 20 | DEC | single_page | Single IFD TIFF | single.tiff | ✅ |
| 21 | DEC | multi_page | Multi-page TIFF (decode first page) | multipage.tiff | ✅ |
| 22 | DEC | error_bad_ifd | Corrupt IFD entry | bad_ifd.tiff | ✅ |
| 23 | ENC | enc_compress_none | None | compression=none | 🔧 |
| 24 | ENC | enc_compress_lzw | None | compression=lzw | 🔧 |
| 25 | ENC | enc_compress_deflate | None | compression=deflate | 🔧 |
| 26 | ENC | enc_compress_packbits | None | compression=packbits | 🔧 |
| 27 | ENC | enc_byte_le | None | byte_order=le | 🔧 |
| 28 | ENC | enc_byte_be | None | byte_order=be | 🔧 |
| 29 | ENC | enc_bilevel | None | color=1bit | 🔧 |
| 30 | ENC | enc_grayscale | None | color=gray | 🔧 |
| 31 | ENC | enc_rgb | None | color=rgb | 🔧 |
| 32 | ENC | enc_rgba | None | color=rgba | 🔧 |
| 33 | ENC | enc_cmyk | None | color=cmyk | 🔧 |
| 34 | ENC | enc_8bit | None | bit_depth=8 | 🔧 |
| 35 | ENC | enc_16bit | None | bit_depth=16 | 🔧 |
| 36 | ENC | enc_32bit | None | bit_depth=32 | 🔧 |
| 37 | ENC | enc_stripped | None | organization=stripped | 🔧 |
| 38 | ENC | enc_tiled | None | organization=tiled | 🔧 |
| 39 | ENC | enc_single_page | None | pages=1 | 🔧 |
| 40 | ENC | enc_multi_page | None | pages=2 | 🔧 |
| 41 | ENC | enc_predictor_none | None | predictor=none | 🔧 |
| 42 | ENC | enc_predictor_horiz | None | predictor=horizontal | 🔧 |
| 43 | ENC | enc_1x1 | None | size=[1, 1] | 🔧 |
| 44 | ENC | enc_uncompressed | Uncompressed | compression=none | 🔧 |
| 45 | ENC | enc_lzw | LZW compression | compression=lzw | 🔧 |
| 46 | ENC | enc_deflate | Deflate compression | compression=deflate | 🔧 |
| 47 | ENC | enc_le | Little-endian | byte_order=le | 🔧 |
| 48 | ENC | enc_be | Big-endian | byte_order=be | 🔧 |
| 49 | ENC | enc_rgb | RGB TIFF | color=rgb | 🔧 |
| 50 | ENC | enc_rgba | RGBA TIFF | color=rgba | 🔧 |
| 51 | ENC | enc_grayscale | Grayscale TIFF | color=gray | 🔧 |
| 52 | ENC | enc_bilevel | Bilevel TIFF | color=1bit | 🔧 |

## ICO (6 decode + 16 encode = 22 total)

| # | Type | ID | Variation | Asset/Params | Status |
|---|------|----|-----------|-------------|--------|
| 1 | DEC | single_icon | Single icon entry | single.ico | ✅ |
| 2 | DEC | multi_res | Multiple resolution entries | multi.ico | ✅ |
| 3 | DEC | png_entry | ICO with embedded PNG data | png_entry.ico | ✅ |
| 4 | DEC | bmp_entry | ICO with embedded BMP data (classic) | bmp_entry.ico | ✅ |
| 5 | DEC | size_16x16 | 16x16 icon (most common) | 16x16.ico | ✅ |
| 6 | DEC | size_256x256 | 256x256 icon (modern) | 256x256.ico | ✅ |
| 7 | ENC | enc_16x16 | None | sizes=['(16', '16)'] | 🔧 |
| 8 | ENC | enc_32x32 | None | sizes=['(32', '32)'] | 🔧 |
| 9 | ENC | enc_48x48 | None | sizes=['(48', '48)'] | 🔧 |
| 10 | ENC | enc_256x256 | None | sizes=['(256', '256)'] | 🔧 |
| 11 | ENC | enc_multi_2 | None | sizes=['(16', '16)', '(32', '32)'] | 🔧 |
| 12 | ENC | enc_multi_4 | None | sizes=['(16', '16)', '(32', '32)', '(48', '48)', '(256', '256)'] | 🔧 |
| 13 | ENC | enc_bmp_entry | None | entry_type=bmp | 🔧 |
| 14 | ENC | enc_png_entry | None | entry_type=png | 🔧 |
| 15 | ENC | enc_24bit | None | bit_depth=24 | 🔧 |
| 16 | ENC | enc_32bit | None | bit_depth=32 | 🔧 |
| 17 | ENC | enc_no_hotspot | None | hotspot=False | 🔧 |
| 18 | ENC | enc_hotspot | None | hotspot=[8, 8] | 🔧 |
| 19 | ENC | enc_16x16 | 16x16 icon | sizes=[[16, 16]] | 🔧 |
| 20 | ENC | enc_32x32 | 32x32 icon | sizes=[[32, 32]] | 🔧 |
| 21 | ENC | enc_multi | Multi-resolution | sizes=[[16, 16], [32, 32]] | 🔧 |
| 22 | ENC | enc_png_entry | PNG-encoded entry | png_entry=True | 🔧 |

## AVIF (6 decode + 0 encode = 6 total)

| # | Type | ID | Variation | Asset/Params | Status |
|---|------|----|-----------|-------------|--------|
| 1 | DEC | baseline | Baseline AVIF (8-bit, 4:2:0) | baseline.avif | ✅ |
| 2 | DEC | high_bitdepth | 10-bit AVIF | 10bit.avif | ✅ |
| 3 | DEC | with_alpha | AVIF with alpha channel | alpha.avif | ✅ |
| 4 | DEC | hdr | HDR AVIF (PQ/HLG transfer) | hdr.avif | ✅ |
| 5 | DEC | grid | Grid AVIF (tiled image) | grid.avif | ✅ |
| 6 | DEC | animated | Animated AVIF (decode first frame) | animated.avif | ✅ |

**Total: 143 decode + 177 encode = 320 rows**
**Decode: 66/74 pass, 8 fail | Encode: 0/177 wired | Assets: 83 files committed**
