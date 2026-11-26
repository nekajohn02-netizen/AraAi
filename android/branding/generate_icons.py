#!/usr/bin/env python3
"""Regenerates every Ara launcher/notification/splash asset from the master
logo (`ara_logo_master.png`, next to this script).

Run manually whenever the master logo changes:

    python3 android/branding/generate_icons.py

Or via Gradle (same effect, run from the `android/` directory):

    ./gradlew regenerateBrandIcons

This writes directly into `android/app/src/main/res/...` — it is NOT part of
the normal `assembleDebug`/`assembleRelease` build graph, so a machine
without Python/Pillow can still build the app using the already-committed
PNGs this script produced. Re-run it and commit the result after editing
the master logo; don't hand-edit the generated PNGs themselves.

Requires: Pillow (``pip install Pillow``).
"""

from __future__ import annotations

import sys
from pathlib import Path

try:
    from PIL import Image
except ImportError:
    print("Missing dependency: Pillow. Install with `pip install Pillow`.", file=sys.stderr)
    sys.exit(1)

BRANDING_DIR = Path(__file__).resolve().parent
MASTER_LOGO = BRANDING_DIR / "ara_logo_master.png"
RES_DIR = BRANDING_DIR.parent / "app" / "src" / "main" / "res"

# Background sampled directly from the master logo's own corner pixels —
# not an invented color — so every generated asset matches the artwork with
# no visible seam against the adaptive-icon/splash background layer.
BRAND_BACKGROUND = (1, 2, 12, 255)

# Tight bounding box of just the "A" emblem/orbit mark in the 1254x1254
# master, excluding the "Ara" wordmark below it (found by thresholding the
# source image against BRAND_BACKGROUND; see git history of this file for
# the derivation if the master logo is ever replaced and this needs
# recomputing).
EMBLEM_BOX = (152, 10, 1101, 766)  # (left, top, right, bottom), with padding

LEGACY_ICON_SIZES = {"mdpi": 48, "hdpi": 72, "xhdpi": 96, "xxhdpi": 144, "xxxhdpi": 192}
ADAPTIVE_FG_SIZES = {"mdpi": 108, "hdpi": 162, "xhdpi": 216, "xxhdpi": 324, "xxxhdpi": 432}
NOTIFICATION_SIZES = {"mdpi": 24, "hdpi": 36, "xhdpi": 48, "xxhdpi": 72, "xxxhdpi": 96}


def _emblem(master: Image.Image) -> Image.Image:
    return master.crop(EMBLEM_BOX)


def _compose(emblem: Image.Image, canvas_px: int, fill_frac: float) -> Image.Image:
    """Centers `emblem` at `fill_frac` of `canvas_px` on the brand background."""
    canvas = Image.new("RGBA", (canvas_px, canvas_px), BRAND_BACKGROUND)
    scale = (canvas_px * fill_frac) / max(emblem.width, emblem.height)
    w, h = max(1, round(emblem.width * scale)), max(1, round(emblem.height * scale))
    resized = emblem.resize((w, h), Image.LANCZOS)
    canvas.paste(resized, ((canvas_px - w) // 2, (canvas_px - h) // 2), resized)
    return canvas


def _notification_glyph(emblem: Image.Image, canvas_px: int, fill_frac: float) -> Image.Image:
    """Android notification icons ignore RGB and use alpha as the mask: a
    white shape derived from the emblem's own brightness (the bright ribbon
    becomes opaque white, the near-black background becomes transparent),
    not a separately drawn icon."""
    scale = (canvas_px * fill_frac) / max(emblem.width, emblem.height)
    w, h = max(1, round(emblem.width * scale)), max(1, round(emblem.height * scale))
    resized = emblem.resize((w, h), Image.LANCZOS)
    gray = resized.convert("L")
    out = Image.new("RGBA", (canvas_px, canvas_px), (255, 255, 255, 0))
    white = Image.new("RGBA", (w, h), (255, 255, 255, 255))
    out.paste(white, ((canvas_px - w) // 2, (canvas_px - h) // 2), gray)
    return out


def main() -> None:
    if not MASTER_LOGO.exists():
        print(f"Master logo not found at {MASTER_LOGO}", file=sys.stderr)
        sys.exit(1)

    master_rgba = Image.open(MASTER_LOGO).convert("RGBA")
    emblem = _emblem(master_rgba)

    for density, px in LEGACY_ICON_SIZES.items():
        d = RES_DIR / f"mipmap-{density}"
        d.mkdir(parents=True, exist_ok=True)
        icon = _compose(emblem, px, 0.80)
        icon.save(d / "ic_launcher.png")
        icon.save(d / "ic_launcher_round.png")

    for density, px in ADAPTIVE_FG_SIZES.items():
        d = RES_DIR / f"mipmap-{density}"
        d.mkdir(parents=True, exist_ok=True)
        _compose(emblem, px, 0.60).save(d / "ic_launcher_foreground.png")

    for density, px in NOTIFICATION_SIZES.items():
        d = RES_DIR / f"drawable-{density}"
        d.mkdir(parents=True, exist_ok=True)
        _notification_glyph(emblem, px, 0.82).save(d / "ic_notification.png")

    nodpi = RES_DIR / "drawable-nodpi"
    nodpi.mkdir(parents=True, exist_ok=True)
    full = master_rgba.copy()
    full.thumbnail((1024, 1024), Image.LANCZOS)
    full.save(nodpi / "ara_logo_full.png")
    _compose(emblem, 512, 0.78).save(nodpi / "ic_launcher_hires.png")

    print(f"Regenerated all brand assets from {MASTER_LOGO} into {RES_DIR}")


if __name__ == "__main__":
    main()
