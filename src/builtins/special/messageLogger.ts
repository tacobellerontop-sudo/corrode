/**
 * Message Logger: keeps deleted messages in chat (marked), shows the older versions of edited
 * messages under them, saves both to a log you can search, and brings logged messages back into
 * the chat after a restart.
 *
 * How it works: MESSAGE_DELETE actions are stopped before Discord's MessageStore sees them, so the
 * message simply stays where it is; the row is then marked through the DOM. Edits are recorded by
 * reading the old content from the MessageStore just before a MESSAGE_UPDATE lands. Messages from
 * channels you haven't opened are remembered from MESSAGE_CREATE in a small cache, so deleting
 * those is logged too. After a restart, LOAD_MESSAGES_SUCCESS (Discord fetching history) gets the
 * logged messages put back in at the right place.
 */
import type { FluxAction } from "../../api/flux";
import { definePlugin } from "../../plugins/types";
import { h } from "../../ui/dom";
import { channelLabel, copyText, getChannel, hotkey, myId, splitList, store, userName, watch, type Ctx } from "../lib/helpers";
import { avatarUrl, channelPath, formatTime, navigate, openDialog, tabs } from "../lib/ui";

/* ------------------------------- snapshots ------------------------------- */

export interface Attachment {
  id?: string;
  filename: string;
  url: string;
  proxy_url?: string;
  content_type?: string;
  size?: number;
  width?: number;
  height?: number;
}

/** What we keep of a message: enough to show it in the log and to rebuild it in chat. */
export interface Snapshot {
  id: string;
  channelId: string;
  guildId?: string;
  author: { id: string; username: string; globalName?: string; avatar?: string | null; bot?: boolean; discriminator?: string };
  content: string;
  attachments: Attachment[];
  /** ISO time the message was sent. */
  sentAt: string;
  editedAt?: string;
  type: number;
  flags: number;
  reference?: { message_id?: string; channel_id?: string; guild_id?: string };
  embeds: number;
}

export interface DeletedEntry extends Snapshot {
  deletedAt: number;
}

export interface EditEntry {
  id: string;
  channelId: string;
  guildId?: string;
  author: Snapshot["author"];
  sentAt: string;
  /** Older versions, oldest first. `until` is when it was replaced. */
  versions: { content: string; until: number }[];
  /** The newest content we have seen. */
  latest: string;
}

const iso = (t: any): string | undefined => {
  if (t == null || t === "") return undefined;
  if (typeof t?.toISOString === "function") {
    try {
      return t.toISOString();
    } catch {
      /* invalid date */
    }
  }
  const d = new Date(typeof t === "object" && typeof t.valueOf === "function" ? t.valueOf() : t);
  return Number.isNaN(d.getTime()) ? undefined : d.toISOString();
};

/** Snowflake → ISO time, for messages without a timestamp. */
const idTime = (id: string) => {
  try {
    return new Date(Number(BigInt(id) >> 22n) + 1420070400000).toISOString();
  } catch {
    return new Date(0).toISOString();
  }
};

/**
 * Turns a message (Discord's MessageStore record or a raw message from the API/gateway) into a
 * plain snapshot. Returns undefined for anything that isn't a usable message.
 */
export function snapshotMessage(m: any, channelIdHint?: string): Snapshot | undefined {
  const id = m?.id;
  const author = m?.author;
  if (typeof id !== "string" || !author?.id) return undefined;
  const channelId = String(m.channel_id ?? m.channelId ?? channelIdHint ?? "");
  if (!channelId) return undefined;
  const ref = m.message_reference ?? m.messageReference;
  return {
    id,
    channelId,
    guildId: m.guild_id ?? m.guildId ?? getChannel(channelId)?.guild_id ?? undefined,
    author: {
      id: String(author.id),
      username: String(author.username ?? "unknown"),
      globalName: author.globalName ?? author.global_name ?? undefined,
      avatar: author.avatar ?? null,
      bot: !!author.bot,
      discriminator: author.discriminator ?? "0",
    },
    content: typeof m.content === "string" ? m.content : "",
    attachments: (Array.isArray(m.attachments) ? m.attachments : [])
      .filter((a: any) => a?.url)
      .map((a: any) => ({
        id: a.id,
        filename: String(a.filename ?? a.title ?? "file"),
        url: String(a.url),
        proxy_url: a.proxy_url ?? a.proxyURL ?? undefined,
        content_type: a.content_type ?? a.contentType ?? undefined,
        size: a.size,
        width: a.width ?? undefined,
        height: a.height ?? undefined,
      })),
    sentAt: iso(m.timestamp) ?? idTime(id),
    editedAt: iso(m.edited_timestamp ?? m.editedTimestamp),
    type: Number(m.type) || 0,
    flags: Number(m.flags) || 0,
    reference: ref && (ref.message_id ?? ref.messageId) ? { message_id: ref.message_id ?? ref.messageId, channel_id: ref.channel_id ?? ref.channelId, guild_id: ref.guild_id ?? ref.guildId } : undefined,
    embeds: Array.isArray(m.embeds) ? m.embeds.length : 0,
  };
}

/** Rebuilds a message in the shape Discord's API returns, so Discord can show it again. */
export function toRawMessage(s: Snapshot): Record<string, any> {
  return {
    id: s.id,
    channel_id: s.channelId,
    ...(s.guildId ? { guild_id: s.guildId } : {}),
    type: s.type,
    content: s.content,
    author: { id: s.author.id, username: s.author.username, global_name: s.author.globalName ?? null, avatar: s.author.avatar ?? null, discriminator: s.author.discriminator ?? "0", bot: s.author.bot || undefined },
    attachments: s.attachments.map((a) => ({ ...a, id: a.id ?? s.id, proxy_url: a.proxy_url ?? a.url, size: a.size ?? 0 })),
    embeds: [],
    mentions: [],
    mention_roles: [],
    mention_everyone: false,
    pinned: false,
    tts: false,
    timestamp: s.sentAt,
    edited_timestamp: s.editedAt ?? null,
    flags: s.flags,
    components: [],
    ...(s.reference ? { message_reference: s.reference } : {}),
  };
}

const cmpId = (a: string, b: string): number => {
  const x = BigInt(a);
  const y = BigInt(b);
  return x < y ? -1 : x > y ? 1 : 0;
};

/**
 * Puts logged messages back into a page of history Discord just fetched (newest first, like the
 * API). A logged message goes in when it falls inside the page, or past its end when there is
 * nothing more to fetch on that side.
 */
export function mergeRestored(
  messages: any[],
  logged: Snapshot[],
  page: { hasMoreBefore?: boolean; hasMoreAfter?: boolean; isBefore?: boolean; isAfter?: boolean } = {},
): any[] {
  if (!logged.length) return messages;
  const have = new Set(messages.map((m) => m?.id));
  const ids = messages.map((m) => m?.id).filter((id): id is string => typeof id === "string");
  const sorted = [...ids].sort(cmpId);
  const min = sorted[0];
  const max = sorted[sorted.length - 1];
  // A plain load (not paging) is the newest page, so anything newer than it belongs there too.
  const newestPage = page.isBefore ? false : page.isAfter ? page.hasMoreAfter === false : page.hasMoreAfter !== true;
  const oldestPage = page.hasMoreBefore === false;
  const extra = logged.filter((s) => {
    if (have.has(s.id)) return false;
    if (!min) return newestPage;
    const afterMin = cmpId(s.id, min) > 0;
    const beforeMax = cmpId(s.id, max) < 0;
    return (afterMin && beforeMax) || (!beforeMax && newestPage) || (!afterMin && oldestPage);
  });
  if (!extra.length) return messages;
  return [...messages, ...extra.map(toRawMessage)].sort((a, b) => cmpId(b.id, a.id));
}

/* --------------------------------- plugin -------------------------------- */

const PASS = "__corrodeLoggerPass";

const STYLES = {
  red: `[data-corrode-deleted] { background: rgba(242, 63, 67, .08) !important; box-shadow: inset 2px 0 0 #f23f43; }
[data-corrode-deleted] [id^="message-content-"], [data-corrode-deleted] [id^="message-accessories-"] { color: #f23f43 !important; }`,
  faded: `[data-corrode-deleted] [class*="contents"], [data-corrode-deleted] [id^="message-accessories-"] { opacity: .5; }`,
  strike: `[data-corrode-deleted] [id^="message-content-"] { text-decoration: line-through; text-decoration-color: #f23f43; opacity: .75; }`,
} as const;

const BASE_CSS = `
.corrode-ml-tag { display: inline-flex; align-items: center; gap: 4px; margin-left: 6px; padding: 0 6px; border-radius: 4px; font-size: 11px; font-weight: 600; line-height: 16px; vertical-align: middle;
  color: #fff; background: #da373c; user-select: none; }
.corrode-ml-tag button { all: unset; cursor: pointer; opacity: .8; }
.corrode-ml-tag button:hover { opacity: 1; }
.corrode-ml-edits { margin: 2px 0; }
.corrode-ml-edit { color: var(--text-muted, #949ba4); font-size: .9em; white-space: pre-wrap; word-break: break-word; user-select: text; }
.corrode-ml-edit::after { content: attr(data-when); margin-left: 6px; font-size: 10px; opacity: .8; }
.corrode-ml-edits-toggle { all: unset; cursor: pointer; font-size: 11px; color: var(--text-muted, #949ba4); }
.corrode-ml-edits-toggle:hover { text-decoration: underline; }
`;

export default definePlugin({
  id: "messageLogger",
  name: "Message Logger",
  description:
    "Keeps deleted messages in chat (marked in red), shows what edited messages said before, and saves both to a searchable log (Ctrl+Shift+L). Logged messages come back after a restart.",
  authors: ["Corrode"],
  settings: {
    keepOnScreen: { type: "boolean", label: "Keep deleted messages in chat", description: "Off: they disappear as usual but are still saved to the log.", default: true },
    deletedStyle: {
      type: "select",
      label: "How deleted messages look",
      default: "red",
      options: [
        { label: "Red", value: "red" },
        { label: "Faded", value: "faded" },
        { label: "Struck through", value: "strike" },
      ],
    },
    showEdits: {
      type: "select",
      label: "Earlier versions of edited messages",
      default: "inline",
      options: [
        { label: "Show them above the message", value: "inline" },
        { label: "Show a \"show edits\" link", value: "collapsed" },
        { label: "Only in the log", value: "off" },
      ],
    },
    restore: { type: "boolean", label: "Bring logged messages back after a restart", description: "Puts deleted messages back into the chat when Discord loads history.", default: true },
    scope: {
      type: "select",
      label: "Log messages in",
      default: "all",
      options: [
        { label: "Servers and DMs", value: "all" },
        { label: "Servers only", value: "servers" },
        { label: "DMs and group chats only", value: "dms" },
      ],
    },
    logOwn: { type: "boolean", label: "Also log my own messages", description: "Your own deleted messages are logged but never kept in chat.", default: false },
    logBots: { type: "boolean", label: "Also log bots", default: false },
    unopened: { type: "boolean", label: "Also catch channels I haven't opened", description: "Remembers recent messages Discord sends you in the background so deleting them is logged too.", default: true },
    ignoreUsers: { type: "string", label: "Ignore these user IDs", description: "Comma separated.", default: "" },
    ignoreChannels: { type: "string", label: "Ignore these channel IDs", description: "Comma separated.", default: "" },
    ignoreServers: { type: "string", label: "Ignore these server IDs", description: "Comma separated.", default: "" },
    limit: { type: "number", label: "Messages to keep in the log", default: 1000, min: 50, max: 10000, step: 50 },
    shortcut: { type: "string", label: "Shortcut to open the log", default: "ctrl+shift+l" },
  },
  actions: [
    { label: "Open log", run: (ctx) => (ctx as any).__openLog?.() },
    { label: "Clear log", run: (ctx) => (ctx as any).__clearLog?.() },
  ],
  onSettingChange(_key, _value, ctx) {
    (ctx as any).__onSetting?.();
  },
  start(ctx) {
    const logger = new MessageLog(ctx);
    (ctx as any).__openLog = () => logger.openViewer();
    (ctx as any).__clearLog = () => logger.clear();
    logger.start();
  },
});

type Settings = {
  keepOnScreen: boolean;
  deletedStyle: keyof typeof STYLES;
  showEdits: "inline" | "collapsed" | "off";
  restore: boolean;
  scope: "all" | "servers" | "dms";
  logOwn: boolean;
  logBots: boolean;
  unopened: boolean;
  ignoreUsers: string;
  ignoreChannels: string;
  ignoreServers: string;
  limit: number;
  shortcut: string;
};

const CACHE_SIZE = 3000;

export class MessageLog {
  readonly deleted = new Map<string, DeletedEntry>();
  readonly edits = new Map<string, EditEntry>();
  /** Message ids being kept on screen (their delete was stopped). */
  readonly kept = new Set<string>();
  /** Recent messages from the gateway, for channels Discord doesn't keep in MessageStore. */
  private readonly cache = new Map<string, Snapshot>();
  private saveTimer: ReturnType<typeof setTimeout> | undefined;

  constructor(private readonly ctx: Ctx) {}

  private get s(): Settings {
    return this.ctx.settings as Settings;
  }

  start() {
    const { ctx } = this;
    for (const e of ctx.storage.get("deleted", []) as DeletedEntry[]) if (e?.id) this.deleted.set(e.id, e);
    for (const e of ctx.storage.get("edits", []) as EditEntry[]) if (e?.id) this.edits.set(e.id, e);

    const sheet = ctx.css.inject("");
    const renderCss = () => sheet.update(`${BASE_CSS}\n${STYLES[this.s.deletedStyle] ?? STYLES.red}`);
    renderCss();

    ctx.flux.intercept((action) => this.handle(action));
    watch(
      ctx,
      'li[id^="chat-messages-"]',
      (row) => this.decorate(row),
      (row) => this.undecorate(row),
    );
    hotkey(ctx, () => String(this.s.shortcut), () => this.openViewer());
    ctx.onStop(() => this.flush());
    // Settings that change how things look: redraw what's on screen.
    (ctx as any).__onSetting = () => {
      renderCss();
      for (const row of document.querySelectorAll<HTMLElement>('li[id^="chat-messages-"]')) this.decorate(row);
    };
  }

  /** Runs before Discord's stores see each Flux action. Returns true to stop the action. */
  handle(action: FluxAction): boolean | void {
    if (action[PASS]) return;
    switch (action.type) {
      case "MESSAGE_CREATE":
        if (!action.optimistic && this.s.unopened) this.remember(action.message, action.channelId);
        return;
      case "MESSAGE_UPDATE":
        this.onUpdate(action.message);
        return;
      case "MESSAGE_DELETE":
        return this.onDelete(action.channelId, action.id);
      case "MESSAGE_DELETE_BULK": {
        if (!Array.isArray(action.ids)) return;
        action.ids = action.ids.filter((id: string) => !this.onDelete(action.channelId, id));
        return action.ids.length === 0;
      }
      case "LOAD_MESSAGES_SUCCESS":
        this.onLoad(action);
        return;
    }
  }

  /* ------------------------------- events -------------------------------- */

  private remember(message: any, channelId?: string) {
    const snap = snapshotMessage(message, channelId);
    if (!snap) return;
    this.cache.delete(snap.id);
    this.cache.set(snap.id, snap);
    if (this.cache.size > CACHE_SIZE) this.cache.delete(this.cache.keys().next().value!);
  }

  private find(channelId: string, id: string): { snap?: Snapshot; inStore: boolean } {
    const record = store("MessageStore")?.getMessage?.(channelId, id);
    if (record) return { snap: snapshotMessage(record, channelId), inStore: true };
    return { snap: this.cache.get(id), inStore: false };
  }

  shouldLog(snap: Snapshot): boolean {
    const s = this.s;
    const me = myId();
    if (snap.author.id === me && !s.logOwn) return false;
    if (snap.author.bot && !s.logBots) return false;
    const guildId = snap.guildId ?? getChannel(snap.channelId)?.guild_id;
    if (s.scope === "servers" && !guildId) return false;
    if (s.scope === "dms" && guildId) return false;
    if (splitList(s.ignoreUsers).includes(snap.author.id)) return false;
    if (splitList(s.ignoreChannels).includes(snap.channelId)) return false;
    const parent = getChannel(snap.channelId)?.parent_id;
    if (parent && splitList(s.ignoreChannels).includes(parent)) return false;
    if (guildId && splitList(s.ignoreServers).includes(guildId)) return false;
    return true;
  }

  /** Returns true when the delete should be stopped (the message stays in chat). */
  onDelete(channelId: string, id: string): boolean {
    if (!channelId || !id) return false;
    if (this.kept.has(id)) return true; // deleted again (e.g. a bulk delete after a single one)
    const { snap, inStore } = this.find(channelId, id);
    if (!snap || !this.shouldLog(snap)) return false;
    this.deleted.delete(id);
    this.deleted.set(id, { ...snap, deletedAt: Date.now() });
    this.cache.delete(id);
    this.save();
    const keep = inStore && this.s.keepOnScreen && snap.author.id !== myId();
    if (keep) {
      this.kept.add(id);
      this.refresh(channelId, id);
    }
    return keep;
  }

  onUpdate(message: any) {
    const id = message?.id;
    const channelId = message?.channel_id ?? message?.channelId;
    if (!id || !channelId || typeof message.content !== "string") return;
    const { snap } = this.find(channelId, id);
    const cached = this.cache.get(id);
    if (cached) cached.content = message.content;
    if (!snap || snap.content === message.content || !this.shouldLog(snap)) return;
    const entry: EditEntry = this.edits.get(id) ?? { id, channelId, guildId: snap.guildId, author: snap.author, sentAt: snap.sentAt, versions: [], latest: snap.content };
    entry.versions.push({ content: snap.content, until: Date.now() });
    if (entry.versions.length > 25) entry.versions.splice(0, entry.versions.length - 25);
    entry.latest = message.content;
    this.edits.delete(id);
    this.edits.set(id, entry);
    // A message deleted after an edit should show its last text in the log.
    const del = this.deleted.get(id);
    if (del) del.content = message.content;
    this.save();
    this.refresh(channelId, id);
  }

  private onLoad(action: FluxAction) {
    if (!this.s.restore || !Array.isArray(action.messages) || !action.channelId) return;
    const logged = [...this.deleted.values()].filter((e) => e.channelId === action.channelId && this.shouldLog(e));
    if (!logged.length) return;
    try {
      action.messages = mergeRestored(action.messages, logged, action as any);
      for (const e of logged) if (action.messages.some((m: any) => m.id === e.id)) this.kept.add(e.id);
    } catch (err) {
      this.ctx.logger.error("Couldn't restore logged messages", err);
    }
  }

  /* ------------------------------- storage ------------------------------- */

  private save() {
    if (this.saveTimer !== undefined) return;
    this.saveTimer = setTimeout(() => this.flush(), 1000);
  }

  flush() {
    if (this.saveTimer !== undefined) clearTimeout(this.saveTimer);
    this.saveTimer = undefined;
    const limit = Math.max(50, Number(this.s.limit) || 1000);
    const trim = <V>(m: Map<string, V>) => {
      while (m.size > limit) m.delete(m.keys().next().value!);
    };
    trim(this.deleted);
    trim(this.edits);
    this.ctx.storage.set("deleted", [...this.deleted.values()]);
    this.ctx.storage.set("edits", [...this.edits.values()]);
  }

  clear() {
    this.deleted.clear();
    this.edits.clear();
    this.flush();
    for (const row of document.querySelectorAll<HTMLElement>("[data-corrode-deleted], [data-corrode-edited]")) this.decorate(row);
    this.ctx.toast("Message log cleared. Messages kept in chat stay until you switch channels.");
  }

  /** Removes a deleted message from chat for real and forgets it. */
  dismiss(channelId: string, id: string) {
    this.deleted.delete(id);
    this.kept.delete(id);
    this.save();
    try {
      this.ctx.flux.dispatch({ type: "MESSAGE_DELETE", channelId, id, [PASS]: true });
    } catch (err) {
      this.ctx.logger.error("Couldn't remove the message", err);
    }
  }

  forget(id: string) {
    this.deleted.delete(id);
    this.edits.delete(id);
    this.save();
  }

  /* --------------------------------- DOM --------------------------------- */

  private refresh(channelId: string, id: string) {
    // Let Discord render the change first.
    setTimeout(() => {
      const row = document.getElementById(`chat-messages-${channelId}-${id}`);
      if (row) this.decorate(row);
    }, 50);
  }

  decorate(row: HTMLElement) {
    const m = row.id.match(/^chat-messages-(\d+)-(\d+)$/);
    if (!m) return;
    const [, channelId, id] = m;
    const content = row.querySelector<HTMLElement>(`#message-content-${id}`);
    const anchor = content ?? row.querySelector<HTMLElement>('[class*="contents"]');

    const del = this.deleted.get(id);
    const showDeleted = !!del && this.kept.has(id);
    row.toggleAttribute("data-corrode-deleted", showDeleted);
    let tag = row.querySelector<HTMLElement>(":scope .corrode-ml-tag");
    if (showDeleted && anchor) {
      if (!tag) {
        tag = h(
          "span",
          { class: "corrode-ml-tag", title: `Deleted ${new Date(del!.deletedAt).toLocaleString()}` },
          `deleted ${formatTime(del!.deletedAt)}`,
          h("button", { type: "button", "aria-label": "Remove from chat", title: "Remove from chat", onClick: () => this.dismiss(channelId, id) }, "✕"),
        );
        if (content) content.after(tag);
        else anchor.append(tag);
      }
    } else tag?.remove();

    const edit = this.edits.get(id);
    const mode = this.s.showEdits;
    let block = row.querySelector<HTMLElement>(":scope .corrode-ml-edits");
    const want = edit && edit.versions.length && mode !== "off" && content ? `${mode}:${edit.versions.length}` : "";
    row.toggleAttribute("data-corrode-edited", !!want);
    if (!want) return void block?.remove();
    if (block?.dataset.state === want) return;
    block?.remove();
    const lines = edit!.versions.map((v) => h("div", { class: "corrode-ml-edit", "data-when": `before ${formatTime(v.until)}` }, v.content || "(empty)"));
    block = h("div", { class: "corrode-ml-edits", "data-state": want });
    if (mode === "collapsed") {
      const list = h("div", { hidden: true }, lines);
      const n = lines.length;
      const btn = h(
        "button",
        {
          class: "corrode-ml-edits-toggle",
          type: "button",
          onClick: () => {
            list.hidden = !list.hidden;
            btn.textContent = list.hidden ? `show ${n} earlier version${n > 1 ? "s" : ""}` : "hide earlier versions";
          },
        },
        `show ${n} earlier version${n > 1 ? "s" : ""}`,
      );
      block.append(btn, list);
    } else block.append(...lines);
    content!.before(block);
  }

  undecorate(row: HTMLElement) {
    row.removeAttribute("data-corrode-deleted");
    row.removeAttribute("data-corrode-edited");
    row.querySelector(":scope .corrode-ml-tag")?.remove();
    row.querySelector(":scope .corrode-ml-edits")?.remove();
  }

  /* -------------------------------- viewer ------------------------------- */

  openViewer() {
    this.flush();
    const dlg = openDialog(this.ctx, "Message log", { width: 760 });
    let tab: "deleted" | "edits" = "deleted";
    let query = "";
    let here = false;
    const search = h("input", { type: "text", placeholder: "Search text, names, channels…", style: "flex:1", "aria-label": "Search the log" });
    search.addEventListener("input", () => {
      query = search.value.trim().toLowerCase();
      render();
    });
    const scope = h(
      "select",
      { "aria-label": "Which channels", onChange: (e: Event) => ((here = (e.target as HTMLSelectElement).value === "here"), render()) },
      h("option", { value: "all" }, "Everywhere"),
      h("option", { value: "here" }, "This channel"),
    );
    const tabBar = h("div");
    const drawTabs = () =>
      tabBar.replaceChildren(
        tabs(
          [
            { value: "deleted", label: `Deleted (${this.deleted.size})` },
            { value: "edits", label: `Edited (${this.edits.size})` },
          ],
          tab,
          (v) => ((tab = v), render()),
        ),
      );
    const clear = h(
      "button",
      {
        class: "corrode-dlg-btn danger",
        onClick: () => {
          if (clear.dataset.armed) {
            this.clear();
            drawTabs();
            render();
            delete clear.dataset.armed;
            clear.textContent = "Clear log";
          } else {
            clear.dataset.armed = "1";
            clear.textContent = "Click again to clear";
            setTimeout(() => {
              delete clear.dataset.armed;
              clear.textContent = "Clear log";
            }, 3000);
          }
        },
      },
      "Clear log",
    );
    dlg.tools.append(tabBar, search, scope, clear);

    const current = () => store("SelectedChannelStore")?.getChannelId?.();
    const matches = (e: { channelId: string; author: Snapshot["author"] }, text: string) => {
      if (here && e.channelId !== current()) return false;
      if (!query) return true;
      return `${text}\n${e.author.username}\n${e.author.globalName ?? ""}\n${channelLabel(e.channelId)}`.toLowerCase().includes(query);
    };
    const head = (e: { id: string; channelId: string; guildId?: string; author: Snapshot["author"] }, when: string, text: string, onForget: () => void) =>
      h(
        "div",
        { class: "corrode-dlg-row-head" },
        h("img", { src: avatarUrl(e.author, 32), width: 20, height: 20, style: "border-radius:50%;align-self:center", alt: "" }),
        h("b", null, e.author.globalName || userName(e.author.id) || e.author.username),
        h("span", { class: "corrode-dlg-muted" }, `${channelLabel(e.channelId)} · ${when}`),
        h(
          "div",
          { class: "corrode-dlg-row-actions" },
          h("button", { class: "corrode-dlg-btn sec", onClick: () => (dlg.close(), navigate(channelPath(e.channelId, e.guildId ?? getChannel(e.channelId)?.guild_id, e.id))) }, "Jump"),
          h("button", { class: "corrode-dlg-btn sec", onClick: () => void copyText(text).then((ok) => this.ctx.toast(ok ? "Copied" : "Couldn't copy")) }, "Copy"),
          h("button", { class: "corrode-dlg-btn sec", onClick: onForget }, "Forget"),
        ),
      );

    const render = () => {
      drawTabs();
      const rows: HTMLElement[] = [];
      if (tab === "deleted") {
        const list = [...this.deleted.values()].reverse().filter((e) => matches(e, e.content));
        for (const e of list.slice(0, 300)) {
          rows.push(
            h(
              "div",
              { class: "corrode-dlg-row" },
              head(e, `sent ${formatTime(e.sentAt)}, deleted ${formatTime(e.deletedAt)}`, e.content, () => (this.forget(e.id), render())),
              e.content ? h("div", { class: "corrode-dlg-text" }, e.content) : h("div", { class: "corrode-dlg-muted" }, e.embeds ? "(embed)" : "(no text)"),
              e.attachments.length > 0 &&
                h(
                  "div",
                  { style: "margin-top:6px;display:flex;flex-direction:column;gap:2px" },
                  e.attachments.map((a) => h("a", { href: a.url, target: "_blank", rel: "noreferrer noopener" }, `📎 ${a.filename}`)),
                ),
            ),
          );
        }
        if (list.length > 300) rows.push(h("div", { class: "corrode-dlg-empty" }, `…and ${list.length - 300} older. Search to narrow it down.`));
      } else {
        const list = [...this.edits.values()].reverse().filter((e) => matches(e, [...e.versions.map((v) => v.content), e.latest].join("\n")));
        for (const e of list.slice(0, 300)) {
          rows.push(
            h(
              "div",
              { class: "corrode-dlg-row" },
              head(e, `sent ${formatTime(e.sentAt)}, edited ${e.versions.length}×`, e.latest, () => (this.forget(e.id), render())),
              e.versions.map((v) => h("div", { class: "corrode-dlg-old", title: `Replaced ${new Date(v.until).toLocaleString()}` }, v.content || "(empty)")),
              h("div", { class: "corrode-dlg-text" }, e.latest || "(empty)"),
            ),
          );
        }
        if (list.length > 300) rows.push(h("div", { class: "corrode-dlg-empty" }, `…and ${list.length - 300} older. Search to narrow it down.`));
      }
      if (!rows.length)
        rows.push(
          h(
            "div",
            { class: "corrode-dlg-empty" },
            query || here ? "Nothing matches." : tab === "deleted" ? "No deleted messages logged yet." : "No edited messages logged yet.",
          ),
        );
      dlg.body.replaceChildren(...rows);
    };
    render();
    search.focus();
  }
}
