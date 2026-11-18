import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
// @ts-expect-error plain JS helper
import { coreModules, installRuntime, loadChunk } from "./harness/fake-discord.js";
import { mergeRestored, snapshotMessage, toRawMessage } from "../src/builtins/special/messageLogger";
import { computePermissions, bit, VIEW_CHANNEL, ADMINISTRATOR, toBits } from "../src/builtins/special/permissions";
import { expandedFolderIds, readFolders } from "../src/builtins/special/folderSidebar";

const win = globalThis as any;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

/* Extra stores for these plugins: a server (50) with roles, members, channels and folders. */
const extraModules = {
  5: function (module: any, exports: any, require: any) {
    const make = (name: string, methods: Record<string, any>) => {
      const Store: any = class {
        getName() {
          return name;
        }
        addChangeListener() {}
        removeChangeListener() {}
      };
      Store.displayName = name;
      return Object.assign(new Store(), methods);
    };
    const channels = [
      { id: "10", type: 0, name: "general", guild_id: "50", position: 0, permissionOverwrites: {} },
      { id: "11", type: 0, name: "staff", guild_id: "50", position: 1, topic: "mods only", last_message_id: "175928847299117063", permissionOverwrites: { 50: { id: "50", type: 0, allow: 0n, deny: 1024n }, 61: { id: "61", type: 0, allow: 1024n, deny: 0n } } },
      { id: "12", type: 2, name: "vc", guild_id: "50", position: 2, permissionOverwrites: { 50: { id: "50", type: 0, allow: 0n, deny: 1024n }, 1: { id: "1", type: 1, allow: 1024n, deny: 0n } } },
    ];
    require.d(exports, {
      a: () => make("GuildRoleStore", { getRolesSnapshot: () => ({ 50: { id: "50", name: "@everyone", permissions: 1024n | 2048n, position: 0 }, 60: { id: "60", name: "Member", permissions: 64n, position: 1 }, 61: { id: "61", name: "Mod", permissions: 8192n, position: 2 } }) }),
      b: () => make("GuildMemberStore", { getMember: (g: string, u: string) => (u === "1" ? { userId: "1", roles: ["60"] } : u === "2" ? { userId: "2", roles: ["61"] } : undefined), getMemberIds: () => ["1", "2"] }),
      c: () => make("SortedGuildStore", { getGuildFolders: () => [{ guildIds: ["50"] }, { folderId: 7, folderName: "Games", folderColor: 0xff0000, guildIds: ["50", "51"] }, { folderId: 8, guildIds: [] }] }),
      d: () => make("ExpandedGuildFolderStore", { getExpandedFolders: () => new Set([7]) }),
      e: () => make("GuildReadStateStore", { hasUnread: (id: string) => id === "51", getMentionCount: (id: string) => (id === "51" ? 3 : 0) }),
      f: () => make("SelectedGuildStore", { getGuildId: () => "50" }),
      g: () => make("GuildStore", { getGuild: (id: string) => ({ 50: { id: "50", name: "Test Server", ownerId: "9" }, 51: { id: "51", name: "Other Place", icon: "abc" } } as any)[id] }),
      h: () => make("ChannelStore", { getChannel: (id: string) => channels.find((c) => c.id === id) ?? (id === "9" ? { id: "9", type: 1 } : undefined), getMutableGuildChannelsForGuild: () => Object.fromEntries(channels.map((c) => [c.id, c])) }),
    });
  },
};

let flux: typeof import("../src/api/flux");
let webpack: typeof import("../src/api/webpack");
let storage: typeof import("../src/api/storage");
let PluginManager: typeof import("../src/plugins/manager").PluginManager;
let mem: Map<string, string>;

async function setup(withExtras: boolean) {
  vi.resetModules();
  delete win.webpackChunkdiscord_app;
  for (const m of ["info", "debug", "log"] as const) vi.spyOn(console, m).mockImplementation(() => {});
  webpack = await import("../src/api/webpack");
  storage = await import("../src/api/storage");
  flux = await import("../src/api/flux");
  ({ PluginManager } = await import("../src/plugins/manager"));
  mem = new Map();
  storage.setBackend({ name: "test", get: (k) => mem.get(k), set: (k, v) => void mem.set(k, v), delete: (k) => void mem.delete(k) });
  webpack.init(win);
  installRuntime(win);
  // The test stores go first so their ChannelStore/GuildStore win over the harness's.
  if (withExtras) loadChunk(win, extraModules);
  loadChunk(win, coreModules);
}

async function startPlugin(id: string, settings: Record<string, unknown> = {}) {
  const { builtinPlugins } = await import("../src/builtins");
  const pm = new PluginManager();
  for (const p of builtinPlugins) pm.register(p);
  for (const [k, v] of Object.entries(settings)) pm.settingsStore(id).set(k, v);
  await pm.startAll();
  await pm.enable(id);
  expect(pm.get(id)!.running).toBe(true);
  return pm;
}

afterEach(() => {
  flux?.teardown();
  webpack?.teardown();
  document.head.innerHTML = "";
  document.body.innerHTML = "";
  vi.restoreAllMocks();
});

describe("message logger", () => {
  beforeEach(() => setup(false));

  it("keeps other people's deleted messages and logs them", async () => {
    const pm = await startPlugin("messageLogger");
    document.body.innerHTML = `<ol><li id="chat-messages-9-100"><div class="contents_x"><div id="message-content-100">hello</div></div></li></ol>`;
    const seen: string[] = [];
    flux.subscribe("MESSAGE_DELETE", (a) => seen.push(a.id));

    flux.dispatch({ type: "MESSAGE_DELETE", channelId: "9", id: "100" }); // Ana's DM message: kept
    flux.dispatch({ type: "MESSAGE_DELETE", channelId: "10", id: "101" }); // my own: goes through
    expect(seen).toEqual(["101"]);

    await sleep(250);
    const row = document.getElementById("chat-messages-9-100")!;
    expect(row.hasAttribute("data-corrode-deleted")).toBe(true);
    expect(row.querySelector(".corrode-ml-tag")?.textContent).toContain("deleted");

    // The ✕ on the tag removes it for real.
    (row.querySelector(".corrode-ml-tag button") as HTMLButtonElement).click();
    expect(seen).toEqual(["101", "100"]);

    pm.disable("messageLogger");
    expect(row.hasAttribute("data-corrode-deleted")).toBe(false);
  });

  it("logs edits, unopened channels and bulk deletes", async () => {
    const pm = await startPlugin("messageLogger");
    const seen: any[] = [];
    flux.subscribe("MESSAGE_DELETE", (a) => seen.push(a.id));
    flux.subscribe("MESSAGE_DELETE_BULK", (a) => seen.push(a.ids));

    flux.dispatch({ type: "MESSAGE_UPDATE", message: { id: "103", channel_id: "10", content: "hey (edited)" } });
    // A message from a channel Discord doesn't keep in its store, then its deletion.
    flux.dispatch({ type: "MESSAGE_CREATE", channelId: "9", message: { id: "555", channel_id: "9", author: { id: "2", username: "ana" }, content: "secret", timestamp: "2026-01-01T00:00:00.000Z", attachments: [] } });
    flux.dispatch({ type: "MESSAGE_DELETE", channelId: "9", id: "555" });
    expect(seen).toEqual(["555"]); // nothing on screen to keep, so it goes through
    flux.dispatch({ type: "MESSAGE_DELETE_BULK", channelId: "9", ids: ["100", "404"] });
    expect(seen).toEqual(["555", ["404"]]);

    pm.disable("messageLogger");
    const data = JSON.parse(mem.get("corrode:pluginData.messageLogger")!);
    expect(data.deleted.map((d: any) => [d.id, d.content])).toEqual([
      ["555", "secret"],
      ["100", "hello [x](https://example.org)"],
    ]);
    expect(data.edits[0]).toMatchObject({ id: "103", latest: "hey (edited)", versions: [{ content: "hey <@1>" }] });
  });

  it("puts logged messages back when history loads", async () => {
    await startPlugin("messageLogger");
    flux.dispatch({ type: "MESSAGE_DELETE", channelId: "9", id: "100" });
    const action: any = { type: "LOAD_MESSAGES_SUCCESS", channelId: "9", messages: [{ id: "120" }, { id: "90" }], hasMoreBefore: true, hasMoreAfter: false };
    flux.dispatch(action);
    expect(action.messages.map((m: any) => m.id)).toEqual(["120", "100", "90"]);
    expect(action.messages[1]).toMatchObject({ content: "hello [x](https://example.org)", author: { id: "2", username: "ana" } });
  });

  it("merges restored messages only where they belong", () => {
    const snap = (id: string) => snapshotMessage({ id, channel_id: "1", author: { id: "2", username: "a" }, content: id })!;
    const page = [{ id: "300" }, { id: "200" }];
    const ids = (ms: any[]) => ms.map((m) => m.id);
    expect(ids(mergeRestored(page, [snap("250"), snap("400"), snap("100")], { hasMoreBefore: true }))).toEqual(["400", "300", "250", "200"]);
    expect(ids(mergeRestored(page, [snap("400"), snap("100")], { isBefore: true, hasMoreBefore: false }))).toEqual(["300", "200", "100"]);
    expect(ids(mergeRestored(page, [snap("300")], {}))).toEqual(["300", "200"]);
    expect(ids(mergeRestored([], [snap("5")], {}))).toEqual(["5"]);
  });

  it("snapshots store records and rebuilds API messages", () => {
    const record = {
      id: "175928847299117063",
      channel_id: "9",
      author: { id: "2", username: "ana", globalName: "Ana", avatar: null },
      content: "hi",
      timestamp: { toISOString: () => "2016-04-30T11:18:25.796Z" },
      attachments: [{ id: "1", filename: "a.png", url: "https://cdn/a.png", proxyURL: "https://media/a.png", contentType: "image/png" }],
      messageReference: { messageId: "5", channelId: "9" },
    };
    const s = snapshotMessage(record)!;
    expect(s).toMatchObject({ sentAt: "2016-04-30T11:18:25.796Z", author: { globalName: "Ana" }, reference: { message_id: "5" } });
    expect(toRawMessage(s)).toMatchObject({ id: record.id, author: { global_name: "Ana" }, attachments: [{ proxy_url: "https://media/a.png", content_type: "image/png" }], message_reference: { message_id: "5" } });
    expect(snapshotMessage({ id: "1" })).toBeUndefined();
  });
});

describe("permissions", () => {
  const roles = [
    { id: "g", name: "@everyone", permissions: VIEW_CHANNEL | bit(11), position: 0 },
    { id: "mod", name: "Mod", permissions: bit(13), position: 2 },
    { id: "admin", name: "Admin", permissions: ADMINISTRATOR, position: 3 },
  ];
  const base = { guildId: "g", roles, userId: "u", memberRoleIds: ["mod"] };

  it("adds up roles and applies overwrites in Discord's order", () => {
    const server = computePermissions(base);
    expect(server & bit(13)).toBe(bit(13));
    const overwrites = [
      { id: "g", type: 0, allow: 0n, deny: VIEW_CHANNEL | bit(11) },
      { id: "mod", type: 0, allow: VIEW_CHANNEL, deny: 0n },
      { id: "u", type: 1, allow: 0n, deny: bit(13) },
    ];
    const ch = computePermissions({ ...base, overwrites });
    expect(ch & VIEW_CHANNEL).toBe(VIEW_CHANNEL); // role allow beats @everyone deny
    expect(ch & bit(11)).toBe(0n);
    expect(ch & bit(13)).toBe(0n); // member overwrite wins last
  });

  it("gives owners and administrators everything", () => {
    const overwrites = [{ id: "g", type: 0, allow: 0n, deny: VIEW_CHANNEL }];
    expect(computePermissions({ ...base, memberRoleIds: ["admin"], overwrites }) & VIEW_CHANNEL).toBe(VIEW_CHANNEL);
    expect(computePermissions({ ...base, memberRoleIds: [], ownerId: "u", overwrites }) & VIEW_CHANNEL).toBe(VIEW_CHANNEL);
    expect(computePermissions({ ...base, timedOut: true }) & bit(11)).toBe(0n);
  });

  it("reads every way Discord writes permissions", () => {
    expect(toBits("1024")).toBe(1024n);
    expect(toBits(1024)).toBe(1024n);
    expect(toBits(1024n)).toBe(1024n);
    expect(toBits(undefined)).toBe(0n);
    expect(toBits("x")).toBe(0n);
  });
});

describe("server plugins", () => {
  beforeEach(() => setup(true));

  it("reads folders", () => {
    const sorted = { getGuildFolders: () => [{ guildIds: ["1"] }, { folderId: 7, folderName: "G", guildIds: ["1", "2"] }] };
    expect(readFolders(sorted)).toEqual([{ id: "7", name: "G", color: null, guildIds: ["1", "2"] }]);
    expect([...expandedFolderIds({ getExpandedFolders: () => new Set([7]) })]).toEqual(["7"]);
    expect(readFolders(undefined)).toEqual([]);
  });

  it("draws open folders in their own column and toggles them", async () => {
    document.body.innerHTML = `<div class="base"><nav class="guilds_abc"></nav><div class="sidebar"></div></div>`;
    const toggles: any[] = [];
    flux.getDispatcher(); // resolve the dispatcher before the plugin starts
    await startPlugin("folderSidebar");
    flux.subscribe("TOGGLE_GUILD_FOLDER_EXPAND", (a) => toggles.push(a.folderId));
    await sleep(80);
    const col = document.querySelector(".corrode-folders") as HTMLElement;
    expect(col.previousElementSibling?.tagName).toBe("NAV");
    expect(col.hidden).toBe(false);
    const guilds = [...col.querySelectorAll<HTMLElement>(".cf-guild")];
    expect(guilds.map((g) => g.title)).toEqual(["Test Server", "Other Place"]);
    expect(guilds[0].hasAttribute("data-selected")).toBe(true);
    expect(guilds[1].querySelector(".cf-badge")?.textContent).toBe("3");
    expect(guilds[1].querySelector("img")?.getAttribute("src")).toContain("/icons/51/abc.webp");
    expect(document.querySelector('style[data-corrode="folderSidebar"]')?.textContent).toContain("folder-items-");
    (col.querySelector(".cf-head") as HTMLElement).click();
    expect(toggles).toEqual([7]);
  });

  it("finds channels hidden from you and who can see them", async () => {
    const { hiddenChannels } = await import("../src/builtins/special/hiddenChannels");
    const { describeAccess, guildRoles, permissionsOf } = await import("../src/builtins/special/permissions");
    // User 1 has only "Member": #staff is hidden, #vc is opened for them personally.
    expect(hiddenChannels("50").map((c) => c.name)).toEqual(["staff"]);
    const staff = hiddenChannels("50")[0];
    expect(describeAccess(staff, "50", guildRoles("50"))).toEqual({ allowed: ["@Mod"], denied: ["@everyone"] });
    expect(permissionsOf("50", "2", staff) & VIEW_CHANNEL).toBe(VIEW_CHANNEL);

    document.body.innerHTML = `<nav><div id="channels"></div></nav>`;
    await startPlugin("hiddenChannels");
    await sleep(80);
    const btn = document.querySelector(".corrode-hc-btn") as HTMLButtonElement;
    expect(btn.hidden).toBe(false);
    expect(btn.textContent).toContain("1 hidden channel");
    btn.click();
    const dlg = document.querySelector(".corrode-dlg")!;
    expect(dlg.textContent).toContain("staff");
    expect(dlg.textContent).toContain("mods only");
    dlg.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
    expect(document.querySelector(".corrode-dlg")).toBeNull();
  });

  it("explains a member's permissions", async () => {
    const { explain } = await import("../src/builtins/special/permissionViewer");
    const { getChannel } = await import("../src/builtins/lib/helpers");
    const why = explain("50", "1", getChannel("11"));
    expect(why.get("VIEW_CHANNEL")).toEqual({ granted: false, from: ["@everyone"], channel: ["denied for @everyone"] });
    expect(why.get("ADD_REACTIONS")).toMatchObject({ granted: true, from: ["@Member"] });
    const mod = explain("50", "2", getChannel("11"));
    expect(mod.get("VIEW_CHANNEL")).toMatchObject({ granted: true, channel: ["denied for @everyone", "allowed for @Mod"] });

    await startPlugin("permissionViewer");
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "p", code: "KeyP", ctrlKey: true, altKey: true }));
    const dlg = document.querySelector(".corrode-dlg")!;
    expect(dlg.textContent).toContain("Permissions in Test Server");
    expect(dlg.querySelectorAll(".corrode-pv-perm").length).toBeGreaterThan(40);
  });
});

describe("flux interceptors", () => {
  beforeEach(() => setup(false));
  it("all see an action even when an earlier one blocks it", () => {
    const seen: string[] = [];
    flux.intercept(() => true);
    flux.intercept((a) => void seen.push(a.type));
    const subscribed: string[] = [];
    flux.subscribe("X", (a) => subscribed.push(a.type));
    flux.dispatch({ type: "X" });
    expect(seen).toEqual(["X"]);
    expect(subscribed).toEqual([]);
  });
});
