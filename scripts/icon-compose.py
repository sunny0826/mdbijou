#!/usr/bin/env python3
"""Compose a macOS-style app icon from logo.png.

macOS Big Sur+ icon conventions applied:
- the artwork sits in the central 824x824 "safe area" of the 1024 canvas
  (a full-bleed icon renders noticeably larger than other Dock icons)
- squircle plate (corner radius ~22.5% of its side) with transparent corners
- subtle vertical gradient background plate
- logo glyph fills ~78% of the plate, keeping clear padding from the edges
- outer white background of the source logo is removed via flood fill so the
  document glyph sits directly on the plate (its white interior is preserved)

Output: assets/mdbijou-icon-1024.png (master for make-icon.sh)
"""

import sys
from PIL import Image, ImageDraw, ImageFilter

SIZE = 1024
PLATE = 824                 # macOS icon grid: 824x824 safe area, centered
PLATE_OFF = (SIZE - PLATE) // 2
RADIUS = int(PLATE * 0.225)  # macOS squircle approx radius
GLYPH_SCALE = 0.62          # glyph width relative to the plate (inner padding)
TOLERANCE = 40              # flood-fill tolerance for near-white background

# Background plate gradient: top -> bottom
GRAD_TOP = (250, 251, 253)
GRAD_BOTTOM = (218, 225, 235)

SRC = sys.argv[1] if len(sys.argv) > 1 else "logo.png"
OUT = sys.argv[2] if len(sys.argv) > 2 else "assets/mdbijou-icon-1024.png"


def remove_white_bg(img: Image.Image) -> Image.Image:
    """Flood-fill the outer near-white background to transparency."""
    img = img.convert("RGBA")
    px = img.load()
    w, h = img.size

    def is_bg(p):
        r, g, b, a = p
        return a > 0 and min(r, g, b) >= 255 - TOLERANCE

    stack = [(0, 0), (w - 1, 0), (0, h - 1), (w - 1, h - 1)]
    seen = set()
    while stack:
        x, y = stack.pop()
        if (x, y) in seen or not (0 <= x < w and 0 <= y < h):
            continue
        seen.add((x, y))
        if not is_bg(px[x, y]):
            continue
        px[x, y] = (255, 255, 255, 0)
        stack.extend([(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)])

    # Soften the edge: one-pixel alpha blur of the cutout.
    alpha = img.getchannel("A").filter(ImageFilter.GaussianBlur(1.5))
    img.putalpha(alpha)
    return img


def crop_to_content(img: Image.Image) -> Image.Image:
    bbox = img.getchannel("A").getbbox()
    return img.crop(bbox) if bbox else img


def make_plate() -> Image.Image:
    """824x824 squircle with a vertical gradient, centered on the 1024 canvas."""
    plate = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    grad = Image.new("RGBA", (PLATE, PLATE))
    gpx = grad.load()
    for y in range(PLATE):
        t = y / (PLATE - 1)
        c = tuple(int(GRAD_TOP[i] + (GRAD_BOTTOM[i] - GRAD_TOP[i]) * t) for i in range(3))
        for x in range(PLATE):
            gpx[x, y] = (*c, 255)
    mask = Image.new("L", (PLATE, PLATE), 0)
    ImageDraw.Draw(mask).rounded_rectangle([0, 0, PLATE - 1, PLATE - 1], radius=RADIUS, fill=255)
    plate.paste(grad, (PLATE_OFF, PLATE_OFF), mask)
    return plate


def main() -> None:
    glyph = crop_to_content(remove_white_bg(Image.open(SRC)))

    plate = make_plate()
    target_w = int(PLATE * GLYPH_SCALE)
    scale = target_w / glyph.width
    target_h = int(glyph.height * scale)
    glyph = glyph.resize((target_w, target_h), Image.LANCZOS)

    # Soft drop shadow for depth, then the glyph itself, centered.
    shadow = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    black = Image.new("RGBA", glyph.size, (30, 35, 48, 255))
    shadow_alpha = glyph.getchannel("A").point(lambda a: a * 0.25)
    black.putalpha(shadow_alpha)
    x = (SIZE - target_w) // 2
    y = (SIZE - target_h) // 2
    shadow.paste(black, (x, y + int(PLATE * 0.012)), black)
    shadow = shadow.filter(ImageFilter.GaussianBlur(PLATE * 0.008))

    icon = Image.alpha_composite(plate, shadow)
    icon.paste(glyph, (x, y), glyph)
    icon.save(OUT)
    print(f"created {OUT}")


if __name__ == "__main__":
    main()
