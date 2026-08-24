# RIPterm 8x8 font

`RIPterm_8x8.raw` contains the 8x8 bitmap glyphs for CP437 codes 32 through
255 used by RIPterm/JDraw for BGI font 0. It was extracted independently from
JDraw running in DOSBox-X;

## Provenance

The extraction was performed on 2026-08-24:

1. Open `tools/ripterm_font_extract.rip` in JDraw/RIPterm under DOSBox-X.
2. Render the scene without scaling or filtering.
3. Capture the complete native 640x350 output as a PNG.
4. Run:

   ```sh
   python3 tools/extract_ripterm_font.py \
       /path/to/native-capture.png \
       crates/icy_engine/data/fonts/Rip/RIPterm_8x8.raw
   ```

The extraction scene selects RIP/BGI font 0, horizontal direction, size 1. It
renders CP437 codes 32 through 255 in a 16 by 14 grid:

- grid origin: `(8, 8)`
- cell spacing: 12 pixels horizontally and vertically
- glyph dimensions: 8 by 8 pixels
- code mapping: `32 + row * 16 + column`
- final glyph position: code 255 at `(188, 164)`

Codes 0 through 31 cannot be carried safely as literal bytes through the RIP
text stream. At runtime, `RIPTERM_FONT` starts with the existing IBM VGA50 font
and replaces codes 32 through 255 with the independently extracted data.

## File format

`RIPterm_8x8.raw` is 1,792 bytes: 224 glyphs times 8 rows. Each row is one
byte, with bit 7 representing the leftmost pixel.

The extractor treats black capture pixels as unset and every non-black pixel as
set. It rejects captures that are not exactly 640x350.

## Verification

Artifacts from the extraction run:

- `tools/ripterm_font_extract.rip`
  - size: 1,665 bytes
  - SHA-256: `ed23fc294184e60c126b4cb1305f776ca430cb0a4e04396a9b5074422ef8f916`
- native DOSBox-X capture (`jdraw_000.png`, not stored)
  - dimensions: 640x350
  - colors: black and white
  - SHA-256: `fd3415d1c88674b7d741e5daa9a97321da345d1269235fd77ff10ef2e7bfce9e`
- `RIPterm_8x8.raw`
  - size: 1,792 bytes
  - SHA-256: `7f70598de78f7a2c9353831dd418cde5c18dff8ee33da6a9c9dbb7edd97129fc`
