/**
 * Folder Sidebar: server folders open in their own column next to the server list instead of
 * expanding inside it.
 *
 * It reads Discord's own folder data (SortedGuildStore) and which folders are open
 * (ExpandedGuildFolderStore), draws its own column after the server list, and opens/closes
 * folders by dispatching the same action Discord's folder button does. Unread dots, mention
 * counts and the selected server come from Discord's stores too, so the column stays in sync.
 */
import { definePlugin } from "../../plugins/types";
import { h } from "../../ui/dom";
import { clamp, currentGuildId, store, type Ctx } from "../lib/helpers";
import { guildIconUrl, initials, keepMounted, navigate, onStoresChange } from "../lib/ui";

export interface Folder {
  id: string;
  name: string;
  color?: number | null;
  guildIds: string[];
}

/** Discord's folder list, without the loose servers (a "folder" with no id is a single server). */
export function readFolders(sorted: any): Folder[] {
  const raw = sorted?.getGuildFolders?.() ?? [];
  const out: Folder[] = [];
  for (const f of Array.isArray(raw) ? raw : []) {
    const id = f?.folderId ?? f?.id;
    const guildIds: string[] = (f?.guildIds ?? f?.guild_ids ?? []).map(String);
    if (id == null || !guildIds.length) continue;
    out.push({ id: String(id), name: f.folderName ?? f.name ?? "", color: f.folderColor ?? f.color ?? null, guildIds });
  }
  return out;
}

export function expandedFolderIds(expanded: any): Set<string> {
  const v = expanded?.getExpandedFolders?.();
  if (v instanceof Set || Array.isArray(v)) return new Set([...v].map(String));
  return new Set();
}

const hex = (n: number | null | undefined) => (typeof n === "number" && n >= 0 ? `#${n.toString(16).padStart(6, "0").slice(-6)}` : "");

const CSS = `
.corrode-folders { --cf-w: 72px; width: var(--cf-w); flex: 0 0 var(--cf-w); height: 100%; box-sizing: border-box; overflow-y: auto; overflow-x: hidden; scrollbar-width: none;
  display: flex; flex-direction: column; align-items: center; gap: 8px; padding: 12px 0 80px; background: var(--background-tertiary, var(--bg-base-tertiary, #1e1f22));
  border-left: 1px solid var(--background-modifier-accent, rgba(255,255,255,.06)); font-family: var(--font-primary, "gg sans", system-ui, sans-serif); }
.corrode-folders::-webkit-scrollbar { display: none; }
.corrode-folders[hidden] { display: none; }
.cf-folder { --cf-c: var(--brand-500, #5865f2); display: flex; flex-direction: column; align-items: center; gap: 8px; width: calc(100% - 16px); padding: 6px 0 8px; border-radius: 16px;
  background: color-mix(in srgb, var(--cf-c) 16%, transparent); }
.cf-head { all: unset; box-sizing: border-box; width: 100%; padding: 2px 4px; cursor: pointer; text-align: center; font-size: 10px; font-weight: 700; line-height: 14px; text-transform: uppercase;
  color: var(--cf-c); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.cf-head:hover { text-decoration: underline; }
.cf-folder[data-closed] { padding-bottom: 6px; }
.cf-guild { all: unset; box-sizing: border-box; position: relative; width: 48px; height: 48px; border-radius: 50%; cursor: pointer; display: flex; align-items: center; justify-content: center;
  background: var(--background-primary, #313338); color: var(--text-normal, #dbdee1); font-size: 14px; font-weight: 600; transition: border-radius .15s, background-color .15s; }
.cf-guild:hover, .cf-guild[data-selected] { border-radius: 16px; background: var(--brand-500, #5865f2); color: #fff; }
.cf-guild img { width: 100%; height: 100%; border-radius: inherit; object-fit: cover; }
.cf-guild::before { content: ""; position: absolute; left: -12px; top: 50%; width: 4px; height: 0; border-radius: 0 4px 4px 0; background: var(--header-primary, #f2f3f5); transform: translateY(-50%); transition: height .15s; }
.cf-guild[data-unread]::before { height: 8px; }
.cf-guild:hover::before { height: 20px; }
.cf-guild[data-selected]::before { height: 40px; }
.cf-guild[data-muted] { opacity: .55; }
.cf-badge { position: absolute; right: -4px; bottom: -4px; min-width: 16px; height: 16px; padding: 0 4px; box-sizing: content-box; border-radius: 12px; border: 3px solid var(--background-tertiary, #1e1f22);
  background: var(--status-danger, #f23f43); color: #fff; font-size: 11px; font-weight: 700; line-height: 16px; text-align: center; }
`;

export default definePlugin({
  id: "folderSidebar",
  name: "Folder Sidebar",
  description: "Opens server folders in their own column next to the server list, with unread dots and mention counts, instead of expanding them inside the list.",
  authors: ["Corrode"],
  settings: {
    show: {
      type: "select",
      label: "Show in the column",
      default: "open",
      options: [
        { label: "Folders I open", value: "open" },
        { label: "Every folder (open ones show their servers)", value: "all" },
      ],
    },
    hideInList: { type: "boolean", label: "Don't also expand open folders in the server list", default: true },
    oneAtATime: { type: "boolean", label: "Only one folder open at a time", default: false },
    closeOnPick: { type: "boolean", label: "Close the folder after picking a server", default: false },
    names: { type: "boolean", label: "Show folder names", default: true },
    width: { type: "number", label: "Column width (pixels)", default: 72, min: 56, max: 160 },
  },
  css: (s) => (s.hideInList ? `ul[id^="folder-items-"] { display: none !important; }` : ""),
  onSettingChange(_key, _value, ctx) {
    (ctx as any).__render?.();
  },
  start(ctx) {
    ctx.css.inject(CSS);
    const column = h("div", { class: "corrode-folders", role: "navigation", "aria-label": "Open server folders", hidden: true });
    keepMounted(ctx, 'nav[class*="guilds"]', column, "after");

    const toggleFolder = (folderId: string) => {
      try {
        ctx.flux.dispatch({ type: "TOGGLE_GUILD_FOLDER_EXPAND", folderId: isNaN(Number(folderId)) ? folderId : Number(folderId) });
      } catch (err) {
        ctx.logger.error("Couldn't open or close the folder", err);
      }
    };

    let lastKey = "";
    const render = () => {
      const s = ctx.settings as any;
      column.style.setProperty("--cf-w", `${clamp(s.width, 56, 160, 72)}px`);
      const folders = readFolders(store("SortedGuildStore"));
      const open = expandedFolderIds(store("ExpandedGuildFolderStore"));
      const shown = s.show === "all" ? folders : folders.filter((f) => open.has(f.id));
      const reads = store("GuildReadStateStore");
      const guilds = store("GuildStore");
      const muted = store("UserGuildSettingsStore");
      const selected = currentGuildId();

      const state = shown.map((f) => ({
        f,
        open: open.has(f.id),
        guilds: open.has(f.id)
          ? f.guildIds
              .map((id) => guilds?.getGuild?.(id))
              .filter(Boolean)
              .map((g: any) => ({
                g,
                unread: !!reads?.hasUnread?.(g.id),
                mentions: Number(reads?.getMentionCount?.(g.id)) || 0,
                muted: !!muted?.isMuted?.(g.id),
              }))
          : [],
      }));
      const key = JSON.stringify([s.names, selected, state.map((x) => [x.f.id, x.f.name, x.f.color, x.open, x.guilds.map((y) => [y.g.id, y.g.name, y.g.icon, y.unread, y.mentions, y.muted])])]);
      if (key === lastKey) return;
      lastKey = key;

      column.hidden = state.length === 0;
      column.replaceChildren(
        ...state.map(({ f, open: isOpen, guilds: list }) => {
          const label = f.name || "Folder";
          return h(
            "div",
            { class: "cf-folder", style: hex(f.color) ? `--cf-c:${hex(f.color)}` : undefined, "data-closed": !isOpen || undefined },
            h("button", { class: "cf-head", type: "button", title: isOpen ? `Close ${label}` : `Open ${label}`, onClick: () => toggleFolder(f.id) }, s.names ? label : isOpen ? "▾" : "▸"),
            list.map(({ g, unread, mentions, muted: isMuted }) => {
              const icon = guildIconUrl(g);
              return h(
                "button",
                {
                  class: "cf-guild",
                  type: "button",
                  title: g.name,
                  "aria-label": `${g.name}${mentions ? `, ${mentions} mentions` : unread ? ", unread" : ""}`,
                  "data-selected": g.id === selected || undefined,
                  "data-unread": unread || undefined,
                  "data-muted": isMuted || undefined,
                  onClick: () => {
                    openGuild(g.id);
                    if ((ctx.settings as any).closeOnPick) toggleFolder(f.id);
                  },
                },
                icon ? h("img", { src: icon, alt: "", loading: "lazy", draggable: false }) : initials(g.name),
                mentions > 0 && h("span", { class: "cf-badge" }, mentions > 99 ? "99+" : String(mentions)),
              );
            }),
          );
        }),
      );
    };
    (ctx as any).__render = () => {
      lastKey = "";
      render();
    };
    onStoresChange(ctx, ["SortedGuildStore", "ExpandedGuildFolderStore", "GuildReadStateStore", "SelectedGuildStore", "GuildStore", "UserGuildSettingsStore"], render);

    // "Only one folder open at a time": when a folder opens, close the others.
    let closing = false;
    ctx.flux.subscribe("TOGGLE_GUILD_FOLDER_EXPAND", (action) => {
      if (closing || !(ctx.settings as any).oneAtATime) return;
      setTimeout(() => {
        const opened = String(action.folderId);
        const open = expandedFolderIds(store("ExpandedGuildFolderStore"));
        if (!open.has(opened)) return;
        closing = true;
        try {
          for (const id of open) if (id !== opened) toggleFolder(id);
        } finally {
          closing = false;
        }
      }, 0);
    });
  },
});

/** Opens a server at the channel you were last in there, like clicking it in the server list. */
export function openGuild(guildId: string) {
  const selected = store("SelectedChannelStore");
  const channelId =
    selected?.getLastSelectedChannelId?.(guildId) ??
    selected?.getMostRecentSelectedTextChannelId?.(guildId) ??
    store("GuildChannelStore")?.getDefaultChannel?.(guildId)?.id;
  navigate(channelId ? `/channels/${guildId}/${channelId}` : `/channels/${guildId}`);
}
