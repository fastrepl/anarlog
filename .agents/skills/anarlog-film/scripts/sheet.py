"""Tiles render.mjs shots into one labelled contact sheet for review.

    uv run --with pillow python3 sheet.py <shotsDir> [columns] [crop x0,y0,x1,y1]

Tiles keep the shots' own aspect ratio, so a 9:16 sheet is not squashed. Each tile's film time sits
in a strip above it, never over the frame (a label on the frame hides the docked mark and rails).
Read sheet.png after every render.
"""

import glob
import os
import re
import sys

from PIL import Image, ImageDraw

shots_dir = sys.argv[1]
cols = int(sys.argv[2]) if len(sys.argv) > 2 else 3
crop = tuple(int(v) for v in sys.argv[3].split(",")) if len(sys.argv) > 3 else None


def stamp(f):
    return float(re.search(r"t([\d.]+)\.png", os.path.basename(f)).group(1))


files = sorted(glob.glob(os.path.join(shots_dir, "t*.png")), key=stamp)
if not files:
    sys.exit(f"no t*.png shots in {shots_dir}")
first = Image.open(files[0])
src_w, src_h = (crop[2] - crop[0], crop[3] - crop[1]) if crop else first.size
w = 640 if src_w >= src_h else 360
h = round(w * src_h / src_w)
rows = -(-len(files) // cols)
gap, strip = 6, 18
cell_h = strip + h
sheet = Image.new(
    "RGB", (w * cols + gap * (cols - 1), cell_h * rows + gap * (rows - 1)), "#0b0b0a"
)
d = ImageDraw.Draw(sheet)
for i, f in enumerate(files):
    im = Image.open(f).convert("RGB")
    if crop:
        im = im.crop(crop)
    im = im.resize((w, h), Image.LANCZOS)
    x, y = (i % cols) * (w + gap), (i // cols) * (cell_h + gap)
    d.text((x + 4, y + 3), f"{stamp(f):.2f}s", fill="#f2f1ef")
    sheet.paste(im, (x, y + strip))
out = os.path.join(shots_dir, "sheet.png")
sheet.save(out)
print(out)
