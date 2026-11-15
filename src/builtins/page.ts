/**
 * Plugins that add to or react to the page itself: timestamps, message rows, links, media and
 * the message box. They work on the DOM (and Discord's message store where they need to know who
 * wrote something), marking elements with `data-corrode-*` attributes that their CSS styles, so
 * Discord's own rendering is never replaced.
 */
import { definePlugin, type PluginDefinition } from "../plugins/types";
import {
  chatBox,
  clamp,
  copyText,
  cssValue,
  escapeRegExp,
  every,
  getChannel,
  hotkey,
  insertIntoChat,
  isFriend,
  listen,
  myId,
  rootFlag,
  rowIds,
  snowflakeDate,
  splitList,
  store,
  typingInField,
  watch,
  watchMessages,
  type Ctx,
} from "./lib/helpers";
import { parsePairs } from "./sending";

const CHAT = 'ol[data-list-id="chat-messages"]';
const TIME = `${CHAT} time[datetime]`;
const usernameOf = (row: HTMLElement) => row.querySelector<HTMLElement>('[id^="message-username-"]');

const pad = (n: number) => String(n).padStart(2, "0");
function relative(date: Date, now = Date.now()): string {
  const s = Math.max(0, Math.round((now - date.getTime()) / 1000));
  if (s < 45) return "just now";
  if (s < 3600) return `${Math.round(s / 60)} min ago`;
  if (s < 86400) return `${Math.round(s / 3600)} h ago`;
  if (s < 86400 * 30) return `${Math.round(s / 86400)} d ago`;
  return date.toLocaleDateString();
}

/** Shows `text(el)` in place of each chat timestamp's own text, refreshed every `refreshMs`. */
function replaceTimestamps(ctx: Ctx, attr: string, text: (date: Date, el: HTMLElement) => string, refreshMs = 0) {
  ctx.css.inject(`${CHAT} time[${attr}] { font-size: 0 !important; }\n${CHAT} time[${attr}]::after { content: attr(${attr}); font-size: .75rem; }\n${CHAT} time[${attr}] i { display: none !important; }`);
  const live = new Set<HTMLElement>();
  const apply = (el: HTMLElement) => {
    const date = new Date(el.getAttribute("datetime") ?? "");
    if (!Number.isNaN(date.getTime())) el.setAttribute(attr, text(date, el));
  };
  watch(
    ctx,
    TIME,
    (el) => {
      live.add(el);
      apply(el);
    },
    (el) => el.removeAttribute(attr),
  );
  const refresh = () => {
    for (const el of live) {
      if (el.isConnected) apply(el);
      else live.delete(el);
    }
  };
  if (refreshMs) every(ctx, refreshMs, refresh);
  return refresh;
}

/** A small fixed panel in a corner, removed when the plugin stops. */
function floating(ctx: Ctx, css: string): HTMLDivElement {
  const el = document.createElement("div");
  el.setAttribute("data-corrode-float", ctx.id);
  el.style.cssText = `position:fixed;z-index:2147482000;font:500 12px/1.3 var(--font-primary,system-ui,sans-serif);color:var(--text-normal,#dbdee1);${css}`;
  const attach = () => document.body?.appendChild(el);
  if (document.body) attach();
  else document.addEventListener("DOMContentLoaded", attach, { once: true });
  ctx.onStop(() => el.remove());
  return el;
}

/** The chat's scrolling container. */
const chatScroller = (): HTMLElement | null => {
  let el: HTMLElement | null = document.querySelector<HTMLElement>(CHAT);
  while (el && !(el.scrollHeight > el.clientHeight + 1 && /(auto|scroll)/.test(getComputedStyle(el).overflowY))) el = el.parentElement;
  return el;
};

const hoveredVideo = (): HTMLVideoElement | null =>
  document.querySelector<HTMLVideoElement>("video:hover") ?? [...document.querySelectorAll<HTMLVideoElement>("video")].find((v) => !v.paused && v.offsetParent !== null) ?? null;

export const pagePlugins: PluginDefinition<any>[] = [
  /* ------------------------------- timestamps ------------------------------ */
  definePlugin({
    id: "preciseTimestamps",
    name: "Precise Timestamps",
    description: "Shows message times with seconds, and optionally the date and a 24-hour clock.",
    authors: ["Corrode"],
    settings: {
      seconds: { type: "boolean", label: "Show seconds", default: true },
      hour24: { type: "boolean", label: "24-hour clock", default: true },
      date: { type: "boolean", label: "Always show the date", default: false },
    },
    start(ctx) {
      const refresh = replaceTimestamps(ctx, "data-corrode-time", (d) => {
        const s = ctx.settings;
        const h = s.hour24 ? pad(d.getHours()) : String(d.getHours() % 12 || 12);
        const time = `${h}:${pad(d.getMinutes())}${s.seconds ? `:${pad(d.getSeconds())}` : ""}${s.hour24 ? "" : d.getHours() < 12 ? " AM" : " PM"}`;
        const today = new Date().toDateString() === d.toDateString();
        return s.date || !today ? `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${time}` : time;
      });
      (this as any).refresh = refresh;
    },
    onSettingChange() {
      (this as any).refresh?.();
    },
  }),
  definePlugin({
    id: "relativeTimestamps",
    name: "Relative Timestamps",
    description: 'Shows message times as "5 min ago" and keeps them up to date. Hover a time for the exact one.',
    authors: ["Corrode"],
    start: (ctx) => void replaceTimestamps(ctx, "data-corrode-ago", (d) => relative(d), 30_000),
  }),

  /* ------------------------------ message rows ------------------------------ */
  definePlugin({
    id: "highlightOwn",
    name: "Highlight My Messages",
    description: "Tints your own messages so they are easy to spot.",
    authors: ["Corrode"],
    settings: { color: { type: "string", label: "Colour", default: "rgba(210,105,47,.10)", placeholder: "rgba(210,105,47,.10)" } },
    css: (s) => `li[data-corrode-own] > div { background-color: ${cssValue(s.color, "rgba(210,105,47,.10)")} !important; }`,
    start: (ctx) => watchMessages(ctx, (row, m) => m.author?.id === myId() && row.setAttribute("data-corrode-own", ""), (row) => row.removeAttribute("data-corrode-own")),
  }),
  definePlugin({
    id: "highlightFriends",
    name: "Highlight Friends",
    description: "Marks messages from your friends with a coloured bar.",
    authors: ["Corrode"],
    settings: { color: { type: "string", label: "Colour", default: "#23a55a", placeholder: "#23a55a" } },
    css: (s) => `li[data-corrode-friend] > div { box-shadow: inset 3px 0 0 ${cssValue(s.color, "#23a55a")} !important; }`,
    start: (ctx) => watchMessages(ctx, (row, m) => m.author?.id && isFriend(m.author.id) && row.setAttribute("data-corrode-friend", ""), (row) => row.removeAttribute("data-corrode-friend")),
  }),
  definePlugin({
    id: "dimBots",
    name: "Dim Bot Messages",
    description: "Fades messages from bots until you hover them.",
    authors: ["Corrode"],
    settings: { opacity: { type: "number", label: "Opacity (%)", default: 45, min: 10, max: 90, step: 5 } },
    css: (s) => `li[data-corrode-bot] > div { opacity: ${clamp(s.opacity, 10, 90, 45) / 100} !important; transition: opacity .12s !important; }\nli[data-corrode-bot]:hover > div { opacity: 1 !important; }`,
    start: (ctx) => watchMessages(ctx, (row, m) => m.author?.bot && row.setAttribute("data-corrode-bot", ""), (row) => row.removeAttribute("data-corrode-bot")),
  }),
  definePlugin({
    id: "userTags",
    name: "User Tags",
    description: "Shows your own private label next to people's names in chat.",
    authors: ["Corrode"],
    settings: {
      tags: { type: "string", multiline: true, label: "Tags", description: "One per line: user ID = label.", placeholder: "123456789012345678 = coworker", default: "" },
    },
    css: `${CHAT} [data-corrode-tag]::after { content: attr(data-corrode-tag); margin-left: 6px; padding: 0 5px; border-radius: 4px; font-size: .7rem; font-weight: 600; vertical-align: middle; background: var(--background-modifier-accent, rgba(78,80,88,.48)); color: var(--text-normal, #dbdee1); }`,
    start(ctx) {
      watchMessages(
        ctx,
        (row, m) => {
          const tag = parsePairs(String(ctx.settings.tags)).get(m.author?.id);
          if (tag) usernameOf(row)?.setAttribute("data-corrode-tag", tag);
        },
        (row) => usernameOf(row)?.removeAttribute("data-corrode-tag"),
      );
    },
  }),
  definePlugin({
    id: "userColors",
    name: "User Colours",
    description: "Gives chosen people's names your own colour in chat.",
    authors: ["Corrode"],
    settings: {
      colors: { type: "string", multiline: true, label: "Colours", description: "One per line: user ID = colour.", placeholder: "123456789012345678 = #ff9900", default: "" },
    },
    start(ctx) {
      watchMessages(
        ctx,
        (row, m) => {
          const color = parsePairs(String(ctx.settings.colors)).get(m.author?.id);
          const name = usernameOf(row);
          if (color && name) {
            name.style.setProperty("color", cssValue(color, ""), "important");
            name.style.setProperty("-webkit-text-fill-color", cssValue(color, ""), "important");
            name.setAttribute("data-corrode-color", "");
          }
        },
        (row) => {
          const name = usernameOf(row);
          if (name?.hasAttribute("data-corrode-color")) {
            name.style.removeProperty("color");
            name.style.removeProperty("-webkit-text-fill-color");
            name.removeAttribute("data-corrode-color");
          }
        },
      );
    },
  }),
  definePlugin({
    id: "userLocalTime",
    name: "User Local Time",
    description: "Shows what time it is for people whose time zone you have saved, next to their messages.",
    authors: ["Corrode"],
    settings: {
      zones: { type: "string", multiline: true, label: "Time zones", description: "One per line: user ID = time zone name.", placeholder: "123456789012345678 = Europe/Paris", default: "" },
    },
    css: `${CHAT} [data-corrode-ltime]::after { content: " · " attr(data-corrode-ltime) " for them"; font-size: .75rem; color: var(--text-muted, #949ba4); }`,
    start(ctx) {
      const tracked = new Map<HTMLElement, string>();
      const show = (el: HTMLElement, zone: string) => {
        try {
          el.setAttribute("data-corrode-ltime", new Intl.DateTimeFormat(undefined, { hour: "2-digit", minute: "2-digit", timeZone: zone }).format(new Date()));
        } catch {
          tracked.delete(el); // not a valid time zone name
        }
      };
      watchMessages(
        ctx,
        (row, m) => {
          const zone = parsePairs(String(ctx.settings.zones)).get(m.author?.id);
          const name = usernameOf(row);
          if (zone && name) {
            tracked.set(name, zone);
            show(name, zone);
          }
        },
        (row) => usernameOf(row)?.removeAttribute("data-corrode-ltime"),
      );
      every(ctx, 30_000, () => {
        for (const [el, zone] of tracked) {
          if (el.isConnected) show(el, zone);
          else tracked.delete(el);
        }
      });
    },
  }),
  definePlugin({
    id: "accountAge",
    name: "Account Age Tooltip",
    description: "Hover a name in chat to see when that account was created.",
    authors: ["Corrode"],
    start(ctx) {
      watchMessages(
        ctx,
        (row, m) => {
          const name = usernameOf(row);
          if (!name || !m.author?.id) return;
          try {
            name.title = `Account created ${snowflakeDate(m.author.id).toLocaleDateString(undefined, { year: "numeric", month: "long", day: "numeric" })}`;
            name.setAttribute("data-corrode-age", "");
          } catch {}
        },
        (row) => {
          const name = usernameOf(row);
          if (name?.hasAttribute("data-corrode-age")) {
            name.removeAttribute("title");
            name.removeAttribute("data-corrode-age");
          }
        },
      );
    },
  }),
  definePlugin({
    id: "collapseLongMessages",
    name: "Collapse Long Messages",
    description: "Shortens very long messages. Click one to open or close it.",
    authors: ["Corrode"],
    settings: { height: { type: "number", label: "Collapse messages taller than (px)", default: 320, min: 100, max: 2000, step: 20 } },
    css: (s) =>
      `${CHAT} [data-corrode-long="closed"] { max-height: ${clamp(s.height, 100, 2000, 320)}px !important; overflow: hidden !important; cursor: pointer; -webkit-mask-image: linear-gradient(#000 70%, transparent); mask-image: linear-gradient(#000 70%, transparent); }\n${CHAT} [data-corrode-long="open"] { cursor: pointer; }`,
    start(ctx) {
      watch(
        ctx,
        `${CHAT} [id^="message-content-"]`,
        (el) => {
          if (el.scrollHeight > clamp(ctx.settings.height, 100, 2000, 320) + 40) el.setAttribute("data-corrode-long", "closed");
        },
        (el) => el.removeAttribute("data-corrode-long"),
      );
      listen(ctx, document, "click", (e: MouseEvent) => {
        const el = (e.target as Element | null)?.closest?.("[data-corrode-long]");
        if (!el || (e.target as Element).closest("a, button, [role=button]") || String(getSelection() ?? "")) return;
        el.setAttribute("data-corrode-long", el.getAttribute("data-corrode-long") === "closed" ? "open" : "closed");
      });
    },
  }),
  definePlugin({
    id: "highlightWords",
    name: "Highlight Words",
    description: "Highlights words you choose wherever they appear in chat.",
    authors: ["Corrode"],
    settings: {
      words: { type: "string", label: "Words", description: "Comma separated.", default: "" },
      color: { type: "string", label: "Highlight colour", default: "rgba(240,178,50,.45)", placeholder: "rgba(240,178,50,.45)" },
    },
    css: (s) => `::highlight(corrode-words) { background-color: ${cssValue(s.color, "rgba(240,178,50,.45)")}; color: inherit; }`,
    start(ctx) {
      const registry = (CSS as any).highlights;
      const HighlightCtor = (globalThis as any).Highlight;
      if (!registry || !HighlightCtor) return void ctx.logger.warn("This webview doesn't support text highlights");
      ctx.onStop(() => registry.delete("corrode-words"));
      let timer: ReturnType<typeof setTimeout> | undefined;
      const scan = () => {
        timer = undefined;
        const words = splitList(ctx.settings.words);
        const chat = document.querySelector(CHAT);
        if (!words.length || !chat) return void registry.delete("corrode-words");
        const re = new RegExp(words.map(escapeRegExp).join("|"), "giu");
        const ranges: Range[] = [];
        const walker = document.createTreeWalker(chat, NodeFilter.SHOW_TEXT);
        for (let node = walker.nextNode(); node && ranges.length < 2000; node = walker.nextNode()) {
          const text = node.nodeValue ?? "";
          if (!text.trim() || !(node.parentElement as Element | null)?.closest('[id^="message-content-"]')) continue;
          for (const m of text.matchAll(re)) {
            const r = new Range();
            r.setStart(node, m.index!);
            r.setEnd(node, m.index! + m[0].length);
            ranges.push(r);
          }
        }
        registry.set("corrode-words", new HighlightCtor(...ranges));
      };
      const schedule = () => {
        if (timer === undefined) timer = setTimeout(scan, 250);
      };
      const observer = new MutationObserver(schedule);
      observer.observe(document.documentElement, { childList: true, subtree: true, characterData: true });
      ctx.onStop(() => {
        observer.disconnect();
        if (timer !== undefined) clearTimeout(timer);
      });
      (this as any).rescan = schedule;
      schedule();
    },
    onSettingChange() {
      (this as any).rescan?.();
    },
  }),

  /* --------------------------------- clicks --------------------------------- */
  definePlugin({
    id: "doubleClickEdit",
    name: "Double Click To Edit",
    description: "Double-click one of your own messages to start editing it.",
    authors: ["Corrode"],
    start(ctx) {
      listen(ctx, document, "dblclick", (e: MouseEvent) => {
        const ids = rowIds(e.target as Element);
        if (!ids || (e.target as Element).closest("a, button, img, video, [class*='reactions']")) return;
        const m = store("MessageStore")?.getMessage?.(ids.channelId, ids.messageId);
        if (!m || m.author?.id !== myId()) return;
        getSelection()?.removeAllRanges();
        ctx.flux.dispatch({ type: "MESSAGE_START_EDIT", channelId: ids.channelId, messageId: ids.messageId, content: m.content ?? "" });
      });
    },
  }),
  definePlugin({
    id: "doubleClickReply",
    name: "Double Click To Reply",
    description: "Double-click someone else's message to start a reply to it.",
    authors: ["Corrode"],
    settings: { mention: { type: "boolean", label: "Ping the person by default", default: true } },
    start(ctx) {
      listen(ctx, document, "dblclick", (e: MouseEvent) => {
        const ids = rowIds(e.target as Element);
        if (!ids || (e.target as Element).closest("a, button, img, video, [class*='reactions']")) return;
        const message = store("MessageStore")?.getMessage?.(ids.channelId, ids.messageId);
        const channel = getChannel(ids.channelId);
        if (!message || !channel || message.author?.id === myId()) return;
        getSelection()?.removeAllRanges();
        ctx.flux.dispatch({ type: "CREATE_PENDING_REPLY", channel, message, shouldMention: !!ctx.settings.mention, showMentionToggle: !!channel.guild_id });
        chatBox()?.focus();
      });
    },
  }),
  definePlugin({
    id: "altClickCopy",
    name: "Alt Click To Copy",
    description: "Hold Alt and click a message to copy its text exactly as it was written, formatting marks included.",
    authors: ["Corrode"],
    start(ctx) {
      listen(
        ctx,
        document,
        "click",
        async (e: MouseEvent) => {
          if (!e.altKey || e.button !== 0) return;
          const ids = rowIds(e.target as Element);
          const m = ids && store("MessageStore")?.getMessage?.(ids.channelId, ids.messageId);
          if (!m?.content) return;
          e.preventDefault();
          e.stopPropagation();
          ctx.toast((await copyText(m.content)) ? "Message copied" : "Couldn't copy the message", { duration: 1500 });
        },
        true,
      );
    },
  }),
  definePlugin({
    id: "ctrlClickMedia",
    name: "Ctrl Click Opens Media",
    description: "Hold Ctrl (or Cmd) and click an image or video in chat to open the original file in your browser.",
    authors: ["Corrode"],
    start(ctx) {
      listen(
        ctx,
        document,
        "click",
        (e: MouseEvent) => {
          if (!(e.ctrlKey || e.metaKey) || e.button !== 0) return;
          const target = e.target as Element;
          const wrapper = target.closest?.(`${CHAT} [class*="imageWrapper"], ${CHAT} [class*="embedVideo"]`);
          if (!wrapper) return;
          const url = wrapper.querySelector<HTMLAnchorElement>('a[class*="originalLink"]')?.href ?? wrapper.querySelector<HTMLAnchorElement>("a[href]")?.href ?? wrapper.querySelector<HTMLVideoElement>("video")?.src ?? wrapper.querySelector<HTMLImageElement>("img")?.src;
          if (!url) return;
          e.preventDefault();
          e.stopPropagation();
          window.open(url.replace(/([?&])(width|height|format|quality)=[^&]*/g, "$1").replace(/[?&]+$/, ""), "_blank", "noopener");
        },
        true,
      );
    },
  }),

  /* ---------------------------------- links --------------------------------- */
  definePlugin({
    id: "linkStatusBar",
    name: "Link Status Bar",
    description: "Shows where a link really goes at the bottom of the window while you hover it, like a browser does.",
    authors: ["Corrode"],
    start(ctx) {
      const bar = floating(ctx, "left:0;bottom:0;max-width:60vw;padding:3px 8px;border-top-right-radius:6px;background:var(--background-floating,#111214);box-shadow:0 0 0 1px rgba(0,0,0,.3);white-space:nowrap;overflow:hidden;text-overflow:ellipsis;pointer-events:none;display:none");
      listen(ctx, document, "mouseover", (e: MouseEvent) => {
        const a = (e.target as Element | null)?.closest?.("a[href]") as HTMLAnchorElement | null;
        if (a && /^https?:/.test(a.href)) {
          bar.textContent = a.href;
          bar.style.display = "block";
        } else bar.style.display = "none";
      });
      listen(ctx, document, "mouseleave", () => (bar.style.display = "none"));
    },
  }),
  definePlugin({
    id: "maskedLinkWarning",
    name: "Masked Link Warning",
    description: "Marks links whose text looks like one address but which actually lead somewhere else.",
    authors: ["Corrode"],
    css: `${CHAT} a[data-corrode-masked] { text-decoration: underline wavy #f23f43 !important; }\n${CHAT} a[data-corrode-masked]::after { content: " ⚠ " attr(data-corrode-masked); color: #f23f43; font-size: .8em; }`,
    start(ctx) {
      watch(
        ctx,
        `${CHAT} [id^="message-content-"] a[href]`,
        (el) => {
          const a = el as HTMLAnchorElement;
          const shown = (a.textContent ?? "").trim().match(/^(?:https?:\/\/)?((?:[a-z0-9-]+\.)+[a-z]{2,})(?:[/:?#]|$)/i)?.[1]?.toLowerCase();
          if (!shown) return;
          try {
            const real = new URL(a.href).hostname.toLowerCase();
            const strip = (h: string) => h.replace(/^www\./, "");
            if (strip(shown) !== strip(real)) a.setAttribute("data-corrode-masked", `goes to ${real}`);
          } catch {}
        },
        (el) => el.removeAttribute("data-corrode-masked"),
      );
    },
  }),
  definePlugin({
    id: "confirmLinks",
    name: "Confirm External Links",
    description: "Asks before opening a link from chat, showing the real address. Sites you trust open straight away.",
    authors: ["Corrode"],
    settings: {
      trusted: { type: "string", label: "Trusted sites", description: "Comma separated.", default: "discord.com, discord.gg, youtube.com, youtu.be, github.com, wikipedia.org" },
    },
    start(ctx) {
      listen(
        ctx,
        window, // before every other click handler, including the client's own link opener
        "click",
        (e: MouseEvent) => {
          const a = (e.target as Element | null)?.closest?.(`${CHAT} a[href]`) as HTMLAnchorElement | null;
          if (!a || !/^https?:/.test(a.href)) return;
          let host: string;
          try {
            host = new URL(a.href).hostname.toLowerCase();
          } catch {
            return;
          }
          if (host === location.hostname) return;
          const trusted = splitList(ctx.settings.trusted).map((d) => d.toLowerCase());
          if (trusted.some((d) => host === d || host.endsWith(`.${d}`))) return;
          if (!window.confirm(`Open this link?\n\n${a.href}`)) {
            e.preventDefault();
            e.stopPropagation();
          }
        },
        true,
      );
    },
  }),
  definePlugin({
    id: "showAltText",
    name: "Show Image Descriptions",
    description: "Shows the description (alt text) people add to their images underneath them.",
    authors: ["Corrode"],
    css: `${CHAT} [data-corrode-alt]::after { content: attr(data-corrode-alt); display: block; margin-top: 4px; font-size: .8rem; color: var(--text-muted, #949ba4); max-width: 480px; white-space: normal; }`,
    start(ctx) {
      watch(
        ctx,
        `${CHAT} [class*="imageWrapper"] img[alt]`,
        (img) => {
          const alt = (img.getAttribute("alt") ?? "").trim();
          const holder = img.closest<HTMLElement>('[class*="imageContainer"], [class*="mosaicItem"]') ?? img.closest<HTMLElement>('[class*="imageWrapper"]')?.parentElement;
          if (alt && !/^(image|gif|video)$/i.test(alt) && holder) holder.setAttribute("data-corrode-alt", alt);
        },
        (img) => img.closest("[data-corrode-alt]")?.removeAttribute("data-corrode-alt"),
      );
    },
  }),

  /* ---------------------------------- media --------------------------------- */
  definePlugin({
    id: "videoSpeedKeys",
    name: "Video Speed Keys",
    description: "Press Shift+> and Shift+< to speed up or slow down the video you are watching.",
    authors: ["Corrode"],
    settings: { step: { type: "number", label: "Step", default: 0.25, min: 0.05, max: 1, step: 0.05 } },
    start(ctx) {
      listen(
        ctx,
        window,
        "keydown",
        (e: KeyboardEvent) => {
          if (!e.shiftKey || e.ctrlKey || e.metaKey || e.altKey || typingInField(e) || (e.key !== ">" && e.key !== "<")) return;
          const video = hoveredVideo();
          if (!video) return;
          e.preventDefault();
          e.stopPropagation();
          const step = clamp(ctx.settings.step, 0.05, 1, 0.25);
          video.playbackRate = Math.min(4, Math.max(0.25, Math.round((video.playbackRate + (e.key === ">" ? step : -step)) * 100) / 100));
          ctx.toast(`Speed ${video.playbackRate}×`, { duration: 1200 });
        },
        true,
      );
    },
  }),
  definePlugin({
    id: "rememberVideoVolume",
    name: "Remember Video Volume",
    description: "Uses the volume you last set for every video and audio file, instead of starting each one at full volume.",
    authors: ["Corrode"],
    start(ctx) {
      const isMedia = (t: EventTarget | null): t is HTMLMediaElement => t instanceof HTMLMediaElement;
      let applying = false;
      listen(
        ctx,
        document,
        "volumechange",
        (e: Event) => {
          if (applying || !isMedia(e.target)) return;
          ctx.storage.set("volume", e.target.volume);
          ctx.storage.set("muted", e.target.muted);
        },
        true,
      );
      listen(
        ctx,
        document,
        "play",
        (e: Event) => {
          const volume = ctx.storage.get("volume");
          if (!isMedia(e.target) || typeof volume !== "number") return;
          applying = true;
          e.target.volume = volume;
          e.target.muted = !!ctx.storage.get("muted");
          queueMicrotask(() => (applying = false));
        },
        true,
      );
    },
  }),
  definePlugin({
    id: "loopVideos",
    name: "Loop Videos",
    description: "Makes videos in chat start again when they finish.",
    authors: ["Corrode"],
    start: (ctx) => watch(ctx, `${CHAT} video`, (v) => ((v as HTMLVideoElement).loop = true), (v) => ((v as HTMLVideoElement).loop = false)),
  }),
  definePlugin({
    id: "pictureInPicture",
    name: "Picture In Picture",
    description: "Press a shortcut to pop the video you are watching out into its own small window.",
    authors: ["Corrode"],
    settings: { shortcut: { type: "string", label: "Shortcut", default: "alt+p" } },
    start(ctx) {
      hotkey(
        ctx,
        () => String(ctx.settings.shortcut),
        async () => {
          try {
            if (document.pictureInPictureElement) return void (await document.exitPictureInPicture());
            const video = hoveredVideo();
            if (!video) return void ctx.toast("Hover a video first", { duration: 2000 });
            await video.requestPictureInPicture();
          } catch (err) {
            ctx.toast("Picture in picture isn't available for this video", { kind: "warning" });
            ctx.logger.warn("Picture in picture failed", err);
          }
        },
      );
    },
  }),

  /* ------------------------------- message box ------------------------------ */
  definePlugin({
    id: "charCounter",
    name: "Character Counter",
    description: "Shows how many characters your message has while you type, and warns when it is too long.",
    authors: ["Corrode"],
    settings: { limit: { type: "number", label: "Limit", description: "2000 without Nitro, 4000 with it.", default: 2000, min: 100, max: 4000, step: 100 } },
    start(ctx) {
      const badge = floating(ctx, "padding:2px 7px;border-radius:8px;background:var(--background-floating,#111214);box-shadow:0 0 0 1px rgba(0,0,0,.3);pointer-events:none;display:none");
      const update = () => {
        const box = chatBox();
        const text = box?.textContent?.replace(/﻿/g, "") ?? "";
        if (!box || !text.length || !box.contains(document.activeElement)) return void (badge.style.display = "none");
        const limit = clamp(ctx.settings.limit, 100, 4000, 2000);
        const rect = box.getBoundingClientRect();
        badge.textContent = `${text.length} / ${limit}`;
        badge.style.color = text.length > limit ? "#f23f43" : text.length > limit * 0.9 ? "#f0b232" : "";
        badge.style.display = "block";
        badge.style.left = `${Math.max(0, rect.right - 90)}px`;
        badge.style.top = `${Math.max(0, rect.top - 24)}px`;
      };
      for (const type of ["input", "keyup", "focusin", "focusout", "click"]) listen(ctx, document, type, () => setTimeout(update), true);
      listen(ctx, window, "resize", update);
    },
  }),
  definePlugin({
    id: "disableSpellcheck",
    name: "Disable Spellcheck",
    description: "Turns off the red spelling underlines in the message box.",
    authors: ["Corrode"],
    start: (ctx) => watch(ctx, '[role="textbox"][contenteditable="true"]', (el) => el.setAttribute("spellcheck", "false"), (el) => el.setAttribute("spellcheck", "true")),
  }),
  definePlugin({
    id: "quoteSelection",
    name: "Quote Selection",
    description: "Select text in chat and press a shortcut to put it in the message box as a quote.",
    authors: ["Corrode"],
    settings: { shortcut: { type: "string", label: "Shortcut", default: "ctrl+shift+q" } },
    start(ctx) {
      hotkey(
        ctx,
        () => String(ctx.settings.shortcut),
        async () => {
          const text = String(getSelection() ?? "").trim();
          if (!text) return void ctx.toast("Select some text first", { duration: 2000 });
          const quote = `${text.split("\n").map((l) => `> ${l}`).join("\n")}\n`;
          const how = await insertIntoChat(quote);
          if (how === "copied") ctx.toast("Quote copied. Paste it into the message box.");
          else if (how === "failed") ctx.toast("Couldn't insert the quote", { kind: "warning" });
        },
      );
    },
  }),
  definePlugin({
    id: "pasteAsCode",
    name: "Paste As Code Block",
    description: "Press a shortcut to paste what you copied wrapped in a code block.",
    authors: ["Corrode"],
    settings: {
      shortcut: { type: "string", label: "Shortcut", default: "ctrl+shift+v" },
      language: { type: "string", label: "Language", description: "Used for syntax colours, for example js or py. Can be empty.", default: "" },
    },
    start(ctx) {
      hotkey(
        ctx,
        () => String(ctx.settings.shortcut),
        async () => {
          let text = "";
          try {
            text = await navigator.clipboard.readText();
          } catch {
            return void ctx.toast("Couldn't read the clipboard. Allow clipboard access and try again.", { kind: "warning" });
          }
          if (!text.trim()) return void ctx.toast("The clipboard has no text", { duration: 2000 });
          const how = await insertIntoChat(`\`\`\`${String(ctx.settings.language ?? "").replace(/[^\w+#-]/g, "")}\n${text.replace(/```/g, "`​``")}\n\`\`\``);
          if (how !== "inserted") ctx.toast(how === "copied" ? "Code block copied. Paste it into the message box." : "Couldn't insert the code block", { kind: how === "copied" ? "info" : "warning" });
        },
      );
    },
  }),
  definePlugin({
    id: "insertDate",
    name: "Insert Date And Time",
    description: "Press a shortcut to type the current date and time into the message box.",
    authors: ["Corrode"],
    settings: {
      shortcut: { type: "string", label: "Shortcut", default: "alt+d" },
      format: {
        type: "select",
        label: "Insert",
        default: "local",
        options: [
          { label: "Date and time as text", value: "local" },
          { label: "Date only", value: "date" },
          { label: "A Discord timestamp (shown in each reader's time zone)", value: "discord" },
        ],
      },
    },
    start(ctx) {
      hotkey(
        ctx,
        () => String(ctx.settings.shortcut),
        async () => {
          const now = new Date();
          const text = ctx.settings.format === "discord" ? `<t:${Math.floor(now.getTime() / 1000)}:f>` : ctx.settings.format === "date" ? now.toLocaleDateString() : now.toLocaleString();
          if ((await insertIntoChat(text)) === "copied") ctx.toast("Copied. Paste it into the message box.");
        },
      );
    },
  }),

  /* --------------------------------- window --------------------------------- */
  definePlugin({
    id: "scrollButtons",
    name: "Scroll Buttons",
    description: "Adds small buttons at the edge of chat to jump to the oldest loaded message or back to the newest.",
    authors: ["Corrode"],
    start(ctx) {
      const box = floating(ctx, "right:22px;bottom:96px;display:flex;flex-direction:column;gap:4px;opacity:.55");
      const button = (label: string, title: string, to: () => void) => {
        const b = document.createElement("button");
        b.textContent = label;
        b.title = title;
        b.style.cssText = "all:unset;cursor:pointer;width:26px;height:26px;border-radius:50%;text-align:center;background:var(--background-floating,#111214);box-shadow:0 0 0 1px rgba(0,0,0,.3);color:inherit";
        b.addEventListener("click", to);
        box.appendChild(b);
      };
      box.addEventListener("mouseenter", () => (box.style.opacity = "1"));
      box.addEventListener("mouseleave", () => (box.style.opacity = ".55"));
      button("↑", "Jump to the oldest loaded message", () => chatScroller()?.scrollTo({ top: 0 }));
      button("↓", "Jump to the newest message", () => {
        const s = chatScroller();
        s?.scrollTo({ top: s.scrollHeight });
      });
      every(ctx, 1000, () => (box.style.display = document.querySelector(CHAT) ? "flex" : "none"));
    },
  }),
  definePlugin({
    id: "cleanTitle",
    name: "Clean Window Title",
    description: "Removes the unread dot and mention count from the window title, or replaces the title with your own.",
    authors: ["Corrode"],
    settings: { title: { type: "string", label: "Fixed title", description: "Leave empty to keep Discord's title without the counters.", default: "" } },
    start(ctx) {
      let original = document.title;
      let writing = false;
      const clean = () => {
        if (writing) return;
        original = document.title;
        const fixed = String(ctx.settings.title ?? "").trim();
        const next = fixed || original.replace(/^(\(\d+\+?\)|•)\s*/, "");
        if (next !== document.title) {
          writing = true;
          document.title = next;
          writing = false;
        }
      };
      const titleEl = document.querySelector("title") ?? document.head?.appendChild(document.createElement("title"));
      if (!titleEl) return;
      const observer = new MutationObserver(clean);
      observer.observe(titleEl, { childList: true, characterData: true, subtree: true });
      ctx.onStop(() => {
        observer.disconnect();
        document.title = original;
      });
      (this as any).clean = clean;
      clean();
    },
    onSettingChange() {
      (this as any).clean?.();
    },
  }),
  definePlugin({
    id: "blurUnfocused",
    name: "Blur When Unfocused",
    description: "Blurs Discord whenever its window isn't the one you're using, so people glancing at your screen can't read it.",
    authors: ["Corrode"],
    settings: { amount: { type: "number", label: "Blur (px)", default: 12, min: 2, max: 40 } },
    css: (s) => `html[data-corrode-unfocused] #app-mount { filter: blur(${clamp(s.amount, 2, 40, 12)}px) !important; transition: filter .15s !important; }`,
    start(ctx) {
      const flag = rootFlag(ctx, "data-corrode-unfocused");
      listen(ctx, window, "blur", () => flag.set(true));
      listen(ctx, window, "focus", () => flag.set(false));
      flag.set(!document.hasFocus());
    },
  }),
  definePlugin({
    id: "privacyBlur",
    name: "Privacy Blur",
    description: "Press a shortcut to blur names, avatars and message text, for screenshots and screen sharing. Hover something to read it.",
    authors: ["Corrode"],
    settings: {
      shortcut: { type: "string", label: "Shortcut", default: "ctrl+shift+b" },
      messages: { type: "boolean", label: "Blur message text too", default: true },
    },
    css: (s) => {
      const on = "html[data-corrode-private]";
      const names = ['[class*="username_"]', '[class*="nameTag"]', '[class*="panelTitle"]', 'img[class*="avatar_"]', '[class*="avatar_"] foreignObject', '[class*="members_"] [class*="name_"]', '[class*="privateChannels"] [class*="name_"]', '[class*="mention"]', '[class*="repliedTextPreview"]'];
      const all = s.messages ? [...names, '[id^="message-content-"]', '[class*="imageWrapper"]', 'article[class*="embed"]'] : names;
      return `${all.map((n) => `${on} ${n}`).join(",\n")} { filter: blur(6px) !important; transition: filter .12s !important; }\n${all.map((n) => `${on} ${n}:hover`).join(",\n")} { filter: none !important; }`;
    },
    start(ctx) {
      const flag = rootFlag(ctx, "data-corrode-private");
      hotkey(ctx, () => String(ctx.settings.shortcut), () => void ctx.toast(flag.toggle() ? "Privacy blur on" : "Privacy blur off", { duration: 1500 }));
    },
  }),
  definePlugin({
    id: "zenMode",
    name: "Zen Mode",
    description: "Press a shortcut to hide the server, channel and member lists and leave only the conversation.",
    authors: ["Corrode"],
    settings: { shortcut: { type: "string", label: "Shortcut", default: "ctrl+shift+z" } },
    css: 'html[data-corrode-zen] nav[class*="guilds"], html[data-corrode-zen] div[class*="sidebar_"], html[data-corrode-zen] [class*="membersWrap"] { display: none !important; }',
    start(ctx) {
      const flag = rootFlag(ctx, "data-corrode-zen");
      hotkey(ctx, () => String(ctx.settings.shortcut), () => void ctx.toast(flag.toggle() ? "Zen mode on. Press the shortcut again to leave." : "Zen mode off", { duration: 2000 }));
    },
  }),
  definePlugin({
    id: "panicKey",
    name: "Panic Key",
    description: "Press a shortcut to cover Discord with a blank screen and silence its media at once. Press it again to come back.",
    authors: ["Corrode"],
    settings: {
      shortcut: { type: "string", label: "Shortcut", default: "ctrl+shift+x" },
      title: { type: "string", label: "Window title while hidden", default: "Documents" },
    },
    css: "html[data-corrode-panic] body { visibility: hidden !important; }\nhtml[data-corrode-panic] { background: #fff !important; }",
    start(ctx) {
      const flag = rootFlag(ctx, "data-corrode-panic");
      let title = "";
      const muted: HTMLMediaElement[] = [];
      const restore = () => {
        if (title) document.title = title;
        title = "";
        for (const m of muted.splice(0)) m.muted = false;
      };
      ctx.onStop(restore);
      hotkey(
        ctx,
        () => String(ctx.settings.shortcut),
        () => {
          if (flag.toggle()) {
            title = document.title;
            document.title = String(ctx.settings.title || "Documents");
            for (const m of document.querySelectorAll<HTMLMediaElement>("video, audio")) {
              if (!m.muted) {
                m.muted = true;
                muted.push(m);
              }
            }
          } else restore();
        },
      );
    },
  }),
  definePlugin({
    id: "cssHooks",
    name: "CSS Hooks",
    description: "Adds data-channel, data-guild, data-dm and data-focused to the page's <html> element so your Quick CSS and themes can style specific channels and servers.",
    authors: ["Corrode"],
    start(ctx) {
      const root = document.documentElement;
      const update = () => {
        const m = location.pathname.match(/^\/channels\/(@me|\d+)(?:\/(\d+))?/);
        const set = (name: string, value?: string | false) => (value ? root.setAttribute(name, value) : root.removeAttribute(name));
        set("data-guild", !!m && m[1] !== "@me" && m[1]);
        set("data-channel", m?.[2]);
        set("data-dm", m?.[1] === "@me" && "true");
        set("data-focused", document.hasFocus() && "true");
      };
      for (const type of ["CHANNEL_SELECT", "WINDOW_FOCUS"]) ctx.flux.subscribe(type, () => setTimeout(update));
      listen(ctx, window, "popstate", update);
      listen(ctx, window, "focus", update);
      listen(ctx, window, "blur", update);
      every(ctx, 2000, update);
      ctx.onStop(() => ["data-guild", "data-channel", "data-dm", "data-focused"].forEach((n) => root.removeAttribute(n)));
      update();
    },
  }),

  /* ---------------------------- lists by ID (CSS) ---------------------------- */
  definePlugin({
    id: "channelColors",
    name: "Channel Colours",
    description: "Gives chosen channels their own colour in the channel list.",
    authors: ["Corrode"],
    settings: { colors: { type: "string", multiline: true, label: "Colours", description: "One per line: channel ID = colour.", placeholder: "123456789012345678 = #23a55a", default: "" } },
    css: (s) =>
      [...parsePairs(String(s.colors))]
        .filter(([id]) => /^\d+$/.test(id))
        .map(([id, color]) => `[data-list-item-id="channels___${id}"] [class*="name_"], [data-list-item-id="channels___${id}"] svg { color: ${cssValue(color, "inherit")} !important; }`)
        .join("\n"),
  }),
  definePlugin({
    id: "hideChannels",
    name: "Hide Channels",
    description: "Removes chosen channels from the channel list without leaving or muting them.",
    authors: ["Corrode"],
    settings: { channels: { type: "string", label: "Channel IDs", description: "Comma separated.", default: "" } },
    css: (s) => {
      const list = splitList(s.channels).filter((id) => /^\d+$/.test(id));
      return list.length ? `${list.map((id) => `li:has([data-list-item-id="channels___${id}"])`).join(",\n")} { display: none !important; }` : "";
    },
  }),
  definePlugin({
    id: "hideServers",
    name: "Hide Servers",
    description: "Removes chosen servers from the server list without leaving them.",
    authors: ["Corrode"],
    settings: { servers: { type: "string", label: "Server IDs", description: "Comma separated.", default: "" } },
    css: (s) => {
      const list = splitList(s.servers).filter((id) => /^\d+$/.test(id));
      return list.length ? `${list.map((id) => `nav[class*="guilds"] [class*="listItem"]:has([data-list-item-id="guildsnav___${id}"])`).join(",\n")} { display: none !important; }` : "";
    },
  }),
  definePlugin({
    id: "hideDms",
    name: "Hide DMs",
    description: "Removes chosen conversations from the DM list without closing them.",
    authors: ["Corrode"],
    settings: { channels: { type: "string", label: "DM channel IDs", description: "Comma separated. Open the DM and copy the last number from the address, or use Copy Channel ID.", default: "" } },
    css: (s) => {
      const list = splitList(s.channels).filter((id) => /^\d+$/.test(id));
      return list.length ? `${list.map((id) => `li:has(a[href="/channels/@me/${id}"])`).join(",\n")} { display: none !important; }` : "";
    },
  }),
];
