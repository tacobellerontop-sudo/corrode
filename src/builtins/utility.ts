/** Utility, shortcut, sound and privacy plugins. */
import { getNative } from "../native";
import { definePlugin, type PluginDefinition } from "../plugins/types";
import { channelLabel, clamp, copyText, currentChannelId, every, guardSend, hotkey, isDm, listen, mentionsMe, myId, onSend, store, userName, watch, windowFocused, type Ctx } from "./lib/helpers";
import { parsePairs } from "./sending";

/* ------------------------------ pure helpers ----------------------------- */

const TRACKING_URL = /\/api\/v\d+\/(science|metrics|track)\b|sentry\.io\/|\/error-reporting-proxy\//;
export const isTrackingUrl = (url: unknown): boolean => TRACKING_URL.test(String(url ?? ""));

const REAL_HOSTS = /(^|\.)(discord\.com|discord\.gg|discordapp\.com|discordapp\.net|discord\.media|discord\.new|discord\.gift|discord\.dev|discord\.co|discordstatus\.com|dis\.gd|steamcommunity\.com|steampowered\.com|steamstatic\.com)$/i;
const LOOKALIKES = [/d[il1|][s5z]+c[o0]+r[dcl]+/i, /d[il1|]sc[o0]r?d?-?(app|gift|nitro|give)/i, /st[e3]+a?m+-?c[o0]m+un[il1|y]+t/i, /n[il1]tr[o0]-?(gift|free|drop)/i, /(gift|free)-?n[il1]tr[o0]/i];
/** True for a host that imitates Discord or Steam without being one of their real domains. */
export function suspiciousHost(host: string): boolean {
  const h = host.toLowerCase().replace(/^www\./, "");
  return !REAL_HOSTS.test(h) && LOOKALIKES.some((r) => r.test(h));
}
/** The first imitation host linked in `text`, if any. */
export function findScamHost(text: string): string | undefined {
  for (const [link] of String(text ?? "").matchAll(/https?:\/\/[^\s<>)]+/g)) {
    try {
      const host = new URL(link).hostname;
      if (suspiciousHost(host)) return host;
    } catch {}
  }
  return undefined;
}

const DURATION_UNITS: Record<string, number> = { s: 1, m: 60, h: 3600, d: 86400 };
/** Parses "!remind 1h30m take a break" into a delay in seconds and the text. */
export function parseReminder(content: string, command = "!remind"): { seconds: number; text: string } | undefined {
  if (!content.toLowerCase().startsWith(`${command.toLowerCase()} `)) return undefined;
  const m = content.slice(command.length).trim().match(/^((?:\d+\s*[smhd]\s*)+)([\s\S]*)$/i);
  if (!m) return undefined;
  let seconds = 0;
  for (const [, n, unit] of m[1].matchAll(/(\d+)\s*([smhd])/gi)) seconds += Number(n) * DURATION_UNITS[unit.toLowerCase()];
  return seconds > 0 ? { seconds, text: m[2].trim() || "Reminder" } : undefined;
}

const dayKey = (d = new Date()) => `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;

function speak(text: string, rate: number, volume: number) {
  try {
    const u = new SpeechSynthesisUtterance(text);
    u.rate = clamp(rate, 0.5, 3, 1);
    u.volume = clamp(volume, 0, 100, 80) / 100;
    speechSynthesis.speak(u);
  } catch {}
}
const speakable = (text: unknown) =>
  String(text ?? "")
    .replace(/```[\s\S]*?```/g, " code block ")
    .replace(/https?:\/\/\S+/g, " link ")
    .replace(/<a?:(\w+):\d+>/g, " $1 ")
    .replace(/<[@#][!&]?\d+>/g, " mention ")
    .replace(/[*_~`|>]/g, "")
    .slice(0, 300);

function panel(ctx: Ctx, css: string): HTMLDivElement {
  const el = document.createElement("div");
  el.setAttribute("data-corrode-float", ctx.id);
  el.style.cssText = `position:fixed;z-index:2147482000;font:14px/1.4 var(--font-primary,system-ui,sans-serif);color:var(--text-normal,#dbdee1);${css}`;
  document.body?.appendChild(el);
  ctx.onStop(() => el.remove());
  return el;
}

export const utilityPlugins: PluginDefinition<any>[] = [
  /* --------------------------------- privacy -------------------------------- */
  definePlugin({
    id: "noTrack",
    name: "No Tracking",
    description: "Blocks the analytics and crash reports Discord's app sends about what you click and do.",
    authors: ["Corrode"],
    start(ctx) {
      let blocked = 0;
      const flagged = new WeakSet<XMLHttpRequest>();
      ctx.patcher.before(XMLHttpRequest.prototype, "open", (args, self) => {
        if (isTrackingUrl(args[1])) flagged.add(self);
      });
      ctx.patcher.instead(XMLHttpRequest.prototype, "send", (args, original, self) => {
        if (!flagged.has(self)) return original(...args);
        blocked++;
        // Let the caller finish quietly instead of waiting forever.
        setTimeout(() => self.dispatchEvent(new ProgressEvent("error")));
      });
      ctx.patcher.instead(window, "fetch", (args, original) => {
        const url = typeof args[0] === "string" ? args[0] : (args[0]?.url ?? String(args[0]));
        if (!isTrackingUrl(url)) return original(...args);
        blocked++;
        return Promise.resolve(new Response(null, { status: 204 }));
      });
      if (typeof navigator.sendBeacon === "function") {
        ctx.patcher.instead(navigator, "sendBeacon", (args, original) => {
          if (!isTrackingUrl(args[0])) return original(...args);
          blocked++;
          return true;
        });
      }
      ctx.onStop(() => ctx.logger.info(`Blocked ${blocked} tracking request(s) this session`));
    },
  }),
  definePlugin({
    id: "scamWarning",
    name: "Scam Link Warning",
    description: "Warns you about links that imitate Discord or Steam addresses, a common way accounts get stolen.",
    authors: ["Corrode"],
    css: 'a[data-corrode-scam] { color: #f23f43 !important; text-decoration: line-through !important; }\na[data-corrode-scam]::after { content: " ⚠ looks like a scam"; font-weight: 700; }',
    start(ctx) {
      watch(
        ctx,
        '[id^="message-content-"] a[href], article[class*="embed"] a[href]',
        (a) => {
          try {
            if (suspiciousHost(new URL((a as HTMLAnchorElement).href).hostname)) a.setAttribute("data-corrode-scam", "");
          } catch {}
        },
        (a) => a.removeAttribute("data-corrode-scam"),
      );
      ctx.flux.subscribe("MESSAGE_CREATE", (action) => {
        const m = action.message;
        if (action.optimistic || !m?.author || m.author.id === myId()) return;
        if (!isDm(m.channel_id) && m.channel_id !== currentChannelId()) return;
        const host = findScamHost(m.content);
        if (host) ctx.toast(`${userName(m.author)} sent a link to ${host}, which imitates a real site. Don't open it or log in there.`, { title: "Possible scam", kind: "error", duration: 15_000 });
      });
    },
  }),
  definePlugin({
    id: "noAutoIdle",
    name: "Stay Online",
    description: "Stops Discord from switching you to Idle when you haven't touched it for a while.",
    authors: ["Corrode"],
    start(ctx) {
      ctx.flux.intercept((action) => (action.type === "IDLE" && action.idle === true) || (action.type === "AFK" && action.afk === true));
    },
  }),

  /* --------------------------------- sounds --------------------------------- */
  definePlugin({
    id: "customPingSound",
    name: "Custom Ping Sound",
    description: "Plays a sound of your choice when you are mentioned or get a DM.",
    authors: ["Corrode"],
    settings: {
      url: { type: "string", label: "Sound link", description: "A direct https link to an mp3, ogg or wav file.", placeholder: "https://example.com/ping.mp3", default: "" },
      volume: { type: "number", label: "Volume (%)", default: 60, min: 0, max: 100 },
      dms: { type: "boolean", label: "Play for DMs", default: true },
      mentions: { type: "boolean", label: "Play for mentions", default: true },
      focused: { type: "boolean", label: "Also play while I'm looking at that conversation", default: false },
    },
    start(ctx) {
      let lastPlayed = 0;
      ctx.flux.subscribe("MESSAGE_CREATE", (action) => {
        const m = action.message;
        const url = String(ctx.settings.url ?? "").trim();
        if (action.optimistic || !m?.author || m.author.id === myId() || !/^https:\/\//.test(url)) return;
        const dm = !m.guild_id && isDm(m.channel_id);
        if (!((ctx.settings.dms && dm) || (ctx.settings.mentions && mentionsMe(m)))) return;
        if (!ctx.settings.focused && m.channel_id === currentChannelId() && windowFocused()) return;
        if (Date.now() - lastPlayed < 1500) return;
        lastPlayed = Date.now();
        const audio = new Audio(url);
        audio.volume = clamp(ctx.settings.volume, 0, 100, 60) / 100;
        audio.play().catch((err) => ctx.logger.warn("Couldn't play the sound", err));
      });
    },
  }),
  definePlugin({
    id: "readAloud",
    name: "Read Messages Aloud",
    description: "Speaks incoming messages using your computer's text-to-speech voice.",
    authors: ["Corrode"],
    settings: {
      scope: {
        type: "select",
        label: "Read",
        default: "current",
        options: [
          { label: "The conversation I have open", value: "current" },
          { label: "DMs", value: "dms" },
          { label: "Messages that mention me", value: "mentions" },
        ],
      },
      names: { type: "boolean", label: "Say who wrote it", default: true },
      rate: { type: "number", label: "Speed", default: 1, min: 0.5, max: 3, step: 0.1 },
      volume: { type: "number", label: "Volume (%)", default: 80, min: 0, max: 100 },
    },
    start(ctx) {
      if (typeof speechSynthesis === "undefined") return void ctx.logger.warn("This webview has no text-to-speech");
      ctx.onStop(() => speechSynthesis.cancel());
      ctx.flux.subscribe("MESSAGE_CREATE", (action) => {
        const m = action.message;
        if (action.optimistic || !m?.author || m.author.id === myId() || m.author.bot || !String(m.content ?? "").trim()) return;
        const scope = ctx.settings.scope;
        if (scope === "current" && m.channel_id !== currentChannelId()) return;
        if (scope === "dms" && (m.guild_id || !isDm(m.channel_id))) return;
        if (scope === "mentions" && !mentionsMe(m)) return;
        speak(`${ctx.settings.names ? `${userName(m.author)} says: ` : ""}${speakable(m.content)}`, Number(ctx.settings.rate), Number(ctx.settings.volume));
      });
    },
  }),
  definePlugin({
    id: "voiceNarrator",
    name: "Voice Channel Narrator",
    description: "Announces out loud who joins and leaves the voice channel you are in.",
    authors: ["Corrode"],
    settings: {
      rate: { type: "number", label: "Speed", default: 1.1, min: 0.5, max: 3, step: 0.1 },
      volume: { type: "number", label: "Volume (%)", default: 70, min: 0, max: 100 },
    },
    start(ctx) {
      if (typeof speechSynthesis === "undefined") return void ctx.logger.warn("This webview has no text-to-speech");
      ctx.onStop(() => speechSynthesis.cancel());
      ctx.flux.subscribe("VOICE_STATE_UPDATES", (action) => {
        const mine = store("SelectedChannelStore")?.getVoiceChannelId?.();
        if (!mine) return;
        for (const v of action.voiceStates ?? []) {
          if (!v?.userId || v.userId === myId() || v.channelId === v.oldChannelId) continue;
          const say = v.channelId === mine ? "joined" : v.oldChannelId === mine ? "left" : "";
          if (say) speak(`${userName(v.userId)} ${say}`, Number(ctx.settings.rate), Number(ctx.settings.volume));
        }
      });
    },
  }),

  /* -------------------------------- shortcuts ------------------------------- */
  definePlugin({
    id: "reloadKey",
    name: "Reload Key",
    description: "Press a key to reload Discord, as F5 does in a browser.",
    authors: ["Corrode"],
    settings: { shortcut: { type: "string", label: "Shortcut", default: "f5" } },
    start: (ctx) => hotkey(ctx, () => String(ctx.settings.shortcut), () => location.reload()),
  }),
  definePlugin({
    id: "devtoolsKey",
    name: "DevTools Key",
    description: "Press a key to open the developer tools.",
    authors: ["Corrode"],
    settings: { shortcut: { type: "string", label: "Shortcut", default: "f12" } },
    start(ctx) {
      hotkey(ctx, () => String(ctx.settings.shortcut), () => {
        const open = getNative().openDevTools;
        if (open) open();
        else ctx.toast("This client can't open the developer tools from a shortcut", { kind: "warning" });
      });
    },
  }),
  definePlugin({
    id: "fullscreenKey",
    name: "Fullscreen Key",
    description: "Press a key to make Discord fill the screen, and again to go back.",
    authors: ["Corrode"],
    settings: { shortcut: { type: "string", label: "Shortcut", default: "f11" } },
    start(ctx) {
      hotkey(ctx, () => String(ctx.settings.shortcut), () => {
        const done = document.fullscreenElement ? document.exitFullscreen() : document.documentElement.requestFullscreen();
        done.catch((err) => ctx.logger.warn("Fullscreen failed", err));
      });
    },
  }),
  definePlugin({
    id: "zoomKeys",
    name: "Zoom Keys",
    description: "Ctrl + and Ctrl - make everything larger or smaller, Ctrl 0 resets. The zoom level is remembered.",
    authors: ["Corrode"],
    start(ctx) {
      const sheet = ctx.css.inject("");
      const apply = (zoom: number) => {
        const z = Math.min(2.5, Math.max(0.5, Math.round(zoom * 20) / 20));
        ctx.storage.set("zoom", z);
        sheet.update(z === 1 ? "" : `html { zoom: ${z} !important; }`);
        return z;
      };
      apply(Number(ctx.storage.get("zoom", 1)) || 1);
      listen(
        ctx,
        window,
        "keydown",
        (e: KeyboardEvent) => {
          if (!(e.ctrlKey || e.metaKey) || e.altKey) return;
          const current = Number(ctx.storage.get("zoom", 1)) || 1;
          const next = e.key === "=" || e.key === "+" ? current + 0.1 : e.key === "-" || e.key === "_" ? current - 0.1 : e.key === "0" ? 1 : undefined;
          if (next === undefined) return;
          e.preventDefault();
          e.stopPropagation();
          ctx.toast(`Zoom ${Math.round(apply(next) * 100)}%`, { duration: 1000 });
        },
        true,
      );
    },
  }),
  definePlugin({
    id: "copyChannelLink",
    name: "Copy Channel Link",
    description: "Press a shortcut to copy a link to the channel or conversation you are in.",
    authors: ["Corrode"],
    settings: { shortcut: { type: "string", label: "Shortcut", default: "ctrl+shift+l" } },
    start(ctx) {
      hotkey(ctx, () => String(ctx.settings.shortcut), async () => {
        if (!/^\/channels\//.test(location.pathname)) return void ctx.toast("Open a channel first", { duration: 2000 });
        ctx.toast((await copyText(`https://discord.com${location.pathname}`)) ? `Copied a link to ${channelLabel(currentChannelId() ?? "")}` : "Couldn't copy the link", { duration: 2000 });
      });
    },
  }),
  definePlugin({
    id: "scratchpad",
    name: "Scratchpad",
    description: "Press a shortcut to open a small notepad that stays on top of Discord. What you write is saved.",
    authors: ["Corrode"],
    settings: { shortcut: { type: "string", label: "Shortcut", default: "ctrl+shift+n" } },
    start(ctx) {
      let box: HTMLDivElement | undefined;
      const close = () => {
        box?.remove();
        box = undefined;
      };
      ctx.onStop(close);
      hotkey(ctx, () => String(ctx.settings.shortcut), () => {
        if (box) return close();
        box = panel(ctx, "right:24px;top:64px;width:320px;padding:10px;border-radius:10px;background:var(--background-floating,#111214);box-shadow:0 8px 24px rgba(0,0,0,.4)");
        const area = document.createElement("textarea");
        area.value = String(ctx.storage.get("text", ""));
        area.placeholder = "Notes…";
        area.style.cssText = "width:100%;height:220px;box-sizing:border-box;resize:vertical;border:0;outline:0;border-radius:6px;padding:8px;font:inherit;color:inherit;background:var(--input-background,#1e1f22)";
        area.addEventListener("input", () => ctx.storage.set("text", area.value));
        // Keep Discord's own shortcuts from firing while typing notes.
        for (const type of ["keydown", "keyup", "keypress"]) area.addEventListener(type, (e) => (e as KeyboardEvent).key !== "Escape" && e.stopPropagation());
        area.addEventListener("keydown", (e) => e.key === "Escape" && close());
        box.appendChild(area);
        area.focus();
      });
    },
  }),

  /* --------------------------------- helpers -------------------------------- */
  definePlugin({
    id: "reminders",
    name: "Reminders",
    description: 'Type "!remind 20m take the pizza out" in any message box. Nothing is sent; you get a notice when the time is up, even after a restart.',
    authors: ["Corrode"],
    start(ctx) {
      type Reminder = { at: number; text: string };
      const load = (): Reminder[] => ctx.storage.get("list", []) ?? [];
      guardSend(ctx, (content) => {
        const r = parseReminder(content);
        if (!r) return true;
        ctx.storage.set("list", [...load(), { at: Date.now() + r.seconds * 1000, text: r.text }]);
        ctx.toast(`I'll remind you at ${new Date(Date.now() + r.seconds * 1000).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}: ${r.text}`);
        return false;
      });
      const check = () => {
        const list = load();
        const due = list.filter((r) => r.at <= Date.now());
        if (!due.length) return;
        ctx.storage.set("list", list.filter((r) => r.at > Date.now()));
        for (const r of due) ctx.toast(r.text, { title: "Reminder", kind: "success", duration: 0 });
      };
      every(ctx, 5000, check);
      check();
    },
  }),
  definePlugin({
    id: "breakReminder",
    name: "Break Reminder",
    description: "Reminds you to take a break after you've had Discord open for a while.",
    authors: ["Corrode"],
    settings: {
      minutes: { type: "number", label: "Remind me every (minutes)", default: 60, min: 5, max: 480, step: 5 },
      message: { type: "string", label: "Message", default: "Time for a short break. Stand up, stretch, drink some water." },
    },
    start(ctx) {
      let last = Date.now();
      every(ctx, 30_000, () => {
        if (Date.now() - last < clamp(ctx.settings.minutes, 5, 480, 60) * 60_000) return;
        last = Date.now();
        ctx.toast(String(ctx.settings.message), { duration: 15_000 });
      });
    },
  }),
  definePlugin({
    id: "birthdays",
    name: "Birthday Reminders",
    description: "Reminds you on the birthdays you save here.",
    authors: ["Corrode"],
    settings: { list: { type: "string", multiline: true, label: "Birthdays", description: "One per line: name = MM-DD.", placeholder: "Alex = 03-14", default: "" } },
    start(ctx) {
      const check = () => {
        const today = dayKey();
        const mmdd = today.slice(5);
        for (const line of String(ctx.settings.list ?? "").split("\n")) {
          const [name, date] = line.split("=").map((x) => x.trim());
          const m = date?.match(/^(\d{1,2})[-/](\d{1,2})$/);
          if (!name || !m || `${m[1].padStart(2, "0")}-${m[2].padStart(2, "0")}` !== mmdd) continue;
          const key = `told:${name.toLowerCase()}`;
          if (ctx.storage.get(key) === today) continue;
          ctx.storage.set(key, today);
          ctx.toast(`It's ${name}'s birthday today 🎂`, { duration: 0 });
        }
      };
      every(ctx, 10 * 60_000, check);
      (this as any).check = check;
      check();
    },
    onSettingChange() {
      // Re-check straight away so a birthday added for today shows up.
      (this as any).check?.();
    },
  }),
  definePlugin({
    id: "dailyStats",
    name: "Daily Stats",
    description: "Counts the messages and words you send each day. Press the shortcut to see today's and yesterday's totals.",
    authors: ["Corrode"],
    settings: { shortcut: { type: "string", label: "Shortcut", default: "ctrl+shift+s" } },
    start(ctx) {
      type Day = { messages: number; words: number };
      const get = (key: string): Day => ctx.storage.get(key, { messages: 0, words: 0 }) ?? { messages: 0, words: 0 };
      onSend(
        ctx,
        (content) => {
          const today = `day:${dayKey()}`;
          const d = get(today);
          ctx.storage.set(today, { messages: d.messages + 1, words: d.words + (content.trim() ? content.trim().split(/\s+/).length : 0) });
          // Keep two weeks of history.
          for (const k of ctx.storage.keys().filter((k) => k.startsWith("day:")).sort().slice(0, -14)) ctx.storage.delete(k);
        },
        { edits: false, order: 90 },
      );
      hotkey(ctx, () => String(ctx.settings.shortcut), () => {
        const t = get(`day:${dayKey()}`);
        const y = get(`day:${dayKey(new Date(Date.now() - 86_400_000))}`);
        ctx.toast(`Today: ${t.messages} messages, ${t.words} words. Yesterday: ${y.messages} messages, ${y.words} words.`, { duration: 8000 });
      });
    },
  }),
];
