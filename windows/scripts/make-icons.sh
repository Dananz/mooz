#!/usr/bin/env bash
# Regenerates the Windows icon assets from the shared design sources.
#   assets/mooz.ico          exe icon (from design/icon/exports/default.png)
#   assets/tray-light.png    dark glyph for light taskbars
#   assets/tray-dark.png     white glyph for dark taskbars
#   assets/icon-128.rgba     raw RGBA app icon for the settings window
#   assets/tray-*.rgba       raw RGBA tray glyphs, sizes 16-48 concatenated
# Needs resvg (cargo install resvg) and Python 3 with Pillow.
set -euo pipefail
cd "$(dirname "$0")/.."
ROOT=..
PATH_D="$(grep -o 'd="[^"]*"' "$ROOT/design/icon/glyph.svg" | head -1)"

tray() { # $1 = fill, $2 = out
  cat > /tmp/mooz-tray.svg <<SVG
<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64" viewBox="12 12 232 232">
  <path fill="$1" $PATH_D/>
</svg>
SVG
  resvg -w 64 -h 64 /tmp/mooz-tray.svg "$2"
}
tray "#1C1C1E" assets/tray-light.png
tray "#FFFFFF" assets/tray-dark.png

# Raw RGBA tray glyphs at the small-icon sizes for 100-300% scaling, concatenated
# smallest first (16, 20, 24, 32, 40, 48). Loaded with CreateIconIndirect.
for theme in light dark; do
  fill="#1C1C1E"; [ "$theme" = dark ] && fill="#FFFFFF"
  : > "assets/tray-$theme.rgba"
  for size in 16 20 24 32 40 48; do
    sed "s/width=\"64\" height=\"64\"/width=\"$size\" height=\"$size\"/" /tmp/mooz-tray.svg \
      | sed "s/fill=\"#[0-9A-Fa-f]*\"/fill=\"$fill\"/" > /tmp/mooz-tray-sized.svg
    resvg -w "$size" -h "$size" /tmp/mooz-tray-sized.svg /tmp/mooz-tray-sized.png
    python3 -c "import sys; from PIL import Image; sys.stdout.buffer.write(Image.open('/tmp/mooz-tray-sized.png').convert('RGBA').tobytes())" >> "assets/tray-$theme.rgba"
  done
done

python3 - "$ROOT/design/icon/exports/default.png" assets/mooz.ico assets/icon-128.rgba <<'PY'
import sys
from PIL import Image
img = Image.open(sys.argv[1]).convert("RGBA")
img.save(sys.argv[2], sizes=[(16, 16), (20, 20), (24, 24), (32, 32), (40, 40), (48, 48), (64, 64), (128, 128), (256, 256)])
# Raw 128x128 RGBA for the settings window icon and About tab (no PNG decoder needed).
open(sys.argv[3], "wb").write(img.resize((128, 128), Image.LANCZOS).tobytes())
PY
echo "icons written to assets/"
