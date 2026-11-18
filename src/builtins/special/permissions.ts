/**
 * Discord permission maths, done the way Discord's servers do it: server-wide permissions from
 * @everyone and the member's roles, then the channel's overwrites (@everyone, then roles, then
 * the member). Used by Permission Viewer and Hidden Channels.
 */
import { getChannel, getGuild, store } from "../lib/helpers";

export const PERMISSIONS: [name: string, bit: number, label: string][] = [
  ["CREATE_INSTANT_INVITE", 0, "Create invite"],
  ["KICK_MEMBERS", 1, "Kick members"],
  ["BAN_MEMBERS", 2, "Ban members"],
  ["ADMINISTRATOR", 3, "Administrator"],
  ["MANAGE_CHANNELS", 4, "Manage channels"],
  ["MANAGE_GUILD", 5, "Manage server"],
  ["ADD_REACTIONS", 6, "Add reactions"],
  ["VIEW_AUDIT_LOG", 7, "View audit log"],
  ["PRIORITY_SPEAKER", 8, "Priority speaker"],
  ["STREAM", 9, "Video and screen share"],
  ["VIEW_CHANNEL", 10, "View channel"],
  ["SEND_MESSAGES", 11, "Send messages"],
  ["SEND_TTS_MESSAGES", 12, "Send text-to-speech messages"],
  ["MANAGE_MESSAGES", 13, "Manage messages"],
  ["EMBED_LINKS", 14, "Embed links"],
  ["ATTACH_FILES", 15, "Attach files"],
  ["READ_MESSAGE_HISTORY", 16, "Read message history"],
  ["MENTION_EVERYONE", 17, "Mention @everyone, @here and all roles"],
  ["USE_EXTERNAL_EMOJIS", 18, "Use external emoji"],
  ["VIEW_GUILD_INSIGHTS", 19, "View server insights"],
  ["CONNECT", 20, "Connect to voice"],
  ["SPEAK", 21, "Speak"],
  ["MUTE_MEMBERS", 22, "Mute members"],
  ["DEAFEN_MEMBERS", 23, "Deafen members"],
  ["MOVE_MEMBERS", 24, "Move members"],
  ["USE_VAD", 25, "Use voice activity"],
  ["CHANGE_NICKNAME", 26, "Change nickname"],
  ["MANAGE_NICKNAMES", 27, "Manage nicknames"],
  ["MANAGE_ROLES", 28, "Manage roles"],
  ["MANAGE_WEBHOOKS", 29, "Manage webhooks"],
  ["MANAGE_GUILD_EXPRESSIONS", 30, "Manage expressions"],
  ["USE_APPLICATION_COMMANDS", 31, "Use application commands"],
  ["REQUEST_TO_SPEAK", 32, "Request to speak"],
  ["MANAGE_EVENTS", 33, "Manage events"],
  ["MANAGE_THREADS", 34, "Manage threads"],
  ["CREATE_PUBLIC_THREADS", 35, "Create public threads"],
  ["CREATE_PRIVATE_THREADS", 36, "Create private threads"],
  ["USE_EXTERNAL_STICKERS", 37, "Use external stickers"],
  ["SEND_MESSAGES_IN_THREADS", 38, "Send messages in threads"],
  ["USE_EMBEDDED_ACTIVITIES", 39, "Use activities"],
  ["MODERATE_MEMBERS", 40, "Time out members"],
  ["VIEW_CREATOR_MONETIZATION_ANALYTICS", 41, "View creator monetization analytics"],
  ["USE_SOUNDBOARD", 42, "Use soundboard"],
  ["CREATE_GUILD_EXPRESSIONS", 43, "Create expressions"],
  ["CREATE_EVENTS", 44, "Create events"],
  ["USE_EXTERNAL_SOUNDS", 45, "Use external sounds"],
  ["SEND_VOICE_MESSAGES", 46, "Send voice messages"],
  ["SEND_POLLS", 49, "Create polls"],
  ["USE_EXTERNAL_APPS", 50, "Use external apps"],
];

export const bit = (n: number) => 1n << BigInt(n);
export const VIEW_CHANNEL = bit(10);
export const ADMINISTRATOR = bit(3);
const ALL = PERMISSIONS.reduce((acc, [, b]) => acc | bit(b), 0n);

/** Discord sends permissions as bigint, number or string depending on where they come from. */
export function toBits(v: unknown): bigint {
  if (typeof v === "bigint") return v;
  if (typeof v === "number" && Number.isFinite(v)) return BigInt(Math.trunc(v));
  if (typeof v === "string" && /^\d+$/.test(v)) return BigInt(v);
  if (v && typeof v === "object" && "toString" in v) {
    const s = String(v);
    if (/^\d+$/.test(s)) return BigInt(s);
  }
  return 0n;
}

export interface Role {
  id: string;
  name: string;
  permissions: bigint;
  position: number;
  color?: number;
}
export interface Overwrite {
  id: string;
  /** 0 = role, 1 = member */
  type: number;
  allow: bigint;
  deny: bigint;
}

export interface PermissionInput {
  guildId: string;
  ownerId?: string;
  /** Every role in the server, including @everyone (whose id is the server id). */
  roles: Role[];
  userId: string;
  memberRoleIds: string[];
  overwrites?: Overwrite[];
  /** Users timed out keep only View channel and Read message history. */
  timedOut?: boolean;
}

/** The permissions a member has, server-wide or (with overwrites) in one channel. */
export function computePermissions(p: PermissionInput): bigint {
  if (p.ownerId && p.ownerId === p.userId) return ALL;
  const byId = new Map(p.roles.map((r) => [r.id, r]));
  let perms = byId.get(p.guildId)?.permissions ?? 0n;
  for (const id of p.memberRoleIds) perms |= byId.get(id)?.permissions ?? 0n;
  if (perms & ADMINISTRATOR) return ALL;
  if (p.overwrites?.length) {
    const ow = new Map(p.overwrites.map((o) => [o.id, o]));
    const everyone = ow.get(p.guildId);
    if (everyone) perms = (perms & ~everyone.deny) | everyone.allow;
    let allow = 0n;
    let deny = 0n;
    for (const id of p.memberRoleIds) {
      const o = ow.get(id);
      if (o && o.type !== 1) {
        allow |= o.allow;
        deny |= o.deny;
      }
    }
    perms = (perms & ~deny) | allow;
    const mine = ow.get(p.userId);
    if (mine && mine.type === 1) perms = (perms & ~mine.deny) | mine.allow;
  }
  if (p.timedOut) perms &= VIEW_CHANNEL | bit(16);
  return perms;
}

export const has = (perms: bigint, b: bigint) => (perms & b) === b;

/* ------------------------- reading Discord's stores ------------------------ */

/** Every role of a server, highest first. Works with the old (guild.roles) and new (GuildRoleStore) layouts. */
export function guildRoles(guildId: string): Role[] {
  const guild = getGuild(guildId);
  const roleStore = store("GuildRoleStore");
  const raw =
    roleStore?.getRolesSnapshot?.(guildId) ??
    roleStore?.getRoles?.(guildId) ??
    roleStore?.getSortedRoles?.(guildId) ??
    guild?.roles ??
    {};
  const list: any[] = Array.isArray(raw) ? raw : Object.values(raw);
  return list
    .filter((r) => r?.id != null)
    .map((r) => ({ id: String(r.id), name: String(r.name ?? "role"), permissions: toBits(r.permissions), position: Number(r.position) || 0, color: r.color }))
    .sort((a, b) => b.position - a.position);
}

export function channelOverwrites(channel: any): Overwrite[] {
  // Threads use their parent channel's permissions.
  const source = channel && (channel.type === 10 || channel.type === 11 || channel.type === 12) && channel.parent_id ? getChannel(channel.parent_id) ?? channel : channel;
  const raw = source?.permissionOverwrites ?? source?.permission_overwrites ?? {};
  const list: any[] = Array.isArray(raw) ? raw : Object.values(raw);
  return list.filter((o) => o?.id != null).map((o) => ({ id: String(o.id), type: Number(o.type) || 0, allow: toBits(o.allow), deny: toBits(o.deny) }));
}

export function memberRoleIds(guildId: string, userId: string): string[] {
  const m = store("GuildMemberStore")?.getMember?.(guildId, userId);
  return (m?.roles ?? []).map(String);
}

/** A member's permissions in a channel (or server-wide without one), read from Discord's stores. */
export function permissionsOf(guildId: string, userId: string, channel?: any): bigint {
  const guild = getGuild(guildId);
  const member = store("GuildMemberStore")?.getMember?.(guildId, userId);
  const until = member?.communicationDisabledUntil ?? member?.communication_disabled_until;
  return computePermissions({
    guildId,
    ownerId: guild?.ownerId ?? guild?.owner_id,
    roles: guildRoles(guildId),
    userId,
    memberRoleIds: (member?.roles ?? []).map(String),
    overwrites: channel ? channelOverwrites(channel) : undefined,
    timedOut: !!until && new Date(until).getTime() > Date.now(),
  });
}

/** Every channel Discord knows about in a server, including the ones hidden from you. */
export function allGuildChannels(guildId: string): any[] {
  const cs = store("ChannelStore");
  const raw = cs?.getMutableGuildChannelsForGuild?.(guildId) ?? cs?.getGuildChannels?.(guildId);
  if (raw) return Array.isArray(raw) ? raw : Object.values(raw);
  const all = cs?.getMutableGuildChannels?.() ?? cs?.getAllChannels?.() ?? {};
  return (Array.isArray(all) ? all : Object.values(all)).filter((c: any) => c?.guild_id === guildId);
}

/** Who a channel's overwrites let in or keep out, in words. */
export function describeAccess(channel: any, guildId: string, roles: Role[]): { allowed: string[]; denied: string[] } {
  const byId = new Map(roles.map((r) => [r.id, r]));
  const allowed: string[] = [];
  const denied: string[] = [];
  for (const o of channelOverwrites(channel)) {
    const name =
      o.type === 1
        ? `@${store("UserStore")?.getUser?.(o.id)?.username ?? o.id}`
        : o.id === guildId
          ? "@everyone"
          : `@${byId.get(o.id)?.name ?? o.id}`;
    if (o.allow & VIEW_CHANNEL) allowed.push(name);
    else if (o.deny & VIEW_CHANNEL) denied.push(name);
  }
  // Roles with Administrator see everything regardless of overwrites.
  for (const r of roles) if (r.permissions & ADMINISTRATOR && r.id !== guildId) allowed.push(`@${r.name} (admin)`);
  return { allowed: [...new Set(allowed)], denied };
}
