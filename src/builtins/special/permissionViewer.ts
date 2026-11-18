/**
 * Permission Viewer: shows what anyone in a server can do, server-wide or in one channel, and
 * which role or channel setting gives or takes away each permission. Also lists the server's
 * roles and what each one grants.
 */
import { definePlugin } from "../../plugins/types";
import { h } from "../../ui/dom";
import { currentChannelId, currentGuildId, getChannel, getGuild, getUser, hotkey, myId, store, type Ctx } from "../lib/helpers";
import { openDialog, tabs } from "../lib/ui";
import { ADMINISTRATOR, allGuildChannels, bit, channelOverwrites, guildRoles, memberRoleIds, PERMISSIONS, permissionsOf, type Role } from "./permissions";

const roleColor = (r: Role) => (r.color ? `#${r.color.toString(16).padStart(6, "0")}` : "inherit");

export interface Explanation {
  granted: boolean;
  /** Roles that give it server-wide. */
  from: string[];
  /** Channel settings that change it, e.g. "denied for @everyone". */
  channel: string[];
}

/** Why a member has (or lacks) each permission. */
export function explain(guildId: string, userId: string, channel?: any): Map<string, Explanation> {
  const guild = getGuild(guildId);
  const roles = guildRoles(guildId);
  const byId = new Map(roles.map((r) => [r.id, r]));
  const mine = new Set([guildId, ...memberRoleIds(guildId, userId)]);
  const perms = permissionsOf(guildId, userId, channel);
  const owner = (guild?.ownerId ?? guild?.owner_id) === userId;
  const admin = owner || roles.some((r) => mine.has(r.id) && r.permissions & ADMINISTRATOR);
  const overwrites = channel ? channelOverwrites(channel) : [];
  const out = new Map<string, Explanation>();
  for (const [name, b] of PERMISSIONS) {
    const flag = bit(b);
    const from = owner ? ["server owner"] : roles.filter((r) => mine.has(r.id) && r.permissions & flag).map((r) => (r.id === guildId ? "@everyone" : `@${r.name}`));
    if (!owner && admin && !from.length) from.push("Administrator");
    const chan: string[] = [];
    if (!admin) {
      for (const o of overwrites) {
        const applies = o.type === 1 ? o.id === userId : mine.has(o.id);
        if (!applies) continue;
        const who = o.type === 1 ? "this member" : o.id === guildId ? "@everyone" : `@${byId.get(o.id)?.name ?? o.id}`;
        if (o.allow & flag) chan.push(`allowed for ${who}`);
        if (o.deny & flag) chan.push(`denied for ${who}`);
      }
    }
    out.set(name, { granted: (perms & flag) === flag, from, channel: chan });
  }
  return out;
}

const CSS = `
.corrode-pv-grid { display: grid; grid-template-columns: 1fr; gap: 4px; }
.corrode-pv-perm { display: flex; gap: 10px; align-items: flex-start; padding: 6px 10px; border-radius: 6px; background: var(--background-secondary, #2b2d31); }
.corrode-pv-mark { width: 18px; flex: 0 0 18px; font-weight: 700; text-align: center; }
.corrode-pv-perm[data-on] .corrode-pv-mark { color: var(--status-positive, #23a55a); }
.corrode-pv-perm:not([data-on]) .corrode-pv-mark { color: var(--status-danger, #f23f43); }
.corrode-pv-perm:not([data-on]) .corrode-pv-name { color: var(--text-muted, #949ba4); }
.corrode-pv-roles { display: flex; flex-wrap: wrap; gap: 4px; margin: 4px 0 10px; }
.corrode-pv-role { display: inline-flex; align-items: center; gap: 4px; padding: 1px 8px; border-radius: 10px; font-size: 12px; background: var(--background-secondary, #2b2d31); }
.corrode-pv-role::before { content: ""; width: 8px; height: 8px; border-radius: 50%; background: var(--c, #99aab5); }
.corrode-pv-rolecard { cursor: pointer; }
.corrode-pv-rolecard summary { list-style: none; display: flex; gap: 8px; align-items: baseline; }
.corrode-pv-rolecard summary::-webkit-details-marker { display: none; }
`;

export default definePlugin({
  id: "permissionViewer",
  name: "Permission Viewer",
  description: "Shows what you or anyone else can do in a server or channel, and which role or channel setting is the reason. Also lists every role's permissions.",
  authors: ["Corrode"],
  settings: {
    shortcut: { type: "string", label: "Shortcut", default: "ctrl+alt+p" },
  },
  actions: [{ label: "Open for this channel", run: (ctx) => (ctx as any).__open?.() }],
  start(ctx) {
    ctx.css.inject(CSS);
    const open = () => {
      const gid = currentGuildId();
      if (!gid) return void ctx.toast("Open a server first. DMs don't have permissions.");
      showPermissions(ctx, gid);
    };
    (ctx as any).__open = open;
    hotkey(ctx, () => String(ctx.settings.shortcut), open);
  },
});

function memberName(guildId: string, userId: string): string {
  const u = getUser(userId);
  const nick = store("GuildMemberStore")?.getNick?.(guildId, userId) ?? store("GuildMemberStore")?.getMember?.(guildId, userId)?.nick;
  return nick ?? u?.globalName ?? u?.global_name ?? u?.username ?? userId;
}

/** Finds a member by id, name or nickname among the members Discord has loaded. */
function findMember(guildId: string, text: string): string | undefined {
  const t = text.trim().replace(/^@/, "").toLowerCase();
  if (!t) return undefined;
  if (/^\d{15,22}$/.test(t)) return t;
  const ids: string[] = (store("GuildMemberStore")?.getMemberIds?.(guildId) ?? store("GuildMemberStore")?.getMembers?.(guildId)?.map((m: any) => m.userId) ?? []).map(String);
  const names = (id: string) => {
    const u = getUser(id);
    return [u?.username, u?.globalName ?? u?.global_name, memberName(guildId, id)].filter(Boolean).map((s: string) => s.toLowerCase());
  };
  return ids.find((id) => names(id).includes(t)) ?? ids.find((id) => names(id).some((n) => n.startsWith(t)));
}

function showPermissions(ctx: Ctx, guildId: string) {
  const guild = getGuild(guildId);
  const dlg = openDialog(ctx, `Permissions in ${guild?.name ?? "this server"}`, { width: 720 });
  let tab: "member" | "roles" = "member";
  let userId = myId() ?? "";
  const here = currentChannelId();
  let channelId = here && getChannel(here)?.guild_id === guildId ? here : "";
  let onlyGranted = false;

  const who = h("input", { type: "text", placeholder: "Member: name, nickname or user ID", style: "flex:1;min-width:160px", "aria-label": "Member", value: memberName(guildId, userId) });
  who.addEventListener("change", () => {
    const id = findMember(guildId, who.value);
    if (!id) return void ctx.toast("No loaded member matches that. Try their user ID.");
    userId = id;
    who.value = memberName(guildId, id);
    render();
  });
  const channels = allGuildChannels(guildId)
    .filter((c) => c && c.type !== 4 && ![10, 11, 12].includes(c.type))
    .sort((a, b) => (a.position ?? 0) - (b.position ?? 0));
  const where = h(
    "select",
    { "aria-label": "Channel", onChange: (e: Event) => ((channelId = (e.target as HTMLSelectElement).value), render()) },
    h("option", { value: "" }, "Server-wide"),
    channels.map((c) => h("option", { value: c.id, selected: c.id === channelId }, `${c.type === 2 || c.type === 13 ? "🔊 " : "#"}${c.name}`)),
  );
  const granted = h("label", { class: "corrode-dlg-muted", style: "display:flex;gap:4px;align-items:center" }, h("input", { type: "checkbox", onChange: (e: Event) => ((onlyGranted = (e.target as HTMLInputElement).checked), render()) }), "Only what they have");
  const memberTools = h("div", { style: "display:contents" }, who, where, granted);
  dlg.tools.append(tabs([{ value: "member", label: "Member" }, { value: "roles", label: "Roles" }], tab, (v) => ((tab = v), render())), memberTools);

  const render = () => {
    memberTools.style.display = tab === "member" ? "contents" : "none";
    if (tab === "roles") return renderRoles();
    const channel = channelId ? getChannel(channelId) : undefined;
    const why = explain(guildId, userId, channel);
    const roles = guildRoles(guildId);
    const mine = new Set(memberRoleIds(guildId, userId));
    const out: HTMLElement[] = [];
    out.push(
      h("div", { class: "corrode-dlg-section" }, `${memberName(guildId, userId)}'s roles`),
      h(
        "div",
        { class: "corrode-pv-roles" },
        roles.filter((r) => mine.has(r.id)).map((r) => h("span", { class: "corrode-pv-role", style: `--c:${roleColor(r)}` }, r.name)),
        !mine.size && h("span", { class: "corrode-dlg-muted" }, "Only @everyone"),
      ),
      h("div", { class: "corrode-dlg-section" }, channel ? `In #${channel.name}` : "Server-wide"),
    );
    const grid = h("div", { class: "corrode-pv-grid" });
    for (const [name, , label] of PERMISSIONS) {
      const e = why.get(name)!;
      if (onlyGranted && !e.granted) continue;
      const reasons = [e.from.length ? `from ${e.from.join(", ")}` : "no role gives this", ...e.channel].join(" · ");
      grid.append(
        h(
          "div",
          { class: "corrode-pv-perm", "data-on": e.granted || undefined },
          h("div", { class: "corrode-pv-mark" }, e.granted ? "✓" : "✕"),
          h("div", null, h("div", { class: "corrode-pv-name" }, label), h("div", { class: "corrode-dlg-muted" }, reasons)),
        ),
      );
    }
    out.push(grid);
    dlg.body.replaceChildren(...out);
  };

  const renderRoles = () => {
    const roles = guildRoles(guildId);
    dlg.body.replaceChildren(
      ...roles.map((r) => {
        const perms = PERMISSIONS.filter(([, b]) => r.permissions & bit(b));
        const admin = !!(r.permissions & ADMINISTRATOR);
        return h(
          "details",
          { class: "corrode-dlg-row corrode-pv-rolecard" },
          h(
            "summary",
            null,
            h("b", { style: `color:${roleColor(r)}` }, r.id === guildId ? "@everyone" : r.name),
            h("span", { class: "corrode-dlg-muted" }, admin ? "Administrator: can do everything" : `${perms.length} permission${perms.length === 1 ? "" : "s"}`),
          ),
          h("div", { class: "corrode-pv-roles", style: "margin-top:8px" }, perms.map(([, , label]) => h("span", { class: "corrode-pv-role", style: `--c:${roleColor(r)}` }, label))),
        );
      }),
    );
  };
  render();
}
