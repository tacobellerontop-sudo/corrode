No Discord logos or proprietary fonts are bundled. The notification sounds are Discord audio assets; see [sound provenance and redistribution limitations](sounds/README.md). The interface leads proportional text with three unmodified SIL Open Font License 1.1 Inter faces and sets titles in two unmodified OFL Source Serif 4 Display faces (Regular, Semibold) as the `solace-serif` families; (Regular, Medium, SemiBold; the heavier two form the `medium`/`semibold` families because egui has no synthetic bold), keeps egui's default faces as fallbacks, and appends three unmodified OFL fallback faces (CJK, Arabic, Math) to every family. They are embedded once in the executable (the CJK face as a zstd archive inflated on first use); there is no runtime font download or render-time filesystem access.

| Asset | Upstream version | Bytes | Copyright and license |
|---|---|---:|---|
| `fonts/NotoSansCJKjp-Regular.otf.zst` | 2.004 | 12,034,822 (16,467,736 inflated) | © 2014–2021 Adobe; [OFL 1.1](fonts/NotoSansCJK-LICENSE.txt) |
| `fonts/NotoSansArabic.ttf` | 2.012 | 844,676 | Copyright 2022 The Noto Project Authors; [OFL 1.1](fonts/NotoSansArabic-OFL.txt) |
| `fonts/NotoSansMath-Regular.otf` | 3.000 (unhinted) | 479,308 | Copyright 2022 The Noto Project Authors; [OFL 1.1](fonts/NotoSansMath-OFL.txt) |
| `fonts/Inter-Regular.ttf` | 3.19 (hinted) | 680,240 | Copyright (c) 2016-2020 The Inter Project Authors; [OFL 1.1](fonts/Inter-OFL.txt) |
| `fonts/Inter-Medium.ttf` | 3.19 (hinted) | 694,512 | Copyright (c) 2016-2020 The Inter Project Authors; [OFL 1.1](fonts/Inter-OFL.txt) |
| `fonts/Inter-SemiBold.ttf` | 3.19 (hinted) | 710,040 | Copyright (c) 2016-2020 The Inter Project Authors; [OFL 1.1](fonts/Inter-OFL.txt) |
| `fonts/SourceSerif4Display-Regular.ttf` | 4.005 (Source Serif 4.5.1 release) | 296,680 | © 2014–2023 Adobe, Reserved Font Name ‘Source’; [OFL 1.1](fonts/SourceSerif4-OFL.md) |
| `fonts/SourceSerif4Display-Semibold.ttf` | 4.005 (Source Serif 4.5.1 release) | 302,412 | © 2014–2023 Adobe, Reserved Font Name ‘Source’; [OFL 1.1](fonts/SourceSerif4-OFL.md) |

Downloaded September 10, 2026 from pinned upstream sources:

- [Noto CJK font](https://github.com/notofonts/noto-cjk/blob/f8d157532fbfaeda587e826d4cd5b21a49186f7c/Sans/OTF/Japanese/NotoSansCJKjp-Regular.otf), [license](https://github.com/notofonts/noto-cjk/blob/f8d157532fbfaeda587e826d4cd5b21a49186f7c/Sans/LICENSE). SHA-256: `68a3fc98800b2a27b371f2fb79991daf3633bd89309d4ffaa6946fd587f375b5`. The repository stores it as `zstd -19 NotoSansCJKjp-Regular.otf` (archive SHA-256 `781dc80b10781f348c6701f059d75fe1e41c3c134eac129926c54e1b905df4ef`); the executable embeds the archive and inflates the unmodified font in memory the first time CJK text is displayed.
- [Noto Sans Arabic font](https://github.com/google/fonts/blob/334b789e33413f3aba4264d9aa6c97f7b94c5a2f/ofl/notosansarabic/NotoSansArabic%5Bwdth%2Cwght%5D.ttf), [license](https://github.com/google/fonts/blob/334b789e33413f3aba4264d9aa6c97f7b94c5a2f/ofl/notosansarabic/OFL.txt). The upstream variable-font filename is shortened locally; font bytes are unchanged. SHA-256: `63111b5b2e074dd48cc67692e0a2726d86ee94c1c37fe8598257b7b4e87e869e`.
- [Noto Sans Math 3.000](https://github.com/notofonts/math/releases/tag/NotoSansMath-v3.000) unhinted OpenType face from the official release archive (SHA-256 `ac351837b41f8a897f020b97fb0f075ad574c1e9669fb5839ada1f92fd748356`). The font bytes are unchanged. SHA-256: `a3a3904ede36039d4ba8177ec0aa9cf90653e7c7927d74d766bf31ea26e2a7c1`.

- [Source Serif 4](https://github.com/adobe-fonts/source-serif) static `TTF/SourceSerif4Display-Regular.ttf` and `TTF/SourceSerif4Display-Semibold.ttf` from Adobe's `source-serif` 4.5.1 npm release tarball (`https://registry.npmjs.org/source-serif/-/source-serif-4.5.1.tgz`), unmodified, with its `LICENSE.md` as `fonts/SourceSerif4-OFL.md`. SHA-256: Regular `a1788d674713c99e61d4541c85e5cfbc3145c4e746b0185c674a0b15f7313295`, Semibold `ba711c7443224e9c95bc5fcd24a98f3d921e50b3873c27667884351067927a03`.

- [Inter 3.19](https://github.com/rsms/inter/releases/tag/v3.19) static TrueType instances `Inter-Regular.ttf`, `Inter-Medium.ttf`, `Inter-SemiBold.ttf`, taken from the `Inter Hinted for Windows/Desktop/` directory of the official `Inter-3.19.zip` release archive (SHA-256 `150ab6230d1762a57bebf35dfc04d606ff91598a31d785f7f100356ecdcc0032`), re-fetched September 15, 2026. The archive's `LICENSE.txt` is byte-identical to the bundled `fonts/Inter-OFL.txt`. SHA-256: Regular `529be850e06f62f8904f22bda77e45bde4834498fdbec4ff4201fa3177447a3a`, Medium `6df88fcb83ac96582350f801355c6eff55f15710093e9627fb431caa40521151`, SemiBold `2de533bda937a063c595b07c6bd9b70c8c5087d0649a1c8330f7ac11fcc05602`.

  These are the hinted TrueType builds. Upstream also ships CFF outlines. egui paints
  grayscale coverage, not DirectWrite ClearType. Solace leaves the TrueType interpreter
  off and keeps sub-pixel binning on. Dark mode remaps coverage with
  `FontColorTransferFunction::Gamma(0.5)`. Light mode leaves the transfer function off.
  Inter faces set `FontTweak.hinting` to `Some(false)`. The files stay the hinted
  TrueType builds. The interpreter does not run. Glyph designs and advance widths are
  unchanged, so layout is unaffected.

The eight font blobs total **20,475,604 bytes (19.53 MiB)**. This raw size is separate from compressed distribution size, font-parser/layout memory and GPU glyph-atlas allocations. Package the license files and third-party copyright notices with the executable.

`cargo test -p solace-ui fonts` checks that every Solace font family lays out text. It does not prove complete Unicode coverage, Arabic shaping, IME behavior or platform rendering; the single CJK face uses Japanese regional Han forms.

## Icons

Solace draws its icons as vector paths in `crates/solace-ui/src/icons.rs`; no icon font or raster atlas is bundled.
