# Ara brand assets

`ara_logo_master.png` is the master source of Ara's visual identity — the
original "A" emblem + orbit + wordmark artwork, supplied by the project
owner. Every launcher icon, notification icon, and splash/onboarding image
in `app/src/main/res/` is **generated from this one file**, not drawn
separately.

**To regenerate** (after replacing the master logo with a new version):

```sh
./gradlew regenerateBrandIcons
```

(or directly: `python3 android/branding/generate_icons.py`, requires
Pillow). This overwrites the mipmap/drawable PNGs in `app/src/main/res/`
in place — review the diff before committing.

This task is **not** part of `assembleDebug`/`assembleRelease` on purpose:
the app always builds from the already-committed PNGs, so a machine without
Python/Pillow is never blocked. Only re-run it when the master logo itself
changes.

See `generate_icons.py` for the exact crop box and composition logic (why
the emblem is cropped out of the full lockup for the launcher icon, the
adaptive-icon safe-zone scaling, and the white-silhouette notification-icon
derivation) — also documented in `IMPLEMENTATION_STATUS.md` at the repo
root.
