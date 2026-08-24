#!/usr/bin/env python3

import argparse
from pathlib import Path

from PIL import Image


def main() -> None:
    parser = argparse.ArgumentParser(description="Extract RIPterm glyphs 32..255 from ripterm_font_extract.rip output")
    parser.add_argument("capture", type=Path, help="Native 640x350 DOSBox PNG capture")
    parser.add_argument("output", type=Path, help="Output raw 8x8 glyph data")
    args = parser.parse_args()

    image = Image.open(args.capture).convert("RGB")
    if image.size != (640, 350):
        raise ValueError(f"expected a native 640x350 capture, got {image.size[0]}x{image.size[1]}")

    data = bytearray()
    for code in range(32, 256):
        index = code - 32
        left = 8 + index % 16 * 12
        top = 8 + index // 16 * 12
        for y in range(8):
            row = 0
            for x in range(8):
                if image.getpixel((left + x, top + y)) != (0, 0, 0):
                    row |= 0x80 >> x
            data.append(row)

    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_bytes(data)


if __name__ == "__main__":
    main()