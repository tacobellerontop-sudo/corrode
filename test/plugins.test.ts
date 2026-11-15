import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
// @ts-expect-error plain JS helper
import { coreModules, installRuntime, loadChunk } from "./harness/fake-discord.js";

const win = globalThis as any;
let webpack: typeof import("../src/api/webpack");
let storage: typeof import("../src/api/storage");
let flux: typeof import("../src/api/flux");
let PluginManager: typeof import("../src/plugins/manager").PluginManager;
let mem: Map<string, string>;

beforeEach(async () => {
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
  loadChunk(win, coreModules);
});
afterEach(() => {
  flux.teardown();
  webpack.teardown();
  document.head.innerHTML = "";
  vi.restoreAllMocks();
});

describe("storage", () => {
  it("persists namespaces as JSON and notifies subscribers", () => {
    const s = storage.createStore("ns");
    const seen: unknown[] = [];
    s.subscribe("a", (v) => seen.push(v));
    s.set("a", { x: 1 });
    expect(JSON.parse(mem.get("corrode:ns")!)).toEqual({ a: { x: 1 } });
    const v = s.get("a");
    v.x = 2; // copies, not references
    expect(s.get("a")).toEqual({ x: 1 });
    s.delete("a");
    expect(mem.has("corrode:ns")).toBe(false);
    expect(seen).toEqual([{ x: 1 }, undefined]);
    expect(s.get("missing", 5)).toBe(5);
  });
});

describe("plugin manager", () => {
  it("runs the full lifecycle: settings, css, flux, patches, cleanup", async () => {
    const pm = new PluginManager();
    const Typing = webpack.findByProps("startTyping");
    const seen: string[] = [];
    const stop = vi.fn();
    pm.register({
      id: "demo",
      name: "Demo",
      settings: { color: { type: "string", label: "Color", default: "red" } },
      css: (s: any) => `body { color: ${s.color}; }`,
      flux: { PING: (a: any) => seen.push(a.value) },
      start(ctx: any) {
        ctx.patcher.instead(Typing, "startTyping", () => {});
        ctx.flux.intercept((a: any) => a.type === "BLOCKED");
      },
      stop,
    });
    expect(pm.isEnabled("demo")).toBe(false);
    await pm.startAll();
    await pm.enable("demo");
    expect(pm.get("demo")!.running).toBe(true);

    const style = document.querySelector('style[data-corrode="demo"]')!;
    expect(style.textContent).toContain("red");
    pm.settingsStore("demo").set("color", "blue");
    expect(style.textContent).toContain("blue");

    flux.dispatch({ type: "PING", value: "one" });
    Typing.startTyping("c");
    expect(seen).toEqual(["one"]);
    expect(Typing.calls).toEqual([]);

    pm.disable("demo");
    expect(stop).toHaveBeenCalledOnce();
    expect(document.querySelector('style[data-corrode="demo"]')).toBeNull();
    flux.dispatch({ type: "PING", value: "two" });
    Typing.startTyping("c");
    expect(seen).toEqual(["one"]);
    expect(Typing.calls).toEqual(["c"]);

    // state persisted
    expect(new PluginManager().isEnabled("demo")).toBe(false);
  });

  it("records start() failures without throwing and cleans up", async () => {
    const pm = new PluginManager();
    vi.spyOn(console, "error").mockImplementation(() => {});
    pm.register({ id: "bad", name: "Bad", enabledByDefault: true, css: "a{}", start: () => Promise.reject(new Error("nope")) });
    await pm.startAll();
    const st = pm.get("bad")!;
    expect(st.running).toBe(false);
    expect(st.error).toBe("nope");
    expect(document.querySelector('style[data-corrode="bad"]')).toBeNull();
  });

  it("installs, persists and uninstalls external plugins", async () => {
    win.corrode = { definePlugin: (d: any) => d };
    const pm = new PluginManager();
    await pm.startAll();
    const id = await pm.installFromSource(`module.exports = corrode.definePlugin({ id: "ext", name: "Ext", start(ctx) { ctx.storage.set("ran", true); } });`);
    await pm.enable(id);
    expect(pm.get("ext")).toMatchObject({ source: "external", running: true });

    const pm2 = new PluginManager();
    pm2.loadExternal();
    expect(pm2.get("ext")?.enabled).toBe(true);

    pm.uninstall("ext");
    expect(pm.get("ext")).toBeUndefined();
    expect(storage.createStore("externalPlugins").keys()).toEqual([]);
    delete win.corrode;
  });

  it("loads the example external plugin", async () => {
    const { readFileSync } = await import("node:fs");
    win.corrode = { definePlugin: (d: any) => d, common: {} };
    const pm = new PluginManager();
    const id = await pm.installFromSource(readFileSync("examples/hello-world.plugin.js", "utf8"));
    expect(id).toBe("helloWorld");
    await pm.startAll();
    await pm.enable(id);
    expect(document.querySelector('style[data-corrode="helloWorld"]')?.textContent).toContain("#b7410e");
    delete win.corrode;
  });

  it("rejects invalid and duplicate ids", () => {
    const pm = new PluginManager();
    expect(() => pm.register({ id: "bad id!", name: "x" })).toThrow();
    pm.register({ id: "a", name: "A" });
    expect(() => pm.register({ id: "a", name: "A" })).toThrow(/already/);
  });

  it("registers source patches only for plugins enabled at boot", () => {
    storage.createStore("plugins").set("p", { enabled: true });
    const pm = new PluginManager();
    pm.register({ id: "p", name: "P", patches: [{ find: "x", replacement: { match: "x", replace: "y" } }] });
    pm.register({ id: "q", name: "Q", patches: [{ find: "x", replacement: { match: "x", replace: "y" } }] });
    expect(webpack.getPatches().map((p) => p.owner)).toEqual(["p"]);
    expect(pm.get("p")!.restartPending).toBe(false);
  });
});
