/**
 * Shared helpers for built-in plugins. Everything here cleans up after itself through the
 * plugin's context, and nothing blocks a plugin's start(): when a Discord module can't be found
 * the plugin logs a warning and simply does nothing.
 */
import { common } from "../../api/common";
import { instead as patchInstead } from "../../api/patcher";
import { filters, findStore, waitFor, type Filter } from "../../api/webpack";
import type { PluginContext } from "../../plugins/types";

export type Ctx = PluginContext<any>;

/* ------------------------------- modules -------------------------------- */

/** Calls `cb` once a module matching `filter` exists. Never blocks and never throws. */
export function withModule(ctx: Ctx, filter: Filter, what: string, cb: (mod: any) => void) {
  waitFor(filter, { signal: ctx.signal, timeout: 120_000 }).then(
    (mod) => {
      if (ctx.signal.aborted) return;
      try {
        cb(mod);
      } catch (err) {
        ctx.logger.error(`Couldn't hook ${what}`, err);
      }
    },
    () => {
      if (!ctx.signal.aborted) ctx.logger.warn(`Couldn't find ${what}; this plugin can't work on this Discord version`);
    },
  );
}

const storeCache = new Map<string, any>();
/** A Flux store by name (cached once found), or undefined. */
export function store(name: string): any {
  let s = storeCache.get(name);
  if (!s) {
    s = findStore(name);
    if (s) storeCache.set(name, s);
  }
  return s;
}

export const myId = (): string | undefined => common.getCurrentUser()?.id;
export const getUser = (id: string): any => store("UserStore")?.getUser?.(id);
export const getChannel = (id: string): any => store("ChannelStore")?.getChannel?.(id);
export const getGuild = (id: string): any => store("GuildStore")?.getGuild?.(id);
export const currentChannelId = (): string | undefined => store("SelectedChannelStore")?.getChannelId?.();
export const currentGuildId = (): string | undefined => store("SelectedGuildStore")?.getGuildId?.();
export const isFriend = (id: string): boolean => !!store("RelationshipStore")?.isFriend?.(id);
export const isBlocked = (id: string): boolean => !!store("RelationshipStore")?.isBlocked?.(id);
export const isIgnored = (id: string): boolean => !!store("RelationshipStore")?.isIgnored?.(id);
export const windowFocused = (): boolean => store("WindowStore")?.isFocused?.() ?? document.hasFocus();

/** 1 = DM, 3 = group DM. */
export function isDm(channelId: string): boolean {
  const type = getChannel(channelId)?.type;
  return type === 1 || type === 3;
}

/** A display name for a user object or id. */
export function userName(user: any): string {
  const u = typeof user === "string" ? getUser(user) : user;
  if (!u) return "Someone";
  const nick = u.id ? store("RelationshipStore")?.getNickname?.(u.id) : undefined;
  return nick ?? u.globalName ?? u.global_name ?? u.username ?? "Someone";
}

/** "#general in Server", "a DM", "a group chat". */
export function channelLabel(channelId: string): string {
  const ch = getChannel(channelId);
  if (!ch) return "a channel";
  if (ch.type === 1) return "a DM";
  if (ch.type === 3) return ch.name ? `the group "${ch.name}"` : "a group chat";
  const guild = ch.guild_id ? getGuild(ch.guild_id)?.name : undefined;
  return `#${ch.name ?? "channel"}${guild ? ` in ${guild}` : ""}`;
}

/** True when `message` mentions the current user directly. */
export function mentionsMe(message: any): boolean {
  const me = myId();
  return !!me && Array.isArray(message?.mentions) && message.mentions.some((m: any) => (m?.id ?? m) === me);
}

/* ------------------------------- sending -------------------------------- */

export interface SendInfo {
  channelId: string;
  /** True when an existing message is being edited. */
  edit: boolean;
  /** Discord's message object (content, tts, ...). */
  message: any;
}
/** Return a string to replace the content; return nothing to leave it alone. */
export type SendTransform = (content: string, info: SendInfo) => string | void;

const messageActions = filters.byProps("sendMessage", "editMessage");

/**
 * All send hooks from all plugins run through one pipeline, in a fixed order, so plugins behave
 * the same whichever were enabled first: guards see the text exactly as typed, then expansions,
 * link clean-up, wording, whitespace, and finally prefixes.
 */
interface SendHook {
  order: number;
  edits: boolean;
  /** A string replaces the content, `false` cancels the send, anything else leaves it alone. */
  run(content: string, info: SendInfo): string | void | false;
}
const sendHooks: SendHook[] = [];
let unhookSend: (() => void) | undefined;

function runSendHooks(channelId: string, message: any, edit: boolean): boolean {
  if (!message || typeof message.content !== "string") return true;
  for (const hook of [...sendHooks]) {
    if (edit && !hook.edits) continue;
    try {
      const out = hook.run(message.content, { channelId, edit, message });
      if (out === false) return false;
      if (typeof out === "string") message.content = out;
    } catch (err) {
      console.error("[Corrode] A send hook threw; skipping it", err);
    }
  }
  return true;
}

function addSendHook(ctx: Ctx, hook: SendHook) {
  withModule(ctx, messageActions, "Discord's message sender", (actions) => {
    sendHooks.push(hook);
    sendHooks.sort((x, y) => x.order - y.order);
    if (!unhookSend) {
      const unsend = patchInstead(actions, "sendMessage", (args, original) => (runSendHooks(args[0], args[1], false) ? original(...args) : Promise.resolve()), "corrode-send");
      const unedit = patchInstead(actions, "editMessage", (args, original) => (runSendHooks(args[0], args[2], true) ? original(...args) : Promise.resolve()), "corrode-send");
      unhookSend = () => {
        unsend();
        unedit();
        unhookSend = undefined;
      };
    }
    ctx.onStop(() => {
      const i = sendHooks.indexOf(hook);
      if (i >= 0) sendHooks.splice(i, 1);
      if (!sendHooks.length) unhookSend?.();
    });
  });
}

/**
 * Runs `transform` on every message the user sends (and on edits unless `edits` is false).
 * `order` places it among other plugins' transforms; lower runs earlier (see the list above).
 */
export function onSend(ctx: Ctx, transform: SendTransform, opts: { edits?: boolean; order?: number } = {}) {
  addSendHook(ctx, { order: opts.order ?? 50, edits: opts.edits !== false, run: transform });
}

/**
 * Lets a plugin stop a message from being sent: `allow` returns false to cancel. Guards run
 * before any transform, on the text exactly as typed. A cancelled send resolves quietly.
 */
export function guardSend(ctx: Ctx, allow: (content: string, info: SendInfo) => boolean, order = 0) {
  addSendHook(ctx, { order, edits: false, run: (content, info) => (allow(content, info) ? undefined : false) });
}

/** Applies `fn` to the parts of a message that are not code, links, mentions or emoji. */
export function mapPlainText(content: string, fn: (text: string) => string): string {
  const protectedParts = /(```[\s\S]*?```|`[^`\n]*`|https?:\/\/\S+|<a?:\w+:\d+>|<[@#][!&]?\d+>|<t:\d+(?::\w)?>|<\/[^>]+>)/g;
  return content
    .split(protectedParts)
    .map((part, i) => (i % 2 ? part : fn(part)))
    .join("");
}

/* --------------------------------- page --------------------------------- */

/** addEventListener that is removed when the plugin stops. */
export function listen<K extends keyof WindowEventMap>(
  ctx: Ctx,
  target: EventTarget,
  type: K | string,
  fn: (e: any) => void,
  options?: boolean | AddEventListenerOptions,
) {
  target.addEventListener(type, fn as EventListener, options);
  ctx.onStop(() => target.removeEventListener(type, fn as EventListener, options));
}

export function every(ctx: Ctx, ms: number, fn: () => void) {
  const t = setInterval(() => {
    try {
      fn();
    } catch (err) {
      ctx.logger.error("Timer callback threw", err);
    }
  }, ms);
  ctx.onStop(() => clearInterval(t));
}

/**
 * Calls `cb` once for every element matching `selector`, now and as Discord adds more.
 * `undo`, if given, runs for each handled element still on the page when the plugin stops.
 */
export function watch(ctx: Ctx, selector: string, cb: (el: HTMLElement) => void, undo?: (el: HTMLElement) => void) {
  const seen = new Set<HTMLElement>();
  let timer: ReturnType<typeof setTimeout> | undefined;
  const scan = () => {
    timer = undefined;
    if (ctx.signal.aborted) return;
    for (const el of document.querySelectorAll<HTMLElement>(selector)) {
      if (seen.has(el)) continue;
      seen.add(el);
      try {
        cb(el);
      } catch (err) {
        ctx.logger.error("watch callback threw", err);
      }
    }
    // Forget elements Discord removed so the set doesn't grow forever.
    if (seen.size > 2000) for (const el of seen) if (!el.isConnected) seen.delete(el);
  };
  const schedule = () => {
    if (timer === undefined) timer = setTimeout(scan, 120);
  };
  const observer = new MutationObserver(schedule);
  const begin = () => {
    if (ctx.signal.aborted) return;
    observer.observe(document.documentElement, { childList: true, subtree: true });
    scan();
  };
  if (document.body) begin();
  else document.addEventListener("DOMContentLoaded", begin, { once: true });
  ctx.onStop(() => {
    observer.disconnect();
    if (timer !== undefined) clearTimeout(timer);
    if (undo) for (const el of seen) if (el.isConnected) undo(el);
    seen.clear();
  });
}

/** Sets an attribute on <html> (for CSS that depends on a toggle); removed when the plugin stops. */
export function rootFlag(ctx: Ctx, name: string) {
  const root = document.documentElement;
  ctx.onStop(() => root.removeAttribute(name));
  return {
    get on() {
      return root.hasAttribute(name);
    },
    set(on: boolean) {
      if (on) root.setAttribute(name, "");
      else root.removeAttribute(name);
    },
    toggle() {
      this.set(!root.hasAttribute(name));
      return root.hasAttribute(name);
    },
  };
}

/* ------------------------------- shortcuts ------------------------------- */

interface Combo {
  mod: boolean;
  shift: boolean;
  alt: boolean;
  key: string;
}

/** Parses "ctrl+shift+k" (ctrl also means Cmd on macOS). Returns undefined for an empty or bad combo. */
export function parseCombo(text: string): Combo | undefined {
  const parts = String(text ?? "")
    .toLowerCase()
    .split("+")
    .map((p) => p.trim())
    .filter(Boolean);
  if (!parts.length) return undefined;
  const combo: Combo = { mod: false, shift: false, alt: false, key: "" };
  for (const p of parts) {
    if (p === "ctrl" || p === "cmd" || p === "meta" || p === "mod" || p === "control") combo.mod = true;
    else if (p === "shift") combo.shift = true;
    else if (p === "alt" || p === "option") combo.alt = true;
    else if (!combo.key) combo.key = p;
    else return undefined;
  }
  return combo.key ? combo : undefined;
}

export function matchesCombo(e: KeyboardEvent, combo: Combo): boolean {
  if ((e.ctrlKey || e.metaKey) !== combo.mod || e.shiftKey !== combo.shift || e.altKey !== combo.alt) return false;
  const key = e.key.toLowerCase();
  const code = e.code.toLowerCase();
  const k = combo.key;
  return key === k || code === k || code === `key${k}` || code === `digit${k}` || (k === "space" && key === " ") || (k === "esc" && key === "escape");
}

/** Runs `fn` when the shortcut returned by `combo()` is pressed (re-read on every key press). */
export function hotkey(ctx: Ctx, combo: () => string, fn: (e: KeyboardEvent) => void) {
  listen(
    ctx,
    window,
    "keydown",
    (e: KeyboardEvent) => {
      const c = parseCombo(combo());
      if (!c || e.repeat || !matchesCombo(e, c)) return;
      e.preventDefault();
      e.stopPropagation();
      fn(e);
    },
    true,
  );
}

/** True while the user is typing into a text field (so single-key shortcuts should stay out of the way). */
export function typingInField(e?: Event): boolean {
  const el = (e?.target as Element | null) ?? document.activeElement;
  if (!(el instanceof HTMLElement)) return false;
  return el.isContentEditable || /^(input|textarea|select)$/i.test(el.tagName) || el.getAttribute("role") === "textbox";
}

/* --------------------------------- misc ---------------------------------- */

export const escapeRegExp = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

/** Splits a comma or newline separated setting into trimmed, non-empty items. */
export const splitList = (s: unknown): string[] =>
  String(s ?? "")
    .split(/[\n,]/)
    .map((x) => x.trim())
    .filter(Boolean);

/** Only lets a CSS value through if it can't break out of its declaration. */
export const cssValue = (v: unknown, fallback: string): string => {
  const s = String(v ?? "").trim();
  return s && !/[;{}<>]/.test(s) ? s : fallback;
};

export const clamp = (n: unknown, min: number, max: number, fallback: number): number => {
  const v = Number(n);
  return Number.isFinite(v) ? Math.min(max, Math.max(min, v)) : fallback;
};

export async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    const ta = document.createElement("textarea");
    ta.value = text;
    ta.style.cssText = "position:fixed;opacity:0";
    document.body.appendChild(ta);
    ta.select();
    let ok = false;
    try {
      ok = document.execCommand("copy");
    } catch {}
    ta.remove();
    return ok;
  }
}

/** The text box of the current channel, if any. */
export const chatBox = (): HTMLElement | null => document.querySelector<HTMLElement>('[class*="channelTextArea"] [role="textbox"]');

/** Message ids from a chat row's element id (`chat-messages-<channel>-<message>`). */
export function rowIds(el: Element | null): { channelId: string; messageId: string } | undefined {
  const row = el?.closest?.('[id^="chat-messages-"]');
  const m = row?.id.match(/^chat-messages-(\d+)-(\d+)$/);
  return m ? { channelId: m[1], messageId: m[2] } : undefined;
}

/** Types `text` into the message box at the cursor. Falls back to the clipboard. */
export async function insertIntoChat(text: string): Promise<"inserted" | "copied" | "failed"> {
  const box = chatBox();
  if (box) {
    box.focus();
    try {
      if (document.execCommand("insertText", false, text)) return "inserted";
    } catch {}
  }
  return (await copyText(text)) ? "copied" : "failed";
}

/**
 * Calls `cb` for each chat message row with Discord's message object for it, as rows appear.
 * `undo` runs for rows still on the page when the plugin stops.
 */
export function watchMessages(ctx: Ctx, cb: (row: HTMLElement, message: any) => void, undo?: (row: HTMLElement) => void) {
  watch(
    ctx,
    'li[id^="chat-messages-"]',
    (row) => {
      const ids = rowIds(row);
      const message = ids && store("MessageStore")?.getMessage?.(ids.channelId, ids.messageId);
      if (message) cb(row, message);
    },
    undo,
  );
}

/** When a snowflake id was created. */
export const snowflakeDate = (id: string): Date => new Date(Number(BigInt(id) >> 22n) + 1420070400000);
