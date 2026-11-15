/**
 * Plugins driven by Discord's event stream (Flux): notices about things that happen, and filters
 * that change or drop events before Discord's own stores see them.
 */
import { definePlugin, type PluginDefinition } from "../plugins/types";
import type { FluxAction } from "../api/flux";
import { channelLabel, currentChannelId, escapeRegExp, getChannel, getGuild, getUser, hotkey, isBlocked, isDm, isFriend, isIgnored, mentionsMe, myId, splitList, store, userName, windowFocused, type Ctx } from "./lib/helpers";

/** Runs `fn` for one action type before Discord's stores see it. Return true to drop the action. */
function before(ctx: Ctx, type: string | string[], fn: (action: FluxAction) => boolean | void) {
  const types = new Set(Array.isArray(type) ? type : [type]);
  ctx.flux.intercept((action) => (types.has(action.type) ? fn(action) : undefined));
}

/** True at most once per `ms` for each key; stops a burst of events from stacking toasts. */
function limiter(ms: number) {
  const last = new Map<string, number>();
  return (key: string) => {
    const now = Date.now();
    if (now - (last.get(key) ?? 0) < ms) return false;
    last.set(key, now);
    if (last.size > 500) for (const [k, t] of last) if (now - t > ms) last.delete(k);
    return true;
  };
}

const preview = (text: unknown, max = 140) => {
  const s = String(text ?? "").replace(/\s+/g, " ").trim();
  return s.length > max ? `${s.slice(0, max - 1)}…` : s;
};
const authorId = (message: any): string | undefined => message?.author?.id;
const ids = (setting: unknown) => new Set(splitList(setting));

/** Keeps only the messages `keep` accepts, in live messages and in loaded history. */
function filterMessages(ctx: Ctx, keep: (message: any) => boolean) {
  before(ctx, ["MESSAGE_CREATE", "MESSAGE_UPDATE"], (action) => {
    if (action.optimistic || !action.message?.author) return;
    if (authorId(action.message) === myId()) return;
    return !keep(action.message);
  });
  before(ctx, "LOAD_MESSAGES_SUCCESS", (action) => {
    if (Array.isArray(action.messages)) action.messages = action.messages.filter((m: any) => authorId(m) === myId() || keep(m));
  });
}

/** Lets a plugin change incoming messages, live and in loaded history. */
function mutateMessages(ctx: Ctx, change: (message: any) => void) {
  before(ctx, ["MESSAGE_CREATE", "MESSAGE_UPDATE"], (action) => {
    if (action.message && !action.optimistic) change(action.message);
  });
  before(ctx, "LOAD_MESSAGES_SUCCESS", (action) => {
    if (Array.isArray(action.messages)) for (const m of action.messages) change(m);
  });
}

const SYSTEM_TYPES: Record<string, number[]> = {
  joins: [7],
  boosts: [8, 9, 10, 11],
  pins: [6],
  threads: [18, 21],
  calls: [3],
  follows: [12],
};

export const eventPlugins: PluginDefinition<any>[] = [
  /* -------------------------------- notices -------------------------------- */
  definePlugin({
    id: "friendNotify",
    name: "Friend Online Notices",
    description: "Tells you when a friend comes online.",
    authors: ["Corrode"],
    settings: {
      only: { type: "string", label: "Only these user IDs", description: "Comma separated. Empty means every friend.", default: "" },
      offline: { type: "boolean", label: "Also tell me when they go offline", default: false },
    },
    start(ctx) {
      const allow = limiter(30_000);
      before(ctx, "PRESENCE_UPDATES", (action) => {
        const presence = store("PresenceStore");
        const only = ids(ctx.settings.only);
        for (const u of action.updates ?? []) {
          const id = u?.user?.id;
          if (!id || id === myId() || (only.size ? !only.has(id) : !isFriend(id))) continue;
          const was = presence?.getStatus?.(id) ?? "offline";
          const now = u.status ?? "offline";
          if (was === "offline" && now !== "offline" && allow(`on:${id}`)) ctx.toast(`${userName(id)} is online`);
          else if (ctx.settings.offline && was !== "offline" && now === "offline" && allow(`off:${id}`)) ctx.toast(`${userName(id)} went offline`);
        }
      });
    },
  }),
  definePlugin({
    id: "gameNotify",
    name: "Friend Activity Notices",
    description: "Tells you when a friend starts playing a game.",
    authors: ["Corrode"],
    settings: { only: { type: "string", label: "Only these user IDs", description: "Comma separated. Empty means every friend.", default: "" } },
    start(ctx) {
      const playing = new Map<string, string>();
      const allow = limiter(60_000);
      before(ctx, "PRESENCE_UPDATES", (action) => {
        const only = ids(ctx.settings.only);
        for (const u of action.updates ?? []) {
          const id = u?.user?.id;
          if (!id || id === myId() || (only.size ? !only.has(id) : !isFriend(id))) continue;
          const game = (u.activities ?? []).find((a: any) => a?.type === 0)?.name ?? "";
          const was = playing.get(id);
          playing.set(id, game);
          if (game && was !== undefined && was !== game && allow(`${id}:${game}`)) ctx.toast(`${userName(id)} started playing ${game}`);
        }
      });
    },
  }),
  definePlugin({
    id: "streamNotify",
    name: "Stream Notices",
    description: "Tells you when a friend starts sharing their screen in a voice channel you can see.",
    authors: ["Corrode"],
    start(ctx) {
      const live = new Set<string>();
      ctx.flux.subscribe("VOICE_STATE_UPDATES", (action) => {
        for (const v of action.voiceStates ?? []) {
          if (!v?.userId || v.userId === myId()) continue;
          const streaming = !!v.selfStream && !!v.channelId;
          if (streaming && !live.has(v.userId) && isFriend(v.userId)) ctx.toast(`${userName(v.userId)} went live in ${channelLabel(v.channelId)}`);
          if (streaming) live.add(v.userId);
          else live.delete(v.userId);
        }
      });
    },
  }),
  definePlugin({
    id: "relationshipNotify",
    name: "Unfriend And Removal Notices",
    description: "Tells you when a friend removes you, or when you are removed from a server.",
    authors: ["Corrode"],
    settings: {
      friends: { type: "boolean", label: "Friends removing me", default: true },
      servers: { type: "boolean", label: "Being removed from servers", default: true },
    },
    start(ctx) {
      let leaving = 0;
      // Removals the user did themselves are not news.
      before(ctx, ["GUILD_LEAVE", "RELATIONSHIP_REMOVE_START", "LEAVE_GUILD_MODAL_SUBMIT"], () => void (leaving = Date.now()));
      before(ctx, "RELATIONSHIP_REMOVE", (action) => {
        const r = action.relationship;
        if (ctx.settings.friends && r?.type === 1 && Date.now() - leaving > 5000) ctx.toast(`${userName(r.id)} is no longer your friend`, { kind: "warning", duration: 10_000 });
      });
      before(ctx, "GUILD_DELETE", (action) => {
        const g = action.guild;
        if (!ctx.settings.servers || !g?.id || g.unavailable || Date.now() - leaving < 5000) return;
        ctx.toast(`You are no longer in ${getGuild(g.id)?.name ?? "a server"}`, { kind: "warning", duration: 10_000 });
      });
    },
  }),
  definePlugin({
    id: "requestNotify",
    name: "Friend Request Notices",
    description: "Shows a notice when someone sends you a friend request.",
    authors: ["Corrode"],
    flux: {
      RELATIONSHIP_ADD(action, ctx) {
        const r = action.relationship;
        if (r?.type === 3) ctx.toast(`${userName(r.user ?? r.id)} sent you a friend request`, { duration: 8000 });
      },
    },
  }),
  definePlugin({
    id: "voiceNotify",
    name: "Voice Join And Leave Notices",
    description: "Shows who joins or leaves the voice channel you are in.",
    authors: ["Corrode"],
    start(ctx) {
      ctx.flux.subscribe("VOICE_STATE_UPDATES", (action) => {
        const mine = store("SelectedChannelStore")?.getVoiceChannelId?.();
        if (!mine) return;
        for (const v of action.voiceStates ?? []) {
          if (!v?.userId || v.userId === myId() || v.channelId === v.oldChannelId) continue;
          if (v.channelId === mine) ctx.toast(`${userName(v.userId)} joined`, { duration: 2500 });
          else if (v.oldChannelId === mine) ctx.toast(`${userName(v.userId)} left`, { duration: 2500 });
        }
      });
    },
  }),
  definePlugin({
    id: "voiceAlerts",
    name: "Voice Moderation Alerts",
    description: "Tells you when someone server-mutes or deafens you, or moves you to another voice channel.",
    authors: ["Corrode"],
    start(ctx) {
      let last: { mute: boolean; deaf: boolean; channelId?: string } | undefined;
      let myMove = 0;
      before(ctx, ["VOICE_CHANNEL_SELECT"], () => void (myMove = Date.now()));
      ctx.flux.subscribe("VOICE_STATE_UPDATES", (action) => {
        const v = (action.voiceStates ?? []).find((x: any) => x?.userId === myId());
        if (!v) return;
        if (last) {
          if (v.mute && !last.mute) ctx.toast("You were muted by a moderator", { kind: "warning" });
          if (!v.mute && last.mute) ctx.toast("You are no longer server muted");
          if (v.deaf && !last.deaf) ctx.toast("You were deafened by a moderator", { kind: "warning" });
          if (!v.deaf && last.deaf) ctx.toast("You are no longer server deafened");
          if (last.channelId && v.channelId && v.channelId !== last.channelId && Date.now() - myMove > 3000) ctx.toast(`You were moved to ${channelLabel(v.channelId)}`, { kind: "warning" });
          if (last.channelId && !v.channelId && Date.now() - myMove > 3000) ctx.toast("You were disconnected from voice", { kind: "warning" });
        }
        last = { mute: !!v.mute, deaf: !!v.deaf, channelId: v.channelId ?? undefined };
      });
    },
  }),
  definePlugin({
    id: "reactionNotify",
    name: "Reaction Notices",
    description: "Tells you when someone reacts to one of your messages.",
    authors: ["Corrode"],
    start(ctx) {
      const allow = limiter(3000);
      ctx.flux.subscribe("MESSAGE_REACTION_ADD", (action) => {
        const me = myId();
        if (!me || action.userId === me || action.optimistic) return;
        const author = action.messageAuthorId ?? store("MessageStore")?.getMessage?.(action.channelId, action.messageId)?.author?.id;
        if (author !== me || !allow(`${action.messageId}:${action.userId}`)) return;
        const emoji = action.emoji?.id ? `:${action.emoji.name}:` : (action.emoji?.name ?? "a reaction");
        ctx.toast(`${userName(action.userId)} reacted ${emoji} in ${channelLabel(action.channelId)}`);
      });
    },
  }),
  definePlugin({
    id: "dmTypingNotify",
    name: "DM Typing Notices",
    description: "Tells you when someone starts typing to you in a DM you don't have open.",
    authors: ["Corrode"],
    start(ctx) {
      const allow = limiter(60_000);
      ctx.flux.subscribe("TYPING_START", (action) => {
        if (!action.userId || action.userId === myId() || !isDm(action.channelId)) return;
        if (action.channelId === currentChannelId() && windowFocused()) return;
        if (allow(`${action.channelId}:${action.userId}`)) ctx.toast(`${userName(action.userId)} is typing to you…`, { duration: 3000 });
      });
    },
  }),
  definePlugin({
    id: "ghostPing",
    name: "Ghost Ping Detector",
    description: "Tells you when a message that mentioned you is deleted, and what it said.",
    authors: ["Corrode"],
    start(ctx) {
      before(ctx, "MESSAGE_DELETE", (action) => {
        const msg = store("MessageStore")?.getMessage?.(action.channelId, action.id);
        if (!msg || msg.author?.id === myId() || !(msg.mentioned || mentionsMe(msg))) return;
        ctx.toast(preview(msg.content) || "(no text)", { title: `Ghost ping from ${userName(msg.author)} in ${channelLabel(action.channelId)}`, kind: "warning", duration: 12_000 });
      });
    },
  }),
  definePlugin({
    id: "deleteNotify",
    name: "Deleted Message Notices",
    description: "Shows what a message said when someone deletes it.",
    authors: ["Corrode"],
    settings: {
      scope: {
        type: "select",
        label: "Watch",
        default: "dms",
        options: [
          { label: "DMs only", value: "dms" },
          { label: "The channel I have open", value: "current" },
          { label: "Everywhere", value: "all" },
        ],
      },
      bots: { type: "boolean", label: "Include bots", default: false },
    },
    start(ctx) {
      before(ctx, "MESSAGE_DELETE", (action) => {
        const scope = ctx.settings.scope;
        if (scope === "dms" && !isDm(action.channelId)) return;
        if (scope === "current" && action.channelId !== currentChannelId()) return;
        const msg = store("MessageStore")?.getMessage?.(action.channelId, action.id);
        if (!msg || msg.author?.id === myId() || (msg.author?.bot && !ctx.settings.bots)) return;
        ctx.toast(preview(msg.content) || "(attachment or embed)", { title: `${userName(msg.author)} deleted a message in ${channelLabel(action.channelId)}`, duration: 8000 });
      });
    },
  }),
  definePlugin({
    id: "editNotify",
    name: "Edited Message Notices",
    description: "Shows what a message said before it was edited.",
    authors: ["Corrode"],
    settings: {
      scope: {
        type: "select",
        label: "Watch",
        default: "dms",
        options: [
          { label: "DMs only", value: "dms" },
          { label: "The channel I have open", value: "current" },
        ],
      },
    },
    start(ctx) {
      before(ctx, "MESSAGE_UPDATE", (action) => {
        const m = action.message;
        if (!m?.id || typeof m.content !== "string" || !m.edited_timestamp) return;
        const channelId = m.channel_id;
        if (ctx.settings.scope === "dms" ? !isDm(channelId) : channelId !== currentChannelId()) return;
        const old = store("MessageStore")?.getMessage?.(channelId, m.id);
        if (!old || old.author?.id === myId() || old.author?.bot || old.content === m.content) return;
        ctx.toast(`Before: ${preview(old.content, 200) || "(empty)"}`, { title: `${userName(old.author)} edited a message in ${channelLabel(channelId)}`, duration: 8000 });
      });
    },
  }),
  definePlugin({
    id: "userWatch",
    name: "User Watch",
    description: "Tells you when chosen people send a message anywhere you can see.",
    authors: ["Corrode"],
    settings: {
      users: { type: "string", label: "User IDs", description: "Comma separated. Right-click a user and Copy User ID (needs Developer Mode).", default: "" },
      skipOpen: { type: "boolean", label: "Skip the channel I'm looking at", default: true },
    },
    flux: {
      MESSAGE_CREATE(action, ctx) {
        const m = action.message;
        if (action.optimistic || !m?.author || !ids(ctx.settings.users).has(m.author.id)) return;
        if (ctx.settings.skipOpen && m.channel_id === currentChannelId() && windowFocused()) return;
        ctx.toast(preview(m.content) || "(attachment or embed)", { title: `${userName(m.author)} in ${channelLabel(m.channel_id)}`, duration: 7000 });
      },
    },
  }),
  definePlugin({
    id: "channelWatch",
    name: "Channel Watch",
    description: "Shows a notice for every message in chosen channels, even muted ones.",
    authors: ["Corrode"],
    settings: {
      channels: { type: "string", label: "Channel IDs", description: "Comma separated. Right-click a channel and Copy Channel ID (needs Developer Mode).", default: "" },
      bots: { type: "boolean", label: "Include bots", default: true },
    },
    flux: {
      MESSAGE_CREATE(action, ctx) {
        const m = action.message;
        if (action.optimistic || !m?.author || m.author.id === myId() || !ids(ctx.settings.channels).has(m.channel_id)) return;
        if ((m.author.bot && !ctx.settings.bots) || (m.channel_id === currentChannelId() && windowFocused())) return;
        ctx.toast(preview(m.content) || "(attachment or embed)", { title: `${userName(m.author)} in ${channelLabel(m.channel_id)}`, duration: 7000 });
      },
    },
  }),
  definePlugin({
    id: "mentionHistory",
    name: "Mention History",
    description: "Remembers the last messages that mentioned you and shows them again with a shortcut.",
    authors: ["Corrode"],
    settings: {
      shortcut: { type: "string", label: "Shortcut", default: "ctrl+shift+m" },
      keep: { type: "number", label: "How many to remember", default: 25, min: 5, max: 200 },
    },
    start(ctx) {
      type Entry = { who: string; where: string; text: string; at: number };
      ctx.flux.subscribe("MESSAGE_CREATE", (action) => {
        const m = action.message;
        if (action.optimistic || !m?.author || m.author.id === myId() || !mentionsMe(m)) return;
        const list: Entry[] = ctx.storage.get("mentions", []) ?? [];
        list.unshift({ who: userName(m.author), where: channelLabel(m.channel_id), text: preview(m.content, 200), at: Date.now() });
        ctx.storage.set("mentions", list.slice(0, Math.max(5, Number(ctx.settings.keep) || 25)));
      });
      hotkey(
        ctx,
        () => String(ctx.settings.shortcut),
        () => {
          const list: Entry[] = ctx.storage.get("mentions", []) ?? [];
          if (!list.length) return void ctx.toast("Nobody has mentioned you since this plugin was turned on.");
          for (const e of list.slice(0, 5).reverse()) ctx.toast(e.text || "(no text)", { title: `${e.who} in ${e.where}, ${new Date(e.at).toLocaleString()}`, duration: 12_000 });
        },
      );
    },
  }),
  definePlugin({
    id: "memberJoinNotify",
    name: "Member Join Notices",
    description: "Tells you when someone joins or leaves chosen servers. Discord only sends this for some servers.",
    authors: ["Corrode"],
    settings: {
      servers: { type: "string", label: "Server IDs", description: "Comma separated.", default: "" },
      leaves: { type: "boolean", label: "Also show people leaving", default: true },
    },
    flux: {
      GUILD_MEMBER_ADD(action, ctx) {
        if (ids(ctx.settings.servers).has(action.guildId)) ctx.toast(`${userName(action.user)} joined ${getGuild(action.guildId)?.name ?? "the server"}`);
      },
      GUILD_MEMBER_REMOVE(action, ctx) {
        if (ctx.settings.leaves && action.user?.id !== myId() && ids(ctx.settings.servers).has(action.guildId)) ctx.toast(`${userName(action.user)} left ${getGuild(action.guildId)?.name ?? "the server"}`);
      },
    },
  }),
  definePlugin({
    id: "nicknameWatch",
    name: "Nickname Change Notices",
    description: "Tells you when a friend changes their nickname in a server you share.",
    authors: ["Corrode"],
    start(ctx) {
      before(ctx, "GUILD_MEMBER_UPDATE", (action) => {
        const id = action.user?.id;
        if (!id || id === myId() || !isFriend(id)) return;
        const old = store("GuildMemberStore")?.getMember?.(action.guildId, id);
        if (!old || (old.nick ?? null) === (action.nick ?? null)) return;
        ctx.toast(`${userName(id)} is now "${action.nick ?? userName(id)}" in ${getGuild(action.guildId)?.name ?? "a server"}`);
      });
    },
  }),
  definePlugin({
    id: "threadNotify",
    name: "New Thread Notices",
    description: "Tells you when a thread or forum post is created in your servers.",
    authors: ["Corrode"],
    settings: { servers: { type: "string", label: "Only these server IDs", description: "Comma separated. Empty means every server.", default: "" } },
    flux: {
      THREAD_CREATE(action, ctx) {
        const ch = action.channel;
        if (!ch?.name || action.isNewlyCreated === false || ch.ownerId === myId() || ch.owner_id === myId()) return;
        const only = ids(ctx.settings.servers);
        if (only.size && !only.has(ch.guild_id)) return;
        ctx.toast(`New thread "${ch.name}" in ${channelLabel(ch.parent_id)}`);
      },
    },
  }),
  definePlugin({
    id: "channelCreateNotify",
    name: "New Channel Notices",
    description: "Tells you when a channel is created in one of your servers.",
    authors: ["Corrode"],
    flux: {
      CHANNEL_CREATE(action, ctx) {
        const ch = action.channel;
        if (!ch?.guild_id || !ch.name || ch.type === 4) return;
        ctx.toast(`New channel #${ch.name} in ${getGuild(ch.guild_id)?.name ?? "a server"}`);
      },
    },
  }),
  definePlugin({
    id: "eventNotify",
    name: "Server Event Notices",
    description: "Tells you when a scheduled event is created in one of your servers.",
    authors: ["Corrode"],
    flux: {
      GUILD_SCHEDULED_EVENT_CREATE(action, ctx) {
        const e = action.guildScheduledEvent;
        if (e?.name) ctx.toast(`New event "${e.name}" in ${getGuild(e.guild_id)?.name ?? "a server"}`, { duration: 8000 });
      },
    },
  }),
  definePlugin({
    id: "connectionNotify",
    name: "Connection Notices",
    description: "Tells you when Discord loses its connection and when it comes back.",
    authors: ["Corrode"],
    start(ctx) {
      let down = false;
      ctx.flux.subscribe("CONNECTION_CLOSED", () => {
        if (!down) ctx.toast("Discord lost its connection", { kind: "warning" });
        down = true;
      });
      const up = () => {
        if (down) ctx.toast("Discord is connected again", { kind: "success" });
        down = false;
      };
      ctx.flux.subscribe("CONNECTION_RESUMED", up);
      ctx.flux.subscribe("CONNECTION_OPEN", up);
    },
  }),
  definePlugin({
    id: "dmNotify",
    name: "DM Notices",
    description: "Shows an in-app notice for every DM you receive in a conversation you don't have open.",
    authors: ["Corrode"],
    flux: {
      MESSAGE_CREATE(action, ctx) {
        const m = action.message;
        if (action.optimistic || !m?.author || m.author.id === myId() || m.guild_id || !isDm(m.channel_id)) return;
        if (m.channel_id === currentChannelId() && windowFocused()) return;
        ctx.toast(preview(m.content) || "(attachment or embed)", { title: userName(m.author), duration: 6000 });
      },
    },
  }),
  definePlugin({
    id: "replyNotify",
    name: "Reply Notices",
    description: "Tells you when someone replies to one of your messages, even if they turned the reply ping off.",
    authors: ["Corrode"],
    flux: {
      MESSAGE_CREATE(action, ctx) {
        const m = action.message;
        const me = myId();
        if (action.optimistic || !m?.author || m.author.id === me || m.referenced_message?.author?.id !== me || mentionsMe(m)) return;
        if (m.channel_id === currentChannelId() && windowFocused()) return;
        ctx.toast(preview(m.content) || "(attachment or embed)", { title: `${userName(m.author)} replied in ${channelLabel(m.channel_id)}`, duration: 7000 });
      },
    },
  }),

  /* -------------------------------- filters -------------------------------- */
  definePlugin({
    id: "keepDeleted",
    name: "Keep Deleted Messages",
    description: "Keeps messages on screen after other people delete them, marked in red, until you restart or reload.",
    authors: ["Corrode"],
    settings: {
      bots: { type: "boolean", label: "Also keep messages from bots", default: false },
      notify: { type: "boolean", label: "Show a notice when something is deleted", default: false },
    },
    start(ctx) {
      const kept: string[] = [];
      const sheet = ctx.css.inject("");
      const render = () => {
        const rows = kept.map((k) => `#chat-messages-${k}`).join(",\n");
        sheet.update(rows ? `${rows} { background: rgba(240, 71, 71, .08) !important; box-shadow: inset 2px 0 0 #f04747; }\n${kept.map((k) => `#chat-messages-${k} [id^="message-content-"]`).join(",\n")} { color: #f04747 !important; }` : "");
      };
      const keep = (channelId: string, id: string) => {
        const msg = store("MessageStore")?.getMessage?.(channelId, id);
        // Unknown messages aren't on screen anyway; our own deletions should just happen.
        if (!msg || msg.author?.id === myId() || (msg.author?.bot && !ctx.settings.bots)) return false;
        kept.push(`${channelId}-${id}`);
        if (kept.length > 300) kept.shift();
        if (ctx.settings.notify) ctx.toast(`${userName(msg.author)} deleted a message in ${channelLabel(channelId)}`);
        return true;
      };
      before(ctx, "MESSAGE_DELETE", (action) => {
        if (!keep(action.channelId, action.id)) return;
        render();
        return true;
      });
      before(ctx, "MESSAGE_DELETE_BULK", (action) => {
        if (!Array.isArray(action.ids)) return;
        action.ids = action.ids.filter((id: string) => !keep(action.channelId, id));
        render();
        return action.ids.length === 0;
      });
    },
  }),
  definePlugin({
    id: "ignoreEveryone",
    name: "Ignore @everyone",
    description: "Stops @everyone and @here from counting as a mention of you. Optionally ignores role mentions too.",
    authors: ["Corrode"],
    settings: {
      roles: { type: "boolean", label: "Also ignore role mentions", default: false },
      servers: { type: "string", label: "Only in these server IDs", description: "Comma separated. Empty means every server.", default: "" },
    },
    start(ctx) {
      before(ctx, "MESSAGE_CREATE", (action) => {
        const m = action.message;
        if (!m || action.optimistic) return;
        const only = ids(ctx.settings.servers);
        if (only.size && !only.has(m.guild_id ?? action.guildId)) return;
        if (m.mention_everyone) m.mention_everyone = false;
        if (ctx.settings.roles && Array.isArray(m.mention_roles) && m.mention_roles.length) m.mention_roles = [];
      });
    },
  }),
  definePlugin({
    id: "noReplyPings",
    name: "No Reply Pings",
    description: "Replies to your messages no longer count as a mention unless the person also @mentions you in the text.",
    authors: ["Corrode"],
    start(ctx) {
      before(ctx, "MESSAGE_CREATE", (action) => {
        const m = action.message;
        const me = myId();
        if (!m || !me || action.optimistic || m.referenced_message?.author?.id !== me || !Array.isArray(m.mentions)) return;
        if (new RegExp(`<@!?${escapeRegExp(me)}>`).test(String(m.content ?? ""))) return;
        m.mentions = m.mentions.filter((u: any) => (u?.id ?? u) !== me);
      });
    },
  }),
  definePlugin({
    id: "hideBlocked",
    name: "Hide Blocked Completely",
    description: 'Removes messages from people you blocked or ignored entirely, instead of showing "1 blocked message".',
    authors: ["Corrode"],
    start: (ctx) => filterMessages(ctx, (m) => !isBlocked(authorId(m)!) && !isIgnored(authorId(m)!)),
  }),
  definePlugin({
    id: "localMute",
    name: "Local Mute",
    description: "Hides messages from chosen people without blocking them. They can't tell.",
    authors: ["Corrode"],
    settings: { users: { type: "string", label: "User IDs", description: "Comma separated.", default: "" } },
    start: (ctx) => filterMessages(ctx, (m) => !ids(ctx.settings.users).has(authorId(m)!)),
  }),
  definePlugin({
    id: "hideBots",
    name: "Hide Bot Messages",
    description: "Hides messages from bots and webhooks.",
    authors: ["Corrode"],
    settings: {
      allow: { type: "string", label: "Bot IDs to keep", description: "Comma separated.", default: "" },
      servers: { type: "string", label: "Only in these server IDs", description: "Comma separated. Empty means everywhere.", default: "" },
    },
    start(ctx) {
      filterMessages(ctx, (m) => {
        if (!m.author?.bot && !m.webhook_id) return true;
        const only = ids(ctx.settings.servers);
        if (only.size && !only.has(m.guild_id ?? getChannel(m.channel_id)?.guild_id)) return true;
        return ids(ctx.settings.allow).has(m.author.id);
      });
    },
  }),
  definePlugin({
    id: "messageFilter",
    name: "Message Filter",
    description: "Hides incoming messages that contain words or phrases you choose.",
    authors: ["Corrode"],
    settings: {
      words: { type: "string", multiline: true, label: "Words or phrases", description: "Comma or line separated. Matches whole words, ignoring case.", default: "" },
      dms: { type: "boolean", label: "Also filter DMs", default: false },
    },
    start(ctx) {
      let cached = "";
      let re: RegExp | undefined;
      filterMessages(ctx, (m) => {
        const raw = String(ctx.settings.words ?? "");
        if (raw !== cached) {
          cached = raw;
          const words = splitList(raw);
          re = words.length ? new RegExp(`(?<![\\p{L}\\p{N}_])(${words.map(escapeRegExp).join("|")})(?![\\p{L}\\p{N}_])`, "iu") : undefined;
        }
        if (!re || (!ctx.settings.dms && !m.guild_id && isDm(m.channel_id))) return true;
        return !re.test(String(m.content ?? ""));
      });
    },
  }),
  definePlugin({
    id: "hideSystemMessages",
    name: "Hide System Messages",
    description: "Hides automatic messages such as joins, boosts and pin notices.",
    authors: ["Corrode"],
    settings: {
      joins: { type: "boolean", label: "Member joins", default: true },
      boosts: { type: "boolean", label: "Server boosts", default: true },
      pins: { type: "boolean", label: "Pin notices", default: false },
      threads: { type: "boolean", label: "Thread created notices", default: false },
      follows: { type: "boolean", label: "Channel follow notices", default: true },
      calls: { type: "boolean", label: "Call notices in DMs", default: false },
    },
    start(ctx) {
      const hidden = () => new Set(Object.keys(SYSTEM_TYPES).filter((k) => ctx.settings[k]).flatMap((k) => SYSTEM_TYPES[k]));
      before(ctx, "MESSAGE_CREATE", (action) => (action.message && !action.optimistic ? hidden().has(action.message.type) : undefined));
      before(ctx, "LOAD_MESSAGES_SUCCESS", (action) => {
        const types = hidden();
        if (Array.isArray(action.messages)) action.messages = action.messages.filter((m: any) => !types.has(m?.type));
      });
    },
  }),
  definePlugin({
    id: "embedBlocklist",
    name: "Embed Blocklist",
    description: "Removes link previews from chosen sites in messages you receive. The links themselves stay.",
    authors: ["Corrode"],
    settings: { domains: { type: "string", label: "Sites", description: "Comma separated, for example tenor.com, tiktok.com.", default: "" } },
    start(ctx) {
      mutateMessages(ctx, (m) => {
        const domains = splitList(ctx.settings.domains).map((d) => d.toLowerCase());
        if (!domains.length || !Array.isArray(m.embeds) || !m.embeds.length) return;
        m.embeds = m.embeds.filter((e: any) => {
          try {
            const host = new URL(e?.url ?? e?.provider?.url ?? "").hostname.toLowerCase();
            return !domains.some((d) => host === d || host.endsWith(`.${d}`));
          } catch {
            return true;
          }
        });
      });
    },
  }),
  definePlugin({
    id: "spoilerImages",
    name: "Spoiler Incoming Media",
    description: "Hides images and videos other people send behind a spoiler until you click them.",
    authors: ["Corrode"],
    settings: { channels: { type: "string", label: "Only in these channel IDs", description: "Comma separated. Empty means everywhere.", default: "" } },
    start(ctx) {
      mutateMessages(ctx, (m) => {
        if (!Array.isArray(m.attachments) || authorId(m) === myId()) return;
        const only = ids(ctx.settings.channels);
        if (only.size && !only.has(m.channel_id)) return;
        for (const a of m.attachments) {
          if (!a || typeof a.filename !== "string" || a.filename.startsWith("SPOILER_")) continue;
          if (/^(image|video)\//.test(a.content_type ?? "") || /\.(png|jpe?g|gif|webp|avif|mp4|webm|mov)$/i.test(a.filename)) {
            a.filename = `SPOILER_${a.filename}`;
            a.spoiler = true;
          }
        }
      });
    },
  }),
  definePlugin({
    id: "noStickersIncoming",
    name: "Strip Incoming Stickers",
    description: "Removes stickers from messages you receive, so they never load.",
    authors: ["Corrode"],
    start: (ctx) =>
      mutateMessages(ctx, (m) => {
        if (Array.isArray(m.sticker_items) && m.sticker_items.length) m.sticker_items = [];
        if (Array.isArray(m.stickers) && m.stickers.length) m.stickers = [];
      }),
  }),
  definePlugin({
    id: "quietTyping",
    name: "Typing Only From Friends",
    description: 'Only shows "is typing" for your friends.',
    authors: ["Corrode"],
    start: (ctx) => before(ctx, "TYPING_START", (action) => !!action.userId && action.userId !== myId() && !isFriend(action.userId)),
  }),
];
