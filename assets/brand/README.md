# Corrode brand assets

Corrode's own application mark: a speech bubble carrying an S-shaped wave, in the icy
blue-to-indigo brand gradient. These files are Corrode design assets, not Apple, Discord or any
other vendor's branding, and no third-party font is embedded in them.

- `corrode.svg` — full-colour gradient mark on its rounded plate; the master vector source.
- `corrode-flat.svg` — flat (non-gradient) variant for small or low-colour contexts.
- `corrode-mark.svg` — solid white silhouette, view box trimmed to the mark's bounding box.
  `tools/generate-icons.py` rasterizes this one into the shared icon atlas as `corrode-mark`,
  which the interface tints at draw time.
- `corrode-1024.png` — 1024×1024 sRGB RGBA preview render of the full-colour mark.
- `corrode-tray.png` — 72×72 black-on-transparent render of `corrode-mark.svg`, embedded by
  `crates/platform` as the macOS menu bar template image; macOS tints it per appearance, so only
  its alpha is used. Regenerate with:

  ```sh
  sed 's/fill="#fff"/fill="#000"/g' assets/brand/corrode-mark.svg |
    rsvg-convert -w 72 -h 72 -o assets/brand/corrode-tray.png
  ```

The packaged platform icons built from the same artwork live in `packaging/` (`macos/Corrode.icns`
and `Corrode.icon`, `windows/Corrode.ico`, `linux/hicolor/*`).

Brand palette: `#8BBEFF` → `#506DDC` → `#2D247C` plate gradient, `#F5FAFF` and `#D1E0FF` waves.
The interface accent (`design::DEFAULT_PRIMARY_RGB`, `#1a72e8`) is drawn from the same family and
is tuned so white text on it keeps a 4.5:1 contrast ratio.
