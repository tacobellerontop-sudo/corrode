/**
 * The theme creator's model: the colours a user picks for parts of Discord, turned into a CSS
 * theme. The choices are saved inside the theme's header (`@corrodeCreator`), so a theme made
 * here can be opened in the creator again.
 *
 * Discord styles itself with CSS variables. Each part sets both the older names and the ones
 * from Discord's 2025 redesign, so the theme works on either.
 */

export type PartId =
  | "serverList"
  | "channelList"
  | "chat"
  | "userPanel"
  | "messageBox"
  | "menus"
  | "hover"
  | "selected"
  | "text"
  | "headers"
  | "muted"
  | "channels"
  | "channelsActive"
  | "links"
  | "accent"
  | "mention"
  | "scrollbar"
  | "borders";

export interface Part {
  id: PartId;
  label: string;
  hint: string;
  group: "Backgrounds" | "Text" | "Accents";
  /** Discord variables this colour sets. */
  vars: string[];
  /** Made see-through when the theme has a background picture or video. */
  panel?: boolean;
}

export const PARTS: Part[] = [
  { id: "serverList", group: "Backgrounds", label: "Server list", hint: "The column of server icons, and the window frame", panel: true, vars: ["--background-tertiary", "--background-base-lowest", "--bg-base-tertiary"] },
  { id: "channelList", group: "Backgrounds", label: "Channel list", hint: "Channels, DMs and the member list", panel: true, vars: ["--background-secondary", "--background-base-lower", "--bg-base-secondary"] },
  { id: "chat", group: "Backgrounds", label: "Chat", hint: "Behind messages", panel: true, vars: ["--background-primary", "--background-base-low", "--bg-base-primary", "--chat-background-default"] },
  { id: "userPanel", group: "Backgrounds", label: "Your account panel", hint: "Your name and mic buttons, bottom left", panel: true, vars: ["--background-secondary-alt", "--panel-bg"] },
  { id: "messageBox", group: "Backgrounds", label: "Message box", hint: "Where you type, and other text boxes", panel: true, vars: ["--channeltextarea-background", "--input-background", "--chat-input-container-background"] },
  { id: "menus", group: "Backgrounds", label: "Menus and popups", hint: "Right-click menus, profiles and dialogs", vars: ["--background-floating", "--background-surface-high", "--background-surface-higher", "--background-surface-highest", "--modal-background", "--bg-surface-overlay"] },
  { id: "hover", group: "Backgrounds", label: "Hover", hint: "Behind a message or channel under the mouse", vars: ["--background-modifier-hover", "--background-mod-subtle"] },
  { id: "selected", group: "Backgrounds", label: "Selected", hint: "The channel you're in", vars: ["--background-modifier-selected", "--background-modifier-active", "--background-mod-normal", "--background-mod-strong"] },
  { id: "text", group: "Text", label: "Message text", hint: "What people write", vars: ["--text-normal", "--text-default"] },
  { id: "headers", group: "Text", label: "Names and headings", hint: "Usernames, titles and the channel name at the top", vars: ["--header-primary", "--text-strong"] },
  { id: "muted", group: "Text", label: "Faded text", hint: "Timestamps, hints and descriptions", vars: ["--text-muted", "--header-secondary", "--text-subtle"] },
  { id: "channels", group: "Text", label: "Channel names", hint: "Channels you aren't in, and icons", vars: ["--channels-default", "--interactive-normal", "--icon-default"] },
  { id: "channelsActive", group: "Text", label: "Active channel", hint: "The channel you're in, and icons under the mouse", vars: ["--interactive-hover", "--interactive-active", "--icon-strong"] },
  { id: "links", group: "Text", label: "Links", hint: "Links in messages", vars: ["--text-link"] },
  {
    id: "accent",
    group: "Accents",
    label: "Accent",
    hint: "Buttons, switches and highlights (Discord's blurple)",
    vars: ["--brand-experiment", "--brand-500", "--brand-experiment-500", "--brand-560", "--brand-experiment-560", "--button-filled-brand-background", "--control-brand-foreground", "--control-brand-foreground-new", "--text-brand", "--bg-brand"],
  },
  { id: "mention", group: "Accents", label: "Mentions", hint: "Messages that ping you", vars: [] },
  { id: "scrollbar", group: "Accents", label: "Scrollbars", hint: "", vars: ["--scrollbar-auto-thumb", "--scrollbar-thin-thumb", "--scrollbar-auto-scrollbar-color-thumb"] },
  { id: "borders", group: "Accents", label: "Dividers", hint: "Lines between parts of the window", vars: ["--background-modifier-accent", "--border-subtle", "--border-faint", "--border-normal"] },
];

export type Colors = Record<PartId, string>;

export interface Preset {
  name: string;
  colors: Colors;
}

const preset = (name: string, c: string[]): Preset => ({ name, colors: Object.fromEntries(PARTS.map((p, i) => [p.id, c[i]])) as Colors });

/** Starting points. Columns follow PARTS. */
export const PRESETS: Preset[] = [
  //                       server     channels   chat       account    msgbox     menus      hover      selected   text       headers    muted      channels   active     links      accent     mention    scrollbar  dividers
  preset("Discord Dark", ["#1e1f22", "#2b2d31", "#313338", "#232428", "#383a40", "#111214", "#2e3035", "#404249", "#dbdee1", "#f2f3f5", "#949ba4", "#80848e", "#f2f3f5", "#00a8fc", "#5865f2", "#f0b232", "#1a1b1e", "#3f4147"]),
  preset("Midnight", ["#000000", "#0a0a0c", "#0f0f12", "#050506", "#17171c", "#0a0a0c", "#16161b", "#22222a", "#d6d8de", "#ffffff", "#80838c", "#6c707a", "#ffffff", "#6cb6ff", "#7b61ff", "#f0b232", "#26262e", "#1d1d22"]),
  preset("Light", ["#e3e5e8", "#f2f3f5", "#ffffff", "#ebedef", "#ebedef", "#ffffff", "#e8eaed", "#d7dade", "#313338", "#060607", "#5c5e66", "#5c5e66", "#060607", "#006ce7", "#5865f2", "#f0b232", "#c4c9ce", "#d3d6da"]),
  preset("Ocean", ["#0b1622", "#10202f", "#142838", "#0d1b28", "#1b3347", "#0b1622", "#1a3042", "#21405a", "#d5e6f3", "#ffffff", "#7f9bb3", "#6f8ba3", "#ffffff", "#5cc8ff", "#2d9cdb", "#f2c94c", "#0b1622", "#1d3446"]),
  preset("Forest", ["#121a14", "#18231b", "#1d2a20", "#141e17", "#26372a", "#101712", "#223226", "#2c4231", "#dce8de", "#ffffff", "#8aa190", "#7a917f", "#ffffff", "#7fd99a", "#3fa66b", "#e8c547", "#121a14", "#2a3a2d"]),
  preset("Rose", ["#1f1418", "#2a1b21", "#311f27", "#24171c", "#3d2830", "#1a1014", "#3a2730", "#4a303b", "#f1dde4", "#ffffff", "#b08d99", "#9c7b87", "#ffffff", "#ff8fb1", "#e0457b", "#ffcf5c", "#1f1418", "#45303a"]),
  preset("Sunset", ["#1d1420", "#271b2b", "#2e2033", "#21172a", "#3a2a40", "#170f1a", "#38283e", "#4a3352", "#f0e1ea", "#ffffff", "#a98ea3", "#957b90", "#ffffff", "#ffb36b", "#ff6b4a", "#ffd166", "#1d1420", "#40304a"]),
];

export type BackgroundKind = "image" | "video";

export interface Background {
  /** An http(s) URL, a `data:` URL, or `media/<file>` in the themes folder. */
  src: string;
  kind: BackgroundKind;
  fit: "cover" | "contain" | "tile";
  /** Blur in px. */
  blur: number;
  /** How much to darken it, 0–90 (%). */
  dim: number;
  /** How solid Discord's panels are over it, 0–100 (%). */
  panels: number;
}

export interface CreatorState {
  name: string;
  author: string;
  colors: Colors;
  /** "" keeps Discord's font. */
  font: string;
  background?: Background;
}

export const FONTS: { label: string; value: string }[] = [
  { label: "Discord's font", value: "" },
  { label: "System", value: 'system-ui, -apple-system, "Segoe UI", Roboto, sans-serif' },
  { label: "Rounded", value: '"Nunito", "Varela Round", "Quicksand", system-ui, sans-serif' },
  { label: "Serif", value: 'Georgia, "Times New Roman", serif' },
  { label: "Monospace", value: 'ui-monospace, "Cascadia Code", Consolas, "JetBrains Mono", monospace' },
];

export function newState(): CreatorState {
  return { name: "My Theme", author: "", colors: { ...PRESETS[0].colors }, font: "" };
}

export const HEX = /^#[0-9a-f]{6}$/i;

export function hexToRgb(hex: string): [number, number, number] {
  const n = parseInt(hex.slice(1), 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

export function rgba(hex: string, alpha: number): string {
  const [r, g, b] = hexToRgb(hex);
  return alpha >= 1 ? hex : `rgba(${r}, ${g}, ${b}, ${+alpha.toFixed(3)})`;
}

/** Lighter (amount > 0) or darker (< 0) version of a colour. */
export function shade(hex: string, amount: number): string {
  const [r, g, b] = hexToRgb(hex);
  const f = (c: number) => Math.round(amount < 0 ? c * (1 + amount) : c + (255 - c) * amount);
  return "#" + [f(r), f(g), f(b)].map((c) => Math.max(0, Math.min(255, c)).toString(16).padStart(2, "0")).join("");
}

const clamp = (n: unknown, lo: number, hi: number, d: number) => (typeof n === "number" && Number.isFinite(n) ? Math.min(hi, Math.max(lo, n)) : d);

/** A source safe to put in a CSS url("…") and in the header comment. */
export function cleanSrc(src: string): string {
  const s = src.trim();
  if (/^data:(image|video)\/[\w.+-]+;base64,[A-Za-z0-9+/=]+$/.test(s)) return s;
  if (/^media\/[\w.()+-]+$/.test(s) && !s.includes("..")) return s;
  try {
    const u = new URL(s);
    if (u.protocol === "https:" || u.protocol === "http:") return u.href.replace(/["'()\s*\\]/g, (c) => encodeURIComponent(c));
  } catch {}
  return "";
}

export function guessKind(src: string): BackgroundKind {
  if (/^data:video\//.test(src)) return "video";
  return /\.(mp4|webm|mov|m4v|ogv)(\?|#|$)/i.test(src) ? "video" : "image";
}

/** Fills gaps and drops anything unexpected (the state comes from a file the user can edit). */
export function normalize(raw: any): CreatorState {
  const s = newState();
  if (!raw || typeof raw !== "object") return s;
  if (typeof raw.name === "string" && raw.name.trim()) s.name = raw.name.trim().slice(0, 80);
  if (typeof raw.author === "string") s.author = raw.author.trim().slice(0, 80);
  for (const p of PARTS) if (typeof raw.colors?.[p.id] === "string" && HEX.test(raw.colors[p.id])) s.colors[p.id] = raw.colors[p.id].toLowerCase();
  if (typeof raw.font === "string") s.font = raw.font.replace(/[;{}<>\\]/g, "").slice(0, 200);
  const b = raw.background;
  if (b && typeof b === "object" && typeof b.src === "string" && cleanSrc(b.src)) {
    s.background = {
      src: cleanSrc(b.src),
      kind: b.kind === "video" ? "video" : b.kind === "image" ? "image" : guessKind(b.src),
      fit: b.fit === "contain" || b.fit === "tile" ? b.fit : "cover",
      blur: clamp(b.blur, 0, 40, 0),
      dim: clamp(b.dim, 0, 90, 30),
      panels: clamp(b.panels, 0, 100, 70),
    };
  }
  return s;
}

const b64encode = (text: string) => btoa(String.fromCharCode(...new TextEncoder().encode(text)));
const b64decode = (b64: string) => new TextDecoder().decode(Uint8Array.from(atob(b64), (c) => c.charCodeAt(0)));

/** Header text can't end the comment it is in. */
const headerText = (s: string) => s.replace(/\*\//g, "* /").replace(/[\r\n]+/g, " ");

/** The CSS file for a theme made in the creator. */
export function buildCss(state: CreatorState): string {
  const s = normalize(state);
  const c = s.colors;
  const bg = s.background;
  const alpha = bg ? bg.panels / 100 : 1;
  const lines: string[] = [];
  for (const p of PARTS) {
    const value = p.panel ? rgba(c[p.id], alpha) : c[p.id];
    for (const v of p.vars) lines.push(`  ${v}: ${value} !important;`);
  }
  // Derived shades.
  lines.push(`  --button-filled-brand-background-hover: ${shade(c.accent, -0.12)} !important;`);
  lines.push(`  --brand-600: ${shade(c.accent, -0.12)} !important;`);
  lines.push(`  --brand-experiment-600: ${shade(c.accent, -0.12)} !important;`);
  lines.push(`  --brand-400: ${shade(c.accent, 0.15)} !important;`);
  lines.push(`  --background-mentioned: ${rgba(c.mention, 0.1)} !important;`);
  lines.push(`  --background-mentioned-hover: ${rgba(c.mention, 0.08)} !important;`);
  lines.push(`  --background-message-highlight: ${rgba(c.mention, 0.1)} !important;`);
  lines.push(`  --background-message-highlight-hover: ${rgba(c.mention, 0.08)} !important;`);
  lines.push(`  --info-warning-foreground: ${c.mention} !important;`);
  lines.push(`  --scrollbar-auto-track: transparent !important;`);
  lines.push(`  --scrollbar-thin-track: transparent !important;`);
  if (s.font) lines.push(`  --font-primary: ${s.font} !important;`, `  --font-display: ${s.font} !important;`);

  const meta = {
    name: s.name,
    author: s.author,
    colors: s.colors,
    font: s.font,
    background: bg && { ...bg, src: undefined },
  };
  const header = [
    "/**",
    ` * @name ${headerText(s.name)}`,
    s.author && ` * @author ${headerText(s.author)}`,
    " * @description Made with Corrode's theme creator.",
    bg && ` * @background ${bg.src}`,
    ` * @corrodeCreator ${b64encode(JSON.stringify(meta))}`,
    " */",
  ].filter(Boolean);

  const css = [header.join("\n"), "", ":root, .theme-dark, .theme-darker, .theme-midnight, .theme-light, .visual-refresh {", ...lines, "}"];
  css.push(
    "",
    "::selection { background: " + rgba(c.accent, 0.4) + "; }",
    `[class*="mentioned_"] { box-shadow: inset 2px 0 0 ${c.mention}; }`,
  );
  if (s.font) css.push(`body, input, textarea, button { font-family: ${s.font}; }`);
  if (bg) {
    // The picture or video itself is shown by Corrode (see ThemeManager): a <corrode-theme-bg>
    // layer behind the app, from the @background line above. These rules make room for it.
    css.push(
      "",
      "/* Background */",
      `html, body, #app-mount, #app-mount [class^="bg_"], #app-mount [class*=" bg_"] { background: transparent !important; }`,
      `corrode-theme-bg {`,
      `  --corrode-bg-size: ${bg.fit === "tile" ? "auto" : bg.fit};`,
      `  --corrode-bg-repeat: ${bg.fit === "tile" ? "repeat" : "no-repeat"};`,
      `  filter: blur(${bg.blur}px) brightness(${+(1 - bg.dim / 100).toFixed(2)});`,
      `}`,
    );
  }
  return css.filter((l) => l !== "").join("\n") + "\n";
}

/** The creator state saved in a theme's header, or undefined if it wasn't made in the creator. */
export function readState(css: string): CreatorState | undefined {
  const m = /@corrodeCreator\s+([A-Za-z0-9+/=]+)/.exec(css);
  if (!m) return undefined;
  try {
    const raw = JSON.parse(b64decode(m[1]));
    const src = /@background\s+(\S+)/.exec(css)?.[1];
    if (raw.background && src) raw.background.src = src;
    else delete raw.background;
    return normalize(raw);
  } catch {
    return undefined;
  }
}

/** File name for a theme: "My Theme" → "My Theme.theme.css". */
export function fileName(name: string): string {
  const base = name.replace(/[\/\\:*?"<>|\x00-\x1f]/g, "").replace(/^[.\s]+|[.\s]+$/g, "").slice(0, 80) || "theme";
  return `${base}.theme.css`;
}
