/**
 * Hidden Channels: lists the channels of a server that exist but you can't see, with their topic,
 * when they were last active, and which roles and people can see them.
 *
 * Discord sends every channel of a server to the client, then hides the ones you lack "View
 * channel" for. This reads them from the ChannelStore and checks the permission itself, so
 * nothing about the channel list changes and Discord never tries to open a channel you can't
 * read. A small "N hidden channels" button sits at the top of the channel list.
 */
import { definePlugin } from "../../plugins/types";
import { h } from "../../ui/dom";
import { currentGuildId, getChannel, getGuild, hotkey, myId, snowflakeDate, store, type Ctx } from "../lib/helpers";
import { formatTime, keepMounted, onStoresChange, openDialog } from "../lib/ui";
import { allGuildChannels, describeAccess, guildRoles, has, permissionsOf, VIEW_CHANNEL } from "./permissions";

const TYPE_ICON: Record<number, string> = { 0: "#", 2: "🔊", 4: "▾", 5: "📢", 13: "🎙", 15: "💬", 16: "🖼" };
const LISTED_TYPES = new Set([0, 2, 4, 5, 13, 15, 16]);

/** Whether you can see a channel. Prefers Discord's own answer, falls back to our own maths. */
export function canView(channel: any, guildId: string): boolean {
  const ps = store("PermissionStore");
  if (typeof ps?.can === "function") {
    try {
      const r = ps.can(VIEW_CHANNEL, channel);
      if (typeof r === "boolean") return r;
    } catch {
      /* fall through */
    }
  }
  const me = myId();
  return me ? has(permissionsOf(guildId, me, channel), VIEW_CHANNEL) : true;
}

export function hiddenChannels(guildId: string): any[] {
  return allGuildChannels(guildId)
    .filter((c) => c && LISTED_TYPES.has(c.type) && !canView(c, guildId))
    .sort((a, b) => (a.position ?? 0) - (b.position ?? 0));
}

const CSS = `
.corrode-hc-btn { all: unset; box-sizing: border-box; display: flex; align-items: center; gap: 6px; margin: 8px 8px 0; padding: 6px 10px; border-radius: 6px; cursor: pointer;
  font: 500 13px/18px var(--font-primary, "gg sans", system-ui, sans-serif); color: var(--channels-default, var(--text-muted, #949ba4));
  background: var(--background-modifier-hover, rgba(255,255,255,.04)); }
.corrode-hc-btn:hover { color: var(--interactive-hover, #fff); background: var(--background-modifier-selected, rgba(255,255,255,.08)); }
.corrode-hc-btn[hidden] { display: none; }
.corrode-hc-chan { display: flex; gap: 10px; }
.corrode-hc-icon { width: 20px; flex: 0 0 20px; text-align: center; color: var(--text-muted, #949ba4); font-weight: 700; }
.corrode-hc-tags { display: flex; flex-wrap: wrap; gap: 4px; margin-top: 6px; }
.corrode-hc-tag { padding: 1px 6px; border-radius: 4px; font-size: 12px; background: var(--background-modifier-accent, rgba(255,255,255,.08)); }
.corrode-hc-tag.no { text-decoration: line-through; opacity: .7; }
`;

export default definePlugin({
  id: "hiddenChannels",
  name: "Hidden Channels",
  description: "Shows which channels in a server are hidden from you, what they're about, when they were last active and who can see them.",
  authors: ["Corrode"],
  settings: {
    button: { type: "boolean", label: "Show a \"hidden channels\" button above the channel list", default: true },
    shortcut: { type: "string", label: "Shortcut", default: "ctrl+alt+h" },
  },
  actions: [{ label: "Show for this server", run: (ctx) => (ctx as any).__open?.() }],
  onSettingChange(_key, _value, ctx) {
    (ctx as any).__update?.();
  },
  start(ctx) {
    ctx.css.inject(CSS);
    const open = () => {
      const gid = currentGuildId();
      if (!gid) return void ctx.toast("Open a server first.");
      showHidden(ctx, gid);
    };
    (ctx as any).__open = open;
    hotkey(ctx, () => String(ctx.settings.shortcut), open);

    const btn = h("button", { class: "corrode-hc-btn", type: "button", hidden: true, onClick: open });
    keepMounted(ctx, "#channels", btn, "before");
    let last = "";
    const update = () => {
      const gid = currentGuildId();
      const n = ctx.settings.button && gid ? hiddenChannels(gid).filter((c) => c.type !== 4).length : 0;
      const key = `${gid}:${n}`;
      if (key === last) return;
      last = key;
      btn.hidden = n === 0;
      btn.replaceChildren(h("span", null, "🔒"), `${n} hidden channel${n === 1 ? "" : "s"}`);
    };
    (ctx as any).__update = () => ((last = ""), update());
    onStoresChange(ctx, ["SelectedGuildStore", "ChannelStore", "PermissionStore"], update);
  },
});

function showHidden(ctx: Ctx, guildId: string) {
  const guild = getGuild(guildId);
  const dlg = openDialog(ctx, `Hidden channels in ${guild?.name ?? "this server"}`, { width: 680 });
  const roles = guildRoles(guildId);
  const list = hiddenChannels(guildId);
  const categories = new Map<string, any[]>();
  for (const c of list) {
    if (c.type === 4) continue;
    const key = c.parent_id ?? "";
    if (!categories.has(key)) categories.set(key, []);
    categories.get(key)!.push(c);
  }
  if (!categories.size) {
    dlg.body.append(h("div", { class: "corrode-dlg-empty" }, "You can see every channel in this server."));
    return;
  }
  const search = h("input", { type: "text", placeholder: "Filter by name or topic…", style: "flex:1", "aria-label": "Filter" });
  dlg.tools.append(search, h("span", { class: "corrode-dlg-muted" }, `${list.filter((c) => c.type !== 4).length} hidden`));
  const order = [...categories.keys()].sort((a, b) => (getChannel(a)?.position ?? -1) - (getChannel(b)?.position ?? -1));
  const render = () => {
    const q = search.value.trim().toLowerCase();
    const out: HTMLElement[] = [];
    for (const cat of order) {
      const chans = categories.get(cat)!.filter((c) => !q || `${c.name}\n${c.topic ?? ""}`.toLowerCase().includes(q));
      if (!chans.length) continue;
      out.push(h("div", { class: "corrode-dlg-section" }, cat ? getChannel(cat)?.name ?? "Category" : "No category"));
      for (const c of chans) {
        const lastId = c.lastMessageId ?? c.last_message_id;
        const access = describeAccess(c, guildId, roles);
        out.push(
          h(
            "div",
            { class: "corrode-dlg-row corrode-hc-chan" },
            h("div", { class: "corrode-hc-icon" }, TYPE_ICON[c.type] ?? "#"),
            h(
              "div",
              { style: "flex:1;min-width:0" },
              h("div", { class: "corrode-dlg-row-head" }, h("b", null, c.name), h("span", { class: "corrode-dlg-muted" }, lastId ? `last message ${formatTime(snowflakeDate(lastId))}` : "no messages yet")),
              c.topic && h("div", { class: "corrode-dlg-text" }, c.topic),
              (access.allowed.length > 0 || access.denied.length > 0) &&
                h(
                  "div",
                  { class: "corrode-hc-tags", title: "Who this channel's permissions let in (crossed out: kept out)" },
                  access.allowed.map((n) => h("span", { class: "corrode-hc-tag" }, n)),
                  access.denied.map((n) => h("span", { class: "corrode-hc-tag no" }, n)),
                ),
            ),
          ),
        );
      }
    }
    dlg.body.replaceChildren(...(out.length ? out : [h("div", { class: "corrode-dlg-empty" }, "Nothing matches.")]));
  };
  search.addEventListener("input", render);
  render();
}
