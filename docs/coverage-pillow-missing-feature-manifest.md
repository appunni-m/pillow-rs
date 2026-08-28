# Pillow reverse-coverage gap manifest

This is generated evidence from the same public parity corpus. It is not a new test denominator.

- Python lines: 6803/16019 (42.47%)
- Python branches: 1570/5694 (27.57%)
- Source files with gaps: 92/97
- Active public operations with mapped gaps: 55

## Ordered public feature gaps

The JSON contains all active public operations in `feature_manifest`. The table below shows only operations whose mapped Pillow function/class still has missing lines or branches in this snapshot; inspect the listed case IDs before adding inputs.

| Rank | Public operation | Cases | Missing lines | Missing branches | Priority |
| ---: | --- | ---: | ---: | ---: | ---: |
| 1 | `PIL.Image.Image.convert` | 785 | 50 | 26 | 5260 |
| 2 | `PIL.ImageFont.truetype` | 15 | 20 | 17 | 2170 |
| 3 | `PIL.ImageFont.ImageFont` | 1 | 16 | 8 | 1680 |
| 4 | `PIL.ImageFont.FreeTypeFont` | 5 | 15 | 8 | 1580 |
| 5 | `PIL.Image.Image.save` | 41 | 12 | 9 | 1290 |
| 6 | `PIL.Image.fromarray` | 125 | 12 | 8 | 1280 |
| 7 | `PIL.ImageFilter.Color3DLUT` | 20 | 11 | 7 | 1170 |
| 8 | `PIL.ImageOps.exif_transpose` | 133 | 9 | 9 | 990 |
| 9 | `PIL.ImagePalette.ImagePalette` | 3 | 9 | 9 | 990 |
| 10 | `PIL.Image.open` | 37 | 9 | 6 | 960 |
| 11 | `PIL.ImageFont.load_path` | 1 | 7 | 4 | 740 |
| 12 | `PIL.ImageDraw.ImageDraw.rounded_rectangle` | 34 | 5 | 19 | 690 |
| 13 | `PIL.ImageSequence.Iterator` | 4 | 6 | 0 | 600 |
| 14 | `PIL.Image.Image.getexif` | 20 | 5 | 6 | 560 |
| 15 | `PIL.Image.Image.remap_palette` | 14 | 5 | 4 | 540 |
| 16 | `PIL.ImageColor.getrgb` | 26 | 5 | 3 | 530 |
| 17 | `PIL.Image.Image.putalpha` | 37 | 5 | 1 | 510 |
| 18 | `PIL.Image.Image.getxmp` | 2 | 4 | 3 | 430 |
| 19 | `PIL.Image.Image.load` | 31 | 4 | 3 | 430 |
| 20 | `PIL.Image.Image.resize` | 225 | 4 | 2 | 420 |
| 21 | `PIL.Image.Image.close` | 6 | 3 | 3 | 330 |
| 22 | `PIL.Image.Image.frombytes` | 19 | 3 | 2 | 320 |
| 23 | `PIL.Image.Image.tobytes` | 19 | 3 | 2 | 320 |
| 24 | `PIL.ImageOps.autocontrast` | 167 | 2 | 4 | 240 |
| 25 | `PIL.ImagePalette.ImagePalette.save` | 1 | 2 | 3 | 230 |
| 26 | `PIL.Image.Image.putpalette` | 12 | 2 | 2 | 220 |
| 27 | `PIL.Image.Image.transform` | 434 | 2 | 2 | 220 |
| 28 | `PIL.ImageDraw.ImageDraw.text` | 65 | 2 | 2 | 220 |
| 29 | `PIL.ImageOps.fit` | 89 | 2 | 2 | 220 |
| 30 | `PIL.ImagePalette.ImagePalette.getcolor` | 14 | 2 | 2 | 220 |
| 31 | `PIL.Image.Image.has_transparency_data` | 4 | 2 | 1 | 210 |
| 32 | `PIL.Image.Image.putpixel` | 162 | 2 | 1 | 210 |
| 33 | `PIL.ImagePalette.ImagePalette.tobytes` | 1 | 2 | 1 | 210 |
| 34 | `PIL.ImageDraw.ImageDraw.bitmap` | 78 | 1 | 2 | 120 |
| 35 | `PIL.ImageFont.FreeTypeFont.set_variation_by_name` | 4 | 1 | 2 | 120 |
| 36 | `PIL.Image.Image.entropy` | 19 | 1 | 1 | 110 |
| 37 | `PIL.Image.Image.point` | 327 | 1 | 1 | 110 |
| 38 | `PIL.Image.Image.toqimage` | 1 | 1 | 1 | 110 |
| 39 | `PIL.Image.Image.toqpixmap` | 1 | 1 | 1 | 110 |
| 40 | `PIL.Image.frombuffer` | 3 | 1 | 1 | 110 |
| 41 | `PIL.ImageFilter.Color3DLUT.__repr__` | 1 | 1 | 1 | 110 |
| 42 | `PIL.ImageFont.load_default` | 2 | 1 | 1 | 110 |
| 43 | `PIL.ImageOps.expand` | 66 | 1 | 1 | 110 |
| 44 | `PIL.ImageFont.ImageFont.getbbox` | 3 | 1 | 0 | 100 |
| 45 | `PIL.ImageFont.ImageFont.getlength` | 3 | 1 | 0 | 100 |
| 46 | `PIL.Image.Image.thumbnail` | 175 | 0 | 1 | 10 |
| 47 | `PIL.ImageDraw.ImageDraw.arc` | 30 | 0 | 1 | 10 |
| 48 | `PIL.ImageDraw.ImageDraw.getfont` | 1 | 0 | 1 | 10 |
| 49 | `PIL.ImageDraw.ImageDraw.line` | 75 | 0 | 1 | 10 |
| 50 | `PIL.ImageDraw.ImageDraw.point` | 44 | 0 | 1 | 10 |
| 51 | `PIL.ImageDraw.ImageDraw.polygon` | 64 | 0 | 1 | 10 |
| 52 | `PIL.ImageFont.FreeTypeFont.get_variation_axes` | 5 | 0 | 1 | 10 |
| 53 | `PIL.ImageFont.FreeTypeFont.get_variation_names` | 6 | 0 | 1 | 10 |
| 54 | `PIL.ImageOps.pad` | 280 | 0 | 1 | 10 |
| 55 | `PIL.ImagePalette.ImagePalette.copy` | 1 | 0 | 1 | 10 |

## Ordered source gaps

| Rank | Pillow source | Missing lines | Missing branches | Priority | Classification |
| ---: | --- | ---: | ---: | ---: | --- |
| 1 | `PIL/PngImagePlugin.py` | 620 | 290 | 64900 | codec_or_support_module_outside_active_surface |
| 2 | `PIL/TiffImagePlugin.py` | 604 | 319 | 63590 | codec_or_support_module_outside_active_surface |
| 3 | `PIL/PdfParser.py` | 548 | 236 | 57160 | pillow_support_module_outside_active_surface |
| 4 | `PIL/Image.py` | 419 | 225 | 44150 | active_public_module |
| 5 | `PIL/GifImagePlugin.py` | 386 | 250 | 41100 | codec_or_support_module_outside_active_surface |
| 6 | `PIL/ImageCms.py` | 289 | 80 | 29700 | pillow_support_module_outside_active_surface |
| 7 | `PIL/BlpImagePlugin.py` | 270 | 120 | 28200 | codec_or_support_module_outside_active_surface |
| 8 | `PIL/Jpeg2KImagePlugin.py` | 249 | 114 | 26040 | codec_or_support_module_outside_active_surface |
| 9 | `PIL/JpegImagePlugin.py` | 247 | 106 | 25760 | codec_or_support_module_outside_active_surface |
| 10 | `PIL/ImageFile.py` | 235 | 113 | 24630 | codec_or_support_module_outside_active_surface |
| 11 | `PIL/EpsImagePlugin.py` | 226 | 98 | 23580 | codec_or_support_module_outside_active_surface |
| 12 | `PIL/DdsImagePlugin.py` | 203 | 98 | 21280 | codec_or_support_module_outside_active_surface |
| 13 | `PIL/PpmImagePlugin.py` | 201 | 110 | 21200 | codec_or_support_module_outside_active_surface |
| 14 | `PIL/IcnsImagePlugin.py` | 187 | 76 | 19460 | codec_or_support_module_outside_active_surface |
| 15 | `PIL/ImageShow.py` | 172 | 58 | 17780 | pillow_support_module_outside_active_surface |
| 16 | `PIL/PsdImagePlugin.py` | 167 | 82 | 17520 | codec_or_support_module_outside_active_surface |
| 17 | `PIL/ImageGrab.py` | 143 | 72 | 15020 | pillow_support_module_outside_active_surface |
| 18 | `PIL/SpiderImagePlugin.py` | 142 | 54 | 14740 | codec_or_support_module_outside_active_surface |
| 19 | `PIL/AvifImagePlugin.py` | 138 | 72 | 14520 | codec_or_support_module_outside_active_surface |
| 20 | `PIL/features.py` | 135 | 77 | 14270 | codec_or_support_module_outside_active_surface |
| 21 | `PIL/ImageText.py` | 131 | 73 | 13830 | pillow_support_module_outside_active_surface |
| 22 | `PIL/IcoImagePlugin.py` | 132 | 50 | 13700 | codec_or_support_module_outside_active_surface |
| 23 | `PIL/BmpImagePlugin.py` | 128 | 75 | 13550 | codec_or_support_module_outside_active_surface |
| 24 | `PIL/PcfFontFile.py` | 130 | 30 | 13300 | pillow_support_module_outside_active_surface |
| 25 | `PIL/PdfImagePlugin.py` | 122 | 54 | 12740 | codec_or_support_module_outside_active_surface |
| 26 | `PIL/ImImagePlugin.py` | 118 | 68 | 12480 | codec_or_support_module_outside_active_surface |
| 27 | `PIL/QoiImagePlugin.py` | 115 | 50 | 12000 | codec_or_support_module_outside_active_surface |
| 28 | `PIL/ImageMorph.py` | 115 | 46 | 11960 | pillow_support_module_outside_active_surface |
| 29 | `PIL/ImageMath.py` | 112 | 42 | 11620 | pillow_support_module_outside_active_surface |
| 30 | `PIL/ImageTk.py` | 105 | 26 | 10760 | pillow_support_module_outside_active_surface |
| 31 | `PIL/FpxImagePlugin.py` | 104 | 32 | 10720 | codec_or_support_module_outside_active_surface |
| 32 | `PIL/IptcImagePlugin.py` | 99 | 48 | 10380 | codec_or_support_module_outside_active_surface |
| 33 | `PIL/WebPImagePlugin.py` | 93 | 50 | 9800 | codec_or_support_module_outside_active_surface |
| 34 | `PIL/TgaImagePlugin.py` | 92 | 57 | 9770 | codec_or_support_module_outside_active_surface |
| 35 | `PIL/ImageWin.py` | 94 | 22 | 9620 | pillow_support_module_outside_active_surface |
| 36 | `PIL/MpoImagePlugin.py` | 89 | 32 | 9220 | codec_or_support_module_outside_active_surface |
| 37 | `PIL/SgiImagePlugin.py` | 87 | 28 | 8980 | codec_or_support_module_outside_active_surface |
| 38 | `PIL/FontFile.py` | 85 | 32 | 8820 | codec_or_support_module_outside_active_surface |
| 39 | `PIL/ImageQt.py` | 84 | 32 | 8720 | pillow_support_module_outside_active_surface |
| 40 | `PIL/FliImagePlugin.py` | 82 | 36 | 8560 | codec_or_support_module_outside_active_surface |
| 41 | `PIL/ContainerIO.py` | 83 | 18 | 8480 | codec_or_support_module_outside_active_surface |
| 42 | `PIL/ImageDraw2.py` | 82 | 22 | 8420 | pillow_support_module_outside_active_surface |
| 43 | `PIL/FitsImagePlugin.py` | 80 | 40 | 8400 | codec_or_support_module_outside_active_surface |
| 44 | `PIL/PcxImagePlugin.py` | 72 | 30 | 7500 | codec_or_support_module_outside_active_surface |
| 45 | `PIL/ImageDraw.py` | 69 | 55 | 7450 | active_public_module |
| 46 | `PIL/XpmImagePlugin.py` | 66 | 30 | 6900 | codec_or_support_module_outside_active_surface |
| 47 | `PIL/MspImagePlugin.py` | 66 | 24 | 6840 | codec_or_support_module_outside_active_surface |
| 48 | `PIL/ImageFont.py` | 61 | 38 | 6480 | active_public_module |
| 49 | `PIL/PSDraw.py` | 62 | 16 | 6360 | pillow_support_module_outside_active_surface |
| 50 | `PIL/BdfFontFile.py` | 57 | 20 | 5900 | pillow_support_module_outside_active_surface |
| 51 | `PIL/WmfImagePlugin.py` | 57 | 17 | 5870 | codec_or_support_module_outside_active_surface |
| 52 | `PIL/PalmImagePlugin.py` | 56 | 20 | 5800 | codec_or_support_module_outside_active_surface |
| 53 | `PIL/GimpGradientFile.py` | 53 | 20 | 5500 | pillow_support_module_outside_active_surface |
| 54 | `PIL/SunImagePlugin.py` | 46 | 28 | 4880 | codec_or_support_module_outside_active_surface |
| 55 | `PIL/MicImagePlugin.py` | 42 | 4 | 4240 | codec_or_support_module_outside_active_surface |
| 56 | `PIL/GbrImagePlugin.py` | 38 | 16 | 3960 | codec_or_support_module_outside_active_surface |
| 57 | `PIL/ImagePalette.py` | 37 | 15 | 3850 | active_public_module |
| 58 | `PIL/GdImageFile.py` | 33 | 6 | 3360 | codec_or_support_module_outside_active_surface |
| 59 | `PIL/GimpPaletteFile.py` | 30 | 14 | 3140 | pillow_support_module_outside_active_surface |
| 60 | `PIL/WalImageFile.py` | 29 | 4 | 2940 | codec_or_support_module_outside_active_surface |
| 61 | `PIL/DcxImagePlugin.py` | 25 | 10 | 2600 | codec_or_support_module_outside_active_surface |
| 62 | `PIL/TarIO.py` | 25 | 8 | 2580 | pillow_support_module_outside_active_surface |
| 63 | `PIL/XbmImagePlugin.py` | 24 | 8 | 2480 | codec_or_support_module_outside_active_surface |
| 64 | `PIL/FtexImagePlugin.py` | 23 | 6 | 2360 | codec_or_support_module_outside_active_surface |
| 65 | `PIL/ImageTransform.py` | 23 | 0 | 2300 | pillow_support_module_outside_active_surface |
| 66 | `PIL/MpegImagePlugin.py` | 22 | 6 | 2260 | codec_or_support_module_outside_active_surface |
| 67 | `PIL/McIdasImagePlugin.py` | 21 | 8 | 2180 | codec_or_support_module_outside_active_surface |
| 68 | `PIL/PaletteFile.py` | 20 | 8 | 2080 | pillow_support_module_outside_active_surface |
| 69 | `PIL/ImageOps.py` | 18 | 22 | 2020 | active_public_module |
| 70 | `PIL/CurImagePlugin.py` | 19 | 10 | 2000 | codec_or_support_module_outside_active_surface |
| 71 | `PIL/XVThumbImagePlugin.py` | 17 | 6 | 1760 | codec_or_support_module_outside_active_surface |
| 72 | `PIL/ImtImagePlugin.py` | 16 | 13 | 1730 | codec_or_support_module_outside_active_surface |
| 73 | `PIL/PcdImagePlugin.py` | 14 | 9 | 1490 | codec_or_support_module_outside_active_surface |
| 74 | `PIL/ImageSequence.py` | 14 | 4 | 1440 | active_public_module |
| 75 | `PIL/BufrStubImagePlugin.py` | 13 | 4 | 1340 | codec_or_support_module_outside_active_surface |
| 76 | `PIL/GribStubImagePlugin.py` | 13 | 4 | 1340 | codec_or_support_module_outside_active_surface |
| 77 | `PIL/Hdf5StubImagePlugin.py` | 13 | 4 | 1340 | codec_or_support_module_outside_active_surface |
| 78 | `PIL/ImageFilter.py` | 12 | 7 | 1270 | active_public_module |
| 79 | `PIL/PixarImagePlugin.py` | 11 | 4 | 1140 | codec_or_support_module_outside_active_surface |
| 80 | `PIL/_tkinter_finder.py` | 11 | 2 | 1120 | pillow_support_module_outside_active_surface |
| 81 | `PIL/_deprecate.py` | 8 | 5 | 850 | pillow_support_module_outside_active_surface |
| 82 | `PIL/__init__.py` | 7 | 0 | 700 | pillow_support_module_outside_active_surface |
| 83 | `PIL/_binary.py` | 6 | 0 | 600 | pillow_support_module_outside_active_surface |
| 84 | `PIL/ImageColor.py` | 5 | 3 | 530 | active_public_module |
| 85 | `PIL/__main__.py` | 4 | 0 | 400 | pillow_support_module_outside_active_surface |
| 86 | `PIL/ImagePath.py` | 3 | 0 | 300 | pillow_support_module_outside_active_surface |
| 87 | `PIL/report.py` | 3 | 0 | 300 | pillow_support_module_outside_active_surface |
| 88 | `PIL/_typing.py` | 2 | 2 | 220 | pillow_support_module_outside_active_surface |
| 89 | `PIL/TiffTags.py` | 2 | 1 | 210 | pillow_support_module_outside_active_surface |
| 90 | `PIL/_util.py` | 2 | 0 | 200 | pillow_support_module_outside_active_surface |
| 91 | `PIL/_version.py` | 2 | 0 | 200 | pillow_support_module_outside_active_surface |
| 92 | `PIL/ImageMode.py` | 1 | 0 | 100 | pillow_support_module_outside_active_surface |
| 93 | `PIL/ExifTags.py` | 0 | 0 | 0 | pillow_support_module_outside_active_surface |
| 94 | `PIL/ImageChops.py` | 0 | 0 | 0 | active_public_module |
| 95 | `PIL/ImageEnhance.py` | 0 | 0 | 0 | active_public_module |
| 96 | `PIL/ImageStat.py` | 0 | 0 | 0 | active_public_module |
| 97 | `PIL/JpegPresets.py` | 0 | 0 | 0 | pillow_support_module_outside_active_surface |

## Reading the manifest

For active public modules, inspect `public_operations_with_gaps`, each operation's `case_ids`, and `missing_symbols` before adding an input. Codec/support modules without active public operations are intentionally listed as unmapped rather than silently treated as missing parity APIs.

WGSL shader execution is reported by the all-backends artifact separately. This Pillow manifest measures only coverage.py's Python source files; Pillow's native extension is not part of these totals.
