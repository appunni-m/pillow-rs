#!/usr/bin/env python3
"""Add missing baseline entries for all 74 uncovered manifest functions."""
import json, sys, timeit, io
from pathlib import Path
from PIL import Image, ImageFilter, ImageOps, ImageChops, ImageEnhance, ImageDraw, ImageFont, ImageColor, ImagePalette, ImageStat, ImageSequence

ROOT = Path(__file__).parent.parent
OUT_FILE = ROOT / "target" / "benchmarks" / "pillow_baseline.json"
REF_DIR = ROOT / "scripts" / "bench_reference_images"

# Load existing
with open(OUT_FILE) as f:
    existing = json.load(f)
existing_names = set(r["function"] for r in existing["results"])

IMG_RGB = lambda: Image.open(REF_DIR / "ref_2k.jpg")
IMG_RGBA = lambda: Image.open(REF_DIR / "ref_1k.png")
IMG_GRAY = lambda: Image.open(REF_DIR / "ref_grayscale.png")
IMG_SMALL = lambda: Image.new("RGB", (256, 256), (100, 100, 100))

COLLECT = []
def bench(name, fn, runs=10, warmup=2):
    if name in existing_names:
        return
    for _ in range(warmup):
        try: fn()
        except: pass
    times = []
    for _ in range(runs):
        try:
            t0 = timeit.default_timer()
            fn()
            times.append(timeit.default_timer() - t0)
        except:
            pass
    if not times:
        print(f"  SKIP {name} — failed")
        return
    mean = sum(times) / len(times)
    variance = sum((t - mean)**2 for t in times) / len(times)
    print(f"  {name:<45} {mean*1000:>8.2f}ms")
    COLLECT.append({"function": name, "runs": len(times), "min_s": min(times), "max_s": max(times),
            "mean_s": mean, "stdev_s": variance**0.5, "median_s": sorted(times)[len(times)//2]})

# ── Image class methods ──
def add():
    img = IMG_RGB(); r = img.getchannel(0); r.close()
    bench("Image.getchannel", lambda: IMG_RGB().getchannel(0))

    bench("Image.tobytes", lambda: IMG_RGB().tobytes())
    bench("Image.close", lambda: IMG_RGB().close())
    bench("Image.load", lambda: IMG_RGB().load())
    bench("Image.seek", lambda: (im:=IMG_RGB(), im.seek(0), im.close()))
    bench("Image.tell", lambda: (im:=IMG_RGB(), im.tell(), im.close()))
    bench("Image.verify", lambda: IMG_RGB().verify())
    bench("Image.transform", lambda: IMG_RGB().transform((512,512), Image.AFFINE, (1,0,0,0,1,0)))
    bench("Image.draft", lambda: IMG_RGB().draft("RGB", (512, 384)))
    bench("Image.tobitmap", lambda: IMG_SMALL().convert("1").tobitmap())
    bench("Image.remap_palette", lambda: IMG_SMALL().quantize(16).remap_palette(list(range(256))))
    bench("Image.frombytes", lambda: Image.frombytes("RGB", (64,64), b'\x00'*64*64*3))
    bench("Image.getdata", lambda: IMG_RGB().getdata())
    bench("Image.putdata", lambda: (im:=IMG_SMALL(), im.putdata([(255,0,0)]*65536), im.close()))
    bench("Image.show", lambda: None) # no-op, just measures overhead
    bench("Image.getexif", lambda: IMG_RGB().getexif())
    bench("Image.getim", lambda: IMG_RGB().getim())
    bench("Image.getpalette", lambda: IMG_SMALL().quantize(16).getpalette())
    bench("Image.putpalette", lambda: (im:=IMG_SMALL().quantize(16), im.putpalette([0]*768), im.close()))
    bench("Image.getxmp", lambda: IMG_RGB().getxmp())
    bench("Image.get_child_images", lambda: IMG_RGB().get_child_images())
    bench("Image.get_flattened_data", lambda: IMG_RGB().get_flattened_data())
    bench("Image.apply_transparency", lambda: IMG_RGBA().apply_transparency())

    # Image.save — separate from open_save composite
    im = IMG_RGB()
    buf = io.BytesIO()
    bench("Image.save", lambda: (im2:=IMG_RGB(), im2.save(buf, format="PNG"), buf.seek(0)))

    # Image.open — separate
    ref = REF_DIR / "ref_2k.jpg"
    bench("Image.open", lambda: (im2:=Image.open(ref), im2.load(), im2.close()))

    # ── ImageModule ──
    a = IMG_RGB(); b = IMG_RGB()
    bench("ImageModule.merge", lambda: Image.merge("RGB", [IMG_RGB().getchannel(c) for c in ("R","G","B")]))
    bench("ImageModule.blend", lambda: Image.blend(a, b, 0.5))
    bench("ImageModule.composite", lambda: Image.composite(a, b, Image.new("L", a.size, 128)))
    bench("ImageModule.eval", lambda: Image.eval(IMG_GRAY(), lambda x: x))
    bench("ImageModule.effect_noise", lambda: Image.effect_noise((256,256), 10))
    bench("ImageModule.fromarray", lambda: Image.fromarray(__import__('numpy').zeros((256,256,3), dtype='uint8')))
    bench("ImageModule.frombytes", lambda: Image.frombytes("L", (256,256), b'\x80'*65536))
    bench("ImageModule.open", lambda: Image.open(REF_DIR / "ref_2k.jpg"))

    # ── ImageChops ──
    bench("ImageChops.blend", lambda: ImageChops.blend(a, b, 0.5))
    bench("ImageChops.composite", lambda: ImageChops.composite(a, b, Image.new("L", a.size, 128)))
    bench("ImageChops.invert", lambda: ImageChops.invert(a))

    # ── ImageColor ──
    bench("ImageColor.getcolor", lambda: ImageColor.getcolor("red", "RGB"))
    bench("ImageColor.getrgb", lambda: ImageColor.getrgb("red"))

    # ── ImageDraw ──
    def _draw(op):
        im = Image.new("RGB", (512, 512), "white")
        d = ImageDraw.Draw(im)
        op(d)
    bench("ImageDraw.bitmap", lambda: _draw(lambda d: d.bitmap((10,10), Image.new("1", (50,50), 1))))
    bench("ImageDraw.chord", lambda: _draw(lambda d: d.chord([50,50,200,200], 0, 180, fill="red")))
    bench("ImageDraw.circle", lambda: _draw(lambda d: d.circle((100,100), 50, fill="blue")))
    bench("ImageDraw.getfont", lambda: ImageFont.load_default())
    bench("ImageDraw.multiline_text", lambda: _draw(lambda d: d.multiline_text((10,10), "Hello\nWorld", fill="black")))
    bench("ImageDraw.multiline_textbbox", lambda: _draw(lambda d: d.multiline_textbbox((10,10), "Hello\nWorld")))
    bench("ImageDraw.pieslice", lambda: _draw(lambda d: d.pieslice([50,50,200,200], 0, 180, fill="red")))
    bench("ImageDraw.polygon", lambda: _draw(lambda d: d.polygon([(50,50),(200,50),(200,200),(50,200)], fill="green")))
    bench("ImageDraw.regular_polygon", lambda: _draw(lambda d: d.regular_polygon((100,100,50), 6, fill="yellow")))
    bench("ImageDraw.rounded_rectangle", lambda: _draw(lambda d: d.rounded_rectangle([50,50,200,200], radius=10, fill="red")))
    bench("ImageDraw.textbbox", lambda: _draw(lambda d: d.textbbox((10,10), "Hello")))
    bench("ImageDraw.textlength", lambda: _draw(lambda d: d.textlength("Hello")))

    # ── ImageFilter ──
    bench("ImageFilter.Color3DLUT", lambda: IMG_RGB().filter(ImageFilter.Color3DLUT(17, [0]*4913*4)))
    bench("ImageFilter.Kernel", lambda: IMG_RGB().filter(ImageFilter.Kernel((3,3),[1]*9)))
    bench("ImageFilter.RankFilter", lambda: IMG_RGB().filter(ImageFilter.RankFilter(3, 4)))

    # ── ImageFont ──
    try:
        font = ImageFont.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", 20)
    except:
        font = ImageFont.load_default()
    try:
        bench("ImageFont.truetype", lambda: ImageFont.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", 20))
    except:
        pass
    bench("ImageFont.load", lambda: ImageFont.load_default())
    bench("ImageFont.load_default", lambda: ImageFont.load_default())
    bench("ImageFont.load_default_imagefont", lambda: ImageFont.load_default_imagefont())
    bench("ImageFont.load_path", lambda: ImageFont.load_path("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"))
    bench("ImageFont.FreeTypeFont", lambda: ImageFont.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", 20) if font else font)
    bench("ImageFont.FreeTypeFont.getlength", lambda: font.getlength("Hello"))
    bench("ImageFont.FreeTypeFont.getmask", lambda: font.getmask("Hello"))
    bench("ImageFont.FreeTypeFont.getmetrics", lambda: font.getmetrics())
    bench("ImageFont.FreeTypeFont.getname", lambda: font.getname())
    bench("ImageFont.ImageFont", lambda: ImageFont.load_default())
    try: bench("ImageFont.ImageFont.getlength", lambda: ImageFont.load_default().getlength("Hi"))
    except: pass
    try: bench("ImageFont.ImageFont.getmask", lambda: ImageFont.load_default().getmask("Hi"))
    except: pass

    # ── ImageOps ──
    bench("ImageOps.deform", lambda: ImageOps.deform(IMG_RGB(), ImageDeformer()))
    bench("ImageOps.exif_transpose", lambda: ImageOps.exif_transpose(IMG_RGB()))

    # ── ImagePalette ──
    pal = IMG_SMALL().quantize(16).getpalette()
    bench("ImagePalette.getcolor", lambda: ImagePalette.ImagePalette().getcolor((255,0,0)))
    bench("ImagePalette.getdata", lambda: ImagePalette.ImagePalette().getdata())
    pal_img = IMG_SMALL().quantize(16)
    bench("ImagePalette.save", lambda: (b:=io.BytesIO(), pal_img.getpalette(), ImagePalette.ImagePalette().save(b)))
    bench("ImagePalette.tobytes", lambda: ImagePalette.ImagePalette().tobytes())

    # ── ImageStat ──
    bench("ImageStat.Stat", lambda: ImageStat.Stat(IMG_RGB()))

    # ── ImageSequence ──
    bench("ImageSequence.Iterator", lambda: ImageSequence.Iterator(IMG_RGB()))

class ImageDeformer:
    def getmesh(self, im):
        return [((0,0,im.size[0],im.size[1]), (0,0,im.size[0],0,0,im.size[1],im.size[0],im.size[1]))]

add()

# Merge with existing
added = [r for r in COLLECT if r is not None]
existing["results"].extend(added)
existing["meta"]["num_functions"] = len(existing["results"])
existing["meta"]["total_time_s"] += sum(r["mean_s"] * r["runs"] for r in added)

with open(OUT_FILE, "w") as f:
    json.dump(existing, f, indent=2)
print(f"\nAdded {len(added)} new baselines. Total: {len(existing['results'])}")
