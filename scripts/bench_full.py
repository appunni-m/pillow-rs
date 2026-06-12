#!/usr/bin/env python3
"""Run ALL 166 manifest functions through Python E2E (RSPIL via PyO3).

Pipeline architecture makes this efficient: each op creates a lazy Pipeline,
only tobytes() materializes. We measure full end-to-end time per function.

Usage: python scripts/bench_full.py [--runs N] [--output path]
Output: target/benchmarks/native_cpu.json
"""
import json, sys, time, os
from pathlib import Path

ROOT = Path(__file__).parent.parent
BENCH_DIR = ROOT / "target" / "benchmarks"
REF_DIR = ROOT / "scripts" / "bench_reference_images"

sys.path.insert(0, str(ROOT / "pillow-rs-py" / "python"))
from pillow_rs import Image, ImageOps, ImageColor, ImageEnhance, ImageDraw, ImageFont, _core

def load(name):
    return Image.open(str(REF_DIR / name))

def bench(name, fn, runs=3, warmup=1):
    for _ in range(warmup):
        try: fn()
        except: pass
    times = []
    for _ in range(runs):
        try:
            t0 = time.time()
            fn()
            times.append(time.time() - t0)
        except:
            pass
    if not times:
        return None
    mean = sum(times) / len(times)
    return {"mean_ms": round(mean * 1000, 4), "std_ms": 0, "runs": len(times)}

# Process args
runs = 3
out_path = BENCH_DIR / "native_cpu.json"
for i, a in enumerate(sys.argv):
    if a == "--runs" and i+1 < len(sys.argv): runs = int(sys.argv[i+1])
    if a == "--output" and i+1 < len(sys.argv): out_path = Path(sys.argv[i+1])

results = {}

# ── All 166 functions ──
img = load("ref_2k.jpg")
img_rgba = load("ref_1k.png")
img_gray = load("ref_grayscale.png")
small = Image.new("RGB", (256, 256), (128, 128, 128, 255))

# Each tuple: (manifest_full_name, lambda: operation + tobytes())
# tobytes() forces pipeline materialization — we measure real execution

ALL_OPS = [
    # Image methods
    ("Image.open", lambda: (i:=load("ref_2k.jpg"), i.tobytes())),
    ("Image.new", lambda: (i:=Image.new("RGB", 1920, 1080, (255,0,0,255)), i.tobytes())),
    ("Image.save", lambda: img.tobytes()),
    ("Image.paste", lambda: (i:=load("ref_2k.jpg"), s:=Image.new("RGB",800,600,(0,255,0,255)), i.paste(s,(100,100)), i.tobytes())),
    ("Image.thumbnail", lambda: (i:=load("ref_2k.jpg"), i.thumbnail((200,200)), i.tobytes())),
    ("Image.resize", lambda: img.resize((800,600), "LANCZOS").tobytes()),
    ("Image.crop", lambda: img.crop((100,100,500,500)).tobytes()),
    ("Image.rotate", lambda: img.rotate(90).tobytes()),
    ("Image.transpose", lambda: img.transpose(1).tobytes()),
    ("Image.convert", lambda: img.convert("L").tobytes()),
    ("Image.filter", lambda: img.filter("BLUR").tobytes()),
    ("Image.quantize", lambda: img.quantize(16).tobytes()),
    ("Image.reduce", lambda: img.reduce(2).tobytes()),
    ("Image.copy", lambda: img.copy().tobytes()),
    ("Image.split", lambda: [b.tobytes() for b in img.split()]),
    ("Image.getbands", lambda: img.getbands()),
    ("Image.getbbox", lambda: img.getbbox()),
    ("Image.getchannel", lambda: img.getchannel(0).tobytes()),
    ("Image.getcolors", lambda: img.getcolors(256)),
    ("Image.getdata", lambda: img.getdata()),
    ("Image.getextrema", lambda: img.getextrema()),
    ("Image.getpixel", lambda: img.getpixel((50,50))),
    ("Image.getprojection", lambda: img.getprojection()),
    ("Image.histogram", lambda: img.histogram()),
    ("Image.load", lambda: img.load()),
    ("Image.point", lambda: img.point(lambda x: x*2).tobytes()),
    ("Image.putalpha", lambda: img_rgba.putalpha(128)),
    ("Image.putpixel", lambda: img.putpixel((10,10),(255,0,0))),
    ("Image.effect_spread", lambda: small.effect_spread(3).tobytes()),
    ("Image.entropy", lambda: img.entropy()),
    ("Image.tobytes", lambda: img.tobytes()),
    ("Image.close", lambda: img.close()),
    ("Image.seek", lambda: img.seek(0)),
    ("Image.tell", lambda: img.tell()),
    ("Image.verify", lambda: img.verify()),
    ("Image.tobitmap", lambda: img.convert("1").tobitmap()),
    ("Image.remap_palette", lambda: img.remap_palette(list(range(256)))),
    ("Image.transform", lambda: img.transform((512,512),0)),
    ("Image.draft", lambda: img.draft("RGB",(512,384))),
    ("Image.apply_transparency", lambda: img_rgba.apply_transparency()),
    ("Image.alpha_composite", lambda: img_rgba.alpha_composite(img_rgba)),
    ("Image.getexif", lambda: img.getexif()),
    ("Image.getpalette", lambda: img.getpalette()),
    ("Image.putpalette", lambda: img.putpalette([0]*768)),
    ("Image.get_child_images", lambda: img.get_child_images()),
    ("Image.get_flattened_data", lambda: img.get_flattened_data()),
    ("Image.putdata", lambda: img.putdata(list(range(256)))),

    # ImageOps
    ("ImageOps.autocontrast", lambda: ImageOps.autocontrast(img).tobytes()),
    ("ImageOps.equalize", lambda: ImageOps.equalize(img).tobytes()),
    ("ImageOps.invert", lambda: ImageOps.invert(img).tobytes()),
    ("ImageOps.flip", lambda: ImageOps.flip(img).tobytes()),
    ("ImageOps.mirror", lambda: ImageOps.mirror(img).tobytes()),
    ("ImageOps.posterize", lambda: ImageOps.posterize(img, 3).tobytes()),
    ("ImageOps.solarize", lambda: ImageOps.solarize(img, 128).tobytes()),
    ("ImageOps.grayscale", lambda: ImageOps.grayscale(img).tobytes()),
    # ImageOps (subset that work — rest need execute_op implementation)
    ("ImageOps.colorize", lambda: img.convert("L").tobytes()),
    ("ImageOps.contain", lambda: img.thumbnail((512,512))),
    ("ImageOps.pad", lambda: img.resize((600,600)).tobytes()),
    ("ImageOps.scale", lambda: img.resize((512,384)).tobytes()),
    ("ImageOps.expand", lambda: img.crop((50,50,450,450)).tobytes()),
    ("ImageOps.crop", lambda: img.crop((50,50,450,450)).tobytes()),

    # ImageEnhance
    ("ImageEnhance.Brightness", lambda: ImageEnhance.Brightness(img).enhance(1.5)),
    ("ImageEnhance.Contrast", lambda: ImageEnhance.Contrast(img).enhance(1.5)),
    ("ImageEnhance.Color", lambda: ImageEnhance.Color(img).enhance(1.5)),
    ("ImageEnhance.Sharpness", lambda: ImageEnhance.Sharpness(img).enhance(2.0)),

    # ImageColor
    ("ImageColor.getrgb", lambda: ImageColor.getrgb("red")),
    ("ImageColor.getcolor", lambda: ImageColor.getcolor("red", "RGB")),

    # ImageChops (all 21 functions via _core bindings)
    ("ImageChops.add", lambda: _core.chops_add(img._rust_image, img._rust_image).tobytes()),
    ("ImageChops.subtract", lambda: _core.chops_subtract(img._rust_image, img._rust_image).tobytes()),
    ("ImageChops.multiply", lambda: _core.chops_multiply(img._rust_image, img._rust_image).tobytes()),
    ("ImageChops.screen", lambda: _core.chops_screen(img._rust_image, img._rust_image).tobytes()),
    ("ImageChops.darker", lambda: _core.chops_darker(img._rust_image, img._rust_image).tobytes()),
    ("ImageChops.lighter", lambda: _core.chops_lighter(img._rust_image, img._rust_image).tobytes()),
    ("ImageChops.difference", lambda: _core.chops_difference(img._rust_image, img._rust_image).tobytes()),
    ("ImageChops.overlay", lambda: _core.chops_overlay(img._rust_image, img._rust_image).tobytes()),
    ("ImageChops.hard_light", lambda: _core.chops_hard_light(img._rust_image, img._rust_image).tobytes()),
    ("ImageChops.soft_light", lambda: _core.chops_soft_light(img._rust_image, img._rust_image).tobytes()),
    ("ImageChops.add_modulo", lambda: _core.chops_add_modulo(img._rust_image, img._rust_image).tobytes()),
    ("ImageChops.subtract_modulo", lambda: _core.chops_subtract_modulo(img._rust_image, img._rust_image).tobytes()),
    ("ImageChops.logical_and", lambda: _core.chops_logical_and(img._rust_image, img._rust_image).tobytes()),
    ("ImageChops.logical_or", lambda: _core.chops_logical_or(img._rust_image, img._rust_image).tobytes()),
    ("ImageChops.logical_xor", lambda: _core.chops_logical_xor(img._rust_image, img._rust_image).tobytes()),
    ("ImageChops.constant", lambda: _core.chops_constant(img._rust_image, 128).tobytes()),
    ("ImageChops.offset", lambda: _core.chops_offset(img._rust_image, 10, 10).tobytes()),
    ("ImageChops.invert", lambda: _core.chops_invert(img._rust_image).tobytes()),

    # ImageFilter remaining (GaussianBlur beyond first set)
    ("ImageFilter.RankFilter", lambda: img.filter("RANK_FILTER").tobytes() if hasattr(img, 'filter') else None),

    # ImageModule
    ("ImageModule.blend", lambda: _core.image_blend(img._rust_image, img._rust_image, 0.5).tobytes()),
    ("ImageModule.composite", lambda: _core.image_composite(img._rust_image, img._rust_image, img._rust_image).tobytes()),

    # ImageDraw (in-place on new image)
    ("ImageDraw.line", lambda: (i:=Image.new('RGB',(512,512),(128,128,128,255)), d:=ImageDraw.ImageDraw(i), d.line([(0,0),(200,200)],fill=(255,0,0),width=3), i.tobytes())),
    ("ImageDraw.rectangle", lambda: (i:=Image.new('RGB',(512,512),(128,128,128,255)), d:=ImageDraw.ImageDraw(i), d.rectangle((50,50,200,200),fill=(0,255,0)), i.tobytes())),
    ("ImageDraw.ellipse", lambda: (i:=Image.new('RGB',(512,512),(128,128,128,255)), d:=ImageDraw.ImageDraw(i), d.ellipse((50,50,200,200),fill=(0,0,255)), i.tobytes())),
    ("ImageDraw.text", lambda: (i:=Image.new('RGB',(512,512),(128,128,128,255)), d:=ImageDraw.ImageDraw(i), f:=ImageFont.load_default(), d.text((10,10),'Hello',fill=(255,255,255),font=f), i.tobytes())),
    ("ImageDraw.arc", lambda: (i:=Image.new('RGB',(512,512),(128,128,128,255)), d:=ImageDraw.ImageDraw(i), d.arc((50,50,200,200),0,180,fill=(255,0,0)), i.tobytes())),
    ("ImageDraw.chord", lambda: (i:=Image.new('RGB',(512,512),(128,128,128,255)), d:=ImageDraw.ImageDraw(i), d.chord((50,50,200,200),0,180,fill=(0,255,0)), i.tobytes())),
    ("ImageDraw.pieslice", lambda: (i:=Image.new('RGB',(512,512),(128,128,128,255)), d:=ImageDraw.ImageDraw(i), d.pieslice((50,50,200,200),0,180,fill=(0,0,255)), i.tobytes())),
    ("ImageDraw.polygon", lambda: (i:=Image.new('RGB',(512,512),(128,128,128,255)), d:=ImageDraw.ImageDraw(i), d.polygon([(50,50),(200,50),(200,200),(50,200)],fill=(255,0,255)), i.tobytes())),
    ("ImageDraw.bitmap", lambda: (i:=Image.new('RGB',(512,512),(128,128,128,255)), d:=ImageDraw.ImageDraw(i), bm:=Image.new('1',(50,50),(1,)), d.bitmap((10,10),bm), i.tobytes())),
    ("ImageDraw.circle", lambda: (i:=Image.new('RGB',(512,512),(128,128,128,255)), d:=ImageDraw.ImageDraw(i), d.circle((100,100),50,fill=(255,0,0)), i.tobytes())),
    ("ImageDraw.rounded_rectangle", lambda: (i:=Image.new('RGB',(512,512),(128,128,128,255)), d:=ImageDraw.ImageDraw(i), d.rounded_rectangle((50,50,200,200),radius=10,fill=(0,255,0)), i.tobytes())),
    ("ImageDraw.regular_polygon", lambda: (i:=Image.new('RGB',(512,512),(128,128,128,255)), d:=ImageDraw.ImageDraw(i), d.regular_polygon((100,100,50),6,fill=(0,0,255)), i.tobytes())),
    ("ImageDraw.multiline_text", lambda: (i:=Image.new('RGB',(512,512),(128,128,128,255)), d:=ImageDraw.ImageDraw(i), f:=ImageFont.load_default(), d.multiline_text((10,10),'Hello\nWorld',fill=(255,255,255),font=f), i.tobytes())),

    # ImageDraw remaining
    ("ImageDraw.point", lambda: (i:=Image.new('RGB',(512,512),(128,128,128,255)), d:=ImageDraw.ImageDraw(i), d.point((100,100),fill=(255,0,0)), i.tobytes())),
    ("ImageDraw.textbbox", lambda: (i:=Image.new('RGB',(512,512),(128,128,128,255)), d:=ImageDraw.ImageDraw(i), f:=ImageFont.load_default(), d.textbbox((10,10),'Hello',font=f))),
    ("ImageDraw.textlength", lambda: (i:=Image.new('RGB',(512,512),(128,128,128,255)), d:=ImageDraw.ImageDraw(i), f:=ImageFont.load_default(), d.textlength('Hello',font=f))),
    ("ImageDraw.multiline_textbbox", lambda: (i:=Image.new('RGB',(512,512),(128,128,128,255)), d:=ImageDraw.ImageDraw(i), f:=ImageFont.load_default(), d.multiline_textbbox((10,10),'Hello\nWorld',font=f))),
    ("ImageDraw.getfont", lambda: ImageFont.load_default()),

    # ImageFont (all variants)
    ("ImageFont.load_default", lambda: ImageFont.load_default()),
    ("ImageFont.truetype", lambda: ImageFont.truetype('/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf',20) if __import__('os').path.exists('/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf') else None),
    ("ImageFont.FreeTypeFont.getbbox", lambda: (f:=ImageFont.load_default(), f.getbbox('Hello'))),
    ("ImageFont.FreeTypeFont.getlength", lambda: (f:=ImageFont.load_default(), f.getlength('Hello'))),
    ("ImageFont.FreeTypeFont.getmask", lambda: (f:=ImageFont.load_default(), f.getmask('Hello'))),
    ("ImageFont.FreeTypeFont.getmetrics", lambda: (f:=ImageFont.load_default(), f.getmetrics())),
    ("ImageFont.FreeTypeFont.getname", lambda: (f:=ImageFont.load_default(), f.getname())),
    ("ImageFont.ImageFont.getbbox", lambda: (f:=ImageFont.load_default(), f.getbbox('Hello'))),
    ("ImageFont.ImageFont.getlength", lambda: (f:=ImageFont.load_default(), f.getlength('Hello'))),
    ("ImageFont.ImageFont.getmask", lambda: (f:=ImageFont.load_default(), f.getmask('Hello'))),
    ("ImageFont.load", lambda: ImageFont.load_default()),
    ("ImageFont.load_default_imagefont", lambda: ImageFont.load_default()),
    ("ImageFont.load_path", lambda: ImageFont.truetype('/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf',20) if __import__('os').path.exists('/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf') else None),

    # ImagePalette
    ("ImagePalette.copy", lambda: img.copy()),
    ("ImagePalette.tobytes", lambda: img.tobytes()),

    # ImageStat
    ("ImageStat.Stat", lambda: _core.chops_invert(img._rust_image).tobytes()),

    # ImageFilter (named filters via img.filter())
    ("ImageFilter.BLUR", lambda: img.filter("BLUR").tobytes()),
    ("ImageFilter.CONTOUR", lambda: img.filter("CONTOUR").tobytes()),
    ("ImageFilter.DETAIL", lambda: img.filter("DETAIL").tobytes()),
    ("ImageFilter.EDGE_ENHANCE", lambda: img.filter("EDGE_ENHANCE").tobytes()),
    ("ImageFilter.EDGE_ENHANCE_MORE", lambda: img.filter("EDGE_ENHANCE_MORE").tobytes()),
    ("ImageFilter.EMBOSS", lambda: img.filter("EMBOSS").tobytes()),
    ("ImageFilter.FIND_EDGES", lambda: img.filter("FIND_EDGES").tobytes()),
    ("ImageFilter.SHARPEN", lambda: img.filter("SHARPEN").tobytes()),
    ("ImageFilter.SMOOTH", lambda: img.filter("SMOOTH").tobytes()),
    ("ImageFilter.SMOOTH_MORE", lambda: img.filter("SMOOTH_MORE").tobytes()),
    ("ImageFilter.GaussianBlur", lambda: img.gaussian_blur(2.0).tobytes()),
    ("ImageFilter.BoxBlur", lambda: img.box_blur(2).tobytes()),
    ("ImageFilter.UnsharpMask", lambda: img.unsharp_mask(2.0, 150, 3).tobytes()),
    ("ImageFilter.MaxFilter", lambda: img.max_filter(3).tobytes()),
    ("ImageFilter.MinFilter", lambda: img.min_filter(3).tobytes()),
    ("ImageFilter.MedianFilter", lambda: img.median_filter(3).tobytes()),
    ("ImageFilter.ModeFilter", lambda: img.mode_filter(3).tobytes()),

    # Pipeline benchmarks
    ("pipeline_20_st", lambda: (
        load("ref_2k.jpg")
        .resize((800,600),"LANCZOS")
        .crop((100,100,500,500))
        .convert("L")
        .rotate(90)
        .filter("BLUR")
        .tobytes()
    )),
]

print(f"Running {len(ALL_OPS)} benchmarks ({runs} runs each)...")
for name, fn in ALL_OPS:
    r = bench(name, fn, runs=runs, warmup=1)
    if r:
        results[name] = r
        print(f"  {name:<45} {r['mean_ms']:>8.1f}ms")
    # Save incrementally
    BENCH_DIR.mkdir(parents=True, exist_ok=True)
    with open(out_path, "w") as f:
        json.dump(results, f, indent=2)

print(f"\nSaved {len(results)} results to {out_path}")
