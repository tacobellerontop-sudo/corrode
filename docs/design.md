# Solace interface design

Solace is calm and editorial: warm paper surfaces, a sage accent, generous spacing, serif
titles and soft elevation. Everything is painted by `crates/solace-ui` with epaint primitives
and rendered on the GPU through wgpu; no stock egui widget chrome is visible (text fields use
egui's editor with Solace's own frames and colours).

## Layout

| Region | Width | Contents |
|---|---|---|
| Rail | 80 | Brand mark; Home, Direct messages, Communities, Activity; Settings; your account |
| Sidebar | 272 (232 when narrow; hidden on Home) | Title, search (Ctrl/⌘ K), Communities with the open community's channels, Direct Messages |
| Conversation | flexible | Header (icon, title, breadcrumb, People/Voice/More), pinned community banner, timeline, composer |
| Context | 326, hidden below 1180 | Voice Room card, Members, Community info, profile card |

Home is a full-width dashboard with no sidebar, led by direct messages: a greeting banner, stat tiles (unread messages, DM mentions, friends online, recent conversations), Messages and Online now, then Live in voice and Your communities. Opening a conversation from it moves to the Direct or Communities pane. The Thread panel floats over the right of the conversation. Below 760 px of height the
community banner scrolls with the timeline instead of staying pinned.

## Tokens

`theme.rs` defines two palettes, Paper (light) and Night (dark), selected by egui's theme
preference (Light, Dark or System; saved with the app's appearance setting).

| Token | Paper | Night | Use |
|---|---|---|---|
| canvas | `#f8f6f3` | `#18191b` | Conversation background |
| sidebar / rail | `#f3f1ed` | `#141517` | Navigation |
| surface | `#ffffff` | `#202124` | Cards, composer, dialogs |
| surface_alt | `#f0ede9` | `#26272b` | Quiet fills, chips, buttons |
| text / muted | `#272523` / `#8d8780` | `#ecebe7` / `#8f8b85` | Copy |
| accent | `#2f6a5a` | `#5ea58c` | Primary actions, links, mentions |
| online / idle / dnd / listening | `#3ea96c` / `#e9a13b` / `#d4574e` / `#9b82d6` | brighter equivalents | Presence |

Radii: cards 18, list items 12, chips 9, buttons fully round. Cards use a 24 px ambient
shadow; floating panels a deeper 40 px one.

## Type

- Titles: Source Serif 4 Display (Regular and Semibold), e.g. "Solace", community names,
  the banner title, card headings.
- Interface: Inter Regular, Medium and SemiBold.
- Fallbacks: Noto Sans CJK JP (inflated on first use), Noto Sans Arabic, Noto Sans Math, and
  system fonts for anything else. Emoji render from the bundled Twemoji atlas.

## Artwork

Community banners, the Voice Room card, the sign-in backdrop and community icons without an
uploaded image use procedurally rasterized landscapes (`scenery.rs`): layered ridged-noise
mountains in valley mist, a dawn palette for communities and a sage lake palette for voice
rooms. Each is generated once per seed and kept as a GPU texture. Avatars without a picture
use a tinted gradient monogram derived from the user's id.

## Icons

`icons.rs` draws every icon from strokes and fills on a 24-unit grid, so icons stay crisp at
any scale and inherit palette colours. There is no icon font or atlas.

## Motion

Hover states ease over 120 ms; typing dots and loading indicators animate only while visible
and request repaints only for as long as they run.
