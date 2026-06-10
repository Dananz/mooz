#!/usr/bin/env python3
"""Render the Mooz DMG background: dark gradient, drag-arrow, title + hint.
Coordinates are top-left origin (matches dmgbuild icon_locations)."""
import sys
from PIL import Image, ImageDraw, ImageFont

W, H = 540, 380
OUT = sys.argv[1]

img = Image.new("RGB", (W, H))
px = img.load()
# vertical gradient: #171E2A (top) -> #0B0E14 (bottom)
top = (23, 30, 42)
bot = (11, 14, 20)
for y in range(H):
    t = y / (H - 1)
    r = round(top[0] + (bot[0] - top[0]) * t)
    g = round(top[1] + (bot[1] - top[1]) * t)
    b = round(top[2] + (bot[2] - top[2]) * t)
    for x in range(W):
        px[x, y] = (r, g, b)

d = ImageDraw.Draw(img, "RGBA")

def load_font(size):
    for p in (
        "/System/Library/Fonts/SFNS.ttf",
        "/System/Library/Fonts/Helvetica.ttc",
        "/System/Library/Fonts/Supplemental/Arial.ttf",
    ):
        try:
            return ImageFont.truetype(p, size)
        except Exception:
            continue
    return ImageFont.load_default()

def center_text(text, y, size, fill):
    f = load_font(size)
    bbox = d.textbbox((0, 0), text, font=f)
    tw = bbox[2] - bbox[0]
    d.text(((W - tw) / 2, y), text, font=f, fill=fill)

# icons sit centered at y=196; arrow spans the gap between them at that height
ay = 196
shaft_c = (150, 152, 158, 235)
d.line([(214, ay), (322, ay)], fill=shaft_c, width=5)
# arrowhead
d.polygon([(338, ay), (318, ay - 12), (318, ay + 12)], fill=shaft_c)

center_text("Mooz", 34, 30, (244, 245, 248, 255))
center_text("Drag Mooz onto Applications to install", 330, 13, (150, 153, 160, 255))

img.save(OUT, "PNG")
print(f"wrote {OUT} ({W}x{H})")
