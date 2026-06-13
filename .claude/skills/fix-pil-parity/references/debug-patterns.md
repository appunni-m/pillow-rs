# Debug Patterns for PIL Parity Tests

## Compare RSPIL vs PIL Output

When a test xfails with a hash mismatch, compare the outputs directly:

```bash
python3 -c "
from PIL import Image as PILImage, ImageFilter as PILF
from pillow_rs import Image, ImageFilter as RSPILF
import json, hashlib

with open('tests/fixtures/<FixtureName>.json') as f: fx = json.load(f)
raw = bytes.fromhex(fx['input']['bytes'])
mode = fx['input']['mode']
size = tuple(fx['input']['size'])

pil = PILImage.frombytes(mode, size, raw).filter(PILF.<FILTER>)
rs = Image.frombytes(mode, size, raw).filter(RSPILF.<FILTER>)

ph = hashlib.sha256(pil.tobytes()).hexdigest()
rh = hashlib.sha256(rs.tobytes()).hexdigest()
diffs = sum(1 for a,b in zip(pil.tobytes(), rs.tobytes()) if a!=b)
print(f'PIL={ph[:12]} RSPIL={rh[:12]} match={ph==rh} diffs={diffs}')

# Find first 5 different pixels
for i, (a,b) in enumerate(zip(pil.tobytes(), rs.tobytes())):
    if a != b:
        y, x = divmod(i, size[0])
        print(f'  ({x},{y}): PIL={a} RSPIL={b}')
        if sum(1 for _ in zip(pil.tobytes(), rs.tobytes()) if _[0]!=_[1]) > 5: break
"
```

## Classify Differences

- **All off by 1**: Rounding issue — check truncation vs rounding vs ceiling
- **Only border pixels**: Edge handling differs — check clamping vs copy vs skip
- **All pixels differ**: Kernel/algorithm is fundamentally wrong — re-research PIL source
- **Large random diffs**: Wrong kernel values, orientation, or formula

## Verify PIL's Actual Runtime Values

Documentation can be wrong. Always verify with:

```bash
python3 -c "from PIL import ImageFilter as PILF; print(PILF.<FILTER>.filterargs)"
```

## Determine Kernel Orientation

To test if kernel ordering matches PIL's C code:

1. Create a simple test image with a single bright pixel
2. Apply the filter in both PIL and RSPIL
3. Compare the output pattern — if the pattern is flipped, the kernel orientation is wrong

PIL C code applies kernels bottom-to-top (ky=0 maps to row y+1). The filterargs kernel values are stored in the order PIL expects for this bottom-to-top application.
