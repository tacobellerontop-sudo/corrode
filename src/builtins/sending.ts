/**
 * Plugins that change (or hold back) what you send. They hook Discord's message sender through
 * `onSend` / `guardSend`; the text work itself lives in small pure functions that are unit tested.
 */
import { definePlugin, type PluginDefinition } from "../plugins/types";
import { copyText, escapeRegExp, guardSend, isDm, mapPlainText, onSend, splitList, withModule } from "./lib/helpers";
import { filters } from "../api/webpack";

/* ------------------------------ pure helpers ----------------------------- */

const TRACKING = [
  /^utm_/i,
  /^(fbclid|gclid|gclsrc|dclid|gbraid|wbraid|msclkid|twclid|yclid|igshid|igsh|mc_cid|mc_eid|_hsenc|_hsmi|oly_anon_id|oly_enc_id|vero_id|s_cid|mkt_tok|ref_src|ref_url|share_id|si|feature|pp|trk|tracking_id|spm|scm|cmpid|_ga|_gl)$/i,
];
// `si` and `feature` are only tracking on these hosts; elsewhere they can be real parameters.
const HOST_ONLY: Record<string, RegExp> = { si: /(^|\.)(youtube\.com|youtu\.be|spotify\.com)$/i, feature: /(^|\.)(youtube\.com|youtu\.be)$/i, pp: /(^|\.)youtube\.com$/i, t: /(^|\.)(twitter\.com|x\.com)$/i, s: /(^|\.)(twitter\.com|x\.com)$/i };

/** Removes tracking parameters from every link in `text`. */
export function clearUrls(text: string): string {
  return text.replace(/https?:\/\/[^\s<>]+/g, (raw) => {
    const trail = raw.match(/[)\].,!?;:'"]+$/)?.[0] ?? "";
    const link = trail ? raw.slice(0, -trail.length) : raw;
    let url: URL;
    try {
      url = new URL(link);
    } catch {
      return raw;
    }
    let changed = false;
    for (const key of [...url.searchParams.keys()]) {
      const hostRule = HOST_ONLY[key.toLowerCase()];
      const tracked = hostRule ? hostRule.test(url.hostname) : TRACKING.some((r) => r.test(key));
      if (tracked) {
        url.searchParams.delete(key);
        changed = true;
      }
    }
    if (!changed) return raw;
    let out = url.toString();
    if (!url.search && out.endsWith("?")) out = out.slice(0, -1);
    return out + trail;
  });
}

export interface ReplaceRule {
  from: string;
  to: string;
  regex?: RegExp;
}
/** Parses "from => to" lines. `/pattern/flags => to` makes a regular expression rule. */
export function parseRules(text: string): ReplaceRule[] {
  const rules: ReplaceRule[] = [];
  for (const line of String(text ?? "").split("\n")) {
    const i = line.indexOf("=>");
    if (i < 0) continue;
    const from = line.slice(0, i).trim();
    const to = line.slice(i + 2).trim();
    if (!from) continue;
    const m = from.match(/^\/(.+)\/([a-z]*)$/);
    if (m) {
      try {
        rules.push({ from, to, regex: new RegExp(m[1], m[2].includes("g") ? m[2] : `${m[2]}g`) });
      } catch {
        /* skip a bad pattern */
      }
    } else rules.push({ from, to });
  }
  return rules;
}
/** Applies rules to whole words (plain rules) outside code, links, mentions and emoji. */
export function applyRules(text: string, rules: ReplaceRule[]): string {
  if (!rules.length) return text;
  return mapPlainText(text, (part) => {
    for (const r of rules) {
      part = r.regex ? part.replace(r.regex, r.to) : part.replace(new RegExp(`(?<![\\p{L}\\p{N}_])${escapeRegExp(r.from)}(?![\\p{L}\\p{N}_])`, "giu"), () => r.to);
    }
    return part;
  });
}

/** Rewrites links to hosts in `map` (host -> replacement host), keeping the rest of the link. */
export function swapHosts(text: string, map: Record<string, string>): string {
  return text.replace(/https?:\/\/(?:www\.)?([a-z0-9.-]+)(\/[^\s<>]*)?/gi, (raw, host: string, rest = "") => {
    const to = map[host.toLowerCase()];
    return to ? `https://${to}${rest}` : raw;
  });
}

const UNITS: Record<string, number> = { s: 1, sec: 1, secs: 1, second: 1, seconds: 1, m: 60, min: 60, mins: 60, minute: 60, minutes: 60, h: 3600, hr: 3600, hrs: 3600, hour: 3600, hours: 3600, d: 86400, day: 86400, days: 86400, w: 604800, week: 604800, weeks: 604800 };

/**
 * Turns `[[...]]` time tokens into Discord timestamps that show in every reader's own time zone:
 * `[[14:30]]`, `[[2:30pm]]`, `[[2026-03-01 18:00]]`, `[[in 2h 30m]]`, `[[now]]`.
 * Text that isn't a recognisable time is left as it is.
 */
export function expandTimestamps(text: string, now = new Date()): string {
  return text.replace(/\[\[([^\[\]\n]{1,40})\]\]/g, (raw, inner: string) => {
    const s = inner.trim().toLowerCase();
    const unix = (d: Date) => Math.floor(d.getTime() / 1000);
    if (s === "now") return `<t:${unix(now)}:t>`;
    const rel = s.match(/^in ((?:\d+\s*[a-z]+\s*)+)$/);
    if (rel) {
      let total = 0;
      for (const [, n, unit] of rel[1].matchAll(/(\d+)\s*([a-z]+)/g)) {
        if (!(unit in UNITS)) return raw;
        total += Number(n) * UNITS[unit];
      }
      return `<t:${unix(now) + total}:R>`;
    }
    const m = s.match(/^(?:(\d{4})-(\d{1,2})-(\d{1,2})[ t]+)?(\d{1,2})(?::(\d{2}))?\s*(am|pm)?$/);
    if (!m || (m[5] === undefined && !m[6] && !m[1])) return raw;
    let hour = Number(m[4]);
    const minute = Number(m[5] ?? 0);
    if (m[6]) {
      if (hour < 1 || hour > 12) return raw;
      hour = (hour % 12) + (m[6] === "pm" ? 12 : 0);
    }
    if (hour > 23 || minute > 59) return raw;
    const d = new Date(now);
    if (m[1]) d.setFullYear(Number(m[1]), Number(m[2]) - 1, Number(m[3]));
    d.setHours(hour, minute, 0, 0);
    return `<t:${unix(d)}:${m[1] ? "f" : "t"}>`;
  });
}

/** Capitalises the first letter of the message and of each sentence. */
export function capitalize(text: string): string {
  let first = true;
  return mapPlainText(text, (part) => {
    // Only the very start of the message counts as a sentence start, not text after a link or emoji.
    const re = first ? /(^\s*|[.!?]\s+|\n\s*)(\p{Ll})/gu : /([.!?]\s+|\n\s*)(\p{Ll})/gu;
    first = false;
    return part.replace(re, (_, lead: string, ch: string) => lead + ch.toUpperCase());
  });
}

export function smartTypography(text: string): string {
  return mapPlainText(text, (part) =>
    part
      .replace(/\.\.\./g, "…")
      .replace(/(?<=\S) -- (?=\S)/g, " — ")
      .replace(/(^|[\s([{])"/g, "$1“")
      .replace(/"/g, "”")
      .replace(/(^|[\s([{])'/g, "$1‘")
      .replace(/'/g, "’"),
  );
}
export function plainTypography(text: string): string {
  return text.replace(/[“”„‟]/g, '"').replace(/[‘’‚‛]/g, "'").replace(/…/g, "...").replace(/[–—]/g, "-").replace(/ /g, " ");
}

/** Trims the ends, removes spaces before line breaks and collapses runs of blank lines. */
export function tidyWhitespace(text: string): string {
  return text
    .split(/(```[\s\S]*?```)/g)
    .map((part, i) => (i % 2 ? part : part.replace(/[ \t]+\n/g, "\n").replace(/\n{3,}/g, "\n\n").replace(/(?<=\S) {2,}(?=\S)/g, " ")))
    .join("")
    .trim();
}

/** Evaluates + - * / % ^ and brackets. Returns undefined for anything that isn't plain arithmetic. */
export function calculate(expr: string): number | undefined {
  const tokens = expr.replace(/\s+/g, "").match(/\d+\.?\d*|\.\d+|[-+*/%^()]/g);
  if (!tokens || tokens.join("") !== expr.replace(/\s+/g, "")) return undefined;
  let pos = 0;
  const peek = () => tokens[pos];
  const primary = (): number => {
    const t = tokens[pos++];
    if (t === "(") {
      const v = sum();
      if (tokens[pos++] !== ")") throw new Error("bracket");
      return v;
    }
    if (t === "-") return -primary();
    if (t === "+") return primary();
    if (t === undefined || !/^[\d.]/.test(t)) throw new Error("number");
    return Number(t);
  };
  const power = (): number => {
    const base = primary();
    return peek() === "^" ? (pos++, Math.pow(base, power())) : base;
  };
  const product = (): number => {
    let v = power();
    while (peek() === "*" || peek() === "/" || peek() === "%") {
      const op = tokens[pos++];
      const r = power();
      v = op === "*" ? v * r : op === "/" ? v / r : v % r;
    }
    return v;
  };
  const sum = (): number => {
    let v = product();
    while (peek() === "+" || peek() === "-") v = tokens[pos++] === "+" ? v + product() : v - product();
    return v;
  };
  try {
    const v = sum();
    return pos === tokens.length && Number.isFinite(v) ? v : undefined;
  } catch {
    return undefined;
  }
}
/** Replaces `{{2+2}}` with its result. */
export function expandCalculations(text: string): string {
  return text.replace(/\{\{([^{}\n]{1,80})\}\}/g, (raw, expr: string) => {
    const v = calculate(expr);
    return v === undefined ? raw : String(Math.round(v * 1e10) / 1e10);
  });
}

/** Parses "name = text" lines into a map. */
export function parsePairs(text: string): Map<string, string> {
  const out = new Map<string, string>();
  for (const line of String(text ?? "").split("\n")) {
    const i = line.indexOf("=");
    if (i <= 0) continue;
    const key = line.slice(0, i).trim();
    if (key) out.set(key.toLowerCase(), line.slice(i + 1).trim());
  }
  return out;
}
/** Replaces `;name` with the snippet called name. */
export function expandSnippets(text: string, snippets: Map<string, string>, trigger = ";"): string {
  if (!snippets.size) return text;
  const t = escapeRegExp(trigger);
  return mapPlainText(text, (part) => part.replace(new RegExp(`(^|\\s)${t}([\\w-]+)`, "g"), (raw, lead: string, name: string) => (snippets.has(name.toLowerCase()) ? lead + snippets.get(name.toLowerCase())!.replace(/\\n/g, "\n") : raw)));
}

/** Wraps links in <> so Discord doesn't build a preview. `domains` empty means every link. */
export function suppressEmbeds(text: string, domains: string[] = []): string {
  return text
    .split(/(```[\s\S]*?```|`[^`\n]*`)/g)
    .map((part, i) =>
      i % 2
        ? part
        : part.replace(/(<)?(https?:\/\/[^\s<>]+)(>)?/g, (raw, open: string | undefined, link: string, close: string | undefined) => {
            if (open && close) return raw;
            const trail = link.match(/[).,!?;:'"]+$/)?.[0] ?? "";
            const clean = trail ? link.slice(0, -trail.length) : link;
            if (domains.length) {
              let host = "";
              try {
                host = new URL(clean).hostname.toLowerCase();
              } catch {
                return raw;
              }
              if (!domains.some((d) => host === d.toLowerCase() || host.endsWith(`.${d.toLowerCase()}`))) return raw;
            }
            return `${open ?? ""}<${clean}>${trail}${close ?? ""}`;
          }),
    )
    .join("");
}

/** Lowercases a message that is mostly capitals. */
export function calmCaps(text: string, threshold = 0.7, minLetters = 8): string {
  const plain = mapPlainText(text, (p) => p);
  const letters = plain.match(/\p{L}/gu) ?? [];
  const upper = plain.match(/\p{Lu}/gu) ?? [];
  if (letters.length < minLetters || upper.length / letters.length < threshold) return text;
  return capitalize(mapPlainText(text, (p) => p.toLowerCase()));
}

export function censor(text: string, words: string[], mode: "spoiler" | "stars"): string {
  if (!words.length) return text;
  const re = new RegExp(`(?<![\\p{L}\\p{N}_])(${words.map(escapeRegExp).join("|")})(?![\\p{L}\\p{N}_])`, "giu");
  return mapPlainText(text, (part) => part.replace(re, (w) => (mode === "spoiler" ? `||${w}||` : w[0] + "*".repeat(Math.max(1, w.length - 1)))));
}

const SMALL_CAPS = "ᴀʙᴄᴅᴇꜰɢʜɪᴊᴋʟᴍɴᴏᴘǫʀsᴛᴜᴠᴡxʏᴢ";
export const TEXT_STYLES: Record<string, (s: string) => string> = {
  smallcaps: (s) => s.replace(/[a-z]/g, (c) => SMALL_CAPS[c.charCodeAt(0) - 97]),
  wide: (s) => s.replace(/[!-~]/g, (c) => String.fromCharCode(c.charCodeAt(0) + 0xfee0)).replace(/ /g, "　"),
  mock: (s) => {
    let i = 0;
    return s.replace(/\p{L}/gu, (c) => (i++ % 2 ? c.toUpperCase() : c.toLowerCase()));
  },
  spaced: (s) => [...s].join(" "),
  reverse: (s) => [...s].reverse().join(""),
  upper: (s) => s.toUpperCase(),
  lower: (s) => s.toLowerCase(),
};
/** `!wide hello` -> `ｈｅｌｌｏ`. Unknown styles are left alone. */
export function applyTextStyle(text: string, trigger = "!"): string {
  if (!text.startsWith(trigger)) return text;
  const m = text.slice(trigger.length).match(/^(\w+)\s+([\s\S]+)$/);
  const fn = m && TEXT_STYLES[m[1].toLowerCase()];
  return fn ? mapPlainText(m![2], fn) : text;
}

export const escapeMarkdown = (text: string) => text.replace(/([\\*_~`|>#\-[\]()])/g, "\\$1");

/** Expands `key:rest` using templates where %s is the (URL-encoded) rest. */
export function expandLinkShortcuts(text: string, shortcuts: Map<string, string>): string {
  if (!shortcuts.size) return text;
  return mapPlainText(text, (part) =>
    part.replace(/(^|\s)([a-z][\w-]*):([^\s]+)/gi, (raw, lead: string, key: string, rest: string) => {
      const template = shortcuts.get(key.toLowerCase());
      if (!template) return raw;
      const trail = rest.match(/[).,!?;:'"]+$/)?.[0] ?? "";
      const value = (trail ? rest.slice(0, -trail.length) : rest).split("/").map(encodeURIComponent).join("/");
      return value ? lead + template.replace(/%s/g, value) + trail : raw;
    }),
  );
}

export const stripInvisible = (text: string) => text.replace(/[​‌⁠﻿­᠎⁡-⁤]/g, "");

/* -------------------------------- plugins -------------------------------- */

const DEFAULT_SNIPPETS = ["shrug = ¯\\\\_(ツ)\\_/¯", "tableflip = (╯°□°)╯︵ ┻━┻", "unflip = ┬─┬ノ( º _ ºノ)", "lenny = ( ͡° ͜ʖ ͡°)", "disapprove = ಠ_ಠ"].join("\n");
const DEFAULT_SHORTCUTS = ["gh = https://github.com/%s", "npm = https://www.npmjs.com/package/%s", "wiki = https://en.wikipedia.org/wiki/%s", "yt = https://www.youtube.com/results?search_query=%s", "mdn = https://developer.mozilla.org/en-US/search?q=%s"].join("\n");

export const sendingPlugins: PluginDefinition<any>[] = [
  definePlugin({
    id: "clearUrls",
    name: "Clear URLs",
    description: "Removes tracking parameters (utm_source, fbclid, si and so on) from links you send.",
    authors: ["Corrode"],
    start: (ctx) => onSend(ctx, (c) => clearUrls(c), { order: 30 }),
  }),
  definePlugin({
    id: "textReplace",
    name: "Text Replace",
    description: "Replaces words or patterns in messages you send, using your own rules.",
    authors: ["Corrode"],
    settings: {
      rules: {
        type: "string",
        multiline: true,
        label: "Rules",
        description: "One per line: from => to. Use /pattern/flags => to for a regular expression.",
        placeholder: "teh => the\nbrb => be right back\n/colou?r/i => colour",
        default: "",
      },
    },
    start: (ctx) => onSend(ctx, (c) => applyRules(c, parseRules(String(ctx.settings.rules))), { order: 25 }),
  }),
  definePlugin({
    id: "embedFixer",
    name: "Embed Fixer",
    description: "Swaps links to sites whose previews don't work in Discord for mirror sites that embed properly. Leave a box empty to keep that site's links as they are.",
    authors: ["Corrode"],
    settings: {
      twitter: { type: "string", label: "Twitter / X", default: "fxtwitter.com" },
      instagram: { type: "string", label: "Instagram", default: "ddinstagram.com" },
      tiktok: { type: "string", label: "TikTok", default: "vxtiktok.com" },
      reddit: { type: "string", label: "Reddit", default: "rxddit.com" },
      bluesky: { type: "string", label: "Bluesky", default: "fxbsky.app" },
      pixiv: { type: "string", label: "Pixiv", default: "phixiv.net" },
    },
    start(ctx) {
      onSend(ctx, (c) => {
        const s = ctx.settings as Record<string, string>;
        const host = (v: string) => String(v ?? "").trim().replace(/^https?:\/\//, "").replace(/\/.*$/, "");
        const map: Record<string, string> = {};
        const add = (to: string, ...from: string[]) => {
          if (host(to)) for (const f of from) map[f] = host(to);
        };
        add(s.twitter, "twitter.com", "x.com", "mobile.twitter.com");
        add(s.instagram, "instagram.com");
        add(s.tiktok, "tiktok.com", "vm.tiktok.com");
        add(s.reddit, "reddit.com", "old.reddit.com");
        add(s.bluesky, "bsky.app");
        add(s.pixiv, "pixiv.net");
        return swapHosts(c, map);
      }, { order: 32 });
    },
  }),
  definePlugin({
    id: "sendTimestamps",
    name: "Send Timestamps",
    description: "Type [[14:30]], [[2:30pm]], [[2026-03-01 18:00]], [[in 2h]] or [[now]] and it is sent as a timestamp everyone sees in their own time zone.",
    authors: ["Corrode"],
    start: (ctx) => onSend(ctx, (c) => expandTimestamps(c), { order: 20 }),
  }),
  definePlugin({
    id: "silentMessages",
    name: "Silent Messages",
    description: "Sends your messages with @silent so they don't make a notification sound for others.",
    authors: ["Corrode"],
    settings: {
      where: {
        type: "select",
        label: "Send silently",
        default: "all",
        options: [
          { label: "Everywhere", value: "all" },
          { label: "Only in DMs", value: "dms" },
          { label: "Only in servers", value: "servers" },
        ],
      },
    },
    start(ctx) {
      onSend(
        ctx,
        (c, info) => {
          const dm = isDm(info.channelId);
          if ((ctx.settings.where === "dms" && !dm) || (ctx.settings.where === "servers" && dm)) return;
          if (!c.trim() || c.startsWith("@silent") || c.startsWith("/")) return;
          return `@silent ${c}`;
        },
        { edits: false, order: 70 },
      );
    },
  }),
  definePlugin({
    id: "autoCapitalize",
    name: "Auto Capitalise",
    description: "Capitalises the first letter of your messages and of each sentence.",
    authors: ["Corrode"],
    start: (ctx) => onSend(ctx, (c) => (c.startsWith("/") ? undefined : capitalize(c)), { order: 42 }),
  }),
  definePlugin({
    id: "typography",
    name: "Typography",
    description: "Converts quotes, dashes and ellipses as you send: either to proper typographic characters, or back to plain ones.",
    authors: ["Corrode"],
    settings: {
      mode: {
        type: "select",
        label: "Convert to",
        default: "smart",
        options: [
          { label: "Curly quotes, em dashes, …", value: "smart" },
          { label: "Plain quotes, hyphens, ...", value: "plain" },
        ],
      },
    },
    start: (ctx) => onSend(ctx, (c) => (ctx.settings.mode === "plain" ? plainTypography(c) : smartTypography(c)), { order: 44 }),
  }),
  definePlugin({
    id: "tidyWhitespace",
    name: "Tidy Whitespace",
    description: "Trims messages and removes doubled spaces and extra blank lines before sending. Code blocks are left alone.",
    authors: ["Corrode"],
    start: (ctx) => onSend(ctx, (c) => tidyWhitespace(c) || undefined, { order: 50 }),
  }),
  definePlugin({
    id: "inlineCalculator",
    name: "Inline Calculator",
    description: "Type {{12*3+4}} in a message and it is sent as the answer.",
    authors: ["Corrode"],
    start: (ctx) => onSend(ctx, (c) => expandCalculations(c), { order: 20 }),
  }),
  definePlugin({
    id: "snippets",
    name: "Snippets",
    description: "Type ;name to insert saved text, for example ;shrug.",
    authors: ["Corrode"],
    settings: {
      snippets: { type: "string", multiline: true, label: "Snippets", description: "One per line: name = text. Use \\n for a line break.", default: DEFAULT_SNIPPETS },
      trigger: { type: "string", label: "Trigger character", default: ";" },
    },
    start: (ctx) => onSend(ctx, (c) => expandSnippets(c, parsePairs(String(ctx.settings.snippets)), String(ctx.settings.trigger || ";")), { order: 20 }),
  }),
  definePlugin({
    id: "suppressEmbeds",
    name: "No Link Previews",
    description: "Sends your links wrapped in <> so Discord doesn't add a preview under the message.",
    authors: ["Corrode"],
    settings: {
      domains: { type: "string", label: "Only for these sites", description: "Comma separated, for example youtube.com, tenor.com. Empty means every link.", default: "" },
    },
    start: (ctx) => onSend(ctx, (c) => suppressEmbeds(c, splitList(ctx.settings.domains)), { order: 35 }),
  }),
  definePlugin({
    id: "mentionGuard",
    name: "Mention Guard",
    description: "Asks before sending a message that pings @everyone or @here, or more people than you allow.",
    authors: ["Corrode"],
    settings: {
      everyone: { type: "boolean", label: "Ask before @everyone and @here", default: true },
      maxMentions: { type: "number", label: "Ask when a message mentions more than this many people (0 = never)", default: 5, min: 0, max: 100 },
    },
    start(ctx) {
      guardSend(ctx, (c) => {
        const plain = mapPlainText(c, (p) => p);
        if (ctx.settings.everyone && /@(everyone|here)\b/.test(plain)) return window.confirm("This message pings @everyone or @here. Send it?");
        const count = new Set(c.match(/<@!?\d+>/g) ?? []).size;
        const max = Number(ctx.settings.maxMentions);
        if (max > 0 && count > max) return window.confirm(`This message mentions ${count} people. Send it?`);
        return true;
      }, 5);
    },
  }),
  definePlugin({
    id: "calmCaps",
    name: "Calm Caps",
    description: "Lowercases messages you send that are mostly capital letters.",
    authors: ["Corrode"],
    settings: { threshold: { type: "number", label: "Capitals needed to count as shouting (%)", default: 70, min: 30, max: 100 } },
    start: (ctx) => onSend(ctx, (c) => calmCaps(c, Number(ctx.settings.threshold) / 100), { order: 40 }),
  }),
  definePlugin({
    id: "wordFilter",
    name: "Word Filter",
    description: "Hides words you choose in your own messages, as spoilers or with asterisks.",
    authors: ["Corrode"],
    settings: {
      words: { type: "string", label: "Words", description: "Comma separated.", default: "" },
      mode: {
        type: "select",
        label: "Replace with",
        default: "spoiler",
        options: [
          { label: "A spoiler", value: "spoiler" },
          { label: "Asterisks", value: "stars" },
        ],
      },
    },
    start: (ctx) => onSend(ctx, (c) => censor(c, splitList(ctx.settings.words), ctx.settings.mode as "spoiler" | "stars"), { order: 46 }),
  }),
  definePlugin({
    id: "messageAffixes",
    name: "Prefix And Suffix",
    description: "Adds text of your choice before or after every message you send.",
    authors: ["Corrode"],
    settings: {
      prefix: { type: "string", label: "Before", default: "" },
      suffix: { type: "string", label: "After", default: "" },
      skipShort: { type: "number", label: "Skip messages shorter than this many characters", default: 0, min: 0, max: 500 },
    },
    start(ctx) {
      onSend(
        ctx,
        (c) => {
          if (!c.trim() || c.startsWith("/") || c.length < Number(ctx.settings.skipShort)) return;
          return `${ctx.settings.prefix ?? ""}${c}${ctx.settings.suffix ?? ""}`;
        },
        { edits: false, order: 60 },
      );
    },
  }),
  definePlugin({
    id: "textStyles",
    name: "Text Styles",
    description: `Start a message with !style to restyle it: ${Object.keys(TEXT_STYLES).join(", ")}. For example "!wide hello".`,
    authors: ["Corrode"],
    settings: { trigger: { type: "string", label: "Trigger character", default: "!" } },
    start: (ctx) => onSend(ctx, (c) => applyTextStyle(c, String(ctx.settings.trigger || "!")), { edits: false, order: 15 }),
  }),
  definePlugin({
    id: "escapeMarkdown",
    name: "Escape Markdown",
    description: "Start a message with \\\\ to send it exactly as typed, with no bold, italics or other formatting.",
    authors: ["Corrode"],
    start: (ctx) => onSend(ctx, (c) => (c.startsWith("\\\\") ? escapeMarkdown(c.slice(2).trimStart()) : undefined), { edits: false, order: 15 }),
  }),
  definePlugin({
    id: "linkShortcuts",
    name: "Link Shortcuts",
    description: "Expands short forms like gh:user/repo or wiki:Rust into full links.",
    authors: ["Corrode"],
    settings: {
      shortcuts: { type: "string", multiline: true, label: "Shortcuts", description: "One per line: key = link, with %s where the rest goes.", default: DEFAULT_SHORTCUTS },
    },
    start: (ctx) => onSend(ctx, (c) => expandLinkShortcuts(c, parsePairs(String(ctx.settings.shortcuts))), { order: 20 }),
  }),
  definePlugin({
    id: "stripInvisible",
    name: "Strip Invisible Characters",
    description: "Removes zero-width and other invisible characters from messages you send, which often sneak in when copying text.",
    authors: ["Corrode"],
    start: (ctx) => onSend(ctx, (c) => stripInvisible(c), { order: 10 }),
  }),
  definePlugin({
    id: "undoSend",
    name: "Undo Send",
    description: "Holds each message for a few seconds before sending it. Click the notice to cancel.",
    authors: ["Corrode"],
    settings: { delay: { type: "number", label: "Delay (seconds)", default: 3, min: 1, max: 30 } },
    start(ctx) {
      withModule(ctx, filters.byProps("sendMessage", "editMessage"), "Discord's message sender", (actions) => {
        const pending = new Set<ReturnType<typeof setTimeout>>();
        ctx.onStop(() => pending.forEach(clearTimeout));
        ctx.patcher.instead(actions, "sendMessage", (args, original) => {
          const message = args[1];
          if (!message || typeof message.content !== "string" || !message.content.trim()) return original(...args);
          const ms = Math.max(1, Number(ctx.settings.delay) || 3) * 1000;
          return new Promise((resolve, reject) => {
            let cancelled = false;
            const timer = setTimeout(() => {
              pending.delete(timer);
              close();
              if (!cancelled) Promise.resolve(original(...args)).then(resolve, reject);
            }, ms);
            pending.add(timer);
            const preview = message.content.length > 60 ? `${message.content.slice(0, 57)}…` : message.content;
            const close = ctx.toast(`Sending "${preview}". Click to cancel.`, {
              duration: ms,
              onClick: () => {
                cancelled = true;
                clearTimeout(timer);
                pending.delete(timer);
                void copyText(message.content);
                ctx.toast("Message cancelled. It was not sent; the text is on your clipboard.", { kind: "warning" });
                resolve(undefined);
              },
            });
          });
        });
      });
    },
  }),
];
