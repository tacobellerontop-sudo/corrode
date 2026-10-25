# Solace brand assets

Solace's mark is a four-pointed star with softly bowed sides, light catching a horizon, in
warm white on a sage tile. These are original Solace design assets, not Discord's or any
other vendor's branding, and contain no embedded fonts.

- `solace.svg` — the full-colour mark on its rounded tile; the master vector source.
- `solace-mark.svg` — the outline star alone (`currentColor`), as drawn in the app's rail.
- `solace-1024.png` — 1024×1024 render of `solace.svg`.

Packaged icons built from the same artwork: `packaging/windows/Solace.ico` (16–256 px) and
`packaging/windows/solace.png` (256 px). The running app also rasterizes the same star for its
window icon at startup (`apps/desktop/src/main.rs`), and the interface draws the mark as
vector strokes (`crates/solace-ui/src/icons.rs`).

Palette: tile `#2f6a5a` → `#435e52`, star `#faf8f3`. The interface accent (`theme::LIGHT.accent`,
`#2f6a5a`) is the same sage.
