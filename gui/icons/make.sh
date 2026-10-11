#!/usr/bin/env bash
# Makes the kanban app's icons from docs/images/logo.svg, the one source of the logo; run it after changing that:
#
# - kanbrain.ico: Windows, embedded into kanbrain.exe by build.rs (window, taskbar, Explorer, Start Menu);
# - kanbrain.icns: macOS, the icon of the app bundle the installer makes;
# - kanbrain-128.png: Linux on X11, set on the window (Wayland desktops find the installed SVG by app id).
#
# Needs resvg and python3.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
logo="$here/../../docs/images/logo.svg"
tmp=$(mktemp -d)
trap 'rm -rf -- "$tmp"' EXIT

for size in 16 24 32 48 64 128 256 512 1024; do
    resvg -w "$size" -h "$size" "$logo" "$tmp/$size.png"
done
cp "$tmp/128.png" "$here/kanbrain-128.png"

python3 - "$tmp" "$here" <<'PY'
import struct, sys
from pathlib import Path

tmp, out = Path(sys.argv[1]), Path(sys.argv[2])
png = lambda size: (tmp / f"{size}.png").read_bytes()

# ICO: a directory of PNG images; a width or height of 0 means 256.
sizes = [16, 24, 32, 48, 64, 128, 256]
images = [png(s) for s in sizes]
header = struct.pack("<HHH", 0, 1, len(sizes))
offset = 6 + 16 * len(sizes)
entries = b""
for size, data in zip(sizes, images):
    side = 0 if size == 256 else size
    entries += struct.pack("<BBBBHHII", side, side, 0, 0, 1, 32, len(data), offset)
    offset += len(data)
(out / "kanbrain.ico").write_bytes(header + entries + b"".join(images))

# ICNS: typed PNG chunks, each size at 1x and the retina (2x) ones.
chunks = [("icp4", 16), ("icp5", 32), ("ic11", 32), ("icp6", 64), ("ic12", 64), ("ic07", 128),
          ("ic13", 256), ("ic08", 256), ("ic14", 512), ("ic09", 512), ("ic10", 1024)]
body = b"".join(kind.encode() + struct.pack(">I", 8 + len(png(size))) + png(size) for kind, size in chunks)
(out / "kanbrain.icns").write_bytes(b"icns" + struct.pack(">I", 8 + len(body)) + body)
PY
echo "made $here/kanbrain.ico, kanbrain.icns and kanbrain-128.png"
