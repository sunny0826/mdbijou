#!/usr/bin/env python3
"""Create a macOS-style icon with a black rounded-square background."""

import sys

from PIL import Image, ImageDraw


SIZE = 1024
PLATE = 824
PLATE_OFFSET = (SIZE - PLATE) // 2
RADIUS = int(PLATE * 0.225)
SRC = sys.argv[1] if len(sys.argv) > 1 else "logo.png"
OUT = sys.argv[2] if len(sys.argv) > 2 else "assets/mdbijou-icon-1024.png"


def main() -> None:
    logo = Image.open(SRC).convert("RGBA").resize(
        (PLATE, PLATE), Image.Resampling.LANCZOS
    )

    icon = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    background = Image.new("RGBA", (PLATE, PLATE), (0, 0, 0, 255))
    mask = Image.new("L", (PLATE, PLATE), 0)
    ImageDraw.Draw(mask).rounded_rectangle(
        (0, 0, PLATE - 1, PLATE - 1), radius=RADIUS, fill=255
    )
    icon.paste(background, (PLATE_OFFSET, PLATE_OFFSET), mask)
    icon.paste(logo, (PLATE_OFFSET, PLATE_OFFSET), Image.composite(
        logo.getchannel("A"), Image.new("L", logo.size, 0), mask
    ))
    icon.save(OUT)
    print(f"created {OUT}")


if __name__ == "__main__":
    main()
